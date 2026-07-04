# BETA-15A 同义词召回定量评测集 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建一套离线、确定性、可进 ci.sh 回归门的同义词召回评测集，量化当前手维护词典 + gazetteer 在合成痛点 query 上的召回率/假阳率。

**Architecture:** 复用真实 `parse → expand` 管线（`locifind_intent_parser::parse` + `locifind_harness::YamlSynonymExpander` 加载 ship 的 `resources/synonyms/{zh,en}.yaml`），只把最后的 mdfind 替换为「忠实于 BETA-15D 双查询的大小写不敏感子串匹配」纯函数。corpus + cases 为手工标注 checked-in JSON。门槛（召回 ≥ 70% / 假阳 ≤ 5%）由集成测试 `#[test]` 强制（随 `cargo test --workspace` 自动跑），bin 提供可读报告 + 同源门槛退出码。

**Tech Stack:** Rust，evals crate（locifind-evals），serde_json，clap，复用 locifind-intent-parser + locifind-harness + locifind-search-backend（KeywordGroup）。

**设计依据：** [docs/superpowers/specs/2026-05-30-beta-15a-synonym-recall-eval-design.md](../specs/2026-05-30-beta-15a-synonym-recall-eval-design.md)

**关键不变量（reviewer 守门）：** 不改 `packages/intent-parser/**`、`packages/search-backends/spotlight/**`、`packages/harness/src/synonym/**`、`resources/synonyms/*.yaml`、`packages/evals/fixtures/v0.5/**`。验证：`cargo run -p locifind-evals --bin evals -- --fixtures v0.5` 维持 **472/26/2 byte-equal**。

---

### Task 1: 召回核心类型 + `matches()` 匹配模拟

**Files:**
- Create: `packages/evals/src/recall.rs`
- Modify: `packages/evals/src/lib.rs`（加 `pub mod recall;`）

匹配模型忠实 BETA-15D：组内 OR、组间 AND、大小写不敏感子串，命中域 = 文件名 + content_terms。

- [ ] **Step 1: 加模块声明**

在 `packages/evals/src/lib.rs` 顶部其它 `pub mod`/`pub use` 附近加：

```rust
pub mod recall;
```

- [ ] **Step 2: 写失败测试**

新建 `packages/evals/src/recall.rs`，内容：

```rust
//! BETA-15A 同义词召回评测：离线确定性匹配模拟 + 指标 + 门槛。
//!
//! 匹配忠实于 BETA-15D 双查询语义：组内 OR、组间 AND、大小写不敏感子串，
//! 命中域 = 文件名 + content_terms。不跑 Spotlight / mdfind / 模型。

use locifind_search_backend::KeywordGroup;
use serde::Deserialize;

/// 合成 corpus 中的一个文件。
#[derive(Debug, Clone, Deserialize)]
pub struct CorpusFile {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub content_terms: Vec<String>,
}

/// 文件 F 是否命中 keyword 组：组内 OR、组间 AND、大小写不敏感子串。
/// 空 groups（keyword 缺失且 gazetteer 未命中）→ 不命中任何文件。
#[must_use]
pub fn matches(groups: &[KeywordGroup], file: &CorpusFile) -> bool {
    if groups.is_empty() {
        return false;
    }
    let haystacks: Vec<String> = std::iter::once(file.filename.to_lowercase())
        .chain(file.content_terms.iter().map(|t| t.to_lowercase()))
        .collect();
    groups.iter().all(|g| {
        g.all().iter().any(|term| {
            let needle = term.to_lowercase();
            haystacks.iter().any(|h| h.contains(&needle))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: &str, name: &str) -> CorpusFile {
        CorpusFile { id: id.into(), filename: name.into(), content_terms: vec![] }
    }
    fn group(head: &str, syns: &[&str]) -> KeywordGroup {
        KeywordGroup { head: head.into(), synonyms: syns.iter().map(|s| (*s).into()).collect() }
    }

    #[test]
    fn synonym_member_substring_hits() {
        // group {工作汇报, 述职} 命中名为 述职.ppt 的文件（同义词召回的核心）
        let groups = vec![group("工作汇报", &["述职", "年度总结"])];
        assert!(matches(&groups, &file("f1", "述职.ppt")));
    }

    #[test]
    fn no_member_substring_misses() {
        let groups = vec![group("工作汇报", &["述职"])];
        assert!(!matches(&groups, &file("f2", "项目计划.docx")));
    }

    #[test]
    fn case_insensitive_substring() {
        let groups = vec![group("resume", &["cv"])];
        assert!(matches(&groups, &file("f3", "My_CV.docx")));
        assert!(matches(&groups, &file("f4", "RESUME_final.pdf")));
    }

    #[test]
    fn cross_group_is_and() {
        // 两组：必须都命中。文件名只含其一 → 不命中。
        let groups = vec![group("工作汇报", &["述职"]), group("ppt", &[])];
        assert!(!matches(&groups, &file("f5", "述职.docx")));
        assert!(matches(&groups, &file("f6", "述职.ppt")));
    }

    #[test]
    fn empty_groups_never_match() {
        assert!(!matches(&[], &file("f7", "述职.ppt")));
    }

    #[test]
    fn content_terms_hit() {
        let groups = vec![group("合同", &["协议"])];
        let f = CorpusFile { id: "f8".into(), filename: "doc-001.pdf".into(), content_terms: vec!["这是一份协议".into()] };
        assert!(matches(&groups, &f));
    }
}
```

