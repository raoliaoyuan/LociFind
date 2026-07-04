# BETA-15B-8 model-runtime pooling type detection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `packages/model-runtime/src/llama.rs:357` 硬编码 `LlamaPoolingType::Last` 改为按 GGUF metadata `<arch>.pooling_type` 动态选择 + architecture-based heuristic fallback，解 BETA-15B-7 v4 cycle 暴露的 cross-arch embedding 评测被错配 pooling 阻断的 infra 缺陷。修复后端到端自验 = qwen3-0.6b vectors.json byte-equal（零回归）+ bge-m3 真水位 sweep（infra 修复有数据指证）。

**Architecture:** 新增 `packages/model-runtime/src/pooling.rs` 纯逻辑模块（不依赖 `LlamaModel`，可直接单测）+ `llama.rs` 内 thin adapter `detect_model_pooling(&LlamaModel)` 拿 metadata 调纯函数 + `worker_main` 函数内 model 加载后 detect 一次存为函数局部变量 + `run_embed` 签名扩 `pooling: LlamaPoolingType` 参数。

**Tech Stack:** Rust 1.x / `llama-cpp-4 = 0.3.0`（底层 `llama-cpp-2 0.1.146`）/ TDD（5-8 个纯逻辑单测）/ superpowers subagent-driven workflow。

**Spec:** [docs/superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md](../specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md)

**Cycle 范围**（spec §3）：纯 model-runtime infra 修复 + bge-m3 端到端自验。不动 `DEFAULT_EMBEDDING_MODEL_PATH` / `baseline.json` / `gate.rs` / `llama-cpp-4` version。

---

## Task 1: 新建 pooling.rs 模块 + 9 个 TDD 单测

**Files:**
- Create: `packages/model-runtime/src/pooling.rs`
- Modify: `packages/model-runtime/src/lib.rs`（加 `mod pooling;` 声明）

**说明**：纯逻辑模块、不依赖 `LlamaModel`、可单测全覆盖。先写所有 9 个单测 + 函数桩（`unimplemented!()`）→ 看测试 fail → 实现三函数 → 看测试 pass → fmt + clippy + commit。

### Step 1.1: 写 `packages/model-runtime/src/pooling.rs` —— 模块骨架 + 9 个单测 + 函数桩（红灯）

- [ ] 创建文件 `packages/model-runtime/src/pooling.rs`，内容如下：

```rust
//! GGUF metadata → LlamaPoolingType 检测（纯逻辑、可单测）。
//!
//! 与 llama.cpp upstream `LLAMA_POOLING_TYPE_*` 枚举对齐：
//! 0=None, 1=Mean, 2=Cls, 3=Last（注：upstream 还有 4=Rank、留 reranker 用、
//! llama-cpp-4 0.3.0 binding 未暴露、本 cycle 不映射、未来接 reranker 升 binding 后扩）。
//! GGUF 标准 metadata key 形式为 `<arch>.pooling_type`（i32/u32），arch 取自 `general.architecture`。
//!
//! BETA-15B-8：替换 `llama.rs` 中硬编码 `LlamaPoolingType::Last`，
//! 解 BETA-15B-7 v4 cycle 暴露的 bge-m3（bert arch 声明 CLS）被错配为 Last 的 infra 缺陷。

#![cfg(feature = "llama-cpp")]

use crate::ModelError;
use llama_cpp_4::context::params::LlamaPoolingType;

pub(crate) fn map_gguf_pooling_value(_v: i64) -> Result<LlamaPoolingType, ModelError> {
    unimplemented!("Task 1 step 1.3")
}

pub(crate) fn default_pooling_for_arch(_arch: &str) -> Result<LlamaPoolingType, ModelError> {
    unimplemented!("Task 1 step 1.4")
}

pub(crate) fn detect_pooling_type(
    _arch: &str,
    _pooling_meta: Option<i64>,
) -> Result<LlamaPoolingType, ModelError> {
    unimplemented!("Task 1 step 1.5")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_gguf_pooling_value_covers_all_known() {
        assert_eq!(map_gguf_pooling_value(0).unwrap(), LlamaPoolingType::None);
        assert_eq!(map_gguf_pooling_value(1).unwrap(), LlamaPoolingType::Mean);
        assert_eq!(map_gguf_pooling_value(2).unwrap(), LlamaPoolingType::Cls);
        assert_eq!(map_gguf_pooling_value(3).unwrap(), LlamaPoolingType::Last);
        // 注：llama-cpp-4 0.3.0 binding 未暴露 `LlamaPoolingType::Rank`（reranker 用）；
        // 本 cycle 不映射 `4`、保持 Err 行为；未来接 reranker 升 binding 后扩。
    }

    #[test]
    fn map_gguf_pooling_value_rejects_out_of_range() {
        assert!(map_gguf_pooling_value(-1).is_err());
        assert!(map_gguf_pooling_value(4).is_err()); // Rank 未暴露
        assert!(map_gguf_pooling_value(5).is_err());
        assert!(map_gguf_pooling_value(99).is_err());
    }

    #[test]
    fn default_pooling_for_arch_bert_family_is_cls() {
        assert_eq!(
            default_pooling_for_arch("bert").unwrap(),
            LlamaPoolingType::Cls
        );
        assert_eq!(
            default_pooling_for_arch("nomic-bert").unwrap(),
            LlamaPoolingType::Cls
        );
        assert_eq!(
            default_pooling_for_arch("jina-bert-v2").unwrap(),
            LlamaPoolingType::Cls
        );
        assert_eq!(
            default_pooling_for_arch("roberta").unwrap(),
            LlamaPoolingType::Cls
        );
    }

    #[test]
    fn default_pooling_for_arch_decoder_family_is_last() {
        assert_eq!(
            default_pooling_for_arch("llama").unwrap(),
            LlamaPoolingType::Last
        );
        assert_eq!(
            default_pooling_for_arch("qwen2").unwrap(),
            LlamaPoolingType::Last
        );
        assert_eq!(
            default_pooling_for_arch("qwen3").unwrap(),
            LlamaPoolingType::Last
        );
        assert_eq!(
            default_pooling_for_arch("mistral").unwrap(),
            LlamaPoolingType::Last
        );
    }

    #[test]
    fn default_pooling_for_arch_t5_is_mean() {
        assert_eq!(
            default_pooling_for_arch("t5").unwrap(),
            LlamaPoolingType::Mean
        );
    }

    #[test]
    fn default_pooling_for_arch_unknown_fails() {
        assert!(default_pooling_for_arch("frobnicator").is_err());
        assert!(default_pooling_for_arch("").is_err());
    }

    #[test]
    fn detect_pooling_type_metadata_overrides_heuristic() {
        // qwen3 + meta=3 → Last（现行 qwen3-embedding-0.6b/8b 行为锚、零回归保护）
        assert_eq!(
            detect_pooling_type("qwen3", Some(3)).unwrap(),
            LlamaPoolingType::Last
        );
        // bert + meta=2 → Cls（修正 v4 cycle bge-m3 错配的回归保护单测）
        assert_eq!(
            detect_pooling_type("bert", Some(2)).unwrap(),
            LlamaPoolingType::Cls
        );
    }

    #[test]
    fn detect_pooling_type_missing_metadata_uses_heuristic() {
        assert_eq!(
            detect_pooling_type("bert", None).unwrap(),
            LlamaPoolingType::Cls
        );
        assert_eq!(
            detect_pooling_type("qwen3", None).unwrap(),
            LlamaPoolingType::Last
        );
        assert!(detect_pooling_type("frobnicator", None).is_err());
    }

    #[test]
    fn detect_pooling_type_invalid_metadata_fails_even_with_known_arch() {
        // arch 已知但 metadata 越界 = 仍 fail（不静默 fallback 到 heuristic、避免藏 bug）
        assert!(detect_pooling_type("qwen3", Some(99)).is_err());
    }
}
```

