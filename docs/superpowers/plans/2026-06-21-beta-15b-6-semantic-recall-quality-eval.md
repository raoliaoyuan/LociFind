# 持久化语义召回质量评测集 + baseline 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `packages/evals` 建一套可提交、确定性、CI 可门控的「语义召回质量」排名评测（合成语料 + 缓存向量 + Recall@10/nDCG@10 × FTS5/向量/hybrid 三臂，hybrid 跑生产融合），并把 `spike-retrieval` 转正为本地现实校准锤。

**Architecture:** 评测全部读 checked-in 合成语料 + checked-in 合成向量缓存（无 PII），三臂排名后算 Recall@10/nDCG@10：FTS5 臂用内存 rusqlite trigram、向量臂用生产 `locifind_indexer::vectors::cosine` + 相似度下限、hybrid 臂用生产 `locifind_result_normalizer::fuse_rrf`。向量由 `--embed` 子模式（feature 门控、需模型）一次性生成后提交，此后评测不需模型即可确定性跑。

**Tech Stack:** Rust（`packages/evals` 新模块 + 新 binary + 集成测试）；rusqlite（bundled SQLite FTS5）；复用 `locifind-indexer`（cosine）、`locifind-result-normalizer`（fuse_rrf）、`locifind-search-backend`（SearchResult）、`locifind-model-runtime`（--embed）。

---

## 关键事实（实现前必读）

- **已有可复用资产**：`packages/spike-retrieval/src/lib.rs` 有经审阅的 `recall_at_k(ranked, relevant:&HashSet, k)` / `ndcg_at_k(ranked, grades:&HashMap<String,u8>, k)`（增益 `2^g−1`、折扣 `1/log2(rank+2)`）+ trigram FTS5 建表/`build_match_query`/`fts_query`。**本计划在 `packages/evals` 重新实现这些**（不依赖 spike-retrieval crate——它是 gitignored 真实数据 crate，架构上 evals 不该依赖它），公式逐字照搬已验证版本。
- **生产融合签名**：`locifind_result_normalizer::fuse_rrf(lists: Vec<Vec<SearchResult>>, k: f64, semantic_weight: f64) -> Vec<MergedResult>`（`packages/result-normalizer/src/lib.rs:99`）；常量 `DEFAULT_RRF_K=60.0`、`DEFAULT_SEMANTIC_WEIGHT=2.0`。`MergedResult{ result: SearchResult, sources, match_types }`。fuse_rrf 按 `r.source == BackendKind::SemanticIndex` 给 `semantic_weight`、其余 1.0，按 path 累加 RRF、降序返回。
- **SearchResult 字段**（`packages/search-backends/common/src/lib.rs:111`）：`{ id:String, path:PathBuf, name:String, source:BackendKind, match_type:MatchType, score:Option<f64>, metadata:SearchResultMetadata }`；`SearchResultMetadata: Default`。`BackendKind::{NativeIndex, SemanticIndex}`、`MatchType::{Content, Semantic}`。
- **cosine**：`locifind_indexer::vectors::cosine(a: &[f32], b: &[f32]) -> f32`（维度不等/零向量/NaN → 0.0）。
- **相似度下限**：生产语义臂在融合前 filter `cosine >= SIMILARITY_FLOOR`（默认 0.30）。评测用常量 `EVAL_SIMILARITY_FLOOR: f32 = 0.30` 复刻，参数化。
- **fixtures 路径惯例**：库内 `Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")...`；集成测试用 `ws()` 倒推仓库根（见 `tests/synonym_recall_gate.rs`）。
- **gitignored 数据缺失跳过**：集成测试缺数据时 `eprintln! + return`（见 `spike-retrieval/tests/evalset_integrity.rs:33`）——本计划的合成数据是**提交的**，gate 在向量/baseline 提交前 skip、提交后常跑。
- **向量缓存无 PII**：合成文本的 embedding，可提交。

### 文件结构

| 文件 | 责任 | 动作 |
|---|---|---|
| `packages/evals/src/semantic_quality/metrics.rs` | `recall_at_k` / `ndcg_at_k` 纯函数 | 新建 |
| `packages/evals/src/semantic_quality/data.rs` | 类型 + 加载 + 完整性 | 新建 |
| `packages/evals/src/semantic_quality/arms.rs` | FTS / 向量 / hybrid 三臂排名 | 新建 |
| `packages/evals/src/semantic_quality/report.rs` | 逐 case 打分 + 分桶聚合 | 新建 |
| `packages/evals/src/semantic_quality/mod.rs` | 模块聚合 + 常量 | 新建 |
| `packages/evals/src/lib.rs` | 挂 `pub mod semantic_quality;` | 改（1 行） |
| `packages/evals/Cargo.toml` | 加 rusqlite / result-normalizer / indexer + `semantic-recall` feature + bin | 改 |
| `packages/evals/src/bin/semantic_quality.rs` | CLI（默认跑缓存 / `--embed` / `--write-baseline` / `--json`） | 新建 |
| `packages/evals/tests/semantic_quality_gate.rs` | 回归门（skip-if-missing） | 新建 |
| `packages/evals/fixtures/semantic-recall/{corpus,cases,vectors,baseline}.json + README.md` | 合成语料 + 评测集 + 缓存向量 + baseline | 新建（authoring + user bootstrap） |
| `packages/spike-retrieval/Cargo.toml` + `README.md` | llama-cpp 改可选 feature + 转正注释 | 改 |

### 执行相位与归属

- **Phase A（Task 1–9，subagent 可做，TDD）**：评测 harness + binary + gate（用微型内联 fixture 单测，不依赖大语料）。
- **Phase B（Task 10–11，生成 subagent）**：合成语料 + 评测集 authoring + 隐私复核。
- **Phase C（Task 12，subagent）**：spike-retrieval 转正 + 清 llama-cpp 污染。
- **Phase D（Task 13，USER 一次性 bootstrap）**：装 `semantic-recall` feature + 模型，跑 `--embed` 生成并提交 `vectors.json`、`--write-baseline` 生成 `baseline.json` + baseline 报告，激活 gate。**非 subagent**（需模型 + Metal，同真机手测归用户）。

---

## Task 1：排名指标模块 `metrics.rs`

**Files:**
- Create: `packages/evals/src/semantic_quality/metrics.rs`
- Create: `packages/evals/src/semantic_quality/mod.rs`
- Modify: `packages/evals/src/lib.rs`

- [ ] **Step 1: 建模块骨架 + 挂载**

新建 `packages/evals/src/semantic_quality/mod.rs`：

```rust
//! BETA-15B-6：语义召回质量评测（合成语料 + 缓存向量 + 三臂排名指标）。
//! 全部读 checked-in 合成数据（无 PII）；hybrid 跑生产融合。

pub mod metrics;

/// 评测相似度下限：复刻生产语义臂融合前过滤（`DEFAULT_SIMILARITY_FLOOR=0.30`）。
pub const EVAL_SIMILARITY_FLOOR: f32 = 0.30;
/// 排名截断 k（指标 @k）。
pub const TOP_K: usize = 10;
```

在 `packages/evals/src/lib.rs` 顶部 mod 区加一行：

```rust
pub mod semantic_quality;
```

- [ ] **Step 2: 写失败测试**

新建 `packages/evals/src/semantic_quality/metrics.rs`，先写测试：

