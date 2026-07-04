//! 图片 OCR 引擎层（BETA-03）。
//!
//! 在 `unsafe_code = forbid` 约束下，原生 OCR API（Windows.Media.Ocr = WinRT / macOS Vision
//! = Obj-C FFI）不能直接调用 → 沿用项目 **shell-out 拿结构化输出** 套路（WindowsSearch 的
//! ADODB、Everything 的 es.exe、Spotlight 的 mdfind）：
//! - [`WindowsOcrEngine`]：`powershell` 调内嵌 `.ps1` 经 WinRT 识别（图片路径走环境变量传入，
//!   脚本不插值用户数据 → 杜绝注入）；
//! - [`TesseractOcrEngine`]：shell-out `tesseract` 兜底（跨平台，需用户装）；
//! - macOS Vision 留后续（trait 已抽象）。
//!
//! 设计见 `docs/superpowers/specs/2026-06-02-beta-03-ocr-image-index-design.md`。

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::IndexError;

/// 单图 OCR 进程超时（大图 WinRT/Tesseract 识别可能数秒）。
const OCR_TIMEOUT: Duration = Duration::from_secs(30);

/// 单图 OCR 引擎。跨平台 + 跨实现（Windows WinRT / Tesseract / 后续 macOS Vision）。
pub trait OcrEngine: Send + Sync + std::fmt::Debug {
    /// 识别单张图片的全部文字（已做 CJK 空格折叠）。
    ///
    /// 失败（解码错 / 引擎错 / 超时 / 进程缺失）返回 [`IndexError::Tag`]，由增量循环计
    /// failed、跳过、不中断整轮。
    fn recognize(&self, image: &Path) -> Result<String, IndexError>;

    /// 引擎名（trace / 诊断用）。
    fn name(&self) -> &'static str;
}

/// 构造 [`IndexError::Tag`]（OCR 是按文件粒度的提取失败语义）。
fn tag_err(path: &Path, detail: impl Into<String>) -> IndexError {
    IndexError::Tag {
        path: path.to_string_lossy().into_owned(),
        detail: detail.into(),
    }
}

/// 折叠 OCR 文字里 **相邻 CJK 表意字符之间** 的空白；拉丁词间空格保留。
///
/// Windows.Media.Ocr 在 CJK 字符间插空格（`会 议 纪 要`），不折叠会破坏 trigram FTS 对
/// `会议` 的匹配。拉丁文 `Hello World` 的词间空格必须保留。
#[must_use]
pub fn normalize_ocr_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            // 找到这段连续空白的下一个非空白字符。
            let mut j = i + 1;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            let prev = out.chars().last();
            let next = chars.get(j).copied();
            // 两侧都是 CJK → 丢弃整段空白；否则保留单个空格。
            if matches!(prev, Some(p) if is_cjk(p)) && matches!(next, Some(n) if is_cjk(n)) {
                // skip
            } else {
                out.push(' ');
            }
            i = j;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// 是否 CJK 表意字符（统一表意 + 扩展 A + 兼容表意），用于空格折叠判定。
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF   // 扩展 A
        | 0x4E00..=0x9FFF // 统一表意
        | 0xF900..=0xFAFF // 兼容表意
    )
}

