# BETA-15D Spotlight macOS 26 谓词修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 macOS 26 NSPredicate parser 拒绝「字符串匹配 + 比较操作符」混合复合谓词的回归，让 keyword + 扩展名/时间/大小 复合搜索在真机可用，并顺带修 CJK 文件名匹配。

**Architecture:** Spotlight backend 把每次含 keyword 的搜索拆成两条单操作符类别子查询：Q1（纯 comparison：文件名 glob 关键词 + 扩展名/时间/大小）与 Q2（纯 string：内容/媒体字段 CONTAINS）。并发执行后在 Rust 端合并去重，Q2 结果按 Q1 同款约束后置过滤。无 keyword 的查询维持单条。

**Tech Stack:** Rust，`packages/search-backends/spotlight`（单 crate 单文件 `src/lib.rs`），chrono（已是依赖），`std::thread`（并发两条 mdfind），`mdfind` 子进程。

**Spec:** [docs/superpowers/specs/2026-05-30-beta-15d-spotlight-macos26-predicate-design.md](../specs/2026-05-30-beta-15d-spotlight-macos26-predicate-design.md)

**全程不变量（每个 task 验证门都要守）：**
- `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings` + `cargo test`（即 `bash scripts/ci.sh` 子集；按 [[feedback_per_task_verify_include_fmt]] **每 task 必跑 fmt**）。
- 只动 `packages/search-backends/spotlight/src/lib.rs`，不碰 parser/common/desktop/evals 源。
- evals v0.5 parser-only 维持 472/26/2（不依赖 spotlight，最终 task 实跑确认即可）。

---

## File Structure

唯一改动文件：`packages/search-backends/spotlight/src/lib.rs`（现 1110 行）。本计划在其中新增/重构：

- `escape_glob_pattern`（新）— glob 元字符转义。
- `PostFilter` + `ExtensionFilter`/`TimeFilter`/`SizeFilter`（新）— Q2 结果的 Rust 端约束判定，附 `matches(&SearchResult)`。
- `TranslatedQuery`（新）— 替代单 `SpotlightQuery`：含 `q1: SpotlightQuery`、`q2: Option<SpotlightQuery>`、`post_filters: Vec<PostFilter>`。
- `QueryBuilder` 重构 — 三桶：`cmp_predicates`（Q1）/`str_predicates`（Q2）/`post_filters`，新增 `and_cmp`/`and_str`/`and_post_filter`，`finish` 产 `TranslatedQuery`。
- 谓词构建函数拆分：`name_glob_predicate`/`name_glob_predicate_expanded`（Q1）、`content_predicate`/`content_predicate_expanded`（Q2）。
- `translate_*` 4 函数 + `add_common_file_constraints` 路由到三桶。
- `search`/`search_expanded` 执行层：并发跑 Q1/Q2 → 合并去重 → 后置过滤 → sort → truncate。
- `run_mdfind`：检测 stdout `Failed to create query` sentinel。

---

## Task 1: glob 元字符转义

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（`escape_predicate_string` 附近，约 714 行）
- Test: 同文件 `#[cfg(test)]` 模块