```rust
//! Recall@k / nDCG@k 纯函数（公式照搬 BETA-26 已验证版本）。

use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn recall_counts_hits_in_top_k_over_total_relevant() {
        let ranked = ids(&["a", "x", "b", "y", "z"]);
        let relevant: HashSet<String> = ids(&["a", "b", "c"]).into_iter().collect();
        // top-3 命中 a,b（c 没进 top-3）→ 2/3
        assert!((recall_at_k(&ranked, &relevant, 3) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn recall_empty_relevant_is_zero() {
        assert_eq!(recall_at_k(&ids(&["a"]), &HashSet::new(), 10), 0.0);
    }

    #[test]
    fn ndcg_perfect_ranking_is_one() {
        let ranked = ids(&["a", "b"]);
        let mut grades = HashMap::new();
        grades.insert("a".to_owned(), 3u8);
        grades.insert("b".to_owned(), 1u8);
        assert!((ndcg_at_k(&ranked, &grades, 10) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ndcg_reversed_ranking_is_less_than_one() {
        let ranked = ids(&["b", "a"]); // 低分在前
        let mut grades = HashMap::new();
        grades.insert("a".to_owned(), 3u8);
        grades.insert("b".to_owned(), 1u8);
        let v = ndcg_at_k(&ranked, &grades, 10);
        assert!(v > 0.0 && v < 1.0, "reversed nDCG 应 ∈(0,1)，实得 {v}");
    }

    #[test]
    fn ndcg_no_relevant_is_zero() {
        assert_eq!(ndcg_at_k(&ids(&["a"]), &HashMap::new(), 10), 0.0);
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::metrics 2>&1 | tail -5`
Expected: 编译失败（`recall_at_k`/`ndcg_at_k` 未定义）。

- [ ] **Step 4: 实现**

在 `metrics.rs` 的 `use` 之后、`mod tests` 之前加：

```rust
/// top-k 命中的相关文档数 / 相关文档总数。
#[must_use]
pub fn recall_at_k(ranked: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if relevant.is_empty() {
        return 0.0;
    }
    let hits = ranked
        .iter()
        .take(k)
        .filter(|id| relevant.contains(*id))
        .count();
    hits as f64 / relevant.len() as f64
}

/// nDCG@k：增益 `2^grade − 1`，折扣 `1/log2(rank+2)`，除以理想排序 DCG。
#[must_use]
pub fn ndcg_at_k(ranked: &[String], grades: &HashMap<String, u8>, k: usize) -> f64 {
    let dcg = |ids: &[String]| -> f64 {
        ids.iter()
            .take(k)
            .enumerate()
            .map(|(i, id)| {
                let g = f64::from(*grades.get(id).unwrap_or(&0));
                (2f64.powf(g) - 1.0) / (i as f64 + 2.0).log2()
            })
            .sum()
    };
    let actual = dcg(ranked);
    let mut ideal_ids: Vec<String> = grades.keys().cloned().collect();
    ideal_ids.sort_by(|a, b| grades[b].cmp(&grades[a]));
    let ideal = dcg(&ideal_ids);
    if ideal == 0.0 {
        0.0
    } else {
        actual / ideal
    }
}
```

> 顶部 `use std::collections::{HashMap, HashSet};` 已在测试块上方；若 clippy 报 `cast_precision_loss`/`float_cmp`，在函数上加 `#[allow(clippy::cast_precision_loss)]` / 对 `ideal == 0.0` 加 `#[allow(clippy::float_cmp)]`（与 spike-retrieval 同款，是刻意边界比较）。

- [ ] **Step 5: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::metrics 2>&1 | tail -8 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: 5 测试 PASS、clippy 0、fmt 净。

- [ ] **Step 6: 提交**

```bash
git add packages/evals/src/semantic_quality/ packages/evals/src/lib.rs
git commit -m "BETA-15B-6 步1：语义质量评测 Recall@k/nDCG@k 指标模块"
```

---

## Task 2：数据类型 + 加载 + 完整性 `data.rs`

**Files:**
- Create: `packages/evals/src/semantic_quality/data.rs`
- Modify: `packages/evals/src/semantic_quality/mod.rs`

- [ ] **Step 1: 写失败测试**

新建 `packages/evals/src/semantic_quality/data.rs`，先写测试（用临时文件 + 内联 JSON）：

```rust
//! 合成语料 / 评测集 / 向量缓存的类型、加载、完整性。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, lang: &str, body: &str) -> SemanticDoc {
        SemanticDoc { doc_id: id.into(), lang: lang.into(), title: id.into(), body: body.into() }
    }
    fn case(id: &str, bucket: &str, rel: &[(&str, u8)]) -> SemanticCase {
        SemanticCase {
            id: id.into(),
            bucket: bucket.into(),
            query: format!("q-{id}"),
            relevant: rel.iter().map(|(d, g)| RelevantDoc { doc_id: (*d).into(), grade: *g }).collect(),
        }
    }

    #[test]
    fn integrity_passes_on_valid_set() {
        let corpus = vec![doc("s1", "zh", "x"), doc("s2", "en", "y")];
        let cases = vec![case("c1", "crosslang", &[("s1", 3), ("s2", 1)])];
        check_integrity(&corpus, &cases).expect("合法集应通过");
    }

    #[test]
    fn integrity_rejects_unknown_doc_id() {
        let corpus = vec![doc("s1", "zh", "x")];
        let cases = vec![case("c1", "synonym", &[("s9", 3)])];
        assert!(check_integrity(&corpus, &cases).is_err());
    }

    #[test]
    fn integrity_rejects_bad_grade() {
        let corpus = vec![doc("s1", "zh", "x")];
        let cases = vec![case("c1", "synonym", &[("s1", 4)])];
        assert!(check_integrity(&corpus, &cases).is_err());
    }

    #[test]
    fn integrity_rejects_unknown_bucket() {
        let corpus = vec![doc("s1", "zh", "x")];
        let cases = vec![case("c1", "nonsense", &[("s1", 3)])];
        assert!(check_integrity(&corpus, &cases).is_err());
    }

    #[test]
    fn vectors_integrity_requires_full_coverage() {
        let corpus = vec![doc("s1", "zh", "x")];
        let cases = vec![case("c1", "synonym", &[("s1", 3)])];
        let mut vc = VectorCache { model_id: "m".into(), dim: 2, doc_vectors: BTreeMap::new(), query_vectors: BTreeMap::new() };
        vc.doc_vectors.insert("s1".into(), vec![1.0, 0.0]);
        vc.query_vectors.insert("c1".into(), vec![1.0, 0.0]);
        check_vectors(&corpus, &cases, &vc).expect("全覆盖应通过");
        // 漏一个 doc 向量 → 失败
        vc.doc_vectors.clear();
        assert!(check_vectors(&corpus, &cases, &vc).is_err());
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::data 2>&1 | tail -5`
Expected: 编译失败（类型/函数未定义）。

- [ ] **Step 3: 实现类型 + 加载 + 完整性**

在 `data.rs` 的 `use` 之后、`mod tests` 之前加：

