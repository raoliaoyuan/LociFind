# BETA-15B-3 簇 A-4：query 语种检测 + 跨语种 vec hit 路由 设计 spec

> 承接 A-3（FTS 置信度路由 / Jaccard 单维信号 / spec §5 降级、路由本 cycle 不生效）；本 cycle 升级 wrapper 内部信号为 **query 语种检测 + 跨语种 vec hit 计数**，对准 A-3 暴露的 crosslang 桶失败模式。

## 1. 背景与动机

A-3 用 FTS/VEC top-K Jaccard 重叠作路由信号，sweep 6 阈值发现：
- **content-not-name 桶 FTS_N=0.849 > VEC_N=0.833**，路由跳 FTS 反伤此桶（t ≥ 0.20 起退步 0.098 破 spec (4b) 各桶 ≥ HYB baseline 硬红线）
- spec §5 字面正解「最保守 t」 → 选 **t\*=0.10**（实测无 case Jaccard 在 (0, 0.10) 触发 → HYBR ≡ HYB → 路由对指标本 cycle 净影响 = 0）
- crosslang HYBR_N 0.649 / OVERALL 0.854 仍等于 A-2 baseline，spec §2.2 (4c)(4d) 未达

A-3 诚实边界结论：**Jaccard 单维信号天然分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」两场景，纯调阈值救不了**。

A-3 baseline 报告暴露的失败模式 = crosslang 桶 FTS 同语言 g1 hit 淹没 VEC 跨语言 g3/g2 召回，nDCG 跌。**直接对准这种失败模式的信号 = query 语种检测**：query 是英文时，FTS 必然只能命中英文 hit；VEC 跨语言能召中文/英文混合 hit；混合后英文 hit 排序占优、跨语言召回被淹。判定信号 = **query 单语种 + vec top-K 中跨语种 hit 计数 ≥ 阈值** → 跳 FTS。

## 2. 目标与验收

### 2.1 目标

- wrapper `fuse_rrf_with_fts_routing` 内部信号从 Jaccard 升级为 query_lang + cross_lang_hits 复合信号；**API 名 / RouteVerdict / HYBR baseline 字段名 / gate 红线架构全部保留**（与 A-3 同款 wrapper 框架延续）
- query 语种检测纯自写 CJK Unicode ratio 三态二阈（Zh / En / Mixed），零依赖、纯 std
- 评测层 sweep + bake → 生产 query-lang-only 直接接入（生产端 `SearchResult` schema 零改动）

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed
(2) `cargo clippy --workspace --all-targets -D warnings` 0 warning
(3) `cargo fmt --all --check` 净
(4) 评测 `semantic_quality_gate` 用新 baseline pass，含四条硬断言：
- (4a) exact-name HYBR_R == 1.0
- (4b) 各桶 HYBR_N ≥ HYB baseline（与 A-2 锁定一致、不退步）
- (4c) OVERALL HYBR_N ≥ baseline.json 新水位（自锁）
- (4d) crosslang HYBR_N ≥ baseline.json 新水位（自锁）

(5) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser）
(6) 前端 tsc + vite build 净（本 cycle 不动前端、自然过）

**优先期望（不进硬红线）**：
- OVERALL HYBR_N > 0.854（A-2 baseline）
- crosslang HYBR_N > 0.649（A-2 baseline）

