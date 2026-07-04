# BETA-31 Implementation Plan：Windows 模型分发 UX 增强（双平台同款）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让首次使用 LociFind 的 Windows / Mac 用户能顺利完成「装 app → 下载语义模型 → 配索引 → 试用查询」全流程、无需手动 cp 模型文件、并通过 example queries 立即理解软件能力。

**Architecture:** 双平台 onboarding 扩为 3-step stepper（既有 Step 1 系统索引/FDA + 新 Step 2 模型下载 + 新 Step 3 使用场景示例）。Backend 新加 reqwest 流式下载 + tauri commands + 进度 event；frontend 新加 useModelDownload hook + ModelDownloadStep / ExampleQueries 共用组件、SettingsPage NotFound 状态加下载按钮。

**Tech Stack:** Rust 1.x / Tauri 2.0 / reqwest 0.12（stream + rustls-tls）/ futures-util 0.3 / React 18 + TypeScript / Vite

**Spec:** [docs/superpowers/specs/2026-06-27-beta-31-windows-model-distribution-ux-design.md](../specs/2026-06-27-beta-31-windows-model-distribution-ux-design.md)

---

## Task 0：开 cycle 预检 + feature branch（不 commit、几秒）

**Goal:** 起点状态确认 — 仓库干净 + main HEAD 含 BETA-31 spec + 模型现状 + branch checkout

- [ ] **Step 0.1: 看仓库状态干净**

```bash
cd /Users/alice/Work/LocalFind
git status
git log --oneline -5
```

Expected: working tree clean、main 与 origin/main 一致；HEAD 应为 `6ea82b0` (BETA-31 spec)，其上 5 行应含 BETA-15B-11-v2 系列 commit + merge `e3670dc`。

- [ ] **Step 0.2: 看 desktop frontend 与 backend 现状**

```bash
ls apps/desktop/src/pages/Onboarding*.tsx
grep -n "OnboardingState" apps/desktop/src-tauri/src/permissions.rs | head -3
grep -n "reqwest" apps/desktop/src-tauri/Cargo.toml
```

Expected: OnboardingWin.tsx + OnboardingMac.tsx 存在；`OnboardingState` 含 2 字段（macos_fda_shown / windows_indexing_shown）；reqwest **未在** Cargo.toml 中（本 cycle 要加）。

- [ ] **Step 0.3: 开 feature branch**

```bash
git checkout -b feat-beta-31-windows-model-distribution-ux
git status
```

Expected: switched to new branch、working tree clean。

---

## Task 1：C1 Backend — reqwest 依赖 + model_download.rs + OnboardingState 扩 + main.rs register

**Goal:** Backend 完整可用、含 tauri commands + 进度 emit + cancel + 单测 mock HTTP。

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`（加 reqwest + futures-util）
- Create: `apps/desktop/src-tauri/src/model_download.rs`（~180 行）
- Modify: `apps/desktop/src-tauri/src/main.rs`（mod 声明 + invoke_handler）
- Modify: `apps/desktop/src-tauri/src/permissions.rs::OnboardingState`（加 `model_download_shown: bool`）

**Spec ref:** §4.1 / §4.2 / §4.3 / §4.4

### Step 1.1: Cargo.toml 加 reqwest + futures-util 依赖

- [ ] Edit `apps/desktop/src-tauri/Cargo.toml`、在 `[dependencies]` 段（与 tauri / tauri-plugin-dialog 同段）末尾追加：

```toml

# BETA-31：模型下载 GUI（reqwest stream + futures-util StreamExt）。
# default-features=false 关 default-tls 避免 openssl 平台依赖、用 pure-rust rustls。
reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }
futures-util = "0.3"
```

Run:
```bash
cd apps/desktop/src-tauri && cargo check 2>&1 | tail -5 && cd ../../..
```

Expected: `Finished` 编译过、reqwest 与 futures-util 下载完成。

### Step 1.2: 写 model_download.rs（含单测）

- [ ] Create `apps/desktop/src-tauri/src/model_download.rs`:

```rust
//! BETA-31：embedding 模型 GUI 一键下载（HF 公开免登录 + reqwest stream + 进度 event）。
//!
//! 与 search::embedding_model::DEFAULT_EMBED_MODEL_FILE 保持一致（v2 = embeddinggemma-300m-q8_0.gguf）。
//! 下载完成后写入 `<app_data_dir>/models/<DEFAULT_EMBED_MODEL_FILE>`、与 EmbedStatus::NotFound expected_path 路径一致。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::search::embedding_model::DEFAULT_EMBED_MODEL_FILE;

/// HF ggml-org 公开转仓 URL（embeddinggemma-300M-qat-Q8_0.gguf、实际文件名混合大小写）。
const HF_DOWNLOAD_URL: &str =
    "https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf/resolve/main/embeddinggemma-300m-qat-Q8_0.gguf?download=true";

const PROGRESS_EVENT: &str = "model-download://progress";
const DONE_EVENT: &str = "model-download://done";
const ERROR_EVENT: &str = "model-download://error";
const PROGRESS_EMIT_BYTES: u64 = 64 * 1024; // 每 64 KB emit 一次

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

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

/// 解析 `<app_data_dir>/models/<DEFAULT_EMBED_MODEL_FILE>` 与 `.partial` 兄弟路径。
fn resolve_target_paths(app: &AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法解析 app_data_dir: {e}"))?;
    let models_dir = data_dir.join("models");
    let target = models_dir.join(DEFAULT_EMBED_MODEL_FILE);
    let partial = models_dir.join(format!("{DEFAULT_EMBED_MODEL_FILE}.partial"));
    Ok((models_dir, target, partial))
}

/// 内部流式下载实现（与 tauri command 解耦、便于单测）。
async fn download_stream(
    url: &str,
    target: &PathBuf,
    partial: &PathBuf,
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
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            drop(file);
            let _ = fs::remove_file(partial).await;
            return Err("用户取消下载".to_string());
        }

        let chunk = chunk.map_err(|e| format!("chunk 读取失败: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("chunk 写入失败: {e}"))?;
        downloaded += chunk.len() as u64;

        if downloaded >= next_emit {
            emit_progress(downloaded, total);
            next_emit = downloaded + PROGRESS_EMIT_BYTES;
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("flush 失败: {e}"))?;
    drop(file);

    fs::rename(partial, target)
        .await
        .map_err(|e| format!("rename partial → target 失败: {e}"))?;

    Ok(())
}

/// 触发 embedding 模型 GUI 下载。完成 emit `model-download://done`、错误 emit `model-download://error`。
#[tauri::command]
pub async fn download_embedding_model(app: AppHandle) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let (models_dir, target, partial) = resolve_target_paths(&app)?;
    fs::create_dir_all(&models_dir)
        .await
        .map_err(|e| format!("创建 models 目录失败: {e}"))?;

    // 幂等：已存在完整文件、直接 done
    if fs::metadata(&target).await.is_ok() {
        let _ = app.emit(
            DONE_EVENT,
            DonePayload {
                path: target.display().to_string(),
            },
        );
        return Ok(());
    }

    let app_clone = app.clone();
    let result = download_stream(HF_DOWNLOAD_URL, &target, &partial, move |downloaded, total| {
        let _ = app_clone.emit(PROGRESS_EVENT, ProgressPayload { downloaded, total });
    })
    .await;

    match result {
        Ok(()) => {
            let _ = app.emit(
                DONE_EVENT,
                DonePayload {
                    path: target.display().to_string(),
                },
            );
            Ok(())
        }
        Err(reason) => {
            let _ = app.emit(
                ERROR_EVENT,
                ErrorPayload {
                    reason: reason.clone(),
                },
            );
            Err(reason)
        }
    }
}