/// spawn 外部 OCR 进程、超时 kill、成功返回 stdout（按 UTF-8 lossy 解码）。
/// 失败统一映射为按图片粒度的 [`IndexError::Tag`]（计 failed，不中断整轮）。
fn spawn_capture_stdout(mut cmd: Command, image: &Path) -> Result<String, IndexError> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    no_window(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| tag_err(image, format!("spawn OCR 进程失败: {e}")))?;
    let start = Instant::now();

    loop {
        if child
            .try_wait()
            .map_err(|e| tag_err(image, e.to_string()))?
            .is_some()
        {
            let output = child
                .wait_with_output()
                .map_err(|e| tag_err(image, e.to_string()))?;
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            return Err(tag_err(
                image,
                format!(
                    "OCR 进程失败: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        if start.elapsed() >= OCR_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(tag_err(image, "OCR 超时"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 给 `Command` 加 `CREATE_NO_WINDOW`（Windows）避免 spawn 时闪现控制台黑框；其他平台 no-op。
fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

// ===== 引擎选择 =====

/// 引擎优先级判定结果（纯逻辑，便于单测，不真调系统）。
#[derive(Debug, PartialEq, Eq)]
enum EnginePick {
    Windows,
    Tesseract,
    None,
}

/// 纯优先级逻辑：Windows 原生优先 → Tesseract 兜底 → 都无则 None。
fn pick_engine(win_available: bool, tess_available: bool) -> EnginePick {
    if win_available {
        EnginePick::Windows
    } else if tess_available {
        EnginePick::Tesseract
    } else {
        EnginePick::None
    }
}

/// 选默认 OCR 引擎：Windows.Media.Ocr 可用 → [`WindowsOcrEngine`]；
/// 否则 PATH 上有 `tesseract` → [`TesseractOcrEngine`]；都没有 → `None`（图片索引优雅跳过）。
#[must_use]
pub fn default_ocr_engine() -> Option<Box<dyn OcrEngine>> {
    let win_available = windows_ocr_available();
    let tess_available = TesseractOcrEngine::detect();
    match pick_engine(win_available, tess_available) {
        #[cfg(windows)]
        EnginePick::Windows => Some(Box::new(WindowsOcrEngine::new())),
        // 非 Windows 永不选 Windows（`windows_ocr_available` 恒 false），但 match 需穷尽。
        #[cfg(not(windows))]
        EnginePick::Windows => None,
        EnginePick::Tesseract => Some(Box::new(TesseractOcrEngine::new())),
        EnginePick::None => None,
    }
}

/// 是否有可用的 Windows.Media.Ocr 识别语言（非 Windows 恒 false）。
fn windows_ocr_available() -> bool {
    #[cfg(windows)]
    {
        WindowsOcrEngine::detect()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

// ===== Windows.Media.Ocr（经 PowerShell WinRT）=====

/// 内嵌 OCR 脚本（spike 验证过的 WinRT 路径）。
#[cfg(windows)]
const WIN_OCR_SCRIPT: &str = include_str!("ocr/win_ocr.ps1");

/// Windows 原生 OCR 引擎（PowerShell + Windows.Media.Ocr WinRT）。
///
/// 经 `-EncodedCommand`（base64 UTF-16LE）传脚本：避免 `-File`/stdin 把整段脚本一次性
/// 编译（导致类型字面量在 `Add-Type` 之前解析而找不到类型），也免去临时文件 / 引号转义。
/// 图片路径走环境变量 `LOCIFIND_OCR_IMAGE`（脚本不插值用户数据 → 杜绝注入）。
#[cfg(windows)]
#[derive(Debug)]
pub struct WindowsOcrEngine {
    /// 预编码的 `-EncodedCommand` 实参（构造时算一次，复用）。
    encoded_command: String,
}

#[cfg(windows)]
impl WindowsOcrEngine {
    /// 探测：本机是否装有可用的 OCR 识别语言。
    #[must_use]
    pub fn detect() -> bool {
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Windows.Media.Ocr.OcrEngine,Windows.Media.Ocr,ContentType=WindowsRuntime] | Out-Null; \
             if ([Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages.Count -gt 0) { exit 0 } else { exit 1 }",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        no_window(&mut cmd);
        matches!(cmd.status(), Ok(s) if s.success())
    }

    /// 构造（预编码脚本，无 IO）。
    #[must_use]
    pub fn new() -> Self {
        Self {
            encoded_command: encode_powershell_command(WIN_OCR_SCRIPT),
        }
    }
}

#[cfg(windows)]
impl Default for WindowsOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl OcrEngine for WindowsOcrEngine {
    fn recognize(&self, image: &Path) -> Result<String, IndexError> {
        // WinRT `GetFileFromPathAsync` 不接受正斜杠路径（报「指定的路径无效」）——
        // daemon TOML 配置的 roots 常写 `/`，walkdir 拼出混合分隔符 path，图片 OCR
        // 全数失败（BETA-40 排查实锤）。统一归一为 `\` 再传给脚本。
        let native_path = image.to_string_lossy().replace('/', "\\");
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-EncodedCommand",
        ])
        .arg(&self.encoded_command)
        .env("LOCIFIND_OCR_IMAGE", native_path);
        let raw = spawn_capture_stdout(cmd, image)?;
        Ok(normalize_ocr_text(&raw))
    }

    fn name(&self) -> &'static str {
        "Windows.Media.Ocr"
    }
}

/// 把脚本编码为 PowerShell `-EncodedCommand` 实参（base64 of UTF-16LE）。
#[cfg(windows)]
fn encode_powershell_command(script: &str) -> String {
    let utf16le: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64_encode(&utf16le)
}

/// 标准 base64 编码（无外部依赖）。
#[cfg(windows)]
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

// ===== Tesseract 兜底（跨平台 shell-out）=====

/// Tesseract OCR 引擎（shell-out `tesseract`，需用户装 + chi_sim/eng 语言数据）。
#[derive(Debug)]
pub struct TesseractOcrEngine {
    /// 识别语言（`tesseract -l` 参数），默认 `chi_sim+eng`。
    langs: String,
}

impl TesseractOcrEngine {
    /// 探测：PATH 上是否有可执行的 `tesseract`。
    #[must_use]
    pub fn detect() -> bool {
        let mut cmd = Command::new("tesseract");
        cmd.arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        no_window(&mut cmd);
        matches!(cmd.status(), Ok(s) if s.success())
    }

    /// 构造（默认 `chi_sim+eng`）。
    #[must_use]
    pub fn new() -> Self {
        Self {
            langs: "chi_sim+eng".to_string(),
        }
    }
}

impl Default for TesseractOcrEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OcrEngine for TesseractOcrEngine {
    fn recognize(&self, image: &Path) -> Result<String, IndexError> {
        let mut cmd = Command::new("tesseract");
        cmd.arg(image).arg("stdout").arg("-l").arg(&self.langs);
        let raw = spawn_capture_stdout(cmd, image)?;
        Ok(normalize_ocr_text(&raw))
    }

    fn name(&self) -> &'static str {
        "Tesseract"
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn normalize_collapses_cjk_spaces() {
        assert_eq!(normalize_ocr_text("会 议 纪 要"), "会议纪要");
    }

    #[test]
    fn normalize_keeps_latin_word_spaces() {
        assert_eq!(normalize_ocr_text("Hello World"), "Hello World");
    }

    #[test]
    fn normalize_mixed_cjk_and_latin() {
        // CJK 间折叠、拉丁词间保留、CJK 与拉丁交界保留单空格。
        assert_eq!(normalize_ocr_text("图 片 abc 文 字"), "图片 abc 文字");
    }

    #[test]
    fn normalize_collapses_multiple_spaces_between_cjk() {
        assert_eq!(normalize_ocr_text("季   度   预 算"), "季度预算");
    }

    #[test]
    fn normalize_empty_and_no_space() {
        assert_eq!(normalize_ocr_text(""), "");
        assert_eq!(normalize_ocr_text("会议"), "会议");
    }

    #[test]
    fn normalize_digit_between_cjk_keeps_separation() {
        // 数字非 CJK：`年 2024 月` 两侧空格应保留（不与数字粘连）。
        assert_eq!(normalize_ocr_text("年 2024 月"), "年 2024 月");
    }

    #[test]
    fn pick_engine_priority() {
        assert_eq!(pick_engine(true, true), EnginePick::Windows);
        assert_eq!(pick_engine(true, false), EnginePick::Windows);
        assert_eq!(pick_engine(false, true), EnginePick::Tesseract);
        assert_eq!(pick_engine(false, false), EnginePick::None);
    }

    #[test]
    fn is_cjk_classifies_correctly() {
        assert!(is_cjk('会'));
        assert!(is_cjk('议'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('2'));
        assert!(!is_cjk(' '));
    }

    #[test]
    fn tesseract_name() {
        assert_eq!(TesseractOcrEngine::new().name(), "Tesseract");
    }

    #[cfg(windows)]
    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b""), "");
    }

    #[cfg(windows)]
    #[test]
    fn encode_powershell_command_round_trips_via_utf16le_base64() {
        // "AB" -> UTF-16LE 字节 [0x41,0x00,0x42,0x00] -> base64。
        assert_eq!(
            encode_powershell_command("AB"),
            base64_encode(&[0x41, 0, 0x42, 0])
        );
    }
}
