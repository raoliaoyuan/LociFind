# BETA-15B-3 簇 A-5：VEC top-1 cosine 绝对分数阈值路由 设计 spec

> 承接 A-4（query 语种检测 + 跨语种 vec hit 路由 / lang 单维信号 / spec §5 降级 N\*=usize::MAX 路由本 cycle 不生效）；本 cycle 升级 wrapper 内部信号为 **VEC top-1 cosine 绝对分数阈值**，对准 A-4 暴露的 crosslang 桶 / content-not-name 桶 不对称失败模式。

## 1. 背景与动机

A-3 用 FTS/VEC top-K Jaccard 重叠 / A-4 用 query 语种检测 + 跨语种 vec hit 计数，连续两 cycle 单维信号路由都撞同款失败模式天花板：

- **crosslang 桶**：vec 强（跨语言召回 cosine 高）+ FTS 同语言 g1 hit 添噪 → 应跳 FTS
- **content-not-name 桶**：vec 弱（cosine 低）+ FTS 字面匹配实际有用（FTS_N=0.930 > VEC_N=0.833）→ 应保留 FTS
- 单维 Jaccard / 单维 lang 信号在这两桶上**触发逻辑相同**（两臂分歧大 / 跨语种 hit 存在），导致路由动作不能不对称区分两场景

A-4 数据指证下 cycle 抓手优先级排序中 **② VEC top-1 cosine 绝对分数阈值最优先**：

- **理论上信号方向与失败模式同构**：cosine 高 ↔ vec 强 ↔ FTS 添噪应跳 ✓；cosine 低 ↔ vec 弱 ↔ FTS 帮应保留 ✓
- **不需训练 / 不需评测集扩量 / 不需更大模型**——最低成本验证路径
- **生产侧 cosine 已可直接触达**：`packages/search-backends/semantic-index/src/lib.rs:163` 注释明示 `score = cosine（升 f64）`
- 评测侧 `packages/evals/src/semantic_quality/arms.rs:104` 当前 `score: None`——A-5 in-scope 改造透传 cosine

若 sweep 后 cosine 单维仍撞同款 (4b) 天花板 → spec §5 字面正解保守降级 bake `cosine_threshold = 1.01`（永不跳、HYBR ≡ HYB），归 A-6 cycle 抓手（候选 ③ 更大 embedding 模型 / ④ 评测集扩量 + 重构 content-not-name 桶）。

## 2. 目标与验收

### 2.1 目标

- wrapper `fuse_rrf_with_fts_routing` 内部信号从 lang 升级为 **VEC top-1 cosine 绝对分数阈值**；**API 名 / RouteVerdict 字段架构 / HYBR baseline 字段名 / gate 红线架构全部保留**（A-3/A-4 基础设施延续）
- 信号源：`vec[0].score.unwrap_or(0.0)` —— 生产侧已是 cosine、评测层 in-scope 改造透传
- 动作方向：`cosine_top1 >= cosine_threshold` 跳 FTS（vec 强信任跳）
- 评测层 sweep + bake → 生产直接接入（生产端 `SearchResult` schema 零改动）
- A-3 jaccard / A-4 detect_lang 函数体删除，保留 `Lang` enum + `RouteVerdict.query_lang` 字段作可观测元数据（评测/badge 用、不再驱动路由）

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
- OVERALL HYBR_N > 0.854（A-4 baseline）
- crosslang HYBR_N > 0.649（A-4 baseline）