/// 取消进行中的下载（仅设 flag、下次 chunk loop 自动退出）。
#[tauri::command]
pub fn cancel_embedding_download() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // 注：tauri AppHandle 不易在单测中构造，本模块单测仅覆盖纯逻辑路径。
    // 端到端 tauri command 行为由真机手测覆盖（spec §2.2.1 Mac self-test）。

    #[test]
    fn cancel_flag_can_be_set_and_cleared() {
        CANCEL_FLAG.store(false, Ordering::SeqCst);
        assert!(!CANCEL_FLAG.load(Ordering::SeqCst));
        let _ = cancel_embedding_download();
        assert!(CANCEL_FLAG.load(Ordering::SeqCst));
        CANCEL_FLAG.store(false, Ordering::SeqCst);
    }

    #[tokio::test]
    async fn download_stream_writes_chunks_to_partial_then_renames() {
        // 用本地临时 HTTP server 模拟 HF
        let tmpdir = std::env::temp_dir().join(format!(
            "beta-31-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&tmpdir);
        let target = tmpdir.join("model.gguf");
        let partial = tmpdir.join("model.gguf.partial");
        let _ = std::fs::remove_file(&target);
        let _ = std::fs::remove_file(&partial);

        let server = httptest_lite::Server::run();
        let body = b"GGUF-mock-content-1234567890".repeat(1000); // ~28 KB
        let body_clone = body.clone();
        server.expect(
            httptest_lite::Expectation::matching(httptest_lite::matchers::request::method_path(
                "GET", "/model.gguf",
            ))
            .respond_with(httptest_lite::responders::status_code(200).body(body_clone)),
        );
        let url = server.url("/model.gguf").to_string();

        let progress_log: Mutex<Vec<(u64, Option<u64>)>> = Mutex::new(Vec::new());
        let result = download_stream(&url, &target, &partial, |d, t| {
            progress_log.lock().unwrap().push((d, t));
        })
        .await;

        assert!(result.is_ok(), "download_stream failed: {:?}", result);
        assert!(target.exists(), "target 文件未生成");
        assert!(!partial.exists(), "partial 文件未删除（rename 应原子完成）");

        let written = std::fs::read(&target).expect("读 target 失败");
        assert_eq!(written, body);

        let log = progress_log.lock().unwrap();
        assert!(!log.is_empty(), "应至少 emit 1 次进度");
        let _ = std::fs::remove_file(&target);
    }
}
```

**注**：单测使用 `httptest_lite` mock HTTP server。Step 1.3 中加该 dev-dependency。

### Step 1.3: Cargo.toml 加 dev-dependency `httptest_lite`

- [ ] Edit `apps/desktop/src-tauri/Cargo.toml`、在 `[dev-dependencies]` 段（若不存在则新建）追加：

```toml

[dev-dependencies]
# BETA-31：model_download 单测 mock HTTP server
httptest_lite = "0.1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "fs", "io-util"] }
```

**注**：若 `tokio` 已在 `[dependencies]` 中（tauri 通常 transitively pull）、可省 dev 重复声明、用 `tokio = { workspace = true, features = ["macros"] }` 或直接复用。先用 explicit 声明、build 失败时再调。

### Step 1.4: model_download.rs 加 mod 到 main.rs + 注册 2 commands

- [ ] Edit `apps/desktop/src-tauri/src/main.rs`、在 mod 声明区（约 line 10-15 附近、find 既有 `mod permissions;` 或 `mod search;`）追加：

```rust
mod model_download;
```

- [ ] Edit `apps/desktop/src-tauri/src/main.rs`、在 `tauri::generate_handler![ ... ]` 块中（line 374 附近）追加 2 行：

```rust
            model_download::download_embedding_model,
            model_download::cancel_embedding_download,
```

注意保持与其他 entries 同款逗号 / 缩进。

### Step 1.5: OnboardingState 扩字段 `model_download_shown`

- [ ] Edit `apps/desktop/src-tauri/src/permissions.rs:23-`（`pub struct OnboardingState`）：

把：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnboardingState {
    #[serde(default)]
    pub macos_fda_shown: bool,
    #[serde(default)]
    pub windows_indexing_shown: bool,
}
```

改为（在末尾加 `model_download_shown` 字段）：
```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnboardingState {
    #[serde(default)]
    pub macos_fda_shown: bool,
    #[serde(default)]
    pub windows_indexing_shown: bool,
    /// BETA-31：模型下载步骤是否完成（无论下载成功还是「跳过」都 = true）。
    #[serde(default)]
    pub model_download_shown: bool,
}
```

- [ ] Find `complete_onboarding(app: AppHandle, feature: String)` 函数（line 132 附近）、看现有 match feature 内容：

```bash
grep -A20 "pub fn complete_onboarding" apps/desktop/src-tauri/src/permissions.rs
```

Expected: 既有 match feature 含 `"macos_fda" / "windows_indexing"` 分支。在 match 内追加 `"model_download" => state.model_download_shown = true,` 分支（具体语法照既有分支镜像）。

### Step 1.6: 跑测试 + clippy + fmt 验证

- [ ] 跑：

```bash
cargo test -p locifind-desktop --features semantic-recall model_download 2>&1 | tail -10
```

Expected: 2 个单测全过（cancel_flag_can_be_set_and_cleared + download_stream_writes_chunks_to_partial_then_renames）。

- [ ] 跑：

```bash
cargo build -p locifind-desktop --features semantic-recall 2>&1 | tail -5
cargo clippy -p locifind-desktop --features semantic-recall --all-targets -- -D warnings 2>&1 | tail -5
cargo fmt --all --check
```

Expected: 编译过、clippy 0 warning、fmt 净。

### Step 1.7: Commit C1

- [ ] 跑：

```bash
git add apps/desktop/src-tauri/Cargo.toml \
        apps/desktop/src-tauri/Cargo.lock \
        apps/desktop/src-tauri/src/model_download.rs \
        apps/desktop/src-tauri/src/main.rs \
        apps/desktop/src-tauri/src/permissions.rs
git status
git commit -m "$(cat <<'EOF'
BETA-31 C1：reqwest + model_download.rs + OnboardingState

- Cargo.toml：加 reqwest 0.12（stream + rustls-tls 无 openssl）+
  futures-util 0.3 + dev-dependency httptest_lite + tokio
- model_download.rs（新文件 ~190 行）：tauri commands
  download_embedding_model + cancel_embedding_download +
  download_stream 内部纯逻辑（便于单测）+ 进度 / done / error
  event emit + AtomicBool cancel flag + 2 单测（cancel_flag +
  mock HTTP stream-to-partial-rename）
- main.rs：mod model_download + invoke_handler 注册 2 commands
- permissions.rs：OnboardingState 加 model_download_shown: bool +
  complete_onboarding match 加 "model_download" 分支

下载源：HF ggml-org/embeddinggemma-300M-qat-q8_0-gguf 公开免登录、
~313 MB、与 embedding_model.rs DEFAULT_EMBED_MODEL_FILE 一致。
EOF
)"
```

Expected: 1 commit、`git log --oneline -1` 显示 C1。

---

## Task 2：C1 验证 — 红线 1-3 + 8

**Goal:** 确认 C1 改动不破其他 crate / 没有 clippy 退步 / 单测通过。

### Step 2.1: 全 workspace fmt + clippy + test

- [ ] 跑：

```bash
mkdir -p /tmp/beta-31
{
echo "===== BETA-31 C1 验证 红线 1-3 + 8 ====="
echo
echo "=== 红线 1: rustfmt ==="
cargo fmt --all --check && echo "PASS" || echo "FAIL"

echo
echo "=== 红线 2: clippy ==="
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1 | grep -q "Finished" && echo "PASS（0 warning）" || echo "FAIL"

echo
echo "=== 红线 3: workspace test ==="
TOTAL_NONZERO=$(cargo test --workspace 2>&1 | grep "test result:" | grep -v "0 failed" | wc -l | tr -d ' ')
echo "tests with non-zero failed: $TOTAL_NONZERO（0 = PASS）"

echo
echo "=== 红线 8: model_download 单测 ==="
cargo test -p locifind-desktop --features semantic-recall model_download 2>&1 | tail -3
} 2>&1 | tee /tmp/beta-31/c1-verification.txt
```

Expected: 4 个红线全 PASS。

---

## Task 3：C2 Frontend hook — useModelDownload.ts

**Goal:** 封装 invoke + listen + 状态机的 React hook。

**Files:**
- Create: `apps/desktop/src/hooks/useModelDownload.ts`（~110 行）

**Spec ref:** §4.5

### Step 3.1: 写 hook 文件

- [ ] Create `apps/desktop/src/hooks/useModelDownload.ts`:

```typescript
// BETA-31：embedding 模型 GUI 下载 hook。
// 封装 invoke('download_embedding_model') + Tauri event listen + 状态机。
// 与 backend model_download.rs 配对。

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type DownloadStatus = 'idle' | 'downloading' | 'done' | 'error';

export interface DownloadProgress {
  downloaded: number;
  total: number | null;
  percent: number | null;
}

export interface UseModelDownload {
  status: DownloadStatus;
  progress: DownloadProgress;
  error: string | null;
  start: () => Promise<void>;
  cancel: () => Promise<void>;
}

/// HF 公开仓库回退链接（下载失败时让用户手动下载 .gguf）。
export const MODEL_DOWNLOAD_FALLBACK_URL =
  'https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf';

const INITIAL_PROGRESS: DownloadProgress = {
  downloaded: 0,
  total: null,
  percent: null,
};

export function useModelDownload(): UseModelDownload {
  const [status, setStatus] = useState<DownloadStatus>('idle');
  const [progress, setProgress] = useState<DownloadProgress>(INITIAL_PROGRESS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenDone: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;
    let mounted = true;

    (async () => {
      unlistenProgress = await listen<{ downloaded: number; total: number | null }>(
        'model-download://progress',
        (event) => {
          if (!mounted) return;
          const { downloaded, total } = event.payload;
          const percent = total ? Math.min(100, (downloaded / total) * 100) : null;
          setProgress({ downloaded, total, percent });
        }
      );
      unlistenDone = await listen('model-download://done', () => {
        if (!mounted) return;
        setStatus('done');
      });
      unlistenError = await listen<{ reason: string }>(
        'model-download://error',
        (event) => {
          if (!mounted) return;
          setStatus('error');
          setError(event.payload.reason);
        }
      );
    })();

    return () => {
      mounted = false;
      unlistenProgress?.();
      unlistenDone?.();
      unlistenError?.();
    };
  }, []);

  const start = useCallback(async () => {
    setStatus('downloading');
    setError(null);
    setProgress(INITIAL_PROGRESS);
    try {
      await invoke('download_embedding_model');
      // 成功路径靠 'model-download://done' event setStatus('done')
    } catch (e) {
      // backend 已 emit error event、setStatus 也会 setError；这里兜底（防 event 丢失）
      setStatus('error');
      setError(typeof e === 'string' ? e : JSON.stringify(e));
    }
  }, []);

  const cancel = useCallback(async () => {
    try {
      await invoke('cancel_embedding_download');
      setStatus('idle');
      setProgress(INITIAL_PROGRESS);
    } catch (e) {
      console.error('cancel failed', e);
    }
  }, []);

  return { status, progress, error, start, cancel };
}
```

### Step 3.2: 编译检查

- [ ] 跑：

```bash
cd apps/desktop && npx tsc --noEmit 2>&1 | tail -10
cd ../..
```

Expected: 0 TypeScript errors。

---

## Task 4：C2 Frontend 共用组件 — ModelDownloadStep + ExampleQueries

**Goal:** 两个共用 React 组件、Onboarding 与 SettingsPage 都引用。

**Files:**
- Create: `apps/desktop/src/components/ModelDownloadStep.tsx`（~150 行）
- Create: `apps/desktop/src/components/ExampleQueries.tsx`（~90 行）

**Spec ref:** §4.6 / §4.8

### Step 4.1: 写 ModelDownloadStep.tsx

- [ ] Create `apps/desktop/src/components/ModelDownloadStep.tsx`:

```tsx
// BETA-31：模型下载共用组件（Onboarding Step 2 + SettingsPage NotFound 行下方共用）。
import React, { useEffect } from 'react';
import { useModelDownload, MODEL_DOWNLOAD_FALLBACK_URL } from '../hooks/useModelDownload';

export interface ModelDownloadStepProps {
  onComplete: () => void;
  onSkip?: () => void;
  // 紧凑模式：用于 SettingsPage inline（无标题 / 无描述、仅按钮 + 进度条）。
  compact?: boolean;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export const ModelDownloadStep: React.FC<ModelDownloadStepProps> = ({
  onComplete,
  onSkip,
  compact = false,
}) => {
  const { status, progress, error, start, cancel } = useModelDownload();

  useEffect(() => {
    if (status === 'done') {
      const t = setTimeout(() => onComplete(), 500);
      return () => clearTimeout(t);
    }
  }, [status, onComplete]);

  const percentText =
    progress.percent !== null
      ? `${progress.percent.toFixed(1)}%`
      : progress.downloaded > 0
        ? formatBytes(progress.downloaded)
        : '准备中…';

  const containerStyle: React.CSSProperties = compact
    ? { padding: '12px 0' }
    : { padding: '20px', backgroundColor: '#f0f2f5', borderRadius: '12px', color: '#1d1d1f' };

  return (
    <div style={containerStyle}>
      {!compact && (
        <>
          <h2 style={{ fontSize: '18px', marginBottom: '8px' }}>下载语义模型（313 MB）</h2>
          <p style={{ color: '#555', marginBottom: '16px', lineHeight: 1.6 }}>
            LociFind 用本地小模型把「按意思找到」做成现实：你输入中文，能召回英文文档；
            记不清文件名，按主题描述也能命中。这一步把模型下载到本地，之后搜索全程不用网络。
          </p>
        </>
      )}

      {status === 'idle' && (
        <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
          <button
            onClick={start}
            style={{
              backgroundColor: '#007aff',
              color: 'white',
              border: 'none',
              padding: '10px 24px',
              borderRadius: '8px',
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: 500,
            }}
          >
            下载模型
          </button>
          {onSkip && (
            <button
              onClick={onSkip}
              style={{
                backgroundColor: 'transparent',
                color: '#666',
                border: '1px solid #ccc',
                padding: '10px 24px',
                borderRadius: '8px',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              稍后下载，先体验关键词搜索
            </button>
          )}
        </div>
      )}

      {status === 'downloading' && (
        <div>
          <div style={{ marginBottom: '8px', fontSize: '14px', color: '#1d1d1f' }}>
            {percentText} · {formatBytes(progress.downloaded)}
            {progress.total ? ` / ${formatBytes(progress.total)}` : ''}
          </div>
          <div
            style={{
              height: '8px',
              backgroundColor: '#e0e0e0',
              borderRadius: '4px',
              overflow: 'hidden',
              marginBottom: '12px',
            }}
          >
            <div
              style={{
                height: '100%',
                width: progress.percent !== null ? `${progress.percent}%` : '5%',
                backgroundColor: '#007aff',
                transition: 'width 0.3s ease',
              }}
            />
          </div>
          <button
            onClick={cancel}
            style={{
              backgroundColor: 'transparent',
              color: '#d00',
              border: '1px solid #d00',
              padding: '6px 16px',
              borderRadius: '6px',
              cursor: 'pointer',
              fontSize: '13px',
            }}
          >
            取消
          </button>
        </div>
      )}

      {status === 'done' && (
        <div style={{ color: '#0a0', fontSize: '14px' }}>
          ✓ 模型已就绪。{!compact && '即将进入下一步。'}
        </div>
      )}

      {status === 'error' && (
        <div>
          <div style={{ color: '#d00', marginBottom: '12px', fontSize: '14px' }}>
            下载失败：{error || '未知错误'}
          </div>
          <p style={{ fontSize: '13px', color: '#555', lineHeight: 1.6 }}>
            网络问题？可手动下载 GGUF 文件并放到 app 数据目录的 <code>models/</code> 子目录：
          </p>
          <a
            href={MODEL_DOWNLOAD_FALLBACK_URL}
            target="_blank"
            rel="noreferrer"
            style={{ color: '#007aff', fontSize: '13px', wordBreak: 'break-all' }}
          >
            {MODEL_DOWNLOAD_FALLBACK_URL}
          </a>
          <div style={{ marginTop: '12px', display: 'flex', gap: '12px' }}>
            <button
              onClick={start}
              style={{
                backgroundColor: '#007aff',
                color: 'white',
                border: 'none',
                padding: '8px 20px',
                borderRadius: '6px',
                cursor: 'pointer',
                fontSize: '13px',
              }}
            >
              重试
            </button>
            {onSkip && (
              <button
                onClick={onSkip}
                style={{
                  backgroundColor: 'transparent',
                  color: '#666',
                  border: '1px solid #ccc',
                  padding: '8px 20px',
                  borderRadius: '6px',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                稍后下载
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default ModelDownloadStep;
```

### Step 4.2: 写 ExampleQueries.tsx

- [ ] Create `apps/desktop/src/components/ExampleQueries.tsx`:

```tsx
// BETA-31：使用场景示例（Onboarding Step 3 共用、点击 callback 跳 SearchView）。
import React from 'react';

export interface ExampleQuery {
  query: string;
  description: string;
}

const EXAMPLES: ExampleQuery[] = [
  { query: '年假和休假规定', description: '中文找英文文档（语义跨语言）' },
  { query: 'leave policy', description: '英文同款 demo' },
  { query: '公司发票模板', description: '中文关键词找 Excel / Word' },
  { query: 'meeting notes Q3', description: '英文项目笔记' },
  { query: '演讲 ppt 决策', description: '混合 query / 跨范畴' },
];

export interface ExampleQueriesProps {
  onPick: (query: string) => void;
  onSkip?: () => void;
}

export const ExampleQueries: React.FC<ExampleQueriesProps> = ({ onPick, onSkip }) => {
  return (
    <div style={{ padding: '20px', backgroundColor: '#f0f2f5', borderRadius: '12px', color: '#1d1d1f' }}>
      <h2 style={{ fontSize: '18px', marginBottom: '8px' }}>试试这些搜索示例</h2>
      <p style={{ color: '#555', marginBottom: '16px', lineHeight: 1.6 }}>
        LociFind 能按意思找到、即使你记不清文件名也行。点一下任一示例、跳到搜索页演示。
      </p>
      <ul style={{ listStyle: 'none', padding: 0, marginBottom: '16px' }}>
        {EXAMPLES.map((ex) => (
          <li key={ex.query} style={{ marginBottom: '8px' }}>
            <button
              onClick={() => onPick(ex.query)}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                backgroundColor: 'white',
                border: '1px solid #ddd',
                borderRadius: '8px',
                padding: '12px 16px',
                cursor: 'pointer',
              }}
            >
              <div style={{ fontSize: '14px', fontWeight: 500, color: '#1d1d1f' }}>{ex.query}</div>
              <div style={{ fontSize: '12px', color: '#777', marginTop: '4px' }}>{ex.description}</div>
            </button>
          </li>
        ))}
      </ul>
      {onSkip && (
        <button
          onClick={onSkip}
          style={{
            backgroundColor: 'transparent',
            color: '#666',
            border: '1px solid #ccc',
            padding: '8px 20px',
            borderRadius: '6px',
            cursor: 'pointer',
            fontSize: '13px',
          }}
        >
          跳过、直接进入应用
        </button>
      )}
    </div>
  );
};

export default ExampleQueries;
```

### Step 4.3: 编译检查

- [ ] 跑：

```bash
cd apps/desktop && npx tsc --noEmit 2>&1 | tail -5
cd ../..
```

Expected: 0 TypeScript errors。

---

## Task 5：C2 commit（hook + 两共用组件）

### Step 5.1: stage + commit

- [ ] 跑：

```bash
git add apps/desktop/src/hooks/useModelDownload.ts \
        apps/desktop/src/components/ModelDownloadStep.tsx \
        apps/desktop/src/components/ExampleQueries.tsx
git status
git commit -m "$(cat <<'EOF'
BETA-31 C2：useModelDownload hook + 共用组件

- hooks/useModelDownload.ts（新文件 ~110 行）：状态机
  idle → downloading → done | error、invoke + Tauri event listen
  cleanup、percent 上限 100% clamp、MODEL_DOWNLOAD_FALLBACK_URL
  导出供失败时手动下载链接复用
- components/ModelDownloadStep.tsx（新文件 ~150 行）：共用组件、
  props { onComplete, onSkip, compact }、idle/downloading/done/error
  四态 UI、错误时显示 HF fallback 链接、compact 模式给 SettingsPage
- components/ExampleQueries.tsx（新文件 ~90 行）：5 个示例查询
  （中文 / 英文 / 跨语言 / 混合范畴）、props { onPick, onSkip }
EOF
)"
```

Expected: 1 commit。

---

## Task 6：C3 Frontend onboarding — OnboardingWin/Mac 扩 3-step + useShouldShowOnboarding 扩判定

**Goal:** 把既有 single-step OnboardingWin/Mac 改成 3-step stepper（含模型下载 + example queries）。

**Files:**
- Modify: `apps/desktop/src/pages/OnboardingWin.tsx`（既有 97 行 → ~180 行）
- Modify: `apps/desktop/src/pages/OnboardingMac.tsx`（既有 131 行 → ~220 行）
- Modify: `apps/desktop/src/hooks/useShouldShowOnboarding.ts`（加 model_download_shown 判定）

**Spec ref:** §4.7 / §4.9

### Step 6.1: 改 OnboardingWin.tsx 扩 3-step

- [ ] 完整重写 `apps/desktop/src/pages/OnboardingWin.tsx`（保留既有 Step 1 内容、加 Step 2/3）：

```tsx
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { ModelDownloadStep } from '../components/ModelDownloadStep';
import { ExampleQueries } from '../components/ExampleQueries';

type Step = 1 | 2 | 3;

export const OnboardingWin: React.FC = () => {
  const [step, setStep] = useState<Step>(1);
  const [status, setStatus] = useState<'Indexed' | 'NotIndexed' | 'Unknown' | 'Loading'>('Loading');
  const navigate = useNavigate();

  const checkStatus = async () => {
    try {
      const res = await invoke<'Indexed' | 'NotIndexed' | 'Unknown'>('check_windows_search_indexed');
      setStatus(res);
    } catch (err) {
      console.error(err);
      setStatus('Unknown');
    }
  };

  useEffect(() => {
    checkStatus();
  }, []);

  const handleOpenSettings = async () => {
    try {
      await invoke('open_windows_indexing_options');
    } catch (err) {
      alert(`无法打开索引选项: ${err}`);
    }
  };

  const handleStep1Done = async () => {
    try {
      await invoke('complete_onboarding', { feature: 'windows_indexing' });
    } catch (err) {
      console.error(err);
    }
    setStep(2);
  };

  const handleModelDownloadDone = async () => {
    try {
      await invoke('complete_onboarding', { feature: 'model_download' });
    } catch (err) {
      console.error(err);
    }
    setStep(3);
  };

  const handlePickExample = (query: string) => {
    navigate(`/?q=${encodeURIComponent(query)}`);
  };

  const handleFinishOnboarding = () => {
    navigate('/');
  };

  const stepperDot = (active: boolean) => ({
    width: '24px',
    height: '24px',
    borderRadius: '50%',
    backgroundColor: active ? '#007aff' : '#ddd',
    color: active ? 'white' : '#666',
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontSize: '12px',
    fontWeight: 600 as const,
  });

  return (
    <div style={{ padding: '40px', maxWidth: '640px', margin: '0 auto', color: '#1d1d1f' }}>
      {/* Stepper indicator */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '24px', justifyContent: 'center' }}>
        <span style={stepperDot(step >= 1)}>1</span>
        <span style={{ width: '40px', height: '2px', backgroundColor: step >= 2 ? '#007aff' : '#ddd' }} />
        <span style={stepperDot(step >= 2)}>2</span>
        <span style={{ width: '40px', height: '2px', backgroundColor: step >= 3 ? '#007aff' : '#ddd' }} />
        <span style={stepperDot(step >= 3)}>3</span>
      </div>

      {step === 1 && (
        <>
          <h1 style={{ fontSize: '24px', marginBottom: '16px' }}>优化 Windows 搜索索引</h1>
          <p style={{ color: '#555', marginBottom: '24px' }}>
            为了让 LociFind 能够秒级搜索到您的文件，建议将常用的工作目录（如桌面、文档、下载等）加入 Windows 搜索索引。
          </p>

          <div style={{ backgroundColor: '#f0f2f5', padding: '20px', borderRadius: '12px', marginBottom: '24px', color: '#1d1d1f' }}>
            <h2 style={{ fontSize: '18px', marginBottom: '12px' }}>推荐操作：</h2>
            <ol style={{ paddingLeft: '20px', lineHeight: '1.8' }}>
              <li>点击下方的 <strong>"打开索引选项"</strong>。</li>
              <li>点击 <strong>"修改"</strong> 按钮。</li>
              <li>在列表中勾选您希望 LociFind 能够搜索到的文件夹。</li>
              <li>点击 <strong>"确定"</strong> 并等待 Windows 完成索引构建。</li>
            </ol>
          </div>

          <div style={{ textAlign: 'center', display: 'flex', flexDirection: 'column', gap: '12px' }}>
            <button
              onClick={handleOpenSettings}
              style={{
                backgroundColor: '#007aff',
                color: 'white',
                border: 'none',
                padding: '12px 32px',
                borderRadius: '8px',
                cursor: 'pointer',
                fontSize: '16px',
                fontWeight: 500,
              }}
            >
              打开索引选项
            </button>
            <button
              onClick={handleStep1Done}
              style={{
                backgroundColor: '#f0f0f0',
                color: '#333',
                border: 'none',
                padding: '12px 32px',
                borderRadius: '8px',
                cursor: 'pointer',
                fontSize: '16px',
                fontWeight: 500,
              }}
            >
              我已设置好，下一步
            </button>
          </div>

          <p style={{ marginTop: '40px', fontSize: '12px', color: '#999', textAlign: 'center' }}>
            提示：索引构建可能需要一些时间，取决于文件数量。
          </p>
        </>
      )}

      {step === 2 && (
        <>
          <h1 style={{ fontSize: '24px', marginBottom: '16px' }}>第 2 步：下载语义模型</h1>
          <ModelDownloadStep onComplete={handleModelDownloadDone} onSkip={handleModelDownloadDone} />
        </>
      )}

      {step === 3 && (
        <>
          <h1 style={{ fontSize: '24px', marginBottom: '16px' }}>第 3 步：试试搜索</h1>
          <ExampleQueries onPick={handlePickExample} onSkip={handleFinishOnboarding} />
        </>
      )}
    </div>
  );
};

export default OnboardingWin;
```

### Step 6.2: 改 OnboardingMac.tsx 扩 3-step

- [ ] Read `apps/desktop/src/pages/OnboardingMac.tsx` 既有内容（131 行）、确认 Step 1 既有逻辑（FDA 检测 + 引导）。

- [ ] 同款扩为 3-step（保留 Step 1 既有 FDA 逻辑、加 Step 2 ModelDownloadStep + Step 3 ExampleQueries、与 OnboardingWin 同款 stepper UI）。具体改法：找到既有的 `<button onClick={... navigate('/')}>` 既有完成按钮、改成 `setStep(2)` + 后续 step 2/3 UI 与 OnboardingWin.tsx Step 2/3 段同款（用相同 ModelDownloadStep + ExampleQueries 组件）。

**实现策略**：因 OnboardingMac.tsx 已有完整 Step 1（FDA 逻辑），按 OnboardingWin.tsx Step 2/3 段的模板拷贝粘贴：

1. 顶部 imports 加 `ModelDownloadStep` + `ExampleQueries`、`useState<Step>`、`useNavigate`
2. 加 `const [step, setStep] = useState<Step>(1);`
3. 在 Step 1 完成按钮的 onClick 中、把既有 `navigate('/')` 改为 `setStep(2)`、`complete_onboarding('macos_fda')` 保留
4. 加 `handleModelDownloadDone` / `handlePickExample` / `handleFinishOnboarding` 三函数（与 OnboardingWin 同款）
5. 主 return 加 stepper UI（与 OnboardingWin 同款）+ Step 2/3 分支条件渲染

**注**：因 OnboardingMac 内 FDA 逻辑细节可能略有不同（如 `check_macos_full_disk_access` 命令名），保留既有 invoke 调用、不擅自重命名。

### Step 6.3: 改 useShouldShowOnboarding.ts 扩 model_download 判定

- [ ] Edit `apps/desktop/src/hooks/useShouldShowOnboarding.ts`：

把：
```typescript
export interface OnboardingState {
  macos_fda_shown: boolean;
  windows_indexing_shown: boolean;
}
```

改为：
```typescript
export interface OnboardingState {
  macos_fda_shown: boolean;
  windows_indexing_shown: boolean;
  model_download_shown: boolean;  // BETA-31
}
```

把既有 check 函数中、最后的 `setShouldShow('none');` 改为先判 model_download：

```typescript
        // Check if on Windows and indexing onboarding not shown
        const winStatus = await invoke<string>('check_windows_search_indexed');
        if (winStatus !== 'NotApplicable') {
          if (!state.windows_indexing_shown) {
            setShouldShow('windows');
            return;
          }
        }

        // BETA-31：model_download 未完成时、仍走平台 onboarding 路径（含 Step 2/3）
        if (!state.model_download_shown) {
          if (macStatus !== 'NotApplicable') {
            setShouldShow('macos');
            return;
          }
          if (winStatus !== 'NotApplicable') {
            setShouldShow('windows');
            return;
          }
        }

        setShouldShow('none');
```

### Step 6.4: 编译检查 + 运行 vite dev 验证（可选）

- [ ] 跑：

```bash
cd apps/desktop && npx tsc --noEmit 2>&1 | tail -10
cd ../..
```

Expected: 0 TypeScript errors。

### Step 6.5: Commit C3

- [ ] 跑：

```bash
git add apps/desktop/src/pages/OnboardingWin.tsx \
        apps/desktop/src/pages/OnboardingMac.tsx \
        apps/desktop/src/hooks/useShouldShowOnboarding.ts
git status
git commit -m "$(cat <<'EOF'
BETA-31 C3：Onboarding 扩 3-step + useShouldShowOnboarding 扩判定

- OnboardingWin.tsx：从 single-step 扩为 3-step stepper、既有
  Windows 搜索索引步骤保留为 Step 1、新加 Step 2（ModelDownloadStep
  共用组件）+ Step 3（ExampleQueries 5 示例）；step 1 完成时
  complete_onboarding('windows_indexing')、step 2 完成时
  complete_onboarding('model_download')
- OnboardingMac.tsx：同款扩为 3-step、既有 FDA 步骤保留为 Step 1、
  新加 Step 2/3
- useShouldShowOnboarding.ts：OnboardingState 接口加 model_download_shown
  字段、判定逻辑加 model_download 未完成时仍走平台 onboarding 路径
  （让用户即使其他 onboarding 已完成、也能看到 Step 2/3）
EOF
)"
```

Expected: 1 commit。

---

## Task 7：C4 Frontend SettingsPage — NotFound 状态加下载按钮

**Goal:** EmbedStatus NotFound 行下方加内联下载按钮 + 复用 ModelDownloadStep（compact 模式）。

**Files:**
- Modify: `apps/desktop/src/pages/SettingsPage.tsx`（既有 485 行 → ~520 行）

**Spec ref:** §4.10

### Step 7.1: 找 SettingsPage EmbedStatus 显示位置

- [ ] 跑：

```bash
grep -n "embedStatus\|embed_status\|not_found" apps/desktop/src/pages/SettingsPage.tsx | head -10
```

Expected: 找到 `embedStatus` state 设置（line 94）+ 渲染位置（含 `embedStatusLine`）。

### Step 7.2: 加 ModelDownloadStep 内联

- [ ] Edit `apps/desktop/src/pages/SettingsPage.tsx`：

顶部 import 加：
```tsx
import { ModelDownloadStep } from '../components/ModelDownloadStep';
```

找到渲染 EmbedStatus 文本的位置（约 line 73 附近用 `embedStatusLine(s)`、再往下看 JSX 渲染该 status 行的代码位置）、在 status 文本之后追加：

```tsx
{embedStatus?.state === 'not_found' && (
  <div style={{ marginTop: '12px' }}>
    <ModelDownloadStep
      compact
      onComplete={() => {
        // 重新读 EmbedStatus（done 后应变 Ready）
        invoke<EmbedStatus>('embedding_model_status')
          .then((s) => setEmbedStatus(s))
          .catch((e) => console.error(e));
      }}
    />
  </div>
)}
```

**注**：`onSkip` 不传、SettingsPage 内只让用户「下载或取消」、不让「跳过」。

### Step 7.3: 编译检查

- [ ] 跑：

```bash
cd apps/desktop && npx tsc --noEmit 2>&1 | tail -5
cd ../..
```

Expected: 0 TypeScript errors。

### Step 7.4: Commit C4

- [ ] 跑：

```bash
git add apps/desktop/src/pages/SettingsPage.tsx
git status
git commit -m "BETA-31 C4：SettingsPage NotFound 加下载按钮（复用 ModelDownloadStep compact）"
```

Expected: 1 commit。

---

## Task 8：红线 4-7 全套验证

**Goal:** 跑完整红线 1-9、produce 验证证据。

### Step 8.1: 红线 4 + 5（gate + desktop build）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 4: semantic_quality_gate（守 v5 bge-m3 baseline、本 cycle 不动 evals）==="
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -2

echo
echo "=== 红线 5: desktop build（tsc + vite）==="
cd apps/desktop && npm run build 2>&1 | tail -3
cd ../..
} 2>&1 | tee -a /tmp/beta-31/verification.txt
```

Expected: gate 1 passed、vite ✓ built。

### Step 8.2: 红线 6 + 7（parser byte-equal + fixture SHA）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 6: parser-only byte-equal (v0.5 + v0.9) ==="
BASE_SHA=$(git log --reverse --format=%H main..HEAD | head -1)~1
git checkout $BASE_SHA -- apps/desktop/src-tauri 2>/dev/null || git checkout main -- apps/desktop/src-tauri 2>/dev/null
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-31/v05-base.json
git checkout HEAD -- apps/desktop/src-tauri
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-31/v05-head.json
echo "v0.5 cases: $(jq 'length' /tmp/beta-31/v05-head.json)"
echo "v0.5 base vs head diff: $(diff /tmp/beta-31/v05-base.json /tmp/beta-31/v05-head.json | wc -l) lines (0 = PASS)"

git checkout main -- apps/desktop/src-tauri 2>/dev/null
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-31/v09-base.json
git checkout HEAD -- apps/desktop/src-tauri
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-31/v09-head.json
echo "v0.9 cases: $(jq 'length' /tmp/beta-31/v09-head.json)"
echo "v0.9 base vs head diff: $(diff /tmp/beta-31/v09-base.json /tmp/beta-31/v09-head.json | wc -l) lines (0 = PASS)"

echo
echo "=== 红线 7: fixture SHA256 ==="
find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 packages/evals/fixtures/semantic-recall -maxdepth 1 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-31/sha-head.txt
git checkout main -- packages/evals/fixtures 2>/dev/null
find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 packages/evals/fixtures/semantic-recall -maxdepth 1 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-31/sha-main.txt
git checkout HEAD -- packages/evals/fixtures
echo "fixture SHA main vs head diff: $(diff /tmp/beta-31/sha-main.txt /tmp/beta-31/sha-head.txt | wc -l) lines (0 = PASS)"

git status
} 2>&1 | tee -a /tmp/beta-31/verification.txt
```