`== "*kw*"cd` glob 把 keyword 里的 `*`/`?`/`\` 当通配。需把它们转义为字面量，避免 keyword `report*` 误展开。Spotlight glob 转义用反斜杠（`\*` `\?`）。

- [ ] **Step 1: Write the failing test**

加到测试模块：

```rust
#[test]
fn escape_glob_pattern_escapes_metacharacters() {
    assert_eq!(escape_glob_pattern("a*b?c"), "a\\*b\\?c");
    assert_eq!(escape_glob_pattern("plain"), "plain");
    // 反斜杠先转义，避免与通配转义叠加产生歧义
    assert_eq!(escape_glob_pattern("a\\b"), "a\\\\b");
    // 双引号仍按谓词字面量转义（复用 escape_predicate_string 不丢）
    assert_eq!(escape_glob_pattern("a\"b"), "a\\\"b");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p locifind-search-spotlight escape_glob_pattern -- --nocapture`
Expected: 编译失败 `cannot find function escape_glob_pattern`

- [ ] **Step 3: Write minimal implementation**

在 `escape_predicate_string` 下方新增（注意先转义 `\`，再转义引号与通配，顺序保证不重复转义）：

```rust
/// 转义 Spotlight glob 谓词字面量：在 `escape_predicate_string`（反斜杠 + 引号）基础上，
/// 额外把 glob 通配符 `*` `?` 转义为字面量，避免 keyword 被当通配展开。
#[must_use]
pub fn escape_glob_pattern(value: &str) -> String {
    escape_predicate_string(value)
        .replace('*', "\\*")
        .replace('?', "\\?")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p locifind-search-spotlight escape_glob_pattern`
Expected: PASS

- [ ] **Step 5: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`
Expected: 全 PASS

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 新增 escape_glob_pattern 转义 glob 元字符(BETA-15D Task 1)"
```

---

## Task 2: PostFilter — Q2 结果的 Rust 端约束判定（最高风险点）

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（新增类型 + impl，建议放 `time_predicate`/`size_predicate_with_field` 附近）
- Test: 同文件测试模块

Q2（内容/字段 CONTAINS）不能携带比较约束，故 Q2 命中的 `SearchResult` 须在 Rust 端按扩展名/时间/大小过滤。本 task 单列并重点 TDD，锁定「Rust 判定 == 谓词语义」。复用 `relative_time_bounds` 与 `unit_value` 的等价逻辑，禁止两处手写区间。

时间锚点：mdfind `$time.today(N)` = 本地时区今天午夜 + N 天。Rust 用 `chrono::Local` 今天午夜 + 偏移，转 `DateTime<Utc>` 与文件时间比较。Absolute/Before/After 用 ISO 日期。

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn extension_filter_matches_case_insensitive_and_negation() {
    let f = ExtensionFilter { extensions: vec!["pdf".into(), "ppt".into()], negate: false };
    assert!(f.matches(Path::new("/x/A.PDF")));
    assert!(f.matches(Path::new("/x/b.ppt")));
    assert!(!f.matches(Path::new("/x/c.txt")));
    let neg = ExtensionFilter { extensions: vec!["tmp".into()], negate: true };
    assert!(neg.matches(Path::new("/x/a.pdf")));
    assert!(!neg.matches(Path::new("/x/a.tmp")));
}

#[test]
fn size_filter_matches_bytes_domain_bounds() {
    use locifind_common::{SizeExpression, SizeUnit};
    // > 1 MB
    let gt = SizeFilter::from_expression(
        &SizeExpression::GreaterThan { value: 1.0, unit: SizeUnit::Mb }, UnitDomain::Bytes,
    ).unwrap();
    assert!(gt.matches(2_000_000));
    assert!(!gt.matches(500_000));
    // between 1KB and 2KB inclusive
    let bt = SizeFilter::from_expression(
        &SizeExpression::Between { min: 1.0, max: 2.0, unit: SizeUnit::Kb }, UnitDomain::Bytes,
    ).unwrap();
    assert!(bt.matches(1_000));
    assert!(bt.matches(2_000));
    assert!(!bt.matches(2_001));
    assert!(!bt.matches(999));
}

#[test]
fn time_filter_before_after_absolute_bounds() {
    use chrono::{TimeZone, Utc};
    use locifind_common::TimeExpression;
    let before = TimeFilter::from_expression(
        &TimeExpression::Before { value: "2026-01-10".into() }).unwrap();
    let t_jan5 = Utc.with_ymd_and_hms(2026, 1, 5, 12, 0, 0).unwrap();
    let t_jan20 = Utc.with_ymd_and_hms(2026, 1, 20, 12, 0, 0).unwrap();
    assert!(before.matches(Some(t_jan5)));
    assert!(!before.matches(Some(t_jan20)));
    // 文件无该时间字段 → 不匹配
    assert!(!before.matches(None));

    let between = TimeFilter::from_expression(
        &TimeExpression::Absolute { from: "2026-01-01".into(), to: "2026-01-31".into() }).unwrap();
    assert!(between.matches(Some(t_jan20)));
    assert!(!between.matches(Some(Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap())));
}

#[test]
fn post_filter_combines_all_constraints_against_result() {
    use chrono::{TimeZone, Utc};
    // 构造 PostFilter：扩展名 pdf + 大小 > 1KB + modified before 2026-01-10
    let pf = PostFilter {
        extension: Some(ExtensionFilter { extensions: vec!["pdf".into()], negate: false }),
        size: Some(SizeFilter::from_expression(
            &locifind_common::SizeExpression::GreaterThan { value: 1.0, unit: locifind_common::SizeUnit::Kb },
            UnitDomain::Bytes).unwrap()),
        time: vec![TimeField::Modified(TimeFilter::from_expression(
            &locifind_common::TimeExpression::Before { value: "2026-01-10".into() }).unwrap())],
    };
    let mut r = sample_result("/x/doc.pdf", 5_000,
        Some(Utc.with_ymd_and_hms(2026, 1, 5, 0, 0, 0).unwrap()));
    assert!(pf.matches(&r));
    // 扩展名不符 → 整体不匹配
    r.path = Path::new("/x/doc.txt").to_path_buf();
    assert!(!pf.matches(&r));
}
```

`sample_result` 测试 helper（放测试模块）：

```rust
fn sample_result(path: &str, size: u64, modified: Option<chrono::DateTime<chrono::Utc>>) -> SearchResult {
    SearchResult {
        id: "t".into(),
        path: Path::new(path).to_path_buf(),
        name: Path::new(path).file_name().unwrap().to_string_lossy().into_owned(),
        source: BackendKind::Spotlight,
        match_type: MatchType::Filename,
        score: None,
        metadata: SearchResultMetadata {
            modified_time: modified,
            created_time: None,
            accessed_time: None,
            size_bytes: Some(size),
            ..SearchResultMetadata::default()
        },
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p locifind-search-spotlight post_filter -- --nocapture`（连带 extension_filter/size_filter/time_filter）
Expected: 编译失败 `cannot find type PostFilter/ExtensionFilter/...`

- [ ] **Step 3: Write minimal implementation**

新增类型与 impl（放在 `size_predicate_with_field` 之后）：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtensionFilter {
    extensions: Vec<String>, // 不含前导点
    negate: bool,
}

impl ExtensionFilter {
    fn matches(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let hit = ext
            .as_deref()
            .is_some_and(|e| self.extensions.iter().any(|want| want.eq_ignore_ascii_case(e)));
        if self.negate { !hit } else { hit }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SizeFilter {
    min: Option<f64>, // 含下界
    max: Option<f64>, // 含上界
}

impl SizeFilter {
    fn from_expression(size: &SizeExpression, domain: UnitDomain) -> Result<Self, SearchError> {
        // 复用 unit_value 的归一化（保证与谓词同源），unit_value 产字符串，这里改用其数值
        let to_bytes = |v: f64, u: SizeUnit| -> Result<f64, SearchError> {
            // unit_value 已含校验 + 取整；postfilter 用未取整数值比较更精确
            unit_value(v, u, domain).map(|s| s.parse::<f64>().unwrap_or(0.0))
        };
        Ok(match size {
            // 谓词用 `>`：严格大于；这里 min 设为 value + epsilon 用 `>` 语义，用 None/Some 区分边界
            SizeExpression::GreaterThan { value, unit } => SizeFilter {
                min: Some(to_bytes(*value, *unit)?),
                max: None,
            },
            SizeExpression::LessThan { value, unit } => SizeFilter {
                min: None,
                max: Some(to_bytes(*value, *unit)?),
            },
            SizeExpression::Between { min, max, unit } => SizeFilter {
                min: Some(to_bytes(*min, *unit)?),
                max: Some(to_bytes(*max, *unit)?),
            },
        })
    }

    fn matches(&self, size_bytes: u64) -> bool {
        let v = size_bytes as f64;
        // GreaterThan/LessThan 谓词是严格 `>` / `<`；Between 是 `>= && <=`。
        // 用标记区分：简化起见 min/max 一律含界，严格性在 from_expression 已无法表达。
        // 为与谓词严格语义一致，改在结构体保留严格标记。见下方修正版。
        self.min.is_none_or(|m| v >= m) && self.max.is_none_or(|m| v <= m)
    }
}
```

> 注：上面 `matches` 对 GreaterThan/LessThan 用了含界 `>=`/`<=`，与谓词的严格 `>`/`<` 略有边界差。为消除分歧，`SizeFilter` 增加 `min_inclusive`/`max_inclusive` 标记：

```rust
#[derive(Debug, Clone, PartialEq)]
struct SizeFilter {
    min: Option<f64>,
    max: Option<f64>,
    min_inclusive: bool,
    max_inclusive: bool,
}

impl SizeFilter {
    fn from_expression(size: &SizeExpression, domain: UnitDomain) -> Result<Self, SearchError> {
        let to_bytes = |v: f64, u: SizeUnit| unit_value(v, u, domain).map(|s| s.parse::<f64>().unwrap_or(0.0));
        Ok(match size {
            SizeExpression::GreaterThan { value, unit } =>
                SizeFilter { min: Some(to_bytes(*value, *unit)?), max: None, min_inclusive: false, max_inclusive: false },
            SizeExpression::LessThan { value, unit } =>
                SizeFilter { min: None, max: Some(to_bytes(*value, *unit)?), min_inclusive: false, max_inclusive: false },
            SizeExpression::Between { min, max, unit } =>
                SizeFilter { min: Some(to_bytes(*min, *unit)?), max: Some(to_bytes(*max, *unit)?), min_inclusive: true, max_inclusive: true },
        })
    }
    fn matches(&self, size_bytes: u64) -> bool {
        let v = size_bytes as f64;
        let lo = self.min.is_none_or(|m| if self.min_inclusive { v >= m } else { v > m });
        let hi = self.max.is_none_or(|m| if self.max_inclusive { v <= m } else { v < m });
        lo && hi
    }
}
```

时间过滤：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct TimeFilter {
    from: Option<chrono::DateTime<chrono::Utc>>, // 含下界
    to: Option<chrono::DateTime<chrono::Utc>>,   // 不含上界（与谓词 `< to` 对齐）
}

impl TimeFilter {
    fn from_expression(time: &TimeExpression) -> Result<Self, SearchError> {
        use chrono::{Duration, Local, NaiveTime, TimeZone, Utc};
        let parse_day = |s: &str, end_of_day: bool| -> Result<chrono::DateTime<Utc>, SearchError> {
            let date = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| {
                SearchError::InvalidIntent { detail: format!("invalid date {s}: {e}") }
            })?;
            let t = if end_of_day { NaiveTime::from_hms_opt(23, 59, 59).unwrap() } else { NaiveTime::MIN };
            Ok(Utc.from_utc_datetime(&date.and_time(t)))
        };
        Ok(match time {
            TimeExpression::Relative { value } => {
                let (from_days, to_days) = relative_time_bounds(*value);
                // 今天本地午夜
                let midnight = Local::now().date_naive().and_time(NaiveTime::MIN);
                let to_utc = |days: i32| Utc.from_utc_datetime(&(midnight + Duration::days(i64::from(days))));
                TimeFilter { from: Some(to_utc(from_days)), to: Some(to_utc(to_days)) }
            }
            TimeExpression::Absolute { from, to } =>
                TimeFilter { from: Some(parse_day(from, false)?), to: Some(parse_day(to, true)?) },
            TimeExpression::Before { value } =>
                TimeFilter { from: None, to: Some(parse_day(value, false)?) },
            TimeExpression::After { value } =>
                TimeFilter { from: Some(parse_day(value, true)?), to: None },
        })
    }
    fn matches(&self, t: Option<chrono::DateTime<chrono::Utc>>) -> bool {
        let Some(t) = t else { return false };
        self.from.is_none_or(|f| t >= f) && self.to.is_none_or(|to| t <= to)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TimeField {
    Modified(TimeFilter),
    Created(TimeFilter),
    Accessed(TimeFilter),
}

#[derive(Debug, Clone, PartialEq, Default)]
struct PostFilter {
    extension: Option<ExtensionFilter>,
    size: Option<SizeFilter>,
    time: Vec<TimeField>,
}

impl PostFilter {
    fn is_empty(&self) -> bool {
        self.extension.is_none() && self.size.is_none() && self.time.is_empty()
    }
    fn matches(&self, result: &SearchResult) -> bool {
        if let Some(ext) = &self.extension {
            if !ext.matches(&result.path) { return false; }
        }
        if let Some(size) = &self.size {
            match result.metadata.size_bytes {
                Some(b) if size.matches(b) => {}
                _ => return false,
            }
        }
        for tf in &self.time {
            let ok = match tf {
                TimeField::Modified(f) => f.matches(result.metadata.modified_time),
                TimeField::Created(f) => f.matches(result.metadata.created_time),
                TimeField::Accessed(f) => f.matches(result.metadata.accessed_time),
            };
            if !ok { return false; }
        }
        true
    }
}
```

> 注：测试里 `pf.time` 用 `vec![TimeField::Modified(...)]`，与上面定义一致。确保 `use locifind_common::{SizeExpression, SizeUnit, TimeExpression};` 已在文件顶部导入（部分已存在，按编译错误补齐）。

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p locifind-search-spotlight -- extension_filter size_filter time_filter post_filter`
Expected: PASS（4 个测试全过）

- [ ] **Step 5: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 新增 PostFilter(扩展名/大小/时间) Rust 端约束判定 + 一致性测试(BETA-15D Task 2)"
```

---

## Task 3: TranslatedQuery + QueryBuilder 三桶重构

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（`SpotlightQuery` 143-153、`QueryBuilder` 521-559）
- Test: 同文件测试模块

`QueryBuilder` 从「单 predicates 列表」改为三桶：`cmp_predicates`（Q1）/`str_predicates`（Q2）/`post_filters`。`finish` 产 `TranslatedQuery`。保留 `SpotlightQuery` 结构（Q1/Q2 各是一个 `SpotlightQuery`）。

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn query_builder_splits_into_q1_q2_and_postfilters() {
    let mut b = QueryBuilder::new(Some(10));
    b.and_cmp("(kMDItemFSName == \"*述职*\"cd)".into());
    b.and_str("kMDItemTextContent CONTAINS[cd] \"述职\"".into());
    b.and_cmp("(kMDItemFSName == \"*.ppt\"cd)".into());
    b.and_post_filter(PostFilter {
        extension: Some(ExtensionFilter { extensions: vec!["ppt".into()], negate: false }),
        ..PostFilter::default()
    });
    let t = b.finish();
    assert_eq!(
        t.q1.predicate,
        "(kMDItemFSName == \"*述职*\"cd) && (kMDItemFSName == \"*.ppt\"cd)"
    );
    let q2 = t.q2.expect("有 str 谓词应产 Q2");
    assert_eq!(q2.predicate, "kMDItemTextContent CONTAINS[cd] \"述职\"");
    assert_eq!(t.post_filters.len(), 1);
    assert_eq!(t.q1.limit, 10);
}

#[test]
fn query_builder_no_str_predicates_yields_no_q2() {
    let mut b = QueryBuilder::new(None);
    b.and_cmp("(kMDItemFSName == \"*.pdf\"cd)".into());
    let t = b.finish();
    assert!(t.q2.is_none());
    assert_eq!(t.q1.predicate, "(kMDItemFSName == \"*.pdf\"cd)");
}

#[test]
fn query_builder_empty_yields_match_all_q1() {
    let t = QueryBuilder::new(None).finish();
    assert_eq!(t.q1.predicate, "kMDItemFSName == \"*\"");
    assert!(t.q2.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p locifind-search-spotlight query_builder`
Expected: 编译失败（`and_cmp`/`TranslatedQuery`/`q1` 不存在）

- [ ] **Step 3: Write minimal implementation**

新增 `TranslatedQuery`（放 `SpotlightQuery` 下方）：

```rust
/// 一次搜索翻译产物：Q1（纯 comparison 复合）+ 可选 Q2（纯 string 复合）+ Q2 后置过滤。
/// only_in / exclude_paths / limit 两条查询共享（Q1 持有，Q2 复制）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedQuery {
    pub q1: SpotlightQuery,
    pub q2: Option<SpotlightQuery>,
    post_filters: Vec<PostFilter>,
}
```

> `post_filters` 私有（仅 crate 内执行层用）。`PostFilter` 需 `Eq`？它含 `f64`（SizeFilter）不能 `Eq`。改：`TranslatedQuery` 只 derive `Debug, Clone, PartialEq`（去 `Eq`）；`SpotlightQuery` 保持 `Eq` 不变。

重构 `QueryBuilder`：

```rust
#[derive(Debug)]
struct QueryBuilder {
    cmp_predicates: Vec<String>,
    str_predicates: Vec<String>,
    post_filters: Vec<PostFilter>,
    only_in: Vec<PathBuf>,
    exclude_paths: Vec<PathBuf>,
    limit: usize,
}

impl QueryBuilder {
    fn new(limit: Option<u32>) -> Self {
        let limit = limit
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(DEFAULT_LIMIT)
            .min(MAX_LIMIT);
        Self {
            cmp_predicates: Vec::new(),
            str_predicates: Vec::new(),
            post_filters: Vec::new(),
            only_in: Vec::new(),
            exclude_paths: Vec::new(),
            limit,
        }
    }

    /// comparison 类谓词 → Q1。
    fn and_cmp(&mut self, predicate: String) {
        self.cmp_predicates.push(predicate);
    }
    /// string 类谓词 → Q2。
    fn and_str(&mut self, predicate: String) {
        self.str_predicates.push(predicate);
    }
    /// Q2 结果的 Rust 端约束（与 Q1 比较约束等价）。
    fn and_post_filter(&mut self, filter: PostFilter) {
        if !filter.is_empty() {
            self.post_filters.push(filter);
        }
    }

    fn finish(self) -> TranslatedQuery {
        let q1_predicate = if self.cmp_predicates.is_empty() {
            "kMDItemFSName == \"*\"".to_owned()
        } else {
            self.cmp_predicates.join(" && ")
        };
        let q1 = SpotlightQuery {
            predicate: q1_predicate,
            only_in: self.only_in.clone(),
            exclude_paths: self.exclude_paths.clone(),
            limit: self.limit,
        };
        let q2 = if self.str_predicates.is_empty() {
            None
        } else {
            Some(SpotlightQuery {
                predicate: self.str_predicates.join(" && "),
                only_in: self.only_in,
                exclude_paths: self.exclude_paths,
                limit: self.limit,
            })
        };
        TranslatedQuery { q1, q2, post_filters: self.post_filters }
    }
}
```

> 编译会因 `translate_*` 仍调旧 `builder.and(...)` 且返回 `SpotlightQuery` 而失败 —— 这些在 Task 4/5 修。本 task 为让测试编译，**临时**保留一个 `fn and(&mut self, p: String) { self.and_cmp(p) }` 兼容垫片，并把 `translate_*` 函数签名暂保持返回 `SpotlightQuery`（用 `builder.finish().q1`）以隔离改动。Task 4/5 移除垫片。

具体：给 `QueryBuilder` 加临时垫片 `fn and(&mut self, p: String) { self.cmp_predicates.push(p); }`；把 4 个 `translate_*` 与 `add_common_file_constraints` 暂时编译通过的最小改法：它们末尾 `Ok(builder.finish())` 改 `Ok(builder.finish().q1)`（保持对外仍返回 `SpotlightQuery`，行为暂等价于"全塞 Q1"——此为中间态，Task 6 才切到 TranslatedQuery）。

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p locifind-search-spotlight query_builder`
Expected: PASS

- [ ] **Step 5: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`
Expected: 全 PASS（既有谓词形态测试因垫片把 cmp 全塞 Q1，行为暂不变 → 不破）

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "refactor(spotlight): QueryBuilder 三桶 + TranslatedQuery(垫片保旧行为, BETA-15D Task 3)"
```

---

## Task 4: 文件搜索谓词拆分 + 路由（name glob → Q1，content → Q2）

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（`keyword_predicate*` 561-584、`add_common_file_constraints` 435-502、`translate_file_search(_expanded)`）
- Test: 同文件测试模块

把 keyword 匹配拆成 `name_glob_predicate`（FSName+DisplayName glob，Q1）与 `content_predicate`（TextContent CONTAINS，Q2），扩展名/时间/大小 同时产谓词（Q1）+ PostFilter。

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn name_glob_predicate_globs_fsname_and_displayname() {
    assert_eq!(
        name_glob_predicate("述职"),
        "(kMDItemFSName == \"*述职*\"cd || kMDItemDisplayName == \"*述职*\"cd)"
    );
    // glob 元字符转义
    assert_eq!(
        name_glob_predicate("a*b"),
        "(kMDItemFSName == \"*a\\*b*\"cd || kMDItemDisplayName == \"*a\\*b*\"cd)"
    );
}

#[test]
fn content_predicate_uses_contains() {
    assert_eq!(content_predicate("述职"), "kMDItemTextContent CONTAINS[cd] \"述职\"");
}

#[test]
fn file_search_keyword_plus_extension_splits_q1_q2_with_postfilter() {
    // intent 一律用 serde_json 构造（FileSearch/MediaSearch 无 Default，且有必填 schema_version）。
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0",
        "intent": "file_search",
        "keywords": ["述职"],
        "extensions": ["ppt"]
    })).unwrap();
    let t = translate_intent(&intent, &resolver()).unwrap();
    // Q1: 文件名 glob && 扩展名 glob（纯 cmp）
    assert_eq!(
        t.q1.predicate,
        "(kMDItemFSName == \"*述职*\"cd || kMDItemDisplayName == \"*述职*\"cd) && (kMDItemFSName == \"*.ppt\"cd)"
    );
    // Q2: 内容 CONTAINS（纯 str）
    assert_eq!(t.q2.unwrap().predicate, "kMDItemTextContent CONTAINS[cd] \"述职\"");
    // PostFilter: 扩展名 ppt
    assert_eq!(t.post_filters.len(), 1);
}

#[test]
fn file_search_pure_extension_no_q2_no_postfilter() {
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0",
        "intent": "file_search",
        "extensions": ["pdf"]
    })).unwrap();
    let t = translate_intent(&intent, &resolver()).unwrap();
    assert_eq!(t.q1.predicate, "(kMDItemFSName == \"*.pdf\"cd)");
    assert!(t.q2.is_none());
    // 无 keyword → 无需后置过滤（Q1 自己已过滤扩展名）
    assert!(t.post_filters.is_empty());
}
```

> intent 构造模式：**所有测试用 `serde_json::from_value(serde_json::json!({...}))`**（与现有 `translates_location_hints_to_onlyin_paths` 一致），不用结构体字面量。resolver helper 是已有的 `resolver() -> MacOsLocationResolver`（852 行）。

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p locifind-search-spotlight file_search`
Expected: 编译失败（`name_glob_predicate`/`content_predicate` 不存在 + `translate_intent` 返回类型仍是 `SpotlightQuery`）

