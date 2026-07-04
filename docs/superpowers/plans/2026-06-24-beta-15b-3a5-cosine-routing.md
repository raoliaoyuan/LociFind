# BETA-15B-3 A-5：VEC top-1 cosine 绝对分数阈值路由 实施 plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** wrapper `fuse_rrf_with_fts_routing` 内部信号从 A-4 的 lang 升级为 VEC top-1 cosine 绝对分数阈值；sweep + bake `DEFAULT_COSINE_ROUTING_THRESHOLD`，对准 A-3/A-4 暴露的 crosslang/content-not-name 不对称失败模式。

**Architecture:** 保留 A-3/A-4 wrapper API 名 / RouteVerdict 结构 / HYBR baseline 字段名 / gate 红线架构；wrapper 签名 6 参 → 5 参（删 `query_lang` + `max_cross_lang_hits`、加 `cosine_threshold: f64`）；信号源 `vec[0].score.unwrap_or(0.0)`（生产侧已是 cosine、评测层改造透传）；动作 `cosine_top1 >= threshold` 跳 FTS。删 `jaccard_overlap_by_path` 函数 + 5 单测（A-3 遗产无消费者）；保留 `detect_lang` + 8 单测（wiring 后置覆写 `verdict.query_lang` 元数据仍需）。

**Tech Stack:** Rust 2024 edition、result-normalizer crate、locifind-evals semantic_quality 模块、harness fanout_merge、cargo workspace。

**关键事实清单**（写 plan 时核对实情）：
- A-4 当前 `DEFAULT_MAX_CROSS_LANG_HITS = usize::MAX` (`packages/result-normalizer/src/lib.rs:105`)
- A-4 wrapper 6 参：`fuse_rrf_with_fts_routing(fts_list, vec_list, rrf_k, semantic_weight, query_lang, max_cross_lang_hits)`
- A-4 RouteVerdict 4 字段：`skipped_fts / query_lang / cross_lang_hits / max_cross_lang_hits`
- A-4 jaccard 5 单测：`jaccard_identical_top_k_is_one / jaccard_disjoint_top_k_is_zero / jaccard_half_overlap_is_one_third / jaccard_both_empty_is_zero / jaccard_top_k_truncates_inputs`
- A-4 wrapper 6 单测：`wrapper_en_query_high_cross_lang_skips_fts / wrapper_en_query_low_cross_lang_does_not_skip / wrapper_mixed_query_never_skips / wrapper_empty_arm_does_not_skip / wrapper_max_usize_never_skips / wrapper_max_zero_always_skips_when_any_cross_lang`
- A-4 arms `vector_rank → Vec<String>`、`to_results` 内部 `score: None`、`hybrid_routed_rank` 7 参签名 `(corpus, fts, vec_ids, query_lang, max_cross_lang_hits, semantic_weight, k)`
- A-4 arms 4 hybrid_routed 单测：`hybrid_routed_en_no_cross_lang_uses_both_arms / hybrid_routed_en_cross_lang_skips_fts / hybrid_routed_max_usize_never_skips / hybrid_routed_both_empty_returns_empty`
- A-4 report `score_case` 8 参签名（第 7 = `max_cross_lang_hits: usize`）、入口 `detect_lang(&case.query)` 填 query_lang 传 hybrid_routed_rank
- A-4 CLI 4 单测：`semantic_weight_flag_parses / semantic_weight_defaults_to_const / max_cross_lang_hits_flag_parses / max_cross_lang_hits_defaults_to_const`
- A-4 harness fanout 3 单测：`fanout_rrf_en_query_with_zh_vec_hits_skips_fts / fanout_rrf_zh_query_no_skip_when_pure_zh_corpus / fanout_rrf_mixed_query_never_skips`
- A-4 desktop wiring 已有 `raw_query: &'a str` 字段（`apps/desktop/src-tauri/src/search.rs:589-591` + `search/fanout.rs:42,76,82`）—— A-5 不需改 desktop 层
- A-4 baseline.json 6 桶（synonym / concept / crosslang / content-not-name / exact-name / OVERALL），每条 8 数值字段（`fts/vec/hybrid/hybrid_routed × recall/ndcg`）；**无元数据字段**——A-5 rewrite 只动数值
- A-4 `semantic_quality_gate.rs` 已动态读 baseline.json + import `DEFAULT_MAX_CROSS_LANG_HITS`——A-5 改 import 名 + score_case 第 7 参数即可

---

## Task 1: 删 jaccard_overlap_by_path 函数 + 5 单测（A-3 遗产清理）

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`

- [ ] **Step 1: 失败前置 = 确认现有单测在删除前全过**

Run: `cargo test -p locifind-result-normalizer --lib jaccard 2>&1 | grep "test result:" | head -5`
Expected: `test result: ok. 5 passed; 0 failed; ...`

- [ ] **Step 2: 删除 `jaccard_overlap_by_path` 函数**

`packages/result-normalizer/src/lib.rs` 中**删除整段 line 170–191**（包含 doc 注释 + 函数体）：

```rust
/// 计算两个有序 `SearchResult` 列表 **top-K 的 `path` 集合** Jaccard 重叠度。
///
/// `|A ∩ B| / |A ∪ B|`，A/B = 各自前 `k` 个 result 的 path 集合；空集 ∪ 空集 = 0.0。
///
/// BETA-15B-3 A-3 引入；A-4 wrapper 改为 lang 信号后此函数留作下 cycle 信号组合预留
/// （Jaccard + lang OR/AND 复合）；仍由 `result-normalizer` 公开 API。
#[must_use]
pub fn jaccard_overlap_by_path(a: &[SearchResult], b: &[SearchResult], k: usize) -> f64 {
    let set_a: HashSet<&Path> = a.iter().take(k).map(|r| r.path.as_path()).collect();
    let set_b: HashSet<&Path> = b.iter().take(k).map(|r| r.path.as_path()).collect();
    let inter = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        // k ≤ ~20（top-K 调用语境），inter/union ≤ k，远小于 f64 精确整数上限 2^53；cast 无损。
        #[allow(clippy::cast_precision_loss)]
        {
            inter as f64 / union as f64
        }
    }
}
```

整段不留任何占位。

- [ ] **Step 3: 删除 5 个 jaccard 单测**

`packages/result-normalizer/src/lib.rs` 中**删除以下 5 个单测**（位于 `#[cfg(test)] mod tests {}` 内）：

```rust
#[test]
fn jaccard_identical_top_k_is_one() { /* ... */ }

#[test]
fn jaccard_disjoint_top_k_is_zero() { /* ... */ }

#[test]
fn jaccard_half_overlap_is_one_third() { /* ... */ }

#[test]
fn jaccard_both_empty_is_zero() { /* ... */ }

#[test]
fn jaccard_top_k_truncates_inputs() { /* ... */ }
```

删除时连带函数体、doc 注释、空行整段去除。

- [ ] **Step 4: 清理无用 use（可能 `std::path::Path` 不再被 lib.rs 顶层使用）**

检查 `packages/result-normalizer/src/lib.rs` 顶部 `use std::path::{Path, PathBuf};`：
- 如果 `Path`（不含 `PathBuf`）在剩余代码中仍被使用 → 不动
- 如果只有 `PathBuf` 被使用 → 改为 `use std::path::PathBuf;`

判定方法：

Run: `grep -n "Path\b" packages/result-normalizer/src/lib.rs | grep -v "PathBuf"`
若结果为空 → 改 `use std::path::PathBuf;`；否则不动。

也检查 `HashSet` 是否仍被使用：

Run: `grep -n "HashSet" packages/result-normalizer/src/lib.rs`
若只出现在 use 行 → `use std::collections::{HashMap, HashSet};` 改为 `use std::collections::HashMap;`

- [ ] **Step 5: 验证编译 + 测试全过**

Run: `cargo test -p locifind-result-normalizer --lib 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`（N 应当 = 删除前总数 - 5）

Run: `cargo clippy -p locifind-result-normalizer --lib --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 净（无 warning、无 error）

Run: `cargo fmt --all --check`
Expected: 净（无输出）

- [ ] **Step 6: 提交**

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-5 task 1：删 jaccard_overlap_by_path 函数 + 5 单测（A-3 遗产清理；A-5 转 cosine 信号、wrapper 不再用 Jaccard、无消费者）"
```

---

## Task 2: lang.rs 升 A-5 doc（保留 detect_lang + 8 单测、新含义 = wiring 后置覆写元数据）

**Files:**
- Modify: `packages/result-normalizer/src/lang.rs`

A-5 不删 `detect_lang`：评测层不再调用驱动路由（wrapper 5 参不接 query_lang），但生产 wiring 层 `harness/fanout_merge::run_fanout_merge_rrf` 仍调 `detect_lang(query)` 在 wrapper 返回后**覆写** `verdict.query_lang` 字段填真值（供 BETA-15B-5 badge 槽位消费）。本 task 只升 doc 注释明确这一新角色。

- [ ] **Step 1: 改 lang.rs 顶层 doc 注释**

`packages/result-normalizer/src/lang.rs` line 1-2 当前：

```rust
//! query 语种检测（CJK ratio 三态二阈、纯 std、零依赖）。
//! BETA-15B-3 A-4：替代 A-3 Jaccard 单维信号，对准 crosslang 桶失败模式。
```

改为：

```rust
//! query 语种检测（CJK ratio 三态二阈、纯 std、零依赖）。
//! BETA-15B-3 A-4 引入作 wrapper 路由信号；A-5 路由信号换 cosine 后，本函数仅供
//! 生产 wiring（`harness::fanout_merge::run_fanout_merge_rrf`）后置覆写
//! `RouteVerdict.query_lang` 字段填可观测元数据（供 BETA-15B-5 badge 槽位消费）。
```

- [ ] **Step 2: 改 Lang enum doc 注释**

`packages/result-normalizer/src/lang.rs` line 4 当前：

```rust
/// query 语种三态。`Mixed` 进保守降级（路由不生效）。
```

改为：

```rust
/// query 语种三态。A-5 起仅作 `RouteVerdict.query_lang` 可观测元数据；不再驱动路由动作。
```

- [ ] **Step 3: 验证编译 + 测试全过**

Run: `cargo test -p locifind-result-normalizer --lib lang 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. 8 passed; 0 failed; ...`（保持 8 个 detect_lang 单测全过）

Run: `cargo fmt --all --check`
Expected: 净

- [ ] **Step 4: 提交**

