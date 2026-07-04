//! 全盘音频路径发现层（BETA-01A）。
//!
//! 复用系统索引快速枚举**全盘**音频路径（仅路径，不读内容），交 [`MusicIndex::index_paths`]
//! （`crate::MusicIndex`）提取入库。Windows 用 Everything `es.exe`、macOS 用 Spotlight `mdfind`。
//! 发现层是**可选加速**——工具不可用时 `discover_audio` 返 [`DiscoveryError::Unavailable`]，
//! 调用方回退目录扫描（守 PROJECT「不强制依赖 Everything」）。

use std::path::PathBuf;

#[cfg(any(windows, target_os = "macos"))]
use std::process::Command;

/// 发现层错误。
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// 发现工具不可用（未安装 / 不在 PATH）→ 调用方应回退目录扫描。
    #[error("发现器不可用: {detail}")]
    Unavailable {
        /// 详细原因。
        detail: String,
    },
    /// 工具存在但执行失败。
    #[error("发现失败: {detail}")]
    Failed {
        /// 详细原因。
        detail: String,
    },
}

/// 全盘音频路径发现（仅枚举路径，不读内容）。
pub trait AudioDiscovery: std::fmt::Debug + Send + Sync {
    /// 枚举系统内所有音频文件路径。工具不可用返回 [`DiscoveryError::Unavailable`]。
    fn discover_audio(&self) -> Result<Vec<PathBuf>, DiscoveryError>;
}

/// 平台默认发现器（Windows Everything / macOS Spotlight）；不支持的平台返回 `None`。
/// 注：返回 `Some` 不代表工具已安装——实际可用性在 [`AudioDiscovery::discover_audio`] 判定。
#[must_use]
pub fn default_audio_discovery() -> Option<Box<dyn AudioDiscovery>> {
    #[cfg(windows)]
    {
        Some(Box::new(EverythingDiscovery))
    }
    #[cfg(target_os = "macos")]
    {
        Some(Box::new(SpotlightDiscovery))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// 把发现工具的输出文本解析为路径列表：去 BOM、按行、trim、滤空。纯函数。
///
/// 调用点在 `#[cfg(windows)]` EverythingDiscovery + `#[cfg(target_os = "macos")]`
/// SpotlightDiscovery + 同模块 `#[cfg(test)]` 单测三处；Linux build 时 lib target
/// 下两个 cfg 块都不编译，函数变 dead。`cargo clippy ... -D warnings` 在 ubuntu runner
/// 上会把 rustc 的 `dead_code` warn 升 error（[CI workflow ci.yml](../../.github/workflows/ci.yml)
/// 在 ubuntu-22.04 上首跑发现）。这里显式 `allow(dead_code)` 在非 windows/macos 平台
/// 上容忍——函数在所有平台仍编译以供 test 用，且 Mac/Win 真实调用点行为不变。
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub(crate) fn parse_paths_lines(text: &str) -> Vec<PathBuf> {
    text.lines()
        .map(|line| line.trim_start_matches('\u{feff}').trim())
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// 音频扩展名查询（Everything `ext:` 语法）。
#[cfg(windows)]
const AUDIO_EXT_QUERY: &str = "ext:mp3;flac;m4a;aac;ogg;opus;wav;wma;aiff;aif;ape";

/// Windows：Everything CLI（`es.exe`）全盘枚举。
#[cfg(windows)]
#[derive(Debug)]
pub struct EverythingDiscovery;

#[cfg(windows)]
impl AudioDiscovery for EverythingDiscovery {
    fn discover_audio(&self) -> Result<Vec<PathBuf>, DiscoveryError> {
        // 经 -export-txt -utf8-bom 导出（规避中文 Windows GBK stdout 破坏 CJK 路径）。
        let export = std::env::temp_dir().join("locifind_audio_discovery.txt");
        let ran = es_candidates()
            .into_iter()
            .any(|es| run_es_export(&es, &export));
        if !ran {
            return Err(DiscoveryError::Unavailable {
                detail: "es.exe（Everything CLI）不可用".to_owned(),
            });
        }
        let bytes = std::fs::read(&export).map_err(|e| DiscoveryError::Failed {
            detail: format!("读导出文件失败: {e}"),
        })?;
        let text =
            String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes));
        Ok(parse_paths_lines(&text))
    }
}

/// 候选 es.exe：PATH（`es.exe`）+ winget 安装路径（经 `LOCALAPPDATA`）。
#[cfg(windows)]
fn es_candidates() -> Vec<String> {
    let mut v = vec!["es.exe".to_owned()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let p = std::path::Path::new(&local)
            .join("Microsoft")
            .join("WinGet")
            .join("Packages")
            .join("voidtools.Everything.Cli_Microsoft.Winget.Source_8wekyb3d8bbwe")
            .join("es.exe");
        v.push(p.to_string_lossy().into_owned());
    }
    v
}

/// 调 es.exe 导出全盘音频路径；spawn 成功且退出码 0 返 true。
/// `CREATE_NO_WINDOW`：索引枚举时 spawn es.exe 不闪现控制台黑框（与 everything 搜索路径一致）。
#[cfg(windows)]
fn run_es_export(es: &str, export: &std::path::Path) -> bool {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    Command::new(es)
        .args([
            AUDIO_EXT_QUERY,
            "-export-txt",
            &export.to_string_lossy(),
            "-utf8-bom",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

/// macOS：Spotlight（`mdfind`）全盘枚举。
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct SpotlightDiscovery;

#[cfg(target_os = "macos")]
impl AudioDiscovery for SpotlightDiscovery {
    fn discover_audio(&self) -> Result<Vec<PathBuf>, DiscoveryError> {
        let output = Command::new("mdfind")
            .arg("kMDItemContentTypeTree == \"public.audio\"")
            .output()
            .map_err(|e| DiscoveryError::Unavailable {
                detail: format!("mdfind 不可用: {e}"),
            })?;
        if !output.status.success() {
            return Err(DiscoveryError::Failed {
                detail: "mdfind 非零退出".to_owned(),
            });
        }
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_paths_lines(&text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_strips_bom_blank_and_trims() {
        let text = "\u{feff}C:\\Music\\周华健-朋友.mp3\r\n\n  C:\\b.flac  \n";
        let paths = parse_paths_lines(text);
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("C:\\Music\\周华健-朋友.mp3"));
        assert_eq!(paths[1], PathBuf::from("C:\\b.flac"));
    }

    #[test]
    fn parse_empty_yields_empty() {
        assert!(parse_paths_lines("").is_empty());
        assert!(parse_paths_lines("\n  \n\u{feff}\n").is_empty());
    }

    #[test]
    fn default_discovery_does_not_panic() {
        // Windows/macOS 返回 Some，其它返回 None；本测只验不 panic。
        let _ = default_audio_discovery();
    }
}