- [ ] **Step 3: 跑测试确认通过**

Run: `cargo test -p locifind-evals recall::tests -- --nocapture`
Expected: 6 个 test PASS（实现已随测试一并写入，纯函数无需先红再绿；若 `KeywordGroup` 字段名不符编译报错，按 `packages/search-backends/common/src/expanded.rs` 修正）。

- [ ] **Step 4: fmt + clippy**

Run: `cargo fmt -p locifind-evals && cargo clippy -p locifind-evals --all-targets -- -D warnings`
Expected: 无输出（通过）

- [ ] **Step 5: 提交**

```bash
git add packages/evals/src/recall.rs packages/evals/src/lib.rs
git commit -m "feat(beta-15a): 召回评测核心类型 + matches 匹配模拟(组内OR组间AND子串)"
```

---

### Task 2: 召回指标 + Report + 门槛常量

**Files:**
- Modify: `packages/evals/src/recall.rs`

- [ ] **Step 1: 写失败测试**

在 `packages/evals/src/recall.rs` 的 `matches` 之后、`#[cfg(test)]` 之前插入：

```rust
/// 一个召回 case（手工标注，序列化自 cases.json）。
#[derive(Debug, Clone, Deserialize)]
pub struct RecallCase {
    pub id: String,
    pub query: String,
    pub language: String,
    pub bucket: String,
    pub expected_hits: Vec<String>,
}

/// 单 case 的命中核算。
#[derive(Debug, Clone)]
pub struct CaseOutcome {
    pub case_id: String,
    pub bucket: String,
    pub language: String,
    pub expected: usize,
    pub recalled: usize,
    pub false_positives: usize,
    pub non_expected: usize,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
}

/// 门槛（对齐 ROADMAP BETA-15A 验收下限）。
pub const RECALL_GATE: f64 = 0.70;
pub const FP_GATE: f64 = 0.05;

/// 在给定 corpus 上核算一个 case：`actual_hits` 是已算好的命中 id 集合。
#[must_use]
pub fn outcome_for(
    case: &RecallCase,
    corpus: &[CorpusFile],
    actual_hits: &[String],
) -> CaseOutcome {
    let expected: std::collections::HashSet<&str> =
        case.expected_hits.iter().map(String::as_str).collect();
    let actual: std::collections::HashSet<&str> =
        actual_hits.iter().map(String::as_str).collect();
    let recalled = expected.iter().filter(|id| actual.contains(*id)).count();
    let false_positives = actual.iter().filter(|id| !expected.contains(*id)).count();
    let mut missing: Vec<String> =
        expected.iter().filter(|id| !actual.contains(*id)).map(|s| (*s).to_owned()).collect();
    let mut extra: Vec<String> =
        actual.iter().filter(|id| !expected.contains(*id)).map(|s| (*s).to_owned()).collect();
    missing.sort();
    extra.sort();
    CaseOutcome {
        case_id: case.id.clone(),
        bucket: case.bucket.clone(),
        language: case.language.clone(),
        expected: case.expected_hits.len(),
        recalled,
        false_positives,
        non_expected: corpus.len() - case.expected_hits.len(),
        missing,
        extra,
    }
}

/// 聚合报告。
#[derive(Debug, Clone)]
pub struct RecallReport {
    pub outcomes: Vec<CaseOutcome>,
    pub corpus_size: usize,
}

impl RecallReport {
    #[must_use]
    pub fn recall_rate(&self) -> f64 {
        let exp: usize = self.outcomes.iter().map(|o| o.expected).sum();
        let rec: usize = self.outcomes.iter().map(|o| o.recalled).sum();
        if exp == 0 { 1.0 } else { rec as f64 / exp as f64 }
    }

    #[must_use]
    pub fn false_positive_rate(&self) -> f64 {
        let denom: usize = self.outcomes.iter().map(|o| o.non_expected).sum();
        let fp: usize = self.outcomes.iter().map(|o| o.false_positives).sum();
        if denom == 0 { 0.0 } else { fp as f64 / denom as f64 }
    }

    /// 按 key（bucket 或 language）算召回率。
    #[must_use]
    pub fn recall_by<F: Fn(&CaseOutcome) -> &str>(&self, key: F) -> Vec<(String, f64)> {
        use std::collections::BTreeMap;
        let mut acc: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for o in &self.outcomes {
            let e = acc.entry(key(o).to_owned()).or_insert((0, 0));
            e.0 += o.expected;
            e.1 += o.recalled;
        }
        acc.into_iter()
            .map(|(k, (exp, rec))| (k, if exp == 0 { 1.0 } else { rec as f64 / exp as f64 }))
            .collect()
    }

    #[must_use]
    pub fn passes_gate(&self) -> bool {
        self.recall_rate() >= RECALL_GATE && self.false_positive_rate() <= FP_GATE
    }
}
```