```bash
git add packages/result-normalizer/src/lang.rs
git commit -m "BETA-15B-3 A-5 task 2：lang.rs doc 升 A-5 新角色（A-4 路由信号 → A-5 wiring 后置覆写 verdict.query_lang 元数据；detect_lang + Lang enum + 8 单测保留）"
```

---

## Task 3: wrapper 签名 6→5 参 + RouteVerdict 字段升级 + 改造 6 单测（A-5 核心实现）

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`

本 task 是 A-5 的核心。把 wrapper `fuse_rrf_with_fts_routing` 签名从 6 参（含 `query_lang + max_cross_lang_hits`）改成 5 参（含 `cosine_threshold: f64`）；RouteVerdict 字段从 `cross_lang_hits + max_cross_lang_hits` 改成 `vec_top1_cosine + cosine_threshold`、**保留 `query_lang` 字段**（默认 `Lang::Mixed` 占位、wiring 后置覆写）；wrapper 内部决策树改用 `cosine_top1 = vec[0].score.unwrap_or(0.0) >= cosine_threshold` 触发跳 FTS；常量 `DEFAULT_MAX_CROSS_LANG_HITS` → `DEFAULT_COSINE_ROUTING_THRESHOLD`（task 8 sweep 后 bake、本 task 占位 = 1.01 spec §5 降级值，使本 cycle 既有路径继续 byte-equal）。

- [ ] **Step 1: 写失败测试 = wrapper 高 cosine 跳 FTS 单测**

在 `packages/result-normalizer/src/lib.rs` 的 `#[cfg(test)] mod tests {}` 内，**替换** `wrapper_en_query_high_cross_lang_skips_fts` 单测为：

```rust
#[test]
fn wrapper_high_cosine_skips_fts() {
    // vec[0].score >= threshold → 跳 FTS
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let mut vec_arm = vec![
        result(
            "/vec_top.md",
            BackendKind::SemanticIndex,
            MatchType::Semantic,
        ),
        result(
            "/vec_second.md",
            BackendKind::SemanticIndex,
            MatchType::Semantic,
        ),
    ];
    vec_arm[0].score = Some(0.85);
    vec_arm[1].score = Some(0.40);
    let (out, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 0.80);
    assert!(v.skipped_fts);
    assert!((v.vec_top1_cosine - 0.85).abs() < f64::EPSILON);
    assert!((v.cosine_threshold - 0.80).abs() < f64::EPSILON);
    assert_eq!(out.len(), 2);
    assert!(out
        .iter()
        .all(|m| m.result.path.to_string_lossy().contains(".md")));
}
```

- [ ] **Step 2: 跑测试验证编译失败 / 测试失败（红）**

Run: `cargo test -p locifind-result-normalizer --lib wrapper_high_cosine_skips_fts 2>&1 | tail -20`
Expected: 编译错误，提示 `fuse_rrf_with_fts_routing` 旧签名期 6 参 / `RouteVerdict.vec_top1_cosine` 字段不存在 / `RouteVerdict.cosine_threshold` 字段不存在等。

- [ ] **Step 3: 升级常量、删 A-4 旧常量**

`packages/result-normalizer/src/lib.rs` line 97-105 当前：

```rust
/// FTS/VEC top-K 跨语种 vec hit 计数路由的默认阈值（≥ 此 count 时跳过 FTS）。
/// BETA-15B-3 A-4 sweep 选定 **N\* = `usize::MAX`**（spec §5 字面正解保守降级）：
/// content-not-name 桶 `FTS_N`=0.930 > `VEC_N`=0.833，任一有限 N（实测 0/1/2/3/5）
/// 都让此桶 `HYBR_N` 跌破 HYB baseline（0.833–0.887 < 0.930），破 (4b) 红线；
/// `usize::MAX` 等价路由本 cycle 不生效、HYBR ≡ HYB。
/// **下 cycle 抓手 = 升级路由信号**（② VEC top-1 cosine 绝对分数阈值 /
/// ③ 更大 embedding 模型 / ④ 评测集扩量 + 重构 content-not-name 桶）。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-4 调优记录节。
pub const DEFAULT_MAX_CROSS_LANG_HITS: usize = usize::MAX;
```

改为：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 引入替代 A-4 的 lang 信号。task 8 sweep 后 bake；
/// 本 task 占位 = 1.01（cosine ∈ [0,1] 物理上限、永不跳、HYBR ≡ HYB）使既有路径 byte-equal。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 1.01;
```

- [ ] **Step 4: 升级 RouteVerdict 字段**

`packages/result-normalizer/src/lib.rs` line 193-205 当前：

```rust
/// 路由判定副产物，便于评测/badge/调试消费。
/// 已透传到 `FanoutOutcome.route_verdict`，作 BETA-15B-5 可解释 v1 badge 槽位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    /// 是否跳过 FTS 臂（true = `cross_lang_hits` ≥ max、hybrid 退化为纯向量）。
    pub skipped_fts: bool,
    /// 检测到的 query 语种。
    pub query_lang: crate::lang::Lang,
    /// vec top-K 中检测到的跨语种 hit 计数（`query_lang` ≠ name 检测出的 lang 且 ≠ Mixed）。
    pub cross_lang_hits: usize,
    /// 当时使用的阈值（便于事后审计）。
    pub max_cross_lang_hits: usize,
}
```

改为：

```rust
/// 路由判定副产物，便于评测/badge/调试消费。
/// 已透传到 `FanoutOutcome.route_verdict`，作 BETA-15B-5 可解释 v1 badge 槽位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    /// 是否跳过 FTS 臂（true = `vec_top1_cosine` ≥ `cosine_threshold`、hybrid 退化为纯向量）。
    pub skipped_fts: bool,
    /// 检测到的 query 语种（A-5 起仅作可观测元数据；wrapper 内部默认 `Mixed` 占位、
    /// wiring 后置覆写填真值；不驱动路由动作）。
    pub query_lang: crate::lang::Lang,
    /// VEC top-1 cosine 实测值（`vec[0].score.unwrap_or(0.0)`、f64 升精度）。
    pub vec_top1_cosine: f64,
    /// 当时使用的阈值（便于事后审计）。
    pub cosine_threshold: f64,
}
```

- [ ] **Step 5: 升级 wrapper 签名 + 内部决策树**

`packages/result-normalizer/src/lib.rs` line 207-280 当前 6 参 wrapper 整段：

```rust
/// 加路由的 RRF 融合 wrapper：FTS/VEC 两臂分别传入,
/// query 单语种且 vec top-K 含跨语种 hit ≥ `max_cross_lang_hits` 时跳过 FTS 臂
/// （hybrid 退化为纯向量）。`query_lang` = Mixed 进保守降级永不跳。
/// ...（整段，省略）
#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts_list: Vec<SearchResult>,
    vec_list: Vec<SearchResult>,
    rrf_k: f64,
    semantic_weight: f64,
    query_lang: crate::lang::Lang,
    max_cross_lang_hits: usize,
) -> (Vec<MergedResult>, RouteVerdict) {
    // ...（整段决策树，省略）
}
```

整段**替换为**：

```rust
/// 加路由的 RRF 融合 wrapper：FTS/VEC 两臂分别传入,
/// VEC top-1 cosine（`vec[0].score`）≥ `cosine_threshold` 时跳过 FTS 臂
/// （hybrid 退化为纯向量）。
///
/// `fuse_rrf` 本身不动；wrapper 只决定 `fts_list` 是否进入 N 列表融合。
///
/// **任一臂空时不跳过 FTS**：无路由信号，保留兜底；`skipped_fts = false`、`vec_top1_cosine = 0.0`。
///
/// **vec[0].score == None**（不应发生但兜底）：`unwrap_or(0.0)` → 退化为不跳。
///
/// **`query_lang` 默认填 `Lang::Mixed` 占位**：wrapper 不知道 query 真值；
/// 评测层不消费、生产 wiring 在 wrapper 返回后用 struct-update 覆写填真值。
///
/// 与 [`fuse_rrf`] 等价性：当不跳 FTS 时，wrapper 结果完全等价 `fuse_rrf(vec![fts, vec], k, weight)`。
#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts_list: Vec<SearchResult>,
    vec_list: Vec<SearchResult>,
    rrf_k: f64,
    semantic_weight: f64,
    cosine_threshold: f64,
) -> (Vec<MergedResult>, RouteVerdict) {
    // 任一臂空 → 无路由信号；不跳过 FTS（preserve 一臂兜底）。
    if fts_list.is_empty() || vec_list.is_empty() {
        let merged = fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight);
        return (
            merged,
            RouteVerdict {
                skipped_fts: false,
                query_lang: crate::lang::Lang::Mixed,
                vec_top1_cosine: 0.0,
                cosine_threshold,
            },
        );
    }

    // 两臂都非空：算 vec top-1 cosine、严格 ≥ 阈值时跳 FTS。
    let cosine_top1 = vec_list[0].score.unwrap_or(0.0);
    let skipped_fts = cosine_top1 >= cosine_threshold;

    let merged = if skipped_fts {
        fuse_rrf(vec![vec_list], rrf_k, semantic_weight)
    } else {
        fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight)
    };

    (
        merged,
        RouteVerdict {
            skipped_fts,
            query_lang: crate::lang::Lang::Mixed,
            vec_top1_cosine: cosine_top1,
            cosine_threshold,
        },
    )
}
```

- [ ] **Step 6: 跑 step 1 新单测验证通过（绿）**

Run: `cargo test -p locifind-result-normalizer --lib wrapper_high_cosine_skips_fts 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step 7: 改造剩余 5 wrapper 单测**

`packages/result-normalizer/src/lib.rs` 的 `#[cfg(test)] mod tests {}` 内（A-4 6 个 wrapper 单测、step 1 已替换 1 个，剩 5 个）：

**替换** `wrapper_en_query_low_cross_lang_does_not_skip` 为：

```rust
#[test]
fn wrapper_low_cosine_does_not_skip() {
    // vec[0].score < threshold → 不跳
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let mut vec_arm = vec![
        result(
            "/policy.md",
            BackendKind::SemanticIndex,
            MatchType::Semantic,
        ),
        result("/leave.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    vec_arm[0].score = Some(0.40);
    vec_arm[1].score = Some(0.30);
    let (out, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 0.80);
    assert!(!v.skipped_fts);
    assert!((v.vec_top1_cosine - 0.40).abs() < f64::EPSILON);
    assert!(out
        .iter()
        .any(|m| m.result.path.to_string_lossy().contains("en.txt")));
}
```

**删除** `wrapper_mixed_query_never_skips`（cosine 信号无 Mixed 概念、独立场景已被 wrapper_empty_arm 覆盖）。

