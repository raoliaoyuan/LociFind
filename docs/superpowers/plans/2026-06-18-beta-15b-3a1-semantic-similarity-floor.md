# BETA-15B-3 簇 A-1 实施计划：语义臂相似度下限

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给语义臂加 cosine 相似度下限（默认 0.30），把明显不相关的候选挡在语义召回之外（不进结果、不打旗舰徽标），修 15B-1 真机发现的「小语料下低相关项也打徽标、徽标廉价」。

**Architecture:** `semantic-index/src/lib.rs` 的 `search_results` 在算完候选 cosine 后，用新纯函数 `filter_rank_topk(scored, floor, k)`（retain ≥floor → 降序 → 截断 K）取代原地的 `sort_by + truncate`。低相关候选不入 RRF 融合池。不碰 parser → evals byte-equal 天然安全。

**Tech Stack:** Rust（`packages/search-backends/semantic-index`）。

参考 spec：[docs/superpowers/specs/2026-06-18-beta-15b-3a1-semantic-similarity-floor-design.md](../specs/2026-06-18-beta-15b-3a1-semantic-similarity-floor-design.md)

---

## File Structure

| 文件 | 职责 | 改动 |
|---|---|---|
| `packages/search-backends/semantic-index/src/lib.rs` | 语义召回后端 | 加 `SIMILARITY_FLOOR` 常量 + 纯函数 `filter_rank_topk`；`search_results` 改调它；改现有排序测试 + 加下限测试 + 纯函数单测 |
| `docs/manual-test-scenarios.md` | 真机手测登记 | 加簇 A-1 节 |

---

## Task 1: 相似度下限过滤 + 测试

**Files:**
- Modify: `packages/search-backends/semantic-index/src/lib.rs`（`search_results` :95-106；测试 `semantic_query_ranks_by_cosine` :214-243）

semantic-index crate 名 = `locifind-semantic-index`（已核实）。

- [ ] **Step 1: 写纯函数失败测试 + 下限行为测试**

在 `lib.rs` 的 `#[cfg(test)] mod tests`（:172）末尾追加：

```rust
    #[test]
    fn filter_rank_topk_filters_sorts_truncates() {
        let scored = vec![
            (0.10_f32, "low.txt".to_owned()),   // < floor，挡掉
            (0.90, "hi.txt".to_owned()),
            (0.50, "mid.txt".to_owned()),
            (0.29, "below.txt".to_owned()),     // < 0.30，挡掉
        ];
        let out = filter_rank_topk(scored, 0.30, 10);
        let names: Vec<&str> = out.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(names, vec!["hi.txt", "mid.txt"], "仅 ≥floor 存活、降序");
    }

    #[test]
    fn filter_rank_topk_truncates_to_k() {
        let scored = vec![
            (0.9_f32, "a".to_owned()),
            (0.8, "b".to_owned()),
            (0.7, "c".to_owned()),
        ];
        let out = filter_rank_topk(scored, 0.30, 2);
        assert_eq!(out.len(), 2, "截断到 K");
        assert_eq!(out[0].1, "a");
        assert_eq!(out[1].1, "b");
    }

    #[test]
    fn filter_rank_topk_all_below_floor_is_empty() {
        let scored = vec![(0.10_f32, "a".to_owned()), (0.20, "b".to_owned())];
        assert!(filter_rank_topk(scored, 0.30, 10).is_empty(), "全低于 floor → 空");
    }

    /// 端到端：低相关（正交，cosine 0）候选被下限挡掉，不进结果。
    #[test]
    fn semantic_floor_filters_low_relevance() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cat.txt"), "关于猫的笔记").unwrap();
        std::fs::write(dir.path().join("dog.txt"), "关于狗的笔记").unwrap();
        let db = dir.path().join("index.db");
        let idx = DocumentIndex::open(&db).unwrap();
        idx.index_dirs(&[dir.path().to_path_buf()]).unwrap();
        let cat = dir.path().join("cat.txt").to_string_lossy().into_owned();
        let dog = dir.path().join("dog.txt").to_string_lossy().into_owned();
        // cat 与查询「猫」=[1,0] 同轴（cosine 1）；dog=[0,1] 正交（cosine 0 < floor）。
        assert!(idx.upsert_vector(&cat, &[1.0, 0.0], "axis", "h1").unwrap());
        assert!(idx.upsert_vector(&dog, &[0.0, 1.0], "axis", "h2").unwrap());

        let backend = SemanticIndexBackend::new(&db, Some(Arc::new(AxisEmbedder)));
        let results = backend.search_results(&file_search("我家的猫")).unwrap();

        assert_eq!(results.len(), 1, "正交低相关 dog 被下限挡掉");
        assert_eq!(results[0].name, "cat.txt");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-semantic-index filter_rank_topk`
Expected: FAIL（`filter_rank_topk` 未定义，编译错误）

- [ ] **Step 3: 实现 `SIMILARITY_FLOOR` + `filter_rank_topk` + 接入 `search_results`**

在 `lib.rs` 顶部常量区（`TOP_K` 附近，:29）加：

```rust
/// 语义臂相似度下限：低于此 cosine 的候选视为不相关，不进结果（不打旗舰徽标）。
/// BETA-26 `embed()` 证伪闸门实测：相关文本 cosine ≈ 0.75、无关 ≈ 0.18；0.30 稳高于无关基线、
/// 远低于真实相关（含 crosslang 命中），只挡明显噪声。命名常量：待簇 A held-out 评测落地后据数据精调。
const SIMILARITY_FLOOR: f32 = 0.30;
```

