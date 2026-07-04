//! BETA-31 / BETA-33 cycle 3 v4：GUI 一键下载模型（HF 公开免登录 + reqwest stream + 进度 event）。
//!
//! 支持两个模型：
//! - `Embedding` = 语义召回向量模型（`DEFAULT_EMBED_MODEL_FILE`，v0.9.0 起默认 embeddinggemma-300m-q8_0）
//! - `Generation` = 生成模型 fallback（`DEFAULT_MODEL_FILE` = qwen3-0.6b-q4_k_m.gguf，v0.9.4 起加）
//!
//! 下载完成后写入 `<locifind_data_dir>/models/<filename>`、与 `EmbeddingModelHandle` /
//! `ModelFallbackHandle` 期望的查找路径完全一致（同走 [`crate::locifind_data_dir`]）。
//!
//! **路径单一信源**（BETA-31-v3 cycle 1，2026-06-30）：本模块的下载位置 **必须** 经
//! [`crate::locifind_data_dir`] 派生，**不要** 改用 `app.path().app_data_dir()`——Tauri 路径
//! 基于 bundle id `ai.locifind.desktop`、与历史代码（index.db / audit.jsonl / `EmbeddingModelHandle`）
//! 用的 `dirs::data_dir().join("LociFind")` 不一致。BETA-31 cycle 0 初版踩过此坑、导致下载
//! 文件 `EmbeddingModelHandle` 永远找不到、设置页保持 NotFound 引导用户重复下载死循环。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::search::embedding_model::DEFAULT_EMBED_MODEL_FILE;

/// 生成模型 fallback 的默认文件名（与 `search::model_fallback::DEFAULT_MODEL_FILE` 保持一致）。
/// 本地这里再声明一份、避免跨 mod 依赖循环；如 model_fallback 侧改名、需同步。
const DEFAULT_GENERATION_MODEL_FILE: &str = "qwen3-0.6b-q4_k_m.gguf";

/// HF ggml-org 公开转仓 URL（embeddinggemma-300M-qat-Q8_0.gguf、实际文件名混合大小写）。
const EMBEDDING_HF_URL: &str =
    "https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf/resolve/main/embeddinggemma-300m-qat-Q8_0.gguf?download=true";

/// HF unsloth Qwen3-0.6B-GGUF 公开仓（Q4_K_M 变体、~400 MB）。
/// 下载后重命名保存为 `qwen3-0.6b-q4_k_m.gguf`（`DEFAULT_GENERATION_MODEL_FILE`），
/// 与 `ModelFallbackHandle` 期望的查找文件名一致。
const GENERATION_HF_URL: &str =
    "https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q4_K_M.gguf?download=true";

const PROGRESS_EMIT_BYTES: u64 = 64 * 1024; // 每 64 KB emit 一次

/// 模型种类（路由 URL / 文件名 / atomic guard / event namespace）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Embedding,
    Generation,
}

impl ModelKind {
    fn url(self) -> &'static str {
        match self {
            Self::Embedding => EMBEDDING_HF_URL,
            Self::Generation => GENERATION_HF_URL,
        }
    }
    fn filename(self) -> &'static str {
        match self {
            Self::Embedding => DEFAULT_EMBED_MODEL_FILE,
            Self::Generation => DEFAULT_GENERATION_MODEL_FILE,
        }
    }
    /// Tauri event 命名空间：`model-download://<ns>/progress` 等。
    /// 兼容 v0.9.3 前的老前端：Embedding 同时 emit 无 ns 的老名字。
    fn event_ns(self) -> &'static str {
        match self {
            Self::Embedding => "embedding",
            Self::Generation => "generation",
        }
    }
}

/// 每种模型独立的取消 flag（互不干扰）。
static EMBEDDING_CANCEL: AtomicBool = AtomicBool::new(false);
static GENERATION_CANCEL: AtomicBool = AtomicBool::new(false);
/// 每种模型独立的 in-flight 守卫（允许并发下载不同种类模型；同种双击兜底）。
static EMBEDDING_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static GENERATION_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn cancel_flag(kind: ModelKind) -> &'static AtomicBool {
    match kind {
        ModelKind::Embedding => &EMBEDDING_CANCEL,
        ModelKind::Generation => &GENERATION_CANCEL,
    }
}

fn in_flight(kind: ModelKind) -> &'static AtomicBool {
    match kind {
        ModelKind::Embedding => &EMBEDDING_IN_FLIGHT,
        ModelKind::Generation => &GENERATION_IN_FLIGHT,
    }
}

/// BETA-12：任一模型下载 in-flight？卸载清理的前置守卫——删 `models/` 会与下载
/// 写入（`.partial` → rename）竞争，进行中一律拒绝清理。
pub(crate) fn any_download_in_flight() -> bool {
    EMBEDDING_IN_FLIGHT.load(Ordering::SeqCst) || GENERATION_IN_FLIGHT.load(Ordering::SeqCst)
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    downloaded: u64,
    total: Option<u64>,
}

#[derive(Clone, Serialize)]
struct DonePayload {
    path: String,
}

#[derive(Clone, Serialize)]
struct ErrorPayload {
    reason: String,
}

