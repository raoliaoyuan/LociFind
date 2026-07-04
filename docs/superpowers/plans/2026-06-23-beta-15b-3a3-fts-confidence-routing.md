# BETA-15B-3 A-3 FTS 置信度路由 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `result-normalizer` 加路由 wrapper `fuse_rrf_with_fts_routing`——FTS top-K 与 VEC top-K 的 Jaccard 重叠 < 阈值时跳过 FTS 臂、hybrid 退化为纯向量；评测/生产共用同一融合路径；新水位锁 baseline.json `hybrid_routed_*` 字段、回归门双守护（HYB 旧 + HYBR 新），把 A-2 触达的 weight 调优天花板（OVERALL 0.854 / crosslang 0.649）拉近纯向量基准（0.864 / 0.726）。

**Architecture:** `result-normalizer::DEFAULT_FTS_JACCARD_THRESHOLD`（单一默认源）+ `DEFAULT_FTS_ROUTING_TOP_K=10`（与评测 TOP_K 一致）→ wrapper 接 `fts_list, vec_list` 两个具体列表 → 内部 `jaccard_overlap_by_path` 决定 list 是否喂 `fuse_rrf` → 返回 `(Vec<MergedResult>, RouteVerdict)`。`fuse_rrf` 本身不动（保持 N 列表纯融合语义）。

**Tech Stack:** Rust (workspace) + clap + serde + rusqlite。无前端改动（UI 不暴露）。

**Spec:** [2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md](../specs/2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md)