```rust
/// 合成文档（含正文，语义召回对内容）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticDoc {
    pub doc_id: String,
    pub lang: String, // "zh" | "en"
    pub title: String,
    pub body: String,
}

/// 分级相关文档。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevantDoc {
    pub doc_id: String,
    pub grade: u8, // 1..=3
}

/// 分级相关性评测 case。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCase {
    pub id: String,
    pub bucket: String,
    pub query: String,
    pub relevant: Vec<RelevantDoc>,
}

/// 缓存向量（合成文本的 embedding，无 PII，可提交）。`BTreeMap` 保序确定性。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorCache {
    pub model_id: String,
    pub dim: usize,
    pub doc_vectors: BTreeMap<String, Vec<f32>>,
    pub query_vectors: BTreeMap<String, Vec<f32>>,
}

/// 合法桶（复刻 BETA-26 + 本评测集；ocr 桶预留但本版可不产）。
pub const BUCKETS: &[&str] = &[
    "synonym",
    "concept",
    "crosslang",
    "content-not-name",
    "exact-name",
];

pub fn load_corpus(path: &Path) -> anyhow::Result<Vec<SemanticDoc>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

pub fn load_cases(path: &Path) -> anyhow::Result<Vec<SemanticCase>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

pub fn load_vectors(path: &Path) -> anyhow::Result<VectorCache> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// 语料 + 评测集引用一致性。
pub fn check_integrity(corpus: &[SemanticDoc], cases: &[SemanticCase]) -> anyhow::Result<()> {
    use std::collections::HashSet;
    let doc_ids: HashSet<&str> = corpus.iter().map(|d| d.doc_id.as_str()).collect();
    anyhow::ensure!(doc_ids.len() == corpus.len(), "corpus doc_id 重复");

    let mut case_ids = HashSet::new();
    for c in cases {
        anyhow::ensure!(case_ids.insert(c.id.as_str()), "case id 重复: {}", c.id);
        anyhow::ensure!(!c.query.trim().is_empty(), "case {} query 空", c.id);
        anyhow::ensure!(BUCKETS.contains(&c.bucket.as_str()), "case {} 非法桶 {}", c.id, c.bucket);
        anyhow::ensure!(!c.relevant.is_empty(), "case {} relevant 空", c.id);
        let mut seen = HashSet::new();
        for r in &c.relevant {
            anyhow::ensure!((1..=3).contains(&r.grade), "case {} grade 越界 {}", c.id, r.grade);
            anyhow::ensure!(doc_ids.contains(r.doc_id.as_str()), "case {} 引用未知 doc {}", c.id, r.doc_id);
            anyhow::ensure!(seen.insert(r.doc_id.as_str()), "case {} doc_id 重复 {}", c.id, r.doc_id);
        }
    }
    Ok(())
}

/// 向量缓存覆盖全 doc + 全 case，维度一致。
pub fn check_vectors(corpus: &[SemanticDoc], cases: &[SemanticCase], vc: &VectorCache) -> anyhow::Result<()> {
    for d in corpus {
        let v = vc.doc_vectors.get(&d.doc_id)
            .ok_or_else(|| anyhow::anyhow!("缺 doc 向量: {}", d.doc_id))?;
        anyhow::ensure!(v.len() == vc.dim, "doc {} 维度 {} != {}", d.doc_id, v.len(), vc.dim);
    }
    for c in cases {
        let v = vc.query_vectors.get(&c.id)
            .ok_or_else(|| anyhow::anyhow!("缺 query 向量: {}", c.id))?;
        anyhow::ensure!(v.len() == vc.dim, "case {} 维度 {} != {}", c.id, v.len(), vc.dim);
    }
    Ok(())
}
```

在 `mod.rs` 加 `pub mod data;`。

- [ ] **Step 4: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::data 2>&1 | tail -8 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: 5 测试 PASS、clippy 0、fmt 净。

- [ ] **Step 5: 提交**

```bash
git add packages/evals/src/semantic_quality/
git commit -m "BETA-15B-6 步2：语义质量评测数据类型 + 加载 + 完整性"
```

---

## Task 3：FTS5 臂 `arms.rs`（rusqlite trigram）

**Files:**
- Modify: `packages/evals/Cargo.toml`
- Create: `packages/evals/src/semantic_quality/arms.rs`
- Modify: `packages/evals/src/semantic_quality/mod.rs`

- [ ] **Step 1: 加 rusqlite 依赖**

在 `packages/evals/Cargo.toml` 的 `[dependencies]` 末尾加：

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

- [ ] **Step 2: 写失败测试**

新建 `packages/evals/src/semantic_quality/arms.rs`，先写 FTS 测试：

```rust
//! 三臂排名：FTS5（trigram）/ 向量（cosine + 下限）/ hybrid（生产 fuse_rrf）。

use super::data::SemanticDoc;

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, body: &str) -> SemanticDoc {
        SemanticDoc { doc_id: id.into(), lang: "zh".into(), title: id.into(), body: body.into() }
    }

    #[test]
    fn fts_ranks_trigram_overlap_doc_first() {
        let corpus = vec![
            doc("d1", "年假和远程办公的规定细则"),
            doc("d2", "完全无关的烹饪食谱内容"),
        ];
        let ranked = fts_rank(&corpus, "年假规定", 10).unwrap();
        assert_eq!(ranked.first().map(String::as_str), Some("d1"));
        assert!(!ranked.contains(&"d2".to_owned()), "无关文档不应命中");
    }

    #[test]
    fn fts_empty_query_yields_empty() {
        let corpus = vec![doc("d1", "x")];
        assert!(fts_rank(&corpus, "   ", 10).unwrap().is_empty());
    }
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::arms::tests::fts 2>&1 | tail -5`
Expected: 编译失败（`fts_rank` 未定义）。

- [ ] **Step 4: 实现 FTS 臂**

在 `arms.rs` 的 `use` 之后加：

```rust
use std::collections::HashSet;

/// 把 query 构造成 FTS5 trigram-OR MATCH 串（照搬 BETA-26 已验证逻辑）。
/// 只留 alphanumeric（CJK+latin+digit），重叠 3-char 窗口去重、各自引号包裹、OR 连接；
/// 不足 3 字退化为整串引号匹配；空 → None。
fn build_match_query(query: &str) -> Option<String> {
    let norm: Vec<char> = query.chars().filter(|c| c.is_alphanumeric()).collect();
    if norm.is_empty() {
        return None;
    }
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    if norm.len() < 3 {
        let s: String = norm.iter().collect();
        return Some(quote(&s));
    }
    let mut seen = HashSet::new();
    let mut terms = Vec::new();
    for w in norm.windows(3) {
        let tri: String = w.iter().collect();
        if seen.insert(tri.clone()) {
            terms.push(quote(&tri));
        }
    }
    Some(terms.join(" OR "))
}

/// FTS5 臂：内存 trigram 索引合成正文 → BM25 排名 → top `limit` 的 doc_id（有序）。
pub fn fts_rank(corpus: &[SemanticDoc], query: &str, limit: usize) -> anyhow::Result<Vec<String>> {
    let Some(match_str) = build_match_query(query) else {
        return Ok(Vec::new());
    };
    let conn = rusqlite::Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE corpus_fts USING fts5(id UNINDEXED, text, tokenize='trigram');",
    )?;
    {
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("INSERT INTO corpus_fts(id, text) VALUES (?1, ?2)")?;
            for d in corpus {
                // title + body 一起喂索引（标题命中也算内容信号）。
                let text = format!("{}\n{}", d.title, d.body);
                stmt.execute((&d.doc_id, &text))?;
            }
        }
        tx.commit()?;
    }
    let mut stmt = conn.prepare(
        "SELECT id FROM corpus_fts WHERE corpus_fts MATCH ?1 ORDER BY bm25(corpus_fts) LIMIT ?2",
    )?;
    let rows = stmt.query_map((&match_str, limit as i64), |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
```

在 `mod.rs` 加 `pub mod arms;`。

- [ ] **Step 5: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::arms::tests::fts 2>&1 | tail -8 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: 2 FTS 测试 PASS、clippy 0、fmt 净。

- [ ] **Step 6: 提交**

```bash
git add packages/evals/Cargo.toml packages/evals/src/semantic_quality/
git commit -m "BETA-15B-6 步3：FTS5 trigram 臂（内存 rusqlite）"
```

---

## Task 4：向量臂（cosine + 相似度下限）