- [ ] **Step 3: Write implementation**

(a) 新增谓词构建（替换 `keyword_predicate`/`keyword_predicate_expanded` 的角色，旧函数删除或改为内部复用）：

```rust
/// 文件名 glob 关键词谓词（Q1，comparison 类）：FSName + DisplayName 两字段子串 glob。
fn name_glob_predicate(keyword: &str) -> String {
    let g = escape_glob_pattern(keyword);
    format!("(kMDItemFSName == \"*{g}*\"cd || kMDItemDisplayName == \"*{g}*\"cd)")
}

/// 内容关键词谓词（Q2，string 类）。
fn content_predicate(keyword: &str) -> String {
    format!("kMDItemTextContent CONTAINS[cd] \"{}\"", escape_predicate_string(keyword))
}

/// 同义词组：文件名 glob，组内所有词跨 FSName/DisplayName OR。
fn name_glob_predicate_expanded(group: &KeywordGroup) -> String {
    if group.is_singleton() {
        return name_glob_predicate(&group.head);
    }
    let mut parts = Vec::with_capacity(group.all().len() * 2);
    for w in group.all() {
        let g = escape_glob_pattern(w);
        parts.push(format!("kMDItemFSName == \"*{g}*\"cd"));
        parts.push(format!("kMDItemDisplayName == \"*{g}*\"cd"));
    }
    format!("({})", parts.join(" || "))
}

/// 同义词组：内容 CONTAINS，组内所有词 OR。
fn content_predicate_expanded(group: &KeywordGroup) -> String {
    if group.is_singleton() {
        return content_predicate(&group.head);
    }
    let parts: Vec<String> = group
        .all()
        .iter()
        .map(|w| format!("kMDItemTextContent CONTAINS[cd] \"{}\"", escape_predicate_string(w)))
        .collect();
    format!("({})", parts.join(" || "))
}
```