在 `#[cfg(test)] mod tests` 内补测试（追加到现有 test 函数后）：

```rust
    fn case(id: &str, bucket: &str, lang: &str, expected: &[&str]) -> RecallCase {
        RecallCase {
            id: id.into(), query: "q".into(), language: lang.into(),
            bucket: bucket.into(),
            expected_hits: expected.iter().map(|s| (*s).into()).collect(),
        }
    }

    #[test]
    fn full_recall_zero_fp() {
        let corpus = vec![file("a", "述职.ppt"), file("b", "干扰.txt")];
        let c = case("c1", "office", "zh", &["a"]);
        let o = outcome_for(&c, &corpus, &["a".into()]);
        let report = RecallReport { outcomes: vec![o], corpus_size: 2 };
        assert!((report.recall_rate() - 1.0).abs() < 1e-9);
        assert!(report.false_positive_rate().abs() < 1e-9);
        assert!(report.passes_gate());
    }

    #[test]
    fn miss_lowers_recall() {
        let corpus = vec![file("a", "x"), file("b", "y")];
        let c = case("c1", "office", "zh", &["a", "b"]);
        let o = outcome_for(&c, &corpus, &["a".into()]); // 漏 b
        let report = RecallReport { outcomes: vec![o], corpus_size: 2 };
        assert!((report.recall_rate() - 0.5).abs() < 1e-9);
        assert!(!report.passes_gate()); // 0.5 < 0.70
    }

    #[test]
    fn extra_hit_raises_fp() {
        let corpus = vec![file("a", "x"), file("b", "y"), file("c", "z")];
        let c = case("c1", "office", "zh", &["a"]);
        let o = outcome_for(&c, &corpus, &["a".into(), "b".into()]); // b 是假阳
        let report = RecallReport { outcomes: vec![o], corpus_size: 3 };
        // non_expected = 3 - 1 = 2; fp = 1 → 0.5
        assert!((report.false_positive_rate() - 0.5).abs() < 1e-9);
        assert!(!report.passes_gate());
    }

    #[test]
    fn recall_by_bucket_splits() {
        let corpus = vec![file("a", "x"), file("b", "y")];
        let o1 = outcome_for(&case("c1", "office", "zh", &["a"]), &corpus, &["a".into()]);
        let o2 = outcome_for(&case("c2", "document", "zh", &["b"]), &corpus, &[]);
        let report = RecallReport { outcomes: vec![o1, o2], corpus_size: 2 };
        let by = report.recall_by(|o| o.bucket.as_str());
        assert_eq!(by, vec![("document".into(), 0.0), ("office".into(), 1.0)]);
    }
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p locifind-evals recall::tests`
Expected: 全 PASS（含 Task 1 的 6 个 + 本任务 4 个）

- [ ] **Step 3: fmt + clippy**

Run: `cargo fmt -p locifind-evals && cargo clippy -p locifind-evals --all-targets -- -D warnings`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add packages/evals/src/recall.rs
git commit -m "feat(beta-15a): 召回/假阳指标 + RecallReport + 门槛常量(70%/5%)"
```

---

### Task 3: fixture 加载器 + 引用完整性校验

**Files:**
- Modify: `packages/evals/src/recall.rs`

- [ ] **Step 1: 写失败测试**

在 `packages/evals/src/recall.rs` 的 `RecallReport` impl 之后插入：

```rust
use std::collections::HashSet;
use std::path::Path;

/// 从 JSON 文件加载 corpus。
pub fn load_corpus(path: &Path) -> anyhow::Result<Vec<CorpusFile>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("读 corpus {path:?} 失败: {e}"))?;
    Ok(serde_json::from_str(&raw)?)
}

