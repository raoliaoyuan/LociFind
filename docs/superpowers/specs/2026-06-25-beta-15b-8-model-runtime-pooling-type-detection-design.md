# BETA-15B-8：model-runtime pooling type detection（按 GGUF metadata 选择 LlamaPoolingType）设计 spec

> 承接 BETA-15B-7 v4 cycle（embedding 模型跨族 + 同族最大档探针、PR #12 已合 main、commit `094e7d0`）的**双 Branch IV-infra 诊断**。本 cycle 修复其中阻塞所有 cross-arch embedding 评测的最高优 infra 缺陷 = `packages/model-runtime/src/llama.rs:357` 硬编码 `LlamaPoolingType::Last`，改为按 GGUF metadata `<arch>.pooling_type` 动态选择 + architecture-based heuristic fallback。
>
> **修订说明**：本 spec 起草前实测三模型 GGUF metadata（[packages/model-runtime/src/llama.rs:357](../../../packages/model-runtime/src/llama.rs#L357)、`qwen3-embedding-0.6b-q8_0.gguf` / `bge-m3-q8_0.gguf` / `qwen3-embedding-8b-q8_0.gguf`），发现 v4 baseline 报告 line 668-669 把 `bert.pooling_type=2` 标注为「MEAN」是 fact 错误——按 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3}`（注：llama-cpp-4 0.3.0 binding 未暴露 `Rank=4`、留 reranker 用），**值 2 实际是 CLS**（与 bge-m3 model card 明示的 `[CLS]` token + L2 norm 一致）。修复方向不变（硬编码 Last 错配 = bug），但本 cycle 收尾时顺手做 doc fixup 校正 v4 节叙述。
>
> **范围哲学**：纯 model-runtime infra 修复 + bge-m3 端到端自验。生产 wiring / `DEFAULT_EMBEDDING_MODEL_PATH` / baseline.json / llama-cpp-4 version / 模型分发**全部不动**。bge-m3 真水位若破 v3 0.6b、bake 推到生产是独立 follow-up cycle BETA-15B-7-v2 的事。
>
> **与 BETA-15B-7-v2 / BETA-15B-Y 的关系**：v4 cycle 在 baseline 报告 line 707-712 / 751-752 登记三独立下 cycle 抓手——本 cycle 是 placeholder **BETA-15B-X 的具体化（ID 落定为 BETA-15B-8）**。BETA-15B-7-v2（修完 infra 后同 spec/同 model/同 sweep 重跑拿全模型真水位）依赖本 cycle + BETA-15B-Y（llama-cpp-4 升级 / qwen3-8b 全零解 bug）；BETA-15B-Y 不在本 cycle 范围。

## 1. 背景与动机

### 1.1 v4 cycle 暴露的 infra 缺陷

BETA-15B-7 v4 cycle 用「评测纯探针」路线测两条独立的「更大 / 更强 embedding 模型」轴：

- **跨族架构轴 bge-m3**（BAAI、~568M、BERT、多语言 SOTA、同尺寸 vs qwen3-0.6b）
- **同族最大档轴 qwen3-embedding-8b**（Qwen 官方、~8B、Qwen3 系列最大）

两条独立轴**都被 model-runtime / llama-cpp-4 infrastructure 层阻断**，cycle 实际产出 = infra 层缺陷诊断而非模型层数据指证。bge-m3 失败根因：

- bge-m3 是 BERT 架构（`general.architecture = bert`、实测 GGUF metadata）
- bge-m3 GGUF 声明 `bert.pooling_type = 2`（实测 `CLS`，**非** v4 doc 误写的 MEAN；与 bge-m3 model card [CLS]+L2 norm 一致）
- 现 [`packages/model-runtime/src/llama.rs:357`](../../../packages/model-runtime/src/llama.rs#L357) 硬编码 `LlamaPoolingType::Last`，**覆盖 GGUF 声明**
- 结果：用 last-token state 取 BERT 模型的「向量」，语义偏（cosine top1 mean 0.719 > qwen3-0.6b 0.660、但 nDCG 反而低 -0.086 = 典型 last-token state collapse）

v4 cycle 主动放弃合并字面 GO/NO GO 模型层结论：「**不能下『bge-m3 比 qwen3-0.6b 弱』结论**——测的是错配 pooling」，明示「修 model-runtime pooling type detection」为下 cycle **最高优**抓手（baseline 报告 line 705-712）。

### 1.2 本 cycle 的命题

> **从 GGUF metadata 动态读 `<arch>.pooling_type` 替代硬编码 Last，infra 修复后 bge-m3 真水位是否破 v3 0.6b 的 OVERALL/crosslang？**

修复后 cycle 产出两层信息：

1. **infra 层闭环**：硬编码 → metadata-driven、未来 cross-arch（EmbeddingGemma / jina / nomic-embed 等）零代码改动可评测
2. **模型层数据指证**（B 路径验证）：bge-m3 真水位 vs qwen3-0.6b 的 OVERALL / crosslang / content-not-name 对照，给独立 follow-up cycle BETA-15B-7-v2 / bake 决策提供数据

### 1.3 为什么是本 cycle 最高优

- **零回归**（实测）：qwen3-0.6b `qwen3.pooling_type=3 → Last`、与现硬编码一致、`vectors.json` 重 embed 后必然 byte-equal、生产侧零行为变化
- **阻塞抓手**：v4 cycle 已明示 = 「阻塞所有 cross-arch embedding 评测」
- **API surface 完全就绪**：llama-cpp-2 0.1.146（llama-cpp-4 0.3.0 底层）`LlamaModel::meta_val_str(key, buf_size)` + `LlamaPoolingType::{None, Mean, Cls, Last}` 枚举可用（注：llama-cpp-4 0.3.0 binding 未暴露 `Rank=4`、留 reranker 用），**不用碰 llama-cpp-4 版本 / 不用 file upstream issue**。注：起草时引用了 `llama-cpp-2 0.1.146` 的旧签名 `meta_val_str(key)`、实测 `llama-cpp-4 0.3.0` binding 实际签名 = `meta_val_str(key, buf_size)`、本 cycle 用 `META_BUF=256`（详 [packages/model-runtime/src/llama.rs](../../../packages/model-runtime/src/llama.rs) `detect_model_pooling` 实现）。
- **可纯逻辑 TDD**：detect 拆纯函数 `(arch: &str, meta: Option<i64>) → Result<LlamaPoolingType>`、单测不挂 GGUF
- **可端到端自验**：bge-m3 重 embed + sweep 拿真水位，infra 修复有数据指证

## 2. 目标与验收

### 2.1 目标

- `packages/model-runtime/src/` 新增 `pooling.rs` 模块（纯逻辑、不依赖 LlamaModel）
- `packages/model-runtime/src/llama.rs` `worker_main` 函数内 model 加载后 detect pooling 一次、存为函数局部变量 `pooling: LlamaPoolingType`、`run_embed` 签名扩 `pooling` 参数、删硬编码 `LlamaPoolingType::Last`
- 新增 thin adapter `detect_model_pooling(&LlamaModel) -> Result<LlamaPoolingType, ModelError>`：读 `general.architecture` + `<arch>.pooling_type`、调纯函数
- 纯函数 `pooling::detect_pooling_type(arch: &str, meta: Option<i64>) -> Result<LlamaPoolingType, ModelError>`：metadata 优先、缺失走 arch heuristic、未知 arch fail-fast
- 单测覆盖 metadata-present / arch-heuristic / unknown-arch-fail / metadata 越界 五路径（不挂 GGUF）
- 集成手验：① qwen3-0.6b 重 embed → `vectors.json` byte-equal 红线；② bge-m3 重 embed → `vectors-bge-m3.json` 覆盖 v4 错配版本、跑 9 阈值 sweep 拿真水位
- baseline 报告追加 **v4-fixup 节**：bge-m3 真水位 sweep 表 + v4 错配对照 + MEAN/CLS doc 校正备注
- v4 spec [docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md](2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md) doc fixup（MEAN→CLS 一处）

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed（含本 cycle 新增 ~5-8 纯逻辑单测）
(2) `cargo clippy --workspace --all-targets -D warnings` 0 warning
(3) `cargo fmt --all --check` 净
(4) `semantic_quality_gate` 用**现 baseline.json**（v3 0.6b 数据）pass、4 红线全过、**本 cycle 不动 baseline.json / 不动 gate.rs 任何断言**
(5) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser / coverage）
(6) **`packages/evals/fixtures/semantic-recall/vectors.json` 字节不动**（qwen3-0.6b 重 embed 出来与旧文件 byte-equal、infra 改动行为透明的硬证据；若不一致 = 本 cycle 引入了未预期的副作用、Branch IV 异常、阻止合并）
(7) `vectors-qwen3-0.6b.json` 字节不动（v4 cycle 入仓的同款 copy）
(8) `vectors-bge-m3.json` 必然变化（pooling 从 Last 改 Cls、向量必不同）；新版入仓覆盖 v4 错配版本
(9) `vectors-qwen3-8b.json` 字节不动（pooling 与 8b 全零无关、本 cycle 不解 8b）
(10) `pooling::detect_pooling_type` 三模型 arch 行为单测覆盖：`("qwen3", Some(3))→Last` / `("bert", Some(2))→Cls` / `("bert", None)→Cls`（heuristic 与 metadata 一致性的回归保护）

### 2.3 GO / 异常判定

| 情景 | 解读 | 行动 |
|---|---|---|
| **GO** | (1)-(10) 全过 + bge-m3 真水位 OVERALL/crosslang/content-not-name 任一桶相比 v4 错配版本明显提升 | 合并、baseline 报告 v4-fixup 节落、follow-up cycle BETA-15B-7-v2 启动 bake 决策（独立 cycle） |
| **GO（无破局）** | (1)-(10) 全过 + bge-m3 真水位 vs qwen3-0.6b 全桶持平或弱于 0.6b（即修了 pooling 也救不动 bge-m3 在本任务上） | 合并、baseline 报告诚实记录「pooling 修复后 bge-m3 仍不破 0.6b、移交下 cycle 抓手 = 更大跨族 / 评测扩量 / 微调」、infra 层 GO 仍成立（下次评测任何 BERT-arch 模型不再被错配阻断） |
| **异常** | qwen3-0.6b 重 embed 后 `vectors.json` 不 byte-equal | **不合并**：infra 改动有未预期副作用、调查根因（detect 路径走对了吗？参数透传错？worker 线程闭包捕获错？）|

**注**：本 cycle 不强求 bge-m3 破 v3 0.6b 才合并。infra 修复本身有独立价值（解阻塞）；模型层结论是数据副产物。

## 3. 范围

### 3.1 In-scope

- 新增 `packages/model-runtime/src/pooling.rs` 模块（~80-120 行含单测）
- 改 `packages/model-runtime/src/llama.rs`：① 删 line 357 硬编码 `.with_pooling_type(LlamaPoolingType::Last)`；② `worker_main` 函数内 model 加载后 detect pooling 一次、存为函数局部变量（不入 `LlamaModelImpl` struct——struct 持 `Mutex<Sender>` 跨线程共享、`LlamaPoolingType` 留在 worker 线程内更直接；detect 在 `ready_tx.send(Ok)` 之前、失败时 `ready_tx.send(Err)` + return、`spawn` 返回 `Err`；详 §4.1）；③ `detect_model_pooling(&LlamaModel)` thin adapter；④ `run_embed` 签名扩 `pooling: LlamaPoolingType` 参数
- `packages/model-runtime/src/lib.rs`：`mod pooling;`（不 pub、`pub(crate)` 即可、单测访问无碍）
- 集成手验脚本（不入 cargo test）：① qwen3-0.6b 重 embed + git diff vectors.json 空；② bge-m3 重 embed + 9 阈值 sweep
- baseline 报告 [docs/reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md) 追加 v4-fixup 节
- v4 spec MEAN→CLS doc fixup
- 新 `vectors-bge-m3.json` 入仓（覆盖 v4 错配版本）

### 3.2 Out-of-scope（主动 YAGNI）

- ❌ **不动 `DEFAULT_EMBEDDING_MODEL_PATH`**：生产侧仍走 qwen3-0.6b、bge-m3 bake 推到生产是独立 cycle
- ❌ **不动 `baseline.json`**：生产仍走 qwen3-0.6b、v3 锚不破、gate 仍守 0.6b
- ❌ **不动 llama-cpp-4 0.3.0 版本**：qwen3-8b 全零 bug 与 pooling 无关、独立 cycle BETA-15B-Y 解
- ❌ **不重跑 qwen3-8b**：pooling_type=3=Last 与现配置已对齐、修 pooling 不改 8b 结果
- ❌ **不抽 `trait PoolingMetadataSource`**：LlamaModel 是闭包外部依赖、trait 抽象重构面大、收益微（detect 函数已纯逻辑可测）
- ❌ **不动 candle / candle-loader**：only llama 路径有 embedding context、candle 路径不走 `LlamaPoolingType`
- ❌ **不动 stub.rs**：stub 是 feature 关时的占位、无 pooling 概念
- ❌ **不暴露 pooling 选择给 desktop UI**：用户不需要看 / 选 pooling type、是模型内禀属性
- ❌ **不加 logging 输出当前 pooling type**：用 `dbg!` / `tracing::debug!` 都不必、出错时 ModelError 已含足够信息

## 4. 设计

### 4.1 模块结构 / 函数签名 / 注入点

**新增 `packages/model-runtime/src/pooling.rs`**：

```rust
//! GGUF metadata → LlamaPoolingType 检测（纯逻辑、可单测）。
//!
//! 与 llama.cpp upstream `LLAMA_POOLING_TYPE_*` 枚举对齐：
//! 0=None, 1=Mean, 2=Cls, 3=Last（注：upstream 还有 4=Rank、留 reranker 用、
//! llama-cpp-4 0.3.0 binding 未暴露、本 cycle 不映射、未来接 reranker 升 binding 后扩）。
//! GGUF 标准 metadata key 形式为 `<arch>.pooling_type`（i32/u32），arch 取自 `general.architecture`。

#[cfg(feature = "llama-cpp")]
use llama_cpp_4::context::params::LlamaPoolingType;
use crate::ModelError;

#[cfg(feature = "llama-cpp")]
pub(crate) fn map_gguf_pooling_value(v: i64) -> Result<LlamaPoolingType, ModelError> {
    match v {
        0 => Ok(LlamaPoolingType::None),
        1 => Ok(LlamaPoolingType::Mean),
        2 => Ok(LlamaPoolingType::Cls),
        3 => Ok(LlamaPoolingType::Last),
        _ => Err(ModelError::LoadError(format!(
            "invalid GGUF pooling_type value: {v} (expected 0..=3; \
             Rank=4 reserved for rerankers, not exposed by llama-cpp-4 0.3.0 bindings)"
        ))),
    }
}

#[cfg(feature = "llama-cpp")]
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

#[cfg(feature = "llama-cpp")]
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

**改 `packages/model-runtime/src/llama.rs`**：

新增 thin adapter（在 llama.rs 内、不抽到 pooling.rs，避免 pooling.rs 触 LlamaModel 类型）：

```rust
#[cfg(feature = "llama-cpp")]
fn detect_model_pooling(model: &LlamaModel) -> Result<LlamaPoolingType, ModelError> {
    // llama-cpp-4 0.3.0 `meta_val_str` 需 `buf_size`；`general.architecture` 是短字符串
    // （如 `qwen3`/`bert`/`jina-bert-v2`）、`pooling_type` 是 "0".."3"，256 字节足够。
    const META_BUF: usize = 256;
    let arch = model
        .meta_val_str("general.architecture", META_BUF)
        .map_err(|e| ModelError::LoadError(
            format!("missing GGUF metadata `general.architecture`: {e}")
        ))?;
    let key = format!("{arch}.pooling_type");
    let pooling_meta = match model.meta_val_str(&key, META_BUF) {
        Ok(s) => Some(s.parse::<i64>().map_err(|e| ModelError::LoadError(
            format!("invalid GGUF metadata `{key}` = `{s}`: {e}")
        ))?),
        Err(_) => None,
    };
    pooling::detect_pooling_type(&arch, pooling_meta)
}
```

**`run_embed` 签名修改**：

```rust
// before:
fn run_embed(backend: &LlamaBackend, model: &LlamaModel, context_size: u32, text: &str)
    -> Result<Vec<f32>, ModelError>

// after:
fn run_embed(backend: &LlamaBackend, model: &LlamaModel, context_size: u32,
             pooling: LlamaPoolingType, text: &str)
    -> Result<Vec<f32>, ModelError>
```

函数体内 `.with_pooling_type(LlamaPoolingType::Last)` → `.with_pooling_type(pooling)`、其他不变。

**`worker_main` 函数流程改动**：

`worker_main`（顶层函数、由 `LlamaModelImpl::spawn` 的 trampoline 闭包调起）：
- 现行：`load_from_file` → `ready_tx.send(Ok(()))` → `while let Ok(req) = req_rx.recv() { match req { ... Embed → run_embed(backend, &model, ctx_size, &text) ... } }`
- 修改：`load_from_file` → **`detect_model_pooling(&model)`**（失败：`ready_tx.send(Err)` + return；成功：进入下一步）→ `ready_tx.send(Ok(()))` → loop { ... → `run_embed(backend, &model, ctx_size, pooling, &text)` ... }

`detect` 在 `ready_tx.send(Ok)` 之前 = 失败让 `LlamaModelImpl::spawn` 直接返回 `Err(ModelError)`，与现 `load_from_file` 失败同款行为路径、不影响 `LlamaModelRuntime` trait。

### 4.2 数据流

```
LlamaLoader::load(path, params)
  → LlamaModelImpl::spawn(backend, path, gpu_layers, ctx_size)
      ├ create (req_tx, req_rx) + (ready_tx, ready_rx)
      ├ thread::spawn(move || worker_main(&backend, &path, gpu_layers, ctx_size, &ready_tx, &req_rx))
      │
      │   worker_main:
      │   ├ LlamaModel::load_from_file(path)           ← 现行
      │   │   失败 → ready_tx.send(Err) + return
      │   ├ detect_model_pooling(&model)                ← 新增：一次性
      │   │   ├ model.meta_val_str("general.architecture", META_BUF)
      │   │   ├ model.meta_val_str("<arch>.pooling_type", META_BUF)  (Option)
      │   │   └ pooling::detect_pooling_type(arch, meta)
      │   │   失败 → ready_tx.send(Err) + return
      │   ├ let pooling: LlamaPoolingType = ...;
      │   ├ ready_tx.send(Ok(()))                       ← 初始化完成回送
      │   └ while let Ok(req) = req_rx.recv() {
      │       match req {
      │         Request::Generate { ... }       → run_plain(...)
      │         Request::GenerateCached { ... } → run_cached(...)
      │         Request::Embed { text, reply }  → run_embed(backend, &model, ctx_size, pooling, &text)
      │                                                                                ^^^^^^^ 新参数
      │       }
      │     }
      └ match ready_rx.recv() {
          Ok(Ok(())) → LlamaModelImpl { req_tx, _handle }
          Ok(Err(e)) → Err(e)                          ← detect 失败走这里
          Err(_)     → Err(LoadError("worker thread exited before model load completed"))
        }
```

**关键不变量**（与 §2.2 验收 (6)(7)(9) 对应）：

- qwen3-0.6b：arch=`qwen3` + meta=`3` → `Last`（与旧硬编码一致）→ vectors byte-equal
- bge-m3：arch=`bert` + meta=`2` → `Cls`（修正 v4 错配）→ vectors 变化（必然，pooling 不同）
- qwen3-8b：arch=`qwen3` + meta=`3` → `Last`（与旧硬编码一致）→ 8b 全零仍是 8b 全零（与 pooling 无关）

### 4.3 Heuristic 表（`default_pooling_for_arch`）

| arch（lowercase, exact match） | 默认 `LlamaPoolingType` | 依据 |
|---|---|---|
| `bert` / `nomic-bert` / `jina-bert-v2` / `roberta` | `Cls` | BERT 系标准用 `[CLS]` token + L2 norm（bge-m3 / jina / nomic-embed-text 均如此）|
| `t5` | `Mean` | T5 encoder embedding 标准是 mean pooling |
| `llama` / `qwen2` / `qwen3` / `mistral` | `Last` | decoder-only 序列模型用 last-token state（Qwen3-Embedding / E5-Mistral / instructor 等）|
| 其他 | **fail-fast** `ModelError::LoadError(...)` | YAGNI、未知 arch 强制用户在 GGUF 声明 pooling_type 或扩展 heuristic 表 |

**注**：只有 GGUF 未声明 `<arch>.pooling_type` 时才走此表。实测三模型都声明 = 此表只对未来 user-supplied 模型生效。表本身可演进、单测覆盖回归保护。

### 4.4 错误处理 + 文档校正

**错误**：复用现 `ModelError::LoadError`（不新增变体、YAGNI）。错误信息包含：

- metadata key 名（`general.architecture` / `<arch>.pooling_type`）
- 看到的值（原始字符串、parse 错时给原文）
- 操作建议（"declare pooling_type in GGUF or extend heuristic table"）

**关联 doc fixup**（本 cycle 收尾顺手）：

- [`docs/reviews/semantic-recall-quality-baseline.md`](../../reviews/semantic-recall-quality-baseline.md) line 668：`bert.pooling_type = 2`（MEAN）→（CLS）
- 同文 line 669：补一行「实测 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3}`（注：llama-cpp-4 0.3.0 binding 未暴露 `Rank=4`、留 reranker 用）；bge-m3 GGUF 声明 CLS 与其 model card `[CLS]` + L2 norm 一致」
- 同文 line 670-671 表述微调：「现硬编码 `Last` 覆盖 `CLS` 声明」（保持诊断方向正确）
- v4 spec [`2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md`](2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md) MEAN→CLS 一处校正

## 5. 测试策略

### 5.1 单测（`packages/model-runtime/src/pooling.rs` 内 `#[cfg(test)]` 模块、cargo test 全跑、不挂 GGUF）

```rust
#[cfg(test)]
#[cfg(feature = "llama-cpp")]
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
        assert_eq!(default_pooling_for_arch("bert").unwrap(), LlamaPoolingType::Cls);
        assert_eq!(default_pooling_for_arch("nomic-bert").unwrap(), LlamaPoolingType::Cls);
        assert_eq!(default_pooling_for_arch("jina-bert-v2").unwrap(), LlamaPoolingType::Cls);
        assert_eq!(default_pooling_for_arch("roberta").unwrap(), LlamaPoolingType::Cls);
    }

    #[test]
    fn default_pooling_for_arch_decoder_family_is_last() {
        assert_eq!(default_pooling_for_arch("llama").unwrap(), LlamaPoolingType::Last);
        assert_eq!(default_pooling_for_arch("qwen3").unwrap(), LlamaPoolingType::Last);
        assert_eq!(default_pooling_for_arch("mistral").unwrap(), LlamaPoolingType::Last);
    }

    #[test]
    fn default_pooling_for_arch_t5_is_mean() {
        assert_eq!(default_pooling_for_arch("t5").unwrap(), LlamaPoolingType::Mean);
    }

    #[test]
    fn default_pooling_for_arch_unknown_fails() {
        assert!(default_pooling_for_arch("frobnicator").is_err());
        assert!(default_pooling_for_arch("").is_err());
    }

    #[test]
    fn detect_pooling_type_metadata_overrides_heuristic() {
        // qwen3 + meta=3 → Last (现行 qwen3-0.6b/8b 行为锚)
        assert_eq!(detect_pooling_type("qwen3", Some(3)).unwrap(), LlamaPoolingType::Last);
        // bert + meta=2 → Cls (修正 v4 错配的回归保护)
        assert_eq!(detect_pooling_type("bert", Some(2)).unwrap(), LlamaPoolingType::Cls);
    }

    #[test]
    fn detect_pooling_type_missing_metadata_uses_heuristic() {
        assert_eq!(detect_pooling_type("bert", None).unwrap(), LlamaPoolingType::Cls);
        assert_eq!(detect_pooling_type("qwen3", None).unwrap(), LlamaPoolingType::Last);
        assert!(detect_pooling_type("frobnicator", None).is_err());
    }

    #[test]
    fn detect_pooling_type_invalid_metadata_fails_even_with_known_arch() {
        // arch 已知但 metadata 越界 = 仍 fail（不静默 fallback 到 heuristic）
        assert!(detect_pooling_type("qwen3", Some(99)).is_err());
    }
}
```

### 5.2 集成手验（不入 cargo test、收尾时按本节脚本跑、产物写入 baseline 报告 v4-fixup 节）

**Step 1：qwen3-0.6b byte-equal 红线**

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality --release -- \
  --embed \
  --model models/qwen3-embedding-0.6b-q8_0.gguf \
  --vectors-file packages/evals/fixtures/semantic-recall/vectors.json
git diff --exit-code packages/evals/fixtures/semantic-recall/vectors.json
# 期望：exit 0、git diff 空
```

**Step 2：bge-m3 真水位（infra 修复后）**

```bash
cargo run -p locifind-evals --bin semantic_quality --release -- \
  --embed \
  --model models/bge-m3-q8_0.gguf \
  --vectors-file packages/evals/fixtures/semantic-recall/vectors-bge-m3.json
# 期望：vectors-bge-m3.json 与 v4 错配版本必然不同（pooling Last→Cls）

# 9 阈值 sweep
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-bge-m3.json \
    --semantic-weight 10.0 --cosine-threshold $t
done
```

**Step 3：写 baseline 报告 v4-fixup 节**

附 bge-m3 真水位 sweep 表（9 阈值 × 6 桶）+ 与 v4 错配版本对照差 + MEAN/CLS doc 校正备注 + GO/GO（无破局）二级判定。

**Step 4：v4 spec MEAN→CLS doc fixup**

`docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md` 一处校正（spec §1.3 修订说明或对应实测节）。

## 6. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| qwen3-0.6b 重 embed 后 vectors.json 不 byte-equal | 低 | 高（验收 (6) 红线、阻止合并）| metadata 实测一致 + 单测覆盖 `("qwen3", Some(3))→Last` 锚；若不一致 = 调查 detect 路径 / 闭包参数透传 / worker 状态 |
| `LlamaModel::meta_val_str` 在某 arch 上抛非预期错误 | 低 | 中 | thin adapter `detect_model_pooling` map_err 时含 key 名、便于诊断；现 v4 cycle 已用三模型验证 metadata 读取 API 正常 |
| heuristic 表漏覆盖某常见 embedding arch | 中 | 低 | 本 cycle YAGNI 只列 bert/t5/llama/qwen 系；未知 arch fail-fast 错误信息明示「扩展 heuristic 表」、下次需要时扩 |
| bge-m3 修复 pooling 后真水位仍弱于 qwen3-0.6b | 中 | 低 | 不阻塞合并（§2.3 GO（无破局）路径）；baseline 报告诚实记录、移交下 cycle 抓手 |
| `cfg(feature = "llama-cpp")` 门控让 pooling.rs 在 feature 关时编译失败 | 低 | 中 | 整个 pooling.rs 模块 `#[cfg(feature = "llama-cpp")]`、与 llama.rs 同款门控；`lib.rs` 内 `#[cfg(feature = "llama-cpp")] mod pooling;` |
| spec / plan 起草引错 `llama-cpp-2 0.1.146` 旧版 API 文档（`meta_val_str(key)` 1 参 + 暴露 `Rank=4`），与 `llama-cpp-4 0.3.0` 实际 binding（`meta_val_str(key, buf_size)` 2 参 + 未暴露 Rank）不符 | 已发生 | 低 | 实施期 implementer + coordinator 协作发现并修正（pooling.rs 4=Err、llama.rs `META_BUF=256`）；spec / plan 同步校正、起草期工程现实风险登记备查 |

## 7. 真机手测剧本

本 cycle 是 model-runtime infra 修复 + 评测端到端覆盖、**桌面用户行为零变化**（生产仍走 qwen3-0.6b、`DEFAULT_EMBEDDING_MODEL_PATH` 不动、baseline.json 不动）。按 superpowers `verification-before-completion` 判断：

- 单测 + qwen3-0.6b byte-equal + bge-m3 端到端 sweep 已构成 infra 修复的完整证据链
- 桌面 UI / 索引 / 搜索行为路径未触达
- **不安排额外桌面真机手测剧本**

例外：若用户希望验证「桌面侧加载 qwen3-0.6b 行为字节等价」、可走一次 BETA-15B-2 暖机剧本（[docs/manual-test-scenarios.md](../../manual-test-scenarios.md) 对应节）、对比首查询时延 + semantic 命中行为。

## 8. 节奏与回看

参照 BETA-15B-7 v4 cycle 节奏：

| 阶段 | 内容 | 工具 / 工序 |
|---|---|---|
| brainstorming | Q1-Q4 + design sections 4 节 + user ack | 本 cycle 已完成 |
| writing-plans | task 分解（预估 5-7 task）+ TDD 友好排序 | superpowers writing-plans |
| subagent-driven | 每 task implementer + 双 reviewer（spec / code-quality）| superpowers subagent-driven-development |
| final integration review | general-purpose subagent 通读 main..HEAD diff + 验收红线核对 | 集成审 |
| 集成手验 | §5.2 四步脚本 | 手动 |
| PR + 收口 | 合 main + STATUS / ROADMAP doc-sync | 本仓 |

**预估 cycle 时长**：1.5-2d。

**与历史 cycle 对照**：本 cycle 是「BETA-15B-3 A-5 + BETA-15B-6 v2/v3 + BETA-15B-7」同款节奏的 model-runtime infra 修复版本——纯逻辑 TDD + 端到端自验 + 不动生产 + 不 bake。

## 9. 链接

- 上 cycle 落点：[BETA-15B-7 v4 spec](2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md) + [v4 plan](../plans/2026-06-24-beta-15b-7-embedding-model-scaling-probe.md)
- 上 cycle 诊断证据：[baseline 报告 v4 节](../../reviews/semantic-recall-quality-baseline.md#v4-数据集节--embedding-模型跨族--同族最大档探针beta-15b-7)
- 现 infra 缺陷位置：[packages/model-runtime/src/llama.rs:357](../../../packages/model-runtime/src/llama.rs#L357)
- llama-cpp-2 API 参考：`~/.cargo/registry/src/index.crates.io-*/llama-cpp-2-0.1.146/src/{context/params.rs,model.rs}`
- ROADMAP 锚：[ROADMAP §3.3 BETA-15B 系列](../../../ROADMAP.md) 下子 ID（本 cycle = BETA-15B-8 待 STATUS / ROADMAP doc-sync 落定）