(b) `add_common_file_constraints` 路由（关键词 → cmp + str；扩展名/时间/大小 → cmp + post_filter）。把 443-447 的 keyword 块改为：

```rust
    if let Some(keywords) = constraints.keywords {
        for keyword in keywords.iter().filter(|keyword| !keyword.is_empty()) {
            builder.and_cmp(name_glob_predicate(keyword));
            builder.and_str(content_predicate(keyword));
        }
    }
```

扩展名块（449-455）改为同时加 post_filter：

```rust
    if let Some(extensions) = constraints.extensions {
        if !extensions.is_empty() {
            builder.and_cmp(extension_predicate(extensions, false));
            builder.and_post_filter(PostFilter {
                extension: Some(ExtensionFilter {
                    extensions: extensions.iter().map(|e| e.trim_start_matches('.').to_owned()).collect(),
                    negate: false,
                }),
                ..PostFilter::default()
            });
        }
    } else if let Some(file_type) = constraints.file_type {
        let exts = file_type_extensions(file_type);
        builder.and_cmp(extension_predicate(exts, false));
        builder.and_post_filter(PostFilter {
            extension: Some(ExtensionFilter {
                extensions: exts.iter().map(|e| (*e).to_owned()).collect(),
                negate: false,
            }),
            ..PostFilter::default()
        });
    }
```