### Step 1.2: 在 `packages/model-runtime/src/lib.rs` 加模块声明

- [ ] 在 `packages/model-runtime/src/lib.rs` 顶部（在 `use serde::{Deserialize, Serialize};` 等 import 之前的位置）追加：

```rust
#[cfg(feature = "llama-cpp")]
mod pooling;
```

如已有现成的 `mod` 声明区域（看具体文件结构）就追加在该区域；否则插入到第 1 行 import 之后、`use` 块之前的合适位置。`pub(crate)` 函数无需对外暴露、`mod pooling;` 即可。

### Step 1.3: 跑单测确认全部 fail（验证 TDD 闭环建立）

- [ ] 运行：

```bash
cargo test -p locifind-model-runtime --features llama-cpp pooling::
```

**Expected**：9 个单测全部 fail（编译能过）。失败原因都是 `unimplemented!()` panic（"Task 1 step 1.x"）。若编译失败检查 `mod pooling;` 是否加对、`use crate::ModelError;` 是否正确解析。

### Step 1.4: 实现 `map_gguf_pooling_value`（绿灯第 1 步）

- [ ] 编辑 `packages/model-runtime/src/pooling.rs`、替换 `map_gguf_pooling_value` 函数体：

```rust
pub(crate) fn map_gguf_pooling_value(v: i64) -> Result<LlamaPoolingType, ModelError> {
    match v {
        0 => Ok(LlamaPoolingType::None),
        1 => Ok(LlamaPoolingType::Mean),
        2 => Ok(LlamaPoolingType::Cls),
        3 => Ok(LlamaPoolingType::Last),
        // 注：llama-cpp-4 0.3.0 binding 未暴露 `LlamaPoolingType::Rank=4`、reranker 用；
        // 本 cycle 不映射、`4` 走下面 Err 分支；未来接 reranker 升 binding 后扩。
        _ => Err(ModelError::LoadError(format!(
            "invalid GGUF pooling_type value: {v} (expected 0..=3; \
             Rank=4 reserved for rerankers, not exposed by llama-cpp-4 0.3.0 bindings)"
        ))),
    }
}
```

### Step 1.5: 实现 `default_pooling_for_arch`（绿灯第 2 步）

- [ ] 替换 `default_pooling_for_arch` 函数体：

```rust
pub(crate) fn default_pooling_for_arch(arch: &str) -> Result<LlamaPoolingType, ModelError> {
    match arch {
        "bert" | "nomic-bert" | "jina-bert-v2" | "roberta" => Ok(LlamaPoolingType::Cls),
        "t5" => Ok(LlamaPoolingType::Mean),
        "llama" | "qwen2" | "qwen3" | "mistral" => Ok(LlamaPoolingType::Last),
        _ => Err(ModelError::LoadError(format!(
            "unknown architecture '{arch}' and GGUF did not declare <arch>.pooling_type; \
             declare pooling_type in GGUF metadata or extend default_pooling_for_arch heuristic table"
        ))),
    }
}
```

### Step 1.6: 实现 `detect_pooling_type`（绿灯第 3 步）

- [ ] 替换 `detect_pooling_type` 函数体：

```rust
pub(crate) fn detect_pooling_type(
    arch: &str,
    pooling_meta: Option<i64>,
) -> Result<LlamaPoolingType, ModelError> {
    match pooling_meta {
        Some(v) => map_gguf_pooling_value(v),
        None => default_pooling_for_arch(arch),
    }
}
```

### Step 1.7: 跑单测确认全部 pass

- [ ] 运行：

```bash
cargo test -p locifind-model-runtime --features llama-cpp pooling::
```

**Expected**：9 passed, 0 failed（实际 9 个 `#[test]` 函数；spec / plan 起草时本来想覆盖 `Rank=4` 的第 10 个 case，因 llama-cpp-4 0.3.0 binding 未暴露 `Rank`、改为「`4` 走 Err 分支」走 `map_gguf_pooling_value_rejects_out_of_range` 同款单测、不单列）。

### Step 1.8: 跑 workspace 测试 + fmt + clippy 验全工程不破

- [ ] 运行（依次执行）：

```bash
cargo fmt --all --check
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo test --workspace
```

**Expected**：fmt 净 / clippy 0 warning / workspace test 不破现状。若 fmt fail 跑 `cargo fmt --all` 修正。

### Step 1.9: Commit Task 1

- [ ] 运行：

```bash
git add packages/model-runtime/src/pooling.rs packages/model-runtime/src/lib.rs
git commit -m "$(cat <<'EOF'
BETA-15B-8 task 1：新建 pooling.rs 纯逻辑模块 + 9 单测

抽 (arch, Option<i64>) → LlamaPoolingType 纯函数：metadata-driven、缺失走 arch heuristic、未知 arch fail-fast。三函数 map_gguf_pooling_value / default_pooling_for_arch / detect_pooling_type 全单测覆盖（9 个 `#[test]` 函数全 pass）；与 llama.cpp upstream LLAMA_POOLING_TYPE_* 枚举对齐（0=None 1=Mean 2=Cls 3=Last 4=Rank）；heuristic 表当下含 bert/nomic-bert/jina-bert-v2/roberta→Cls + t5→Mean + llama/qwen2/qwen3/mistral→Last；未来 cross-arch 模型未知 arch 给明示错误信息（含「扩展 heuristic 表」建议）；本 task 仅新增模块 + lib.rs 加 mod 声明、未触 llama.rs run_embed 硬编码 Last（留 task 2）。验证：cargo fmt 净、clippy 0、workspace test 不破。
EOF
)"
```

---

## Task 2: llama.rs 接入（adapter + worker_main + run_embed）+ qwen3-0.6b byte-equal 红线手验

**Files:**
- Modify: `packages/model-runtime/src/llama.rs`

**说明**：把 Task 1 的纯函数接到生产路径。三个改动：① 新增 `detect_model_pooling(&LlamaModel)` thin adapter（在 llama.rs 内、不抽到 pooling.rs，避免 pooling.rs 触 LlamaModel 类型）；② `worker_main` 函数内 model 加载后 detect 一次、失败早退、成功存为函数局部变量 `pooling`；③ `run_embed` 签名扩 `pooling: LlamaPoolingType` 参数、删硬编码 `LlamaPoolingType::Last`。完成后跑 qwen3-0.6b 重 embed 验 `vectors.json` byte-equal（spec §2.2 验收 (6) 红线、本 task 端到端硬证据）。

### Step 2.1: 加 `detect_model_pooling` adapter

- [ ] 在 `packages/model-runtime/src/llama.rs` 中、`run_embed` 函数定义**之前**（约第 343 行附近、`/// BETA-26：embedding 模式` 注释之上）插入新函数：