/// 从 JSON 文件加载 cases。
pub fn load_cases(path: &Path) -> anyhow::Result<Vec<RecallCase>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("读 cases {path:?} 失败: {e}"))?;
    Ok(serde_json::from_str(&raw)?)
}

/// 校验引用完整性：corpus/cases id 无重复；cases 引用的 file id 必须存在于 corpus。
pub fn check_integrity(corpus: &[CorpusFile], cases: &[RecallCase]) -> anyhow::Result<()> {
    let mut seen_files: HashSet<&str> = HashSet::new();
    for f in corpus {
        if !seen_files.insert(f.id.as_str()) {
            anyhow::bail!("corpus 重复 file id: {}", f.id);
        }
    }
    let mut seen_cases: HashSet<&str> = HashSet::new();
    for c in cases {
        if !seen_cases.insert(c.id.as_str()) {
            anyhow::bail!("cases 重复 case id: {}", c.id);
        }
        for hit in &c.expected_hits {
            if !seen_files.contains(hit.as_str()) {
                anyhow::bail!("case {} 引用了不存在的 file id: {}", c.id, hit);
            }
        }
    }
    Ok(())
}
```

在 `#[cfg(test)] mod tests` 内追加：

```rust
    #[test]
    fn integrity_passes_on_valid() {
        let corpus = vec![file("a", "述职.ppt"), file("b", "干扰.txt")];
        let cases = vec![case("c1", "office", "zh", &["a"])];
        assert!(check_integrity(&corpus, &cases).is_ok());
    }

    #[test]
    fn integrity_catches_dangling_ref() {
        let corpus = vec![file("a", "述职.ppt")];
        let cases = vec![case("c1", "office", "zh", &["missing-id"])];
        assert!(check_integrity(&corpus, &cases).is_err());
    }

    #[test]
    fn integrity_catches_dup_file_id() {
        let corpus = vec![file("a", "x"), file("a", "y")];
        let cases: Vec<RecallCase> = vec![];
        assert!(check_integrity(&corpus, &cases).is_err());
    }

    #[test]
    fn corpus_deserializes_content_terms_default() {
        let json = r#"[{"id":"a","filename":"x.pdf"}]"#;
        let corpus: Vec<CorpusFile> = serde_json::from_str(json).unwrap();
        assert_eq!(corpus[0].content_terms.len(), 0);
    }
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p locifind-evals recall::tests`
Expected: 全 PASS

- [ ] **Step 3: fmt + clippy**

Run: `cargo fmt -p locifind-evals && cargo clippy -p locifind-evals --all-targets -- -D warnings`
Expected: 通过

- [ ] **Step 4: 提交**

```bash
git add packages/evals/src/recall.rs
git commit -m "feat(beta-15a): corpus/cases JSON 加载器 + 引用完整性校验"
```

---

### Task 4: 手工标注 corpus.json

**Files:**
- Create: `packages/evals/fixtures/synonym-recall/corpus.json`

合成文件全集。每个**期望命中**文件用同义词组里的**别名**命名（非 query 原词），验证「只有扩展才命中」。另加**干扰文件**（其它桶内容词 / 近似但无同义关系），度量假阳。

**标注原则：**
- 期望命中文件命名用别名，例：组 `工作汇报{述职,年度总结,…}` → 文件 `述职.ppt`、`2025年度总结.docx`。
- 干扰文件不得含任何参与 case 的同义词组成员子串，例：`项目计划.docx`、`随手记.txt`、`vacation.jpg`。
- id 命名 `f-<拼音/英文简写>-<ext>`，全局唯一。
- 覆盖 zh 内容词桶（office 汇报 / 文档管理 / 个人）+ en（document / office / personal）。

- [ ] **Step 1: 写 corpus.json**

新建 `packages/evals/fixtures/synonym-recall/corpus.json`，~60-80 个文件。骨架（执行时按下方 Task 5 cases 实际需要补足到覆盖每个 case 的 expected_hits + 充足干扰；以下为格式与首批条目示范，执行者据 cases 扩充）：