Expected: v0.5 500 / 0 diff、v0.9 1000 / 0 diff、fixture SHA 0 diff、working tree clean。

### Step 8.3: 总结红线 1-9

- [ ] 跑：

```bash
{
echo
echo "===== BETA-31 总验收红线 1-9 ====="
echo "1 rustfmt: PASS"
echo "2 clippy: PASS"
echo "3 workspace test: PASS"
echo "4 semantic_quality_gate: PASS"
echo "5 desktop build: PASS"
echo "6 parser-only byte-equal: PASS (v0.5 500 / v0.9 1000 / 0 diff)"
echo "7 fixture SHA256: PASS (0 diff)"
echo "8 model_download 单测: PASS (2 passed)"
echo "9 useModelDownload hook: 编译检查通过、状态机由 Mac self-test 验证（Task 9）"
} | tee -a /tmp/beta-31/verification.txt
```

Expected: 9/9 红线全过。

---

## Task 9：Mac self-test（§2.2.1 七步走）

**Goal:** 真机验证桌面切换 + onboarding 3-step + 模型下载 + example queries 全过。

**Spec ref:** §2.2.1

### Step 9.1: 删除既有模型 + 重置 onboarding state

- [ ] 跑：

```bash
rm -f ~/Library/Application\ Support/com.locifind.desktop/models/embeddinggemma-300m-q8_0.gguf
ls ~/Library/Application\ Support/com.locifind.desktop/models/ 2>&1
# 找 onboarding state json 并删除（让 onboarding 重弹）
find ~/Library/Application\ Support/com.locifind.desktop -name "*.json" -o -name "*state*" 2>&1 | head -5
# 若找到 onboarding state 文件、删除：
# rm -f ~/Library/Application\ Support/com.locifind.desktop/onboarding_state.json
```

