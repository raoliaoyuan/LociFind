# BETA-15B-10 设计：bge-m3 baseline 重锚 + cosine_threshold sweep & bake + 评测集长文本扩量 + evals embed 截断解除

> **类型**：评测层 + 融合参数 bake cycle（无桌面 wiring 改动、无 infra 改动、不动 result-normalizer 路由 API）
> **承接**：BETA-15B-7-v2（bge-m3 推到桌面默认）+ BETA-15B-6 v3（评测集扩量节奏）+ BETA-15B-7-v2 hotfix（BERT encode n_ubatch panic）
> **目标**：把评测层（baseline.json + cosine_threshold）从守 qwen3-0.6b 彻底重锚到 bge-m3 真水位 + 评测集合成集首次覆盖 > 512 token 长文本 case + 解除 evals embed 1200 char 截断（让 evals embed 路径与 desktop indexer 真实路径对齐）
> **范围**：单 cycle、3 commit、~2d
> **不涉及**：desktop wiring（已切 bge-m3）、infra（llama-cpp-4 / pooling / model-runtime）、长文本 > 2048 token 处理、cosine 路由架构变更

## §1 背景与动机

### §1.1 BETA-15B-7-v2 后续 + hotfix 暴露的两个独立 gap

**BETA-15B-7-v2 已落地** [`apps/desktop/src-tauri/src/search/embedding_model.rs`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs) 把桌面默认 embedding 从 qwen3-Embedding-0.6B-Q8_0.gguf 切换到 bge-m3-Q8_0.gguf（[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15) merged、merge commit `ee78f75`、保 cosine_threshold=0.70 不动、baseline.json 仍守 qwen3-0.6b）。**Hotfix PR #16 已合**（merge commit `32667ac`）修复 BERT encode 路径 `GGML_ASSERT(cparams.n_ubatch >= n_tokens)` panic。

BETA-15B-7-v2 collected 三条 follow-up：① cosine_threshold 在 bge-m3 上重 sweep & bake；② evals baseline.json + gate.rs 红线重锚到 bge-m3；③ evals 合成集扩 > 512 token 长文本 case。本 cycle 把三件合并执行。

### §1.2 cycle 起手发现的 evals embed 截断（spec 修订）