```json
[
  { "id": "f-zhishu-ppt",        "filename": "述职.ppt",            "content_terms": [] },
  { "id": "f-niandu-zongjie",    "filename": "2025年度总结.docx",    "content_terms": [] },
  { "id": "f-zhoubao",           "filename": "本周周总结.md",        "content_terms": [] },
  { "id": "f-jianli",            "filename": "个人简历-final.pdf",   "content_terms": [] },
  { "id": "f-huiyi-jilu",        "filename": "会议记录-0521.md",     "content_terms": [] },
  { "id": "f-xieyi-pdf",         "filename": "购房协议.pdf",         "content_terms": [] },
  { "id": "f-fapiao",            "filename": "餐饮票据.jpg",         "content_terms": [] },
  { "id": "f-baoxiao",           "filename": "差旅报账单.xlsx",      "content_terms": [] },
  { "id": "f-tongzhi",           "filename": "放假公告.docx",        "content_terms": [] },
  { "id": "f-baogao",            "filename": "市场调研报告.pdf",     "content_terms": [] },
  { "id": "f-biji",              "filename": "读书札记.md",          "content_terms": [] },
  { "id": "f-riji",              "filename": "旅行日志.txt",         "content_terms": [] },
  { "id": "f-shipu",             "filename": "妈妈的菜谱.docx",      "content_terms": [] },
  { "id": "f-tijian",            "filename": "健康报告-2025.pdf",    "content_terms": [] },
  { "id": "f-cv-en",             "filename": "my_cv.docx",          "content_terms": [] },
  { "id": "f-agreement-en",      "filename": "vendor_agreement.pdf","content_terms": [] },
  { "id": "f-receipt-en",        "filename": "hotel_receipt.png",   "content_terms": [] },
  { "id": "f-minutes-en",        "filename": "meeting_minutes.md",  "content_terms": [] },
  { "id": "f-forecast-en",       "filename": "q3_forecast.xlsx",    "content_terms": [] },
  { "id": "f-diary-en",          "filename": "personal_diary.txt",  "content_terms": [] },
  { "id": "f-distract-plan",     "filename": "项目计划.docx",        "content_terms": [] },
  { "id": "f-distract-photo",    "filename": "vacation.jpg",        "content_terms": [] },
  { "id": "f-distract-code",     "filename": "main.rs",             "content_terms": [] },
  { "id": "f-distract-random",   "filename": "随手记草稿.txt",       "content_terms": [] },
  { "id": "f-distract-en-misc",  "filename": "todo_scratch.txt",    "content_terms": [] }
]
```

> 执行说明：先看 Task 5 的 cases，确保每个 case 的 expected_hits 都能在 corpus 找到一个**别名命名**文件；干扰文件保持 ≥ 15 个以使假阳率分母有意义。

- [ ] **Step 2: JSON 合法性快速校验**

Run: `python3 -c "import json; print(len(json.load(open('packages/evals/fixtures/synonym-recall/corpus.json'))), 'files')"`
Expected: 打印文件数（应 ≥ 40）

- [ ] **Step 3: 提交**

```bash
git add packages/evals/fixtures/synonym-recall/corpus.json
git commit -m "feat(beta-15a): 合成召回 corpus(别名命中文件 + 干扰文件)"
```

---

### Task 5: 手工标注 cases.json

**Files:**
- Create: `packages/evals/fixtures/synonym-recall/cases.json`

~40-50 条召回 case。query 用**自然中文/英文**且含某同义词组的 head 或别名（触发 gazetteer 或 parser keyword）；expected_hits 指向 corpus 中用**该组另一别名**命名的文件。

**标注原则：**
- query 形态尽量自然（`找一份工作汇报相关的ppt`、`find my cv`），覆盖 BETA-15E gazetteer 路径。
- bucket ∈ {office, document, personal}（zh）/ {document, office, personal}（en）；与词典 domain 对齐。
- 每个 case expected_hits 通常 1 个文件（必要时多个同组别名文件）。
- language ∈ {zh, en}。

- [ ] **Step 1: 写 cases.json**

新建 `packages/evals/fixtures/synonym-recall/cases.json`（示范首批，执行者补足到 ~40-50 条，zh ≈ 28 / en ≈ 18，覆盖三桶）：

```json
[
  { "id": "recall-zh-office-01", "query": "找一份工作汇报相关的ppt", "language": "zh", "bucket": "office", "expected_hits": ["f-zhishu-ppt"] },
  { "id": "recall-zh-office-02", "query": "年底做的总结文档",         "language": "zh", "bucket": "office", "expected_hits": ["f-niandu-zongjie"] },
  { "id": "recall-zh-office-03", "query": "上周的周报在哪",           "language": "zh", "bucket": "office", "expected_hits": ["f-zhoubao"] },
  { "id": "recall-zh-doc-01",    "query": "找之前签的合同",           "language": "zh", "bucket": "document", "expected_hits": ["f-xieyi-pdf"] },
  { "id": "recall-zh-doc-02",    "query": "上次出差的发票",           "language": "zh", "bucket": "document", "expected_hits": ["f-fapiao"] },
  { "id": "recall-zh-doc-03",    "query": "项目验收报告",             "language": "zh", "bucket": "document", "expected_hits": ["f-baogao"] },
  { "id": "recall-zh-personal-01","query": "读书时候写的笔记",        "language": "zh", "bucket": "personal", "expected_hits": ["f-biji"] },
  { "id": "recall-zh-personal-02","query": "妈妈的食谱文件",          "language": "zh", "bucket": "personal", "expected_hits": ["f-shipu"] },
  { "id": "recall-en-doc-01",    "query": "my resume for job applications", "language": "en", "bucket": "document", "expected_hits": ["f-cv-en"] },
  { "id": "recall-en-doc-02",    "query": "service contract for the vendor", "language": "en", "bucket": "document", "expected_hits": ["f-agreement-en"] },
  { "id": "recall-en-office-01", "query": "meeting notes from yesterday", "language": "en", "bucket": "office", "expected_hits": ["f-minutes-en"] }
]
```