```rust
/// BETA-15B-8：从 GGUF metadata 读 `general.architecture` + `<arch>.pooling_type`，
/// 调 `pooling::detect_pooling_type` 拿正确的 `LlamaPoolingType`。
///
/// metadata 缺失时走 `pooling::default_pooling_for_arch` 启发式；未知 arch fail-fast。
/// 由 `worker_main` 在 model 加载后调一次、结果存为函数局部变量传给 `run_embed`。
///
/// `meta_val_str` 需要 `buf_size` —— `general.architecture` 值是短字符串（如 `qwen3`/`bert`/
/// `jina-bert-v2`）、`pooling_type` 是 `"0"`..=`"3"`，256 字节足够（与 llama-cpp-4 `metadata()`
/// 默认 key buf 同款）。
///
/// 注：起草时引用了 `llama-cpp-2 0.1.146` 的旧签名 `meta_val_str(key)` 1 参；实测
/// `llama-cpp-4 0.3.0` binding 实际签名 = `meta_val_str(key, buf_size)` 2 参、本 cycle 用
/// `META_BUF=256`。
#[cfg(feature = "llama-cpp")]
fn detect_model_pooling(model: &LlamaModel) -> Result<LlamaPoolingType, ModelError> {
    const META_BUF: usize = 256;
    let arch = model
        .meta_val_str("general.architecture", META_BUF)
        .map_err(|e| {
            ModelError::LoadError(format!(
                "missing GGUF metadata `general.architecture`: {e}"
            ))
        })?;
    let key = format!("{arch}.pooling_type");
    let pooling_meta = match model.meta_val_str(&key, META_BUF) {
        Ok(s) => Some(s.parse::<i64>().map_err(|e| {
            ModelError::LoadError(format!("invalid GGUF metadata `{key}` = `{s}`: {e}"))
        })?),
        Err(_) => None,
    };
    crate::pooling::detect_pooling_type(&arch, pooling_meta)
}
```

### Step 2.2: 改 `run_embed` 签名 + 删硬编码 Last

- [ ] 编辑 `packages/model-runtime/src/llama.rs:344-402`（`run_embed` 函数）：
  - 把函数注释开头的 `Qwen3-Embedding 用 last-token 池化，故显式设 LlamaPoolingType::Last` 替换为「按 GGUF metadata 选 `pooling`（BETA-15B-8，detect 在 `worker_main` 一次性完成）」
  - 函数签名加 `pooling: LlamaPoolingType` 参数（放在 `context_size: u32` 之后、`text: &str` 之前）
  - 函数体内 `.with_pooling_type(LlamaPoolingType::Last)` 改为 `.with_pooling_type(pooling)`

完整修改后函数（替换 line 344-402 的整段）：

```rust
/// BETA-26：embedding 模式。新建一个启用 embeddings 的专用 context（与生成路径并行，互不
/// 干扰），prefill 整段文本后取池化后的句向量。`pooling` 由 `worker_main` 在 model 加载后
/// 通过 `detect_model_pooling` 一次性确定（BETA-15B-8，替代之前硬编码 `LlamaPoolingType::Last`
/// 错配 BERT 系 arch 的 bug）。llama.cpp 在 decode 后把池化结果写入 seq 0 的 embedding 槽，
/// 经 `embeddings_seq_ith(0)` 读取。最后做 L2 归一化，方便上层直接用点积当 cosine。
#[cfg(feature = "llama-cpp")]
fn run_embed(
    backend: &LlamaBackend,
    model: &LlamaModel,
    context_size: u32,
    pooling: LlamaPoolingType,
    text: &str,
) -> Result<Vec<f32>, ModelError> {
    let ctx_params = make_ctx_params(context_size)
        .with_embeddings(true)
        .with_pooling_type(pooling);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| ModelError::InferenceError(format!("Failed to create embed context: {e}")))?;

    let tokens = model
        .str_to_token(text, AddBos::Always)
        .map_err(|e| ModelError::InferenceError(format!("Failed to tokenize: {e}")))?;
    if tokens.is_empty() {
        return Err(ModelError::InferenceError(
            "empty token sequence for embedding".to_owned(),
        ));
    }

    // 池化模式下需要每个 token 都标记 output（logits=true），llama.cpp 才会对整段做池化。
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    for (i, &token) in tokens.iter().enumerate() {
        // i 不会接近 i32 上限；超长输入会在 decode 处被 n_ctx 拒绝。
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let pos = i as i32;
        batch
            .add(token, pos, &[0], true)
            .map_err(|e| ModelError::InferenceError(format!("Failed to add token: {e}")))?;
    }
    ctx.decode(&mut batch)
        .map_err(|e| ModelError::InferenceError(format!("Failed to decode: {e}")))?;

    let emb = ctx
        .embeddings_seq_ith(0)
        .map_err(|e| ModelError::InferenceError(format!("Failed to read embeddings: {e}")))?;
    let mut v = emb.to_vec();
    if v.is_empty() {
        return Err(ModelError::InferenceError(
            "empty embedding vector".to_owned(),
        ));
    }

    // L2 归一化。
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    Ok(v)
}
```

### Step 2.3: 改 `worker_main` —— detect 一次 + 早退 + 透传 pooling 给 run_embed

- [ ] 编辑 `packages/model-runtime/src/llama.rs:211-274`（`worker_main` 函数）。修改两处：

**(a)** 在 line 234 `if ready_tx.send(Ok(())).is_err() { ... }` 之**前**（即 model load 成功后、ready 信号发送之前）插入 detect pooling 块：

```rust
    let pooling = match detect_model_pooling(&model) {
        Ok(p) => p,
        Err(e) => {
            let _ = ready_tx.send(Err(e));
            return;
        }
    };
```