若优先期望未达 → spec §5 字面正解，bake `cosine_threshold = 1.01`（路由本 cycle 不生效），归下 cycle 抓手（A-6 候选 ③ 更大 embedding 模型 / ④ 评测集扩量）。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- wrapper `fuse_rrf_with_fts_routing` 签名升级：A-4 6 参 `(fts, vec, k, w, query_lang, max_cross_lang_hits)` → A-5 5 参 `(fts, vec, k, w, cosine_threshold: f64)`
- 常量升级：`DEFAULT_MAX_CROSS_LANG_HITS` → `DEFAULT_COSINE_ROUTING_THRESHOLD: f64`；`DEFAULT_FTS_ROUTING_TOP_K=10` 保留（empty-arm guard 仍参考 top-K 概念但 cosine 只看 top-1）
- `RouteVerdict` 字段升级：`cross_lang_hits: usize` → `vec_top1_cosine: f64`；`max_cross_lang_hits` → `cosine_threshold: f64`；**保留 `query_lang: Lang`**（评测/badge 可观测元数据、不再驱动路由）；保留 `skipped_fts: bool`
- 评测层 `arms::vector_rank` 签名改：返 `Vec<(String, f32)>`（doc_id + cosine）替代当前 `Vec<String>`
- 评测层 `arms::to_results` 签名改：接 `score: Option<f32>`（每 doc 各自的 cosine 或 None）+ `to_results` 把 cosine 升 f64 挂在 `SearchResult.score`
- `arms::hybrid_routed_rank` 签名改：参数 `(query_lang, max_cross_lang_hits)` → `cosine_threshold: f64`
- `report::score_case` 入口仍调 `detect_lang(&case.query)` 填 `RouteVerdict.query_lang` 元数据但**不**传 wrapper；签名 `max_cross_lang_hits` → `cosine_threshold`
- `bin/semantic_quality.rs` flag `--max-cross-lang-hits` → `--cosine-threshold`（f64）
- 生产 `fanout_merge::run_fanout_merge_rrf` 入口移除 `detect_lang(query)` 调用、wrapper 调用传 `DEFAULT_COSINE_ROUTING_THRESHOLD`；call-site `query: &str` 透传保留（评测层仍需）
- 评测 sweep + bake、写新 baseline.json
- gate 4 条红线动态读 baseline（数值无需替换、A-4 已是动态读）
- baseline 报告追加 A-5 调优记录节
- A-3 `jaccard_overlap_by_path` 函数体删除 + 5 单测删除
- A-4 `lang::detect_lang` 函数体删除 + 8 单测删除；**保留** `lang::Lang` enum（`RouteVerdict.query_lang` 仍需）

### 3.2 Out-of-scope（明确 YAGNI）

- ❌ cosine + lang/jaccard 组合信号（YAGNI、分步验证单维 cosine 是否破局；若 spec §5 降级则下 cycle 再做组合）
- ❌ VEC top-K 均值 cosine / VEC top-1 - top-2 差距等其他 cosine 变体（YAGNI、top-1 最简最直观）
- ❌ UI 暴露 cosine 阈值（与 A-3/A-4 同款不暴露；路由是后端启发式 vs floor/weight 的"用户偏好"语义不同）
- ❌ 软调权（FTS RRF 权重 × cosine_factor）—— 与 A-3/A-4 wrapper「硬跳」不同构、不本 cycle
- ❌ FTS hit 级过滤 / per-bucket 自适应阈值
- ❌ A-5 vs A-4 同期对照 HYBR2 列（baseline.json 字段保持简洁、HYBR 字段重定义为 cosine 路由）
- ❌ 删 `Lang` enum / 删 `RouteVerdict.query_lang` 字段（保留作评测可观测元数据、为 BETA-15B-5 badge 槽位预留）
- ❌ feature flag / env switch（默认启用、bake 即开关）
- ❌ 改 `SearchIntent / FileSearch` schema / 改生产 `SearchResult` schema（生产侧 `score=cosine` 已就绪）

## 4. 架构

### 4.1 模块图