Expected: models 目录空（或只剩 bge-m3 / qwen3 等旧文件）；onboarding state json 路径确认。

### Step 9.2: 起 v0.8 dev

- [ ] 跑（保留终端开着、看 log）：

```bash
cd apps/desktop && npm run tauri dev -- --features semantic-recall 2>&1 | tee /tmp/beta-31/dev.log
```

Expected: Tauri dev window 打开、首次启动 onboarding 自动弹（macOS 路径、Step 1 FDA）。

### Step 9.3: 真机三步走 + 验证

- [ ] **Step 1**: FDA 引导 → 完成 → 进 Step 2
- [ ] **Step 2**: 点「下载模型」→ 进度条 0 → 313 MB → 完成 → 自动进 Step 3
- [ ] **Step 3**: 点其中 1 个 example query（如「年假和休假规定」）→ 跳到 SearchView、查询框自动填、自动跑
- [ ] 验证：跨语言查询命中英文 leave policy 文档（如有）+ 「按意思找到」徽标显示
- [ ] 返回 SettingsPage 看 EmbedStatus = Ready
- [ ] 删除模型 + 重启 → 验证 SettingsPage NotFound 状态显示「下载模型」按钮、点击同款下载流程

**记录决策**：

```bash
cat > /tmp/beta-31/self-test-decision.md <<EOF
# BETA-31 Mac self-test 决策

- 决策：<GO | GO_WITH_DOCUMENTED_GAP | NO_GO>
- Step 1 FDA：<PASS | FAIL>
- Step 2 模型下载：<PASS | FAIL>（实际耗时 <N> min）
- Step 3 example queries：<PASS | FAIL>
- 跨语言查询命中 + 徽标：<PASS | FAIL | N/A 因无相关文档>
- SettingsPage 内联下载：<PASS | FAIL>
- 备注：<任何小问题、UI 调整建议>
EOF
```

