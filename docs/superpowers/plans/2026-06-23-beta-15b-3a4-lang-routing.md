# BETA-15B-3 A-4：query 语种检测 + 跨语种 vec hit 路由 实施 plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 A-3 wrapper 内部信号从 Jaccard 升级为 query_lang + 跨语种 vec hit 计数；wrapper 名 / RouteVerdict / HYBR baseline 字段名 / gate 红线架构全部保留；评测 sweep + bake → 生产 query-lang-only 接入。

**Architecture:** 自写 CJK Unicode ratio 三态二阈检测（Zh/En/Mixed）作为 `result-normalizer::lang` 内部模块；wrapper 签名 `threshold: f64` → `(query_lang, max_cross_lang_hits)`；Mixed 进保守降级永不跳；评测 `to_results` 取 `name=title` 让合成集 doc 也带 CJK 信号；生产 `run_fanout_merge_rrf` 入口检测 query_lang 传 wrapper。

**Tech Stack:** Rust 1.80（MSRV）、纯 std（lang 检测零依赖）、保留 A-3 的 `jaccard_overlap_by_path` 工具函数为下 cycle 信号组合预留。

**Spec:** [docs/superpowers/specs/2026-06-23-beta-15b-3a4-lang-routing-design.md](../specs/2026-06-23-beta-15b-3a4-lang-routing-design.md)

---

## File Structure

**Create:**
- `packages/result-normalizer/src/lang.rs` — `Lang` enum + `detect_lang` fn

**Modify:**
- `packages/result-normalizer/src/lib.rs` — `RouteVerdict` 字段升级、`fuse_rrf_with_fts_routing` 签名升级、`DEFAULT_MAX_CROSS_LANG_HITS` 常量、`pub mod lang;` 导出
- `packages/evals/src/semantic_quality/arms.rs` — `to_results` 加 `corpus` 参 + `name=title`、`hybrid_routed_rank` 改参
- `packages/evals/src/semantic_quality/report.rs` — `score_case` 入口 detect_lang、参数改名
- `packages/evals/src/bin/semantic_quality.rs` — CLI flag `--max-cross-lang-hits`、表头不变
- `packages/evals/fixtures/semantic-recall/baseline.json` — HYBR 字段 rewrite + `max_cross_lang_hits_bake` 元数据
- `packages/evals/tests/semantic_quality_gate.rs` — 4 条红线断言数值替换
- `packages/harness/src/fanout_merge.rs` — `run_fanout_merge_rrf` 签名扩 `query: &str` + 入口 `detect_lang`、wrapper 调用改新签名
- `apps/desktop/src-tauri/src/search/fanout.rs` — call-site 透传 `query: &str`
- `apps/desktop/src-tauri/src/search/tests.rs` — 测试 call-site 透传
- `packages/harness/src/lib.rs` — 若 re-export 表面 `run_fanout_merge_rrf` 签名需同步
- `docs/reviews/semantic-recall-quality-baseline.md` — 追加 A-4 调优记录节

**Carry forward unchanged:**
- `packages/result-normalizer/src/lib.rs::jaccard_overlap_by_path` + 单测（保留为下 cycle 信号组合预留，标 `#[allow(dead_code)]`）

---

## Task 1: lang.rs 模块 + `detect_lang` 三态二阈检测

**Files:**
- Create: `packages/result-normalizer/src/lang.rs`
- Modify: `packages/result-normalizer/src/lib.rs:1` 加 `pub mod lang;`

- [ ] **Step 1.1：先写最小 stub 让 lib.rs 引用通过**

新建文件 `packages/result-normalizer/src/lang.rs`：

```rust
//! query 语种检测（CJK ratio 三态二阈、纯 std、零依赖）。
//! BETA-15B-3 A-4：替代 A-3 Jaccard 单维信号，对准 crosslang 桶失败模式。

/// query 语种三态。`Mixed` 进保守降级（路由不生效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
    Mixed,
}

/// CJK ratio 三态二阈检测：>0.6=Zh、<0.05=En、之间=Mixed。
/// 分母 = CJK chars + ASCII alphanumeric chars 总数；分母为 0 → Mixed（保守降级）。
/// CJK 覆盖范围：Unified Ideographs (U+4E00–U+9FFF)
/// + Compatibility (U+F900–U+FAFF) + Ext-A (U+3400–U+4DBF)。
#[must_use]
pub fn detect_lang(text: &str) -> Lang {
    let mut cjk = 0_usize;
    let mut alnum = 0_usize;
    for c in text.chars() {
        if is_cjk(c) {
            cjk += 1;
        } else if c.is_ascii_alphanumeric() {
            alnum += 1;
        }
    }
    let total = cjk + alnum;
    if total == 0 {
        return Lang::Mixed;
    }
    #[allow(clippy::cast_precision_loss)]
    let ratio = cjk as f64 / total as f64;
    if ratio > 0.6 {
        Lang::Zh
    } else if ratio < 0.05 {
        Lang::En
    } else {
        Lang::Mixed
    }
}

const fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF
        | 0xF900..=0xFAFF
        | 0x3400..=0x4DBF
    )
}
```

在 `packages/result-normalizer/src/lib.rs:1` 后追加：

```rust
pub mod lang;
```

- [ ] **Step 1.2：在 lang.rs 写 8 条单测**

把以下测试块追加到 `packages/result-normalizer/src/lang.rs` 末尾：

```rust
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn pure_chinese_query_is_zh() {
        assert_eq!(detect_lang("年假规定与远程办公细则"), Lang::Zh);
    }

    #[test]
    fn pure_english_query_is_en() {
        assert_eq!(detect_lang("annual leave policy"), Lang::En);
    }

    #[test]
    fn mostly_chinese_with_one_english_term_is_zh() {
        // CJK ratio > 0.6
        assert_eq!(detect_lang("iPhone 备份指南文档"), Lang::Zh);
    }

    #[test]
    fn english_query_with_small_punctuation_is_en() {
        // 小于 0.05 CJK → En
        assert_eq!(detect_lang("git push origin main"), Lang::En);
    }

    #[test]
    fn balanced_mix_is_mixed() {
        // 「qwen 调优」3 ASCII + 2 CJK = ratio 2/5 = 0.4 → Mixed
        assert_eq!(detect_lang("qwen 调优"), Lang::Mixed);
    }

    #[test]
    fn empty_query_is_mixed() {
        // 分母 0 → 保守降级
        assert_eq!(detect_lang(""), Lang::Mixed);
    }

    #[test]
    fn whitespace_and_punct_only_is_mixed() {
        // 既无 CJK 也无 alnum → 分母 0 → 保守降级
        assert_eq!(detect_lang("   ... !!"), Lang::Mixed);
    }

    #[test]
    fn cjk_ext_a_is_zh() {
        // U+3400 Ext-A 区
        let s: String = std::iter::once(char::from_u32(0x3400).unwrap()).collect();
        assert_eq!(detect_lang(&s), Lang::Zh);
    }
}
```