```
packages/result-normalizer/src/
├── lib.rs                          # fuse_rrf_with_fts_routing wrapper 签名升级、RouteVerdict 字段升级、删 jaccard_overlap_by_path 函数体 + 5 单测
└── lang.rs                         # 删 detect_lang 函数体 + 8 单测，保留 Lang enum

packages/harness/src/fanout_merge.rs
└── run_fanout_merge_rrf            # 入口移除 detect_lang(query) 调用、传 DEFAULT_COSINE_ROUTING_THRESHOLD

packages/evals/src/semantic_quality/
├── arms.rs                         # vector_rank 返 (doc_id, cosine)、to_results 挂 score、hybrid_routed_rank 改 cosine_threshold
├── report.rs                       # score_case 仍 detect_lang 填 RouteVerdict.query_lang 元数据但不传 wrapper；签名改 cosine_threshold
└── data.rs                         # 不动（SemanticDoc 不动）

packages/evals/src/bin/semantic_quality.rs
└── --cosine-threshold flag         # 替换 --max-cross-lang-hits

packages/evals/fixtures/semantic-recall/
├── baseline.json                   # HYBR 字段 rewrite + 加 cosine_threshold_bake 元数据（删 max_cross_lang_hits_bake）
└── （vectors/cases/corpus 不动）

packages/evals/tests/semantic_quality_gate.rs   # 4 条红线动态读 baseline，架构不变
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
// detect_lang 函数体删除（评测层不再调用驱动路由；若评测仍要记元数据可保留独立 helper 或直接挂 Mixed 占位）

// result-normalizer/src/lib.rs
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = /* sweep 后 bake，spec §5 保守限 = 1.01 */;
pub const DEFAULT_FTS_ROUTING_TOP_K: usize = 10;  // A-3 复用，empty-arm 概念性 top-K（cosine 实际只看 top-1）

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    pub skipped_fts: bool,
    /// 评测/badge 元数据：query 语种检测结果。本 cycle 起不再驱动路由动作。
    pub query_lang: crate::lang::Lang,
    /// vec top-1 cosine 实测值（`vec[0].score.unwrap_or(0.0)`），路由动作的实际输入。
    pub vec_top1_cosine: f64,
    /// 当时使用的阈值（便于事后审计）。
    pub cosine_threshold: f64,
}

#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts: Vec<SearchResult>,
    vec: Vec<SearchResult>,
    k: f64,
    semantic_weight: f64,
    cosine_threshold: f64,
) -> (Vec<MergedResult>, RouteVerdict);
```

注：`RouteVerdict` 由 wrapper 内部直接构造，wrapper 不知道 query_lang 真实值——`query_lang` 默认填 `Lang::Mixed` 占位；评测/生产 wiring 若要填真实值，须在 wrapper 返回后**覆写** `verdict.query_lang = detect_lang(query)` 一次（或后续重构 wrapper 接受 `query_lang_for_metadata: Lang` 6 参，仍维持 cosine 单维驱动）。**本 cycle 落 wrapper 内部填 `Lang::Mixed` 占位，调用侧后置覆写**，保持 wrapper 签名 5 参纯净。

### 4.3 wrapper 内部决策树

```
入口（fts, vec, k, w, cosine_threshold）
  ├─ 任一臂空（fts.is_empty() || vec.is_empty()） → empty-arm guard
  │   不跳；调 fuse_rrf(fts ⊕ vec, k, w)；
  │   verdict = { skipped_fts: false, query_lang: Mixed,
  │              vec_top1_cosine: vec.first().and_then(|r| r.score).unwrap_or(0.0),
  │              cosine_threshold }
  │
  └─ 两臂都非空：
       1. let cosine_top1 = vec[0].score.unwrap_or(0.0);
       2. if cosine_top1 >= cosine_threshold:
              跳 FTS；调 fuse_rrf(vec![] ⊕ vec, k, w)；
              verdict.skipped_fts = true、vec_top1_cosine = cosine_top1、cosine_threshold = threshold
          else:
              不跳；调 fuse_rrf(fts ⊕ vec, k, w)；
              verdict.skipped_fts = false、vec_top1_cosine = cosine_top1、cosine_threshold = threshold
```