/// RAII guard：drop 时清 in_flight 位、保证所有路径（成功 / Err / panic）都释放。
struct InFlightGuard(ModelKind);
impl Drop for InFlightGuard {
    fn drop(&mut self) {
        in_flight(self.0).store(false, Ordering::SeqCst);
    }
}

/// 删 partial 文件（防 disk leak、Err 路径调用）。Windows 下 file handle 必须先 drop。
async fn cleanup_partial(partial: &Path) {
    let _ = fs::remove_file(partial).await;
}

/// 解析 `<locifind_data_dir>/models/`、目标文件、`.partial` 兄弟路径（**必须** 与
/// `EmbeddingModelHandle::default_model_path` / `ModelFallbackHandle::default_model_path`
/// 同走 [`crate::locifind_data_dir`]、详 mod doc）。
fn resolve_target_paths(kind: ModelKind) -> (PathBuf, PathBuf, PathBuf) {
    let models_dir = crate::locifind_data_dir().join("models");
    let filename = kind.filename();
    let target = models_dir.join(filename);
    let partial = models_dir.join(format!("{filename}.partial"));
    (models_dir, target, partial)
}

/// 发一次 progress event（新命名空间 + Embedding 兼容老名字）。
fn emit_progress(app: &AppHandle, kind: ModelKind, downloaded: u64, total: Option<u64>) {
    let ns = kind.event_ns();
    let payload = ProgressPayload { downloaded, total };
    let _ = app.emit(&format!("model-download://{ns}/progress"), payload.clone());
    if kind == ModelKind::Embedding {
        // 兼容 <= v0.9.3 前端（listener 挂在无 ns 的老名字）。可在下版本删。
        let _ = app.emit("model-download://progress", payload);
    }
}

fn emit_done(app: &AppHandle, kind: ModelKind, target: &Path) {
    let ns = kind.event_ns();
    let payload = DonePayload {
        path: target.display().to_string(),
    };
    let _ = app.emit(&format!("model-download://{ns}/done"), payload.clone());
    if kind == ModelKind::Embedding {
        let _ = app.emit("model-download://done", payload);
    }
}

fn emit_error(app: &AppHandle, kind: ModelKind, reason: &str) {
    let ns = kind.event_ns();
    let payload = ErrorPayload {
        reason: reason.to_owned(),
    };
    let _ = app.emit(&format!("model-download://{ns}/error"), payload.clone());
    if kind == ModelKind::Embedding {
        let _ = app.emit("model-download://error", payload);
    }
}

/// 内部流式下载实现（与 tauri command 解耦、便于单测）。
///
/// **错误契约**：所有 Err 路径（reqwest 失败 / 写文件失败 / 用户取消）都已
/// 删除 partial 文件、调用方无需重复 cleanup。成功后 partial 已 rename 到
/// target、target 必存在。
///
/// **进度契约**：首次 chunk 必 emit + 之后每跨 `PROGRESS_EMIT_BYTES` (64 KB) emit。
async fn download_stream(
    url: &str,
    target: &Path,
    partial: &Path,
    cancel: &AtomicBool,
    mut emit_progress: impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("reqwest client build 失败: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("HF 下载请求失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HF 下载 HTTP {}", resp.status()));
    }

    let total = resp.content_length();
    let mut file = fs::File::create(partial)
        .await
        .map_err(|e| format!("创建 partial 文件失败: {e}"))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut next_emit: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::SeqCst) {
            drop(file); // Windows: 关闭 handle 后才能 remove_file
            cleanup_partial(partial).await;
            return Err("用户取消下载".to_string());
        }

        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                cleanup_partial(partial).await;
                return Err(format!("chunk 读取失败: {e}"));
            }
        };
        if let Err(e) = file.write_all(&chunk).await {
            drop(file);
            cleanup_partial(partial).await;
            return Err(format!("chunk 写入失败: {e}"));
        }
        downloaded += chunk.len() as u64;

        if downloaded >= next_emit {
            emit_progress(downloaded, total);
            next_emit = downloaded + PROGRESS_EMIT_BYTES;
        }
    }

    if let Err(e) = file.flush().await {
        drop(file);
        cleanup_partial(partial).await;
        return Err(format!("flush 失败: {e}"));
    }
    drop(file);

    if let Err(e) = fs::rename(partial, target).await {
        cleanup_partial(partial).await;
        return Err(format!("rename partial → target 失败: {e}"));
    }

    Ok(())
}

/// 通用下载编排：路径解析、幂等检查、in-flight guard、cancel 重置、事件 emit。
async fn download_model_impl(app: AppHandle, kind: ModelKind) -> Result<(), String> {
    // 重入守卫：同一模型只允许单 in-flight。
    if in_flight(kind)
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("下载已在进行中、请等待当前下载完成或取消".to_string());
    }
    let _guard = InFlightGuard(kind);

    cancel_flag(kind).store(false, Ordering::SeqCst);

    let (models_dir, target, partial) = resolve_target_paths(kind);
    fs::create_dir_all(&models_dir)
        .await
        .map_err(|e| format!("创建 models 目录失败: {e}"))?;

    // 幂等：已存在完整文件、直接 done
    if fs::metadata(&target).await.is_ok() {
        emit_done(&app, kind, &target);
        return Ok(());
    }

    let app_for_progress = app.clone();
    let cancel = cancel_flag(kind);
    let result = download_stream(
        kind.url(),
        &target,
        &partial,
        cancel,
        move |downloaded, total| {
            emit_progress(&app_for_progress, kind, downloaded, total);
        },
    )
    .await;

    match result {
        Ok(()) => {
            emit_done(&app, kind, &target);
            Ok(())
        }
        Err(reason) => {
            emit_error(&app, kind, &reason);
            Err(reason)
        }
    }
}