**保留** `wrapper_empty_arm_does_not_skip` 但更新签名调用（从 6 参改 4 参）：

```rust
#[test]
fn wrapper_empty_arm_does_not_skip() {
    // vec 空 → empty-arm guard → 不跳、fuse_rrf 兜底
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let vec_arm: Vec<SearchResult> = vec![];
    let (out, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 0.50);
    assert!(!v.skipped_fts);
    assert!((v.vec_top1_cosine - 0.0).abs() < f64::EPSILON);
    assert_eq!(v.query_lang, crate::lang::Lang::Mixed); // empty 时默认 Mixed 占位
    assert_eq!(out.len(), 1);
}
```

**替换** `wrapper_max_usize_never_skips` 为：

```rust
#[test]
fn wrapper_threshold_above_one_never_skips() {
    // threshold = 1.01 > cosine ∈ [0,1] 上限 → 永不跳（spec §5 降级值）
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let mut vec_arm = vec![result(
        "/vec_top.md",
        BackendKind::SemanticIndex,
        MatchType::Semantic,
    )];
    vec_arm[0].score = Some(0.99); // 极高 cosine
    let (_, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 1.01);
    assert!(!v.skipped_fts);
}
```

**替换** `wrapper_max_zero_always_skips_when_any_cross_lang` 为：

```rust
#[test]
fn wrapper_threshold_zero_always_skips() {
    // threshold = 0.0 → 任意 cosine ≥ 0 → 永远跳（≈纯 vec 控制）
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let mut vec_arm = vec![result(
        "/vec_top.md",
        BackendKind::SemanticIndex,
        MatchType::Semantic,
    )];
    vec_arm[0].score = Some(0.10);
    let (out, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 0.0);
    assert!(v.skipped_fts);
    assert!(out
        .iter()
        .all(|m| !m.result.path.to_string_lossy().contains("en.txt")));
}
```

**新增** `wrapper_no_score_treated_as_zero`（vec[0].score=None 兜底）：

```rust
#[test]
fn wrapper_no_score_treated_as_zero() {
    // vec[0].score = None → unwrap_or(0.0) → cosine_top1 = 0 → 不跳（除非 threshold ≤ 0）
    let fts = vec![result(
        "/en.txt",
        BackendKind::NativeIndex,
        MatchType::Content,
    )];
    let vec_arm = vec![result(
        "/vec_top.md",
        BackendKind::SemanticIndex,
        MatchType::Semantic,
    )]; // score: None default
    let (out, v) = fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, 0.50);
    assert!(!v.skipped_fts);
    assert!((v.vec_top1_cosine - 0.0).abs() < f64::EPSILON);
    assert_eq!(out.len(), 2);
}
```

- [ ] **Step 8: 检查 `use crate::lang::Lang;` 是否仍被需要**

`packages/result-normalizer/src/lib.rs` line 495 当前 `use crate::lang::Lang;` 在 `#[cfg(test)] mod tests {}` 内。
A-5 wrapper_empty_arm 单测仍引用 `crate::lang::Lang::Mixed`（step 7）——保留此 use。

- [ ] **Step 9: 验证全过**

Run: `cargo test -p locifind-result-normalizer --lib 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`（N = 删除 5 jaccard + 删除 1 mixed 单测 + 新加 1 no_score 单测的总数）

Run: `cargo clippy -p locifind-result-normalizer --lib --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 净

Run: `cargo fmt --all --check`
Expected: 净

- [ ] **Step 10: 验证 workspace 编译**（wrapper 签名变更可能破其他 crate）

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: 编译错误（harness/fanout_merge.rs + evals/semantic_quality 仍调旧签名）—— **本 task 不修这些下游、留 task 4/5/6/8/9 修**。

为本 task 验证：cargo build -p locifind-result-normalizer 净即可：

Run: `cargo build -p locifind-result-normalizer 2>&1 | tail -5`
Expected: `Finished ... target(s) in N.NNs`

- [ ] **Step 11: 提交**

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-5 task 3：wrapper 签名 6→5 参 + RouteVerdict 字段升级 + DEFAULT_COSINE_ROUTING_THRESHOLD 占位 1.01 + 6 单测改造（A-5 核心实现）"
```

---

## Task 4: evals arms.rs 改造（vector_rank 返 tuple + to_results 挂 score + hybrid_routed_rank 5 参）

**Files:**
- Modify: `packages/evals/src/semantic_quality/arms.rs`

本 task 改 evals 评测层：`vector_rank` 返 `Vec<(String, f32)>`（doc_id + cosine）让 cosine 透传；`to_results` 加 `score` 参数挂 `SearchResult.score`；`hybrid_routed_rank` 签名从 7 参（含 `query_lang + max_cross_lang_hits`）改成 6 参（含 `cosine_threshold: f64`）；改造现有 4 个 hybrid_routed 单测。

- [ ] **Step 1: 写失败测试 = vector_rank 返 tuple**

在 `packages/evals/src/semantic_quality/arms.rs` 的 `#[cfg(test)] mod tests {}` 内，**替换** `vector_ranks_by_cosine_and_applies_floor` 单测为：

```rust
#[test]
fn vector_ranks_by_cosine_and_applies_floor_with_scores() {
    use std::collections::BTreeMap;
    let mut docs: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    docs.insert("near".into(), vec![1.0, 0.0]);
    docs.insert("mid".into(), vec![0.6, 0.8]);
    docs.insert("far".into(), vec![0.0, 1.0]);
    let q = vec![1.0_f32, 0.0];
    let ranked = vector_rank(&q, &docs, 0.30, 10);
    // 返 (doc_id, cosine) tuple、降序、地板过滤；近 cosine≈1.0、中 cosine≈0.6、远 cosine=0.0 < 0.30 被过滤
    assert_eq!(ranked.len(), 2);
    assert_eq!(ranked[0].0, "near");
    assert!((ranked[0].1 - 1.0).abs() < 1e-5);
    assert_eq!(ranked[1].0, "mid");
    assert!((ranked[1].1 - 0.6).abs() < 1e-5);
}
```

- [ ] **Step 2: 跑测试验证编译失败（红）**

Run: `cargo test -p locifind-evals --lib vector_ranks_by_cosine_and_applies_floor_with_scores 2>&1 | tail -15`
Expected: 编译错误 / 类型不匹配（`vector_rank` 当前返 `Vec<String>` 不是 `Vec<(String, f32)>`）

- [ ] **Step 3: 升级 `vector_rank` 签名**

`packages/evals/src/semantic_quality/arms.rs` line 62-81 当前：

```rust
/// 向量臂：query 向量 vs 全 doc 向量 cosine → 过滤 `>= floor` → 降序 → top `limit` 的 `doc_id`。
#[must_use]
pub fn vector_rank(
    query_vec: &[f32],
    doc_vectors: &BTreeMap<String, Vec<f32>>,
    floor: f32,
    limit: usize,
) -> Vec<String> {
    let mut scored: Vec<(f32, &str)> = doc_vectors
        .iter()
        .map(|(id, v)| (cosine(query_vec, v), id.as_str()))
        .filter(|(s, _)| *s >= floor)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, id)| id.to_owned())
        .collect()
}
```

改为：

```rust
/// 向量臂：query 向量 vs 全 doc 向量 cosine → 过滤 `>= floor` → 降序 → top `limit` 的 `(doc_id, cosine)`。
/// BETA-15B-3 A-5：返 tuple 让 cosine 透传给 `hybrid_routed_rank` 内 `to_results_with_scores`，
/// `SearchResult.score` 挂 cosine 后 wrapper 取 `vec[0].score` 作路由信号。
#[must_use]
pub fn vector_rank(
    query_vec: &[f32],
    doc_vectors: &BTreeMap<String, Vec<f32>>,
    floor: f32,
    limit: usize,
) -> Vec<(String, f32)> {
    let mut scored: Vec<(f32, &str)> = doc_vectors
        .iter()
        .map(|(id, v)| (cosine(query_vec, v), id.as_str()))
        .filter(|(s, _)| *s >= floor)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(s, id)| (id.to_owned(), s))
        .collect()
}
```

- [ ] **Step 4: 跑测试验证 step 1 单测过（绿）**

Run: `cargo test -p locifind-evals --lib vector_ranks_by_cosine_and_applies_floor_with_scores 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`

但其他单测会失败（`hybrid_rank` / `hybrid_routed_rank` / `score_case` 都消费 `vector_rank` 旧签名），先继续 step 5。

- [ ] **Step 5: 加 `to_results_with_scores` helper（保留 to_results 给 FTS 臂）**

`packages/evals/src/semantic_quality/arms.rs` line 83-109 现 `to_results` 函数不动（FTS 臂仍需 `score: None`）。**新增**一个并行 helper：

在 `to_results` 函数下方加：

```rust
/// 把 `(doc_id, cosine)` 列表包装成生产 `SearchResult`：与 [`to_results`] 同款 path/name 规则，
/// 多挂 `score = Some(f64::from(cosine))`。BETA-15B-3 A-5：让 wrapper 内
/// `vec[0].score.unwrap_or(0.0)` 拿到真 cosine 作路由信号。
fn to_results_with_scores(
    corpus: &[SemanticDoc],
    scored: &[(String, f32)],
    source: BackendKind,
    mt: MatchType,
) -> Vec<SearchResult> {
    scored
        .iter()
        .map(|(id, s)| {
            let name = corpus
                .iter()
                .find(|d| d.doc_id == *id)
                .map_or_else(|| id.clone(), |d| d.title.clone());
            SearchResult {
                id: id.clone(),
                path: PathBuf::from(id),
                name,
                source,
                match_type: mt,
                score: Some(f64::from(*s)),
                metadata: SearchResultMetadata::default(),
            }
        })
        .collect()
}
```

- [ ] **Step 6: 升级 `hybrid_rank` 接受新 vector_rank 输出**

`packages/evals/src/semantic_quality/arms.rs` line 113-134 `hybrid_rank` 当前接 `vec_ids: &[String]`。改签名接 `vec_scored: &[(String, f32)]`、内部用 `to_results_with_scores`：

替换原 `hybrid_rank` 为：