若优先期望未达 → spec §5 字面正解，bake `max_cross_lang_hits = usize::MAX`（路由本 cycle 不生效），归下 cycle 抓手（候选 ② VEC cosine 阈值 / ③ 更大 embedding 模型 / ④ 评测集扩量）。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- 新增 `result-normalizer/src/lang.rs` 内部模块：`pub enum Lang { Zh, En, Mixed }` + `pub fn detect_lang(text: &str) -> Lang`（CJK Unified Ideographs + Compatibility + Ext-A Unicode range ratio）
- `fuse_rrf_with_fts_routing` 签名升级：`threshold: f64` → `query_lang: Lang, max_cross_lang_hits: usize`
- 常量重命名：`DEFAULT_FTS_JACCARD_THRESHOLD` → `DEFAULT_MAX_CROSS_LANG_HITS`；`DEFAULT_FTS_ROUTING_TOP_K=10` 复用
- `RouteVerdict` 字段升级：`jaccard: f64` → `cross_lang_hits: usize`；新增 `query_lang: Lang`；保留 `skipped_fts/fts_top_k/vec_top_k`
- 评测层 `to_results` 改：`name = title`（带自然语言 CJK 信号），`path = doc_id`（dedup key 不变）；签名扩 `corpus: &[SemanticDoc]`
- `arms::hybrid_routed_rank` 参数 `jaccard_threshold` → `(query_lang, max_cross_lang_hits)`
- `report::score_case` 入口加 `let query_lang = detect_lang(&case.query);`
- `bin/semantic_quality.rs` flag `--jaccard-threshold` → `--max-cross-lang-hits`
- 生产 `fanout_merge::run_fanout_merge_rrf` 入口 `let query_lang = detect_lang(&query);`，wrapper 调用传 `query_lang + DEFAULT_MAX_CROSS_LANG_HITS`；call-site 透传 `query: &str`（若签名无）
- 评测 sweep + bake、写新 baseline.json
- gate 4 条红线断言数值替换为新 baseline
- baseline 报告追加 A-4 调优记录节

### 3.2 Out-of-scope（明确 YAGNI）

- ❌ 拆 `lang-detect` 独立 crate（暂作 `result-normalizer/src/lang.rs` 内部模块；未来若 fanout/agent harness 也要 detect 再拆）
- ❌ 给生产 `SearchResult` 加 `language: Option<Lang>` 字段（生产用 `name` 自检测、零 schema 改动）
- ❌ UI 暴露路由开关 / 阈值（与 A-3 一致：路由是后端启发式，旋钮 bake 即开关，不入 settings）
- ❌ 改 `SearchIntent / FileSearch` schema
- ❌ 删 Jaccard 工具函数 `jaccard_overlap_by_path` 与单测（标 `#[allow(dead_code)]` 保留为下 cycle 信号组合预留）
- ❌ feature flag / env switch（默认启用、bake 即开关）
- ❌ 多语种 Set / 4 语种以上检测（YAGNI、EN+ZH 三态足够）
- ❌ 软调权（FTS RRF 权重 × lang_match_factor）—— 与 A-3 wrapper「硬跳」不同构、不本 cycle
- ❌ FTS hit 级过滤（保留 FTS 走但 hit 级过 lang ≠ query_lang 的 hit）—— wrapper API 装不下、gate 架构 ×
- ❌ A-3 Jaccard 信号删除 / wrapper 改名 / RouteVerdict 字段精简（基础设施零破坏）

## 4. 架构

### 4.1 模块图

```
packages/result-normalizer/src/
├── lib.rs                          # fuse_rrf / fuse_rrf_with_fts_routing wrapper 升级、RouteVerdict 升级
└── lang.rs                         # 新增：Lang enum + detect_lang fn

packages/harness/src/fanout_merge.rs
└── run_fanout_merge_rrf            # 入口 detect_lang(&query) → 传 wrapper

packages/evals/src/semantic_quality/
├── arms.rs                         # to_results 加 corpus 参；hybrid_routed_rank 改参
├── report.rs                       # score_case 加 query_lang 检测；CaseScores/BucketAgg HYBR 字段名保留
└── data.rs                         # 不动（SemanticDoc.lang 已有，不动 schema）

packages/evals/src/bin/semantic_quality.rs
└── --max-cross-lang-hits flag      # 替换 --jaccard-threshold

packages/evals/fixtures/semantic-recall/
├── baseline.json                   # HYBR 字段 rewrite + 加 max_cross_lang_hits_bake 元数据
└── （vectors/cases/corpus 不动）

packages/evals/tests/semantic_quality_gate.rs   # 4 条红线断言数值替换
```

### 4.2 关键类型（最终签名）