- [ ] **Step 1.3：跑测试，验证全过**

Run: `cargo test -p locifind-result-normalizer --lib lang::`
Expected: 8 passed; 0 failed

- [ ] **Step 1.4：fmt + clippy 验证**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -D warnings`
Expected: 0 warning、0 fmt diff

- [ ] **Step 1.5：commit**

```bash
git add packages/result-normalizer/src/lang.rs packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-4 task 1：result-normalizer 加 lang.rs (Lang enum + detect_lang) + 8 单测"
```

---

## Task 2: `RouteVerdict` 升级 + `fuse_rrf_with_fts_routing` 内部信号升级

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs:95-103` 常量改名 + 新增
- Modify: `packages/result-normalizer/src/lib.rs:188-198` `RouteVerdict` 字段升级
- Modify: `packages/result-normalizer/src/lib.rs:200-255` wrapper 签名 + 内部决策树升级
- 既有 wrapper 单测（lib.rs 末尾 `mod tests`）改 lang 信号；新增 4 单测覆盖三态分支

- [ ] **Step 2.1：替换常量声明**

把 `packages/result-normalizer/src/lib.rs:95-103` 替换为：

```rust
/// FTS/VEC top-K 跨语种 vec hit 计数路由的默认阈值（≥ 此 count 时跳过 FTS）。
/// BETA-15B-3 A-4 sweep 选定（详 docs/reviews/semantic-recall-quality-baseline.md A-4 调优记录节）。
/// 占位为 usize::MAX（永不跳过、spec §5 保守降级）；task 7 sweep 后 bake 实际值。
pub const DEFAULT_MAX_CROSS_LANG_HITS: usize = usize::MAX;

/// 路由计数的 top-K 截断窗口；与评测 `TOP_K` 一致。
pub const DEFAULT_FTS_ROUTING_TOP_K: usize = 10;
```

把原 `DEFAULT_FTS_JACCARD_THRESHOLD` 常量删掉。

- [ ] **Step 2.2：升级 `RouteVerdict` 字段**

把 `packages/result-normalizer/src/lib.rs:188-198` 替换为：

```rust
/// 路由判定副产物，便于评测/badge/调试消费。
/// 本 cycle 暂存预留（生产 wiring 透传到 FanoutOutcome）；BETA-15B-5 可解释 v1 badge 槽位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    /// 是否跳过 FTS 臂（true = cross_lang_hits ≥ max、hybrid 退化为纯向量）。
    pub skipped_fts: bool,
    /// 检测到的 query 语种。
    pub query_lang: crate::lang::Lang,
    /// vec top-K 中检测到的跨语种 hit 计数（query_lang ≠ name 检测出的 lang 且 ≠ Mixed）。
    pub cross_lang_hits: usize,
    /// 当时使用的阈值（便于事后审计）。
    pub max_cross_lang_hits: usize,
}
```

- [ ] **Step 2.3：升级 wrapper 签名 + 内部决策树**

把 `packages/result-normalizer/src/lib.rs:200-255`（`fuse_rrf_with_fts_routing` 整个函数）替换为：

```rust
/// 加路由的 RRF 融合 wrapper：FTS/VEC 两臂分别传入，
/// query 单语种且 vec top-K 含跨语种 hit ≥ `max_cross_lang_hits` 时跳过 FTS 臂
/// （hybrid 退化为纯向量）。query_lang = Mixed 进保守降级永不跳。
///
/// `fuse_rrf` 本身不动；wrapper 只决定 `fts_list` 是否进入 N 列表融合。
///
/// **任一臂空时不跳过 FTS**：无路由信号，保留兜底；`skipped_fts = false`、`cross_lang_hits = 0`。
///
/// 与 [`fuse_rrf`] 等价性：当不跳 FTS 时，wrapper 结果完全等价 `fuse_rrf(vec![fts, vec], k, weight)`。
#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts_list: Vec<SearchResult>,
    vec_list: Vec<SearchResult>,
    rrf_k: f64,
    semantic_weight: f64,
    query_lang: crate::lang::Lang,
    max_cross_lang_hits: usize,
) -> (Vec<MergedResult>, RouteVerdict) {
    // 任一臂空 → 无路由信号；不跳过 FTS（preserve 一臂兜底）。
    if fts_list.is_empty() || vec_list.is_empty() {
        let merged = fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight);
        return (
            merged,
            RouteVerdict {
                skipped_fts: false,
                query_lang,
                cross_lang_hits: 0,
                max_cross_lang_hits,
            },
        );
    }

    // Mixed query 保守降级：永不跳。
    if query_lang == crate::lang::Lang::Mixed {
        let merged = fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight);
        return (
            merged,
            RouteVerdict {
                skipped_fts: false,
                query_lang,
                cross_lang_hits: 0,
                max_cross_lang_hits,
            },
        );
    }

    // 数 vec top-K 中跨语种 hit（detect_lang(name) ≠ query_lang 且 ≠ Mixed）。
    let top = vec_list.len().min(DEFAULT_FTS_ROUTING_TOP_K);
    let cross_lang_hits = vec_list[..top]
        .iter()
        .filter(|r| {
            let hit_lang = crate::lang::detect_lang(&r.name);
            hit_lang != query_lang && hit_lang != crate::lang::Lang::Mixed
        })
        .count();

    let skipped_fts = cross_lang_hits >= max_cross_lang_hits;

    let merged = if skipped_fts {
        fuse_rrf(vec![vec_list], rrf_k, semantic_weight)
    } else {
        fuse_rrf(vec![fts_list, vec_list], rrf_k, semantic_weight)
    };

    (
        merged,
        RouteVerdict {
            skipped_fts,
            query_lang,
            cross_lang_hits,
            max_cross_lang_hits,
        },
    )
}
```

- [ ] **Step 2.4：保留 Jaccard 工具函数为后续预留**

在 `packages/result-normalizer/src/lib.rs:172` 的 `jaccard_overlap_by_path` 函数声明前加 `#[allow(dead_code)]` 属性、把 doc 注释更新指 A-4：