**Files:**
- Modify: `packages/evals/Cargo.toml`
- Modify: `packages/evals/src/semantic_quality/arms.rs`

- [ ] **Step 1: 加 indexer 依赖**

在 `packages/evals/Cargo.toml` 的 `[dependencies]` 加：

```toml
locifind-indexer = { path = "../indexer" }
```

- [ ] **Step 2: 写失败测试**

在 `arms.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn vector_ranks_by_cosine_and_applies_floor() {
        use std::collections::BTreeMap;
        let mut docs: BTreeMap<String, Vec<f32>> = BTreeMap::new();
        docs.insert("near".into(), vec![1.0, 0.0]);   // cosine 1.0
        docs.insert("mid".into(), vec![0.6, 0.8]);    // cosine 0.6
        docs.insert("far".into(), vec![0.0, 1.0]);    // cosine 0.0 < floor
        let q = vec![1.0_f32, 0.0];
        let ranked = vector_rank(&q, &docs, 0.30, 10);
        assert_eq!(ranked, vec!["near".to_owned(), "mid".to_owned()]); // far 被下限挡
    }
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::arms::tests::vector 2>&1 | tail -5`
Expected: FAIL（`vector_rank` 未定义）。

- [ ] **Step 4: 实现向量臂**

在 `arms.rs` 加（`use` 区加 `use std::collections::BTreeMap;`）：

```rust
use locifind_indexer::vectors::cosine;
use std::collections::BTreeMap;

/// 向量臂：query 向量 vs 全 doc 向量 cosine → 过滤 `>= floor` → 降序 → top `limit` 的 doc_id。
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
    scored.into_iter().take(limit).map(|(_, id)| id.to_owned()).collect()
}
```

- [ ] **Step 5: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::arms::tests::vector 2>&1 | tail -8 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: PASS、clippy 0、fmt 净。

- [ ] **Step 6: 提交**

```bash
git add packages/evals/Cargo.toml packages/evals/src/semantic_quality/arms.rs
git commit -m "BETA-15B-6 步4：向量臂（cosine + 相似度下限）"
```

---

## Task 5：hybrid 臂（生产 `fuse_rrf`）

**Files:**
- Modify: `packages/evals/Cargo.toml`
- Modify: `packages/evals/src/semantic_quality/arms.rs`

- [ ] **Step 1: 加 result-normalizer 依赖**

在 `packages/evals/Cargo.toml` 的 `[dependencies]` 加：

```toml
locifind-result-normalizer = { path = "../result-normalizer" }
```

- [ ] **Step 2: 写失败测试**

在 `arms.rs` 的 `mod tests` 内追加：

```rust
    #[test]
    fn hybrid_fuses_both_arms_and_weights_semantic() {
        // FTS 只给 d_fts；向量给 d_both（也在 FTS）+ d_vec。双中的 d_both 应靠前。
        let fts = vec!["d_both".to_owned(), "d_fts".to_owned()];
        let vec = vec!["d_both".to_owned(), "d_vec".to_owned()];
        let ranked = hybrid_rank(&fts, &vec, 2.0, 60.0);
        assert_eq!(ranked.first().map(String::as_str), Some("d_both"));
        // 三个 id 都出现
        for id in ["d_both", "d_fts", "d_vec"] {
            assert!(ranked.contains(&id.to_owned()), "{id} 应在融合结果");
        }
    }
```

- [ ] **Step 3: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::arms::tests::hybrid 2>&1 | tail -5`
Expected: FAIL（`hybrid_rank` 未定义）。

- [ ] **Step 4: 实现 hybrid 臂（构造 SearchResult 跑生产 fuse_rrf）**

在 `arms.rs` 加：

```rust
use locifind_result_normalizer::fuse_rrf;
use locifind_search_backend::{BackendKind, MatchType, SearchResult, SearchResultMetadata};
use std::path::PathBuf;

/// 把有序 doc_id 列表包装成生产 `SearchResult`（path/name 用 doc_id，metadata 默认）。
fn to_results(ids: &[String], source: BackendKind, mt: MatchType) -> Vec<SearchResult> {
    ids.iter()
        .map(|id| SearchResult {
            id: id.clone(),
            path: PathBuf::from(id),
            name: id.clone(),
            source,
            match_type: mt,
            score: None,
            metadata: SearchResultMetadata::default(),
        })
        .collect()
}

/// hybrid 臂：FTS 臂(NativeIndex) + 向量臂(SemanticIndex) 喂**生产** `fuse_rrf`，
/// 取融合后有序 doc_id。`semantic_weight`/`k` 即生产融合的两个调优旋钮。
#[must_use]
pub fn hybrid_rank(fts: &[String], vec: &[String], semantic_weight: f64, k: f64) -> Vec<String> {
    let lists = vec![
        to_results(fts, BackendKind::NativeIndex, MatchType::Content),
        to_results(vec, BackendKind::SemanticIndex, MatchType::Semantic),
    ];
    fuse_rrf(lists, k, semantic_weight)
        .into_iter()
        .map(|m| m.result.id)
        .collect()
}
```

> `locifind-search-backend` 已是 evals 依赖（`locifind-search-backend = { path = "../search-backends/common" }`），直接 `use`。

- [ ] **Step 5: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::arms 2>&1 | tail -10 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: 全部 arms 测试 PASS、clippy 0、fmt 净。

- [ ] **Step 6: 提交**

```bash
git add packages/evals/Cargo.toml packages/evals/src/semantic_quality/arms.rs
git commit -m "BETA-15B-6 步5：hybrid 臂（跑生产 fuse_rrf）"
```

---

## Task 6：逐 case 打分 + 分桶聚合 `report.rs`

**Files:**
- Create: `packages/evals/src/semantic_quality/report.rs`
- Modify: `packages/evals/src/semantic_quality/mod.rs`

- [ ] **Step 1: 写失败测试**

新建 `packages/evals/src/semantic_quality/report.rs`，先写测试：

```rust
//! 逐 case 三臂打分 + 分桶聚合。

use super::arms::{fts_rank, hybrid_rank, vector_rank};
use super::data::{SemanticCase, SemanticDoc, VectorCache};
use super::metrics::{ndcg_at_k, recall_at_k};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn aggregate_means_per_bucket_and_overall() {
        let scores = vec![
            CaseScores { id: "a".into(), bucket: "crosslang".into(), fts_recall: 0.0, vec_recall: 1.0, hybrid_recall: 1.0, fts_ndcg: 0.0, vec_ndcg: 1.0, hybrid_ndcg: 1.0 },
            CaseScores { id: "b".into(), bucket: "crosslang".into(), fts_recall: 0.0, vec_recall: 0.0, hybrid_recall: 0.5, fts_ndcg: 0.0, vec_ndcg: 0.0, hybrid_ndcg: 0.5 },
        ];
        let aggs = aggregate(&scores);
        let cl = aggs.iter().find(|a| a.bucket == "crosslang").unwrap();
        assert_eq!(cl.n, 2);
        assert!((cl.hybrid_recall - 0.75).abs() < 1e-9);
        let overall = aggs.iter().find(|a| a.bucket == "OVERALL").unwrap();
        assert_eq!(overall.n, 2);
    }

    #[test]
    fn score_case_runs_three_arms() {
        let corpus = vec![
            SemanticDoc { doc_id: "d1".into(), lang: "zh".into(), title: "年假".into(), body: "年假和远程办公规定".into() },
            SemanticDoc { doc_id: "d2".into(), lang: "en".into(), title: "leave".into(), body: "annual leave policy".into() },
        ];
        let case = SemanticCase {
            id: "c1".into(), bucket: "crosslang".into(), query: "年假规定".into(),
            relevant: vec![super::super::data::RelevantDoc { doc_id: "d1".into(), grade: 3 }],
        };
        let mut vc = VectorCache { model_id: "m".into(), dim: 2, doc_vectors: BTreeMap::new(), query_vectors: BTreeMap::new() };
        vc.doc_vectors.insert("d1".into(), vec![1.0, 0.0]);
        vc.doc_vectors.insert("d2".into(), vec![0.9, 0.1]);
        vc.query_vectors.insert("c1".into(), vec![1.0, 0.0]);
        let s = score_case(&case, &corpus, &vc, 0.30, 2.0, 60.0, 10);
        assert_eq!(s.id, "c1");
        assert!(s.vec_recall > 0.0, "向量臂应召回 d1");
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p locifind-evals semantic_quality::report 2>&1 | tail -5`
Expected: FAIL（类型/函数未定义）。

- [ ] **Step 3: 实现**

在 `report.rs` 的 `use` 之后加：

```rust
/// 单 case 三臂打分。
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
}