时间块（457-465）每个 `time_predicate` 后加对应 `TimeField` post_filter；大小块（466-472）加 `SizeFilter` post_filter；exclude_extensions/exclude_file_type（490-499）加 `negate:true` 的 ExtensionFilter post_filter。示例（modified_time）：

```rust
    if let Some(time) = constraints.modified_time {
        builder.and_cmp(time_predicate("kMDItemContentModificationDate", time));
        builder.and_post_filter(PostFilter {
            time: vec![TimeField::Modified(TimeFilter::from_expression(time)?)],
            ..PostFilter::default()
        });
    }
```

（created_time → `TimeField::Created` + `kMDItemContentCreationDate`；accessed_time → `TimeField::Accessed` + `kMDItemLastUsedDate`；size → `SizeFilter::from_expression(size, UnitDomain::Bytes)`，field `kMDItemFSSize`。）

> **DRY 提醒**：post_filter 与 cmp 谓词必须从同一 `constraints` 字段派生（如上），禁止两处独立计算阈值。

(c) 删除 Task 3 的临时 `and` 垫片；把 `translate_intent`/`translate_intent_expanded`/`translate_file_search(_expanded)`/`translate_media_search(_expanded)` 返回类型从 `SpotlightQuery` 改为 `TranslatedQuery`，末尾 `Ok(builder.finish())`（不再 `.q1`）。`translate_file_search_expanded` 里 `builder.and(keyword_predicate_expanded(group))` 改为：