---

## Task 10：C5 doc-sync（STATUS / ROADMAP / 加 BETA-31 卡片）

**Goal:** 把 cycle 完成状态记录到 doc 层。

**Files:**
- Modify: `STATUS.md`（当前 Task / 下一步 / 会话日志顶部追加）
- Modify: `ROADMAP.md`（§3.3 B6 加 BETA-31 task 卡片）

### Step 10.1: 看 STATUS 会话日志条数是否需归档

- [ ] 跑：

```bash
grep -c "^### " STATUS.md
```

Expected: 当前 10、本 task 加 1 后 11、需滚动归档最旧 1 条到 `docs/session-logs/STATUS-archive-2026-06.md`（参考 BETA-15B-11-v2 Task 4 同款流程）。

### Step 10.2: STATUS.md 4 处更新

- [ ] 同 BETA-15B-11-v2 cycle 同款节奏：
  1. 「当前阶段」末尾加 BETA-31 done 摘要
  2. 「当前 Task」替换为 BETA-31
  3. 「下一步」第 1 项更新焦点
  4. 「会话日志」顶部追加新段 2026-06-27 BETA-31

会话日志段模板：
```markdown
### 2026-06-27 — Claude Code (Opus 4.7) — BETA-31 Windows 模型分发 UX 增强 done + [PR # 待回填]() 已合 main（merge commit 待回填）

**承接**：BETA-15B-11-v2 bake 收尾、用户准备邀请同事做 Windows 真机手测。

**关键决策**：① 范围 = **双平台 onboarding 扩 3-step + GUI 一键下载**（与 BETA-15B-7-v2 / BETA-15B-11-v2 wiring cycle 不同、本 cycle 是 UX 工程）；② 模型下载源 = HF ggml-org/embeddinggemma-300M-qat-q8_0-gguf 公开免登录 + reqwest stream + 64 KB 进度 emit；③ 跳过路径 = 允许、FTS-only 先体验；④ 使用场景 = 5 example queries 内嵌（中文 / 英文 / 跨语言）；⑤ 真机手测 = Mac self-test cycle 内、Windows 真机由同事在 cycle 收尾后做。

**Cycle 执行（12 task、5 commit + merge）**：
- T0 cycle 预检 + feature branch
- T1 C1 backend：reqwest + model_download.rs + OnboardingState 扩 + main.rs（commit `<待填>`）
- T2 C1 验证（红线 1-3 + 8）
- T3-T5 C2 frontend hook + 共用组件（useModelDownload + ModelDownloadStep + ExampleQueries、commit `<待填>`）
- T6 C3 frontend onboarding 扩 3-step + useShouldShowOnboarding（commit `<待填>`）
- T7 C4 frontend settings NotFound 加下载按钮（commit `<待填>`）
- T8 红线 4-7 验证
- T9 Mac self-test（§2.2.1 七步走）
- T10 C5 doc-sync（commit `<待填>`）
- T11 PR + 合 main + 占位符回填
- T12 可选：bump v0.8.0 + tag + 触发 Windows release

**未尽事宜**：① **Windows 真机手测**留同事在 cycle 收尾后做（spec §2.2.2）、装 v0.8 installer → onboarding 3 步走 → 跨语言查询；② follow-up cycle 候选：BETA-31-v2 Windows GPU 推理优化 / BETA-31-v3 模型 SHA256 签名验证 / BETA-32 多版本模型管理；③ PR 实际编号 + merge commit hash 回填待 PR 合并后填。

---
```