/// 分桶（及 OVERALL）均值。
#[derive(Debug, Clone, Serialize)]
pub struct BucketAgg {
    pub bucket: String,
    pub n: usize,
    pub fts_recall: f64,
    pub vec_recall: f64,
    pub hybrid_recall: f64,
    pub fts_ndcg: f64,
    pub vec_ndcg: f64,
    pub hybrid_ndcg: f64,
}

/// 跑三臂 + 算 Recall@k/nDCG@k。`floor`/`weight`/`k_rrf` 是生产融合旋钮。
#[must_use]
pub fn score_case(
    case: &SemanticCase,
    corpus: &[SemanticDoc],
    vectors: &VectorCache,
    floor: f32,
    weight: f64,
    k_rrf: f64,
    top_k: usize,
) -> CaseScores {
    let relevant_set: HashSet<String> = case.relevant.iter().map(|r| r.doc_id.clone()).collect();
    let grades: HashMap<String, u8> = case.relevant.iter().map(|r| (r.doc_id.clone(), r.grade)).collect();

    // 各臂取较深 pool（50）再算 @k，贴近生产 fan-out。
    const POOL: usize = 50;
    let fts = fts_rank(corpus, &case.query, POOL).unwrap_or_default();
    let empty = Vec::new();
    let qv = vectors.query_vectors.get(&case.id).unwrap_or(&empty);
    let vec = vector_rank(qv, &vectors.doc_vectors, floor, POOL);
    let hybrid = hybrid_rank(&fts, &vec, weight, k_rrf);

    CaseScores {
        id: case.id.clone(),
        bucket: case.bucket.clone(),
        fts_recall: recall_at_k(&fts, &relevant_set, top_k),
        vec_recall: recall_at_k(&vec, &relevant_set, top_k),
        hybrid_recall: recall_at_k(&hybrid, &relevant_set, top_k),
        fts_ndcg: ndcg_at_k(&fts, &grades, top_k),
        vec_ndcg: ndcg_at_k(&vec, &grades, top_k),
        hybrid_ndcg: ndcg_at_k(&hybrid, &grades, top_k),
    }
}

/// 分桶（保出现序）+ OVERALL 均值。
#[must_use]
pub fn aggregate(scores: &[CaseScores]) -> Vec<BucketAgg> {
    let mut buckets: Vec<String> = Vec::new();
    for s in scores {
        if !buckets.contains(&s.bucket) {
            buckets.push(s.bucket.clone());
        }
    }
    let agg_for = |subset: &[&CaseScores], name: &str| -> BucketAgg {
        let n = subset.len();
        let mean = |sel: &dyn Fn(&CaseScores) -> f64| -> f64 {
            if n == 0 { 0.0 } else { subset.iter().map(|s| sel(s)).sum::<f64>() / n as f64 }
        };
        BucketAgg {
            bucket: name.to_owned(),
            n,
            fts_recall: mean(&|s| s.fts_recall),
            vec_recall: mean(&|s| s.vec_recall),
            hybrid_recall: mean(&|s| s.hybrid_recall),
            fts_ndcg: mean(&|s| s.fts_ndcg),
            vec_ndcg: mean(&|s| s.vec_ndcg),
            hybrid_ndcg: mean(&|s| s.hybrid_ndcg),
        }
    };
    let mut out: Vec<BucketAgg> = buckets
        .iter()
        .map(|b| {
            let subset: Vec<&CaseScores> = scores.iter().filter(|s| &s.bucket == b).collect();
            agg_for(&subset, b)
        })
        .collect();
    let all: Vec<&CaseScores> = scores.iter().collect();
    out.push(agg_for(&all, "OVERALL"));
    out
}
```

在 `mod.rs` 加 `pub mod report;`。

- [ ] **Step 4: 运行确认通过 + clippy/fmt**

Run: `cargo test -p locifind-evals semantic_quality::report 2>&1 | tail -8 && cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: 2 测试 PASS、clippy 0、fmt 净。

- [ ] **Step 5: 提交**

```bash
git add packages/evals/src/semantic_quality/
git commit -m "BETA-15B-6 步6：逐 case 三臂打分 + 分桶聚合"
```

---

## Task 7：`semantic_quality` binary（跑缓存 / --json / --write-baseline）

**Files:**
- Modify: `packages/evals/Cargo.toml`
- Create: `packages/evals/src/bin/semantic_quality.rs`

> 本 task 只做**读缓存跑评测 + 报告 + 写 baseline**；`--embed`（需模型）留 Task 8。

- [ ] **Step 1: 注册 bin**

在 `packages/evals/Cargo.toml` 末尾加：

```toml
[[bin]]
name = "semantic_quality"
path = "src/bin/semantic_quality.rs"
```

- [ ] **Step 2: 实现 binary（无独立单测，靠 Task 9 gate + 手跑验证）**

新建 `packages/evals/src/bin/semantic_quality.rs`：

```rust
//! BETA-15B-6：语义召回质量评测 CLI。默认读 checked-in 合成语料 + 缓存向量，
//! 跑三臂 Recall@10/nDCG@10 分桶报告。`--json` 机读；`--write-baseline` 写 baseline.json。

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use locifind_evals::semantic_quality::data::{
    check_integrity, check_vectors, load_cases, load_corpus, load_vectors,
};
use locifind_evals::semantic_quality::report::{aggregate, score_case, BucketAgg};
use locifind_evals::semantic_quality::{EVAL_SIMILARITY_FLOOR, TOP_K};
use locifind_result_normalizer::{DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT};

#[derive(Parser)]
#[command(name = "semantic_quality", about = "BETA-15B-6 语义召回质量评测")]
struct Cli {
    /// 输出 JSON（分桶聚合）。
    #[arg(long)]
    json: bool,
    /// 把当前 hybrid 分桶结果写成 baseline.json（用户 bootstrap 用）。
    #[arg(long)]
    write_baseline: bool,
}

fn fixt(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/semantic-recall").join(rel)
}

fn print_table(aggs: &[BucketAgg]) {
    println!("{:<18} {:>3} | {:>7} {:>7} | {:>7} {:>7} | {:>7} {:>7}",
        "bucket", "n", "FTS_R", "FTS_N", "VEC_R", "VEC_N", "HYB_R", "HYB_N");
    println!("{}", "-".repeat(78));
    for a in aggs {
        println!("{:<18} {:>3} | {:>7.3} {:>7.3} | {:>7.3} {:>7.3} | {:>7.3} {:>7.3}",
            a.bucket, a.n, a.fts_recall, a.fts_ndcg, a.vec_recall, a.vec_ndcg, a.hybrid_recall, a.hybrid_ndcg);
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let corpus = load_corpus(&fixt("corpus.json")).expect("读 corpus.json");
    let cases = load_cases(&fixt("cases.json")).expect("读 cases.json");
    check_integrity(&corpus, &cases).expect("语料/评测集完整性");
    let vectors = load_vectors(&fixt("vectors.json"))
        .expect("读 vectors.json（缺则先跑 --embed，见 README）");
    check_vectors(&corpus, &cases, &vectors).expect("向量覆盖完整性");

    let scores: Vec<_> = cases.iter()
        .map(|c| score_case(c, &corpus, &vectors, EVAL_SIMILARITY_FLOOR, DEFAULT_SEMANTIC_WEIGHT, DEFAULT_RRF_K, TOP_K))
        .collect();
    let aggs = aggregate(&scores);

    if cli.write_baseline {
        // baseline.json = 分桶 hybrid recall/ndcg（gate 比对锚点）。
        let json = serde_json::to_string_pretty(&aggs).expect("序列化 baseline");
        std::fs::write(fixt("baseline.json"), json).expect("写 baseline.json");
        eprintln!("已写 baseline.json（{} 桶含 OVERALL）", aggs.len());
    }

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&aggs).expect("序列化"));
    } else {
        print_table(&aggs);
    }
    ExitCode::SUCCESS
}
```