**(b)** 在 line 268-270 `Request::Embed { text, reply } => { let res = run_embed(backend, &model, context_size, &text); ... }` 中、把 `run_embed` 调用加 `pooling` 参数：

```rust
            Request::Embed { text, reply } => {
                let res = run_embed(backend, &model, context_size, pooling, &text);
                let _ = reply.send(res);
            }
```

完整修改后 `worker_main`（替换 line 211-274 的整段）：

```rust
/// worker 线程主体：加载模型，然后循环处理请求。`session` 保存常驻 context 与已 prefill
/// 的固定前缀，跨 `GenerateCached` 调用复用其 KV。`model` 先于 `session` 声明，故 `session`
/// 内借用 `model` 的 context 合法（非自引用结构体，只是后声明的局部借用先声明的局部）。
///
/// BETA-15B-8：model 加载成功后立刻 `detect_model_pooling`、`pooling` 存为函数局部变量、
/// 失败时 `ready_tx.send(Err)` 早退（与 `load_from_file` 失败同款路径）。
#[cfg(feature = "llama-cpp")]
fn worker_main(
    backend: &LlamaBackend,
    path: &Path,
    gpu_layers: u32,
    context_size: u32,
    ready_tx: &Sender<Result<(), ModelError>>,
    req_rx: &Receiver<Request>,
) {
    let mut model_params = LlamaModelParams::default();
    if gpu_layers > 0 {
        model_params = model_params.with_n_gpu_layers(gpu_layers);
    }

    let model = match LlamaModel::load_from_file(backend, path, &model_params) {
        Ok(m) => m,
        Err(e) => {
            let _ = ready_tx.send(Err(ModelError::LoadError(format!(
                "Failed to load model: {e}"
            ))));
            return;
        }
    };

    let pooling = match detect_model_pooling(&model) {
        Ok(p) => p,
        Err(e) => {
            let _ = ready_tx.send(Err(e));
            return;
        }
    };

    if ready_tx.send(Ok(())).is_err() {
        return; // 调用方已放弃
    }

    // 常驻前缀会话（懒初始化）。
    let mut session: Option<PrefixSession> = None;

    while let Ok(req) = req_rx.recv() {
        match req {
            Request::Generate {
                prompt,
                params,
                reply,
            } => {
                let res = run_plain(backend, &model, context_size, &prompt, &params);
                let _ = reply.send(res);
            }
            Request::GenerateCached {
                prefix,
                suffix,
                params,
                reply,
            } => {
                let res = run_cached(
                    backend,
                    &model,
                    context_size,
                    &mut session,
                    &prefix,
                    &suffix,
                    &params,
                );
                let _ = reply.send(res);
            }
            Request::Embed { text, reply } => {
                let res = run_embed(backend, &model, context_size, pooling, &text);
                let _ = reply.send(res);
            }
        }
    }
}
```

### Step 2.4: 编译 + 工程测试 + clippy + fmt

- [ ] 运行：

```bash
cargo build -p locifind-model-runtime --features llama-cpp
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
# 关键：model-runtime default = ["stub"]、workspace clippy 不开 llama-cpp
# 必须显式跑 llama-cpp feature 检 llama.rs / pooling.rs
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo fmt --all --check
```

**Expected**：编译成功 / workspace test 0 failed / clippy 两次都 0 warning / fmt 净。
若 build 报错最常见原因 = `detect_model_pooling` 中 `crate::pooling::detect_pooling_type` 路径解析失败（Task 1 step 1.2 的 `mod pooling;` 没加对、或 `pub(crate)` 漏写）—— 回 Task 1 修。

### Step 2.5: qwen3-0.6b byte-equal 红线手验（spec §2.2 验收 (6)）

> **关键回归门**：现行 qwen3-embedding-0.6b GGUF 声明 `qwen3.pooling_type = 3` → 新 detect 应映射到 `LlamaPoolingType::Last`、与旧硬编码完全一致 → 重 embed 出来的 `vectors.json` 必须与旧文件字节相等。若不等 = infra 改动有未预期副作用、Branch IV 异常、**阻止合并**。

- [ ] 备份旧 vectors.json：

```bash
cp packages/evals/fixtures/semantic-recall/vectors.json /tmp/vectors-qwen3-0.6b-baseline.json
```

- [ ] 跑 qwen3-0.6b 重 embed（Mac Metal + release）：

```bash
cargo run -p locifind-evals --features metal --bin semantic_quality --release -- \
  --embed \
  --model models/qwen3-embedding-0.6b-q8_0.gguf
```

（`--vectors-file` 不带、走默认 `packages/evals/fixtures/semantic-recall/vectors.json` —— BETA-15B-7 v4 cycle 已加该 flag，默认值即此路径。若该 binary 还有其他必填 flag 看 `--help` 或参考 BETA-15B-7 v4 plan 的同步骤命令。）

- [ ] 比对 byte-equal：

```bash
diff -q /tmp/vectors-qwen3-0.6b-baseline.json packages/evals/fixtures/semantic-recall/vectors.json
```

**Expected**：无输出（exit 0 = byte-equal）。
若有输出 = vectors.json 变化、说明 detect_model_pooling 走到了错误分支、回 Step 2.1-2.3 debug（最可能：`qwen3.pooling_type` metadata 读不到走了 heuristic、但 heuristic qwen3→Last 也是 Last、不该差—— 应进一步用 Python GGUF dumper 重新核对 metadata）。

**异常路径**：若 byte diff 不空、**不要继续 Task 2 commit**。回头调查根因：① `cargo run` 是否真用了 --features metal 而非 cpu fallback；② detect_model_pooling 是否真返回 Last；③ pooling.rs 单测是否真过；④ worker_main 修改是否正确编译进了二进制。

- [ ] 还原旧 vectors.json（byte-equal 验证完成后无需保留差异）：

```bash
git checkout packages/evals/fixtures/semantic-recall/vectors.json
# 等价：若 diff 空、文件本就一致、可省略
```

### Step 2.6: Commit Task 2

- [ ] 运行：

```bash
git add packages/model-runtime/src/llama.rs
git commit -m "$(cat <<'EOF'
BETA-15B-8 task 2：llama.rs 接入 pooling detection + qwen3-0.6b byte-equal 红线过

三处改动：① 加 detect_model_pooling(&LlamaModel) thin adapter（读 general.architecture + <arch>.pooling_type、调 pooling::detect_pooling_type）；② worker_main model 加载后立刻 detect 一次、失败 ready_tx.send(Err) 早退、成功存为函数局部变量；③ run_embed 签名扩 pooling: LlamaPoolingType 参数、删硬编码 LlamaPoolingType::Last 错配 bug。验证：cargo build / workspace test 0 failed / clippy 0 / fmt 净 + qwen3-embedding-0.6b 重 embed 后 vectors.json byte-equal（spec §2.2 验收 (6) 红线、infra 改动行为透明硬证据、零回归）。
EOF
)"
```