在 `vector_hit_to_result`（:144）之前或 `search_results` 之后加纯函数：

```rust
/// 按相似度下限过滤 + 降序排序 + 截断 topK（纯函数，可单测）。
/// 全部候选低于 `floor` → 返回空（语义臂空，整链优雅降级 FTS-only）。
fn filter_rank_topk(mut scored: Vec<(f32, String)>, floor: f32, k: usize) -> Vec<(f32, String)> {
    scored.retain(|(s, _)| *s >= floor);
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate(k);
    scored
}
```

把 `search_results` 里（:100-101）的：

```rust
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(TOP_K);
```

替换为：

```rust
        let scored = filter_rank_topk(scored, SIMILARITY_FLOOR, TOP_K);
```

注意：`scored` 原是 `let mut scored`，改为 `filter_rank_topk` 取走所有权后返回新 `scored`（shadowing），下方 `.into_iter()` 不变。若编译器提示 `mut` 不再需要，去掉 `scored` 绑定处的 `mut`（`let scored: Vec<(f32, String)> = candidates.into_iter()...collect();`）。

- [ ] **Step 4: 改现有 `semantic_query_ranks_by_cosine` 的向量（避免被下限误杀）**

现有测试（:227-228）手工挂的 dog 向量 `[0.0, 1.0]` 与查询 `[1,0]` 正交（cosine 0）会被新下限挡掉、破坏「两条都返回」断言。改这两行为两者都过下限但有序：

```rust
        assert!(idx.upsert_vector(&cat, &[1.0, 0.2], "axis", "h1").unwrap()); // cosine ≈ 0.98
        assert!(idx.upsert_vector(&dog, &[1.0, 1.0], "axis", "h2").unwrap()); // cosine ≈ 0.71
```

其余断言（len==2、cat 排第一、score 递减）保持不变——cat 0.98 > dog 0.71 仍成立，两者都 > 0.30。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p locifind-semantic-index`
Expected: PASS（4 个新测试 + 改后的 `semantic_query_ranks_by_cosine` + 既有 `no_embedder_is_unavailable_and_empty` 全绿）

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt -p locifind-semantic-index
cargo clippy -p locifind-semantic-index --all-targets -- -D warnings
git add packages/search-backends/semantic-index/src/lib.rs
git commit -m "BETA-15B-3 簇A-1：语义臂相似度下限（cosine 0.30，挡低相关项）"
```

---

## Task 2: 回归门 + 手测登记

**Files:**
- Modify: `docs/manual-test-scenarios.md`

- [ ] **Step 1: 全量回归门**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: fmt 净；clippy 仅一条无害 non-root profile 提示；`cargo test --workspace` 全绿。

- [ ] **Step 2: evals byte-equal 硬门**

本切片不碰 parser，evals 应不动。
```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9
```
Expected: v0.5 pass=473、v0.9 pass=726（与基线逐字相符）。

- [ ] **Step 3: 登记真机手测**

在 `docs/manual-test-scenarios.md` 加「BETA-15B-3 簇 A-1」节：

```markdown
## BETA-15B-3 簇 A-1（语义臂相似度下限）

前提：feature `semantic-recall`（+ metal）构建 + 放 embedding 模型 + 已 reindex 出向量。

1. **低相关不再凑数**：用一个与本机文档都不太相关的查询 → 结果不再出现一堆打「按意思找到」徽标的凑数语义项（cosine < 0.30 的被挡）。
2. **真实跨语言命中仍浮现**：复跑 15B-1 的跨语言用例（中文「年假和休假规定」命中纯英文 leave policy 文档）→ 仍正常返回 + 打「按意思找到」徽标（高 cosine，不受 0.30 下限影响）。
3. **阈值观感**：若仍觉得有低相关项漏网（偏松）或真实命中被挡（偏紧），记录现象——`SIMILARITY_FLOOR` 是一行可调的命名常量，留簇 A held-out 评测精调。
```

- [ ] **Step 4: commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-15B-3 簇A-1：回归门通过 + 登记真机手测场景"
```

---

## Self-Review 覆盖核对

- **spec §3.1 下限过滤（常量 + 纯函数 + 接入 search_results）** → Task 1 Step 3 ✅
- **spec §3.2 阈值 0.30 + 命名常量 + 依据注释** → Task 1 Step 3 ✅
- **spec §6 测试（纯函数单测 + 改排序测试 + 新增下限测试 + 回归门 + evals byte-equal + 手测登记）** → Task 1 Step 1/4 + Task 2 ✅
- **spec §5 错误处理（全低于 floor → 空 → 降级）** → `filter_rank_topk_all_below_floor_is_empty` 测试 + 既有降级路径 ✅
- **类型一致性**：`SIMILARITY_FLOOR: f32 = 0.30`、`filter_rank_topk(scored: Vec<(f32,String)>, floor: f32, k: usize) -> Vec<(f32,String)>`、`TOP_K` 全计划内一致 ✅
- **不破坏现有测试**：`semantic_query_ranks_by_cosine` 向量改为都过下限（Task 1 Step 4）；`no_embedder_is_unavailable_and_empty` 不受影响（早返回空，不到 filter） ✅