> 需 `locifind_evals::semantic_quality::*` 公开可达——确认 `lib.rs` 已 `pub mod semantic_quality;`、各子模块 `pub mod`。`DEFAULT_RRF_K`/`DEFAULT_SEMANTIC_WEIGHT` 从 `locifind-result-normalizer` 导出（已是依赖）。

- [ ] **Step 3: 编译验证（无 fixtures 时不跑，仅编译）**

Run: `cargo build -p locifind-evals --bin semantic_quality 2>&1 | tail -6`
Expected: 编译通过（运行需 fixtures，留 Phase B/D）。

- [ ] **Step 4: clippy/fmt + 提交**

Run: `cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: clippy 0、fmt 净。

```bash
git add packages/evals/Cargo.toml packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-6 步7：semantic_quality binary（跑缓存 + 报告 + 写 baseline）"
```

---

## Task 8：`--embed` 子模式（feature 门控，生成向量缓存）

**Files:**
- Modify: `packages/evals/Cargo.toml`
- Modify: `packages/evals/src/bin/semantic_quality.rs`

- [ ] **Step 1: 加 feature**

在 `packages/evals/Cargo.toml` 的 `[features]` 加（与现有 `model-fallback` 同款拉 llama-cpp）：

```toml
semantic-recall = ["locifind-model-runtime/llama-cpp"]
semantic-recall-metal = ["semantic-recall", "locifind-model-runtime/metal"]
```

- [ ] **Step 2: 实现 --embed（feature gate）**

在 `semantic_quality.rs` 的 `Cli` 加字段：

```rust
    /// 调模型重算 doc+query 向量、写 vectors.json（需 feature semantic-recall + 模型）。
    #[arg(long)]
    embed: bool,
    /// embedding 模型路径（仅 --embed）。
    #[arg(long, default_value = "models/qwen3-embedding-0.6b-q8_0.gguf")]
    model: String,
```

在 `main()` 顶部、`check_integrity` 之后、`load_vectors` 之前插入 embed 分支：

```rust
    if cli.embed {
        #[cfg(feature = "semantic-recall")]
        {
            embed_and_write(&corpus, &cases, &cli.model);
            eprintln!("已写 vectors.json");
            return ExitCode::SUCCESS;
        }
        #[cfg(not(feature = "semantic-recall"))]
        {
            eprintln!("--embed 需 feature semantic-recall（且放好模型）。见 fixtures/semantic-recall/README.md");
            return ExitCode::from(2);
        }
    }
```

在文件末尾加 feature-gated 生成函数：

```rust
#[cfg(feature = "semantic-recall")]
fn embed_and_write(
    corpus: &[locifind_evals::semantic_quality::data::SemanticDoc],
    cases: &[locifind_evals::semantic_quality::data::SemanticCase],
    model: &str,
) {
    use locifind_evals::semantic_quality::data::VectorCache;
    use locifind_model_runtime::{get_default_loader, ModelLoadParams};
    use std::collections::BTreeMap;
    use std::path::Path;

    let loader = get_default_loader();
    let rt = loader
        .load(Path::new(model), &ModelLoadParams { gpu_layers: 99, context_size: 2048 })
        .expect("加载 embedding 模型");

    let embed = |text: &str| -> Vec<f32> {
        let t: String = text.chars().take(1200).collect(); // 截断同 BETA-26
        rt.embed(&t).expect("embed 失败")
    };

    let mut doc_vectors = BTreeMap::new();
    let mut dim = 0usize;
    for d in corpus {
        let v = embed(&format!("{}\n{}", d.title, d.body));
        dim = v.len();
        doc_vectors.insert(d.doc_id.clone(), v);
    }
    let mut query_vectors = BTreeMap::new();
    for c in cases {
        query_vectors.insert(c.id.clone(), embed(&c.query));
    }
    let vc = VectorCache { model_id: model.to_owned(), dim, doc_vectors, query_vectors };
    let json = serde_json::to_string(&vc).expect("序列化向量");
    std::fs::write(fixt("vectors.json"), json).expect("写 vectors.json");
}
```

> `get_default_loader`/`ModelLoadParams`/`.embed()` 签名见 `spike-retrieval/src/bin/embed_corpus.rs`（同款）。若 `model_id` 实际用更精确串，照模型文件名记。

- [ ] **Step 3: 编译验证（默认 + feature 两形态）**

Run: `cargo build -p locifind-evals --bin semantic_quality 2>&1 | tail -4`
Expected: 默认构建通过（不编 llama-cpp，`--embed` 走 not-feature 分支提示）。
> feature 形态 `cargo build -p locifind-evals --bin semantic_quality --features semantic-recall` 需 cmake/模型，**留 Phase D 用户验**；本步只确认默认构建编过 + cfg 分支语法正确。

- [ ] **Step 4: clippy/fmt + 提交**

Run: `cargo clippy -p locifind-evals --all-targets -- -D warnings 2>&1 | tail -3 && cargo fmt -p locifind-evals --check`
Expected: clippy 0（默认形态）、fmt 净。

```bash
git add packages/evals/Cargo.toml packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-6 步8：--embed 子模式（feature 门控生成向量缓存）"
```

---

## Task 9：回归门集成测试（skip-if-missing）

**Files:**
- Create: `packages/evals/tests/semantic_quality_gate.rs`

- [ ] **Step 1: 写测试**

新建 `packages/evals/tests/semantic_quality_gate.rs`：

```rust
//! BETA-15B-6 回归门：合成集 hybrid 在关键桶不跌破提交 baseline。
//! 跑 checked-in 缓存向量（确定性）。vectors.json / baseline.json 未提交（Phase D 前）→ 跳过。

use std::path::{Path, PathBuf};

use locifind_evals::semantic_quality::data::{check_integrity, check_vectors, load_cases, load_corpus, load_vectors};
use locifind_evals::semantic_quality::report::{aggregate, score_case, BucketAgg};
use locifind_evals::semantic_quality::{EVAL_SIMILARITY_FLOOR, TOP_K};
use locifind_result_normalizer::{DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT};

fn fixt(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/semantic-recall").join(rel)
}