```rust
/// 计算两个有序 `SearchResult` 列表 **top-K 的 `path` 集合** Jaccard 重叠度。
///
/// `|A ∩ B| / |A ∪ B|`，A/B = 各自前 `k` 个 result 的 path 集合；空集 ∪ 空集 = 0.0。
///
/// BETA-15B-3 A-3 引入；A-4 wrapper 改为 lang 信号后此函数留作下 cycle 信号组合预留
/// （Jaccard + lang OR/AND 复合）；仍由 `result-normalizer` 公开 API。
#[allow(dead_code)]
#[must_use]
pub fn jaccard_overlap_by_path(a: &[SearchResult], b: &[SearchResult], k: usize) -> f64 {
    ...
}
```

实际上由于 `jaccard_overlap_by_path` 是 `pub` 暴露 + 有单测调它，编译器不会判 dead；但 attr 留语义注释。如果 clippy 报「unused」则去掉 attr。

- [ ] **Step 2.5：改既有 wrapper 单测 + 加新 4 单测**

定位 `packages/result-normalizer/src/lib.rs` 末尾 `mod tests` 中既有 6 个 `fuse_rrf_with_fts_routing` 单测（参考 `c647c60` commit 引入的测试）：

```
- fuse_rrf_with_fts_routing_high_overlap_uses_fts ...
- fuse_rrf_with_fts_routing_low_overlap_skips_fts ...
- fuse_rrf_with_fts_routing_both_empty_returns_empty ...
- fuse_rrf_with_fts_routing_threshold_one_always_skips ...
- fuse_rrf_with_fts_routing_threshold_zero_never_skips ...
- jaccard_threshold_equal_does_not_skip （task 2 fixup 加的）
```

把 6 个全部改名 + 改参 + 改语义（保留控制对照精神）：

```rust
use crate::lang::Lang;

#[test]
fn wrapper_en_query_high_cross_lang_skips_fts() {
    // En query + vec top-K 全 ZH name → cross_lang_hits=2 ≥ max=2 → 跳 FTS
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm = vec![
        result("/年假.md", BackendKind::SemanticIndex, MatchType::Semantic),
        result("/规章.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    let (out, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::En, 2);
    assert!(v.skipped_fts);
    assert_eq!(v.cross_lang_hits, 2);
    assert_eq!(out.len(), 2);
    assert!(out.iter().all(|m| m.result.path.to_string_lossy().contains(".md")));
}

#[test]
fn wrapper_en_query_low_cross_lang_does_not_skip() {
    // En query + vec top-K 全 EN name → cross_lang_hits=0 < max=2 → 不跳
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm = vec![
        result("/policy.md", BackendKind::SemanticIndex, MatchType::Semantic),
        result("/leave.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    let (out, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::En, 2);
    assert!(!v.skipped_fts);
    assert_eq!(v.cross_lang_hits, 0);
    assert!(out.iter().any(|m| m.result.path.to_string_lossy().contains("en.txt")));
}

#[test]
fn wrapper_mixed_query_never_skips() {
    // Mixed query → 保守降级，永不跳
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm = vec![
        result("/年假.md", BackendKind::SemanticIndex, MatchType::Semantic),
        result("/规章.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    let (out, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::Mixed, 1);
    assert!(!v.skipped_fts);
    assert_eq!(v.cross_lang_hits, 0);
    assert_eq!(v.query_lang, Lang::Mixed);
}

#[test]
fn wrapper_empty_arm_does_not_skip() {
    // vec 空 → empty-arm guard → 不跳、fuse_rrf 兜底
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm: Vec<SearchResult> = vec![];
    let (out, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::En, 1);
    assert!(!v.skipped_fts);
    assert_eq!(v.cross_lang_hits, 0);
    assert_eq!(out.len(), 1);
}

#[test]
fn wrapper_max_usize_never_skips() {
    // max = usize::MAX → 永不跳（与 A-3 spec §5 降级值同义）
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm = vec![
        result("/年假.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    let (_, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::En, usize::MAX);
    assert!(!v.skipped_fts);
}

#[test]
fn wrapper_max_zero_always_skips_when_any_cross_lang() {
    // max = 0 → cross_lang_hits=1 ≥ 0 永真 → 跳
    let fts = vec![
        result("/en.txt", BackendKind::NativeIndex, MatchType::Content),
    ];
    let vec_arm = vec![
        result("/年假.md", BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    let (out, v) =
        fuse_rrf_with_fts_routing(fts, vec_arm, DEFAULT_RRF_K, 10.0, Lang::En, 0);
    assert!(v.skipped_fts);
    assert!(out.iter().all(|m| !m.result.path.to_string_lossy().contains("en.txt")));
}
```

把上面 6 个 test 完全替换原 6 个 `fuse_rrf_with_fts_routing` 单测。其它 `merge_results` / `fuse_rrf` 等单测**不动**。

- [ ] **Step 2.6：跑测试**

Run: `cargo test -p locifind-result-normalizer`
Expected: 全过、含新 6 个 wrapper 单测、含原 Jaccard / fuse_rrf / merge_results 单测

- [ ] **Step 2.7：fmt + clippy 验证**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -D warnings`
Expected: 0 warning、0 fmt diff

注意此时 evals/harness 调 wrapper 旧签名的调用方还没改、可能编译失败。**先 build 整 workspace 看具体爆点**：

Run: `cargo build --workspace 2>&1 | head -30`

爆出的具体调用方留 task 3-9 去改、本 task 验证只跑 `result-normalizer` 包内测试。

- [ ] **Step 2.8：commit**（此 commit 让 workspace 编不过、待 task 3-9 收拢）

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-4 task 2：result-normalizer 加 lang 信号 wrapper + RouteVerdict 字段升级 + 6 单测（workspace 暂编不过 task 3-9 收拢）"
```

---

## Task 3: 评测 arms.rs 改造（`to_results` 加 corpus + `hybrid_routed_rank` 改参）

**Files:**
- Modify: `packages/evals/src/semantic_quality/arms.rs:82-95` `to_results` 签名扩参
- Modify: `packages/evals/src/semantic_quality/arms.rs:115-132` `hybrid_routed_rank` 签名改
- Modify: 既有 5 个 `hybrid_routed_*` 单测改 lang 信号

- [ ] **Step 3.1：改 `to_results` 签名（name 取 title）**

把 `packages/evals/src/semantic_quality/arms.rs:82-95` 替换为：