---

## Task 3: bge-m3 真水位重 embed + 9 阈值 sweep + vectors-bge-m3.json 覆盖入仓

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/vectors-bge-m3.json`（v4 错配版本被新版覆盖）

**说明**：infra 修复的端到端自验。bge-m3 在修复后用 Cls pooling、向量必然变化（与 v4 错配 Last 版不同）；跑 9 阈值 sweep 拿真水位、Task 4 写进 baseline 报告 v4-fixup 节。

### Step 3.1: 备份 v4 错配版本 vectors-bge-m3.json（用于 Task 4 对照）

- [ ] 运行：

```bash
cp packages/evals/fixtures/semantic-recall/vectors-bge-m3.json \
   /tmp/vectors-bge-m3-v4-mismatch.json
```

### Step 3.2: bge-m3 重 embed（Mac Metal + release）

- [ ] 运行：

```bash
# 注：`--vectors-file` 传 basename 即可；evals binary `semantic_quality.rs:55 fixt()`
# 会重新 join `packages/evals/fixtures/semantic-recall/` 前缀、传全路径会触发 panic。
cargo run -p locifind-evals --features metal --bin semantic_quality --release -- \
  --embed \
  --model models/bge-m3-q8_0.gguf \
  --vectors-file vectors-bge-m3.json
```

**Expected**：文件被覆盖、大小与原来相近（~2-3 MB 量级、与 qwen3-0.6b vectors.json 同款 schema）、命令 exit 0。

### Step 3.3: 验向量非全零、L2 norm ≈ 1（健康性检查）

- [ ] 运行：

```bash
python3 <<'PY'
import json, math
with open("packages/evals/fixtures/semantic-recall/vectors-bge-m3.json") as f:
    data = json.load(f)
# 抽样验证前 3 个 doc + 前 3 个 query 向量
def check(label, vec):
    nz = sum(1 for x in vec if x != 0)
    l2 = math.sqrt(sum(x*x for x in vec))
    print(f"  {label}: dim={len(vec)} nonzero={nz}/{len(vec)} L2={l2:.4f}")
print("=== bge-m3 vectors health check ===")
# 适配 vectors.json schema —— 与 v4 cycle 同款（docs/queries 字段）
for k in ("docs", "queries"):
    if k in data:
        items = data[k][:3]
        for i, item in enumerate(items):
            v = item.get("vector") or item.get("embedding") or item.get("vec")
            if v is not None:
                check(f"{k}[{i}]", v)
PY
```

**Expected**：每条向量 nonzero ≈ dim（≥95% 非零）、L2 ≈ 1.000（±0.001 内、L2 norm 在 run_embed 内做了）。
若全零或 L2 远离 1.0 = bge-m3 推理异常、调查（可能 Task 2 改动错位、metal feature 没真启用、bge-m3 GGUF 文件本身问题）—— 不继续后续 step。

### Step 3.4: 跑 9 阈值 sweep × bge-m3 收集真水位 nDCG 数据

- [ ] 运行（9 阈值依次跑、output 重定向到 /tmp 文件待 Task 4 提取）：

```bash
# 同上：`--vectors-file` 传 basename、fixt() 会重新 join 前缀。
mkdir -p /tmp/beta-15b-8-sweep
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== T=$t ===" | tee -a /tmp/beta-15b-8-sweep/bge-m3.log
  cargo run -p locifind-evals --features metal --bin semantic_quality --release -- \
    --vectors-file vectors-bge-m3.json \
    --semantic-weight 10.0 \
    --cosine-threshold $t \
    2>&1 | tee -a /tmp/beta-15b-8-sweep/bge-m3.log
done
```

（与 BETA-15B-7 v4 plan task 4 同款命令、sweep 9 阈值 = `{0.0, 0.30, 0.45, 0.60, 0.70, 0.80, 0.90, 0.99, 1.01}`。）

**Expected**：9 次运行、每次输出含 6 桶 nDCG（exact-name / synonym / concept / crosslang / content-not-name / OVERALL）的 HYB / VEC / HYBR 三组数据。控制对照应满足：T=0.0 时 HYBR≈VEC、T=1.01 时 HYBR≡HYB。

### Step 3.5: 提取 sweep 表给 Task 4 用

- [ ] 运行（人工读 `/tmp/beta-15b-8-sweep/bge-m3.log`、提取 9 阈值 × 6 桶 nDCG 数值、整理成 markdown 表）：

```bash
cat /tmp/beta-15b-8-sweep/bge-m3.log | grep -E "T=|OVERALL|exact-name|synonym|concept|crosslang|content-not-name|HYBR_N|VEC_N|HYB_N" > /tmp/beta-15b-8-sweep/bge-m3-extracted.txt
head -100 /tmp/beta-15b-8-sweep/bge-m3-extracted.txt
```

（具体表格化的 awk/python 由 implementer 视 stdout 实际格式自行写、产物供 Task 4 step 4.3 用。）

### Step 3.6: Commit Task 3

- [ ] 运行：

```bash
git add packages/evals/fixtures/semantic-recall/vectors-bge-m3.json
git commit -m "$(cat <<'EOF'
BETA-15B-8 task 3：bge-m3 真水位重 embed + 9 阈值 sweep + vectors-bge-m3.json 覆盖入仓

infra 修复（Task 2 落地）后用 Mac Metal 重跑 bge-m3 embed：v4 错配版本（hardcoded Last pooling、向量看似近但 last-token state collapse）被新版（按 GGUF bert.pooling_type=2 = Cls 检测后用 CLS pooling）覆盖。健康性：所有抽样向量非全零、L2 norm ≈ 1.000。9 阈值 sweep（cosine-threshold ∈ {0.0, 0.30, 0.45, 0.60, 0.70, 0.80, 0.90, 0.99, 1.01}、W=10.0）数据收集到 /tmp/beta-15b-8-sweep/bge-m3.log、Task 4 写进 baseline 报告 v4-fixup 节。
EOF
)"
```

---

## Task 4: 文档校正 + baseline 报告 v4-fixup 节

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（line 668-671 MEAN→CLS 校正 + 追加 v4-fixup 节）
- Modify: `docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md`（v4 spec MEAN→CLS 一处校正）

**说明**：本 cycle spec §4.4 关联 doc fixup（v4 cycle 把 `bert.pooling_type=2` 标注成「MEAN」是 fact 错误、实际是 CLS）+ baseline 报告追加 v4-fixup 节（bge-m3 真水位 sweep 表 + 与 v4 错配对照 + GO/GO（无破局）二级判定）。

### Step 4.1: v4 baseline 报告 line 668-671 MEAN→CLS 校正

- [ ] 编辑 `docs/reviews/semantic-recall-quality-baseline.md`：
  - line 668 `bert.pooling_type = 2`（MEAN） → `bert.pooling_type = 2`（CLS）
  - line 669 「BERT 架构标准用 CLS token 或 mean pooling 提取句向量」 → 「实测 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3, RANK=4}`；bge-m3 GGUF 声明值=2=CLS、与 bge-m3 model card `[CLS]` token + L2 norm 一致；本 cycle (BETA-15B-8) 在 model-runtime infra 修复后跑出真水位、详 v4-fixup 节」
  - line 670-671 表述微调（若有「错配 MEAN」字样改为「错配 CLS」）

