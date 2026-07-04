# BETA-31 设计：Windows 模型分发 UX 增强（双平台同款）

> **类型**：Frontend + Backend UX cycle（含 reqwest 依赖新增、tauri commands 新增、3 个新 frontend 文件、2 个 frontend 文件扩、1 个 backend 文件扩）
> **承接**：BETA-15B-11-v2 收尾（[PR #19](https://github.com/raoliaoyuan/LociFind/pull/19) merged commit `e3670dc`）+ 用户准备邀请同事做 Windows 真机手测
> **目标**：让首次使用 LociFind 的 Windows / Mac 用户能顺利完成「装 app → 下载语义模型 → 配索引 → 试用查询」全流程、无需手动 cp 模型文件、并通过 example queries 立即理解软件能力
> **范围**：双平台 onboarding 加 2 个新 step（下载模型 + 使用场景示例）+ GUI 一键下载 + 设置页 NotFound 状态加下载按钮
> **不涉及**：Windows GPU 推理优化（vulkan/cuda、留 follow-up）、模型签名验证、多版本管理、自动升级、断点续传

## §1 背景与动机

### §1.1 BETA-15B-11-v2 收尾后的真机手测准备

BETA-15B-11-v2 把 default embedding model 切到 embeddinggemma-300m-q8_0.gguf（313 MB / 双过 spec 字面 OVERALL 0.874 / crosslang 0.716 vs v5 bge-m3 baseline）。但真机手测时旧用户 / 新用户都需要手动 cp 模型文件到 `<app_data_dir>/models/embeddinggemma-300m-q8_0.gguf`，这对邀请非开发同事做 Windows 真机手测是**阻塞性体验 gap**。

### §1.2 现状 — 已有但不完整的 UX 基础设施

经过 `apps/desktop/src/pages/` 和 `apps/desktop/src-tauri/src/permissions.rs` 探索：

| 已有 | 状态 | 缺什么 |
|---|---|---|
| `OnboardingWin.tsx`（97 行）| 仅引导 Windows 搜索索引、单 step、`navigate('/')` 完结 | 缺模型下载 step + 使用场景介绍 step |
| `OnboardingMac.tsx`（131 行）| FDA + Spotlight 引导 | 同上、缺模型下载 + 使用场景 |
| `useShouldShowOnboarding.ts`（47 行）| 判定 `'macos' / 'windows' / 'none'`、读 `OnboardingState { macos_fda_shown, windows_indexing_shown }` | 缺 `model_download_shown` 字段 |
| `SettingsPage.tsx` `EmbedStatus`（line 56-94）| NotFound 状态显示「放到 ${expected_path} 后将自动启用」文本 | 缺「下载模型」一键按钮 |
| `permissions.rs::OnboardingState`（line 23-）| `macos_fda_shown` + `windows_indexing_shown` 持久化 | 缺 `model_download_shown` |
| Backend 下载基础设施 | 无 reqwest 依赖、无 model_download mod | 全新加 |

### §1.3 用户体验目标

**北极星指标**：邀请非开发同事做 Windows 真机手测时、装完 v0.8 installer → 跟着 onboarding 三步走 → 能跑通跨语言查询「年假规定」命中英文 leave policy 文档 + 「按意思找到」徽标显示。整个过程 < 10 min、零 CLI 命令、零手动文件操作。

## §2 接受标准与红线

### §2.1 验证门

| # | 红线 | 验证命令 | 目标 |
|---|---|---|---|
| 1 | rustfmt | `cargo fmt --all --check` | 净 |
| 2 | clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| 3 | workspace test | `cargo test --workspace` | 全过 |
| 4 | semantic_quality_gate | `cargo test -p locifind-evals --test semantic_quality_gate` | 1 passed（baseline.json 不动、本 cycle 不动 evals 层）|
| 5 | desktop tsc + vite | `npm run -w apps/desktop build` | 净 + vite 成功 |
| 6 | parser-only byte-equal | v0.5 / v0.9 evals binary 与 main byte-equal（jq -S 规范化）| 500 / 1000 cases / 0 diff |
| 7 | fixture SHA256 | parser-rs / v0.5 / v0.9 / semantic-recall 既有 fixture | 与 main byte-equal |
| 8 | model_download 单测 | `cargo test -p locifind-desktop --features semantic-recall model_download` | 含 mock HTTP server 验进度 event + 文件写入 |
| 9 | useModelDownload hook 单测 | `npm run -w apps/desktop test`（如有 vitest 配置）或 desktop build 编译检查 | hook 类型正确 + 状态机 idle→downloading→done/error |

**注**：红线 9 若 frontend 无 vitest 配置、降级为「desktop build 编译检查通过 + 真机手测验状态机正确」。

### §2.2 真机手测（必做、cycle 内）

**§2.2.1 Mac self-test（你自己做、~10 min、cycle 收口前必做）**：

1. 删除 `~/Library/Application Support/com.locifind.desktop/models/embeddinggemma-300m-q8_0.gguf`（模拟新用户）
2. `npm run tauri dev --features semantic-recall` 起 v0.8 dev
3. Onboarding 自动弹（macOS Step 1: FDA → Step 2: 下载模型 → Step 3: 使用场景示例）
4. Step 2 点「下载模型」→ 进度条显示 0 → 313 MB → 完成
5. Step 3 5 个 example queries 显示、点其中 1 个跳到 SearchView 自动填查询框
6. SearchView 跑跨语言查询「年假和休假规定」→ 命中英文 leave policy 文档 + 「按意思找到」徽标
7. 同 onboarding 在 Step 2 走「跳过」路径、验证 SettingsPage NotFound 状态显示「下载模型」按钮、点击同款下载流程

**§2.2.2 Windows 真机手测（邀请同事做、cycle 收尾后）**：

- 装 v0.8 installer（NSIS、CPU-only embedding）→ 首启 → Windows 路径 onboarding 3 步走 → 跨语言查询命中 + 关键词搜索 + 模型下载完成

### §2.3 GO 判定

| Branch | 条件 | 行动 |
|---|---|---|
| **GO**（默认）| 红线 1-9 + §2.2.1 Mac self-test 全过 | 落库 / doc-sync / PR / 合 main / 你邀请同事做 §2.2.2 Windows 真机手测 |
| **GO with documented gap** | 红线 1-9 全过 + §2.2.1 Mac self-test 某项小问题（如 example queries 文案微调）但不阻塞 | 落库 / PR 标 `[手测 follow-up]` / 合 main / Mac self-test 修复留 follow-up commit |
| **NO GO** | 红线 1-9 任一不过 + Mac 下载失败 + 进度条 stuck 等 | 不发布、回滚 / 调研 / cycle 标 done-with-rollback |

## §3 改动清单（YAGNI）

### §3.1 做什么

| # | 文件 | 改动 | 体量估算 |
|---|---|---|---|
| 1 | `apps/desktop/src-tauri/Cargo.toml` | 加 `reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }` + 可能 `futures = "0.3"` 用 stream | +2 行 |
| 2 | `apps/desktop/src-tauri/src/model_download.rs` | 新文件、~150 行：tauri commands `download_embedding_model` / `cancel_embedding_download` + reqwest stream + progress emit + cancel flag + 单测 mock HTTP | 新 150 行 |
| 3 | `apps/desktop/src-tauri/src/main.rs` | `mod model_download;` + invoke_handler 注册 2 新 commands | +3 行 |
| 4 | `apps/desktop/src-tauri/src/permissions.rs` | `OnboardingState` 加字段 `model_download_shown: bool`（默认 false）+ 现有 `complete_onboarding` 自动支持新 feature str | +1 行 + serde derive 自动 |
| 5 | `apps/desktop/src/hooks/useModelDownload.ts` | 新文件、~80 行：状态机 + invoke + listen + cleanup | 新 80 行 |
| 6 | `apps/desktop/src/components/ModelDownloadStep.tsx` | 新文件、~120 行：共用组件、props { onComplete, onSkip }、进度条 / 跳过 / 错误时显示 HF 链接 fallback | 新 120 行 |
| 7 | `apps/desktop/src/pages/OnboardingWin.tsx` | 扩为 3-step stepper（既有 Step 1 系统索引 + 新 Step 2 模型下载 + 新 Step 3 使用场景示例）+ step 状态机 | +130 行 / -10 行 |
| 8 | `apps/desktop/src/pages/OnboardingMac.tsx` | 同款扩 3-step（既有 Step 1 FDA + 新 Step 2 模型下载 + 新 Step 3 使用场景示例）| +130 行 / -10 行 |
| 9 | `apps/desktop/src/hooks/useShouldShowOnboarding.ts` | 加 `model_download_shown` 判定：即使 macos_fda 或 windows_indexing 完成、若 model_download_shown=false 也弹（show 'macos' or 'windows' 路径含 Step 2/3）| +5 行 |
| 10 | `apps/desktop/src/pages/SettingsPage.tsx` | NotFound 状态行下方加「下载模型」按钮 + 复用 useModelDownload + 进度条 inline 显示 | +30 行 |
| 11 | `apps/desktop/src/components/ExampleQueries.tsx` | 新文件 ~60 行：5 个 example queries（中文 / 英文 / 跨语言）、点击 callback `onPick(query: string)` | 新 60 行 |

**总体量**：backend ~155 行 + frontend ~430 行新增 + ~270 行扩、约 850 行净增。

### §3.2 不做什么

- 不动 [`packages/result-normalizer/`](../../packages/result-normalizer/)（cosine_threshold / similarity_floor / semantic_weight）
- 不动 [`packages/evals/**`](../../packages/evals/)（baseline.json / gate.rs / vectors / cases / corpus）
- 不动 [`packages/model-runtime/**`](../../packages/model-runtime/)（pooling / llama.rs / llama-cpp-4 版本）
- 不动 [`packages/indexer/**`](../../packages/indexer/) / [`packages/spike-retrieval/**`](../../packages/spike-retrieval/)
- 不动 [`apps/desktop/src-tauri/src/search/embedding_model.rs`](../../apps/desktop/src-tauri/src/search/embedding_model.rs)（v2 已切 embeddinggemma-300m、本 cycle 不改）
- 不动 `apps/desktop/src/SearchView.tsx`（example queries 通过 query params 或 store 传递、不动主 SearchView 逻辑、仅可能加 `useSearchParams` 读初始 query）
- 不引 Tauri 2 `tauri-plugin-http` frontend plugin（用 backend reqwest 更可控）
- 不加 SHA256 校验（reqwest stream 时 chunked SHA256 + 完成后比对、留 BETA-31-v3）
- 不加断点续传 / 多线程下载（aria2c -x 8 类）— 313 MB 单线程 reqwest 可接受、留 follow-up
- 不加 OnboardingMac 既有 FDA 步骤改动（只增量加 Step 2/3、Step 1 既有逻辑保持）

## §4 数据与代码改动详细

### §4.1 `Cargo.toml` 加 reqwest

```toml
# apps/desktop/src-tauri/Cargo.toml [dependencies] 段
reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }
futures-util = "0.3"  # for stream try_next
```

**注**：`default-features = false` 关闭 `default-tls`（避免 openssl 平台依赖、用 pure-rust rustls）；`stream` feature 支持 chunked body 读取；`futures-util` 提供 `StreamExt::next` for reqwest body stream。

### §4.2 `model_download.rs` 新文件

```rust
//! BETA-31：embedding 模型 GUI 一键下载（HF 公开免登录 + reqwest stream + 进度 event）。
//!
//! 与 search::embedding_model::DEFAULT_EMBED_MODEL_FILE 保持一致（v2 = embeddinggemma-300m-q8_0.gguf）。
//! 下载完成后写入 `<app_data_dir>/models/<DEFAULT_EMBED_MODEL_FILE>`、与 EmbedStatus::NotFound expected_path 路径一致。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
const PROGRESS_EMIT_BYTES: u64 = 64 * 1024;  // 每 64 KB emit 一次（~5000 次 over 313 MB）

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

/// 触发 embedding 模型 GUI 下载。流式写入 `<app_data_dir>/models/<DEFAULT_EMBED_MODEL_FILE>`。
/// 进度通过 `model-download://progress` event emit。完成 emit `model-download://done`、错误 emit `model-download://error`。
#[tauri::command]
pub async fn download_embedding_model(app: AppHandle) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法解析 app_data_dir: {e}"))?;
    let models_dir = data_dir.join("models");
    fs::create_dir_all(&models_dir)
        .await
        .map_err(|e| format!("创建 models 目录失败: {e}"))?;

    let target_path = models_dir.join(DEFAULT_EMBED_MODEL_FILE);
    let partial_path = models_dir.join(format!("{DEFAULT_EMBED_MODEL_FILE}.partial"));

    // 若已存在完整文件、直接 done（幂等）
    if fs::metadata(&target_path).await.is_ok() {
        let _ = app.emit(
            "model-download://done",
            DonePayload {
                path: target_path.display().to_string(),
            },
        );
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))  // 5 min per chunk timeout
        .build()
        .map_err(|e| format!("reqwest client build 失败: {e}"))?;

    let resp = client
        .get(HF_DOWNLOAD_URL)
        .send()
        .await
        .map_err(|e| format!("HF 下载请求失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("HF 下载 HTTP {}", resp.status()));
    }

    let total = resp.content_length();
    let mut file = fs::File::create(&partial_path)
        .await
        .map_err(|e| format!("创建 partial 文件失败: {e}"))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut next_emit: u64 = 0;

    while let Some(chunk) = stream.next().await {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            drop(file);
            let _ = fs::remove_file(&partial_path).await;
            return Err("用户取消下载".to_string());
        }

        let chunk = chunk.map_err(|e| format!("chunk 读取失败: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("chunk 写入失败: {e}"))?;
        downloaded += chunk.len() as u64;

        if downloaded >= next_emit {
            let _ = app.emit(
                PROGRESS_EVENT,
                ProgressPayload { downloaded, total },
            );
            next_emit = downloaded + PROGRESS_EMIT_BYTES;
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("flush 失败: {e}"))?;
    drop(file);

    fs::rename(&partial_path, &target_path)
        .await
        .map_err(|e| format!("rename partial → target 失败: {e}"))?;

    let _ = app.emit(
        "model-download://done",
        DonePayload {
            path: target_path.display().to_string(),
        },
    );

    Ok(())
}

/// 取消进行中的下载（仅设 flag、下次 chunk loop 自动退出）。
#[tauri::command]
pub fn cancel_embedding_download() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    Ok(())
}
```

**注释**：`AtomicBool` cancel flag 用全局 static 简化（单文件下载、不并发）；若未来支持多并发下载、改 per-download state。

### §4.3 `main.rs` register

```rust
mod model_download;  // 加到 mod 声明区

// invoke_handler 中加：
model_download::download_embedding_model,
model_download::cancel_embedding_download,
```

### §4.4 `permissions.rs::OnboardingState` 扩字段

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OnboardingState {
    #[serde(default)]
    pub macos_fda_shown: bool,
    #[serde(default)]
    pub windows_indexing_shown: bool,
    #[serde(default)]
    pub model_download_shown: bool,  // BETA-31 新加
}
```

`complete_onboarding(feature: String)` 已有 match `"macos_fda" / "windows_indexing"`、加 `"model_download" => state.model_download_shown = true`。

### §4.5 `useModelDownload.ts` 新文件

```typescript
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

const HF_FALLBACK_URL = 'https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf';

export function useModelDownload(): UseModelDownload {
  const [status, setStatus] = useState<DownloadStatus>('idle');
  const [progress, setProgress] = useState<DownloadProgress>({ downloaded: 0, total: null, percent: null });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenDone: UnlistenFn | null = null;
    let unlistenError: UnlistenFn | null = null;

    (async () => {
      unlistenProgress = await listen<{ downloaded: number; total: number | null }>(
        'model-download://progress',
        (event) => {
          const { downloaded, total } = event.payload;
          const percent = total ? (downloaded / total) * 100 : null;
          setProgress({ downloaded, total, percent });
        }
      );
      unlistenDone = await listen('model-download://done', () => {
        setStatus('done');
      });
      unlistenError = await listen<{ reason: string }>(
        'model-download://error',
        (event) => {
          setStatus('error');
          setError(event.payload.reason);
        }
      );
    })();

    return () => {
      unlistenProgress?.();
      unlistenDone?.();
      unlistenError?.();
    };
  }, []);

  const start = useCallback(async () => {
    setStatus('downloading');
    setError(null);
    try {
      await invoke('download_embedding_model');
    } catch (e) {
      setStatus('error');
      setError(typeof e === 'string' ? e : JSON.stringify(e));
    }
  }, []);

  const cancel = useCallback(async () => {
    try {
      await invoke('cancel_embedding_download');
      setStatus('idle');
    } catch (e) {
      console.error('cancel failed', e);
    }
  }, []);

  return { status, progress, error, start, cancel };
}

export const MODEL_DOWNLOAD_FALLBACK_URL = HF_FALLBACK_URL;
```

### §4.6 `ModelDownloadStep.tsx` 新组件

Props: `{ onComplete: () => void; onSkip: () => void }`。含：
- 标题「下载语义模型（313 MB）」
- 描述：解释什么是语义搜索、为什么需要、下载到本地后续不再用网络
- 「下载」按钮（status='idle'）→ `start()` → 显示进度条
- 进度条 + 百分比 + 字节数（downloading）
- 「跳过先体验关键词搜索」按钮（idle / error 时显示）→ `onSkip()`
- 错误时显示 `error` + 「手动下载」链接到 `MODEL_DOWNLOAD_FALLBACK_URL` + 路径提示
- 完成（status='done'）自动 `onComplete()`（useEffect 监听）

### §4.7 `OnboardingWin.tsx` / `OnboardingMac.tsx` 扩为 3-step stepper

```tsx
const [step, setStep] = useState<1 | 2 | 3>(1);

// Step 1: 既有内容（Windows 搜索索引 / Mac FDA）
//   完成 callback → setStep(2)
// Step 2: <ModelDownloadStep onComplete={() => setStep(3)} onSkip={async () => {
//   await invoke('complete_onboarding', { feature: 'model_download' });
//   setStep(3);
// }} />
// Step 3: <ExampleQueries onPick={(q) => {
//   navigate(`/?q=${encodeURIComponent(q)}`);  // 或通过 zustand store 传递初始 query
// }} />
//   完成 callback → navigate('/') + complete_onboarding('windows_indexing' / 'macos_fda') 已在 Step 1 调过

// 顶部 stepper UI：1 ─ 2 ─ 3 圈状指示器
```

### §4.8 `ExampleQueries.tsx` 新组件

```tsx
const EXAMPLES = [
  { label: '年假和休假规定', desc: '中文 query 找英文文档（语义跨语言）' },
  { label: 'leave policy', desc: '英文 query 同款 demo' },
  { label: '公司发票模板', desc: '中文关键词找 Excel / Word' },
  { label: 'meeting notes Q3', desc: '英文项目笔记' },
  { label: '演讲 ppt 决策', desc: '混合 query / 跨范畴' },
];

export function ExampleQueries({ onPick }: { onPick: (q: string) => void }) {
  return (
    <div>
      <h2>试试这些搜索示例</h2>
      <p>LociFind 能按意思找到、即使你不记得文件名也行。</p>
      <ul>
        {EXAMPLES.map((ex) => (
          <li key={ex.label}>
            <button onClick={() => onPick(ex.label)}>{ex.label}</button>
            <span>{ex.desc}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

### §4.9 `useShouldShowOnboarding.ts` 扩判定

```typescript
// 加 model_download 判定：
// 若 model_download_shown=false、即使其他 onboarding 已 shown、也返回 platform 路径让其走 Step 2/3
// 若 model_download_shown=true 且 platform onboarding 已 shown、返回 'none'

if (!state.model_download_shown) {
  // 走 macos 或 windows 路径（既有 platform onboarding 含 Step 2/3）
  // 优先用当前 platform、若都不适用 fallback 用通用路径（暂时不做、留 follow-up）
  if (macStatus !== 'NotApplicable') {
    setShouldShow('macos');  // OnboardingMac 含 Step 2/3
    return;
  }
  if (winStatus !== 'NotApplicable') {
    setShouldShow('windows');
    return;
  }
}
```

### §4.10 `SettingsPage.tsx` NotFound 状态加按钮

EmbedStatus NotFound 行下方加（about line 73 后）：

```tsx
{embedStatus?.state === 'not_found' && (
  <ModelDownloadInline />  // 复用 useModelDownload + 简化版进度条
)}
```

新增 `ModelDownloadInline` 组件（或直接 inline）：含「下载模型」按钮 + 进度条、状态 done 后 setTimeout 重新读 EmbedStatus（或 listen done event 后触发 reload）。

## §5 异常处理

| 场景 | 触发 | 处理 |
|---|---|---|
| HF CDN 503 / 网络断 | `resp.status().is_success() == false` 或 stream error | emit `model-download://error` + frontend 显示「下载失败、可重试」+ 显示 HF 链接 fallback |
| 磁盘空间不足 | `file.write_all` IO error | 同上、提示磁盘空间 |
| 用户中途取消 | `cancel_embedding_download()` set flag、下次 chunk loop 检测 | 删 partial 文件 + emit error reason='用户取消下载' |
| 已存在完整文件 | `fs::metadata(&target_path).await.is_ok()` 早返 | 立即 emit done、不重下（幂等） |
| App restart 时有 partial 文件 | partial 文件无 cleanup、占磁盘 | 启动时 `cleanup_partial_files()`（如有 model_download.rs init 调用）— follow-up 可选 |
| 下载完成后 EmbedStatus 仍 NotFound | possibly file write 未刷盘 | done event listener 触发 `invoke('embedding_model_status')` 重新读、或加 5s 后 retry |
| 用户跳过下载、之后想下 | SettingsPage NotFound 行下方「下载模型」按钮 | 与 onboarding 同款 useModelDownload 流程 |

## §6 范围（YAGNI）— 不做的事

明确不在本 cycle 范围、留独立 follow-up：

1. **Windows GPU 推理优化**（vulkan / cuda）— BETA-31-v2、~1w
2. **模型 SHA256 签名验证**（reqwest stream chunked SHA256 + 完成后比对 `6fa0c02a...`）— BETA-31-v3、~0.5d
3. **多版本模型管理**（用户能装 bge-m3 + embeddinggemma 两个 + 切换默认）— BETA-32、~1w
4. **自动 app 升级**（用户手动装新 installer）— BETA-33
5. **下载断点续传 / 多线程**（aria2c -x 8 / Range header）— 313 MB 单线程可接受
6. **Onboarding `none` 平台路径**（非 Mac 非 Win、如 Linux）— 不在 LociFind 范围
7. **example queries 配置化**（用户能编辑示例列表）— 写死 5 个即可、~1d follow-up
8. **下载完成后自动 reindex 触发**（用户需要重启 app 或手动 reindex）— BETA-15B-2 spawn_semantic_index 已支持自动暖机、本 cycle 验证下载完成后 EmbedStatus → Ready 后自动 reindex 走通即可

## §7 操作清单（执行顺序、与 plan task 编号对齐）

| Task | 描述 | 估时 | 输出 |
|---|---|---|---|
| T0 | cycle 预检 + feature branch（feat-beta-31-windows-model-distribution-ux）| ~5 min | branch checkout |
| T1 | C1 backend：Cargo.toml + model_download.rs + permissions.rs OnboardingState 扩字段 + main.rs register | ~3h | C1 commit |
| T2 | T1 验证：workspace test + clippy + fmt + 单测 mock HTTP | ~30 min | 红线 1-3 + 8 |
| T3 | C2 frontend hook：useModelDownload.ts | ~1h | C2 commit 前置 |
| T4 | C2 frontend 共用组件：ModelDownloadStep.tsx + ExampleQueries.tsx | ~2h | C2 commit 前置 |
| T5 | C2 commit（hook + 两共用组件）| ~10 min | C2 commit |
| T6 | C3 frontend onboarding：OnboardingWin / OnboardingMac 扩 3-step + useShouldShowOnboarding 加 model_download 判定 | ~2h | C3 commit |
| T7 | C4 frontend settings：SettingsPage NotFound 加下载按钮 | ~30 min | C4 commit |
| T8 | 红线 4-7 全套验证 | ~30 min | 验证证据落 /tmp |
| T9 | Mac self-test（§2.2.1 7 step）| ~10 min | self-test 决策 |
| T10 | C5 doc-sync：STATUS / ROADMAP 加 BETA-31 卡片 + 可选 bump 版本 v0.8.0 | ~30 min | C5 commit |
| T11 | PR + 合 main + 占位符回填 + push origin/main | ~15 min | merge commit + 占位符回填 |
| T12 | 可选：bump v0.8.0 + tag + push 触发 Windows release workflow（如 user 准备让同事跑 §2.2.2 真机手测）| ~15 min | tag v0.8.0 + Windows installer artifact |

**总估时**：~9-11h（GO 路径）+ ~10 min Mac self-test。

## §8 链接

- [BETA-15B-11-v2 spec](./2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md)（前置 cycle）
- [BETA-15B-11-v2 PR #19](https://github.com/raoliaoyuan/LociFind/pull/19)（merge commit `e3670dc`）
- [embedding_model.rs](../../apps/desktop/src-tauri/src/search/embedding_model.rs)（DEFAULT_EMBED_MODEL_FILE / EMBED_MODEL_ID）
- [permissions.rs](../../apps/desktop/src-tauri/src/permissions.rs)（OnboardingState）
- [OnboardingWin.tsx](../../apps/desktop/src/pages/OnboardingWin.tsx) / [OnboardingMac.tsx](../../apps/desktop/src/pages/OnboardingMac.tsx)
- [useShouldShowOnboarding.ts](../../apps/desktop/src/hooks/useShouldShowOnboarding.ts)
- [SettingsPage.tsx](../../apps/desktop/src/pages/SettingsPage.tsx)
- HF 模型仓库：https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf
- 实际 HF GGUF URL：https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf/resolve/main/embeddinggemma-300m-qat-Q8_0.gguf?download=true
- [docs/manual-test-scenarios.md](../../docs/manual-test-scenarios.md)（cycle 收尾后可加 BETA-31 章节、留 follow-up doc cycle）