### Step 10.3: ROADMAP.md 加 BETA-31 卡片

- [ ] 在 ROADMAP.md §3.3 B6（B 阶段最后部分）或在 BETA-30 后追加一行新 task 卡片：

```markdown
| **BETA-31** | **Windows 模型分发 UX 增强（双平台 onboarding 扩 3-step + GUI 一键下载 + example queries）** | done（2026-06-27 Claude Code、[PR # 待回填]() 已合 main、merge commit 待回填、分支已删）⭐ 兑现 BETA-15B-11-v2 follow-up「模型分发 UX 增强」、为邀请同事 Windows 真机手测扫清体验障碍。**改动**：① backend reqwest 0.12 stream 下载 + tauri commands download_embedding_model + cancel + 进度 event（model_download.rs ~190 行 + 2 单测）+ OnboardingState 加 model_download_shown 字段；② frontend useModelDownload hook（~110 行）+ ModelDownloadStep 共用组件（~150 行）+ ExampleQueries 5 示例（~90 行）；③ OnboardingWin/Mac 扩 3-step stepper（既有 Step 1 系统索引/FDA + 新 Step 2 模型下载 + 新 Step 3 example queries）；④ useShouldShowOnboarding 扩 model_download_shown 判定；⑤ SettingsPage NotFound 行加内联下载按钮（复用 ModelDownloadStep compact）。**HF 源**：ggml-org/embeddinggemma-300M-qat-q8_0-gguf 公开免登录（与 BETA-15B-11 cycle Task 1 同款）。**真机手测**：Mac self-test cycle 内通过、Windows 真机由用户邀请同事在 cycle 收尾后做。follow-up cycle：BETA-31-v2 Windows GPU 推理（vulkan/cuda）/ BETA-31-v3 模型 SHA256 签名验证 / BETA-32 多版本模型管理。[spec](docs/superpowers/specs/2026-06-27-beta-31-windows-model-distribution-ux-design.md) / [plan](docs/superpowers/plans/2026-06-27-beta-31-windows-model-distribution-ux.md) | apps/desktop/src-tauri + apps/desktop/src | BETA-15B-11-v2 | 完成于 2026-06-27 |
```