```rust
        for group in groups.iter().filter(|g| !g.head.is_empty()) {
            builder.and_cmp(name_glob_predicate_expanded(group));
            builder.and_str(content_predicate_expanded(group));
        }
```

> Task 5 处理 media 的 artist/title/album/genre。本 task 先让 media 编译通过：media 的 `builder.and(...)` 暂改 `builder.and_str(...)`（artist/title/album/genre 本就是 CONTAINS，归 Q2 正确）；lossless extension 与 duration 改 `and_cmp` + 相应 post_filter（Task 5 补全测试）。

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p locifind-search-spotlight file_search name_glob content_predicate`
Expected: PASS。**既有谓词形态测试会因形态变化而失败** —— 这是预期的，下一步处理。

- [ ] **Step 5: 更新既有谓词形态断言**

既有测试（`translates_schema_search_cases_1_to_30`、`singleton_group_predicate_is_byte_equal_to_keyword_predicate`、`multi_group_predicate_*` 等）断言旧 `keyword_predicate` 三字段 CONTAINS 形态。逐个改为新 Q1/Q2 形态：
- 取每个测试，把 `translate_*(...).predicate` 改为 `.q1.predicate` / `.q2`，断言新 glob 形态。
- `singleton_group_predicate_is_byte_equal_to_keyword_predicate` 重命名为 `singleton_group_q1_byte_equal_to_name_glob` + `..._q2_byte_equal_to_content`，断言 singleton 组的 Q1==`name_glob_predicate(head)`、Q2==`content_predicate(head)`。
- `multi_group_predicate_or_joins_all_members_across_three_fields`：改为跨 **两** 字段（FSName/DisplayName）的 Q1 + Q2 内容 OR，更新计数（每词 Q1 2 项、Q2 1 项）。
- `escapes_predicate_string_for_shell_injection_resistance`（920 行附近）直接调 `keyword_predicate(malicious)` —— Task 4 删/改该函数后此调用编译失败。改为调 `name_glob_predicate(malicious)` + `content_predicate(malicious)`，断言转义仍生效（`\\\"` + `\\\\`，且 glob 形态含 `== "*...*"cd`）。
- `translates_schema_search_cases_1_to_30`（864 行）断言 `query.predicate` → 改 `query.q1.predicate`（fixture cases 多为带扩展名/类型的搜索，Q1 非空）。

逐个改完跑：`cargo test -p locifind-search-spotlight`
Expected: 全 PASS

- [ ] **Step 6: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 文件搜索 keyword 拆 name-glob(Q1)/content(Q2) + 约束产 PostFilter(BETA-15D Task 4)"
```

---

## Task 5: 媒体搜索路由 + lossless/duration PostFilter

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（`translate_media_search(_expanded)` 239-317 / 346-419）
- Test: 同文件测试模块

媒体字段 artist/title/album/genre 是 CONTAINS（str → Q2，Task 4 已暂改 `and_str`，本 task 加测试锁定）；lossless 默认扩展名 + duration 是 comparison（→ Q1 + PostFilter）。

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn media_search_artist_to_q2_extension_to_q1_and_postfilter() {
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0",
        "intent": "media_search",
        "media_type": "audio",
        "artist": "周杰伦"
    })).unwrap();
    let t = translate_intent(&intent, &resolver()).unwrap();
    // Audio → file_type Audio → 扩展名 glob 进 Q1
    assert!(t.q1.predicate.contains("kMDItemFSName == \"*.mp3\"cd"));
    // artist CONTAINS → Q2
    let q2 = t.q2.unwrap();
    assert!(q2.predicate.contains("kMDItemAuthors CONTAINS[cd] \"周杰伦\""));
    assert!(q2.predicate.contains("kMDItemMusicalGenre CONTAINS[cd] \"周杰伦\""));
    // Audio 默认扩展名集 → PostFilter（让 Q2 命中也被扩展名过滤）
    assert!(t.post_filters.iter().any(|p| p.extension.is_some()));
}

#[test]
fn media_search_duration_to_q1_cmp_no_postfilter() {
    // duration（>10min）形态 = kMDItemDurationSeconds > 600；按 schema 的 duration 表达填充。
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0",
        "intent": "media_search",
        "media_type": "video",
        "duration": { "type": "greater_than", "value": 10.0, "unit": "m" }
    })).unwrap();
    let t = translate_intent(&intent, &resolver()).unwrap();
    assert!(t.q1.predicate.contains("kMDItemDurationSeconds > 600"));
    // duration 不产 PostFilter（metadata 无 duration 字段，见 limitation 注释）
}
```

> duration 用 `kMDItemDurationSeconds`（非 FSSize）；duration 的 PostFilter 因 Q2 不含 duration 字段而**无法在 Rust 端从文件元数据复刻**（`SearchResultMetadata` 无 duration）。决策：**duration 约束不产 PostFilter** —— 媒体 Q2（artist/title 命中）若需 duration 过滤会漏过滤，但 duration+artist 组合罕见，且 `SearchResultMetadata` 无 duration 字段无法过滤。在代码注释标注此 known limitation（YAGNI，不为此扩 metadata）。lossless 默认扩展名**产** PostFilter（扩展名可从 path 过滤）。

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p locifind-search-spotlight media_search`
Expected: FAIL（断言不符 / 编译）

- [ ] **Step 3: Write implementation**

`translate_media_search` 与 `translate_media_search_expanded` 中：
- artist/title/album/genre 四块的 `builder.and(...)` → `builder.and_str(...)`（Task 4 已改，确认无误）。
- lossless 块：`builder.and_cmp(extension_predicate(&["flac","wav","aiff","ape"], false))` + `builder.and_post_filter(PostFilter{ extension: Some(ExtensionFilter{ extensions: vec!["flac","wav","aiff","ape"].into_iter().map(str::to_owned).collect(), negate:false }), ..Default::default() })`。
- duration 块：`builder.and_cmp(size_predicate_with_field("kMDItemDurationSeconds", duration, UnitDomain::Duration)?)`，**不加 post_filter**（加注释说明 limitation）。
- `add_common_file_constraints` 已在 Task 4 处理 keyword/扩展名/时间/大小路由 + PostFilter（media 经它注入 file_type 扩展名 + PostFilter），无需重复。

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p locifind-search-spotlight media_search`
Expected: PASS