#[test]
fn hybrid_does_not_regress_key_buckets_vs_baseline() {
    if !fixt("vectors.json").exists() || !fixt("baseline.json").exists() {
        eprintln!("跳过：vectors.json / baseline.json 未提交（Phase D 用户 bootstrap 前）");
        return;
    }
    let corpus = load_corpus(&fixt("corpus.json")).unwrap();
    let cases = load_cases(&fixt("cases.json")).unwrap();
    check_integrity(&corpus, &cases).unwrap();
    let vectors = load_vectors(&fixt("vectors.json")).unwrap();
    check_vectors(&corpus, &cases, &vectors).unwrap();

    let scores: Vec<_> = cases.iter()
        .map(|c| score_case(c, &corpus, &vectors, EVAL_SIMILARITY_FLOOR, DEFAULT_SEMANTIC_WEIGHT, DEFAULT_RRF_K, TOP_K))
        .collect();
    let aggs = aggregate(&scores);

    let baseline: Vec<BucketAgg> =
        serde_json::from_str(&std::fs::read_to_string(fixt("baseline.json")).unwrap()).unwrap();
    let base_of = |b: &str| baseline.iter().find(|a| a.bucket == b).cloned();
    let now_of = |b: &str| aggs.iter().find(|a| a.bucket == b).cloned();

    const EPS: f64 = 1e-6;
    for bucket in ["crosslang", "exact-name", "OVERALL"] {
        if let (Some(base), Some(now)) = (base_of(bucket), now_of(bucket)) {
            assert!(
                now.hybrid_recall + EPS >= base.hybrid_recall,
                "{bucket} hybrid Recall@10 回退: {:.3} < baseline {:.3}",
                now.hybrid_recall, base.hybrid_recall
            );
        }
    }
}
```

- [ ] **Step 2: 运行确认通过（当前 skip）**

Run: `cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -6`
Expected: PASS（vectors/baseline 未提交 → 测试内 skip 分支、`test result: ok`）。

- [ ] **Step 3: 提交**

```bash
git add packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-6 步9：hybrid 回归门集成测试（skip-if-missing）"
```

---

## Task 10：合成语料 authoring（content，生成 subagent）

**Files:**
- Create: `packages/evals/fixtures/semantic-recall/corpus.json`
- Create: `packages/evals/fixtures/semantic-recall/README.md`

> 这是内容生成任务（无代码逻辑）。生成后须过 Task 2 的 `check_integrity` 间接约束（doc_id 唯一）+ 隐私自查。

- [ ] **Step 1: 生成合成语料**

用 LLM 生成 `corpus.json`——一个 `SemanticDoc[]` 数组（字段 `doc_id`/`lang`/`title`/`body`），要求：
- **规模 ~150–250 篇**，zh 与 en 大致各半。
- **虚构似真个人文档**：假简历、会议纪要、项目报告、学习笔记、政策文档、技术方案等；正文 ~80–400 字，似真但**完全虚构**。
- **跨语言配对主题**：至少 ~15 组"同一虚构主题的 zh 文档 + en 文档"（如"年假政策"中英各一），让跨语言 query 能命中、且词面不共享。
- **充足干扰文档**：大量主题各异的 distractor，使 top-10 命中非平凡。
- `doc_id` 用 `s00001` 递增、唯一。
- **隐私硬约束（CONVENTIONS §7）**：**零真实 PII**——不得含任何真实人名/公司/邮箱/电话/精确财务数字/真实路径。用明显虚构的名字（如"李示例""Acme Corp""sample@example.com"）。

- [ ] **Step 2: 写元数据 README**

新建 `packages/evals/fixtures/semantic-recall/README.md`，记（风险清单 §5.2）：
```markdown
# 语义召回质量评测集（BETA-15B-6）

- dataset_name: semantic-recall-quality
- version: v1
- generation_method: LLM 生成虚构文档 + 人工/规则复核，全合成零 PII
- privacy_review_status: reviewed —— 无真实人名/公司/邮箱/财务/路径
- created_at: 2026-06-21
- reviewer: <填工具名>
- corpus: corpus.json（~N 篇合成多语言文档）
- cases: cases.json（~M 条 graded 相关性，5 桶）
- vectors: vectors.json（model_id=qwen3-embedding-0.6b，--embed 生成；合成文本 embedding，无 PII）
- baseline: baseline.json（当前配置 hybrid 分桶锚点，gate 比对）

## 隐私自查清单（commit 前）
- [ ] corpus/cases 无真实人名、公司、邮箱、电话、精确薪资、真实绝对路径
- [ ] 跨语言桶是虚构配对主题
- [ ] doc_id / case id 唯一

## 跑法
- 评测（读缓存）：`cargo run -p locifind-evals --bin semantic_quality`
- 生成向量（一次，需模型）：`cargo run -p locifind-evals --bin semantic_quality --features semantic-recall-metal -- --embed`
- 写 baseline：`... --write-baseline`
```

- [ ] **Step 3: 隐私自查 + 提交**

人工/grep 自查无 PII（如 `grep -nE '@(gmail|qq|163)\.com|/Users/|身份证|[0-9]{6,}' corpus.json` 应无敏感命中）。

```bash
git add packages/evals/fixtures/semantic-recall/corpus.json packages/evals/fixtures/semantic-recall/README.md
git commit -m "BETA-15B-6 步10：合成多语言语料集（~N 篇，零 PII）"
```

---

## Task 11：分级相关性评测集 authoring（content，生成 subagent）

**Files:**
- Create: `packages/evals/fixtures/semantic-recall/cases.json`

- [ ] **Step 1: 生成 cases**

用 LLM 生成 `cases.json`——`SemanticCase[]`（字段 `id`/`bucket`/`query`/`relevant:[{doc_id,grade}]`），要求：
- **~50–70 条**，5 桶大致均衡：
  - `synonym`：同义改述 query 指向同一文档。
  - `concept`：概念/主题跳跃（高抽象描述特定内容）。
  - `crosslang`：中文 query → 英文文档（或反向），用 Task 10 的配对主题，**词面不共享**。
  - `content-not-name`：query 描述内容要点而非文件名。
  - `exact-name`：query = 某合成文档的精确标题（守护桶，验语义不拖垮精确）。
- `relevant` 用 Task 10 真实存在的 `doc_id`，grade 1–3（3 完全相关）。
- `id` 用 `c001` 递增唯一。

- [ ] **Step 2: 完整性验证**

写一个一次性检查（或临时 `cargo test`）确认 `check_integrity(corpus, cases)` 通过：所有 `relevant.doc_id ∈ corpus`、bucket 合法、grade∈1..=3、id 唯一。可临时加一个 `#[ignore]` 测试或直接跑 binary 的 `check_integrity`（binary 启动即校验）。

Run（间接验证，需 vectors 暂缺会在 vectors 步报错——此处只验 integrity，可临时注释 binary 的 load_vectors 跑一次，或写个 `tests` 临时断言）：用 Task 9 思路写一个**临时**完整性测试跑 `check_integrity` 后删除，或人工核对。
Expected: check_integrity 无 panic。

- [ ] **Step 3: 提交**

```bash
git add packages/evals/fixtures/semantic-recall/cases.json
git commit -m "BETA-15B-6 步11：分级相关性评测集（~M 条，5 桶）"
```

---

## Task 12：spike-retrieval 转正 + 清 llama-cpp 污染

**Files:**
- Modify: `packages/spike-retrieval/Cargo.toml`
- Create/Modify: `packages/spike-retrieval/README.md`

- [ ] **Step 1: llama-cpp 改可选 feature**