控制对照不变性：
- `cosine_threshold = 0.0` → 永远跳（≈纯 vec 控制；exact-name 桶因两臂完全重叠 HYBR_R 仍 1.0）
- `cosine_threshold = 1.01` → 永不跳（≡HYB 控制，与 A-4 spec §5 降级值同义；cosine ∈ [0,1] 物理上限）
- `vec.is_empty()` → empty-arm guard 不跳（保护单臂兜底）

### 4.4 数据流

**生产**：
```
agent harness → fanout_merge::run_fanout_merge_rrf(deps, query, …)
  ├─ fts_results = native arm
  ├─ vec_results = semantic arm（SearchResult.score = Some(cosine) 已就绪）
  ├─ (merged, verdict) = fuse_rrf_with_fts_routing(fts, vec, RRF_K,
  │                          weight_provider.semantic_weight(),
  │                          DEFAULT_COSINE_ROUTING_THRESHOLD)
  └─ FanoutOutcome { merged, route_verdict: Some(verdict) }
  // verdict.query_lang 仍为 Mixed 占位；若 BETA-15B-5 badge 要展示真实 query lang，
  // 后续 cycle 在 wiring 入口处覆写 verdict.query_lang = detect_lang(&query)
```

**评测**：
```
score_case(case, corpus, cosine_threshold, …)
  ├─ query_lang = detect_lang(&case.query)（元数据，填进 CaseScores.query_lang）
  ├─ fts_ids = fts_rank(...)
  ├─ vec_scored = vector_rank(...)  → Vec<(doc_id, cosine)>
  ├─ hybrid_routed_rank(corpus, &fts_ids, &vec_scored, cosine_threshold, w, k)
  │     │
  │     └─ to_results(corpus, &vec_scored, source=SemanticIndex, mt=Semantic) 内部：
  │           对每 (id, cosine)：
  │             name = corpus.iter().find(|d| d.doc_id == id).map(|d| d.title.clone()).unwrap_or(id)
  │             path = PathBuf::from(id)（dedup key 不变）
  │             score = Some(f64::from(cosine))
  │           FTS 臂仍走 to_results_no_score(corpus, &fts_ids, source=NativeIndex, mt=Native)
  │           （或 to_results 拼 score=None 走第二条路径）
  │     └─ fuse_rrf_with_fts_routing(fts_results, vec_results, k, w, cosine_threshold)
  └─ nDCG / report 写入；report 在 wrapper 返 verdict 后覆写 verdict.query_lang = query_lang
```

**单测**：直接传 `cosine_threshold: f64`（0.0 / 0.5 / 1.01）、构造 `vec[0].score = Some(cosine)` 验证三态分支（empty-arm / cosine≥threshold-跳 / cosine<threshold-不跳）。

**CLI**：`semantic_quality --cosine-threshold <F>` 暴露 bake 旋钮；query_lang 仍从 case.query 检测填元数据、不接 CLI flag。

## 5. 调优工作流（spec 阶段执行）

```bash
# 步骤 0：分支上落 wrapper 签名升级 + RouteVerdict 字段升级 + 评测改造 + binary flag
# （task 1–6 全 TDD、每 task 验证门 = fmt + clippy + test，含 v0.5 byte-equal 闸门）

# 步骤 1：跑 sweep（用现有 vectors.json 缓存，零模型推理；W 固定 10.0；top-K 复用 10）
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --cosine-threshold $T \
    >> sweep-cosine.log
done
# 控制对照：T=0.0 应使 HYBR ≈ VEC（exact-name 因两臂完全重叠仍 1.0）；T=1.01 应使 HYBR ≡ HYB

# 步骤 2：人工读 sweep-cosine.log，选满足以下四条的 T*：
#   ① exact-name HYBR_R = 1.000（不满足 = bug）
#   ② 各桶 HYBR_N ≥ HYB baseline 同桶（不退步硬红线）
#   ③ OVERALL HYBR_N 最大化（优先 > 0.854）
#   ④ crosslang HYBR_N 最大化（优先 > 0.649）
# 若 ② 与 ③/④ 冲突（如 A-3/A-4 content-not-name 桶反伤重现）→ spec §5 字面正解：
#   取最保守值 T = 1.01（路由本 cycle 不生效，HYBR ≡ HYB），归 A-6 候选 ③/④

# 步骤 3：bake T* 到 DEFAULT_COSINE_ROUTING_THRESHOLD
# 步骤 4：rewrite baseline.json（HYBR 字段新水位 + cosine_threshold_bake 元数据；HYB 字段保留）
# 步骤 5：跑回归门验证
cargo test -p locifind-evals --test semantic_quality_gate
# 步骤 6：baseline 报告追加 A-5 调优记录节（sweep 全表 + bake 路径 + 诚实边界 + 下 cycle 抓手）
```