- [ ] **Step 5: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 媒体搜索字段路由 Q2 + lossless PostFilter(duration limitation 标注, BETA-15D Task 5)"
```

---

## Task 6: 执行层 — 双查询并发 + 合并去重 + 后置过滤 + stdout sentinel

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`（`search`/`search_expanded` 83-139、`run_mdfind` 735-788）
- Test: 同文件测试模块（fake-mdfind 集成测试）

`search`/`search_expanded` 改为：translate → 并发跑 Q1 + Q2（若有）→ 各自 path→SearchResult → Q2 结果过 `post_filters` → 合并去重（canonical path）→ sort → truncate。`run_mdfind` 检测 `Failed to create query` sentinel。

- [ ] **Step 1: Write the failing test（fake-mdfind 双查询合并）**

复用现有 fake-mdfind 测试基建（`write_executable_script`、`with_mdfind_path`，960-1030 行）。新增测试：fake-mdfind 对 Q1/Q2 不同谓词返回不同路径集，断言合并去重 + Q2 后置过滤生效。

```rust
#[test]
fn dual_query_merges_q1_q2_dedups_and_postfilters_q2() {
    // fake mdfind：根据传入谓词（含 "CONTAINS" 与否）输出不同文件列表
    // Q1（cmp，无 CONTAINS）→ 输出真实存在的 a.ppt
    // Q2（str，含 CONTAINS）→ 输出 a.ppt（重复，应去重）+ b.txt（应被 ppt PostFilter 滤掉）
    // 用真实临时文件让 result_from_path 成功
    let root = std::env::temp_dir().join(format!("locifind-b15d-{}", std::process::id()));
    let _ = fs::create_dir_all(&root);
    let a = root.join("a.ppt"); fs::write(&a, b"x").unwrap();
    let b = root.join("b.txt"); fs::write(&b, b"x").unwrap();
    let script = root.join("fake-mdfind.sh");
    write_executable_script(&script, &format!(
        "#!/bin/sh\ncase \"$*\" in\n  *CONTAINS*) printf '{a}\\n{b}\\n';;\n  *) printf '{a}\\n';;\nesac\n",
        a = a.display(), b = b.display(),
    ));
    let backend = SpotlightBackend::with_resolver(resolver()).with_mdfind_path(script);
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0",
        "intent": "file_search",
        "keywords": ["x"],
        "extensions": ["ppt"]
    })).unwrap();
    // 复用现有 block_on helper（947 行）+ stream.collect 模式（见 mdfind_output_is_post_sorted_before_streaming）
    let stream = block_on(backend.search(&intent, CancellationToken::new())).unwrap();
    let results: Vec<_> = block_on(stream.collect::<Vec<_>>())
        .into_iter()
        .map(|r| r.unwrap())
        .collect();
    let names: Vec<_> = results.iter().map(|r| r.name.clone()).collect();
    assert_eq!(names, vec!["a.ppt".to_string()]); // 去重 + b.txt 被 ppt PostFilter 滤掉
    let _ = fs::remove_dir_all(&root);
}
```

> stream 收集形态以现有 `mdfind_output_is_post_sorted_before_streaming`（1003-1005 行）为准（`block_on(stream.collect::<Vec<_>>())` 后逐个 `.unwrap()`）。

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p locifind-search-spotlight dual_query`
Expected: FAIL（当前 search 只跑单查询）

- [ ] **Step 3: Write implementation**

(a) `run_mdfind` 在读到 stdout 后、返回前，检测 sentinel：

```rust
            if output.status.success() {
                let stdout = String::from_utf8(output.stdout).map_err(|error| SearchError::Io {
                    detail: error.to_string(),
                })?;
                // macOS 26：mdfind 拒绝谓词时把 "Failed to create query" 打到 stdout 且 rc=0。
                if stdout.starts_with("Failed to create query") {
                    return Err(SearchError::Io { detail: stdout.trim().to_owned() });
                }
                return Ok(stdout.lines().map(ToOwned::to_owned).collect());
            }
```

(b) 抽公共执行逻辑 `fn run_translated(&self, t: &TranslatedQuery, sort: SortOrder, cancel: &CancellationToken) -> Result<Vec<SearchResult>, SearchError>`：

```rust
fn run_translated(
    mdfind_path: &Path,
    timeout: Duration,
    query: &TranslatedQuery,
    sort: Option<SortOrder>, // intent_sort_order 返回 Option<SortOrder>；sort_results 已接受 Option
    cancel: &CancellationToken,
) -> Result<Vec<SearchResult>, SearchError> {
    // 并发跑 Q1 / Q2（Q2 可空）。用 std::thread::scope。
    let q1 = &query.q1;
    let q2 = query.q2.as_ref();
    let (r1, r2): (Result<Vec<String>, SearchError>, Option<Result<Vec<String>, SearchError>>) =
        std::thread::scope(|s| {
            let h2 = q2.map(|q| s.spawn(|| run_mdfind(mdfind_path, q, timeout, cancel)));
            let r1 = run_mdfind(mdfind_path, q1, timeout, cancel);
            let r2 = h2.map(|h| h.join().expect("mdfind thread panicked"));
            (r1, r2)
        });
    let lines1 = r1?;
    let lines2 = match r2 { Some(r) => r?, None => Vec::new() };

    let mut seen = std::collections::HashSet::new();
    let mut results: Vec<SearchResult> = Vec::new();
    let mut push = |line: &str, post: Option<&[PostFilter]>, results: &mut Vec<SearchResult>, seen: &mut std::collections::HashSet<PathBuf>| {
        if cancel.is_cancelled() { return; }
        let line = line.trim();
        if line.is_empty() { return; }
        let path = PathBuf::from(line);
        if is_excluded(&path, &q1.exclude_paths) { return; }
        if let Ok(result) = result_from_path(&path) {
            if let Some(filters) = post {
                if !filters.iter().all(|f| f.matches(&result)) { return; }
            }
            if seen.insert(result.path.clone()) {
                results.push(result);
            }
        }
    };
    // Q1 命中无需后置过滤（谓词已含约束）；Q2 命中需过 post_filters。
    for l in &lines1 { push(l, None, &mut results, &mut seen); }
    for l in &lines2 { push(l, Some(&query.post_filters), &mut results, &mut seen); }

    sort_results(&mut results, sort);
    results.truncate(q1.limit);
    Ok(results)
}
```

> 闭包借用冲突若编译不过，改为普通函数或内联两个循环。`SortOrder` 即 `intent_sort_order` 返回类型，按现有签名命名。`std::thread::scope` 中 `cancel`/`mdfind_path`/`timeout` 借用 OK（scope 保证 join）。

(c) `search` / `search_expanded` 改为：

```rust
    fn search<'a>(&'a self, intent: &'a SearchIntent, cancel: CancellationToken) -> BackendSearchFuture<'a> {
        Box::pin(async move {
            let query = translate_intent(intent, &self.resolver)?;
            let results = run_translated(&self.mdfind_path, self.timeout, &query, intent_sort_order(intent), &cancel)?;
            Ok(backend_stream_from_results(results, cancel))
        })
    }