```rust
/// hybrid 臂：FTS 臂(`NativeIndex`) + 向量臂(`SemanticIndex`) 喂**生产** `fuse_rrf`,
/// 取融合后有序 `doc_id`。`semantic_weight`/`k` 即生产融合的两个调优旋钮。
#[must_use]
pub fn hybrid_rank(
    corpus: &[SemanticDoc],
    fts: &[String],
    vec_scored: &[(String, f32)],
    semantic_weight: f64,
    k: f64,
) -> Vec<String> {
    let lists = vec![
        to_results(corpus, fts, BackendKind::NativeIndex, MatchType::Content),
        to_results_with_scores(
            corpus,
            vec_scored,
            BackendKind::SemanticIndex,
            MatchType::Semantic,
        ),
    ];
    fuse_rrf(lists, k, semantic_weight)
        .into_iter()
        .map(|m| m.result.id)
        .collect()
}
```

- [ ] **Step 7: 升级 `hybrid_routed_rank` 7 参 → 6 参 + cosine 信号**

`packages/evals/src/semantic_quality/arms.rs` line 136-173 `hybrid_routed_rank` 当前 7 参。整段替换为：

```rust
/// 加 cosine 路由的 hybrid 臂：VEC top-1 cosine ≥ `cosine_threshold` 时跳过 FTS
/// （hybrid 退化为纯向量）。喂 **生产 wrapper** `fuse_rrf_with_fts_routing`，
/// `semantic_weight`/`k`/`cosine_threshold` 即生产路由的三个旋钮。
///
/// 评测层不消费 `RouteVerdict.query_lang`（默认 Mixed 占位、wiring 后置覆写）。
#[must_use]
pub fn hybrid_routed_rank(
    corpus: &[SemanticDoc],
    fts: &[String],
    vec_scored: &[(String, f32)],
    cosine_threshold: f64,
    semantic_weight: f64,
    k: f64,
) -> Vec<String> {
    let fts_results = to_results(corpus, fts, BackendKind::NativeIndex, MatchType::Content);
    let vec_results = to_results_with_scores(
        corpus,
        vec_scored,
        BackendKind::SemanticIndex,
        MatchType::Semantic,
    );
    let (merged, _verdict) = locifind_result_normalizer::fuse_rrf_with_fts_routing(
        fts_results,
        vec_results,
        k,
        semantic_weight,
        cosine_threshold,
    );
    merged.into_iter().map(|m| m.result.id).collect()
}
```

注意签名顺序：`cosine_threshold` 放在 `semantic_weight` 前，延续 A-3/A-4 arms 层「(routing 旋钮, rrf 旋钮)」约定（与生产 wrapper 「(rrf 旋钮, routing 旋钮)」颠倒）。

- [ ] **Step 8: 删 lang use（arms.rs 不再消费 Lang）**

`packages/evals/src/semantic_quality/arms.rs` line 6 当前：

```rust
use locifind_result_normalizer::lang::Lang;
```

**删除此行**（A-5 arms 不再调 detect_lang / 不再传 Lang 给 wrapper）。

- [ ] **Step 9: 改造 4 个 hybrid_routed 单测**

`packages/evals/src/semantic_quality/arms.rs` 的 `#[cfg(test)] mod tests {}` 内：

**替换** `hybrid_routed_en_no_cross_lang_uses_both_arms` 为：

```rust
#[test]
fn hybrid_routed_low_cosine_uses_both_arms() {
    // vec[0].cosine < threshold → 不跳；hybrid 用两臂
    let corpus = vec![doc("d_a", "policy text"), doc("d_b", "leave guide")];
    let fts = vec!["d_a".to_owned()];
    let vec_scored = vec![("d_a".to_owned(), 0.40), ("d_b".to_owned(), 0.30)];
    let ranked = hybrid_routed_rank(&corpus, &fts, &vec_scored, 0.80, 10.0, 60.0);
    for id in ["d_a", "d_b"] {
        assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
    }
}
```

**替换** `hybrid_routed_en_cross_lang_skips_fts` 为：

```rust
#[test]
fn hybrid_routed_high_cosine_skips_fts() {
    // vec[0].cosine >= threshold → 跳 FTS；hybrid 退化为纯向量
    let corpus = vec![doc("d_fts", "fts only"), doc("d_vec", "semantic hit")];
    let fts = vec!["d_fts".to_owned()];
    let vec_scored = vec![("d_vec".to_owned(), 0.85)];
    let ranked = hybrid_routed_rank(&corpus, &fts, &vec_scored, 0.80, 10.0, 60.0);
    assert!(
        !ranked.contains(&"d_fts".to_owned()),
        "跳 FTS 后 d_fts 不在结果"
    );
    assert!(ranked.contains(&"d_vec".to_owned()));
}
```

**替换** `hybrid_routed_max_usize_never_skips` 为：

```rust
#[test]
fn hybrid_routed_threshold_above_one_never_skips() {
    // threshold = 1.01 > cosine 物理上限 → 永不跳（与 hybrid_rank 等价）
    let corpus = vec![doc("d_a", "policy"), doc("d_vec", "vector hit")];
    let fts = vec!["d_a".to_owned()];
    let vec_scored = vec![("d_vec".to_owned(), 0.99)];
    let routed = hybrid_routed_rank(&corpus, &fts, &vec_scored, 1.01, 10.0, 60.0);
    let direct = hybrid_rank(&corpus, &fts, &vec_scored, 10.0, 60.0);
    assert_eq!(routed, direct);
}
```

**替换** `hybrid_routed_both_empty_returns_empty` 为：

```rust
#[test]
fn hybrid_routed_both_empty_returns_empty() {
    let corpus = vec![];
    let ranked = hybrid_routed_rank(&corpus, &[], &[], 0.50, 10.0, 60.0);
    assert!(ranked.is_empty());
}
```

**改造** `hybrid_fuses_both_arms_and_weights_semantic`（hybrid_rank 单测，签名变更）：

```rust
#[test]
fn hybrid_fuses_both_arms_and_weights_semantic() {
    let corpus = vec![doc("d_both", "x"), doc("d_fts", "y"), doc("d_vec", "z")];
    let fts = vec!["d_both".to_owned(), "d_fts".to_owned()];
    let vec_scored = vec![("d_both".to_owned(), 0.85), ("d_vec".to_owned(), 0.70)];
    let ranked = hybrid_rank(&corpus, &fts, &vec_scored, 2.0, 60.0);
    assert_eq!(ranked.first().map(String::as_str), Some("d_both"));
    for id in ["d_both", "d_fts", "d_vec"] {
        assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
    }
}
```

- [ ] **Step 10: 验证 arms 模块全过**

Run: `cargo test -p locifind-evals --lib semantic_quality::arms 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`（fts 2 + vector 1 + hybrid 1 + hybrid_routed 4 = 8 个单测）

Run: `cargo build -p locifind-evals 2>&1 | tail -10`
Expected: 编译错误（report.rs 仍调旧签名）—— task 5 修。

- [ ] **Step 11: 提交**

```bash
git add packages/evals/src/semantic_quality/arms.rs
git commit -m "BETA-15B-3 A-5 task 4：evals arms 改造 vector_rank 返 (id, cosine) + to_results_with_scores helper + hybrid_routed_rank 6 参 cosine 信号 + 4 单测改造"
```

---

## Task 5: evals report.rs `score_case` 入口改 + 删 detect_lang 调用

**Files:**
- Modify: `packages/evals/src/semantic_quality/report.rs`

本 task 把 `score_case` 第 7 参 `max_cross_lang_hits: usize` 改成 `cosine_threshold: f64`、入口删 `detect_lang` 调用（评测层不消费 query_lang）、call_site `vector_rank` 接 tuple 返回、call_site `hybrid_routed_rank` 改 6 参签名；改造现有 `score_case_runs_four_arms` 单测。

- [ ] **Step 1: 写失败测试 = score_case 接 cosine_threshold**

`packages/evals/src/semantic_quality/report.rs` 的 `#[cfg(test)] mod tests {}` 内 **替换** `score_case_runs_four_arms` 为：

```rust
#[test]
fn score_case_runs_four_arms_with_cosine_routing() {
    let corpus = vec![
        SemanticDoc {
            doc_id: "d1".into(),
            lang: "zh".into(),
            title: "年假".into(),
            body: "年假和远程办公规定".into(),
        },
        SemanticDoc {
            doc_id: "d2".into(),
            lang: "en".into(),
            title: "leave".into(),
            body: "annual leave policy".into(),
        },
    ];
    let case = SemanticCase {
        id: "c1".into(),
        bucket: "crosslang".into(),
        query: "年假规定".into(),
        relevant: vec![super::super::data::RelevantDoc {
            doc_id: "d1".into(),
            grade: 3,
        }],
    };
    let mut vc = VectorCache {
        model_id: "m".into(),
        dim: 2,
        doc_vectors: BTreeMap::new(),
        query_vectors: BTreeMap::new(),
    };
    vc.doc_vectors.insert("d1".into(), vec![1.0, 0.0]);
    vc.doc_vectors.insert("d2".into(), vec![0.9, 0.1]);
    vc.query_vectors.insert("c1".into(), vec![1.0, 0.0]);
    // d1 cosine = 1.0、d2 cosine ≈ 0.9；threshold = 0.5 → cosine_top1 = 1.0 ≥ 0.5 → 跳 FTS
    let s = score_case(&case, &corpus, &vc, 0.30, 2.0, 60.0, 0.50, 10);
    assert_eq!(s.id, "c1");
    assert!(s.vec_recall > 0.0, "向量臂应召回 d1");
    // HYBR-routed 跳 FTS → 应等价纯 vec
    assert!(
        (s.hybrid_routed_recall - s.vec_recall).abs() < 1e-9,
        "跳 FTS 后 HYBR_R 应等于 VEC_R"
    );
}
```

- [ ] **Step 2: 跑测试验证编译失败（红）**

Run: `cargo test -p locifind-evals --lib score_case_runs_four_arms_with_cosine_routing 2>&1 | tail -15`
Expected: 编译错误（`score_case` 第 7 参旧签名 / `vector_rank` 返 tuple 等）

- [ ] **Step 3: 改 `score_case` 签名 + 入口**

`packages/evals/src/semantic_quality/report.rs` line 43-91 当前 `score_case`。改造为：

第 6-7 行 import 段，**删除** `use locifind_result_normalizer::lang::detect_lang;`（A-5 评测层不再调）。

整段 `score_case` 替换为：