```rust
/// 把有序 `doc_id` 列表包装成生产 `SearchResult`：
/// `path` 仍 `doc_id`（dedup key 不变）；`name` 取 corpus 中 doc.title（带自然语言 CJK 信号、
/// 供 wrapper 内部 detect_lang 用）；找不到 → fallback `doc_id`。
fn to_results(
    corpus: &[SemanticDoc],
    ids: &[String],
    source: BackendKind,
    mt: MatchType,
) -> Vec<SearchResult> {
    ids.iter()
        .map(|id| {
            let name = corpus
                .iter()
                .find(|d| d.doc_id == *id)
                .map(|d| d.title.clone())
                .unwrap_or_else(|| id.clone());
            SearchResult {
                id: id.clone(),
                path: PathBuf::from(id),
                name,
                source,
                match_type: mt,
                score: None,
                metadata: SearchResultMetadata::default(),
            }
        })
        .collect()
}
```

- [ ] **Step 3.2：改 `hybrid_routed_rank` 签名**

把 `packages/evals/src/semantic_quality/arms.rs:111-132` 替换为：

```rust
use locifind_result_normalizer::lang::Lang;

/// 加 lang 路由的 hybrid 臂：query 单语种 + vec top-K 中跨语种 hit ≥ `max_cross_lang_hits`
/// 时跳过 FTS（hybrid 退化为纯向量）。喂 **生产 wrapper** `fuse_rrf_with_fts_routing`，
/// `semantic_weight`/`k`/`max_cross_lang_hits` 即生产路由的三个旋钮，`query_lang` 由
/// caller 从 case.query 检测后传入。
#[must_use]
pub fn hybrid_routed_rank(
    corpus: &[SemanticDoc],
    fts: &[String],
    vec_ids: &[String],
    query_lang: Lang,
    max_cross_lang_hits: usize,
    semantic_weight: f64,
    k: f64,
) -> Vec<String> {
    let fts_results = to_results(corpus, fts, BackendKind::NativeIndex, MatchType::Content);
    let vec_results = to_results(corpus, vec_ids, BackendKind::SemanticIndex, MatchType::Semantic);
    let (merged, _verdict) = locifind_result_normalizer::fuse_rrf_with_fts_routing(
        fts_results,
        vec_results,
        k,
        semantic_weight,
        query_lang,
        max_cross_lang_hits,
    );
    merged.into_iter().map(|m| m.result.id).collect()
}
```

- [ ] **Step 3.3：同步同包内 `hybrid_rank` caller**

`hybrid_rank` 也用 `to_results`。把 `packages/evals/src/semantic_quality/arms.rs:97-109` 替换为：

```rust
/// hybrid 臂：FTS 臂(`NativeIndex`) + 向量臂(`SemanticIndex`) 喂**生产** `fuse_rrf`，
/// 取融合后有序 `doc_id`。`semantic_weight`/`k` 即生产融合的两个调优旋钮。
#[must_use]
pub fn hybrid_rank(
    corpus: &[SemanticDoc],
    fts: &[String],
    vec_ids: &[String],
    semantic_weight: f64,
    k: f64,
) -> Vec<String> {
    let lists = vec![
        to_results(corpus, fts, BackendKind::NativeIndex, MatchType::Content),
        to_results(corpus, vec_ids, BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    fuse_rrf(lists, k, semantic_weight)
        .into_iter()
        .map(|m| m.result.id)
        .collect()
}
```

- [ ] **Step 3.4：改既有 5 个 `hybrid_routed_*` 单测 + 既有 `hybrid_*` 单测**

定位 `packages/evals/src/semantic_quality/arms.rs:148` 起 `mod tests`：
- 既有 `hybrid_fuses_both_arms_and_weights_semantic` 改新 `hybrid_rank` 签名（加 corpus 参）
- 既有 `hybrid_routed_high_overlap_uses_both_arms` → 改成 `hybrid_routed_en_no_cross_lang_uses_both_arms`（En query + vec ids 内 corpus 找不到 → title fallback 为 doc_id 全 ASCII → cross_lang_hits=0 不跳）
- 既有 `hybrid_routed_low_overlap_skips_fts` → 改成 `hybrid_routed_en_cross_lang_skips_fts`（vec ids 在 corpus 中 doc.title 是中文 → cross_lang_hits 满足 → 跳）
- 既有 `hybrid_routed_threshold_zero_never_skips` → 改成 `hybrid_routed_max_usize_never_skips`
- 既有 `hybrid_routed_both_empty_returns_empty` 保留语义、改签名

替换示例（完整 5 测）：

```rust
#[test]
fn hybrid_fuses_both_arms_and_weights_semantic() {
    let corpus = vec![doc("d_both", "x"), doc("d_fts", "y"), doc("d_vec", "z")];
    let fts = vec!["d_both".to_owned(), "d_fts".to_owned()];
    let vec_ids = vec!["d_both".to_owned(), "d_vec".to_owned()];
    let ranked = hybrid_rank(&corpus, &fts, &vec_ids, 2.0, 60.0);
    assert_eq!(ranked.first().map(String::as_str), Some("d_both"));
    for id in ["d_both", "d_fts", "d_vec"] {
        assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
    }
}

#[test]
fn hybrid_routed_en_no_cross_lang_uses_both_arms() {
    // En query + vec doc.title 全 ASCII → cross_lang_hits=0 < max=1 → 不跳
    let corpus = vec![
        doc("d_a", "policy text"),
        doc("d_b", "leave guide"),
    ];
    let fts = vec!["d_a".to_owned()];
    let vec_ids = vec!["d_a".to_owned(), "d_b".to_owned()];
    let ranked = hybrid_routed_rank(&corpus, &fts, &vec_ids, Lang::En, 1, 10.0, 60.0);
    for id in ["d_a", "d_b"] {
        assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
    }
}

#[test]
fn hybrid_routed_en_cross_lang_skips_fts() {
    // En query + vec doc.title 全 ZH → cross_lang_hits=2 ≥ max=2 → 跳 FTS
    let corpus = vec![
        doc("d_zh1", "年假规定"),
        doc("d_zh2", "员工手册"),
        doc("d_fts", "fts only"),
    ];
    let fts = vec!["d_fts".to_owned()];
    let vec_ids = vec!["d_zh1".to_owned(), "d_zh2".to_owned()];
    let ranked = hybrid_routed_rank(&corpus, &fts, &vec_ids, Lang::En, 2, 10.0, 60.0);
    assert!(!ranked.contains(&"d_fts".to_owned()), "跳 FTS 后 d_fts 不在结果");
    assert!(ranked.contains(&"d_zh1".to_owned()));
}

#[test]
fn hybrid_routed_max_usize_never_skips() {
    // max=usize::MAX → 永不跳（与 hybrid_rank 等价 modulo Mixed/empty guard）
    let corpus = vec![doc("d_a", "policy"), doc("d_zh", "年假")];
    let fts = vec!["d_a".to_owned()];
    let vec_ids = vec!["d_zh".to_owned()];
    let routed =
        hybrid_routed_rank(&corpus, &fts, &vec_ids, Lang::En, usize::MAX, 10.0, 60.0);
    let direct = hybrid_rank(&corpus, &fts, &vec_ids, 10.0, 60.0);
    assert_eq!(routed, direct);
}

#[test]
fn hybrid_routed_both_empty_returns_empty() {
    let corpus = vec![];
    let ranked = hybrid_routed_rank(&corpus, &[], &[], Lang::En, 1, 10.0, 60.0);
    assert!(ranked.is_empty());
}
```