```rust
// result-normalizer/src/lang.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
    Mixed,
}

/// CJK Unified Ideographs (U+4E00–U+9FFF) + Compatibility (U+F900–U+FAFF)
/// + Ext-A (U+3400–U+4DBF) ratio 三态二阈：>0.6=Zh、<0.05=En、之间=Mixed。
/// 分母为「CJK + alphanumeric ASCII」chars 总数（忽略空格/标点）。
/// 分母为 0 → 返回 Mixed（保守降级、退化为不路由）。
pub fn detect_lang(text: &str) -> Lang;

// result-normalizer/src/lib.rs
pub const DEFAULT_MAX_CROSS_LANG_HITS: usize = /* sweep 后 bake，spec §5 保守限 */;
pub const DEFAULT_FTS_ROUTING_TOP_K: usize = 10;  // A-3 复用

#[derive(Debug, Clone, PartialEq)]
pub struct RouteVerdict {
    pub skipped_fts: bool,
    pub query_lang: Lang,
    pub cross_lang_hits: usize,
    pub fts_top_k: usize,
    pub vec_top_k: usize,
}

#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts: Vec<SearchResult>,
    vec: Vec<SearchResult>,
    k: f64,
    semantic_weight: f64,
    query_lang: Lang,
    max_cross_lang_hits: usize,
) -> (Vec<MergedResult>, RouteVerdict);
```

### 4.3 wrapper 内部决策树

```
入口（fts, vec, k, w, query_lang, max_cross_lang_hits）
  ├─ 任一臂空（fts.is_empty() || vec.is_empty()） → empty-arm guard
  │   不跳；调 fuse_rrf(fts ⊕ vec, k, w)；
  │   verdict = { skipped_fts: false, query_lang, cross_lang_hits: 0,
  │              fts_top_k: 0/vec_top_k: 0 / 按实际填 }
  │
  ├─ query_lang == Mixed → 保守降级
  │   不跳；调 fuse_rrf(fts ⊕ vec, k, w)；
  │   verdict = { skipped_fts: false, query_lang: Mixed, cross_lang_hits: 0, ... }
  │
  └─ query_lang ∈ { Zh, En }：
       1. let vec_top = &vec[..min(DEFAULT_FTS_ROUTING_TOP_K, vec.len())];
       2. let count = vec_top.iter()
              .filter(|r| detect_lang(&r.name) != query_lang
                       && detect_lang(&r.name) != Mixed)
              .count();
       3. if count >= max_cross_lang_hits：
              跳 FTS；调 fuse_rrf(vec![] ⊕ vec, k, w)；
              verdict.skipped_fts = true、cross_lang_hits = count、其他填实际
          else：
              不跳；调 fuse_rrf(fts ⊕ vec, k, w)；
              verdict.skipped_fts = false、cross_lang_hits = count、其他填实际
```

控制对照不变性（与 A-3 同款语义）：
- `max_cross_lang_hits = 0` → 任意 count ≥ 0 永真 → 永远跳（≈纯 vec 控制，但 Mixed 例外仍不跳）
- `max_cross_lang_hits = usize::MAX` → 永不跳（≡HYB 控制，与 A-3 spec §5 降级值同义）
- `query_lang = Mixed` 永不跳（保守降级、独立于阈值）

### 4.4 数据流

**生产**：
```
agent harness → fanout_merge::run_fanout_merge_rrf(deps, query, …)
  ├─ query_lang = detect_lang(&query)（入口一次性）
  ├─ fts_results = native arm
  ├─ vec_results = semantic arm
  └─ fuse_rrf_with_fts_routing(fts, vec, RRF_K, weight_provider.semantic_weight(),
                                query_lang, DEFAULT_MAX_CROSS_LANG_HITS)
      → FanoutOutcome { merged, route_verdict: Some(verdict) }
```

