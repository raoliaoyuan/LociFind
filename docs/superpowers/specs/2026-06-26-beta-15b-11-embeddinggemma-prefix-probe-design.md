# BETA-15B-11 设计：EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验

> **类型**：评测探针 cycle（无桌面 wiring 改动、无 baseline.json 改动、不动 result-normalizer / 不动 parser、不动模型分发 UX）
> **承接**：BETA-15B-10 v5 baseline（bge-m3 / OVERALL 0.864 / crosslang 0.686 / [PR #17](https://github.com/raoliaoyuan/LociFind/pull/17) 合 main、merge commit `8e707cf`）+ BETA-15B-7 探针 cycle 节奏（双轴 / 四 Branch 决策 / GO 候选则 follow-up cycle bake）
> **目标**：验证 **EmbeddingGemma-300M**（gemma-embedding arch / 768 dim / Mean pooling / 100+ lang / Google 2025-09）能否冲过 spec 字面 **OVERALL ≥ 0.864 + crosslang ≥ 0.700**；新增子命题 = **prefix 契约**（HF model card 强制 query/document 双 prompt 包装）对最终分数的真实影响 = 裸 embed vs 标准 prefix 双轴对照
> **范围**：单 cycle、5 commit、~2-3d（含 binding 升级应急 budget）
> **不涉及**：桌面 wiring、模型分发 UX、其他候选模型（jina-v3 / bge-multilingual-gemma2）、result-normalizer 重 sweep cosine_threshold、parser / harness / search-backends

## §1 背景与动机

### §1.1 v5 baseline 留下的 crosslang 字面天花板

BETA-15B-10 把桌面默认 + 评测层全部重锚到 **bge-m3** 后，v5 真水位（dataset 81 cases / 127 docs / dim 1024 / W=10.0 / T=0.70）：

| 桶 | v5 HYBR | spec 字面目标 | 差距 |
|---|---|---|---|
| OVERALL | **0.864** | ≥ 0.864 | ✓ 等过 |
| crosslang | **0.686** | ≥ 0.700 | ✗ -0.014 |
| content-not-name | 0.869 | — | — |
| exact-name | 1.000 | = 1.000 | ✓ |

**bge-m3 sweep best（v4-fixup 表）**：T=0.45 时 OVERALL=0.869 / crosslang=0.708。即便选 sweep best 而非 BETA-15B-10 保守的 T=0.70，crosslang 也只到 0.708 / 微过字面 0.700。

**v5 cycle 主动放弃字面 crosslang 0.700**、明示「移交未来 cycle = 更大 / 跨厂 embedding 模型」（见 [baseline 报告 v5 节诚实边界](../reviews/semantic-recall-quality-baseline.md#v5-数据集节--beta-15b-10-bge-m3-baseline-重锚--cosine-sweep--bake--评测集长文本扩量--evals-embed-截断解除-done)）。

**本 cycle 是这个移交的兑现**：用更强的跨厂候选冲击字面 spec 目标。

### §1.2 候选模型选型：EmbeddingGemma-300M 单点 + 双 prefix 对照

参考 STATUS [下一步] §1 列的三候选 = EmbeddingGemma-300M / jina-embeddings-v3 / bge-multilingual-gemma2-9B。本 cycle brainstorming 阶段做的跨厂调研报告（外部 WebSearch + HF model card + llama.cpp upstream issue 抽样）给出明确推荐排序：

| 候选 | 优势 | 风险 | 决策 |
|---|---|---|---|
| **EmbeddingGemma-300M** | 329 MB / 768 dim / 100+ lang / sub-500M MTEB-multilingual SOTA / 官方 GGUF `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF` 完备 | (a) llama-cpp-4 0.3.2 vendored llama.cpp 须 ≥ 2025-09（`gemma-embedding` arch 入主线时间）；(b) **强制 prompt prefix 契约**，不做会让模型看起来比 bge-m3 还差；(c) [pooling.rs](../../packages/model-runtime/src/pooling.rs) 默认 arch 白名单不含 `gemma-embedding` | **选 ⭐ 单点** |
| jina-embeddings-v3 | MIRACL nDCG@10 61.9 / 89 lang | (a) `jina-bert-v3` 不在白名单（v2 在）；(b) LoRA per-task adapter 是结构性阻塞；(c) GGUF 转换 issue #9585 仍开 | 不做（双阻 + 研究 spike 性质） |
| bge-multilingual-gemma2-9B | 跨厂 SOTA | (a) 9B 桌面不现实（Q4_K_M ~5.5 GB / dim 3584）；(b) **查不到任何 GGUF**；(c) gemma2 `--embeddings` 路径无公开成功案例 | 不做（三独立 blocker 任一致命） |

**结论**：EmbeddingGemma-300M 单点是本 cycle 唯一值得做的候选。同尺寸 / 比 bge-m3 还小一半的可执行性 + cross-lingual SOTA 评测分都满足条件。

### §1.3 本 cycle 的两个命题

> **命题 1（主）**：EmbeddingGemma-300M 在 LociFind 合成集上能否冲过 OVERALL ≥ 0.864 + crosslang ≥ 0.700 字面 spec 目标？
>
> **命题 2（子）**：HF model card 强制的 prefix 契约（query 包 `task: search result | query: ...` / doc 包 `title: none | text: ...`），对最终分数的真实影响有多大？没有 prefix 的 baseline 与 prefix 的对照实验，能否反推「prefix 契约」是模型设计的必要条件还是 nice-to-have？

命题 2 是与 BETA-15B-7 比新增的 cycle 价值——为未来 cycle 的候选模型（特别是带 prompt 契约的现代 LLM-style embedder）提供「prefix 必要性 vs 模型本身能力」的分离数据点。

### §1.4 关键 infra 风险（pooling 白名单 / binding / prefix）

[BETA-15B-9 教训](../reviews/semantic-recall-quality-baseline.md#v4-fixup2-数据集节--llama-cpp-4-升级--qwen3-embedding-8b-全零-bug-4-hypothesis-ladder-全-fail-beta-15b-9)：qwen3-embedding-8b 在 llama-cpp-4 0.3.2 binding 状态下推理 ~1 min 早期短路、vec 全零、4-hypothesis 全 FAIL。本 cycle 必须先 de-risk infra 才能下模型层结论。

三件 de-risk 必做：

1. **扩 [pooling.rs](../../packages/model-runtime/src/pooling.rs) arch 白名单**：当前 `default_pooling_for_arch` 不识别 `gemma-embedding`、加载会 panic 在 fallback。本 cycle 改 1 行 + 1 单测。
2. **验 llama-cpp-4 0.3.2 vendored llama.cpp 是否识别 `gemma-embedding` arch**：若不识别 → `unknown model architecture: 'gemma-embedding'` error（参考 llama-cpp-python issue #2065）。
3. **prefix 契约实现**：仅在 [`semantic_quality.rs`](../../packages/evals/src/bin/semantic_quality.rs) embed 闭包内 inline（评测 only / 不动 model-runtime API）。

应急条款（用户 brainstorming Q5 已拍）：**若 0.3.2 不识别 → 升 llama-cpp-4 binding + qwen3-0.6b 语义等价闸**（cos ≥ 0.9999 / max abs ≤ 1e-3，与 BETA-15B-9 同款）。若升级后仍不支持 → Branch IV-infra 收 infra 诊断、不当模型层结论。

## §2 接受标准与红线

### §2.1 验证门

| # | 红线 | 验证命令 | 目标 |
|---|---|---|---|
| 1 | rustfmt | `cargo fmt --all --check` | 净 |
| 2 | clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| 3 | workspace test | `cargo test --workspace` | 全过 |
| 4 | semantic_quality_gate | `cargo test -p locifind-evals --test semantic_quality_gate` | 1 passed（守 **v5 baseline** 不退步、本 cycle 不改 baseline.json / 不改 gate.rs assert）|
| 5 | desktop tsc | `npm run -w apps/desktop typecheck` + `npm run -w apps/desktop build` | 净 + vite 成功 |
| 6 | parser-only byte-equal | `cargo run -p locifind-evals --release -- v05` 与 main byte-equal、`v09` 同 | v0.5=473 + v0.9=877 case 数 + 0 diff（本 cycle 不动 parser）|
| 7 | fixture SHA256 | `sha256sum packages/evals/fixtures/*.json packages/parser-rs/fixtures/*.json` | parser-rs / v0.5 / v0.9 fixture 与 main 等价；semantic-recall 既有 corpus.json / cases.json / vectors.json / baseline.json **字节不动**（本 cycle 不改主 vectors / 不改 baseline）；**新增** vectors-embeddinggemma-300m-no-prefix.json + vectors-embeddinggemma-300m-prefix.json |
| 8 | vectors.json schema | `check_vectors(corpus, cases, vectors)` 自动跑 | 全 cases + corpus 都有 vector、dim=768 for embeddinggemma、L2 norm 抽样 ≈ 1.0 |
| 9 | 若 binding 升级 | qwen3-0.6b 重 embed 后与 main vectors.json 语义等价闸 | cos min ≥ 0.9999 / max abs ≤ 1e-3、SHA256 可变化但语义等价 |
| 10 | prefix 双 mode 区分性 | `diff vectors-embeddinggemma-300m-no-prefix.json vectors-embeddinggemma-300m-prefix.json` | NOT byte-equal（prefix 真包到 query/doc 的 sanity check）|

### §2.2 接受标准（spec 红线、与 gate.rs 4 红线一一对应）

- **(4a)** exact-name HYBR_R = 1.000（硬红线、不可破）
- **(4b)** 各桶（synonym / concept / crosslang / content-not-name / exact-name / OVERALL）HYBR_N ≥ **v5 baseline HYB**（不退步、gate 动态读 baseline.json 自锁）
- **(4c)** crosslang HYBR_N ≥ **v5 baseline HYBR**（自锁、gate 4 红线动态读、不退步即可）
- **(4d)** OVERALL HYBR_N ≥ **v5 baseline HYBR**（自锁）

**gate 不改 assert**：本 cycle 不动 [`semantic_quality_gate.rs`](../../packages/evals/tests/semantic_quality_gate.rs) 任何 assert / 不动 baseline.json / 不动 gate.rs doc 注释。新候选 vectors 文件 sweep 是**研究产物**、不构成产线行为变化。

### §2.3 GO 判定（cycle 决策表、独立于 gate.rs）

GO 判定仅作 cycle 决策、决定是否开 follow-up cycle BETA-15B-11-v2 bake 推生产。不动 gate.rs assert。

| Branch | 字面 spec 目标判定 | 复活字面 = OVERALL ≥ 0.864 + crosslang ≥ 0.700 | 行动 |
|---|---|---|---|
| **I-a ⭐ GO** | 标准 prefix mode 下 **双过字面** | 全过 | **首选**：开 follow-up cycle BETA-15B-11-v2 bake 推生产（DEFAULT_EMBEDDING_MODEL_FILENAME 替换 + baseline.json rewrite + 模型分发 UX 准备 + Windows 适配）|
| **I-b GO 但 prefix 必要** | 标准 prefix 双过 + 裸 mode 任一不过 | I-a 同 + 裸 mode FAIL | I-a 同 + spec 提示 follow-up cycle 必须实现 [model-runtime](../../packages/model-runtime) 层 prefix API（`embed_query` / `embed_doc`）|
| **II NO GO trade-off** | crosslang HYBR_N 反退 < v5 baseline 0.686 | 任一桶退步 | **不 bake**——任一新模型让 crosslang 退步则可疑；记录数据；移交下 cycle 抓手 = 评测扩量 + 其他跨族候选（jina-v3 spike）|
| **III 见顶** | 双不过字面 spec 目标 | 双不过、但各桶不退步 | 认知结论 = **EmbeddingGemma + bge-m3 都在 cosine 单维 + 当前合成集见顶**；移交下 cycle 抓手 = bge-multilingual-gemma2 转 GGUF / 评测扩量到 200+ case / 微调 |
| **IV 异常** | 任意 | 任一桶破 ≥ HYB baseline 或 infra 失败 | **不应发生**——更强模型反而退步说明 bug（vectors 文件错位 / 模型加载错 / dim 计算错 / 推理短路 / vec 全零 / pooling 错配 / prefix 双 mode vectors byte-equal）；调查不发布、留 STATUS 异常记录 + file upstream issue draft 入仓（与 BETA-15B-9 同款）|

**"GO 候选" 不等于 bake**：本 cycle 是探针、即便 I-a/I-b 命中也**不**改 `DEFAULT_EMBEDDING_MODEL_FILENAME` / 不动 desktop wiring / 不动 baseline.json。bake 推生产是 follow-up cycle BETA-15B-11-v2 的事。

### §2.4 控制对照不变性

- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等、cosine 阈值不触发跳 FTS）
- T=1.01 时 HYBR ≡ HYB（六桶完全相等、cosine 路由始终不生效）

## §3 范围（YAGNI）

### §3.1 做什么

| # | 文件 | 改动 | 检查 |
|---|---|---|---|
| 1 | [`packages/model-runtime/src/pooling.rs`](../../packages/model-runtime/src/pooling.rs) `default_pooling_for_arch` | 加 `"gemma-embedding" => Mean` 分支（默认）、添 1 单测 | clippy 0、9 → 10 单测、不破现有 arch 单测 |
| 2 | [`packages/evals/src/bin/semantic_quality.rs`](../../packages/evals/src/bin/semantic_quality.rs) | 加 `--prefix-mode {none,standard}` clap flag、default `none` 守向下兼容、embed 闭包按 mode inline 包 prefix、单测向下兼容 + prefix 解析单测 | clippy 0、unit test 全过、`--prefix-mode standard` query 包 `task: search result \| query: {text}`、doc 包 `title: none \| text: {text}`、`--prefix-mode none` 行为 = 现状 |
| 3 | `models/embeddinggemma-300m-q8_0.gguf` | 下载 `ggml-org/embeddinggemma-300m-qat-q8_0-GGUF` 仓库（公开 google 系列、需 license accept、~329 MB）、文件名规范化 `embeddinggemma-300m-q8_0.gguf`（与现 bge-m3-q8_0.gguf 同款）| GGUF magic 校验、metadata 抽 `general.architecture` = `gemma-embedding`、dim = 768 |
| 4 | `packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json` | Mac Metal `--embed --vectors-file <p> --prefix-mode none` 全集（cases 81 + corpus 127、dim 768）| SHA256 落库、L2 norm 抽样 ≈ 1.0、check_vectors 过、推理时长 > 30s（防短路）|
| 5 | `packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json` | 同上、`--prefix-mode standard` | 同上 + 与 #4 NOT byte-equal（红线 10）|
| 6 | binding 应急（仅若 §1.4 第 2 件失败）| `Cargo.toml` llama-cpp-4 从 0.3.2 升到最新发布版（参考 BETA-15B-9 Phase 0 同款流程）+ qwen3-0.6b 重 embed 后 vectors-qwen3-0.6b.json 语义等价闸 | cos min ≥ 0.9999 / max abs ≤ 1e-3、workspace test 全过 |
| 7 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 v6 节（dataset 81/127 / dim 768 / 2 prefix mode × 9 阈值 sweep 全表 / Branch 判定 / 诚实边界 / 与 v5 bge-m3 对照表）| 沿 v5 同款结构、不重写 v5 节 |
| 8 | `packages/evals/fixtures/semantic-recall/README.md` | 升 v6 版本说明（新 vectors-embeddinggemma-* 文件 + prefix mode 解释）| 沿 v5 同款风格 |

### §3.2 不做什么

- 不动 [`apps/desktop/src-tauri/src/search/embedding_model.rs`](../../apps/desktop/src-tauri/src/search/embedding_model.rs)（DEFAULT_EMBEDDING_MODEL_FILENAME 保 bge-m3）
- 不动 [`apps/desktop/src-tauri/src/settings.rs`](../../apps/desktop/src-tauri/src/settings.rs)
- 不动 [`packages/result-normalizer/src/lib.rs`](../../packages/result-normalizer/src/lib.rs)（DEFAULT_COSINE_ROUTING_THRESHOLD / DEFAULT_SEMANTIC_WEIGHT / DEFAULT_RRF_K / DEFAULT_SIMILARITY_FLOOR 全不动）
- 不动 `packages/evals/fixtures/semantic-recall/baseline.json`（gate 守 v5 不退步即可）
- 不动 [`packages/evals/tests/semantic_quality_gate.rs`](../../packages/evals/tests/semantic_quality_gate.rs)（assert 字节不动、注释也不升 v6）
- 不动 corpus.json / cases.json / 主 vectors.json（保 v5 状态）
- 不动 vectors-bge-m3.json / vectors-qwen3-0.6b.json 既有 reference snapshot（除非 §3.1 #6 binding 升级需要重 embed qwen3-0.6b 跑语义等价闸）
- 不动 parser / harness / search-backends / desktop UI
- 不动 model-runtime API 公共接口（不加 `embed_query` / `embed_doc`，留 GO 候选命中后的 follow-up cycle）
- 不引 jina-v3 / bge-multilingual-gemma2 / 其他候选（单点单 cycle）
- 不动模型分发 UX / 首启引导 / Windows 真机性能验证（独立 cycle）

## §4 数据与代码改动详细

### §4.1 [`pooling.rs`](../../packages/model-runtime/src/pooling.rs) 扩白名单

**改动点**：[`packages/model-runtime/src/pooling.rs:32-40`](../../packages/model-runtime/src/pooling.rs#L32) `default_pooling_for_arch`

```rust
// 改前
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

// 改后
pub(crate) fn default_pooling_for_arch(arch: &str) -> Result<LlamaPoolingType, ModelError> {
    match arch {
        "bert" | "nomic-bert" | "jina-bert-v2" | "roberta" => Ok(LlamaPoolingType::Cls),
        "t5" | "gemma-embedding" => Ok(LlamaPoolingType::Mean),
        "llama" | "qwen2" | "qwen3" | "mistral" => Ok(LlamaPoolingType::Last),
        _ => Err(ModelError::LoadError(format!(
            "unknown architecture '{arch}' and GGUF did not declare <arch>.pooling_type; \
             declare pooling_type in GGUF metadata or extend default_pooling_for_arch heuristic table"
        ))),
    }
}
```

**新增单测**：

```rust
#[test]
fn default_pooling_for_arch_gemma_embedding_is_mean() {
    assert_eq!(
        default_pooling_for_arch("gemma-embedding").unwrap(),
        LlamaPoolingType::Mean
    );
}
```

**注**：若 GGUF 实际声明 `gemma-embedding.pooling_type` metadata、`detect_pooling_type` 路径会取声明值、覆盖 `default_pooling_for_arch` 兜底。本 cycle T0 GGUF metadata 抽时验证实际声明值、记录到 spec 末尾「执行日志」节。

### §4.2 [`semantic_quality.rs`](../../packages/evals/src/bin/semantic_quality.rs) `--prefix-mode` flag

**改动点**：clap struct + embed 闭包

```rust
// clap 字段（新增）
#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum PrefixMode {
    #[default]
    None,
    Standard,
}

#[arg(long, value_enum, default_value_t = PrefixMode::None)]
prefix_mode: PrefixMode,

// embed 闭包（改 body、签名不动）
let embed = |text: &str, role: EmbedRole| -> Vec<f32> {
    let wrapped = match (args.prefix_mode, role) {
        (PrefixMode::None, _) => text.to_string(),
        (PrefixMode::Standard, EmbedRole::Query) => format!("task: search result | query: {text}"),
        (PrefixMode::Standard, EmbedRole::Doc) => format!("title: none | text: {text}"),
    };
    rt.embed(&wrapped).expect("embed 失败")
};
```

**EmbedRole**：新加内部 enum 区分 query vs doc（评测 binary 内部用、不外露）。调用方区分：`--embed` 子命令的 corpus 遍历循环传 `EmbedRole::Doc`、cases query 字段遍历循环传 `EmbedRole::Query`。`--embed` 路径之外的调用方（如 sweep 路径读 vectors.json 不重 embed）不受影响。

**dim 检查**：本 cycle 引入第二种 embedding dim（768 vs bge-m3 v5 状态 1024）。Plan 阶段验证 [`check_vectors`](../../packages/evals/src/) helper 行为 = 抽 vectors.json 内 dim 字段动态确认、不硬编码 dim。若发现硬编码（如 BETA-15B-7 期遗留），plan 阶段加 task 修。

**单测**（新增 2）：
- `prefix_mode_defaults_to_none`：默认值 = `None`
- `prefix_mode_standard_parses`：`--prefix-mode standard` 解析 = `Standard`

**向下兼容**：默认 `none` 时 wrapped = text、行为与 BETA-15B-10 完全一致。

### §4.3 GGUF 下载与 metadata 验证

**下载**：

```bash
# 1. HuggingFace login（已有 token、~/.huggingface/token）
huggingface-cli login

# 2. 接受 google 仓库 license（在 HF 网页确认 / 或 ggml-org 转仓不需）
# 优先尝试 ggml-org 转仓（公开免登录）
huggingface-cli download ggml-org/embeddinggemma-300m-qat-q8_0-GGUF \
    --include "*Q8_0.gguf" \
    --local-dir /tmp/embeddinggemma-download

# 3. 文件名规范化（与现 bge-m3-q8_0.gguf 同款命名）
mv /tmp/embeddinggemma-download/embeddinggemma-300m-qat-Q8_0.gguf \
   models/embeddinggemma-300m-q8_0.gguf

# 4. SHA256 + 大小验证
sha256sum models/embeddinggemma-300m-q8_0.gguf
ls -la models/embeddinggemma-300m-q8_0.gguf  # 期望 ~329 MB
```

**metadata 抽**：

```bash
# 用 llama-cpp-4 自带 gguf-info 或 Python gguf-parser
python -c "from gguf import GGUFReader; r = GGUFReader('models/embeddinggemma-300m-q8_0.gguf'); print({f.name: f.parts for f in r.fields if 'pool' in f.name or 'arch' in f.name or 'embedding_length' in f.name})"
```

**期望抽出**：
- `general.architecture` = `gemma-embedding`
- `gemma-embedding.embedding_length` = `768`
- `gemma-embedding.pooling_type` = `1`（Mean）（如声明、否则 fallback `default_pooling_for_arch`）

若抽出 arch ≠ `gemma-embedding` 或 dim ≠ 768 → STOP、检 GGUF 是否被替换。

### §4.4 binding 升级应急条款（§1.4 第 2 件套触发时）

**触发条件**：T1 step 2 实测 `cargo run -p locifind-evals -- semantic-quality --model models/embeddinggemma-300m-q8_0.gguf --embed --vectors-file /tmp/test.json` 返 `unknown model architecture: 'gemma-embedding'` error。

**应急流程**（与 BETA-15B-9 Phase 0 同款）：

1. 查 llama-cpp-4 最新发布版（参考 https://crates.io/crates/llama-cpp-4）
2. 升 `Cargo.toml` workspace 中 `llama-cpp-4 = "0.3.2"` → 最新（如 `0.3.3` / `0.4.x`）
3. `cargo build --workspace` 验编译过
4. `cargo run -p locifind-evals -- semantic-quality --model models/qwen3-embedding-0.6b-q8_0.gguf --embed --vectors-file /tmp/qwen3-postupgrade.json --prefix-mode none`（重 embed qwen3-0.6b）
5. 与 main 仓 `packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json` 跑 **语义等价闸**：
   - cos min(per-vector cosine) ≥ 0.9999
   - max abs(per-element) ≤ 1e-3
   - SHA256 可不一致（SemVer-compatible 数值微调不可避免、Metal kernel 浮点累加顺序）
6. 通过 → 继续 T2 embed embeddinggemma
7. 不通过 → 回滚 binding 升级、走 Branch IV-infra 同款路径：file upstream issue draft 入仓、收 infra 诊断、不当模型层结论

**与 BETA-15B-9 差异**：BETA-15B-9 升 0.3.0 → 0.3.2 时主 vectors.json 还是 qwen3-0.6b 内容（commit `38d255d` 一并入仓、SHA256 `0315b8d0...`）。本 cycle 主 vectors.json 已在 BETA-15B-10 v5 cycle 切到 **bge-m3 内容**（SHA256 `4f0de346b581d58d…`）。若再升 binding、本 cycle 需重 embed **两个** vectors 文件跑语义等价闸：
1. `vectors-qwen3-0.6b.json`（reference snapshot、与 BETA-15B-9 同款）
2. `vectors.json`（**v5 主 vectors / bge-m3 内容**、因 gate 守的就是它、必须确认 binding 升级语义等价）

两文件 SHA256 都可能变化（SemVer-compatible 数值微调）、但语义等价闸（cos ≥ 0.9999 / max abs ≤ 1e-3）必过。重 embed 后 SHA256 同步落 `docs/reviews/semantic-recall-quality-baseline.md` v6 节备查。

**估时**：~0.5d（与 BETA-15B-9 Phase 0 同款）。

## §5 异常处理

### §5.1 GGUF arch 错位

**触发**：T0 metadata 抽出 `general.architecture` ≠ `gemma-embedding`

**处理**：STOP、检 HuggingFace 仓库（ggml-org 转仓 vs google 官方仓 vs 第三方误标）、找到正确 GGUF 重下、不进 T1。

### §5.2 binding 升级后仍不支持

**触发**：§4.4 应急升级后仍报 `unknown model architecture: 'gemma-embedding'`

**处理**：
- 回滚 llama-cpp-4 升级（保 0.3.2 状态）
- 走 Branch IV-infra 路径：收 infra 诊断、写 file upstream issue draft 入仓 `docs/reviews/2026-06-26-beta-15b-11-upstream-issue-body.md`（138 行模板沿 BETA-15B-9 同款：GGUF metadata 抽出 + Rust 复现代码 + 4 hypothesis 排查表 + llama.cpp upstream commit 查询）
- 本 cycle 仍合并发布、写 baseline 报告 v6-fail 节、明示「不当模型层结论」（与 BETA-15B-9 v4-fixup2 同款节奏）
- STATUS 记录、留用户后续手动 file issue + 监控 upstream

### §5.3 推理短路

**触发**：Mac Metal embed 全集（127 doc + 81 query = 208 次推理）耗时 < 10s（参考 BETA-15B-9 8B 短路 ~27s vs 估算 13-40 min）

**处理**：
- 抽 sweep 出的 vectors-embeddinggemma-*.json L2 norm 分布
- 若 mean ≈ 0 / 全零 → vec 全零 bug（BETA-15B-9 同款失败模式）
- Branch IV-infra、不当模型层结论
- file upstream issue draft 入仓
- 本 cycle 仍合并发布 baseline 报告 v6-fail 节

### §5.4 prefix 双 mode vectors byte-equal

**触发**：`diff vectors-embeddinggemma-300m-no-prefix.json vectors-embeddinggemma-300m-prefix.json` 输出空

**处理**：
- prefix 没真包到 query/doc
- 检 [semantic_quality.rs](../../packages/evals/src/bin/semantic_quality.rs) embed 闭包逻辑 + clap 解析
- 修 + 重 embed prefix 版
- 重新走红线 10

## §6 操作清单（执行顺序、与 plan task 编号对齐）

| Task | 描述 | 估时 | 输出 |
|---|---|---|---|
| T0 | GGUF 下载 + metadata 抽（§4.3）| ~0.5h | models/embeddinggemma-300m-q8_0.gguf + metadata 截图 |
| T1 | C1 infra de-risk：扩 pooling.rs 白名单 + 单测（§4.1）；本机实测 binding 是否识别 `gemma-embedding`；若不识别 → §4.4 应急升级 | ~0.5-1d | C1 commit / 若需升级则 commit 含 Cargo.toml + vectors-qwen3-0.6b.json + 主 vectors.json 重 embed |
| T2 | C2 加 `--prefix-mode` flag + 单测 + 文档（§4.2）| ~0.5d | C2 commit |
| T3 | Mac Metal `--embed --prefix-mode none` 全集 → vectors-embeddinggemma-300m-no-prefix.json；同款 `--prefix-mode standard` → vectors-embeddinggemma-300m-prefix.json；两文件 SHA256 落库 + check_vectors 验过 + 红线 10 区分性验 | ~30 min | 2 新 vectors 文件入仓 |
| T4 | 9 阈值 sweep × 2 prefix mode = 18 次（用 `--vectors-file` flag 切换）；产 6 桶 × 9 阈值 × 2 mode 决策矩阵 | ~1h | /tmp/sweep-matrix.md |
| T5 | 人工读决策矩阵 + 按 §2.3 四 Branch 表判定；coordinator 拍 Branch I-a / I-b / II / III / IV | ~30 min | /tmp/branch-decision.md |
| T6 | C3 commit（baseline 报告 v6 节 + README.md v6 段、§3.1 #7 + #8）| ~1h | C3 commit |
| T7 | 总验收红线 1-10 全过；workspace test / clippy / fmt / desktop tsc / parser byte-equal / fixture SHA256 | ~30 min | 验证日志 / tmp/beta-15b-11-verification-evidence.txt |
| T8 | C4 commit（STATUS + ROADMAP doc-sync）| ~30 min | C4 commit |
| T9 | C5 PR + 合 main（gh CLI 401 时走 BETA-15B-9 同款本地 merge fallback）| ~15 min | PR # + merge commit hash 回填 STATUS |

**Cycle 总估时**：~2-3d（含 binding 升级应急 0.5d budget）。

## §7 真机手测策略

**纯评测探针 + 不动桌面 wiring + 不动 baseline + 不动 cosine_threshold**、按 spec §7 判**平凡**未安排手测剧本（与 BETA-15B-7 / BETA-15B-8 / BETA-15B-9 同款）。

GO 候选命中后开 follow-up cycle BETA-15B-11-v2 bake 推生产时再安排 Mac + Windows 真机手测（与 BETA-15B-7-v2 / BETA-15B-10 同款 [`docs/manual-test-scenarios.md`](../../docs/manual-test-scenarios.md) 三步走）。

## §8 链接

- [BETA-15B-10 spec](./2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md)（v5 baseline 重锚 + cosine bake + 长文本扩量）
- [BETA-15B-7 spec](./2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md)（双轴探针节奏模板）
- [BETA-15B-9 spec](./2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md)（binding 升级 + 语义等价闸 + Branch IV-infra 节奏）
- [BETA-15B-8 spec](./2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md)（pooling type detection 修复）
- [baseline 报告 v5 节](../reviews/semantic-recall-quality-baseline.md#v5-数据集节--beta-15b-10-bge-m3-baseline-重锚--cosine-sweep--bake--评测集长文本扩量--evals-embed-截断解除-done)（承接基准）
- [pooling.rs 模块](../../packages/model-runtime/src/pooling.rs)
- [EmbeddingGemma HF 卡（google）](https://huggingface.co/google/embeddinggemma-300m)
- [EmbeddingGemma GGUF（ggml-org）](https://huggingface.co/ggml-org/embeddinggemma-300m-qat-q8_0-GGUF)