- [ ] **Step 3.5：跑测试**

Run: `cargo test -p evals --lib semantic_quality::arms::`
Expected: 全过

如果 `hybrid_*` 调用方（report.rs / bin / lib_tests）签名不匹配编译失败，先记下、task 4 起改。本 task 验证只包内 arms.rs 单测过。

- [ ] **Step 3.6：commit**

```bash
git add packages/evals/src/semantic_quality/arms.rs
git commit -m "BETA-15B-3 A-4 task 3：evals arms 加 corpus 参 + hybrid_routed_rank 改 lang 信号 + 单测改造"
```

---

## Task 4: 评测 report.rs 改造（`score_case` 入口 detect_lang + 参数改名）

**Files:**
- Modify: `packages/evals/src/semantic_quality/report.rs` `score_case` 入口 + 签名
- Modify: 既有 `hybrid_routed_*` 字段名保留不变（语义升级、字段名延续）

- [ ] **Step 4.1：改 `score_case` 签名 + 入口 detect_lang + call-site 同步**

当前 signature（[packages/evals/src/semantic_quality/report.rs:42-51](packages/evals/src/semantic_quality/report.rs:42)）：

```rust
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
    ...
    let hybrid = hybrid_rank(&fts, &vec, weight, k_rrf);
    let hybrid_routed =
        super::arms::hybrid_routed_rank(&fts, &vec, jaccard_threshold, weight, k_rrf);
    ...
}
```

整体替换为：

```rust
use locifind_result_normalizer::lang::detect_lang;

#[must_use]
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn score_case(
    case: &SemanticCase,
    corpus: &[SemanticDoc],
    vectors: &VectorCache,
    floor: f32,
    weight: f64,
    k_rrf: f64,
    max_cross_lang_hits: usize,
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

    let query_lang = detect_lang(&case.query);
    let fts = fts_rank(corpus, &case.query, POOL).unwrap_or_default();
    let empty = Vec::new();
    let qv = vectors.query_vectors.get(&case.id).unwrap_or(&empty);
    let vec = vector_rank(qv, &vectors.doc_vectors, floor, POOL);
    let hybrid = hybrid_rank(corpus, &fts, &vec, weight, k_rrf);
    let hybrid_routed = super::arms::hybrid_routed_rank(
        corpus,
        &fts,
        &vec,
        query_lang,
        max_cross_lang_hits,
        weight,
        k_rrf,
    );

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
}
```

`CaseScores` / `BucketAgg` 字段名（`hybrid_routed_recall` / `hybrid_routed_ndcg`）**完全不动**——spec 决策：字段名语义延续。

- [ ] **Step 4.2：跑测试**

Run: `cargo test -p evals --lib semantic_quality::report::`
Expected: 编译过、既有测试过（report 单测多围绕 nDCG/aggregation 计算、与信号无关）

如包间引用 score_case 签名爆错（CLI binary / gate test 调），task 5/8 收拢。

- [ ] **Step 4.3：commit**

```bash
git add packages/evals/src/semantic_quality/report.rs
git commit -m "BETA-15B-3 A-4 task 4：evals report score_case 入口 detect_lang + 参数 max_cross_lang_hits"
```

---

## Task 5: CLI flag `--max-cross-lang-hits`

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs` flag 名 + 字段类型

- [ ] **Step 5.1：定位 flag 声明**

Run: `rg -n "jaccard_threshold|jaccard-threshold" packages/evals/src/bin/semantic_quality.rs`

定位 clap derive 字段。

- [ ] **Step 5.2：改 flag**

把 `--jaccard-threshold` flag 改为 `--max-cross-lang-hits`：

```rust
/// 跨语种 vec hit 计数路由阈值；≥ 此值时跳过 FTS 臂。
/// `usize::MAX` ≈ 永不跳（与 A-3 spec §5 降级值同义）。
/// 默认 = `DEFAULT_MAX_CROSS_LANG_HITS`（task 7 sweep 后 bake）。
#[arg(long, default_value_t = locifind_result_normalizer::DEFAULT_MAX_CROSS_LANG_HITS)]
max_cross_lang_hits: usize,
```

把 `args.jaccard_threshold` 全部 grep 替换为 `args.max_cross_lang_hits`，并把传给 `score_case` 的位置同步：

Run: `rg -n "jaccard_threshold" packages/evals/src/bin/semantic_quality.rs`

逐处替换。

- [ ] **Step 5.3：保留表头 HYBR_R / HYBR_N 列名**

确认表头打印不动（spec 决策：HYBR 字段名延续）：

Run: `rg -n "HYBR_R|HYBR_N" packages/evals/src/bin/semantic_quality.rs`

输出 8-12 列项均不动。

- [ ] **Step 5.4：build + 试跑（看 CLI parse 过）**

Run: `cargo build -p evals --bin semantic_quality`
Expected: 编译过

Run: `cargo run -p evals --bin semantic_quality --release -- --help | head -30`
Expected: `--max-cross-lang-hits <USIZE>` 出现、默认值 18446744073709551615 (usize::MAX)

- [ ] **Step 5.5：commit**

```bash
git add packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-3 A-4 task 5：semantic_quality binary --max-cross-lang-hits flag"
```

---

## Task 6: 手动 sweep + 控制对照

**这是评测/数据 task，不产代码 commit。** 产物 = `sweep-lang.log`（可不入仓、报告里贴关键行）+ 选定的 N*。

- [ ] **Step 6.1：在 spec §5 调优工作流跑 sweep**

```bash
mkdir -p /tmp/locifind-a4-sweep
for N in 0 1 2 3 5; do
  echo "=== max_cross_lang_hits = $N ===" >> /tmp/locifind-a4-sweep/sweep-lang.log
  cargo run --release -p evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --max-cross-lang-hits $N \
    2>&1 | tee -a /tmp/locifind-a4-sweep/sweep-lang.log