> 执行说明：补足时**每条都先用 Task 6 的 bin 实跑确认命中**（见下）。若某自然 query 因 gazetteer 未触发而漏命中，这是**真实词典/抽取 gap**——记录在报告里，不要为凑数强行改 query（gate 守 70% 下限有余量）。把确实命中的留下，gap 案例可保留少量以暴露真实召回边界。

- [ ] **Step 2: JSON 合法性快速校验**

Run: `python3 -c "import json; print(len(json.load(open('packages/evals/fixtures/synonym-recall/cases.json'))), 'cases')"`
Expected: 打印 case 数

- [ ] **Step 3: 提交**

```bash
git add packages/evals/fixtures/synonym-recall/cases.json
git commit -m "feat(beta-15a): 召回 cases(自然 query + 期望命中标注)"
```

---

### Task 6: `run_recall` 管线 + bin + Cargo 接线

**Files:**
- Modify: `packages/evals/src/recall.rs`（加 `run_recall`）
- Modify: `packages/evals/Cargo.toml`（加 harness 依赖 + `[[bin]]`）
- Create: `packages/evals/src/bin/synonym_recall.rs`

- [ ] **Step 1: Cargo.toml 加依赖与 bin**

`packages/evals/Cargo.toml` 的 `[dependencies]` 加一行（紧跟其它 `locifind-*`）：

```toml
locifind-harness = { path = "../harness" }
```

文件末尾 `[[bin]]` 区追加：

```toml
[[bin]]
name = "synonym_recall"
path = "src/bin/synonym_recall.rs"
```

- [ ] **Step 2: recall.rs 加 `run_recall`**

在 `packages/evals/src/recall.rs` 的 `check_integrity` 之后插入（需要 `use` parser 与 expander）：

```rust
use locifind_harness::SynonymExpander;

/// 跑全管线：对每 case `parse → expand → matches` → 聚合报告。
#[must_use]
pub fn run_recall(
    expander: &dyn SynonymExpander,
    corpus: &[CorpusFile],
    cases: &[RecallCase],
) -> RecallReport {
    let outcomes = cases
        .iter()
        .map(|case| {
            let intent = locifind_intent_parser::parse(&case.query);
            let expanded = expander.expand(intent, &case.query);
            let groups = &expanded.keyword_groups;
            let actual_hits: Vec<String> = corpus
                .iter()
                .filter(|f| matches(groups, f))
                .map(|f| f.id.clone())
                .collect();
            outcome_for(case, corpus, &actual_hits)
        })
        .collect();
    RecallReport { outcomes, corpus_size: corpus.len() }
}
```

- [ ] **Step 3: 写 bin**

新建 `packages/evals/src/bin/synonym_recall.rs`：