`packages/spike-retrieval/Cargo.toml`：把 model-runtime 依赖的无条件 `features = ["llama-cpp"]` 去掉，改用本 crate 的 feature 透传。改为：

```toml
model-runtime = { package = "locifind-model-runtime", path = "../model-runtime" }
```

并在 `[features]` 加（已有 `metal`）：

```toml
[features]
default = []
llama-cpp = ["model-runtime/llama-cpp"]
metal = ["llama-cpp", "model-runtime/metal"]
```

把三个 `[[bin]]`（build-corpus/embed-corpus/run-retrieval 中调 embed 的）加 `required-features`：embed-corpus / run-retrieval 调模型 → `required-features = ["llama-cpp"]`；build-corpus 不调模型 → 不加。

```toml
[[bin]]
name = "embed-corpus"
path = "src/bin/embed_corpus.rs"
required-features = ["llama-cpp"]

[[bin]]
name = "run-retrieval"
path = "src/bin/run_retrieval.rs"
required-features = ["llama-cpp"]
```

> 若 `embed_corpus.rs`/`run_retrieval.rs` 顶层无条件 `use model_runtime::...embed`，确保它们在无 llama-cpp 时不编译——`required-features` 已保证不参与默认构建。

- [ ] **Step 2: 改注释 + README 转正**

`Cargo.toml` 顶部注释从「一次性探针，GO/NO-GO 后可整包删除」改为：

```toml
# BETA-26 起源的语义检索探针；BETA-15B-6 起转为「本地现实校准锤」——
# 用真实 home 数据（gitignored）周期性核对合成评测集是否同向。默认构建不编 llama-cpp。
```

新建/更新 `packages/spike-retrieval/README.md` 说明：它是本地现实校准工具，真实 corpus/vectors/evalset 仍 gitignored；跑法 `--features metal`；与 `packages/evals` 合成集的关系（合成集若与真实集背离则警示合成集失真）。

- [ ] **Step 3: 验证 workspace 默认不再编 llama-cpp**

Run: `cargo build --workspace 2>&1 | tail -6 && cargo tree -p spike-retrieval -e features 2>&1 | grep -i llama || echo "默认无 llama-cpp（预期）"`
Expected: 默认 workspace 构建不拉 llama-cpp（spike-retrieval 默认 feature 空）；`cargo test --workspace` 不再因 spike-retrieval 编译 llama-cpp。

- [ ] **Step 4: 全 workspace 测试 + clippy/fmt**

Run: `cargo test --workspace 2>&1 | tail -8 && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5 && cargo fmt --check 2>&1 | tail -3`
Expected: 0 failed、clippy 0、fmt 净。
> 注：BETA-15B-2 提到的"stub-loader 测试 + let-else 止血"——确认这些测试现在因 spike-retrieval 不再无条件拉 llama-cpp 而恢复正常；若仍有 let-else 守卫可视情况保留（不在本 task 强行清理，除非测试失败指向它）。

- [ ] **Step 5: 提交**

```bash
git add packages/spike-retrieval/Cargo.toml packages/spike-retrieval/README.md
git commit -m "BETA-15B-6 步12：spike-retrieval 转正本地校准锤 + llama-cpp 改可选 feature"
```

---

## Task 13：用户 bootstrap（Phase D，**USER 执行，非 subagent**）

**Files:**
- Create: `packages/evals/fixtures/semantic-recall/vectors.json`
- Create: `packages/evals/fixtures/semantic-recall/baseline.json`
- Create: `docs/reviews/semantic-recall-quality-baseline.md`

> 需 `semantic-recall` feature + embedding 模型 + (Mac) Metal，归用户（同真机手测）。subagent 跳过，输出指引给用户。

- [ ] **Step 1: 生成向量缓存**（用户，Mac）

```bash
cargo run -p locifind-evals --bin semantic_quality --features semantic-recall-metal -- --embed
```
产出 `fixtures/semantic-recall/vectors.json`（合成文本 embedding，无 PII，可提交）。

- [ ] **Step 2: 写 baseline + 看报告**

```bash
cargo run -p locifind-evals --bin semantic_quality -- --write-baseline   # 写 baseline.json
cargo run -p locifind-evals --bin semantic_quality                       # 看分桶表
```

- [ ] **Step 3: baseline 报告**

把分桶 × 三臂 Recall@10/nDCG@10 表 + 配置（weight=2.0/floor=0.30/model_id）+ 与 BETA-26 真实集定性对照写进 `docs/reviews/semantic-recall-quality-baseline.md`。

- [ ] **Step 4: 激活 gate + 提交**

提交 `vectors.json` + `baseline.json` 后，Task 9 的 gate 自动从 skip 转为常跑。验证：

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -6   # 现应真跑、PASS
git add packages/evals/fixtures/semantic-recall/vectors.json packages/evals/fixtures/semantic-recall/baseline.json docs/reviews/semantic-recall-quality-baseline.md
git commit -m "BETA-15B-6 步13：提交合成向量缓存 + baseline + 报告（激活回归门）"
```

- [ ] **Step 5: 现实校准锤交叉核对（可选，用户）**

跑 `spike-retrieval` 真实集（`--features metal`），确认合成集的 hybrid>FTS5 趋势与真实集**同向**（若背离则合成集失真，记入 baseline 报告待下一 cycle 调语料）。

---

## Self-Review（计划对照 spec）

- **spec §2.1.1 合成语料** → Task 10 ✅
- **spec §2.1.2 分级评测集** → Task 11 ✅
- **spec §2.1.3 排名跑法+指标（Recall@10/nDCG@10 × 三臂，跑生产融合）** → Task 1（指标）+ Task 3/4/5（三臂，hybrid 用生产 fuse_rrf）+ Task 6（打分聚合）+ Task 7（binary）✅
- **spec §2.1.4 提交合成向量缓存** → Task 8（--embed 生成）+ Task 13（用户提交）✅
- **spec §2.1.5 回归门** → Task 9（skip-if-missing，激活于 Task 13）✅
- **spec §2.1.6 真实本地锤转正** → Task 12 ✅
- **spec §2.1.7 baseline 报告** → Task 13 ✅
- **spec §4.8 测试/隐私** → Task 1/2/3/4/5/6 单测 + Task 2 完整性 + Task 10 隐私自查 + 各 task clippy/fmt；生产 byte-equal 不变（本计划不碰 parser/索引/融合生产路径——仅**新增** evals 模块 + 读 result-normalizer/indexer 的**公开** API，不改它们）✅
- **spec §2.2 非目标**：不调优（无权重/下限/模型改动）✅；真实 PII 不进 packages/evals（Task 10 全合成、真实数据留 spike-retrieval gitignored）✅；不并入 byte-equal parser 闸门（独立 gate）✅
- **占位扫描**：无 TBD/TODO；Task 11 Step 2 的"临时完整性验证"给了两条可行路径（临时测试 / binary 启动校验），非占位。
- **类型一致**：`SemanticDoc{doc_id,lang,title,body}`、`SemanticCase{id,bucket,query,relevant}`、`RelevantDoc{doc_id,grade}`、`VectorCache{model_id,dim,doc_vectors,query_vectors}`、`CaseScores`/`BucketAgg`、`recall_at_k`/`ndcg_at_k`/`fts_rank`/`vector_rank`/`hybrid_rank`/`score_case`/`aggregate` 全程一致 ✅
- **依赖新增**：rusqlite(Task3) / locifind-indexer(Task4) / locifind-result-normalizer(Task5) / semantic-recall feature(Task8)——收工时登记 `docs/third-party-licenses.md`（rusqlite 已被 spike-retrieval 引入、indexer/result-normalizer 是内部 crate）。
```