Spec 起草过程中实测 [`packages/evals/src/bin/semantic_quality.rs:165`](../../../packages/evals/src/bin/semantic_quality.rs#L165) 发现 `text.chars().take(1200)` 字符截断：

```rust
let embed = |text: &str| -> Vec<f32> {
    let t: String = text.chars().take(1200).collect();
    rt.embed(&t).expect("embed 失败")
};
```

**影响**：
- evals binary embed 所有文本被截到 1200 char、永远 < 2048 token、永远不会触发 BERT encode n_ubatch panic（被截断保护）
- 真机 desktop indexer 不走 evals binary、走 `apps/desktop/src-tauri/src/search/` 的 worker 路径、**不截断**、所以才在 BETA-15B-7-v2 真机暴露 panic
- en 长文本（1200 char ≈ 300 token）即使加进 corpus、在 evals 也进不到 BERT encode > 512 path
- evals 跑出的 sweep / baseline 数据所基于的 vectors **不代表 desktop indexer 实际 embed 行为**（对 < 1200 char doc 完全等价、对 > 1200 char doc 行为分裂）

**framing 修订**：本 cycle ③ 的真实价值不是「防 panic」（evals 截断已经天然防 panic），是 **让 sweep 出的 cosine_threshold 在长文本场景有真实数据支持 + evals embed 路径与 desktop indexer 行为对齐**。

### §1.3 核心数据指证

来自 baseline 报告 v4-fixup 节（v3 数据集 78 cases / 124 docs / W=10.0 固定）：

| T (cosine_threshold) | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 / 0.30 / 0.45 ⭐（v4-fixup sweep best）| 1.000 | **0.869** | **0.685** | 0.875 |
| 0.70（BETA-15B-7-v2 bake、桌面现行）| 1.000 | 0.864 | 0.662 | 0.875 |

**bake 拿满 ROI 上界**：OVERALL +0.005、crosslang +0.023（v4-fixup 表面值、真值需基于扩量 + 解除截断后 corpus 重 sweep 确认）。

## §2 接受标准与红线

### §2.1 验证门

| # | 红线 | 验证命令 | 目标 |
|---|---|---|---|
| 1 | rustfmt | `cargo fmt --all --check` | 净 |
| 2 | clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| 3 | workspace test | `cargo test --workspace` | 全过 |
| 4 | semantic_quality_gate | `cargo test -p locifind-evals --test semantic_quality_gate` | 1 passed（4 红线动态读 baseline 全过）|
| 5 | desktop tsc | `npm run -w apps/desktop typecheck` + `npm run -w apps/desktop build` | 净 + vite 成功 |
| 6 | parser-only byte-equal | `cargo run -p locifind-evals --release -- v05` 与 main byte-equal、`v09` 同 | v0.5=473 + v0.9=877 case 数 + 0 diff |
| 7 | fixture SHA256 | `sha256sum packages/evals/fixtures/*.json packages/parser-rs/fixtures/*.json` | parser-rs/v0.5/v0.9 fixture 与 main 等价（semantic-recall fixture **预期变化**：corpus.json / cases.json / vectors.json / baseline.json 全改）|
| 8 | vectors.json schema | `check_vectors(corpus, cases, vectors)` 自动跑过 | 全 cases + corpus 都有 vector、dim=1024、L2 norm 抽样 ≈ 1.0 |
| 9 | 长文本 token 数 | 从 `--embed` log 抽 s00125/s00126/s00127 实际 token 数 | 全在 `[513, 2048]` 区间（解除 1200 char 截断后真值才有意义）|

### §2.2 接受标准（spec 红线、与 gate.rs 4 红线一一对应）

- **(4a)** exact-name HYBR_R = 1.000（硬红线、不可破）
- **(4b)** 各桶（synonym / concept / crosslang / content-not-name / exact-name / OVERALL）HYBR_N ≥ 新 baseline HYB（不退步）+ HYBR_R ≥ 新 baseline HYB recall（不退步）
- **(4c)** crosslang HYBR_N ≥ 新 baseline HYBR（自锁、本 cycle 不追字面 0.700 spec 目标、移交未来 cycle = 更大 / 跨厂 embedding 模型）
- **(4d)** OVERALL HYBR_N ≥ 新 baseline HYBR（自锁、bake sweep best 后理应 = 新 baseline）

### §2.3 控制对照不变性

- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等、cosine 阈值不触发跳 FTS）
- T=1.01 时 HYBR ≡ HYB（六桶完全相等、cosine 路由始终不生效退化为纯 HYB）

## §3 范围（YAGNI）

### §3.1 做什么

| # | 文件 | 改动 | 检查 |
|---|---|---|---|
| 1 | [`packages/evals/src/bin/semantic_quality.rs:163-167`](../../../packages/evals/src/bin/semantic_quality.rs#L163) | 删 `let t: String = text.chars().take(1200).collect();` 一行、把 `rt.embed(&t)` 改为 `rt.embed(text)` | clippy 0、unit test 全过、不动 CLI flag / 不动 embed_and_write 签名 |
| 2 | `packages/evals/fixtures/semantic-recall/cases.json` | +3 case（c079 / c080 / c081）| 桶分类正确（content-not-name × 2 + crosslang × 1）、JSON schema 不破、`check_integrity` 过 |
| 3 | `packages/evals/fixtures/semantic-recall/corpus.json` | +3 doc（s00125 / s00126 / s00127）、目标 token 数 [600, 1800] 安全区间 | 全虚构 + 零 PII + 主题与 case 对齐 + 长文本 token 数验过红线 9 |
| 4 | `packages/evals/fixtures/semantic-recall/vectors.json` | Mac Metal `--embed` 全集重跑（cases 78→81、corpus 124→127）、用未截断后的 embed 路径 | SHA256 落库、L2 norm 抽样验、长文本 token 数从 log 抽 |
| 5 | `packages/result-normalizer/src/lib.rs` | `DEFAULT_COSINE_ROUTING_THRESHOLD` 字面 `0.70` → sweep best（取值在 §5 决策表给定）| clippy 0、调用方不需改 |
| 6 | `packages/evals/fixtures/semantic-recall/baseline.json` | 全 6 bucket 全字段 rewrite 自 bge-m3 + bake T 下单次跑（`--write-baseline`）| gate 4 红线自动套 |
| 7 | `packages/evals/tests/semantic_quality_gate.rs` | 注释升 v5（仅 doc comment 段、assert 代码字节不变）| 注释一致性 |
| 8 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 v5 节（dataset 81 / 127 + sweep 全表 + 4 红线核对 + 诚实边界 + 1200 char 截断解除影响分析）| 沿 v2/v3/v4-fixup 同款结构 |
| 9 | `packages/evals/fixtures/semantic-recall/README.md` | 升 v5 版本说明、列长文本 case 设计意图 + evals embed 路径与 desktop indexer 对齐说明 | 沿 v3 同款风格 |

### §3.2 不做什么

- 不动 desktop wiring（`apps/desktop/src-tauri/src/search/embedding_model.rs` / `apps/desktop/src-tauri/src/settings.rs` 与本 cycle 无关）
- 不动 model-runtime（pooling / llama.rs / context_size 全保 BETA-15B-7-v2 hotfix 状态）
- 不重 embed `vectors-qwen3-0.6b.json`（qwen3-0.6b 已退役、保 v3 状态作 reference snapshot）
- 不重 embed `vectors-bge-m3.json`（保 BETA-15B-8 v4-fixup snapshot 状态作 reference、对应 baseline 报告 v4-fixup 节 metrics）
- 不解决 > 2048 token doc 触发 BERT encode panic（属另一 cycle、本 cycle 长文本红线 [513, 2048] / 设计目标 [600, 1800] 留缓冲）
- 不追 crosslang 字面 0.700 spec 目标（已知 bge-m3 真水位 sweep best ~0.685 < 0.700、移交未来 cycle）
- 不动 wrapper API / RouteVerdict / FanoutOutcome（cosine 路由架构稳定）
- 不动 CLI flag（解除截断 only 改 embed 闭包 body、不改 Cli struct / clap 字段）
- 不动 result-normalizer 其他 DEFAULT_*（DEFAULT_SEMANTIC_WEIGHT / DEFAULT_RRF_K / DEFAULT_SIMILARITY_FLOOR 保 BETA-15B-3 A-2 调优值）

## §4 数据与代码改动详细

### §4.1 evals embed 截断解除

**改动点**：[`packages/evals/src/bin/semantic_quality.rs:163-167`](../../../packages/evals/src/bin/semantic_quality.rs#L163)

```rust
// 改前（BETA-15B-3/6/7/8/9 沿用）
let embed = |text: &str| -> Vec<f32> {
    let t: String = text.chars().take(1200).collect();
    rt.embed(&t).expect("embed 失败")
};

// 改后（BETA-15B-10）
let embed = |text: &str| -> Vec<f32> {
    rt.embed(text).expect("embed 失败")
};
```

**影响范围**：现有 124 doc + 78 query 全部 < 1200 char、改前改后 embed 输入完全等价、向量值理论上 byte-equal（但本 cycle 因 corpus 扩量本来就要全重 embed、不依赖向量层 byte-equal 守恒）。

**与 desktop indexer 对齐说明**：desktop indexer 路径（`apps/desktop/src-tauri/src/search/`）调 `model-runtime::embed` 不截断；解除 evals 截断后两条路径行为等价（同模型 / 同 pooling / 同 context_size=2048 上限）。差异仅在「模型加载入口不同」。

### §4.2 cases.json 扩 3 条

```json
{
  "id": "c079",
  "query": "<zh 长文本 query，目标 corpus s00125>",
  "lang": "zh",
  "bucket": "content-not-name",
  "expected_top_doc_ids": ["s00125"],
  "comment": "BETA-15B-10 长文本 case 1 / zh / 故障复盘 5 Whys 延伸版"
},
{
  "id": "c080",
  "query": "<en 长文本 query，目标 corpus s00126>",
  "lang": "en",
  "bucket": "content-not-name",
  "expected_top_doc_ids": ["s00126"],
  "comment": "BETA-15B-10 长文本 case 2 / en / canary release strategy 延伸版"
},
{
  "id": "c081",
  "query": "<zh query，目标 corpus s00127（en doc）>",
  "lang": "zh",
  "bucket": "crosslang",
  "expected_top_doc_ids": ["s00127"],
  "comment": "BETA-15B-10 长文本 case 3 / zh→en 跨语言 / 日志保留策略 vs log retention policy"
}
```

具体 query 文本在 plan 阶段落、不写进 spec（避免 spec 修订成本）。

### §4.3 corpus.json 扩 3 条

| doc_id | lang | 目标 token 数 | 目标 char 数（参考）| 主题 |
|---|---|---|---|---|
| s00125 | zh | [600, 1200] | ~800-1500 char（zh 0.7-1 token/char）| 事故复盘 5 Whys 模板 + 虚构事故场景 |
| s00126 | en | [600, 1500] | ~2500-6000 char（en 0.25 token/char）| canary release strategy + traffic splitting playbook |
| s00127 | en | [600, 1200] | ~2500-4800 char | log retention policy + compliance considerations（c081 zh→en cross-lang 配对）|

**token 数验证**：commit C1 落库前用 `cargo run -p locifind-evals --bin semantic_quality --release --features metal -- --embed --model models/bge-m3-q8_0.gguf` 看 stderr log 中 `llama_perf_context_print` 输出每 doc 的 prompt eval token 数、抽 s00125/s00126/s00127 三条落入 spec §3.1 红线 9 + plan T2 / T4 验收报告。

**全虚构 + 零 PII**：沿 BETA-15B-6 v1/v2/v3 同款约定。

### §4.4 vectors.json 重 embed

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality --release --features metal -- \
  --embed \
  --model models/bge-m3-q8_0.gguf \
  --vectors-file vectors.json
```

- 模型：`models/bge-m3-q8_0.gguf`（相对 cwd 工作区根、与 BETA-15B-7/8 同款路径）
- 输出 schema：`{ "model_id": "models/bge-m3-q8_0.gguf", "dim": 1024, "doc_vectors": {...}, "query_vectors": {...} }`（实际 schema 见 [`VectorCache`](../../../packages/evals/src/semantic_quality/data.rs)）
- 估时：127 doc + 81 query = 208 个 embed call、Mac Metal ~15-20s（含长文本 BERT encode 时间）
- SHA256 落 plan T4 + spec §6

### §4.5 DEFAULT_COSINE_ROUTING_THRESHOLD bake

[`packages/result-normalizer/src/lib.rs`](../../../packages/result-normalizer/src/lib.rs)（精确行号在 plan）：

```rust
// 旧
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 0.70;

// 新（值由 §5 决策表给定）
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = <sweep_best>;
```

**调用方**：
- `semantic_quality_gate.rs:49`（透传给 `score_case`）— 自动套新值
- desktop 通过 `result-normalizer::DEFAULT_COSINE_ROUTING_THRESHOLD` 间接消费 — 自动套新值（这是 cycle 的桌面行为变更点）

### §4.6 baseline.json rewrite

跑 bge-m3 + 新 corpus + bake T 下单次：

```bash
cargo run -p locifind-evals --bin semantic_quality --release -- \
  --vectors-file vectors.json \
  --semantic-weight 10.0 \
  --cosine-threshold <bake T> \
  --write-baseline
```

输出全 6 bucket 全字段写入 [`packages/evals/fixtures/semantic-recall/baseline.json`](../../../packages/evals/fixtures/semantic-recall/baseline.json)：

```json
[
  {
    "bucket": "OVERALL",
    "n": 81,
    "fts_recall": <f>, "fts_ndcg": <f>,
    "vec_recall": <f>, "vec_ndcg": <f>,
    "hybrid_recall": <f>, "hybrid_ndcg": <f>,
    "hybrid_routed_recall": <f>, "hybrid_routed_ndcg": <f>
  },
  ...
]
```

### §4.7 gate.rs 注释升 v5

只改 doc comment 段（line 1 模块 doc + line 85-89 段落 doc），**不改 assert 代码**（保 code 层 byte-equal）：

```rust
//! BETA-15B-6 → ... → BETA-15B-10 回归门：合成集 hybrid 在关键桶不跌破提交 baseline。
```

```rust
// BETA-15B-3 A-5 红线 + BETA-15B-6 v2 → v3 → BETA-15B-10 v5 校验：...
// —— 4 红线动态读 baseline、A-5 T*=0.60 → v2/v3 T*=0.70 → v5 T*=<sweep_best> bake 后数值无需替换。
// 诚实边界：bge-m3 真水位 crosslang ~0.685 < 0.700 spec 字面、移交未来 cycle = 更大 / 跨厂 embedding 模型。
// 详 docs/reviews/semantic-recall-quality-baseline.md v5 节。
```

## §5 sweep 流程与 Branch 决策表

### §5.1 sweep 流程

```bash
cd /Users/alice/Work/LocalFind
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== T=$t ==="
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file vectors.json \
    --semantic-weight 10.0 \
    --cosine-threshold $t
done | tee /tmp/beta-15b-10-sweep.log
```

9 阈值与 BETA-15B-3 A-5 / BETA-15B-6 v2/v3 同款（控制对照含 0.0 与 1.01）。

### §5.2 Branch 决策表

| Branch | sweep best 落点 | bake 值 | 决策 |
|---|---|---|---|
| **A** | T ∈ {0.0, 0.30, 0.45}（多档并列 sweep best）| **T*=0.45**（取边界上限保 cosine 路由不完全等价 VEC）| **GO** + 文档明示「sweep 多档并列、取上限」|
| **B** | T = 0.60 / 0.70 | bake 该档 | **GO** + 文档明示「ROI 未拿满 v4-fixup 表面 best、因解除截断 / corpus 扩量改变 corpus 形态」 |
| **C** | sweep 全表无任何档过 (4b) 各桶不退步 baseline 红线 | — | **NO GO** + 不合并 + 文档记录 + 留下个 cycle |
| **D** | 长文本 token 数实测不在 [513, 2048] | — | 修 case body 重测、不进入 sweep（plan T2 验收前断）|

**Branch A 推荐处理**：若 sweep 表显示 T ∈ {0.0, 0.30, 0.45} 完全等价，取 **T*=0.45** 作为 bake 值（边界上限）。

**Branch B vs Branch A 的语义差**：Branch A 表示 cosine 信号在 < 0.45 区间没有进一步分辨力（多档并列）；Branch B 表示新 corpus（解除截断 + 加长文本）让 cosine 信号边界后移。两个都是 GO，文档记录差异。

### §5.3 与 spec §2.2 的关系

任何 Branch 进 GO 都需 §2.2 (4a)~(4d) 全过、其中 (4b) 是 (4c)/(4d) 自锁的前提。若 (4b) 因长文本 case 的 `expected_top_doc_ids` 设计偏置导致某桶退步、回 §4.2/§4.3 重设计 case 或调 expected_top_doc_ids（plan T6 验收时断）。

## §6 验证产物 / 落库

- `vectors.json` SHA256（plan T4 验、入 baseline 报告 v5 节）
- `baseline.json` 内容（plan T7 跑完 `--write-baseline` 后入仓）
- baseline 报告 v5 节：
  - dataset 81 / 127 全量 sweep 表（9 阈值 × 6 桶）
  - 4 红线核对表
  - 1200 char 截断解除影响分析（现有 124 doc 改前改后向量等价、新 3 doc 是截断解除后才能完整 embed）
  - 与 v3 / v4-fixup 对比表
  - 诚实边界（crosslang < 0.700 spec 字面、不追）
- fixtures README v5 注脚：长文本 case 设计意图 + token [600, 1800] 区间约束 + evals embed 路径与 desktop indexer 对齐说明
- gate.rs doc comment v5 注释

## §7 风险 / 诚实边界

### §7.1 数据风险

- **R1**：corpus 127 vs 124、+3 长文本可能使 sweep best 偏离 v4-fixup 表的 {0.0/0.30/0.45}。本 cycle 接受 sweep best 移到 0.60+ 的可能（走 Branch B）、不强求 T*=0.45。
- **R2**：3 条长文本占 corpus 2.4%、对 OVERALL ndcg 影响理论 < 0.005 量级、不应主导 sweep 决策。若 sweep 表显示 OVERALL ±0.02+ 剧变、说明 case 设计存在偏置（c079/c080/c081 之一的 expected 太刚好或太异常），回 §4.2/§4.3 重设计。
- **R3**：crosslang 桶现 13 例 + c081 → 14 例、仍偏小。本 cycle 不解决（评测扩量到 crosslang 20+ 在 STATUS follow-up 登记）。
- **R5**：解除 1200 char 截断后理论上现有 124 doc 全 < 1200 char、向量等价。若实际 `git diff vectors.json` 显示现有 doc 向量也变（非新增 3 doc 之外的字段），说明假设错（如某 doc 字符数刚好 > 1200），需 plan T4 验收时 diff 检查 + 接受变化（重 sweep / 重 baseline 自动覆盖）。

### §7.2 桌面行为变更与真机手测

- **R4**：bake 后 `DEFAULT_COSINE_ROUTING_THRESHOLD` 字面值变（0.70 → §5 给定值）、桌面 cosine 路由触发更宽松。本 cycle 按 spec §3.2 / §7 判平凡（cosine 阈值是数值微调、路由架构不变、`apps/desktop/src` 编译过 + tsc 净已覆盖）→ **Mac 真机手测 DEFERRED**（GO with documented gap 路径、留用户首次升级时按 `docs/manual-test-scenarios.md` 走三步、与 BETA-15B-7-v2 T3 同款）。

### §7.3 v3 → v5 编号断档

- v4 系列（BETA-15B-7 v4 + BETA-15B-8 v4-fixup）只动 baseline 报告（追加节）、未动 vectors.json / baseline.json / cases.json / corpus.json。dataset 正式版本号仍是 v3、v4 系列产物是 v3 数据集上的二次 sweep 探针。本 cycle 第一次实质改动 dataset 内容（cases + corpus + vectors） + baseline.json + 解除截断、命名 v5 跳过 v4（避免与 BETA-15B-7/8 v4 系列分析报告混淆）。

### §7.4 qwen3-0.6b / bge-m3 vectors snapshot

- `vectors-qwen3-0.6b.json` 保 v3 124 doc 状态作 reference snapshot（baseline 报告 v5 节明示）
- `vectors-bge-m3.json` 保 BETA-15B-8 v4-fixup snapshot 状态作 reference snapshot（对应 baseline 报告 v4-fixup 节 metrics、是 124 doc + 截断状态的快照）
- 主 active vectors 走 `vectors.json` = 本 cycle 重 embed 的 bge-m3 全 81/127 数据（解除截断 + 扩量后的全新状态）

### §7.5 evals 截断解除的 collateral 影响

- **R6**：解除 1200 char 截断后 evals binary embed 时间略增（每 doc / query 端到端走完整 BERT encode、不被截断短路）。208 个 embed call 估时从 ~12s 增到 ~15-20s、不影响 CI 时间预算。
- **R7**：解除截断 + 加入长文本 case 后、evals 跑出的 cosine 分布更代表真机场景。但合成 corpus 仍非真实用户 doc、覆盖度天然受限（评测扩量 / BETA-30 失败样本箱在 STATUS 已登记）。

## §8 commit 颗粒度（方案 B）

- **C1 — corpus + vectors + 截断解除**
  - 文件：§3.1 row 1 + row 2 + row 3 + row 4
  - 验证：red line 1/2/3/4/8/9 全过、gate 不退步**现行**（守 qwen3-0.6b 的 v3）baseline（理论上不退步：新增 case/doc 加进来、现有 case 不动、`take(1200)` 解除对 < 1200 char doc 等价）
  - 关键：截断解除 + corpus 扩量绑在同一 commit、向量层一次性切换、避免 `vectors.json` 在中间状态偏离

- **C2 — cosine bake + baseline rewrite + gate.rs 注释升 v5**
  - 文件：§3.1 row 5 + row 6 + row 7
  - 验证：red line 1/2/3/4/5/6/7 全过、gate 4 红线全过**新**（守 bge-m3 + 81/127）baseline
  - 关键：bake 值 + baseline 数值绑在同一 commit、避免任意中间状态 gate FAIL

- **C3 — doc-sync + PR**
  - 文件：§3.1 row 8 + row 9 + STATUS.md + ROADMAP.md
  - 验证：red line 1/2 净（doc-only、不动代码）

每 commit 推 origin 后跑全验证门；C3 落地后开 PR、走 BETA-15B-7-v2 同款 gh CLI 路径合 main。

## §9 PR 与合 main 路径

- 分支：`feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text`
- PR title：「BETA-15B-10：bge-m3 baseline 重锚 + cosine_threshold sweep & bake + 评测集长文本扩量 + evals embed 截断解除」
- PR body：cycle 范围 + 关键数据 + 接受标准 + Mac 真机手测 DEFERRED 声明 + 未尽事宜
- 合 main：`gh pr merge <N> --merge --delete-branch`（同 BETA-15B-7-v2 / BETA-15B-9 同款）

## §10 修订记录

- 2026-06-26 v1：spec 初稿（brainstorming 3 段批准后落库）
- 2026-06-26 v2：cycle 起手实测发现 [`packages/evals/src/bin/semantic_quality.rs:165`](../../../packages/evals/src/bin/semantic_quality.rs#L165) 有 `text.chars().take(1200)` 字符截断、`--threshold-sweep` flag 不存在（实际用 shell 循环）。修订：
  - §1.2 新增「evals embed 截断发现 + framing 修订」段
  - §3.1 row 1 新增截断解除改动
  - §4 新增 §4.1 截断解除细节、§4.4/§4.6 命令格式修正（加 `--bin` + `--model`）
  - §5.1 sweep 流程改 shell 循环
  - §7.5 新增解除截断 collateral 影响段
  - §8 C1 加入截断解除改动