done
# usize::MAX 控制对照（≡HYB）
echo "=== max_cross_lang_hits = usize::MAX ===" >> /tmp/locifind-a4-sweep/sweep-lang.log
cargo run --release -p evals --bin semantic_quality -- \
  --semantic-weight 10.0 \
  --max-cross-lang-hits 18446744073709551615 \
  2>&1 | tee -a /tmp/locifind-a4-sweep/sweep-lang.log
```

- [ ] **Step 6.2：人工读 sweep-lang.log，按 spec §2.2 红线 + spec §5 降级路径选 N\***

依据：
1. **(4a)** 所有 N 下 exact-name HYBR_R = 1.000（不满足 = bug，回 task 2 排查）
2. **(4b)** 各桶 HYBR_N ≥ HYB baseline（A-2 baseline.json 同桶值；查 baseline.json `hybrid_recall.${bucket}` / `hybrid_ndcg.${bucket}`）
3. **优先 (c)** OVERALL HYBR_N 最大（> 0.854 = A-2 baseline）
4. **优先 (d)** crosslang HYBR_N 最大（> 0.649 = A-2 baseline）

**spec §5 降级路径**（A-3 同款）：若任一 N\* < usize::MAX 都破 (4b) 红线，**N\* = usize::MAX**（路由本 cycle 不生效、HYBR ≡ HYB）。

- [ ] **Step 6.3：控制对照核验**

确认 N=usize::MAX 时 HYBR ≡ HYB（既有 baseline.json 的 hybrid_routed_recall/ndcg ≈ hybrid_recall/ndcg）；N=0 时 HYBR ≈ VEC（modulo Mixed query 例外仍 HYB）。如不满足，回 task 2 排查 wrapper 逻辑。

- [ ] **Step 6.4：把 sweep 全表 + N\* 决策记到 plan 旁注（备 task 10 报告用）**

不入仓、记到自己的工作 note 或临时 markdown。

---

## Task 7: bake `DEFAULT_MAX_CROSS_LANG_HITS` + rewrite baseline.json

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs` `DEFAULT_MAX_CROSS_LANG_HITS` 实际值
- Rewrite: `packages/evals/fixtures/semantic-recall/baseline.json` HYBR 字段 + `max_cross_lang_hits_bake` 元数据

- [ ] **Step 7.1：bake 常量**

把 task 2 占位的 `DEFAULT_MAX_CROSS_LANG_HITS = usize::MAX` 替换为 task 6 选定的 N\*（如 sweep 结果 N\*=2，写 `pub const DEFAULT_MAX_CROSS_LANG_HITS: usize = 2;`；spec §5 降级则保持 `usize::MAX`）。

更新 doc 注释：

```rust
/// FTS/VEC top-K 跨语种 vec hit 计数路由的默认阈值（≥ 此 count 时跳过 FTS）。
/// BETA-15B-3 A-4 sweep 选定 N\*={具体值}（详 docs/reviews/semantic-recall-quality-baseline.md A-4 调优记录节）。
/// {若 spec §5 降级：补一句「路由本 cycle 等价不生效、下 cycle 抓手 = 更强信号」}
pub const DEFAULT_MAX_CROSS_LANG_HITS: usize = /* N* 值 */;
```

- [ ] **Step 7.2：rewrite baseline.json**

跑 N\* 配置下的完整评测、把输出的每桶 `hybrid_routed_recall` / `hybrid_routed_ndcg` 写回 `packages/evals/fixtures/semantic-recall/baseline.json`：

Run: `cargo run --release -p evals --bin semantic_quality -- --semantic-weight 10.0 --max-cross-lang-hits {N*} --write-baseline`

（确认 binary 有 `--write-baseline` flag；A-2/A-3 已有此机制）

- [ ] **Step 7.3：baseline.json 加元数据字段**

打开 `packages/evals/fixtures/semantic-recall/baseline.json`，在顶层加（如不存在）：

```json
{
  "semantic_weight": 10.0,
  "max_cross_lang_hits_bake": {N*},
  /* 既有 buckets / hybrid_recall / hybrid_ndcg / hybrid_routed_* / vector_only_* 等字段 */
}
```

- [ ] **Step 7.4：byte-equal 守护跑一遍 v0.5/v0.9 parser-only**

Run（按 [[project-evals-reporter-nondeterministic]] 规范化逐 case 比对）：

```bash
# 用既有 evals reporter v0.5/v0.9 规范化 JSON 比对（按 id sort + 去 elapsed_ms 字段后 diff）
# 具体命令见 BETA-13 G16 commit 历史的 byte-equal 闸门脚本
cargo run --release -p evals --bin reporter -- --version v05 --json /tmp/v05.json
cargo run --release -p evals --bin reporter -- --version v09 --json /tmp/v09.json
# 与 main 分支同样输出按 id 规范化对比，要求 0 diff
```

Expected: v0.5 passed=473、v0.9 passed=877、规范化 diff=0

- [ ] **Step 7.5：commit**

```bash
git add packages/result-normalizer/src/lib.rs packages/evals/fixtures/semantic-recall/baseline.json
git commit -m "BETA-15B-3 A-4 task 7：bake DEFAULT_MAX_CROSS_LANG_HITS = {N*} + rewrite baseline.json 含 HYBR 字段"
```

---

## Task 8: gate 4 条红线断言数值替换

**Files:**
- Modify: `packages/evals/tests/semantic_quality_gate.rs` 4 条 HYBR 断言数值

- [ ] **Step 8.1：定位 A-3 写下的 4 条 HYBR 断言**

Run: `rg -n "hybrid_routed|HYBR" packages/evals/tests/semantic_quality_gate.rs`

定位 (4a)(4b)(4c)(4d) 四条 assert 块。

- [ ] **Step 8.2：把 baseline.json 读入 + 断言数值替换**