```

`search_expanded` 同理（`translate_intent_expanded` + `intent_sort_order(&expanded.base)`）。

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p locifind-search-spotlight`
Expected: PASS（含既有 sort/cancel/timeout 集成测试 —— 它们的 fake-mdfind 现会被调用 1~2 次；cancel/timeout 测试用纯 keyword 或纯 ext intent，确认仍过；若某测试因双查询多调一次 mdfind 而断言调用次数，更新断言）

- [ ] **Step 5: stdout sentinel 单测**

```rust
#[test]
fn run_mdfind_treats_failed_to_create_query_as_error() {
    let root = std::env::temp_dir().join(format!("locifind-b15d-sentinel-{}", std::process::id()));
    let _ = fs::create_dir_all(&root);
    let script = root.join("fake-mdfind.sh");
    write_executable_script(&script, "#!/bin/sh\nprintf 'Failed to create query for ...\\n'\n");
    let backend = SpotlightBackend::with_resolver(resolver()).with_mdfind_path(script);
    let intent: SearchIntent = serde_json::from_value(serde_json::json!({
        "schema_version": "1.0", "intent": "file_search", "extensions": ["pdf"]
    })).unwrap();
    // search 内 run_mdfind 报错 → future resolve 为 Err
    let err = block_on(backend.search(&intent, CancellationToken::new())).unwrap_err();
    assert!(matches!(err, SearchError::Io { .. }));
    let _ = fs::remove_dir_all(&root);
}
```

Run: `cargo test -p locifind-search-spotlight run_mdfind`
Expected: PASS

- [ ] **Step 6: 验证门 + Commit**

Run: `cargo fmt --check && cargo clippy -p locifind-search-spotlight --all-targets -- -D warnings && cargo test -p locifind-search-spotlight`

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 执行层双查询并发+合并去重+Q2后置过滤+stdout sentinel(BETA-15D Task 6)"
```

---

## Task 7: 全量验证 + 真机端到端准备 + 文档同步

**Files:**
- Modify: `docs/manual-test-scenarios.md`（新增 BETA-15D 真机 case）
- Verify-only: `bash scripts/ci.sh`、evals

- [ ] **Step 1: 全 workspace CI**

Run: `bash scripts/ci.sh`
Expected: 全 PASS（fmt + clippy + 全 crate test）。若 desktop crate 因 spotlight API 变更（`translate_intent` 返回 `TranslatedQuery`）编译失败 —— 检查 desktop 是否直接调 `translate_intent`/`SpotlightQuery`（应不调，desktop 经 `SearchBackend` trait）。若有调用点按新类型调整（应无，spec 限定不改 desktop 源；若有则是隐藏耦合，记录并最小修）。

- [ ] **Step 2: evals byte-equal 回归**

Run: `cargo run -p locifind-evals --bin evals --release -- --baseline`（按 repo 实际 evals 跑法，见 `packages/evals`）
Expected: parser-only **472 / 26 / 2**（不依赖 spotlight，应 byte-equal）

- [ ] **Step 3: 真机谓词 smoke（agent 可跑 mdfind，验证不再 Failed to create query）**

Run（验证修复后 Q1 形态被 mdfind 接受）：
```bash
mdfind '(kMDItemFSName == "*述职*"cd || kMDItemDisplayName == "*述职*"cd) && (kMDItemFSName == "*.ppt"cd)' | head
```
Expected: 输出 `/Users/alice/Desktop/locifind-beta11-fixtures/述职.ppt`，**无** `Failed to create query`

- [ ] **Step 4: 更新手测文档**

在 `docs/manual-test-scenarios.md` BETA-11 节后新增 BETA-15D 真机 case（用户驱动 `LOCIFIND_TRACE=/tmp/b15d.jsonl npm run tauri dev`）：
1. `找一份工作汇报相关的ppt` → 命中 述职.ppt（同义词 + 复合，端到端必过）
2. `找最近一周的 pdf`（keyword+时间复合，验证不再 Failed）
3. `找大于 1MB 的 ppt`（keyword+大小复合）
4. `述职`（纯 keyword，验证 CJK 文件名命中）
5. `find pdf`（纯扩展名回归，第 29 阶段路径不破）

- [ ] **Step 5: Commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "docs(beta-15d): 新增真机手测 5 case + 全量验证通过(BETA-15D Task 7)"
```

---

## Self-Review（写完计划后自查，已执行）

**Spec coverage：**
- §2 决策 1 统一双查询 → Task 3（builder）+ Task 6（执行）。
- §2 决策 2 文件名 glob / 内容 CONTAINS → Task 4。
- §2 决策 3 PostFilter Rust 复刻 → Task 2（核心）+ Task 4/5（注入）。
- §2 决策 4 glob 转义 → Task 1。
- §3 改动清单（4 translate + builder + run + escape + 单测）→ Task 3-6。
- §5 stdout sentinel 加固 → Task 6 Step 1/5。
- media 覆盖 → Task 5。search_expanded 覆盖 → Task 4/6。
- §7 验证门 → 每 task + Task 7。

**已知取舍（spec 内已认可 / 计划内标注）：** duration PostFilter 不复刻（metadata 无 duration 字段，YAGNI）；时间锚点用 Local 今天午夜对齐 mdfind `$time.today`。

**Type 一致性：** `TranslatedQuery{q1,q2,post_filters}`、`PostFilter{extension,size,time}`、`ExtensionFilter{extensions,negate}`、`SizeFilter{min,max,min_inclusive,max_inclusive}`、`TimeFilter{from,to}`、`TimeField::{Modified,Created,Accessed}`、builder `and_cmp/and_str/and_post_filter` —— 跨 task 命名一致。

**Placeholder 扫描：** 无 TBD/TODO；每个 code step 含完整代码（Task 4 的时间/exclude 块给了模板 + 明确字段映射，属机械重复非 placeholder）。