具体 line 号若与当前文件略有偏差，按 `grep -n 'bert.pooling_type' docs/reviews/semantic-recall-quality-baseline.md` 定位实际行。

### Step 4.2: v4 spec MEAN→CLS 校正

- [ ] 编辑 `docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md`：
  - 在 v4 spec §1.3 cycle 末修订说明节、或 spec 任意提到「`bert.pooling_type=2` (MEAN)」的位置改为「(CLS、参 BETA-15B-8 fact-check)」
  - 用 `grep -n 'MEAN' docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md` 定位、按上下文判断是否需校正（只校正涉及 bert.pooling_type=2 这个具体值的位置、不动其他 MEAN 字样如「mean pooling」概念解释）

### Step 4.3: 追加 baseline 报告 v4-fixup 节（在 v4 节之后、其他历史节之前）

- [ ] 编辑 `docs/reviews/semantic-recall-quality-baseline.md`、在 v4 节末尾之后追加一节：

```markdown
### v4-fixup 数据集节 — model-runtime pooling type detection 修复后 bge-m3 真水位（BETA-15B-8）

承接 v4 cycle 的 Branch IV-A 推断（bge-m3 因 `LlamaPoolingType::Last` 硬编码错配 bert arch 声明的 CLS pooling）。BETA-15B-8 cycle 修复 `packages/model-runtime/src/llama.rs` 硬编码、改为按 GGUF `<arch>.pooling_type` 动态检测：
- 抽 `packages/model-runtime/src/pooling.rs` 纯逻辑模块 + 9 单测覆盖
- `worker_main` 加载 model 后 detect pooling 一次、存为函数局部变量
- `run_embed` 签名扩 `pooling: LlamaPoolingType` 参数
- qwen3-embedding-0.6b 重 embed 后 `vectors.json` byte-equal 验证零回归（GGUF 声明 `qwen3.pooling_type=3` → Last、与旧硬编码一致）
- bge-m3 用 CLS pooling 重 embed、`vectors-bge-m3.json` 覆盖 v4 错配版本

**fact-check 校正**：v4 节 line 668 把 `bert.pooling_type=2` 标注为「MEAN」是 fact 错误。按 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3, RANK=4}`，值 2 实际是 CLS（与 bge-m3 model card 明示的 `[CLS]` token + L2 norm 一致）。修复方向不变（硬编码 Last 错配 bge-m3 声明的 CLS）。

**bge-m3 真水位 sweep**（v3 数据集 78 cases / 124 docs / dim 1024、CLS pooling、W=10.0）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | <填> | <填> | <填> | <填> |
| 0.30 | <填> | <填> | <填> | <填> |
| 0.45 | <填> | <填> | <填> | <填> |
| 0.60 | <填> | <填> | <填> | <填> |
| 0.70 | <填> | <填> | <填> | <填> |
| 0.80 | <填> | <填> | <填> | <填> |
| 0.90 | <填> | <填> | <填> | <填> |
| 0.99 | <填> | <填> | <填> | <填> |
| 1.01 (≡HYB) | <填> | <填> | <填> | <填> |

（数据由 Task 3 step 3.5 `/tmp/beta-15b-8-sweep/bge-m3-extracted.txt` 提供、人工填入。）

**v4 错配 vs v4-fixup 对照**（按各模型 best OVERALL HYBR_N 比对）：

| 指标 | v4 错配 (Last) | v4-fixup (CLS) | Δ |
|---|---|---|---|
| best OVERALL HYBR_N | 0.770 | <填> | <填> |
| best crosslang HYBR_N | 0.543 | <填> | <填> |
| best content-not-name HYBR_N | 0.822 | <填> | <填> |

**判定**（按 BETA-15B-8 spec §2.3）：

- 若 bge-m3 CLS pooling 后 OVERALL / crosslang / content-not-name 任一桶相比 v4 Last 错配版本明显提升 → **GO（infra 修复有数据指证）**、若再相比 v3 qwen3-0.6b（OVERALL 0.856 / crosslang 0.717 / content-not-name 0.870）双过 spec 目标 → 开 follow-up cycle BETA-15B-7-v2 启动 bake 决策
- 若 bge-m3 真水位 vs v3 qwen3-0.6b 全桶持平或弱 → **GO（无破局）**、infra 层修复仍成立（解阻塞）、移交下 cycle 抓手 = 更大跨族（bge-multilingual-gemma2 9B）/ 评测扩量 / 微调
- 若 bge-m3 真水位反而比 v4 Last 错配版本退步 → **异常**（更不该发生）、调查根因不发布

（具体判定文字由 implementer 按实测数据填写、不预设结论。）

**下 cycle 抓手优先级修正**（v4-fixup 数据指证）：

| 抓手 | 优先级（v4-fixup 修订）|
|---|---|
| 升 llama-cpp-4 0.3.0 / 解 qwen3-8b 全零 bug（BETA-15B-Y）| <按 bge-m3 真水位决定：若 bge-m3 双过 spec 目标 → 中优；若 bge-m3 不破 → 高优、Qwen3 系最大档真测仍是抓手>|
| BETA-15B-7-v2 重跑（修完 infra 后同 spec / 同 model / 同 sweep 拿全模型真水位）| 依赖上一项 |
| 跨厂替代候选（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B）| <按 bge-m3 真水位决定>|
| 评测扩量 | 低优、若以上无效再做 |

**链接**：[BETA-15B-8 spec](../superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md) / [BETA-15B-8 plan](../superpowers/plans/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection.md) / [pooling.rs 模块](../../packages/model-runtime/src/pooling.rs)
```

按 Task 3 实测数据填 `<填>` 占位、判定按数据走、不预设结论。

### Step 4.4: 跑 fmt + Markdown lint（如有）+ 验工程不破

- [ ] 运行：

```bash
cargo fmt --all --check
cargo test --workspace
```

**Expected**：fmt 净 / workspace test 不破。文档改动不影响 Rust 编译、本步骤主要确认未误改代码文件。

### Step 4.5: Commit Task 4

- [ ] 运行：

```bash
git add docs/reviews/semantic-recall-quality-baseline.md \
        docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md