四条不变量结构保留、数值改为：
- (4a) exact-name `hybrid_routed_recall == 1.0` （硬值，不变）
- (4b) **各桶** `hybrid_routed_ndcg[bucket] ≥ hybrid_ndcg[bucket]`（A-2 hybrid baseline，自 baseline.json 读）
- (4c) `hybrid_routed_ndcg["OVERALL"] ≥ baseline.json["hybrid_routed_ndcg"]["OVERALL"]`（自锁、新 baseline 写入值）
- (4d) `hybrid_routed_ndcg["crosslang"] ≥ baseline.json["hybrid_routed_ndcg"]["crosslang"]`（自锁、新 baseline 写入值）

具体改动看 A-3 commit `c647c60`（gate 加 HYBR 断言）的 diff 模式。

- [ ] **Step 8.3：跑 gate 测试**

Run: `cargo test -p evals --test semantic_quality_gate`
Expected: 全过

- [ ] **Step 8.4：跑全 workspace 测试一次**

Run: `cargo test --workspace`
Expected: 全过、含本 cycle 新 lang.rs / wrapper / arms / report / gate 所有新测

- [ ] **Step 8.5：fmt + clippy 全 workspace**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -D warnings`
Expected: 0 warning、0 fmt diff

- [ ] **Step 8.6：commit**

```bash
git add packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-3 A-4 task 8：semantic_quality_gate HYBR 4 条红线断言数值替换为 A-4 baseline"
```

---

## Task 9: 生产 wiring：`run_fanout_merge_rrf` 入口 detect_lang + wrapper 调用改新签名

**Files:**
- Modify: `packages/harness/src/fanout_merge.rs:111-187` `run_fanout_merge_rrf` 签名扩 `query: &str` + 入口 detect_lang + wrapper 调用
- Modify: `packages/harness/src/lib.rs` 若 re-export 同步
- Modify: `apps/desktop/src-tauri/src/search/fanout.rs` call-site 透传 query 字符串
- Modify: `apps/desktop/src-tauri/src/search/tests.rs` 测试 call-site 透传
- Modify: `packages/harness/src/fanout_merge.rs:238+` 既有 mod tests 测试 call-site 透传 + 新增 wrapper 三态分支单测

- [ ] **Step 9.1：改 `run_fanout_merge_rrf` 签名扩 `query: &str`**

把 `packages/harness/src/fanout_merge.rs:111-117` 替换为：

```rust
pub async fn run_fanout_merge_rrf<R>(
    backends: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut R,
    semantic_weight: f64,
    query: &str,
) -> FanoutOutcome
```

- [ ] **Step 9.2：函数体入口 detect_lang + wrapper 调用改新签名**

`packages/harness/src/fanout_merge.rs:120` 起加：

```rust
use locifind_result_normalizer::lang::detect_lang;

// ... 既有 fts_list/vec_list/sources_queried/errors 收集逻辑保持
```

把 `packages/harness/src/fanout_merge.rs:169-175`（`fuse_rrf_with_fts_routing` 调用）改为：

```rust
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

同时把 import 改：

```rust
use locifind_result_normalizer::{
    fuse_rrf_with_fts_routing, merge_results, MergedResult, RouteVerdict,
    DEFAULT_MAX_CROSS_LANG_HITS, DEFAULT_RRF_K,
};
```

把原 `DEFAULT_FTS_JACCARD_THRESHOLD` import 删除。

- [ ] **Step 9.3：找全 call-sites 改签名透传 query**

Run: `rg -n "run_fanout_merge_rrf" packages/ apps/ 2>&1 | head -30`

为每个 call-site 提取 user query 字符串透传：
- `apps/desktop/src-tauri/src/search/fanout.rs` — desktop 主路径；user query 已在上下文（搜索框输入），直接透传
- `apps/desktop/src-tauri/src/search/tests.rs` — 测试 call-site 给个固定 query 字符串
- `packages/harness/src/fanout_merge.rs:416 / 461 / 470 / 520` — 自身 unit tests 的 4 处 call-site，给个固定 query 字符串

为每个 call-site 实际定位 query 字符串来源：

```bash
rg -n "run_fanout_merge_rrf\(" apps/desktop/src-tauri/src/search/fanout.rs -A 10
```

让 query 从已有 SearchIntent / state 透传；若 desktop 那边没现成 query 字符串，从上层 caller 拿（搜索 entry point 一定有 raw query）。

- [ ] **Step 9.4：harness/src/lib.rs 若 re-export 签名同步**

Run: `rg -n "run_fanout_merge_rrf" packages/harness/src/lib.rs`
预期看到 re-export；签名不需要重写、但确认导出表面无 stale doc 注释。

- [ ] **Step 9.5：harness 单测加 wrapper 三态覆盖**

在 `packages/harness/src/fanout_merge.rs` 末尾的 `mod tests` 中追加 3 个单测：

```rust
#[test]
fn fanout_rrf_en_query_with_zh_vec_hits_skips_fts() {
    // En query + vec backend 返回 ZH name → wrapper 应跳 FTS
    // 假设 sweep N*=2，构造 2 条 ZH name vec hits 触发跳过
    // 用 mock SearchBackend
    // ...
    let outcome = block_on(run_fanout_merge_rrf(
        &backends, &expanded, cancel, &mut sink,
        DEFAULT_SEMANTIC_WEIGHT,
        "annual leave policy",  // En query
    ));
    let v = outcome.route_verdict.expect("透传 verdict");
    assert!(v.skipped_fts || DEFAULT_MAX_CROSS_LANG_HITS == usize::MAX,
            "N*={DEFAULT_MAX_CROSS_LANG_HITS} 下应跳 FTS（或 spec §5 降级）");
    assert_eq!(v.query_lang, Lang::En);
}

#[test]
fn fanout_rrf_zh_query_no_skip_when_pure_zh_corpus() {
    let outcome = block_on(run_fanout_merge_rrf(
        &backends, &expanded, cancel, &mut sink,
        DEFAULT_SEMANTIC_WEIGHT,
        "年假规定",  // Zh query
    ));
    let v = outcome.route_verdict.expect("透传 verdict");
    assert!(!v.skipped_fts);
    assert_eq!(v.query_lang, Lang::Zh);
}

#[test]
fn fanout_rrf_mixed_query_never_skips() {
    let outcome = block_on(run_fanout_merge_rrf(
        &backends, &expanded, cancel, &mut sink,
        DEFAULT_SEMANTIC_WEIGHT,
        "qwen 调优",  // Mixed query
    ));
    let v = outcome.route_verdict.expect("透传 verdict");
    assert!(!v.skipped_fts, "Mixed query 保守降级、永不跳");
    assert_eq!(v.query_lang, Lang::Mixed);
}
```