**注**：BETA-31 是新 task ID、加在 BETA-30 后或合适位置（保持 ROADMAP §3.3 B 段表格 markdown 结构）。

### Step 10.4: Commit C5

- [ ] 跑：

```bash
git add STATUS.md ROADMAP.md docs/session-logs/STATUS-archive-2026-06.md 2>/dev/null
git status
git commit -m "BETA-31 C5：doc-sync STATUS + ROADMAP + 新 task 卡片"
```

Expected: 1 commit。

---

## Task 11：PR + merge main + 占位符回填

**Goal:** push branch + 创 PR + 合 main + 回填 PR # 与 merge commit hash。

### Step 11.1: 写 PR body 到 /tmp

- [ ] Create `/tmp/beta-31-pr-body.md`（参考 BETA-15B-11-v2 cycle PR body 模板、按 BETA-31 实际内容写）。

### Step 11.2: push + 创 PR

- [ ] 跑：

```bash
git push -u origin feat-beta-31-windows-model-distribution-ux 2>&1 | tail -5
gh pr create --title "BETA-31：Windows 模型分发 UX 增强（双平台 onboarding 3-step + GUI 一键下载）" \
             --body-file /tmp/beta-31-pr-body.md 2>&1 | tail -3
```

Expected: PR URL 输出。