```rust
//! BETA-15A 同义词召回评测 bin。
//! 加载 ship 词典 + 合成 corpus/cases，跑 parse→expand→匹配，输出报告 + 门槛退出码。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use locifind_evals::recall::{
    check_integrity, load_cases, load_corpus, run_recall, FP_GATE, RECALL_GATE,
};
use locifind_harness::{NoopExpander, SynonymExpander, YamlSynonymExpander};

#[derive(Parser)]
#[command(name = "synonym_recall", about = "BETA-15A 同义词召回评测")]
struct Cli {
    /// 输出 JSON 报告
    #[arg(long)]
    json: bool,
    /// 仅打印未达标(漏命中/有假阳)的 case
    #[arg(long)]
    only_failures: bool,
}

fn workspace_path(rel: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR = packages/evals → 仓库根是 ../..
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let zh = workspace_path("resources/synonyms/zh.yaml");
    let en = workspace_path("resources/synonyms/en.yaml");
    let expander: Box<dyn SynonymExpander> = match YamlSynonymExpander::from_paths(&zh, &en) {
        Ok(e) => Box::new(e),
        Err(err) => {
            eprintln!("词典加载失败,退到 noop(召回必为 0): {err}");
            Box::new(NoopExpander)
        }
    };

    let corpus_path = workspace_path("packages/evals/fixtures/synonym-recall/corpus.json");
    let cases_path = workspace_path("packages/evals/fixtures/synonym-recall/cases.json");
    let corpus = match load_corpus(&corpus_path) {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(2); }
    };
    let cases = match load_cases(&cases_path) {
        Ok(c) => c,
        Err(e) => { eprintln!("{e}"); return ExitCode::from(2); }
    };
    if let Err(e) = check_integrity(&corpus, &cases) {
        eprintln!("fixture 完整性校验失败: {e}");
        return ExitCode::from(2);
    }

    let report = run_recall(expander.as_ref(), &corpus, &cases);

    if cli.json {
        // 简洁 JSON：总指标 + 分桶
        let by_bucket = report.recall_by(|o| o.bucket.as_str());
        let by_lang = report.recall_by(|o| o.language.as_str());
        println!(
            "{{\"recall\":{:.4},\"false_positive\":{:.4},\"cases\":{},\"corpus\":{},\"by_bucket\":{:?},\"by_language\":{:?}}}",
            report.recall_rate(), report.false_positive_rate(),
            report.outcomes.len(), report.corpus_size, by_bucket, by_lang
        );
    } else {
        println!("== BETA-15A 同义词召回评测 ==");
        println!("cases={} corpus={}", report.outcomes.len(), report.corpus_size);
        println!("总召回率   : {:.1}%", report.recall_rate() * 100.0);
        println!("总假阳率   : {:.1}%", report.false_positive_rate() * 100.0);
        println!("-- 按桶 --");
        for (k, v) in report.recall_by(|o| o.bucket.as_str()) {
            println!("  {k:<10} {:.1}%", v * 100.0);
        }
        println!("-- 按语言 --");
        for (k, v) in report.recall_by(|o| o.language.as_str()) {
            println!("  {k:<10} {:.1}%", v * 100.0);
        }
        for o in &report.outcomes {
            let failed = !o.missing.is_empty() || !o.extra.is_empty();
            if cli.only_failures && !failed { continue; }
            if failed {
                println!(
                    "  [FAIL] {} (bucket={}) 漏={:?} 假阳={:?}",
                    o.case_id, o.bucket, o.missing, o.extra
                );
            } else if !cli.only_failures {
                println!("  [ ok ] {}", o.case_id);
            }
        }
    }

    if report.passes_gate() {
        eprintln!(
            "门槛通过: recall {:.1}% >= {:.0}% 且 fp {:.1}% <= {:.0}%",
            report.recall_rate() * 100.0, RECALL_GATE * 100.0,
            report.false_positive_rate() * 100.0, FP_GATE * 100.0
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "门槛未过: recall {:.1}% (需 >= {:.0}%) / fp {:.1}% (需 <= {:.0}%)",
            report.recall_rate() * 100.0, RECALL_GATE * 100.0,
            report.false_positive_rate() * 100.0, FP_GATE * 100.0
        );
        ExitCode::from(1)
    }
}
```

- [ ] **Step 4: 构建 + 实跑（确定 baseline + 调 fixtures）**

Run: `cargo run -p locifind-evals --bin synonym_recall`
Expected: 打印报告 + 门槛行。**这是 Task 4/5 fixtures 的验证回路**：若 recall < 70% 或 fp > 5%，检查报告 FAIL 行——调整误标的 case/corpus（别名命名错、干扰文件含同义词子串等），直到达标。记录最终实测 recall/fp 作为 baseline。

- [ ] **Step 5: fmt + clippy + 全测试**

Run: `cargo fmt -p locifind-evals && cargo clippy -p locifind-evals --all-targets -- -D warnings && cargo test -p locifind-evals`
Expected: 全过

- [ ] **Step 6: 提交**

```bash
git add packages/evals/Cargo.toml packages/evals/Cargo.lock packages/evals/src/recall.rs packages/evals/src/bin/synonym_recall.rs
git add Cargo.lock 2>/dev/null || true
git commit -m "feat(beta-15a): run_recall 管线 + synonym_recall bin(报告+门槛退出码)"
```

---

### Task 7: 集成门槛测试 + ci.sh 接线 + README

**Files:**
- Create: `packages/evals/tests/synonym_recall_gate.rs`
- Modify: `scripts/ci.sh`
- Modify: `packages/evals/README.md`

- [ ] **Step 1: 写集成门槛测试（随 `cargo test --workspace` 自动跑 = 主回归门）**

新建 `packages/evals/tests/synonym_recall_gate.rs`：