具体 mock backend / `expanded` 构造照搬同文件既有 `mod tests` 里的 helper（如 `MockBackend`、`intent()` 等）。

- [ ] **Step 9.6：跑全 workspace 测试**

Run: `cargo test --workspace`
Expected: 全过、含本 cycle 全部新单测

- [ ] **Step 9.7：fmt + clippy 全 workspace**

Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -D warnings`
Expected: 0 warning、0 fmt diff

- [ ] **Step 9.8：前端 tsc + vite build**

Run: `cd apps/desktop && pnpm tsc --noEmit && pnpm vite build 2>&1 | tail -20`
Expected: 净（本 cycle 不动前端、应自然过）

- [ ] **Step 9.9：commit**

```bash
git add packages/harness/src/fanout_merge.rs apps/desktop/src-tauri/src/search/fanout.rs apps/desktop/src-tauri/src/search/tests.rs
git commit -m "BETA-15B-3 A-4 task 9：fanout_merge_rrf 签名扩 query + 入口 detect_lang + wrapper 新签名 + 3 单测"
```

---

## Task 10: baseline 报告追加 A-4 调优记录节 + 总验收

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md` 追加 A-4 节

- [ ] **Step 10.1：追加 A-4 调优记录节**

在 `docs/reviews/semantic-recall-quality-baseline.md` 末尾追加（仿 A-2 / A-3 节结构）：

```markdown
## A-4 query 语种检测 + 跨语种 vec hit 路由（2026-06-23 Claude Code）

**承接**：A-3 sweep 暴露 Jaccard 单维信号天然分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」、t*=0.10 spec §5 降级路由本 cycle 不生效。A-4 升级 wrapper 内部信号为 query 语种检测 + 跨语种 vec hit 计数。

**信号设计**：
- 自写 CJK Unicode ratio 三态二阈（Zh / En / Mixed、纯 std、零依赖）
- wrapper API 名 / RouteVerdict / HYBR baseline 字段名 / gate 红线架构保留
- query_lang = Mixed 永不跳（保守降级）

**Sweep 全表**（W=10.0 固定、top_k=10、N = max_cross_lang_hits）：

| N | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0 | {填实测} | {填实测} | {填实测} | {填实测} |
| 1 | … | … | … | … |
| 2 | … | … | … | … |
| 3 | … | … | … | … |
| 5 | … | … | … | … |
| usize::MAX (≡HYB 控制) | 1.000 | 0.854 | 0.649 | 0.930 |

**N\* 选定 = {N*}**，理由：{依 spec §2.2 红线 + spec §5 降级路径}。

**实测变化**（HYBR vs A-2 baseline HYB）：
- OVERALL HYBR_N: 0.854 → {新}（{Δ}）
- crosslang HYBR_N: 0.649 → {新}（{Δ}）
- content-not-name HYBR_N: 0.930 → {新}（{Δ}）
- 其它桶 HYBR_N: 各桶变化列出

**诚实边界**：
- {若 (4c)(4d) 优先期望达成} → 升级信号 work、本 cycle 抬升完成、下 cycle 抓手回到 ② VEC cosine 阈值 / ③ 更大模型 / ④ 评测集扩量。
- {若 (4c)(4d) 未达 但 (4b) 各桶 ≥ HYB} → 路由不退步、但信号增益不足，下 cycle 抓手 = ②/③/④ 之一。
- {若 (4b) 破红线} → spec §5 字面正解，N\* = usize::MAX、路由本 cycle 不生效、信号不够区分场景；下 cycle 抓手 = ②/③/④ 之一。

**基础设施完整入栈**：lang 模块（result-normalizer 内部）+ wrapper 升级 + 评测设施 corpus 透传 + baseline 新水位 + gate 4 条红线、为下 cycle 信号升级（候选 ②/③/④）留好旋钮。

链接：[spec](../superpowers/specs/2026-06-23-beta-15b-3a4-lang-routing-design.md) / [plan](../superpowers/plans/2026-06-23-beta-15b-3a4-lang-routing.md)
```

- [ ] **Step 10.2：总验收 checklist**

逐项跑、写到本会话 STATUS 收工日志、对照 spec §2.2：

```bash
cargo test --workspace                                        # (1) 0 failed
cargo clippy --workspace --all-targets -D warnings            # (2) 0 warning
cargo fmt --all --check                                       # (3) 净
cargo test -p evals --test semantic_quality_gate              # (4) HYBR 4 红线 pass
# (5) evals parser-only byte-equal v0.5=473/v0.9=877
cd apps/desktop && pnpm tsc --noEmit && pnpm vite build       # (6) 前端净
```

- [ ] **Step 10.3：commit**

```bash
git add docs/reviews/semantic-recall-quality-baseline.md
git commit -m "BETA-15B-3 A-4 task 10：baseline 报告追加 A-4 调优记录 + 总验收过"
```

---

## Self-Review 结论

- ✅ **Spec coverage**：spec §6 task 1–10 与 plan task 1–10 一一对应
- ✅ **No placeholders**：所有 `/* N* */` 替代值 task 6 sweep 决定后由 task 7 bake 填实，符合 TDD「数据驱动决策」精神，非懒散 TBD
- ✅ **Type consistency**：`Lang::{Zh, En, Mixed}`、`RouteVerdict { skipped_fts, query_lang, cross_lang_hits, max_cross_lang_hits }`、`fuse_rrf_with_fts_routing(fts, vec, k, w, query_lang, max)` 在 task 1-9 间一致
- ✅ **Bite-sized steps**：每 task ≤ 8 步、每步含具体代码/命令/预期输出
- ✅ **Sweep + bake**：task 6 是评测决策 task、不产代码 commit；task 7 bake + byte-equal 守护
- ✅ **byte-equal 守护**：task 7 / task 10 验收均含 v0.5=473 / v0.9=877 不退守
- ✅ **风险预案**：sweep 红线破时 N\* = usize::MAX、spec §5 字面正解、task 7 doc 注释明示

## 链接

- spec：[../specs/2026-06-23-beta-15b-3a4-lang-routing-design.md](../specs/2026-06-23-beta-15b-3a4-lang-routing-design.md)
- A-3 plan（前案）：[2026-06-23-beta-15b-3a3-fts-confidence-routing.md](./2026-06-23-beta-15b-3a3-fts-confidence-routing.md)
- A-2 plan（前案）：[2026-06-22-beta-15b-3a2-semantic-weight-tuning.md](./2026-06-22-beta-15b-3a2-semantic-weight-tuning.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)