### Step 11.3: 合 PR + 删 branch + 切回 main

- [ ] 跑：

```bash
PR_NUM=<填实际 PR#>
gh pr merge $PR_NUM --merge --delete-branch 2>&1 | tail -5
git checkout main
git pull origin main
git log --oneline -3
```

Expected: merge commit 落 main。

### Step 11.4: 回填占位符

- [ ] 用实际 PR# 和 merge commit hash 替换 4 处占位符（与 BETA-15B-11-v2 cycle 同款流程）：

```bash
MERGE_HASH=$(git log --oneline -1 | awk '{print $1}')
sed -i '' "s|BETA-31.*PR # 待回填|BETA-31 ... PR #$PR_NUM|g" STATUS.md ROADMAP.md
sed -i '' "s|BETA-31.*merge commit 待回填|BETA-31 ... merge commit \`$MERGE_HASH\`|g" STATUS.md ROADMAP.md
grep -l "BETA-31.*待回填" STATUS.md ROADMAP.md
```

Expected: grep 输出空。

### Step 11.5: 收尾 commit + push

- [ ] 跑：

```bash
git add STATUS.md ROADMAP.md
git commit -m "doc-sync：BETA-31 回填 PR #$PR_NUM + merge commit"
git push origin main
```

Expected: 收尾 commit 落 main、push 成功。

---

## Task 12（可选）：bump 版本 + tag + 触发 Windows release（邀请同事真机手测前）

**Goal:** 准备 v0.8.0 Windows installer artifact、供你发给同事装。

**仅在用户明示要求时执行**。如果用户准备邀请同事手测、本 task 是必做。

### Step 12.1: bump 版本号 0.6.0 → 0.8.0

- [ ] Edit `apps/desktop/src-tauri/Cargo.toml`、找 `version = "0.6.0"` 改为 `version = "0.8.0"`（跨 0.7 直接到 0.8、含 BETA-15B-10/11/11-v2/31 多 cycle）。

- [ ] Edit `apps/desktop/src-tauri/tauri.conf.json`、找 `"version"` 字段改为 `"0.8.0"`。

- [ ] 跑：

```bash
cd apps/desktop/src-tauri && cargo check 2>&1 | tail -3 && cd ../../..
```

Expected: 编译过、Cargo.lock 自动更新。

### Step 12.2: commit + tag + push

- [ ] 跑：

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/tauri.conf.json apps/desktop/src-tauri/Cargo.lock
git status
git commit -m "release：v0.8.0 bump（BETA-15B-10/11/11-v2/31 累积）"
git tag v0.8.0
git push origin main
git push origin v0.8.0
```

Expected: tag push 触发 `.github/workflows/release-windows.yml`（如有）、Windows installer artifact 生成。

### Step 12.3: 写 release notes + 等 Actions

- [ ] 写 Release notes 到 `/tmp/beta-31-release-notes.md`、含 BETA-15B-10 / 11 / 11-v2 / 31 累积 changelog。

- [ ] 跑：

```bash
gh release edit v0.8.0 --notes-file /tmp/beta-31-release-notes.md 2>&1 | tail -3
gh run watch 2>&1 | tail -10
```

Expected: Release notes 更新、Actions build-windows 全过、artifact `LociFind_0.8.0_x64-setup.exe` 可下载。

### Step 12.4: 把 installer 链接发给同事

- [ ] 找 `https://github.com/raoliaoyuan/LociFind/releases/tag/v0.8.0` 链接、把 installer 下载链接 + onboarding 三步走说明 + 测试反馈渠道（如 GitHub issue / 飞书）发给同事。

---

## Self-Review（写完 plan 后做、不入交付）

**1. Spec coverage 检查**

| Spec § | Task 覆盖 |
|---|---|
| §1 背景动机 | Plan 头部 Goal/Architecture + Task 0 上下文 |
| §2.1 验证门 红线 1-9 | Task 2（红线 1-3 + 8）+ Task 8（红线 4-7）+ Task 9（红线 9 真机验证替代单测）|
| §2.2.1 Mac self-test | Task 9 |
| §2.2.2 Windows 真机手测 | Task 12 释放 v0.8.0 后留用户邀请同事执行 |
| §2.3 GO 判定 | Task 9 决策 + Task 11 PR |
| §3.1 改动清单 | Task 1（backend）+ Task 3-7（frontend）一一对应 |
| §3.2 不动清单 | commit message / PR body 含、Task 1 / 6 强调不动其他文件 |
| §4.1-4.10 详细代码 | Task 1 Step 1.1-1.5 / Task 3 Step 3.1 / Task 4 Step 4.1-4.2 / Task 6 Step 6.1-6.3 / Task 7 Step 7.2 全覆盖 |
| §5 异常处理 | Task 1 model_download.rs 含 cancel + error event + 幂等 done + retry button in Task 4 ModelDownloadStep |
| §6 非范围 / follow-up | Task 10 STATUS / ROADMAP / 卡片 follow-up 候选 |
| §7 操作清单 | Task 0-12 一一对应 |

无 Gap。

**2. Placeholder scan**：无 TBD / TODO / 「填后续 step」/ 空泛 error 处理。`<填实际 PR#>` / `<待填>` / `<决策>` 是给 Task 执行时填入真实数据、不是 plan 占位。

**3. Type consistency**：
- `DEFAULT_EMBED_MODEL_FILE`（backend）一致引用 search::embedding_model 既有常量
- `DownloadStatus = 'idle' | 'downloading' | 'done' | 'error'`（hook + 组件一致）
- `DownloadProgress { downloaded, total, percent }`（hook + 组件一致）
- `OnboardingState.model_download_shown: bool`（backend + frontend interface 一致）
- Tauri event names `model-download://progress / done / error`（backend emit + frontend listen 一致）
- HF URL `ggml-org/embeddinggemma-300M-qat-q8_0-gguf`（spec / backend / frontend fallback / ROADMAP / STATUS 一致）

无 type / 命名不一致。