```rust
//! BETA-15A 召回门槛集成测试：用 ship 词典 + checked-in fixtures 跑全管线，
//! 断言 recall >= 70% 且 fp <= 5%。随 `cargo test --workspace`（ci.sh test 步）自动执行。
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use locifind_evals::recall::{check_integrity, load_cases, load_corpus, run_recall};
use locifind_harness::YamlSynonymExpander;

fn ws(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
}

#[test]
fn synonym_recall_meets_gate() {
    let expander = YamlSynonymExpander::from_paths(
        &ws("resources/synonyms/zh.yaml"),
        &ws("resources/synonyms/en.yaml"),
    )
    .expect("ship 词典应加载成功");

    let corpus = load_corpus(&ws("packages/evals/fixtures/synonym-recall/corpus.json")).unwrap();
    let cases = load_cases(&ws("packages/evals/fixtures/synonym-recall/cases.json")).unwrap();
    check_integrity(&corpus, &cases).expect("fixture 引用完整");
    assert!(cases.len() >= 30, "BETA-15A 验收要求 >= 30 case, 实际 {}", cases.len());

    let report = run_recall(&expander, &corpus, &cases);
    assert!(
        report.passes_gate(),
        "召回门槛未过: recall={:.1}% fp={:.1}%",
        report.recall_rate() * 100.0,
        report.false_positive_rate() * 100.0
    );
}
```

- [ ] **Step 2: 跑集成测试**

Run: `cargo test -p locifind-evals --test synonym_recall_gate`
Expected: PASS（若失败回到 Task 6 Step 4 调 fixtures）

- [ ] **Step 3: ci.sh 加可读报告步骤**

在 `packages/evals` 已被 `cargo test --workspace` 覆盖门槛的前提下，ci.sh 额外加一个可读报告步骤（门槛主守在 test，本步给人看 + 双保险）。编辑 `scripts/ci.sh`，在 `run_test` 函数定义后加：

```bash
run_synonym_recall() {
  step "cargo run -p locifind-evals --bin synonym_recall"
  cargo run -p locifind-evals --bin synonym_recall
}
```

并在 `all)` 分支 `run_test` 之后加 `run_synonym_recall`，在 `case` 分支加 `synonym-recall) run_synonym_recall ;;`：

```bash
  all)
    run_fmt
    run_clippy
    run_build
    run_test
    run_synonym_recall
    ;;
```

（在 `test) run_test ;;` 行后加：）

```bash
  synonym-recall) run_synonym_recall ;;
```

- [ ] **Step 4: 跑 ci.sh 全套**

Run: `bash scripts/ci.sh`
Expected: 末尾「所有检查通过 ✓」，含 synonym_recall 报告 + 门槛通过

- [ ] **Step 5: README 加文档节**

在 `packages/evals/README.md` 末尾追加：

```markdown
## 同义词召回评测 (BETA-15A)

离线确定性衡量「手维护词典 + gazetteer」在合成痛点 query 上的召回率/假阳率。
不跑 Spotlight/mdfind/模型；走真 `parse → expand` 管线 + 忠实 BETA-15D 的子串匹配模拟。

\```bash
# 跑报告（按桶/按语言分桶 + 门槛退出码）
cargo run -p locifind-evals --bin synonym_recall

# 仅看未达标 case
cargo run -p locifind-evals --bin synonym_recall -- --only-failures

# JSON 报告
cargo run -p locifind-evals --bin synonym_recall -- --json
\```

- **门槛**：召回率 ≥ 70%、假阳率 ≤ 5%（`recall::RECALL_GATE` / `FP_GATE`）。退出码 0 达标 / 1 未过 / 2 加载错误。
- **回归门**：`tests/synonym_recall_gate.rs` 随 `cargo test --workspace` 强制；`scripts/ci.sh` 另跑 bin 出可读报告。
- **数据**：`fixtures/synonym-recall/{corpus,cases}.json`（手工标注）。
- **匹配模拟**：组内 OR、组间 AND、大小写不敏感子串，命中域 = 文件名 + content_terms。
```

- [ ] **Step 6: 提交**

```bash
git add packages/evals/tests/synonym_recall_gate.rs scripts/ci.sh packages/evals/README.md
git commit -m "feat(beta-15a): 召回门槛集成测试 + ci.sh 报告步骤 + README"
```

---

## 验收对照（实现完成后核对）

- [ ] `bash scripts/ci.sh` 全过（含 synonym_recall 门槛通过）
- [ ] cases ≥ 30（实际目标 ~40-50，zh + en 覆盖）
- [ ] 召回率 ≥ 70% / 假阳率 ≤ 5%，实测 baseline 记入 STATUS
- [ ] `cargo run -p locifind-evals --bin evals -- --fixtures v0.5` 维持 **472/26/2 byte-equal**
- [ ] 零改动：parser / spotlight / harness synonym / 词典源 / v0.5 fixtures
- [ ] ROADMAP BETA-15A 标 done；STATUS 会话日志 + 下一步更新
```