```rust
/// 跑四臂 + 算 Recall@k/nDCG@k。`floor`/`weight`/`k_rrf`/`cosine_threshold` 是生产融合旋钮。
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn score_case(
    case: &SemanticCase,
    corpus: &[SemanticDoc],
    vectors: &VectorCache,
    floor: f32,
    weight: f64,
    k_rrf: f64,
    cosine_threshold: f64,
    top_k: usize,
) -> CaseScores {
    /// 三臂取的候选池大小（指标只看前 `top_k`，池子放宽以容纳融合重排）。
    const POOL: usize = 50;

    let relevant_set: HashSet<String> = case.relevant.iter().map(|r| r.doc_id.clone()).collect();
    let grades: HashMap<String, u8> = case
        .relevant
        .iter()
        .map(|r| (r.doc_id.clone(), r.grade))
        .collect();

    let fts = fts_rank(corpus, &case.query, POOL).unwrap_or_default();
    let empty = Vec::new();
    let qv = vectors.query_vectors.get(&case.id).unwrap_or(&empty);
    let vec_scored = vector_rank(qv, &vectors.doc_vectors, floor, POOL);
    // 只要 doc_id 给 ndcg / recall 算分（不消费 cosine）
    let vec_ids: Vec<String> = vec_scored.iter().map(|(id, _)| id.clone()).collect();
    let hybrid = hybrid_rank(corpus, &fts, &vec_scored, weight, k_rrf);
    let hybrid_routed = super::arms::hybrid_routed_rank(
        corpus,
        &fts,
        &vec_scored,
        cosine_threshold,
        weight,
        k_rrf,
    );

    CaseScores {
        id: case.id.clone(),
        bucket: case.bucket.clone(),
        fts_recall: recall_at_k(&fts, &relevant_set, top_k),
        vec_recall: recall_at_k(&vec_ids, &relevant_set, top_k),
        hybrid_recall: recall_at_k(&hybrid, &relevant_set, top_k),
        fts_ndcg: ndcg_at_k(&fts, &grades, top_k),
        vec_ndcg: ndcg_at_k(&vec_ids, &grades, top_k),
        hybrid_ndcg: ndcg_at_k(&hybrid, &grades, top_k),
        hybrid_routed_recall: recall_at_k(&hybrid_routed, &relevant_set, top_k),
        hybrid_routed_ndcg: ndcg_at_k(&hybrid_routed, &grades, top_k),
    }
}
```

- [ ] **Step 4: 跑 step 1 单测过（绿）**

Run: `cargo test -p locifind-evals --lib score_case_runs_four_arms_with_cosine_routing 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step 5: 跑 evals 模块全过**

Run: `cargo test -p locifind-evals --lib 2>&1 | grep "test result:" | head -5`
Expected: 0 failed（`aggregate_means_per_bucket_and_overall` 不动、新 cosine 单测过）

但 bin/semantic_quality.rs 仍调旧签名、tests/semantic_quality_gate.rs 仍 import 旧常量——build 失败，task 6/8 修。

- [ ] **Step 6: 提交**

```bash
git add packages/evals/src/semantic_quality/report.rs
git commit -m "BETA-15B-3 A-5 task 5：evals report score_case 第 7 参 cosine_threshold + 删 detect_lang 调用（评测层不消费 query_lang）+ vector_rank tuple 输出消费 + 单测改造"
```

---

## Task 6: bin/semantic_quality.rs CLI flag `--cosine-threshold`

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs`

CLI flag `--max-cross-lang-hits` 改 `--cosine-threshold`（f64、默认 `DEFAULT_COSINE_ROUTING_THRESHOLD`）；改造 2 个 CLI 单测；改 `score_case` call 第 7 参数。

- [ ] **Step 1: 写失败测试 = `--cosine-threshold` 解析**

`packages/evals/src/bin/semantic_quality.rs` 的 `mod cli_tests {}` 内**替换** `max_cross_lang_hits_flag_parses` 为：

```rust
#[test]
fn cosine_threshold_flag_parses() {
    let cli = Cli::parse_from(["semantic_quality", "--cosine-threshold", "0.85"]);
    assert!((cli.cosine_threshold - 0.85).abs() < f64::EPSILON);
}
```

**替换** `max_cross_lang_hits_defaults_to_const` 为：

```rust
#[test]
fn cosine_threshold_defaults_to_const() {
    use locifind_result_normalizer::DEFAULT_COSINE_ROUTING_THRESHOLD;
    let cli = Cli::parse_from(["semantic_quality"]);
    assert!((cli.cosine_threshold - DEFAULT_COSINE_ROUTING_THRESHOLD).abs() < f64::EPSILON);
}
```

- [ ] **Step 2: 跑测试验证编译失败（红）**

Run: `cargo test -p locifind-evals --bin semantic_quality cosine_threshold 2>&1 | tail -15`
Expected: 编译错误（`cosine_threshold` 字段未定义 / `DEFAULT_COSINE_ROUTING_THRESHOLD` 未 import 等）

- [ ] **Step 3: 改 CLI struct 字段 + import**

`packages/evals/src/bin/semantic_quality.rs` line 14-16 当前：