## 6. 代码改动清单（按 plan task 颗粒度）

| Task | 文件 | 改动 |
|---|---|---|
| T1 | `result-normalizer/src/lib.rs` | 删 `jaccard_overlap_by_path` 函数体 + 5 单测（保留模块路径无需占位） |
| T2 | `result-normalizer/src/lang.rs` | 删 `detect_lang` 函数体 + 8 单测；保留 `Lang` enum + `Lang::Mixed` Default 占位（若需）|
| T3 | `result-normalizer/src/lib.rs` | wrapper 签名 6→5 参；RouteVerdict 字段升级（cross_lang_hits → vec_top1_cosine、max_cross_lang_hits → cosine_threshold、query_lang 保留作元数据默认 Mixed）；DEFAULT_COSINE_ROUTING_THRESHOLD 占位（task 8 bake）；wrapper 内部决策改 cosine；A-4 既有 6 个 wrapper 单测改 cosine 信号（empty-arm 保留、Mixed-不跳删、cosine ≥ / cosine < / 边界 = 等 ≥ 4 单测）|
| T4 | `evals/src/semantic_quality/arms.rs` | `vector_rank` 返 `Vec<(String, f32)>`（doc_id + cosine）；`to_results` 改 fn 签名接 `Vec<(String, Option<f32>)>` 或加 helper `to_results_with_score`；`hybrid_routed_rank` 签名 `(query_lang, max_cross_lang_hits)` → `cosine_threshold: f64`；既有 5 个 hybrid_routed_* 单测改 cosine 信号 |
| T5 | `evals/src/semantic_quality/report.rs` | `score_case` 入口仍调 `detect_lang(&case.query)` 填 `CaseScores.query_lang` 元数据（不传 wrapper、wrapper 后置覆写 verdict.query_lang）；签名 `max_cross_lang_hits` → `cosine_threshold` |
| T6 | `evals/src/bin/semantic_quality.rs` | CLI flag `--max-cross-lang-hits` → `--cosine-threshold`（f64、默认 1.01）；表格 HYBR_R/HYBR_N 列保留 |
| T7 | （手动 sweep） | 步骤 1–2，产出 sweep-cosine.log |
| T8 | `result-normalizer/src/lib.rs` + `baseline.json` | bake `DEFAULT_COSINE_ROUTING_THRESHOLD = T*`；rewrite baseline.json HYBR 字段 + `cosine_threshold_bake` 元数据替换 `max_cross_lang_hits_bake` |
| T9 | `harness/src/fanout_merge.rs` + `apps/desktop/src-tauri/src/search.rs` + `apps/desktop/src-tauri/src/search/fanout.rs` | `run_fanout_merge_rrf` 入口移除 `detect_lang(query)` 调用、wrapper 调用改新签名传 `DEFAULT_COSINE_ROUTING_THRESHOLD`；`query: &str` 签名保留（评测仍要、且若未来 verdict.query_lang 要填真值仍需）；call-site 透传不动；新增单测覆盖 wrapper 三态分支（empty-arm / cosine 高跳 / cosine 低不跳）|
| T10 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 A-5 调优记录节 + 总验收过 |

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `cargo test --workspace`
- 必要 task 追加：
  - T3 / T8：evals parser-only byte-equal v0.5=473 / v0.9=877 不变（规范化逐 case 比对，[[project-evals-reporter-nondeterministic]]）
  - T8：`cargo test -p locifind-evals --test semantic_quality_gate` 用新 baseline pass
  - T9：fanout 单测覆盖 wrapper 三态（empty-arm / cosine 高跳 / cosine 低不跳）
  - T10：baseline 报告 A-5 节完整含 sweep 全表 + bake 路径 + 诚实边界