git commit -m "$(cat <<'EOF'
BETA-15B-8 task 4：v4 doc fact-check 校正（MEAN→CLS）+ baseline 报告追加 v4-fixup 节

① v4 baseline 报告 line 668-671 校正：bert.pooling_type=2 标注从「MEAN」改为「CLS」（按 llama.cpp upstream 枚举 LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3, RANK=4}、与 bge-m3 model card [CLS] token + L2 norm 一致）；② v4 spec 同款一处校正；③ baseline 报告追加 v4-fixup 节：bge-m3 CLS pooling 真水位 9 阈值 sweep 表 + 与 v4 错配 Last 版本对照 + GO/GO（无破局）/异常 三档判定（按实测数据走、不预设结论）+ 下 cycle 抓手优先级修正（按 bge-m3 真水位决定）。修复方向不变（硬编码 Last 错配是 bug）、仅 fact 表述校正。
EOF
)"
```

---

## Task 5: 总验收 + STATUS / ROADMAP doc-sync + cycle 收口

**Files:**
- Modify: `STATUS.md`（当前 Task / 下一步 / 会话日志顶部追加）
- Modify: `ROADMAP.md`（BETA-15B-8 task 卡片登记 + 状态 done）

**说明**：cycle 收口、按 [CONVENTIONS §3 收工流程](../../../CONVENTIONS.md) 更新 STATUS + ROADMAP + 最终 commit。

### Step 5.1: 总验收 5 项全过

- [ ] 运行：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
# 显式 llama-cpp feature 检 llama.rs / pooling.rs（workspace default 不开）
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo test --workspace
# pooling.rs 9 单测在 llama-cpp feature 下编译、需显式跑
cargo test -p locifind-model-runtime --features llama-cpp pooling::
cargo test -p locifind-evals --test semantic_quality_gate
# evals parser-only byte-equal 子集（v0.5 + v0.9 精确不变、本 cycle 不动 parser/coverage）
cargo run -p locifind-evals --bin parser_eval --release -- --json > /tmp/parser-current.json
# 与本 cycle 起点对照（main 上 094e7d0 commit 后的 v0.5=473/25/2 + v0.9=877/119/4 应不变）
# 若有项目内规范化脚本走规范化、否则直接看 stdout 报表 line（详记忆 [[project-evals-reporter-nondeterministic]]）
```

**Expected**：
- fmt 净
- clippy 0 warning
- workspace test 0 failed（含本 cycle 新增 8 个 pooling 单测）
- semantic_quality_gate 1 passed（4 红线全过、baseline.json 未动）
- evals parser-only byte-equal：v0.5=473/25/2 + v0.9=877/119/4 精确不变（本 cycle 不动 parser/coverage、必然成立但走一遍确认）

### Step 5.2: vectors 文件状态验证（spec §2.2 (6)(7)(8)(9)）

- [ ] 运行：

```bash
git status packages/evals/fixtures/semantic-recall/
git diff --stat packages/evals/fixtures/semantic-recall/vectors.json
git diff --stat packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json
git diff --stat packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
git log --oneline -5 packages/evals/fixtures/semantic-recall/vectors-bge-m3.json
```

**Expected**：
- `vectors.json`：unchanged（diff 空）
- `vectors-qwen3-0.6b.json`：unchanged（diff 空）
- `vectors-qwen3-8b.json`：unchanged（diff 空）
- `vectors-bge-m3.json`：Task 3 commit 中被覆盖入仓（git log 应显示 Task 3 commit）

若 vectors.json / vectors-qwen3-0.6b.json / vectors-qwen3-8b.json 任一有 diff = Branch IV 异常、阻止合并、回头排查（最可能：Task 2 / 3 误把别的模型 embed 写到了错的文件、或 --vectors-file flag 没正确传）。

### Step 5.3: 更新 STATUS.md

- [ ] 编辑 `STATUS.md`：

**(a)** 「当前阶段」节：在 M/B 阶段进度行末加一句：「BETA-15B-8 model-runtime pooling type detection done（infra 修复、bge-m3 CLS pooling 真水位拿出）」

**(b)** 「当前 Task」节：整段替换为 BETA-15B-8 done 摘要（参照 BETA-15B-7 v4 cycle 同款风格、含「做了什么」+「bge-m3 真水位 vs v4 错配对照」+「诚实边界结论」+「下 cycle 抓手优先级修正」+「未尽事宜」五段）

**(c)** 「会话日志」节：**顶部**追加新条：

```markdown
### 2026-06-25 — Claude Code (Opus 4.7) — BETA-15B-8 model-runtime pooling type detection done + [PR #13](https://github.com/raoliaoyuan/LociFind/pull/13) 已合 main（merge commit `5305ee1`）⭐ infra 阻塞解除 + bge-m3 真水位拿出

**承接**：BETA-15B-7 v4 cycle merge 后用户「继续」→ 启动 v4 数据指证最高优抓手 = 修 model-runtime pooling type detection。完整 superpowers 全流程：brainstorming Q1-Q4 → spec → plan 5 task → subagent-driven 驱动 + 每 task 双审 + final integration review + cycle 末统一 commit。

**关键决策（brainstorming Q1-Q4 收敛）**：① 范围 = **只修 pooling type detection**（最窄、不动 llama-cpp-4 / 不动 BETA-15B-7-v2 重跑）；② fallback = architecture-based default + 未知 arch fail-fast；③ 验证边界 = 代码 + qwen3-0.6b byte-equal + bge-m3 重跑拿真水位（不动 baseline.json / 不 bake）；④ 代码组织 = 抽 helper + 纯逻辑单测 + load 时算一次。

**产出（5 task 全 done、N commits 落 main）**：
- T1 pooling.rs 纯逻辑模块 + 9 单测全 pass
- T2 llama.rs 接入 + qwen3-0.6b byte-equal 红线过（零回归硬证据）
- T3 bge-m3 重 embed + 9 阈值 sweep 拿真水位 + vectors-bge-m3.json 覆盖入仓
- T4 v4 baseline / spec MEAN→CLS fact-check 校正 + 追加 v4-fixup 节
- T5 总验收 + STATUS/ROADMAP doc-sync + cycle 收口

**bge-m3 真水位 vs v4 错配对照**：<按 Task 3/4 实测数据填、不预设结论>

**诚实边界结论**：<按数据走 GO / GO（无破局）/ 异常 三档判定>

**下 cycle 抓手优先级修正（v4-fixup 数据指证）**：<按 bge-m3 真水位决定>

**未尽事宜**：① <按实测填>；② PR / GitHub 合 main 状态。

---
```

（具体数据 / 结论 / 抓手优先级由 implementer 按 Task 3/4 实测填、不预设。）

### Step 5.4: 更新 ROADMAP.md