**评测**：
```
score_case(case, corpus, …)
  ├─ query_lang = detect_lang(&case.query)（入口）
  ├─ fts_ids = fts_rank(...)
  ├─ vec_ids = vector_rank(...)
  ├─ hybrid_routed_rank(corpus, &fts_ids, &vec_ids, query_lang, max_cross_lang_hits, w, k)
  │     │
  │     └─ to_results(corpus, ids, source, mt) 内部：
  │           name = corpus.iter().find(|d| d.doc_id == id).map(|d| d.title.clone()).unwrap_or(id)
  │           path = PathBuf::from(id)（dedup key）
  └─ nDCG / report 写入
```

**单测**：直接传 `Lang::Zh / Lang::En / Lang::Mixed`，不跑 detect_lang。

**CLI**：`semantic_quality --max-cross-lang-hits <N>` 只暴露 bake 旋钮；query_lang 从 case.query 检测，不接 CLI flag。

## 5. 调优工作流（spec 阶段执行）

```bash
# 步骤 0：分支上落 lang.rs + wrapper 升级 + RouteVerdict 升级 + 评测改造 + binary flag
# （task 1–6 全 TDD、每 task 验证门 = fmt + clippy + test，含 v0.5 byte-equal 闸门）

# 步骤 1：跑 sweep（用现有 vectors.json 缓存，零模型推理；W 固定 10.0；top_k 复用 10）
for N in 0 1 2 3 5; do
  cargo run --release -p evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --max-cross-lang-hits $N \
    >> sweep-lang.log
done
# 控制对照：N=0 应使 HYBR ≈ VEC（Mixed 例外仍 HYB）；N=usize::MAX 应使 HYBR ≡ HYB
cargo run --release -p evals --bin semantic_quality -- \
  --semantic-weight 10.0 --max-cross-lang-hits 18446744073709551615 \
  >> sweep-lang.log
# usize::MAX 用十进制写法或通过 binary 加 max sentinel；细节 plan 阶段定

# 步骤 2：人工读 sweep-lang.log，选满足以下四条的 N*：
#   ① exact-name HYBR_R = 1.000（不满足 = bug）
#   ② 各桶 HYBR_N ≥ HYB baseline 同桶（不退步硬红线）
#   ③ OVERALL HYBR_N 最大化（优先 > 0.854）
#   ④ crosslang HYBR_N 最大化（优先 > 0.649）
# 若 ② 与 ③/④ 冲突（如 A-3 content-not-name 桶 FTS 强反伤重现）→ spec §5 字面正解：
#   取最保守值 N = usize::MAX（路由本 cycle 不生效，HYBR ≡ HYB）

# 步骤 3：bake N* 到 DEFAULT_MAX_CROSS_LANG_HITS
# 步骤 4：rewrite baseline.json（HYBR 字段新水位 + max_cross_lang_hits_bake 元数据；HYB 字段保留）
# 步骤 5：跑回归门验证
cargo test -p evals --test semantic_quality_gate
# 步骤 6：baseline 报告追加 A-4 调优记录节（sweep 全表 + bake 路径 + 诚实边界 + 下 cycle 抓手）
```

## 6. 代码改动清单（按 plan task 颗粒度）