## 8. 风险与已记忆教训

### 8.1 风险

| 风险 | 缓解 |
|---|---|
| **cosine 信号同样撞 (4b) 天花板**（content-not-name 桶 FTS_N=0.930 > VEC_N=0.833，cosine 高的 case 即使在 content-not-name 桶也可能错跳） | spec §5 降级路径已留：取最保守 T=1.01、路由不生效、HYBR ≡ HYB；归下 cycle 抓手（A-6 候选 ③ 更大 embedding 模型 / ④ 评测集扩量）；不强行 bake 越红线值 |
| **vec[0].score == None**（不应发生但兜底） | `.unwrap_or(0.0)` 退化为不跳（cosine_top1=0 < 任意有效阈值）；单测覆盖 |
| **评测层 SearchResult.score 透传可能破坏 fuse_rrf 内部 dedup**（path 仍是 key、score 不影响 dedup） | 既有 `fuse_rrf` 单测全跑过即不破坏；T4 验证门必加 |
| **byte-equal 风险 v0.5/v0.9 退步** | 本 cycle 完全不动 parser、coverage、不动 model fallback；只动 result-normalizer wrapper + evals 设施 + fanout 入口；byte-equal 自然保持 |
| **wrapper 5 参签名比 A-4 6 参少了 query_lang 输入，但 RouteVerdict.query_lang 字段保留为占位 Mixed** | 文档明示「wrapper 不知道 query_lang 真实值、默认 Mixed 占位、调用侧后置覆写」；若未来 badge 要展示真值，wiring 显式覆写 verdict.query_lang = detect_lang(&query) |
| **删 A-3 jaccard / A-4 detect_lang 函数体后下 cycle 要做组合信号须重写** | YAGNI 防御不养 dead code；git history 可恢复原实现；下 cycle 评估再决定 |
| **`hybrid_routed_*` baseline 字段名重用为 cosine 路由含义改变** | A-4 N\*=usize::MAX 时 HYBR ≡ HYB，业务上字段「亲路由但未生效」；A-5 重定义为 cosine 路由直接 rewrite 字段值、`cosine_threshold_bake` 元数据替换 `max_cross_lang_hits_bake` 元数据；HYB 字段保持不动；gate 红线动态读 baseline 不动 |

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 coverage，不触发
- [[project-evals-reporter-nondeterministic]]：T3/T8 byte-equal 闸门用规范化逐 case 比对，不裸 diff JSON
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告，三件套全做
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring，不触发
- [[project-rrf-weight-tuning-ceiling]]：A-2 RRF weight 已到天花板，本 cycle 不调 W=10.0，专注路由信号
- [[project-g15-ambiguity-disambig]] / [[project-pull-full-distribution-before-convention-call]]：spec §5 降级 vs 强 bake 决策依据「sweep 数据全表」而非「单 case 直觉」

## 9. 链接

- A-4 spec：[2026-06-23-beta-15b-3a4-lang-routing-design.md](./2026-06-23-beta-15b-3a4-lang-routing-design.md)
- A-3 spec：[2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md](./2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md)
- A-2 spec：[2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md](./2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)（本 cycle T10 追加 A-5 节）
- 项目状态：[../../../STATUS.md](../../../STATUS.md) §「下一步」候选 ② VEC top-1 cosine 绝对分数阈值