```rust
use locifind_result_normalizer::{
    DEFAULT_MAX_CROSS_LANG_HITS, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

改为：

```rust
use locifind_result_normalizer::{
    DEFAULT_COSINE_ROUTING_THRESHOLD, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

line 37-41 当前 `max_cross_lang_hits` 字段：

```rust
    /// 跨语种 vec hit 计数路由阈值；≥ 此值时跳过 FTS 臂。
    /// `usize::MAX` ≈ 永不跳（与 A-3 spec §5 降级值同义）。
    /// 默认 = `DEFAULT_MAX_CROSS_LANG_HITS`（task 7 sweep 后 bake）。BETA-15B-3 A-4。
    #[arg(long, default_value_t = DEFAULT_MAX_CROSS_LANG_HITS)]
    max_cross_lang_hits: usize,
```

整段替换为：

```rust
    /// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS 臂。
    /// `1.01` ≈ 永不跳（cosine ∈ [0,1] 物理上限、与 spec §5 降级值同义）。
    /// 默认 = `DEFAULT_COSINE_ROUTING_THRESHOLD`（task 8 sweep 后 bake）。BETA-15B-3 A-5。
    #[arg(long, default_value_t = DEFAULT_COSINE_ROUTING_THRESHOLD)]
    cosine_threshold: f64,
```

- [ ] **Step 4: 改 main() 内 `score_case` call 第 7 参数**

`packages/evals/src/bin/semantic_quality.rs` line 99-113 `main()` 内当前：

```rust
    let scores: Vec<_> = cases
        .iter()
        .map(|c| {
            score_case(
                c,
                &corpus,
                &vectors,
                EVAL_SIMILARITY_FLOOR,
                cli.semantic_weight,
                DEFAULT_RRF_K,
                cli.max_cross_lang_hits,
                TOP_K,
            )
        })
        .collect();
```

第 7 参数 `cli.max_cross_lang_hits` 改为 `cli.cosine_threshold`：

```rust
    let scores: Vec<_> = cases
        .iter()
        .map(|c| {
            score_case(
                c,
                &corpus,
                &vectors,
                EVAL_SIMILARITY_FLOOR,
                cli.semantic_weight,
                DEFAULT_RRF_K,
                cli.cosine_threshold,
                TOP_K,
            )
        })
        .collect();
```

- [ ] **Step 5: 跑 step 1 单测过（绿）**

Run: `cargo test -p locifind-evals --bin semantic_quality 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. 4 passed; 0 failed; ...`（2 semantic_weight + 2 cosine_threshold 替换 max_cross_lang_hits）

- [ ] **Step 6: 跑 evals 模块全过 + workspace 编译**

Run: `cargo test -p locifind-evals 2>&1 | grep "test result:" | head -5`
Expected: 0 failed（lib + bin 都过；gate.rs 仍跑 + 但若 vectors.json/baseline.json 存在则同步走 task 8 改 import）

Run: `cargo build --workspace 2>&1 | tail -10`
Expected: 编译错误（harness/fanout_merge.rs 仍调旧 wrapper 签名）—— task 9 修。

为本 task 验证 evals crate 净：

Run: `cargo build -p locifind-evals 2>&1 | tail -5`
Expected: `Finished ... target(s) in N.NNs`

- [ ] **Step 7: 提交**

```bash
git add packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-3 A-5 task 6：semantic_quality binary --cosine-threshold flag（替换 --max-cross-lang-hits、CLI 4 单测改造）"
```

---

## Task 7: 手动 sweep 9 个 cosine_threshold 值 + 选 T*

**Files:**
- 无文件改动；产 sweep-cosine.log + 决定 T*

**前置条件**：task 1-6 完成、`harness/fanout_merge.rs` 仍编译失败但**不阻塞评测 binary**——evals binary 独立编译跑。

- [ ] **Step 1: 临时绕过 workspace 编译错误**

由于 task 7 之前 harness 还未改造（task 9 后才编通），不能直接 `cargo run -p locifind-evals --bin semantic_quality` 因为 workspace 依赖图。

直接限定 crate：

Run: `cargo build --release -p locifind-evals --bin semantic_quality 2>&1 | tail -5`
Expected: `Finished release ... target(s) in N.NNs`

如失败：检查是否有 evals 内部对 result-normalizer 的 lang/Jaccard 间接依赖未清——通常 task 1-6 已干净。

- [ ] **Step 2: 跑 9 个阈值的 sweep**

```bash
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== cosine_threshold = $T ==="
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --cosine-threshold $T
done 2>&1 | tee /tmp/sweep-cosine.log
```

每 T 应输出 6 桶表格含 `HYBR_R / HYBR_N`（同 A-3/A-4 binary 表格格式）。

- [ ] **Step 3: 人工读 sweep 结果，按 spec §2.2 顺序选 T***

打开 `/tmp/sweep-cosine.log`，提取每 T 各桶 `HYBR_R / HYBR_N`。

**红线 (4a)**：每 T 的 exact-name HYBR_R 必须 = 1.000（否则 = bug）

**红线 (4b)**：A-4 baseline 各桶 HYB_N：
- synonym 0.9051
- concept 0.8190
- crosslang 0.6492
- content-not-name 0.9303
- exact-name 1.0
- OVERALL 0.8536

T* 必须满足**所有桶** HYBR_N ≥ HYB_N 同桶。

**优先期望 (4c)(4d)**：
- OVERALL HYBR_N 最大化、优先 > 0.854
- crosslang HYBR_N 最大化、优先 > 0.649

**降级路径**：若所有 T < 1.01 都至少破一桶 (4b) → T* = 1.01（永不跳、HYBR ≡ HYB、与 A-4 同款 spec §5 降级）

- [ ] **Step 4: 记录 sweep 全表（手抄进调优记录草稿、task 10 写正式）**

```markdown
| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec) | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.30 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.45 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.60 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.70 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.80 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.90 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.99 | <填> | <填> | <填> | <填> | <填> | <填> |
| 1.01 (≡HYB) | 1.000 | 0.854 | 0.649 | 0.930 | 0.819 | 0.905 |
```

控制对照核验：
- T=0.0 → HYBR ≈ VEC（除 exact-name 因两臂完全重叠仍 HYBR_R=1.0）
- T=1.01 → HYBR ≡ HYB（六桶完全相等）

- [ ] **Step 5: 决定 T\* 并暂存供 task 8 bake**

决定逻辑（按 spec §2.2 顺序）：
1. 排除所有破 (4b) 的 T 候选
2. 余者按 OVERALL HYBR_N 降序排
3. 选最大 OVERALL 的 T
4. 若步骤 1 后无候选 → T\* = 1.01（spec §5 降级）

把决定写进 `/tmp/T-star.txt`：

```bash
echo "T* = <填具体数值>" > /tmp/T-star.txt
echo "理由：<填一句话>" >> /tmp/T-star.txt
```

- [ ] **Step 6: 无 commit**（手动步骤、task 8 落 bake commit）

---

## Task 8: bake T* 进 DEFAULT_COSINE_ROUTING_THRESHOLD + rewrite baseline.json + gate.rs import 同步

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`
- Modify: `packages/evals/tests/semantic_quality_gate.rs`

- [ ] **Step 1: bake T\* 到 DEFAULT_COSINE_ROUTING_THRESHOLD**

`packages/result-normalizer/src/lib.rs` line 97-104（task 3 step 3 写入的）当前：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 引入替代 A-4 的 lang 信号。task 8 sweep 后 bake；
/// 本 task 占位 = 1.01（cosine ∈ [0,1] 物理上限、永不跳、HYBR ≡ HYB）使既有路径 byte-equal。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 1.01;
```

改为（含 T\* 决策依据简述）：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 **T\* = <填具体值>**：<填简述：spec §2.2 顺序 + 选定理由
/// 或 spec §5 降级原因 + 失败桶名>。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = <填具体值>;
```

例若 T\* = 1.01（spec §5 降级）：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 **T\* = 1.01**（spec §5 字面正解保守降级）：
/// 所有 T < 1.01 在合成集 sweep 中至少破一桶 spec (4b) ≥ HYB baseline 硬红线
/// （多为 content-not-name 桶 FTS 强场景被跳掉反伤）；T = 1.01 等价 HYBR ≡ HYB、
/// 路由本 cycle 不生效。下 cycle 抓手 = ③ 更大 embedding 模型 / ④ 评测集扩量。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 1.01;
```

- [ ] **Step 2: rewrite baseline.json**

跑评测把当前 hybrid_routed_* 数值写入新 baseline：

```bash
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 \
  --write-baseline
```

注意：`semantic_weight` 显式传 10.0（默认值同、显式更稳）；不传 `--cosine-threshold` → 使用刚 bake 的 `DEFAULT_COSINE_ROUTING_THRESHOLD = T*`；`--write-baseline` 让 binary 把当前 aggs 写成 `baseline.json`。

验证写入：

Run: `head -20 packages/evals/fixtures/semantic-recall/baseline.json`
Expected: 6 桶 JSON、每桶 `hybrid_routed_recall / hybrid_routed_ndcg` 数值反映新 T\*。

- [ ] **Step 3: gate.rs 同步 import + score_case call**

`packages/evals/tests/semantic_quality_gate.rs` line 12-14 当前：

```rust
use locifind_result_normalizer::{
    DEFAULT_MAX_CROSS_LANG_HITS, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

改为：

```rust
use locifind_result_normalizer::{
    DEFAULT_COSINE_ROUTING_THRESHOLD, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

line 39-53 `score_case` call 第 7 参 `DEFAULT_MAX_CROSS_LANG_HITS` 改 `DEFAULT_COSINE_ROUTING_THRESHOLD`：

```rust
    let scores: Vec<_> = cases
        .iter()
        .map(|c| {
            score_case(
                c,
                &corpus,
                &vectors,
                EVAL_SIMILARITY_FLOOR,
                DEFAULT_SEMANTIC_WEIGHT,
                DEFAULT_RRF_K,
                DEFAULT_COSINE_ROUTING_THRESHOLD,
                TOP_K,
            )
        })
        .collect();
```

line 86-89 gate.rs doc 注释当前提 A-3 t\* / A-4 N\* —— **更新**为 A-5 T\*：

```rust
    // BETA-15B-3 A-5 红线：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=<填> bake 后数值无需替换。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
```

- [ ] **Step 4: 跑 gate + workspace 测试全过**

Run: `cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`（4 红线全过）

Run: `cargo test --workspace 2>&1 | grep "test result:" | tail -10`
Expected: harness 仍有错（task 9 修）、其余净。

- [ ] **Step 5: 跑 v0.5 + v0.9 parser-only byte-equal 检查**

Run: `cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/a5-v05.json && python3 -c "
import json
with open('/tmp/a5-v05.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.5', counts)
"`
Expected: `v0.5 {'pass': 473, 'partial': 25, 'fail': 2}` 精确

Run: `cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null > /tmp/a5-v09.json && python3 -c "
import json
with open('/tmp/a5-v09.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.9', counts)
"`
Expected: `v0.9 {'pass': 877, 'partial': 119, 'fail': 4}` 精确

- [ ] **Step 6: 提交**

```bash
git add packages/result-normalizer/src/lib.rs \
        packages/evals/fixtures/semantic-recall/baseline.json \
        packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-3 A-5 task 8：bake DEFAULT_COSINE_ROUTING_THRESHOLD = <T*值> + rewrite baseline.json + gate import 同步（4 红线动态读 baseline）"
```

---

## Task 9: harness fanout_merge.rs 改 wrapper 新签名 + 后置覆写 verdict.query_lang + 改 3 单测

**Files:**
- Modify: `packages/harness/src/fanout_merge.rs`

本 task 让生产 wiring 跟上 wrapper 新签名：① import 改成 `DEFAULT_COSINE_ROUTING_THRESHOLD`；② `run_fanout_merge_rrf` 内删 `let query_lang = detect_lang(query)` 调用从 wrapper 调用前面拿掉（wrapper 不再接 query_lang）；③ 改 wrapper 调用为 5 参；④ 后置覆写 `verdict.query_lang = detect_lang(query)` 用 struct-update 填真值（BETA-15B-5 badge 槽位元数据）；⑤ 改造 3 个 A-4 fanout 单测（mock backend 加 score、断言 vec_top1_cosine / cosine_threshold）；⑥ desktop wiring 不需改动（raw_query 已透传）。

- [ ] **Step 1: 升级 import + run_fanout_merge_rrf 内部**

`packages/harness/src/fanout_merge.rs` line 13-15 当前：

```rust
use locifind_result_normalizer::{
    fuse_rrf_with_fts_routing, lang::detect_lang, merge_results, MergedResult, RouteVerdict,
    DEFAULT_MAX_CROSS_LANG_HITS, DEFAULT_RRF_K,
};
```

改为：

```rust
use locifind_result_normalizer::{
    fuse_rrf_with_fts_routing, lang::detect_lang, merge_results, MergedResult, RouteVerdict,
    DEFAULT_COSINE_ROUTING_THRESHOLD, DEFAULT_RRF_K,
};
```

`run_fanout_merge_rrf` 函数体 line 170-180 当前：

```rust
    // 任一臂空 → wrapper 内 early-return guard 兜底（无路由信号、不跳 FTS）；
    // wiring 不再带 ad-hoc 边界条件，wrapper 是单一不变量来源。
    let query_lang = detect_lang(query);
    let (merged, verdict) = fuse_rrf_with_fts_routing(
        fts_list,
        vec_list,
        DEFAULT_RRF_K,
        semantic_weight,
        query_lang,
        DEFAULT_MAX_CROSS_LANG_HITS,
    );
```

改为：

```rust
    // BETA-15B-3 A-5：wrapper 5 参（cosine_threshold 替换 A-4 6 参的 lang 信号 + max）。
    // 任一臂空 → wrapper 内 early-return guard 兜底；wrapper 内部 verdict.query_lang
    // 默认 Mixed 占位、wiring 后置覆写填真值（BETA-15B-5 badge 元数据）。
    let (merged, verdict) = fuse_rrf_with_fts_routing(
        fts_list,
        vec_list,
        DEFAULT_RRF_K,
        semantic_weight,
        DEFAULT_COSINE_ROUTING_THRESHOLD,
    );
    let verdict = RouteVerdict {
        query_lang: detect_lang(query),
        ..verdict
    };
```

doc 注释（line 103-109）也同步改：当前提 A-4 lang 信号 + `DEFAULT_MAX_CROSS_LANG_HITS`，改成 A-5 cosine 信号 + `DEFAULT_COSINE_ROUTING_THRESHOLD`：

```rust
/// （语义召回臂 + FTS 臂的 hybrid 路径用此变体）。
///
/// 区别于 [`run_fanout_merge`] 的扁平 `merge_results`：此处把每个后端结果保留为一条
/// **有序列表**（到达顺序=rank），交 [`fuse_rrf_with_fts_routing`] 跨列表按 path 累加加权倒数排名。
/// 语义臂（`BackendKind::SemanticIndex`）列表用调用方指定的 `semantic_weight`
/// （生产用 `AppSettings.semantic_weight` live-read，详 BETA-15B-3 A-2 spec），其余权重 1.0。
/// BETA-15B-3 A-5 起按 `backend_kind` 把 `SemanticIndex` 入 vec 臂、其它入 fts 臂，
/// `vec[0].score`（cosine）≥ `DEFAULT_COSINE_ROUTING_THRESHOLD` 时 wrapper 跳过 FTS 臂。
/// 判定 `RouteVerdict` 透传到 `FanoutOutcome.route_verdict` 供后续 UI 消费；
/// `verdict.query_lang` 由 wiring 用 `detect_lang(query)` 后置覆写填真值
/// （wrapper 内部默认 Mixed 占位）。
/// 错误/取消语义与 [`run_fanout_merge`] 一致：部分失败记 `errors`、不中断其它后端。
```

- [ ] **Step 2: 编译并检查 step 1 后 fanout 模块通过 + 其余单测因 mock backend 无 score 临时失败**

Run: `cargo build -p locifind-harness 2>&1 | tail -10`
Expected: `Finished ... target(s) in N.NNs`

Run: `cargo test -p locifind-harness --lib fanout_merge 2>&1 | grep "test result:" | head -3`
Expected: A-4 3 个 fanout 单测可能失败（mock 无 score → cosine_top1=0、threshold=T* 可能 → skipped_fts=false 与原测期不符），需 step 3 改造。

- [ ] **Step 3: 改造 3 个 A-4 fanout 单测**

`packages/harness/src/fanout_merge.rs` `mod tests {}` 内：

**替换** `fanout_rrf_en_query_with_zh_vec_hits_skips_fts` 为：

```rust
#[test]
fn fanout_rrf_high_cosine_skips_fts() {
    // vec backend 返带 score=0.9 的 SearchResult → cosine_top1=0.9
    // bake 后 T* < 0.9 时跳 FTS；T*>=0.9（含 1.01 降级）时不跳
    let backends = vec![
        tool(
            "search.local",
            BackendKind::NativeIndex,
            Script::Results(vec![result_at(
                "/annual_leave.md",
                BackendKind::NativeIndex,
                MatchType::Filename,
            )]),
        ),
        tool(
            "search.semantic",
            BackendKind::SemanticIndex,
            Script::Results(vec![{
                let mut r = result_at(
                    "/year_off_rules.md",
                    BackendKind::SemanticIndex,
                    MatchType::Content,
                );
                r.score = Some(0.9);
                r
            }]),
        ),
    ];
    let mut got: Vec<MergedResult> = Vec::new();
    let outcome = block_on(run_fanout_merge_rrf(
        &backends,
        &ExpandedSearchIntent::identity(intent()),
        CancellationToken::new(),
        &mut |m| got.push(m),
        10.0,
        "annual leave policy", // En query → query_lang 覆写为 En
    ));
    let v = outcome.route_verdict.expect("RRF 路径应填 route_verdict");
    assert_eq!(v.query_lang, locifind_result_normalizer::lang::Lang::En);
    assert!((v.vec_top1_cosine - 0.9).abs() < f64::EPSILON);
    // T* < 0.9 时跳；T* ≥ 0.9（如 1.01 降级）时不跳 —— 用 `||` 让任一 T* bake 都通过
    assert!(
        v.skipped_fts || locifind_result_normalizer::DEFAULT_COSINE_ROUTING_THRESHOLD > 0.9,
        "T*={} 下 cosine=0.9 应跳 FTS（或 spec §5 降级 T*>0.9 时不跳）",
        locifind_result_normalizer::DEFAULT_COSINE_ROUTING_THRESHOLD
    );
}
```

**替换** `fanout_rrf_zh_query_no_skip_when_pure_zh_corpus` 为：

```rust
#[test]
fn fanout_rrf_low_cosine_does_not_skip() {
    // vec backend 返带 score=0.10 → cosine_top1=0.10 < 任一合理 T* → 不跳
    let backends = vec![
        tool(
            "search.local",
            BackendKind::NativeIndex,
            Script::Results(vec![result_at(
                "/年假规定.md",
                BackendKind::NativeIndex,
                MatchType::Content,
            )]),
        ),
        tool(
            "search.semantic",
            BackendKind::SemanticIndex,
            Script::Results(vec![{
                let mut r = result_at(
                    "/年假规定.md",
                    BackendKind::SemanticIndex,
                    MatchType::Content,
                );
                r.score = Some(0.10);
                r
            }]),
        ),
    ];
    let mut got: Vec<MergedResult> = Vec::new();
    let outcome = block_on(run_fanout_merge_rrf(
        &backends,
        &ExpandedSearchIntent::identity(intent()),
        CancellationToken::new(),
        &mut |m| got.push(m),
        10.0,
        "年假规定", // ZH query → query_lang 覆写为 Zh
    ));
    let v = outcome.route_verdict.expect("RRF 路径应填 route_verdict");
    assert_eq!(v.query_lang, locifind_result_normalizer::lang::Lang::Zh);
    assert!((v.vec_top1_cosine - 0.10).abs() < f64::EPSILON);
    assert!(
        !v.skipped_fts || locifind_result_normalizer::DEFAULT_COSINE_ROUTING_THRESHOLD <= 0.10,
        "cosine=0.10 应不跳（除非 T* ≤ 0.10）"
    );
}
```

**替换** `fanout_rrf_mixed_query_never_skips` 为：

```rust
#[test]
fn fanout_rrf_verdict_query_lang_metadata_mixed() {
    // Mixed query 元数据覆写：query_lang 字段填 Mixed；不影响路由判定（cosine 信号驱动）
    let backends = vec![
        tool(
            "search.local",
            BackendKind::NativeIndex,
            Script::Results(vec![result_at(
                "/qwen_tuning.md",
                BackendKind::NativeIndex,
                MatchType::Filename,
            )]),
        ),
        tool(
            "search.semantic",
            BackendKind::SemanticIndex,
            Script::Results(vec![{
                let mut r = result_at(
                    "/年假规定.md",
                    BackendKind::SemanticIndex,
                    MatchType::Content,
                );
                r.score = Some(0.40);
                r
            }]),
        ),
    ];
    let mut got: Vec<MergedResult> = Vec::new();
    let outcome = block_on(run_fanout_merge_rrf(
        &backends,
        &ExpandedSearchIntent::identity(intent()),
        CancellationToken::new(),
        &mut |m| got.push(m),
        10.0,
        "qwen 调优", // Mixed query：3 ASCII + 2 CJK → ratio 0.4
    ));
    let v = outcome.route_verdict.expect("RRF 路径应填 route_verdict");
    assert_eq!(v.query_lang, locifind_result_normalizer::lang::Lang::Mixed);
    assert!((v.vec_top1_cosine - 0.40).abs() < f64::EPSILON);
}
```

- [ ] **Step 4: 跑 harness 模块全过**

Run: `cargo test -p locifind-harness --lib fanout_merge 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`（A-4 3 个 + 已有 + 改造单测全过）

- [ ] **Step 5: 跑 workspace 测试全过**

Run: `cargo test --workspace 2>&1 | grep "test result:" | awk '{passed += $4; failed += $6} END {print "total passed:", passed, "/ failed:", failed}'`
Expected: `total passed: N / failed: 0`（N ≈ 865 ±）

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 净

Run: `cargo fmt --all --check`
Expected: 净

- [ ] **Step 6: 检查 desktop 端 raw_query 字段仍有 detect_lang 间接依赖（无须改动）**

Run: `grep -n "detect_lang\|raw_query" apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/search/fanout.rs | head -10`
Expected: `raw_query` 字段仍透传到 `run_fanout_merge_rrf`、不直接调 `detect_lang`（detect_lang 在 harness 内部调）。无须改动 desktop。

- [ ] **Step 7: 跑 evals byte-equal 复检**

Run: `cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/a5-v05-recheck.json && python3 -c "
import json
with open('/tmp/a5-v05-recheck.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.5', counts)
"`
Expected: `v0.5 {'pass': 473, 'partial': 25, 'fail': 2}` 精确

- [ ] **Step 8: 提交**

```bash
git add packages/harness/src/fanout_merge.rs
git commit -m "BETA-15B-3 A-5 task 9：harness fanout_merge.rs wrapper 5 参 + DEFAULT_COSINE_ROUTING_THRESHOLD + 后置覆写 verdict.query_lang struct-update + 3 fanout 单测改造（mock 加 score 模拟 cosine）"
```

---

## Task 10: baseline 报告追加 A-5 调优记录节 + 总验收

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`

- [ ] **Step 1: 在 baseline 报告末尾追加 A-5 调优记录节**

`docs/reviews/semantic-recall-quality-baseline.md` 末尾追加：

````markdown

## A-5 VEC top-1 cosine 绝对分数阈值路由（2026-06-24 Claude Code）

**承接**：A-4 sweep 暴露 lang 单维信号天然分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」、N\*=usize::MAX spec §5 降级路由本 cycle 不生效。A-5 升级 wrapper 内部信号为 **VEC top-1 cosine 绝对分数阈值**（`vec[0].score >= threshold` 跳 FTS），对准 A-4 暴露的不对称失败模式。

**信号设计**：
- 信号源 `vec[0].score.unwrap_or(0.0)`（生产 `packages/search-backends/semantic-index/src/lib.rs:163` 注释「`score = cosine`（升 f64）」、A-5 评测层 `vector_rank → Vec<(String, f32)>` + `to_results_with_scores` 把 cosine 挂在 `SearchResult.score` 透传）
- 动作方向 `cosine_top1 >= cosine_threshold` 跳 FTS（vec 强信任跳；与 A-4 数据指证方向同构）
- wrapper API 名 / RouteVerdict 结构 / HYBR baseline 字段名 / gate 红线架构保留（A-3/A-4 基础设施 zero-touch）
- wrapper 签名 6 参 → 5 参（删 `query_lang + max_cross_lang_hits`、加 `cosine_threshold: f64`）
- RouteVerdict 字段：`cross_lang_hits → vec_top1_cosine: f64`、`max_cross_lang_hits → cosine_threshold: f64`、**保留 `query_lang: Lang`** 字段（wrapper 默认 Mixed 占位、生产 wiring `run_fanout_merge_rrf` 用 struct-update 后置覆写 `verdict.query_lang = detect_lang(query)` 填真值供 BETA-15B-5 badge 槽位）
- A-3 `jaccard_overlap_by_path` 函数 + 5 单测删除（A-4 wrapper 已不用）；A-4 `detect_lang` + 8 单测保留（wiring 仍用）

**Sweep 全表**（W=10.0 固定、T = cosine_threshold）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| <填 sweep 9 行实测>|
| **<T\* 行>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** |

A-2 HYB baseline（对照基准）：synonym 0.905 / concept 0.819 / crosslang 0.649 / content-not-name 0.930 / exact-name 1.000 / OVERALL 0.854。

**控制对照核验**：
- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等）✓
- T=1.01 时 HYBR ≡ HYB（六桶完全相等）✓
- wrapper 行为正确、cosine 真透传

**T\* 选定 = <填>，<填降级理由 / 抬指标理由>**：

依据 spec §2.2 红线顺序：
1. **exact-name HYBR_R = 1.000** ✅ 硬红线（所有 T 都满足、两臂在精确名查询天然高重叠）
2. **各桶 HYBR_N ≥ HYB baseline 同桶** <填验证情况>
3. **OVERALL HYBR_N** <填实测数值 vs spec §2.2 (4d) 0.864 目标>
4. **crosslang HYBR_N** <填实测数值 vs spec §2.2 (4c) 0.700 目标>

**实测变化（HYBR vs A-4 baseline HYB）**：
- <填各桶 Δ>

**诚实边界 — <填：cosine 信号是否突破 A-3/A-4 暴露的两场景天花板>**：

<填结论：cosine 信号是否如理论预期分开两场景，或同样撞天花板>

**下 cycle 抓手 = <填 A-6 优先级>**：
- ③ 更大 embedding 模型 qwen3-0.6b→1.5b/3b（需 Mac 训练 + 模型分发）
- ④ 评测集扩量 + 重构 content-not-name 桶 case（合成集 11 例可能过窄）
- 或：cosine + lang/jaccard 组合信号（复用 A-3 jaccard git history + 现有 detect_lang）

**基础设施完整入栈**（为下 cycle 留好旋钮）：
- `result-normalizer::lang::Lang/detect_lang` 保留作 wiring 元数据
- wrapper `fuse_rrf_with_fts_routing` 5 参签名 + `RouteVerdict { skipped_fts, query_lang, vec_top1_cosine, cosine_threshold }`
- 评测 `vector_rank → (id, cosine)` + `to_results_with_scores` + `score_case` cosine_threshold + binary `--cosine-threshold` flag
- 生产 `run_fanout_merge_rrf` 5 参 wrapper 调用 + struct-update 后置覆写 query_lang
- baseline.json HYBR 字段 rewrite + gate 4 红线动态读 baseline

链接：[spec](../superpowers/specs/2026-06-23-beta-15b-3a5-cosine-routing-design.md) / [plan](../superpowers/plans/2026-06-24-beta-15b-3a5-cosine-routing.md)
````

把 `<填>` 占位符替换成 task 7 sweep 实测 + task 8 bake 决定值。

- [ ] **Step 2: 总验收**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 净（无 warning、无 error）

Run: `cargo test --workspace 2>&1 | grep "test result:" | awk '{passed += $4; failed += $6} END {print "total passed:", passed, "/ failed:", failed}'`
Expected: `total passed: N / failed: 0`

Run: `cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5`
Expected: `test result: ok. 1 passed; 0 failed; ...`

Run: 跑 v0.5 + v0.9 byte-equal 检查（同 task 8 step 5）
Expected: v0.5 = 473/25/2 / v0.9 = 877/119/4 精确

- [ ] **Step 3: 提交**

```bash
git add docs/reviews/semantic-recall-quality-baseline.md
git commit -m "BETA-15B-3 A-5 task 10：baseline 报告追加 A-5 调优记录 + 总验收过（T*=<填>、<降级 or 抬指标>、A-6 候选 3 项）"
```

---

## 验证 checklist 汇总

- [x] task 每步独立 fmt + clippy + test 验证门
- [x] task 8 + task 9 step 5 含 v0.5/v0.9 parser-only byte-equal 闸门（reporter JSON 非确定 → 规范化逐 case 比对计数，[[project-evals-reporter-nondeterministic]]）
- [x] task 8 + task 10 step 2 含 `cargo test -p locifind-evals --test semantic_quality_gate` 用新 baseline pass
- [x] task 9 含 mock backend 加 score 让 wrapper cosine 路由被真触发（A-4 mock 仅 ZH/EN 文件名不能直接复用）

## 风险与对策汇总

| 风险 | 对策 |
|---|---|
| cosine 信号同样撞 (4b) 天花板 | task 7 sweep 全表读、按 spec §2.2 顺序选；若全破 → T\* = 1.01 spec §5 降级 |
| vec[0].score == None 边界 | wrapper `unwrap_or(0.0)` 兜底、`wrapper_no_score_treated_as_zero` 单测覆盖 |
| 评测 SearchResult.score 透传破 fuse_rrf dedup | path 仍是 dedup key、既有 fuse_rrf 单测全跑过即不破；task 4/5/6 验证门覆盖 |
| byte-equal v0.5/v0.9 退步 | 本 cycle 完全不动 parser/coverage/model fallback；只动 result-normalizer wrapper + evals 设施 + harness wiring；byte-equal 自然保持 |
| wrapper 5 参签名比 A-4 6 参少了 query_lang 输入 | 文档明示「wrapper 默认 Mixed 占位、调用侧后置覆写」；harness step 1 已写后置覆写、单测覆盖 |
| 删 jaccard 后下 cycle 想做组合信号须重写 | YAGNI、git history 可恢复；spec §3.2 已记 |

## 链接

- spec：[../specs/2026-06-23-beta-15b-3a5-cosine-routing-design.md](../specs/2026-06-23-beta-15b-3a5-cosine-routing-design.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)
- A-4 plan（参考节奏）：[2026-06-23-beta-15b-3a4-lang-routing.md](./2026-06-23-beta-15b-3a4-lang-routing.md)
- A-3 plan：[2026-06-23-beta-15b-3a3-fts-confidence-routing.md](./2026-06-23-beta-15b-3a3-fts-confidence-routing.md)

---

## Plan Self-Review（writing-plans skill 要求）

### 1. Spec coverage 检查

逐 spec §3.1 in-scope 项对应 task：

| spec §3.1 项 | 对应 task |
|---|---|
| wrapper 签名 6→5 参 | task 3 step 5 |
| 常量 `DEFAULT_MAX_CROSS_LANG_HITS` → `DEFAULT_COSINE_ROUTING_THRESHOLD` | task 3 step 3 + task 8 step 1 bake |
| `RouteVerdict` 字段升级 + 保留 `query_lang` | task 3 step 4 |
| 评测层 `vector_rank` 返 `Vec<(String, f32)>` | task 4 step 3 |
| 评测层 `to_results` 加 `to_results_with_scores` helper 挂 score | task 4 step 5 |
| `hybrid_routed_rank` 签名改 cosine_threshold | task 4 step 7 |
| `score_case` 入口仍调 `detect_lang` 填元数据 | **NOTE：spec 写「仍调 detect_lang 填 CaseScores.query_lang 元数据」，但 `CaseScores` 实际无 `query_lang` 字段、评测层确实不消费——plan task 5 step 3 直接删 `detect_lang` 调用。这与 spec §3.1 描述有出入，但与 spec §4.4 数据流「报告侧不消费 verdict.query_lang」一致。本 plan 选择「删评测层 detect_lang 调用、保留 wiring 层 detect_lang 调用」是合理执行决策。** |
| binary CLI flag 改 `--cosine-threshold` | task 6 |
| 生产 `run_fanout_merge_rrf` 入口移除 `detect_lang(query)` 传 wrapper、后置覆写 verdict.query_lang | task 9 step 1 |
| 评测 sweep + bake | task 7 + task 8 |
| baseline.json HYBR 字段 rewrite | task 8 step 2 |
| gate 4 红线动态读 baseline | task 8 step 3（仅 import + score_case 第 7 参改） |
| baseline 报告 A-5 节 | task 10 step 1 |
| 删 `jaccard_overlap_by_path` 函数 + 5 单测 | task 1 |
| 删 `detect_lang` 函数 + 8 单测 | **NOTE：spec §3.1 / §4.2 说「删 detect_lang 函数体」；但 wiring 后置覆写 verdict.query_lang 仍调 detect_lang——若删则 wiring 拿不到真值。plan 修正为「保留 detect_lang + 8 单测、只升 doc 说明 A-5 新角色」（task 2）。这是 plan vs spec 的合理修正，brainstorming 阶段「部分保留」决策的真实意图。** |

**两处 plan vs spec 的修正**已在 task 描述中明示，brainstorming 阶段「部分保留」决策的字面意图是「保留 Lang enum + RouteVerdict.query_lang 字段作可观测元数据」，wiring 要填真值必须保留 detect_lang。建议 task 10 收口时同步更新 spec §3.1 / §4.2 措辞。

### 2. Placeholder 扫描

**Sweep 数据占位（合理）**：
- task 7 step 4 sweep 全表 `<填>` 9 行 × 6 列 = 54 处
- task 8 step 1 bake 注释 `<填具体值>` 2 处
- task 10 step 1 调优记录节 `<填>` ~10 处

**性质**：与 A-4 plan 的 `{N*}` / `/* N* */` 占位同款——sweep 数据驱动决策的天然「执行时填」、不是懒散 TBD。约定：
- task 7 sweep 后产 `/tmp/sweep-cosine.log` + `/tmp/T-star.txt`（含 T\* + 理由），是 task 8/10 占位的数据源
- subagent 执行 task 7 后必须暂存 sweep 全表
- task 8 + task 10 执行时按 `/tmp/T-star.txt` + `/tmp/sweep-cosine.log` 逐处替换 `<填>`

**非 sweep 占位**：plan 通篇无 TBD / TODO / implement later。

### 3. Type consistency 检查

| 概念 | 类型签名 | 跨 task 一致性 |
|---|---|---|
| `cosine_threshold` | `f64` | task 3 wrapper / task 4 hybrid_routed_rank / task 5 score_case / task 6 CLI / task 8 const 全统一 `f64` ✓ |
| `DEFAULT_COSINE_ROUTING_THRESHOLD` | `f64` const | task 3 step 3 占位 1.01 / task 8 step 1 bake 实际值 / task 6/8 import / task 9 import 全统一 ✓ |
| `RouteVerdict` 字段顺序 | `skipped_fts / query_lang / vec_top1_cosine / cosine_threshold` | task 3 step 4 定义 / task 3 单测 / task 9 单测断言全一致 ✓ |
| `vector_rank` 返回 | `Vec<(String, f32)>` | task 4 step 3 定义 / task 4 单测 / task 5 score_case 消费全一致 ✓ |
| `hybrid_routed_rank` 参数顺序 | `(corpus, fts, vec_scored, cosine_threshold, semantic_weight, k)` | task 4 step 7 定义 / task 4 单测 / task 5 调用全一致 ✓ |
| `score_case` 参数顺序 | `(case, corpus, vectors, floor, weight, k_rrf, cosine_threshold, top_k)` | task 5 定义 / task 5 单测 / task 6 main 调用 / task 8 gate.rs 调用全一致 ✓ |

### 4. Scope 检查

✅ 单一实施 plan、单一 cycle、~10 task TDD 颗粒度合理；与 A-3/A-4 plan 节奏对齐。

### 5. 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 coverage、不触发
- [[project-evals-reporter-nondeterministic]]：task 8 step 5 / task 9 step 7 byte-equal 闸门用 status 计数（spec §2.2 (5) parser-only byte-equal）
- [[feedback-baseline-lock-red-line-pattern]]：task 8 bake + lock baseline + task 10 调优记录追加，三件套全做
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring、不触发
- [[project-rrf-weight-tuning-ceiling]]：W=10.0 固定、不重调 weight
- [[feedback-per-task-verify-include-fmt]]：每 task 验证门必含 fmt + clippy + test ✓
- [[feedback-three-tool-collab]]：plan 文件名含日期，会话日志含工具名「Claude Code」