// ===================== Tauri commands（每模型一对） =====================

/// 触发 embedding 模型 GUI 下载。写入 `<locifind_data_dir>/models/<DEFAULT_EMBED_MODEL_FILE>`。
///
/// 事件：`model-download://embedding/{progress,done,error}` + 兼容老 `model-download://{progress,done,error}`。
#[tauri::command]
pub async fn download_embedding_model(app: AppHandle) -> Result<(), String> {
    download_model_impl(app, ModelKind::Embedding).await
}

/// 取消进行中的 embedding 模型下载（仅设 flag、下次 chunk loop 自动退出）。
#[tauri::command]
pub fn cancel_embedding_download() -> Result<(), String> {
    EMBEDDING_CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

/// BETA-33 cycle 3 v4：触发 generation 模型（Qwen3-0.6B）GUI 下载。
/// 写入 `<locifind_data_dir>/models/qwen3-0.6b-q4_k_m.gguf`。
///
/// 事件：`model-download://generation/{progress,done,error}`。
#[tauri::command]
pub async fn download_generation_model(app: AppHandle) -> Result<(), String> {
    download_model_impl(app, ModelKind::Generation).await
}

/// 取消进行中的 generation 模型下载。
#[tauri::command]
pub fn cancel_generation_download() -> Result<(), String> {
    GENERATION_CANCEL.store(true, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // 注：tauri AppHandle 不易在单测中构造，本模块单测仅覆盖纯逻辑路径
    // （cancel flag + download_stream 写入/重命名）。端到端 tauri command 行为由
    // 真机手测覆盖（spec §2.2.1 Mac self-test）。

    #[test]
    fn cancel_flag_can_be_set_and_cleared() {
        EMBEDDING_CANCEL.store(false, Ordering::SeqCst);
        assert!(!EMBEDDING_CANCEL.load(Ordering::SeqCst));
        let _ = cancel_embedding_download();
        assert!(EMBEDDING_CANCEL.load(Ordering::SeqCst));
        EMBEDDING_CANCEL.store(false, Ordering::SeqCst);
    }

    #[test]
    fn generation_cancel_flag_independent_from_embedding() {
        // 两个模型的 cancel flag 应互不影响（并发下载场景兜底）。
        EMBEDDING_CANCEL.store(false, Ordering::SeqCst);
        GENERATION_CANCEL.store(false, Ordering::SeqCst);
        let _ = cancel_generation_download();
        assert!(GENERATION_CANCEL.load(Ordering::SeqCst));
        assert!(!EMBEDDING_CANCEL.load(Ordering::SeqCst));
        GENERATION_CANCEL.store(false, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn download_stream_writes_chunks_to_partial_then_renames() {
        // 用本地临时 HTTP server 模拟 HF（spec 原写 httptest_lite 实际不存在、
        // 真实 crate 名为 httptest v0.16、API 与 spec 镜像一致）。
        use httptest::matchers::request;
        use httptest::responders::status_code;
        use httptest::{Expectation, Server};

        // 取消标志先清零、避免被前一个测试残留污染。
        EMBEDDING_CANCEL.store(false, Ordering::SeqCst);

        let tmpdir = std::env::temp_dir().join(format!("beta-31-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmpdir);
        let target = tmpdir.join("model.gguf");
        let partial = tmpdir.join("model.gguf.partial");
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(&partial);

        let server = Server::run();
        let body: Vec<u8> = b"GGUF-mock-content-1234567890".repeat(1000); // ~28 KB
        let body_clone = body.clone();
        server.expect(
            Expectation::matching(request::method_path("GET", "/model.gguf"))
                .respond_with(status_code(200).body(body_clone)),
        );
        let url = server.url("/model.gguf").to_string();

        let progress_log: Mutex<Vec<(u64, Option<u64>)>> = Mutex::new(Vec::new());
        let result = download_stream(&url, &target, &partial, &EMBEDDING_CANCEL, |d, t| {
            progress_log.lock().unwrap().push((d, t));
        })
        .await;

        assert!(result.is_ok(), "download_stream failed: {result:?}");
        assert!(target.exists(), "target 文件未生成");
        assert!(!partial.exists(), "partial 文件未删除（rename 应原子完成）");

        let written = std::fs::read(&target).expect("读 target 失败");
        assert_eq!(written, body);

        let log = progress_log.lock().unwrap();
        assert!(!log.is_empty(), "应至少 emit 1 次进度");

        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_dir_all(&tmpdir);
    }
}