| Task | 文件 | 改动 |
|---|---|---|
| T1 | `result-normalizer/src/lang.rs`（新增） | `Lang` enum + `detect_lang` 函数 + ≥ 5 单测（Zh/En/Mixed 边界 + 空串 + ratio 边界 0.05/0.6 邻域） |
| T2 | `result-normalizer/src/lib.rs` | `RouteVerdict` 字段改造；`fuse_rrf_with_fts_routing` 签名 + 内部信号升级；`DEFAULT_MAX_CROSS_LANG_HITS` 占位（task 7 bake）；A-3 既有 wrapper 单测改 lang 信号；新加 Mixed-不跳 / 跨语种触发 / 计数边界 等 ≥ 4 单测；Jaccard 工具函数与单测保留 `#[allow(dead_code)]` |
| T3 | `evals/src/semantic_quality/arms.rs` | `to_results` 加 `corpus` 参；`hybrid_routed_rank` 改参；既有 5 个 hybrid_routed_* 单测改 lang 信号 |
| T4 | `evals/src/semantic_quality/report.rs` | `score_case` 入口 `detect_lang(&case.query)`；签名 `jaccard_threshold` → `max_cross_lang_hits`；`CaseScores/BucketAgg` HYBR 字段名不变 |
| T5 | `evals/src/bin/semantic_quality.rs` | CLI flag `--jaccard-threshold` → `--max-cross-lang-hits`（usize）；表格 HYBR_R/HYBR_N 列保留；usize::MAX sentinel 由 binary 接受（特殊值 "max" 或 `--no-route` flag，细节 plan 阶段定） |
| T6 | （手动 sweep） | 步骤 1–2，产出 sweep-lang.log |
| T7 | `result-normalizer/src/lib.rs` + `baseline.json` | bake `DEFAULT_MAX_CROSS_LANG_HITS = N*`；rewrite baseline.json |
| T8 | `evals/tests/semantic_quality_gate.rs` | 4 条红线断言数值替换；架构不变 |
| T9 | `harness/src/fanout_merge.rs` | `run_fanout_merge_rrf` 入口 `detect_lang(&query)`；wrapper 调用改新签名；签名扩 `query: &str`（若需）；call-site 透传；新增单测覆盖 wrapper 三态分支；`FanoutOutcome.route_verdict` 透传不动 |
| T10 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 A-4 调优记录节 + 总验收过 |

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `cargo test --workspace`
- 必要 task 追加：
  - T2 / T7：evals parser-only byte-equal v0.5=473 / v0.9=877 不变（规范化逐 case 比对，[[project-evals-reporter-nondeterministic]]）
  - T8：`cargo test -p evals --test semantic_quality_gate` 用新 baseline pass
  - T9：fanout 单测覆盖 wrapper 三态（Zh 路由 / En 路由 / Mixed 不跳）
  - T10：baseline 报告 A-4 节完整含 sweep 全表 + bake 路径 + 诚实边界

## 8. 风险与已记忆教训

### 8.1 风险

| 风险 | 缓解 |
|---|---|
| **lang 信号同样撞 content-not-name 桶反伤**（A-3 同款失败模式重现） | spec §5 降级路径已留：取最保守 N=usize::MAX、路由不生效、HYBR ≡ HYB；归下 cycle 抓手（候选 ②/③/④）；不强行 bake 越红线值 |
| **detect_lang 在 Mixed 边界误判**（"iPhone 备份"、"qwen 调优"、"BETA-13 决策"） | Mixed 进保守降级（不路由）；阈值二阈 0.05/0.6 留宽 Mixed 区；单测覆盖典型混合 query |
| **评测 `to_results` 改 `name=title` 可能破坏 fuse_rrf dedup** | path=doc_id 仍是 dedup key，name 仅供 detect_lang 信号；既有 fuse_rrf 单测全跑过即不破坏；T3 验证门必加 |
| **生产 `run_fanout_merge_rrf` 签名扩 query 影响 call-site** | call-site 已知主要在 desktop wiring 路径；T9 完整透传不留 `query=""` 占位 |
| **byte-equal 风险 v0.5/v0.9 退步** | 本 cycle 完全不动 parser、coverage、不动 model fallback；只动 result-normalizer wrapper + evals 设施 + fanout 入口检测；byte-equal 自然保持 |
| **A-3 既有 wrapper 单测全要改 lang 信号** | 信号语义升级、单测改名 + 用 Lang 枚举重写；保留控制对照（max=0 / max=MAX / empty-arm guard / Mixed-不跳）|

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 coverage，不触发
- [[project-evals-reporter-nondeterministic]]：T2/T7 byte-equal 闸门用规范化逐 case 比对，不裸 diff JSON
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告，三件套全做
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring，不触发
- [[project-rrf-weight-tuning-ceiling]]：A-2 RRF weight 已到天花板，本 cycle 不调 W=10.0，专注路由信号

## 9. 链接

- A-3 spec：[2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md](./2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md)
- A-2 spec：[2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md](./2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)（本 cycle T10 追加 A-4 节）
- 项目状态：[../../../STATUS.md](../../../STATUS.md) §「下一步」候选 ① query 语种检测 + 文档语种过滤