**前置事实（合 main 2026-06-22）：**
- A-2 baked `DEFAULT_SEMANTIC_WEIGHT = 10.0` ([packages/result-normalizer/src/lib.rs:93](../../../packages/result-normalizer/src/lib.rs#L93))
- baseline.json 当前 schema = 6 字段 × 6 桶（FTS_R/FTS_N/VEC_R/VEC_N/HYB_R/HYB_N）；OVERALL HYB_N = 0.854、crosslang HYB_N = 0.649、exact-name HYB_R = 1.000
- `semantic_quality_gate.rs` 现守护 3 桶 hybrid_recall 不退步 + exact-name=1.0 硬断言
- 生产 hybrid 路径单一入口 = [`packages/harness/src/fanout_merge.rs:101 run_fanout_merge_rrf`](../../../packages/harness/src/fanout_merge.rs#L101)；fanout.rs `has_semantic` 分支独占（[apps/desktop/src-tauri/src/search/fanout.rs:71-83](../../../apps/desktop/src-tauri/src/search/fanout.rs#L71-L83)）

**每 task 验证门必含三件套**（[[feedback-per-task-verify-include-fmt]]）：
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

**分支约定**：从 main 拉 `feat-beta-15b-3a3-fts-confidence-routing`，开工前确认在该分支：
```
git switch -c feat-beta-15b-3a3-fts-confidence-routing
git rev-parse --abbrev-ref HEAD  # → feat-beta-15b-3a3-fts-confidence-routing
```

---

## Task 1：`jaccard_overlap_by_path` 工具函数 + 单测

**Goal:** 提供路由判定的核心数值——两个 `SearchResult` 列表 top-K 的 `path` 集合 Jaccard 重叠度。

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`（在 `fuse_rrf` 之后、`max_opt` 之前的位置；约第 153 行后）

- [ ] **Step 1：写失败测试（5 个 Jaccard 用例）**

加到 `packages/result-normalizer/src/lib.rs` 测试模块（在文件末尾既有 `#[cfg(test)] mod tests` 块内，紧跟 `fuse_rrf_empty_lists_yield_empty` 测试后；约第 300 行）：

```rust
    #[test]
    fn jaccard_identical_top_k_is_one() {
        let a = vec![
            result("/a", BackendKind::NativeIndex, MatchType::Filename),
            result("/b", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let b = vec![
            result("/a", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/b", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        let j = jaccard_overlap_by_path(&a, &b, 10);
        assert!((j - 1.0).abs() < f64::EPSILON, "全重叠应=1.0，实测 {j}");
    }

    #[test]
    fn jaccard_disjoint_top_k_is_zero() {
        let a = vec![result("/a", BackendKind::NativeIndex, MatchType::Filename)];
        let b = vec![result("/b", BackendKind::SemanticIndex, MatchType::Semantic)];
        let j = jaccard_overlap_by_path(&a, &b, 10);
        assert!(j.abs() < f64::EPSILON, "全不重叠应=0.0，实测 {j}");
    }

    #[test]
    fn jaccard_half_overlap_is_one_third() {
        let a = vec![
            result("/a", BackendKind::NativeIndex, MatchType::Filename),
            result("/b", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let b = vec![
            result("/a", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/c", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        // |∩|=1 ({a}), |∪|=3 ({a,b,c}) → 1/3
        let j = jaccard_overlap_by_path(&a, &b, 10);
        assert!((j - 1.0 / 3.0).abs() < 1e-9, "半重叠应=1/3，实测 {j}");
    }

    #[test]
    fn jaccard_both_empty_is_zero() {
        let a: Vec<SearchResult> = vec![];
        let b: Vec<SearchResult> = vec![];
        let j = jaccard_overlap_by_path(&a, &b, 10);
        assert!(j.abs() < f64::EPSILON, "双空应=0.0（良定义退化），实测 {j}");
    }

    #[test]
    fn jaccard_top_k_truncates_inputs() {
        // a 有 3 个、b 有 3 个，但 top-K=2 → 只看前两个：
        // a[0..2]={x,y}, b[0..2]={y,z} → |∩|=1, |∪|=3 → 1/3
        let a = vec![
            result("/x", BackendKind::NativeIndex, MatchType::Filename),
            result("/y", BackendKind::NativeIndex, MatchType::Filename),
            result("/zz", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let b = vec![
            result("/y", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/z", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/x", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        let j = jaccard_overlap_by_path(&a, &b, 2);
        assert!((j - 1.0 / 3.0).abs() < 1e-9, "top-K=2 截断后=1/3，实测 {j}");
    }
```

- [ ] **Step 2：跑失败测试，确认 `jaccard_overlap_by_path` 还未定义**

```
cargo test -p locifind-result-normalizer jaccard 2>&1 | tail -15
```
Expected: `error[E0425]: cannot find function jaccard_overlap_by_path`。

- [ ] **Step 3：实现 `jaccard_overlap_by_path`**

在 `packages/result-normalizer/src/lib.rs` 中、`fuse_rrf` 函数之后（约第 153 行末尾、`fn max_opt` 之前）加：

```rust
/// 计算两个有序 `SearchResult` 列表 **top-K 的 `path` 集合** Jaccard 重叠度。
///
/// `|A ∩ B| / |A ∪ B|`，A/B = 各自前 `k` 个 result 的 path 集合；空集 ∪ 空集 = 0.0
/// （良定义退化分支：调用方应据此判 FTS 弱命中 = skip）。
///
/// BETA-15B-3 A-3 FTS 置信度路由的核心信号函数；详 docs/superpowers/specs/2026-06-23-beta-15b-3a3-fts-confidence-routing-design.md。
#[must_use]
pub fn jaccard_overlap_by_path(a: &[SearchResult], b: &[SearchResult], k: usize) -> f64 {
    let set_a: std::collections::HashSet<&std::path::Path> =
        a.iter().take(k).map(|r| r.path.as_path()).collect();
    let set_b: std::collections::HashSet<&std::path::Path> =
        b.iter().take(k).map(|r| r.path.as_path()).collect();
    let inter = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        {
            inter as f64 / union as f64
        }
    }
}
```

- [ ] **Step 4：跑通过测试**

```
cargo test -p locifind-result-normalizer jaccard 2>&1 | tail -15
```
Expected: 5 个 jaccard_* 测试全 pass。

- [ ] **Step 5：三件套验证门**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 0 failed、clippy 0 warning、fmt 净。

- [ ] **Step 6：提交**

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-3 task 1：result-normalizer 加 jaccard_overlap_by_path + 5 单测"
```

---

## Task 2：`RouteVerdict` + `fuse_rrf_with_fts_routing` wrapper + 两个常量

**Goal:** 加路由 wrapper——Jaccard < 阈值时只把 vec_list 喂 `fuse_rrf`、否则两 list 都喂；返回 `(MergedResult, RouteVerdict)`；两个常量提供单一默认源。

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`（加 struct/wrapper/两常量；fuse_rrf 不动）

**注**：本 task 阈值用**占位初值** `0.30`；Task 7 sweep 后 bake 真 t\*。

- [ ] **Step 1：写失败测试（wrapper 5 例 + 等价性 1 例）**

加到 `packages/result-normalizer/src/lib.rs` 测试模块（紧跟 Task 1 加的 jaccard 测试后）：

```rust
    #[test]
    fn wrapper_high_overlap_keeps_fts_arm() {
        // Jaccard = 1.0 > threshold 0.5 → 不跳 FTS；fuse_rrf 接两 list
        let fts = vec![
            result("/a", BackendKind::NativeIndex, MatchType::Filename),
            result("/b", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let vec = vec![
            result("/a", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/b", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        let (out, verdict) = fuse_rrf_with_fts_routing(fts, vec, DEFAULT_RRF_K, 10.0, 0.5);
        assert!(!verdict.skipped_fts, "高重叠不应跳 FTS");
        assert!((verdict.jaccard - 1.0).abs() < f64::EPSILON);
        assert!((verdict.threshold - 0.5).abs() < f64::EPSILON);
        assert_eq!(out.len(), 2, "两 list 都喂应有两条结果");
        // sources 应含 NativeIndex 和 SemanticIndex
        assert!(out[0]
            .sources
            .iter()
            .any(|s| matches!(s, BackendKind::NativeIndex)));
        assert!(out[0]
            .sources
            .iter()
            .any(|s| matches!(s, BackendKind::SemanticIndex)));
    }

    #[test]
    fn wrapper_low_overlap_skips_fts_arm() {
        // Jaccard = 0.0 < threshold 0.3 → 跳 FTS；只 vec_list 喂 fuse_rrf
        let fts = vec![result("/x", BackendKind::NativeIndex, MatchType::Filename)];
        let vec = vec![result("/a", BackendKind::SemanticIndex, MatchType::Semantic)];
        let (out, verdict) = fuse_rrf_with_fts_routing(fts, vec, DEFAULT_RRF_K, 10.0, 0.3);
        assert!(verdict.skipped_fts, "无重叠应跳 FTS");
        assert!(verdict.jaccard.abs() < f64::EPSILON);
        assert_eq!(out.len(), 1, "跳 FTS 后只剩 vec 的 1 条");
        assert_eq!(out[0].result.path, std::path::Path::new("/a"));
        // sources 不应含 NativeIndex
        assert!(
            out[0]
                .sources
                .iter()
                .all(|s| !matches!(s, BackendKind::NativeIndex)),
            "跳 FTS 后 sources 不应含 NativeIndex"
        );
    }

    #[test]
    fn wrapper_both_empty_returns_empty() {
        let (out, verdict) =
            fuse_rrf_with_fts_routing(vec![], vec![], DEFAULT_RRF_K, 10.0, 0.3);
        assert!(out.is_empty(), "双空应返回空");
        // 双空 Jaccard 退化 = 0.0 < 0.3 → skipped_fts = true（行为良定义）
        assert!(verdict.skipped_fts);
        assert!(verdict.jaccard.abs() < f64::EPSILON);
    }

    #[test]
    fn wrapper_threshold_one_always_skips() {
        // threshold=1.0 → 只有完全重叠才不跳；本例半重叠 = 1/3 < 1.0 → 跳 FTS
        let fts = vec![
            result("/a", BackendKind::NativeIndex, MatchType::Filename),
            result("/b", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let vec = vec![
            result("/a", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/c", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        let (out, verdict) = fuse_rrf_with_fts_routing(fts, vec, DEFAULT_RRF_K, 10.0, 1.0);
        assert!(verdict.skipped_fts, "threshold=1.0、Jaccard<1.0 → 跳");
        // 跳 FTS → 等价 vec-only：2 条
        assert_eq!(out.len(), 2);
        let paths: Vec<&std::path::Path> =
            out.iter().map(|m| m.result.path.as_path()).collect();
        assert!(paths.contains(&std::path::Path::new("/a")));
        assert!(paths.contains(&std::path::Path::new("/c")));
        assert!(!paths.contains(&std::path::Path::new("/b")), "FTS 独有 /b 不应在");
    }

    #[test]
    fn wrapper_threshold_zero_never_skips() {
        // threshold=0.0 → 任何重叠（≥0.0）都不跳；FTS 总参与
        let fts = vec![result("/x", BackendKind::NativeIndex, MatchType::Filename)];
        let vec = vec![result("/a", BackendKind::SemanticIndex, MatchType::Semantic)];
        let (out, verdict) = fuse_rrf_with_fts_routing(fts, vec, DEFAULT_RRF_K, 10.0, 0.0);
        assert!(!verdict.skipped_fts, "threshold=0.0 → FTS 总参与");
        assert_eq!(out.len(), 2, "两 list 都喂");
    }

    #[test]
    fn wrapper_equivalence_to_fuse_rrf_when_not_skipping() {
        // Jaccard 高于阈值时，wrapper 等价于 fuse_rrf 直调
        let fts = vec![
            result("/a", BackendKind::NativeIndex, MatchType::Filename),
            result("/b", BackendKind::NativeIndex, MatchType::Filename),
        ];
        let vec = vec![
            result("/a", BackendKind::SemanticIndex, MatchType::Semantic),
            result("/b", BackendKind::SemanticIndex, MatchType::Semantic),
        ];
        // wrapper 路径：threshold=0.0 必不跳
        let (wrap_out, _) =
            fuse_rrf_with_fts_routing(fts.clone(), vec.clone(), DEFAULT_RRF_K, 10.0, 0.0);
        // 直调 fuse_rrf 路径
        let direct_out = fuse_rrf(vec![fts, vec], DEFAULT_RRF_K, 10.0);
        assert_eq!(wrap_out.len(), direct_out.len());
        for (w, d) in wrap_out.iter().zip(direct_out.iter()) {
            assert_eq!(w.result.path, d.result.path, "wrapper 与直调顺序应一致");
            assert!(
                (w.result.score.unwrap_or(0.0) - d.result.score.unwrap_or(0.0)).abs()
                    < f64::EPSILON,
                "wrapper 与直调 score 应一致"
            );
        }
    }
```

- [ ] **Step 2：跑失败测试确认 wrapper 未定义**

```
cargo test -p locifind-result-normalizer wrapper 2>&1 | tail -15
```
Expected: `error[E0425]: cannot find function fuse_rrf_with_fts_routing` 等。

- [ ] **Step 3：实现 RouteVerdict + 两常量 + wrapper**

在 `packages/result-normalizer/src/lib.rs` 中、`DEFAULT_SEMANTIC_WEIGHT` 常量后（约第 94 行后）加两个常量：

```rust
/// FTS/VEC top-K Jaccard 路由的默认阈值（< 阈值时跳过 FTS 臂、hybrid 退化为纯向量）。
/// BETA-15B-3 A-3 sweep 后 bake；详 docs/reviews/semantic-recall-quality-baseline.md A-3 调优记录节。
pub const DEFAULT_FTS_JACCARD_THRESHOLD: f64 = 0.30;

/// 路由 Jaccard 计算的 top-K 截断窗口；与评测 `TOP_K` 一致。
pub const DEFAULT_FTS_ROUTING_TOP_K: usize = 10;
```

紧跟 `jaccard_overlap_by_path`（Task 1 加）之后加 `RouteVerdict` + wrapper：

```rust
/// 路由判定副产物，便于评测/badge/调试消费。
/// 本 cycle 暂存预留（生产 wiring 透传到 FanoutOutcome）；BETA-15B-5 可解释 v1 badge 槽位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    /// 是否跳过 FTS 臂（true = Jaccard < threshold、hybrid 退化为纯向量）。
    pub skipped_fts: bool,
    /// 实测 Jaccard 重叠度。
    pub jaccard: f64,
    /// 当时使用的阈值（便于事后审计）。
    pub threshold: f64,
}

/// 加路由的 RRF 融合 wrapper：FTS/VEC 两臂分别传入，
/// `jaccard_overlap_by_path` < `jaccard_threshold` 时跳过 FTS 臂（hybrid 退化为纯向量）。
///
/// `jaccard_threshold` ∉ [0, 1] → `debug_assert!` + clamp 到 [0, 1]。
/// `fuse_rrf` 本身不动；wrapper 只决定 fts_list 是否进入 N 列表融合。
///
/// 与 [`fuse_rrf`] 等价性：当 Jaccard ≥ 阈值时，wrapper 结果完全等价 `fuse_rrf(vec![fts, vec], k, weight)`。
#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts_list: Vec<SearchResult>,
    vec_list: Vec<SearchResult>,
    rrf_k: f64,
    semantic_weight: f64,
    jaccard_threshold: f64,
) -> (Vec<MergedResult>, RouteVerdict) {
    debug_assert!(
        (0.0..=1.0).contains(&jaccard_threshold),
        "jaccard_threshold 须在 [0, 1]"
    );
    let threshold = jaccard_threshold.clamp(0.0, 1.0);
    let jaccard = jaccard_overlap_by_path(&fts_list, &vec_list, DEFAULT_FTS_ROUTING_TOP_K);
    let skipped_fts = jaccard < threshold;

    let merged = if skipped_fts {
        fuse_rrf(vec![vec_list], rrf_k, semantic_weight)
    } else {
        fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight)
    };

    (
        merged,
        RouteVerdict {
            skipped_fts,
            jaccard,
            threshold,
        },
    )
}
```

- [ ] **Step 4：跑通过测试**

```
cargo test -p locifind-result-normalizer 2>&1 | tail -20
```
Expected: 既有 fuse_rrf 测试 + 5 jaccard + 6 wrapper 测试全 pass。

- [ ] **Step 5：三件套验证门**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 0 failed、clippy 0、fmt 净。

- [ ] **Step 6：提交**

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-3 task 2：result-normalizer 加 RouteVerdict + fuse_rrf_with_fts_routing wrapper + 两常量 + 6 单测"
```

---

## Task 3：评测 `arms.rs` 加 `hybrid_routed_rank` helper

**Goal:** 评测层有平行 helper 调路由 wrapper、返回有序 doc_id 列表，供 `score_case` 跑第四臂。

**Files:**
- Modify: `packages/evals/src/semantic_quality/arms.rs`（加 helper + 4 单测；既有 hybrid_rank 不动）

- [ ] **Step 1：写失败测试**

加到 `packages/evals/src/semantic_quality/arms.rs` 末尾 `#[cfg(test)] mod tests` 块内（紧跟 `hybrid_fuses_both_arms_and_weights_semantic` 测试后）：

```rust
    #[test]
    fn hybrid_routed_high_overlap_uses_both_arms() {
        // 两臂 top-K 全重叠 → 不跳 FTS → 与 hybrid_rank 同效
        let fts = vec!["d_a".to_owned(), "d_b".to_owned()];
        let vec = vec!["d_a".to_owned(), "d_b".to_owned()];
        let ranked = hybrid_routed_rank(&fts, &vec, 0.5, 10.0, 60.0);
        for id in ["d_a", "d_b"] {
            assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
        }
    }

    #[test]
    fn hybrid_routed_low_overlap_skips_fts() {
        // 全不重叠 → Jaccard=0.0 < 0.3 → 跳 FTS → 只剩 vec 结果
        let fts = vec!["d_x".to_owned(), "d_y".to_owned()];
        let vec = vec!["d_a".to_owned(), "d_b".to_owned()];
        let ranked = hybrid_routed_rank(&fts, &vec, 0.3, 10.0, 60.0);
        assert_eq!(ranked, vec!["d_a".to_owned(), "d_b".to_owned()]);
    }

    #[test]
    fn hybrid_routed_threshold_zero_never_skips() {
        // threshold=0.0 → 总不跳 → 与 hybrid_rank 等价
        let fts = vec!["d_x".to_owned()];
        let vec = vec!["d_a".to_owned()];
        let routed = hybrid_routed_rank(&fts, &vec, 0.0, 10.0, 60.0);
        let direct = hybrid_rank(&fts, &vec, 10.0, 60.0);
        assert_eq!(routed, direct);
    }

    #[test]
    fn hybrid_routed_both_empty_returns_empty() {
        let ranked = hybrid_routed_rank(&[], &[], 0.5, 10.0, 60.0);
        assert!(ranked.is_empty());
    }
```

- [ ] **Step 2：跑失败测试**

```
cargo test -p locifind-evals hybrid_routed 2>&1 | tail -15
```
Expected: `error[E0425]: cannot find function hybrid_routed_rank`。

- [ ] **Step 3：实现 `hybrid_routed_rank`**

在 `packages/evals/src/semantic_quality/arms.rs` 中、`hybrid_rank` 函数之后（约第 109 行后、`#[cfg(test)]` 前）加：

```rust
/// 加 FTS 置信度路由的 hybrid 臂：FTS top-K 与 VEC top-K Jaccard < `jaccard_threshold`
/// 时跳过 FTS（hybrid 退化为纯向量）。喂 **生产 wrapper** `fuse_rrf_with_fts_routing`，
/// `semantic_weight`/`k`/`jaccard_threshold` 即生产路由的三个旋钮。
#[must_use]
pub fn hybrid_routed_rank(
    fts: &[String],
    vec: &[String],
    jaccard_threshold: f64,
    semantic_weight: f64,
    k: f64,
) -> Vec<String> {
    let fts_results = to_results(fts, BackendKind::NativeIndex, MatchType::Content);
    let vec_results = to_results(vec, BackendKind::SemanticIndex, MatchType::Semantic);
    let (merged, _verdict) = locifind_result_normalizer::fuse_rrf_with_fts_routing(
        fts_results,
        vec_results,
        k,
        semantic_weight,
        jaccard_threshold,
    );
    merged.into_iter().map(|m| m.result.id).collect()
}
```

- [ ] **Step 4：跑通过测试**

```
cargo test -p locifind-evals --lib semantic_quality::arms 2>&1 | tail -15
```
Expected: 既有 4 个 + 新增 4 个共 8 个测试 pass。

- [ ] **Step 5：三件套验证门**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 6：提交**

```bash
git add packages/evals/src/semantic_quality/arms.rs
git commit -m "BETA-15B-3 A-3 task 3：evals arms 加 hybrid_routed_rank helper + 4 单测"
```

---

## Task 4：`report.rs` 加 HYBR 字段 + `score_case` 接 `jaccard_threshold`

**Goal:** `CaseScores` 和 `BucketAgg` 加 `hybrid_routed_recall` / `hybrid_routed_ndcg` 字段；`score_case` 加 `jaccard_threshold` 参数、跑第四臂；`aggregate` 同步加 HYBR 均值。

**Files:**
- Modify: `packages/evals/src/semantic_quality/report.rs`（CaseScores/BucketAgg/score_case/aggregate + 既有 score_case_runs_three_arms 测试调整 + 新增 score_case_routed 测试）

- [ ] **Step 1：加 HYBR 字段到 `CaseScores` 和 `BucketAgg`**

在 `packages/evals/src/semantic_quality/report.rs:11-20` 的 `CaseScores` struct 末尾加两字段：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct CaseScores {
    pub id: String,
    pub bucket: String,
    pub fts_recall: f64,
    pub vec_recall: f64,
    pub hybrid_recall: f64,
    pub fts_ndcg: f64,
    pub vec_ndcg: f64,
    pub hybrid_ndcg: f64,
    pub hybrid_routed_recall: f64,
    pub hybrid_routed_ndcg: f64,
}
```

同样改 `BucketAgg`（第 23-33 行）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BucketAgg {
    pub bucket: String,
    pub n: usize,
    pub fts_recall: f64,
    pub vec_recall: f64,
    pub hybrid_recall: f64,
    pub fts_ndcg: f64,
    pub vec_ndcg: f64,
    pub hybrid_ndcg: f64,
    pub hybrid_routed_recall: f64,
    pub hybrid_routed_ndcg: f64,
}
```

- [ ] **Step 2：改 `score_case` 接 `jaccard_threshold` 参数 + 跑第四臂**

在 `packages/evals/src/semantic_quality/report.rs` 第 36-46 行的 `score_case` 签名加一个参数（在 `top_k` 之前）：

```rust
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn score_case(
    case: &SemanticCase,
    corpus: &[SemanticDoc],
    vectors: &VectorCache,
    floor: f32,
    weight: f64,
    k_rrf: f64,
    jaccard_threshold: f64,
    top_k: usize,
) -> CaseScores {
```

函数体里 `let hybrid = hybrid_rank(...)` 后加一行（第 61 行后）：

```rust
    let hybrid = hybrid_rank(&fts, &vec, weight, k_rrf);
    let hybrid_routed = super::arms::hybrid_routed_rank(&fts, &vec, jaccard_threshold, weight, k_rrf);
```

`CaseScores` 字段填充（第 63-72 行）末尾加两字段：

```rust
    CaseScores {
        id: case.id.clone(),
        bucket: case.bucket.clone(),
        fts_recall: recall_at_k(&fts, &relevant_set, top_k),
        vec_recall: recall_at_k(&vec, &relevant_set, top_k),
        hybrid_recall: recall_at_k(&hybrid, &relevant_set, top_k),
        fts_ndcg: ndcg_at_k(&fts, &grades, top_k),
        vec_ndcg: ndcg_at_k(&vec, &grades, top_k),
        hybrid_ndcg: ndcg_at_k(&hybrid, &grades, top_k),
        hybrid_routed_recall: recall_at_k(&hybrid_routed, &relevant_set, top_k),
        hybrid_routed_ndcg: ndcg_at_k(&hybrid_routed, &grades, top_k),
    }
```

- [ ] **Step 3：改 `aggregate` 加 HYBR 均值**

第 94-103 行 `agg_for` 的 `BucketAgg` 构造末尾加两字段：

```rust
        BucketAgg {
            bucket: name.to_owned(),
            n,
            fts_recall: mean(&|s| s.fts_recall),
            vec_recall: mean(&|s| s.vec_recall),
            hybrid_recall: mean(&|s| s.hybrid_recall),
            fts_ndcg: mean(&|s| s.fts_ndcg),
            vec_ndcg: mean(&|s| s.vec_ndcg),
            hybrid_ndcg: mean(&|s| s.hybrid_ndcg),
            hybrid_routed_recall: mean(&|s| s.hybrid_routed_recall),
            hybrid_routed_ndcg: mean(&|s| s.hybrid_routed_ndcg),
        }
```

- [ ] **Step 4：改既有 `aggregate_means_per_bucket_and_overall` 测试 + `score_case_runs_three_arms` 测试**

`aggregate_means_per_bucket_and_overall`（第 123-153 行）：两个 `CaseScores` 字面量末尾加 `hybrid_routed_recall: 0.0, hybrid_routed_ndcg: 0.0,`：

```rust
            CaseScores {
                id: "a".into(),
                bucket: "crosslang".into(),
                fts_recall: 0.0,
                vec_recall: 1.0,
                hybrid_recall: 1.0,
                fts_ndcg: 0.0,
                vec_ndcg: 1.0,
                hybrid_ndcg: 1.0,
                hybrid_routed_recall: 1.0,
                hybrid_routed_ndcg: 1.0,
            },
            CaseScores {
                id: "b".into(),
                bucket: "crosslang".into(),
                fts_recall: 0.0,
                vec_recall: 0.0,
                hybrid_recall: 0.5,
                fts_ndcg: 0.0,
                vec_ndcg: 0.0,
                hybrid_ndcg: 0.5,
                hybrid_routed_recall: 0.5,
                hybrid_routed_ndcg: 0.5,
            },
```

`score_case_runs_three_arms`（第 156-192 行）：改 `score_case` 调用加 `jaccard_threshold` 参数（在 60.0 与 10 之间，传 0.30）：

```rust
        let s = score_case(&case, &corpus, &vc, 0.30, 2.0, 60.0, 0.30, 10);
```

并改函数名为 `score_case_runs_four_arms` 反映新结构；末尾加一行断言 HYBR 字段存在（值可不严格）：

```rust
    #[test]
    fn score_case_runs_four_arms() {
        // ...（既有内容不变，只改 score_case 调用 + 加 assertion + 函数名）
        let s = score_case(&case, &corpus, &vc, 0.30, 2.0, 60.0, 0.30, 10);
        assert_eq!(s.id, "c1");
        assert!(s.vec_recall > 0.0, "向量臂应召回 d1");
        // HYBR 臂存在性（jaccard_threshold=0.30 + Jaccard=0 → 跳 FTS → 等价 vec_recall）
        assert!((s.hybrid_routed_recall - s.vec_recall).abs() < 1e-9, "跳 FTS 后 HYBR 应等于 VEC");
    }
```

- [ ] **Step 5：跑改完测试通过**

```
cargo test -p locifind-evals --lib semantic_quality::report 2>&1 | tail -15
```
Expected: aggregate_means_per_bucket_and_overall + score_case_runs_four_arms 两个 pass。

- [ ] **Step 6：三件套验证门（注意：binary/gate 还未改、`semantic_quality.rs` 和 `semantic_quality_gate.rs` 调 score_case 旧签名会编译错——但这是预期，下一 Task 修）**

先单独跑 lib 测试确认本 task 的库内一致性：
```
cargo test -p locifind-evals --lib 2>&1 | tail -20
```
Expected: lib 测试 pass。

`cargo test --workspace` 会因下游调用方未改而编译失败——**这是预期的，Task 5 修 binary、Task 8 修 gate**。本 task **不要求 workspace 测试全通过**。

```
cargo fmt --all -- --check
```

- [ ] **Step 7：提交**

```bash
git add packages/evals/src/semantic_quality/report.rs
git commit -m "BETA-15B-3 A-3 task 4：report.rs 加 HYBR 字段 + score_case 接 jaccard_threshold（binary/gate 调用方下一刀修）"
```

---

## Task 5：`semantic_quality` binary 加 `--jaccard-threshold` flag + HYBR 列

**Goal:** 评测 binary 接受 `--jaccard-threshold=<f64>` 参数（默认 = `DEFAULT_FTS_JACCARD_THRESHOLD`），表格新增 HYBR_R/HYBR_N 两列。

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs`（Cli struct + main + print_table + cli_tests）

- [ ] **Step 1：写失败测试**

在 `packages/evals/src/bin/semantic_quality.rs` 末尾 `mod cli_tests` 块（第 168-185 行）内加两个新测试：

```rust
    #[test]
    fn jaccard_threshold_flag_parses() {
        let cli = Cli::parse_from(["semantic_quality", "--jaccard-threshold", "0.4"]);
        assert!((cli.jaccard_threshold - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn jaccard_threshold_defaults_to_const() {
        use locifind_result_normalizer::DEFAULT_FTS_JACCARD_THRESHOLD;
        let cli = Cli::parse_from(["semantic_quality"]);
        assert!((cli.jaccard_threshold - DEFAULT_FTS_JACCARD_THRESHOLD).abs() < f64::EPSILON);
    }
```

- [ ] **Step 2：跑失败测试**

```
cargo test -p locifind-evals --bin semantic_quality 2>&1 | tail -15
```
Expected: 编译错 `no field jaccard_threshold on type Cli`。

- [ ] **Step 3：加 `jaccard_threshold` 字段到 Cli + import 常量**

文件顶部 import（第 14 行）改为：

```rust
use locifind_result_normalizer::{
    DEFAULT_FTS_JACCARD_THRESHOLD, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

`Cli` struct 在 `semantic_weight` 字段后（第 35 行附近）加：

```rust
    /// FTS 置信度路由 Jaccard 阈值（默认 = result-normalizer::DEFAULT_FTS_JACCARD_THRESHOLD）。
    /// sweep 用：`--jaccard-threshold=0.5` 等。BETA-15B-3 A-3。
    #[arg(long, default_value_t = DEFAULT_FTS_JACCARD_THRESHOLD)]
    jaccard_threshold: f64,
```

- [ ] **Step 4：main 把 `cli.jaccard_threshold` 透传给 `score_case`**

第 90-103 行的 `score_case` 调用改为：

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
                cli.jaccard_threshold,
                TOP_K,
            )
        })
        .collect();
```

- [ ] **Step 5：`print_table` 加 HYBR 两列**

第 43-62 行 `print_table` 改为（表头多 2 列 + 数据行多 2 列）：

```rust
fn print_table(aggs: &[BucketAgg]) {
    println!(
        "{:<18} {:>3} | {:>7} {:>7} | {:>7} {:>7} | {:>7} {:>7} | {:>7} {:>7}",
        "bucket", "n", "FTS_R", "FTS_N", "VEC_R", "VEC_N", "HYB_R", "HYB_N", "HYBR_R", "HYBR_N"
    );
    println!("{}", "-".repeat(98));
    for a in aggs {
        println!(
            "{:<18} {:>3} | {:>7.3} {:>7.3} | {:>7.3} {:>7.3} | {:>7.3} {:>7.3} | {:>7.3} {:>7.3}",
            a.bucket,
            a.n,
            a.fts_recall,
            a.fts_ndcg,
            a.vec_recall,
            a.vec_ndcg,
            a.hybrid_recall,
            a.hybrid_ndcg,
            a.hybrid_routed_recall,
            a.hybrid_routed_ndcg
        );
    }
}
```

- [ ] **Step 6：跑通过测试**

```
cargo test -p locifind-evals --bin semantic_quality 2>&1 | tail -15
```
Expected: 4 个 cli_tests（既有 2 + 新增 2）全 pass。

- [ ] **Step 7：三件套验证门（gate 仍编译错——预期，Task 8 修）**

```
cargo test -p locifind-evals --lib 2>&1 | tail -10
cargo test -p locifind-evals --bins 2>&1 | tail -10
cargo fmt --all -- --check
```

- [ ] **Step 8：提交**

```bash
git add packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-3 A-3 task 5：semantic_quality binary 加 --jaccard-threshold flag + 表格 HYBR 两列"
```

---

## Task 6：手动 sweep + 选 t\*

**Goal:** 跑 6 个 Jaccard 阈值 sweep、控制对照核验（t=0.0 = VEC、t=1.0 = HYB）、选满足 spec §2.2 红线的 t\*；产物用于 Task 7 bake。本 task **不入仓**，但 sweep 表 trace 在 Task 10 写入 baseline 报告。

**Files:**（无文件改动；产出在终端表格 + 本任务 step 4 的人工选择 trace）

- [ ] **Step 1：先确认 baseline 已就绪（vectors.json / baseline.json 提交在仓内）**

```
ls -la packages/evals/fixtures/semantic-recall/vectors.json packages/evals/fixtures/semantic-recall/baseline.json
```
Expected: 两文件存在（A-2 已 commit）。

- [ ] **Step 2：跑 sweep（6 个阈值；W 固定 10.0 = DEFAULT_SEMANTIC_WEIGHT）**

```bash
for t in 0.0 0.10 0.20 0.30 0.50 1.0; do
  echo "============================================="
  echo "=== threshold=$t ==="
  echo "============================================="
  cargo run -p locifind-evals --bin semantic_quality -- --jaccard-threshold=$t
done | tee /tmp/sweep-jaccard.log
```

- [ ] **Step 3：控制对照核验**

读 `/tmp/sweep-jaccard.log`，验证：
- `threshold=0.0` 时 **HYBR ≡ VEC**（任何 jaccard ≥ 0.0 → 不跳，但…等等：spec §5 说 t=0.0 应使 HYBR ≡ VEC，因为「总跳过」？）

**澄清**：路由判定 `skipped_fts = jaccard < threshold`。
- t=0.0 → `jaccard < 0.0` 永假 → 永不跳 → **HYBR ≡ HYB**
- t=1.0 → `jaccard < 1.0` 几乎永真（除非完全重叠）→ 几乎总跳 → **HYBR ≈ VEC**（exact-name 桶完全重叠时不跳，仍 HYB）

修正：验证
- t=0.0 时 OVERALL HYBR_R/HYBR_N 应**等于** OVERALL HYB_R/HYB_N（永不跳）
- t=1.0 时除 exact-name 外 HYBR 应**逼近** VEC（几乎总跳）

若实测违背、说明 wrapper 实现 bug，回到 Task 2。

- [ ] **Step 4：选 t\***

按 spec §5 步骤 2 四条标准人工读表：
1. exact-name HYBR_R = 1.000（所有 t 都应满足）
2. OVERALL HYBR_N 最大（目标 ≥ 0.864 = 纯向量基准）
3. crosslang HYBR_N 最大（目标 ≥ 0.700）
4. 其他桶 HYBR_N ≥ HYB baseline 同桶（不退步）

在终端记下选定 t\* 值（如 t\*=0.20）和理由（如「在 t=0.20 时 OVERALL HYBR_N=0.870、crosslang HYBR_N=0.715，其他桶都 ≥ baseline」），供 Task 7 bake 和 Task 10 写报告。

**异常分支（按 spec §5 处理）**：
- 没有 t 满足 (2)/(3) → 选满足 (1)/(4) 且 OVERALL 最大的 t；记报告诚实暴露「路由触达天花板」、下 cycle 抓手 = 更强信号
- 任何 t 让 (1) 跌破 → abort、回 Task 2 复查 wrapper

- [ ] **Step 5：把完整 sweep 表 + 选定 t\* 临时存到 `/tmp/sweep-summary.md`**

不入仓，但为 Task 10 写 baseline 报告留草稿：

```bash
cat > /tmp/sweep-summary.md << 'EOF'
# A-3 Sweep 结果（2026-06-23）

## Sweep 全表（W=10.0 固定）

| t | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | synonym HYBR_N | concept HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (=HYB) | <填> | <填> | ... |
| 0.10 | ... |
| 0.20 | ... |
| 0.30 | ... |
| 0.50 | ... |
| 1.0 | ... |

## 控制对照
- t=0.0：HYBR ≡ HYB（永不跳）— ✓ 验证通过
- t=1.0：HYBR ≈ VEC（除 exact-name 完全重叠桶外几乎总跳）— ✓ 验证通过

## 选定 t\* = <填>
理由：<填>
EOF
```

填好后供 Task 10 引用。

---

## Task 7：bake `DEFAULT_FTS_JACCARD_THRESHOLD = t*` + rewrite baseline.json

**Goal:** 用 Task 6 选出的 t\* 替换占位 `0.30`、更新 doc-comment、跑 `--write-baseline` 重写 baseline.json（含 HYBR 字段）。

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`（DEFAULT_FTS_JACCARD_THRESHOLD 值 + doc-comment）
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`（重写、加 HYBR 字段）

- [ ] **Step 1：替换 `DEFAULT_FTS_JACCARD_THRESHOLD` 占位值**

`packages/result-normalizer/src/lib.rs` 中找到 Task 2 加的：
```rust
pub const DEFAULT_FTS_JACCARD_THRESHOLD: f64 = 0.30;
```
改成 Task 6 选定的 t\*（举例：若 t\*=0.20，改为 `0.20`）。**用 Task 6 step 4 记录的真值**。

同时更新 doc-comment（在该常量上方）：

```rust
/// FTS/VEC top-K Jaccard 路由的默认阈值（< 阈值时跳过 FTS 臂、hybrid 退化为纯向量）。
/// BETA-15B-3 A-3 sweep 选定 t*=<填实际值>；详 docs/reviews/semantic-recall-quality-baseline.md A-3 调优记录节。
pub const DEFAULT_FTS_JACCARD_THRESHOLD: f64 = <填实际值>;
```

- [ ] **Step 2：跑 `--write-baseline` 重写 baseline.json**

```
cargo run -p locifind-evals --bin semantic_quality -- --write-baseline
```
Expected: stderr 输出 `已写 baseline.json（6 桶含 OVERALL）`。

- [ ] **Step 3：验证 baseline.json 新增 HYBR 字段、HYB 字段保留**

```
cat packages/evals/fixtures/semantic-recall/baseline.json | grep -E 'hybrid_recall|hybrid_routed_recall|hybrid_ndcg|hybrid_routed_ndcg' | head -10
```
Expected: 每个桶都同时出现 `hybrid_recall` / `hybrid_ndcg` 和 `hybrid_routed_recall` / `hybrid_routed_ndcg`。

- [ ] **Step 4：跑既有 result-normalizer 单测确认新常量值未破坏 wrapper 等价性**

```
cargo test -p locifind-result-normalizer 2>&1 | tail -10
```
Expected: 全 pass（wrapper 测试用具体阈值不依赖常量值；jaccard/fuse_rrf 测试本身不依赖 threshold const）。

- [ ] **Step 5：三件套验证门**

```
cargo fmt --all -- --check
cargo test -p locifind-result-normalizer
cargo test -p locifind-evals --lib
cargo test -p locifind-evals --bins
```
Expected: 全 pass。

注：`cargo test --workspace` 仍会因 gate 调用方未改报错——Task 8 修。

- [ ] **Step 6：提交**

```bash
git add packages/result-normalizer/src/lib.rs packages/evals/fixtures/semantic-recall/baseline.json
git commit -m "BETA-15B-3 A-3 task 7：bake DEFAULT_FTS_JACCARD_THRESHOLD = <t*> + rewrite baseline.json 含 HYBR 字段"
```

---

## Task 8：`semantic_quality_gate` 加 HYBR 红线断言

**Goal:** 回归门双守护——HYB 老红线保留 + 新增 HYBR 各桶红线（exact-name HYBR_R=1.0 硬断言 + 各桶不退步 + crosslang/OVERALL HYBR_N 目标）。

**Files:**
- Modify: `packages/evals/tests/semantic_quality_gate.rs`

- [ ] **Step 1：改 `score_case` 调用加新参数**

`packages/evals/tests/semantic_quality_gate.rs:12` import 加常量：

```rust
use locifind_result_normalizer::{
    DEFAULT_FTS_JACCARD_THRESHOLD, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT,
};
```

`score_case` 调用（第 39-49 行）加 `DEFAULT_FTS_JACCARD_THRESHOLD` 参数：

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
                DEFAULT_FTS_JACCARD_THRESHOLD,
                TOP_K,
            )
        })
        .collect();
```

- [ ] **Step 2：HYBR 红线断言（4a/4b/4c/4d）**

紧跟既有 exact-name 硬断言（第 71-80 行）之后加：

```rust
    // BETA-15B-3 A-3 红线：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 达 spec 目标（或诚实降级）。
    let baseline: Vec<BucketAgg> =
        serde_json::from_str(&std::fs::read_to_string(fixt("baseline.json")).unwrap()).unwrap();
    // 4a：exact-name HYBR_R = 1.0（硬红线）
    let exact_name_now = aggs
        .iter()
        .find(|a| a.bucket == "exact-name")
        .expect("exact-name 桶");
    assert!(
        (exact_name_now.hybrid_routed_recall - 1.0).abs() < EPS,
        "exact-name HYBR_R 跌破 1.0：{} ← A-3 硬红线",
        exact_name_now.hybrid_routed_recall
    );
    // 4b：各桶 HYBR_N ≥ HYB baseline 同桶（不退步）+ HYBR_R 不退步 HYB baseline
    for bucket in ["synonym", "concept", "crosslang", "content-not-name", "exact-name", "OVERALL"] {
        if let (Some(base), Some(now)) =
            (find_bucket(&baseline, bucket), find_bucket(&aggs, bucket))
        {
            assert!(
                now.hybrid_routed_ndcg + EPS >= base.hybrid_ndcg,
                "{bucket} HYBR_N 退步 HYB baseline：{:.3} < {:.3}",
                now.hybrid_routed_ndcg,
                base.hybrid_ndcg
            );
            assert!(
                now.hybrid_routed_recall + EPS >= base.hybrid_recall,
                "{bucket} HYBR_R 退步 HYB baseline：{:.3} < {:.3}",
                now.hybrid_routed_recall,
                base.hybrid_recall
            );
        }
    }
    // 4c / 4d：HYBR_N 守 baseline 锁定水位（A-3 sweep 实测 t* 时所得；新 baseline 已写、此处与 baseline.hybrid_routed_* 相比即可）
    for bucket in ["crosslang", "OVERALL"] {
        if let (Some(base), Some(now)) =
            (find_bucket(&baseline, bucket), find_bucket(&aggs, bucket))
        {
            assert!(
                now.hybrid_routed_ndcg + EPS >= base.hybrid_routed_ndcg,
                "{bucket} HYBR_N 跌破新 baseline：{:.3} < baseline {:.3}",
                now.hybrid_routed_ndcg,
                base.hybrid_routed_ndcg
            );
        }
    }
```

- [ ] **Step 3：跑 gate 通过**

```
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -15
```
Expected: `1 passed` pass。

- [ ] **Step 4：三件套验证门**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass（lib + bins + tests 都通过）。

- [ ] **Step 5：提交**

```bash
git add packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-3 A-3 task 8：semantic_quality_gate 加 HYBR 红线断言（exact-name=1.0 + 各桶不退 HYB + HYBR 自锁）"
```

---

## Task 9：生产 `run_fanout_merge_rrf` 改 wrapper（按 BackendKind 分离两臂）

**Goal:** 生产 hybrid 路径统一走 `fuse_rrf_with_fts_routing`；按 `BackendKind` 拆分两臂列表；`RouteVerdict` 透传到 `FanoutOutcome` 暂存为后续 BETA-15B-5 badge 槽位（本 cycle 不画 UI）。

**Files:**
- Modify: `packages/harness/src/fanout_merge.rs`（`run_fanout_merge_rrf` 函数体 + `FanoutOutcome` 加字段 + 既有测试调整）

- [ ] **Step 1：`FanoutOutcome` 加 `route_verdict` 字段**

在 `packages/harness/src/fanout_merge.rs` 顶部 `FanoutOutcome` struct 定义处（用 `grep -n "struct FanoutOutcome" packages/harness/src/fanout_merge.rs` 确认行号；通常在前 60 行）加字段：

```rust
pub struct FanoutOutcome {
    pub total: usize,
    pub sources_queried: Vec<BackendKind>,
    pub errors: Vec<(String, String)>,
    /// FTS 置信度路由判定（仅 `run_fanout_merge_rrf` 路径填充；其他路径为 `None`）。
    /// BETA-15B-3 A-3 暂存预留为后续 BETA-15B-5 badge 槽位。
    pub route_verdict: Option<locifind_result_normalizer::RouteVerdict>,
}
```

并改文件顶部 import：
```rust
use locifind_result_normalizer::{
    fuse_rrf_with_fts_routing, merge_results, MergedResult, RouteVerdict,
    DEFAULT_FTS_JACCARD_THRESHOLD, DEFAULT_RRF_K,
};
```

（如果 `fuse_rrf` 仍被同文件其它代码使用，保留 import；否则移除以免 unused 警告。grep `fuse_rrf` 在本文件用法决定。）

- [ ] **Step 2：所有现有 `FanoutOutcome { total, sources_queried, errors }` 字面量都补 `route_verdict: None`**

```
grep -n "FanoutOutcome {" packages/harness/src/fanout_merge.rs
```
列出所有出现处（包括 `run_fanout_merge_rrf` 内、`run_fanout_merge_with_fallback` 内、`run_fanout_merge` 内、测试用例等），每处加 `route_verdict: None`。

例：原 `run_fanout_merge_with_fallback` 末尾返回（约第 200 行）：
```rust
    FanoutOutcome {
        total: fb.total,
        sources_queried,
        errors,
    }
```
改为：
```rust
    FanoutOutcome {
        total: fb.total,
        sources_queried,
        errors,
        route_verdict: None,
    }
```

`run_fanout_merge_rrf` 末尾会在 Step 4 一并改。

- [ ] **Step 3：改 `run_fanout_merge_rrf` 函数体——按 BackendKind 分离 fts/vec 两臂列表**

`packages/harness/src/fanout_merge.rs:111-146` 函数体改为：

```rust
    // BETA-15B-3 A-3：按 BackendKind 分离 FTS 臂（任何非 SemanticIndex）与 VEC 臂（SemanticIndex），
    // 喂 fuse_rrf_with_fts_routing wrapper；Jaccard 重叠 < 阈值时跳过 FTS 臂。
    let mut fts_list: Vec<SearchResult> = Vec::new();
    let mut vec_list: Vec<SearchResult> = Vec::new();
    let mut sources_queried: Vec<BackendKind> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    for tool in backends {
        if cancel.is_cancelled() {
            break;
        }
        let tool_id = tool.id().to_owned();
        let backend_kind = tool.capability().backend_kind;
        match tool.search_expanded(expanded, cancel.clone()).await {
            Err(err) => errors.push((tool_id, err.to_string())),
            Ok(mut stream) => {
                if let Some(kind) = backend_kind {
                    if !sources_queried.contains(&kind) {
                        sources_queried.push(kind);
                    }
                }
                let mut list: Vec<SearchResult> = Vec::new();
                while let Some(item) = stream.next().await {
                    if cancel.is_cancelled() {
                        break;
                    }
                    match item {
                        Ok(result) => list.push(result),
                        Err(err) => {
                            errors.push((tool_id.clone(), err.to_string()));
                            break;
                        }
                    }
                }
                // 按 backend_kind 归口：SemanticIndex → vec_list；其它 → fts_list（生产 hybrid
                // 路径几乎总是 1 NativeIndex + 1 SemanticIndex；多 same-kind backend 时 extend
                // 串起来，rank 信息会模糊但属罕见情形，路由信号仍可计算）。
                if matches!(backend_kind, Some(BackendKind::SemanticIndex)) {
                    vec_list.extend(list);
                } else {
                    fts_list.extend(list);
                }
            }
        }
    }

    let (merged, verdict) = fuse_rrf_with_fts_routing(
        fts_list,
        vec_list,
        DEFAULT_RRF_K,
        semantic_weight,
        DEFAULT_FTS_JACCARD_THRESHOLD,
    );
    let total = merged.len();
    for m in merged {
        on_result(m);
    }

    FanoutOutcome {
        total,
        sources_queried,
        errors,
        route_verdict: Some(verdict),
    }
```

注：`RouteVerdict` 已在 Step 1 加到 import 列表；如未导入 `BackendKind`，确认顶部 import 已含（搜 `use locifind_search_backend` 块）。

- [ ] **Step 4：既有测试调整**

`packages/harness/src/fanout_merge.rs` 测试模块（约第 207 行后）里所有 `run_fanout_merge_rrf` 调用本身签名不变（5 参）；但若有 `FanoutOutcome { ... }` 字面量比对（unlikely），加 `route_verdict: None` 或 `Some(verdict)`。检查测试模块：

```
grep -n "FanoutOutcome {" packages/harness/src/fanout_merge.rs
```

对每处补 `route_verdict` 字段。生产代码 `FanoutOutcome` 字面量在 Step 2 已统一加；测试里若有比对 `outcome.route_verdict` 用 Option 比对即可（多数测试无需断言此字段）。

新增 1 个生产 wiring 测试（紧跟既有 `rrf_fuses_ranks_semantic_weighted` 测试约第 411 行后）。用真 helper `tool(id, kind, Script::Results(...))`：

```rust
    #[test]
    fn rrf_low_overlap_route_skips_fts_and_reports_verdict() {
        // 两个 backend 一 FTS（NativeIndex）一 VEC（SemanticIndex），结果完全不重叠 → Jaccard=0.0 → 路由应跳 FTS
        let backends = vec![
            tool(
                "search.local",
                BackendKind::NativeIndex,
                Script::Results(vec![result_at(
                    "/fts_only",
                    BackendKind::NativeIndex,
                    MatchType::Filename,
                )]),
            ),
            tool(
                "search.semantic",
                BackendKind::SemanticIndex,
                Script::Results(vec![result_at(
                    "/vec_only",
                    BackendKind::SemanticIndex,
                    MatchType::Content,
                )]),
            ),
        ];
        let mut got: Vec<MergedResult> = Vec::new();
        let outcome = block_on(run_fanout_merge_rrf(
            &backends,
            &ExpandedSearchIntent::identity(intent()),
            CancellationToken::new(),
            &mut |m| got.push(m),
            10.0,
        ));
        let verdict = outcome
            .route_verdict
            .expect("RRF 路径应填 route_verdict");
        assert!(verdict.skipped_fts, "Jaccard=0.0 < 阈值 → 应跳 FTS");
        assert_eq!(got.len(), 1, "跳 FTS 后只剩 vec 结果");
        assert_eq!(
            got[0].result.path,
            std::path::PathBuf::from("/vec_only")
        );
    }
```

注：`tool` / `result_at` / `intent` / `Script::Results` 测试 helper 已在现测试模块（fanout_merge.rs:248-315）。若新增 import 需要，参考既有 `rrf_fuses_ranks_semantic_weighted` 测试块（约第 362-411 行）。

- [ ] **Step 5：跑 harness 测试通过**

```
cargo test -p locifind-harness 2>&1 | tail -15
```
Expected: 既有测试 + 新增 wiring 测试 pass。

- [ ] **Step 6：三件套验证门**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 0 failed、clippy 0 warning（含 `route_verdict` 字段被新生产代码消费，无 dead_code 警告）、fmt 净。

如有 `route_verdict` 暂未消费的 `unused` 警告（`FanoutOutcome { ..., route_verdict: None, .. }` 调用方丢弃此字段），用 `#[allow(dead_code)]` 或保持原状（Option 字段未消费不触发 warn）。

- [ ] **Step 7：byte-equal 闸门验证（路由是融合层加法，理论 byte-equal；本步骤确证）**

```
cargo run -p locifind-evals --bin evals -- --json --kind parser-only --version v0.5 > /tmp/a3-v05.json
cargo run -p locifind-evals --bin evals -- --json --kind parser-only --version v0.9 > /tmp/a3-v09.json
```
对照 main 上 A-2 末态的 parser-only 输出（如未保存，用 `git stash` 比对）：
```
cargo run -p locifind-evals --bin evals -- --json --kind parser-only --version v0.5 | grep -E '"passed"|"total"|"partial"|"failed"' | head -5
```
Expected: `passed=473 / total=500` v0.5；`passed=877 / total=1000` v0.9（parser-only 不受融合层加法影响）。

- [ ] **Step 8：提交**

```bash
git add packages/harness/src/fanout_merge.rs
git commit -m "BETA-15B-3 A-3 task 9：fanout_merge 改 wrapper（按 BackendKind 分两臂） + FanoutOutcome.route_verdict 透传"
```

---

## Task 10：A-3 调优记录追加 baseline 报告 + 总验收

**Goal:** 把 Task 6 sweep 结果（全表 + 选定 t\* + 控制对照）+ 与 A-2 HYB baseline 对照 + spec §2.2 红线达成情况 写入 `docs/reviews/semantic-recall-quality-baseline.md`；跑总验收三件套 + 实测 + byte-equal。

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（在文件末尾「调优记录（2026-06-22，BETA-15B-3 A-2）」节后追加新节）

- [ ] **Step 1：追加「A-3 调优记录（2026-06-23）」节**

在 `docs/reviews/semantic-recall-quality-baseline.md` 文件末尾追加：

```markdown
## A-3 调优记录（2026-06-23，BETA-15B-3 A-3）

> 起于 A-2 诚实边界「纯抬 weight 见底、OVERALL 0.854 距纯向量 0.864 差 0.010、crosslang 0.649 距 0.726 差 0.077」。本轮做 FTS 置信度路由（Jaccard < 阈值跳 FTS 臂），路由必要性证据见 A-2 节末尾。

### Sweep 全表（W=10.0 固定）

`jaccard_threshold=t` 各桶 hybrid_routed recall / nDCG（FTS/VEC/HYB 不随 t 变，省）：

| t | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | concept HYBR_N | synonym HYBR_N | content-not-name HYBR_N |
| --- | --- | --- | --- | --- | --- | --- |
| 0.0 (=HYB 控制对照) | <Task6 step5 填> | ... |
| 0.10 | ... |
| 0.20 | ... |
| 0.30 | ... |
| 0.50 | ... |
| 1.0 (≈VEC 控制对照) | ... |

（其中 **t\* = <Task6 step4 选定值>**）

### 控制对照核验

- t=0.0：HYBR ≡ HYB（永不跳）— ✓ 实测：OVERALL HYBR_N = HYB baseline 0.854 一致
- t=1.0：HYBR ≈ VEC（除 exact-name 等完全重叠桶外几乎总跳）— ✓ 实测：OVERALL HYBR_N ≈ VEC 0.864（exact-name 完全重叠 → HYBR_R 仍 1.0）

### 选定 t\*

`DEFAULT_FTS_JACCARD_THRESHOLD = <填实际值>`（bake 进 `result-normalizer/src/lib.rs:<填行号>`；生产 `run_fanout_merge_rrf` 经 `fuse_rrf_with_fts_routing` wrapper 调用、不暴露 UI、内部启发式）

理由（按 spec §2.2 硬约束顺序）：
1. **exact-name HYBR_R = 1.000** ✅ 硬红线守住（所有 t 都满足；两臂在精确名查询天然高重叠不触发跳过）。
2. **OVERALL HYBR_N = <填>**（sweep 最大 / 是否达 spec §2.2 (4d) 0.864 目标 → 写实测结果与差距 / 达成或降级）。
3. **crosslang HYBR_N = <填>**（vs A-2 baseline 0.649 +<填> Δ / 是否达 spec §2.2 (4c) 0.700 目标 → 达成或降级）。
4. **其他桶 HYBR_N 全部 ≥ HYB baseline 同桶** ✅ 验证不退步。

### 与 A-2 baseline (HYB) 对照

| 桶 | A-2 HYB_N | A-3 HYBR_N | Δ |
| --- | --- | --- | --- |
| OVERALL | 0.854 | <填> | <填> |
| crosslang | 0.649 | <填> | <填> |
| concept | 0.819 | <填> | <填> |
| content-not-name | 0.930 | <填> | <填> |
| synonym | 0.905 | <填> | <填> |
| exact-name | 1.000 | <填> | 0.000（红线） |

### 路由必要性证据复盘 / 下 cycle 抓手

- 若 (4c)(4d) 达成：路由有效、A-3 收束、下 cycle 转新特性。
- 若 (4c)(4d) 任一未达：写实测「sweep 任 t 均不达 0.X、纯路由触达天花板」+ 下 cycle 抓手候选（更强信号如 query 语种 / 文档语种 / VEC 余弦绝对分数 / 更大 embedding 模型 / 评测集扩量降同语言 g1 比例）。

### 已待跟进项更新

- 「待跟进 1（floor 字面量漂移）」未消化——本刀只做路由，不涉 floor。
- 「待跟进 2（语料 108 < 150）」未消化——本刀不扩量。

### 验收红线达成情况（spec §2.2）

| 条目 | 目标 | 实测 | 达成 |
| --- | --- | --- | --- |
| (1) cargo test workspace 0 failed | 0 | <Task10 step3 填> | ✅/⚠️ |
| (2) clippy/fmt/tsc 净 | 净 | <填> | ✅/⚠️ |
| (3) evals parser-only byte-equal | v0.5=473、v0.9=877 不变 | <填> | ✅/⚠️ |
| (4a) exact-name HYBR_R = 1.0 | 1.0 | <填> | ✅/⚠️ |
| (4b) 各桶 HYBR ≥ HYB baseline 同桶 | ≥ | <填> | ✅/⚠️ |
| (4c) crosslang HYBR_N ≥ 0.700 | ≥ 0.700 | <填> | ✅/⚠️ |
| (4d) OVERALL HYBR_N ≥ 0.864 | ≥ 0.864 | <填> | ✅/⚠️ |
| (5) wrapper clamp[0,1] + 双空良定义 | 守护 | task 2 单测覆盖 | ✅ |
```

把每个 `<填>` 替换为 Task 6 sweep 实测 / Task 7 bake 值 / Task 10 step 3-4 实测结果。

- [ ] **Step 2：总验收三件套**

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass。

- [ ] **Step 3：实测**

```
cargo run -p locifind-evals --bin semantic_quality
```
读输出表格、对照 spec §2.2 红线表填 Step 1 「验收红线达成情况」表。

```
cargo test -p locifind-evals --test semantic_quality_gate
```
Expected: `1 passed`。

- [ ] **Step 4：byte-equal 闸门确证**

```
cargo run -p locifind-evals --bin evals -- --json --kind parser-only --version v0.5 2>/dev/null | jq '.summary | {passed, partial, failed}'
cargo run -p locifind-evals --bin evals -- --json --kind parser-only --version v0.9 2>/dev/null | jq '.summary | {passed, partial, failed}'
```
Expected: v0.5 `passed=473`；v0.9 `passed=877`（与 A-2 末态完全一致）。

- [ ] **Step 5：前端 tsc + vite build 净（确认 RouteVerdict 元数据未透到前端类型；本 cycle 不画 UI）**

```
cd apps/desktop && pnpm install && pnpm tsc --noEmit && pnpm vite build
```
Expected: tsc 0 error、vite build 净。

- [ ] **Step 6：提交**

```bash
git add docs/reviews/semantic-recall-quality-baseline.md
git commit -m "BETA-15B-3 A-3 task 10：baseline 报告追加 A-3 调优记录 + 总验收过"
```

---

## Self-Review（plan 完成后自查）

### 1. Spec 覆盖

| Spec §6 task | Plan task |
|---|---|
| Spec 1（result-normalizer wrapper + 9 单测） | **Plan 1+2**（拆为 Jaccard + Wrapper 两步 TDD） |
| Spec 2（arms.rs helper） | **Plan 3** |
| Spec 3（binary CLI flag） | **Plan 5** |
| Spec 4（report.rs HYBR 字段 + baseline.json schema） | **Plan 4**（schema 改加字段）+ **Plan 7**（rewrite 实际 baseline.json） |
| Spec 5（manual sweep） | **Plan 6** |
| Spec 6（bake 常量） | **Plan 7** |
| Spec 7（production fanout wiring） | **Plan 9** |
| Spec 8（baseline.json rewrite） | **Plan 7**（合并进 bake 一步） |
| Spec 9（gate HYBR 断言） | **Plan 8** |
| Spec 10（A-3 调优记录 baseline 报告） | **Plan 10** |

10 spec tasks → 10 plan tasks，1:1 覆盖（Plan 1+2 = Spec 1 拆 TDD；Plan 7 = Spec 6+8 合并）。

### 2. Placeholder scan

- Plan Task 2 用占位阈值 `0.30`——Task 7 sweep 后 bake 真 t\*。**有意空位**，标注清楚。
- Plan Task 6 `<填>` / Plan Task 10 `<填>` 是 sweep 后实测数据填入位——**有意空位**，结构已就绪。
- Plan Task 7 `<填实际值>` / `<填行号>` 同上。
- 无其它 TBD / TODO / 待补 / 类似模糊措辞。

### 3. Type 一致性

- `RouteVerdict` 在 Task 2 定义 → Task 3 helper 解构 `(_, _verdict)` → Task 9 透传 `FanoutOutcome.route_verdict`：字段名 `skipped_fts`/`jaccard`/`threshold` 三处一致。
- `jaccard_overlap_by_path` 签名 `(a: &[SearchResult], b: &[SearchResult], k: usize) -> f64`：Task 1 定义、Task 2 wrapper 调用、Task 6 控制对照逻辑引用——参数顺序一致。
- `fuse_rrf_with_fts_routing` 签名 `(fts_list, vec_list, rrf_k, semantic_weight, jaccard_threshold)`：Task 2 定义、Task 3 helper 调用、Task 9 fanout 调用——参数顺序一致。
- `hybrid_routed_recall` / `hybrid_routed_ndcg` 字段名：Task 4 schema、Task 5 binary 表格、Task 7 baseline.json、Task 8 gate 断言、Task 10 报告——五处一致。
- `DEFAULT_FTS_JACCARD_THRESHOLD` 常量：Task 2 定义占位、Task 7 bake、Task 5 binary 默认、Task 8 gate 调用、Task 9 fanout 调用——五处一致。
- `BackendKind::SemanticIndex` 判定：Task 9 用 `matches!(backend_kind, Some(BackendKind::SemanticIndex))`——单点判定，与 `result-normalizer/lib.rs:110` 现 fuse_rrf 加权逻辑同源。

---

## Execution Handoff

Plan 完工保存在 `docs/superpowers/plans/2026-06-23-beta-15b-3a3-fts-confidence-routing.md`。

两个执行选项：

**1. Subagent-Driven（推荐）** — 每 task 一个 subagent + 两阶段审（spec 审 + 质量审）+ 主线 review checkpoint；与 A-2 同款，已有 7 个 task 模板成功经验。

**2. Inline Execution** — 在当前会话顺序跑 task、批量 checkpoint。适合 plan 较短或 Task 间状态紧密耦合。

哪个？