- [ ] 编辑 `ROADMAP.md` §3.3 B 阶段 BETA-15B 系列：

**(a)** 找 BETA-15B-7 task 卡片附近、追加新 task 卡片 BETA-15B-8：

```markdown
- **BETA-15B-8**：model-runtime pooling type detection（按 GGUF metadata 选 LlamaPoolingType 替换硬编码 Last）。状态：done（2026-06-25、[PR #13](https://github.com/raoliaoyuan/LociFind/pull/13)、merge commit `5305ee1`）。承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项；新增 packages/model-runtime/src/pooling.rs 纯逻辑模块 + 9 单测 + llama.rs detect_model_pooling adapter + worker_main detect 一次 + run_embed 签名扩 pooling 参数；qwen3-embedding-0.6b 重 embed 后 vectors.json byte-equal（零回归硬证据）+ bge-m3 CLS pooling 真水位拿出（解 v4 Branch IV-A 错配诊断）+ v4 doc MEAN→CLS fact-check 校正。Spec：docs/superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md
```

**(b)** 若 ROADMAP §5 长周期事项 / §3.3 BETA-15B-7 卡片中有「BETA-15B-X model-runtime pooling type detection」placeholder、把 BETA-15B-X 替换为 BETA-15B-8 + 状态从「待启」改为「done（2026-06-25）」。

### Step 5.5: cycle 末 commit（含 spec + plan + STATUS + ROADMAP）

- [ ] 运行：

```bash
git add docs/superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md \
        docs/superpowers/plans/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection.md \
        STATUS.md ROADMAP.md
git commit -m "$(cat <<'EOF'
BETA-15B-8 spec + plan + STATUS/ROADMAP doc-sync 落库 + cycle 收口

承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中最高优抓手 = 修 model-runtime pooling type detection、按 GGUF metadata <arch>.pooling_type 动态选 LlamaPoolingType + arch heuristic fallback、解硬编码 Last 错配 cross-arch embedding 评测的 infra 缺陷。完整 superpowers 全流程（brainstorming Q1-Q4 → spec → plan 5 task → subagent-driven 实施 + 每 task 双审 + final integration review）。本 commit 含本 cycle spec / plan 作 cycle 文献 + STATUS 当前 Task / 会话日志 / ROADMAP §3.3 BETA-15B-8 task 卡片 done 登记 + v4 baseline / spec MEAN→CLS fact-check 校正（已在 task 4 commit 落、本 commit 不重复）。
EOF
)"
```

### Step 5.6: PR + 合 main

- [ ] 推 feature branch、开 PR、合 main（按 LociFind 仓常规流程）。若 `gh` CLI 401 凭据问题、走本地 merge + push origin/main + 删本地+远程 feature branch（与 BETA-15B-7 PR #12 同款流程）。

具体命令（视分支情况）：

```bash
# 假设当前在 feat-beta-15b-8 分支、main 已 fast-forward 可合
git push -u origin feat-beta-15b-8
# 开 PR（gh 可用时）
gh pr create --title "BETA-15B-8 model-runtime pooling type detection" \
  --body "见 [spec](docs/superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md) + [plan](docs/superpowers/plans/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection.md) + [v4-fixup baseline 节](docs/reviews/semantic-recall-quality-baseline.md#v4-fixup)"
# 合并 + 删分支
gh pr merge --merge --delete-branch
```

或本地 merge fallback（参照 BETA-15B-7 v4 cycle 会话日志 PR #12 同款流程）。

### Step 5.7: 收工

- [ ] 向用户确认 cycle done、报告 PR 编号 / merge commit / bge-m3 真水位判定结果 / 下 cycle 抓手优先级建议。

---

## Self-Review（plan 写完后自检、修任何 gap）

**Spec coverage check** — spec §2.1 目标 8 项是否每项都有 task 实现：

1. ✅ 新增 pooling.rs → Task 1
2. ✅ worker_main detect + 局部变量 + run_embed 扩参 + 删硬编码 → Task 2
3. ✅ detect_model_pooling adapter → Task 2 step 2.1
4. ✅ pooling::detect_pooling_type 纯函数 → Task 1 step 1.6
5. ✅ 单测 5 路径 → Task 1 step 1.1 + 1.4-1.6（8 个 case）
6. ✅ qwen3-0.6b byte-equal + bge-m3 重 embed + sweep → Task 2 step 2.5 + Task 3
7. ✅ baseline 报告 v4-fixup 节 → Task 4 step 4.3
8. ✅ v4 spec MEAN→CLS fixup → Task 4 step 4.2

**spec §2.2 验收 10 红线全有 task 覆盖**：
- (1)(2)(3) cargo test / clippy / fmt → Task 5 step 5.1
- (4) gate.rs baseline.json 不动 → Task 5 step 5.1
- (5) evals parser byte-equal → Task 5 step 5.1
- (6) vectors.json byte-equal → Task 2 step 2.5
- (7) vectors-qwen3-0.6b.json 不动 → Task 5 step 5.2
- (8) vectors-bge-m3.json 必变 → Task 3 step 3.2
- (9) vectors-qwen3-8b.json 不动 → Task 5 step 5.2
- (10) pooling 三模型 arch 单测覆盖 → Task 1 step 1.1（`detect_pooling_type_metadata_overrides_heuristic` + `detect_pooling_type_missing_metadata_uses_heuristic`）

**Placeholder scan**：✅ 无 TBD/TODO；`<填>` / `<编号>` 占位均明示「按实测数据填」/「按 cycle 落定填」、是合理 deferred 项。

**Type consistency**：
- `LlamaPoolingType` 全文一致
- `ModelError::LoadError` 全文一致（已校正 Spec 中 ModelLoadError 笔误）
- `detect_pooling_type` / `default_pooling_for_arch` / `map_gguf_pooling_value` / `detect_model_pooling` 函数名前后一致
- `worker_main` 函数名前后一致

**修订记录**：plan 写作过程中发现 spec 三处描述与实际代码不符（`ModelError::ModelLoadError` → `LoadError`、"worker spawn 闭包" → "`worker_main` 函数"、§4.2 数据流图未含 `ready_tx.send(Err)` 早退路径），已在 spec 同步修正、plan 与修正后的 spec 一致。

---

## 执行选项

**Plan complete and saved to `docs/superpowers/plans/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection.md`。**

两种执行模式（按 LociFind 历史 cycle 惯例推荐 1）：

1. **Subagent-Driven（推荐）** — fresh subagent per task + 每 task spec/code-quality 双审 + final integration review、与 BETA-15B-7 v4 cycle 同款节奏
2. **Inline Execution** — 当前会话连续执行 5 task、checkpoint 在 Task 2 末 byte-equal 红线 / Task 3 末 bge-m3 sweep 结果 / Task 4 末 baseline 节填数据

**选哪个？**
