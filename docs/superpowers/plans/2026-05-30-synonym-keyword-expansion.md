# 同义词关键词扩展（手维护词典）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 给 LociFind 加一条轻量、可解释、可演示的同义词扩展能力，让自然语言查询 `找一份工作汇报相关的ppt` 能命中文件 `述职.ppt`。

**Architecture:** harness 层新增 `SynonymExpander` 中间件，介于 IntentRouter 与 SearchBackend 之间。`SearchIntent` 不动（保 evals）；新建 `ExpandedSearchIntent`（含 `KeywordGroup`）作组间 AND / 组内 OR 的载体。词典手维护 YAML（zh + en 分文件），加载期 lint + 运行期 cap。`SearchBackend` trait 加 default `search_expanded`，BETA-11 仅 Spotlight 覆盖；其它后端自动 fallback。

**Tech Stack:** Rust 1.95、serde_yaml、Tauri 2、harness/common/spotlight 三 crate、desktop crate `SearchDeps`（第 33 阶段刚收拢）。

**对 spec 的偏离**：spec §4.1 把 `ExpandedSearchIntent` 放在 `packages/harness/src/synonym/expanded.rs`。实施时发现 `SearchBackend` 在 `common` crate 且 harness 依赖 common（反向不行）。本 plan 把类型挪到 `packages/search-backends/common/src/expanded.rs`，harness 再 re-export，其它语义零差异。

---

## 文件结构

**新建**

| 路径 | 责任 |
|---|---|
| `packages/search-backends/common/src/expanded.rs` | `ExpandedSearchIntent` + `KeywordGroup` 类型 |
| `packages/harness/src/synonym/mod.rs` | synonym 子模块入口 |
| `packages/harness/src/synonym/expander.rs` | `SynonymExpander` trait + `NoopExpander` |
| `packages/harness/src/synonym/yaml.rs` | `YamlSynonymExpander` + `ExpanderError` + lint + cap |
| `resources/synonyms/zh.yaml` | 中文同义词词典 |
| `resources/synonyms/en.yaml` | 英文同义词词典 |

**修改**

| 路径 | 改动要点 |
|---|---|
| `packages/search-backends/common/src/lib.rs` | 导出 `expanded` 模块；`SearchBackend` trait 加 `search_expanded` default method |
| `packages/search-backends/common/Cargo.toml` | 已有依赖（无新增） |
| `packages/search-backends/spotlight/src/lib.rs` | 覆盖 `search_expanded`；新增 `keyword_predicate_expanded` |
| `packages/harness/src/lib.rs` | 加 `pub mod synonym;` + re-export `ExpandedSearchIntent` / `KeywordGroup` / `SynonymExpander` / `NoopExpander` / `YamlSynonymExpander` / `ExpanderError` |
| `packages/harness/Cargo.toml` | 加 `serde_yaml` 依赖 |
| `packages/harness/src/tracing.rs` | 加 `SynonymExpandEvent` + `Tracer::on_synonym_expand` + `TracingHook::on_synonym_expand` 默认空实现 |
| `apps/desktop/src-tauri/src/search.rs` | `SearchDeps` 新增 `synonym_expander: Arc<dyn SynonymExpander>`；`search_impl` 调 `expand()` + trace |
| `apps/desktop/src-tauri/src/main.rs` | 启动期 `setup` hook 内拿 `AppHandle` 解析词典路径（dev / packaged 两态）+ 构造 `YamlSynonymExpander`，失败 fallback `NoopExpander` |
| `apps/desktop/src-tauri/tauri.conf.json` | `bundle.resources` 新增 `["../../../resources/synonyms/*.yaml"]` |
| `docs/manual-test-scenarios.md` | 追加 BETA-11 节，列 8 个手测 scenario |

---

## Task 1：`ExpandedSearchIntent` + `KeywordGroup` 类型（common crate）

**Files:**
- Create: `packages/search-backends/common/src/expanded.rs`
- Modify: `packages/search-backends/common/src/lib.rs`

- [ ] **Step 1: Write the failing test**

新文件 `packages/search-backends/common/src/expanded.rs`：

```rust
//! 经过同义词扩展后的搜索意图。组间 AND、组内 OR。

use crate::SearchIntent;
use serde::{Deserialize, Serialize};

/// 单个 keyword 的同义词组。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeywordGroup {
    /// parser 原始词（lookup key）。
    pub head: String,
    /// 不含 head 的等价别名。与 head OR 拼起。
    pub synonyms: Vec<String>,
}

impl KeywordGroup {
    /// 构造一个仅含 head 的组（未扩词）。
    #[must_use]
    pub fn singleton(head: impl Into<String>) -> Self {
        Self {
            head: head.into(),
            synonyms: Vec::new(),
        }
    }

    /// 组内所有词（head 在首位 + synonyms 顺序保留）。
    #[must_use]
    pub fn all(&self) -> Vec<&str> {
        let mut out = Vec::with_capacity(1 + self.synonyms.len());
        out.push(self.head.as_str());
        out.extend(self.synonyms.iter().map(String::as_str));
        out
    }

    /// 是否未扩词。
    #[must_use]
    pub fn is_singleton(&self) -> bool {
        self.synonyms.is_empty()
    }
}

/// 扩展后的搜索意图。保留原 `SearchIntent` 不动，附带按 keyword 顺序对齐的同义词组。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExpandedSearchIntent {
    pub base: SearchIntent,
    /// 与 `base` 中 *Search variant 的 keywords 顺序对齐；非 search variant 时为空。
    pub keyword_groups: Vec<KeywordGroup>,
}

impl ExpandedSearchIntent {
    /// 构造一个未扩词的 expanded（恒等映射）。
    #[must_use]
    pub fn identity(base: SearchIntent) -> Self {
        let keyword_groups = base
            .search_keywords()
            .map(|kws| kws.iter().map(KeywordGroup::singleton).collect())
            .unwrap_or_default();
        Self { base, keyword_groups }
    }

    /// 是否所有组都未扩词。
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.keyword_groups.iter().all(KeywordGroup::is_singleton)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleton_group_has_no_synonyms() {
        let g = KeywordGroup::singleton("工作汇报");
        assert_eq!(g.head, "工作汇报");
        assert!(g.synonyms.is_empty());
        assert!(g.is_singleton());
        assert_eq!(g.all(), vec!["工作汇报"]);
    }

    #[test]
    fn group_all_preserves_head_then_synonyms_order() {
        let g = KeywordGroup {
            head: "工作汇报".into(),
            synonyms: vec!["述职".into(), "年度总结".into()],
        };
        assert_eq!(g.all(), vec!["工作汇报", "述职", "年度总结"]);
        assert!(!g.is_singleton());
    }
}
```

Modify `packages/search-backends/common/src/lib.rs`：在文件中找一个合适位置（其他 `pub mod` 声明附近）插入：

```rust
pub mod expanded;
pub use expanded::{ExpandedSearchIntent, KeywordGroup};
```

另外查找 `SearchIntent` 是否已有 `search_keywords()` 方法。若没有，加：

```rust
impl SearchIntent {
    /// 返回 *Search variant 的 keywords 切片（FileSearch / MediaSearch）。其他 variant 返回 None。
    #[must_use]
    pub fn search_keywords(&self) -> Option<&[String]> {
        match self {
            Self::FileSearch(s) => s.keywords.as_deref(),
            Self::MediaSearch(s) => s.keywords.as_deref(),
            _ => None,
        }
    }
}
```

> 注：实际 variant 与字段名以仓内 `SearchIntent` 定义为准。若 `keywords` 类型是 `Vec<String>` 而非 `Option<Vec<String>>`，去掉 `.as_deref()` 直接 `Some(&s.keywords)`。

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-search-backend expanded::tests -- --exact 2>&1 | head -30
```

预期：编译失败（`expanded` 模块未导出）或 test 未找到。

- [ ] **Step 3: Write minimal implementation**

Step 1 已写完整实现，Step 3 留空（合并到 Step 1）。直接进 Step 4。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-search-backend --all-targets -- -D warnings
cargo test -p locifind-search-backend expanded::tests
```

预期：3 项全过；2 test pass。

- [ ] **Step 5: Commit**

```bash
git add packages/search-backends/common/src/expanded.rs \
        packages/search-backends/common/src/lib.rs
git commit -m "feat(common): 新增 ExpandedSearchIntent + KeywordGroup 类型(BETA-11 Task 1)"
```

---

## Task 2：`SynonymExpander` trait + `NoopExpander`（harness crate）

**Files:**
- Create: `packages/harness/src/synonym/mod.rs`
- Create: `packages/harness/src/synonym/expander.rs`

- [ ] **Step 1: Write the failing test**

新文件 `packages/harness/src/synonym/mod.rs`：

```rust
//! 同义词关键词扩展（BETA-11）。
//!
//! 在 IntentRouter 之后、SearchBackend 之前把单 keyword 扩成等价词组。

pub mod expander;
pub mod yaml;

pub use expander::{NoopExpander, SynonymExpander};
pub use yaml::{ExpanderError, YamlSynonymExpander};
```

新文件 `packages/harness/src/synonym/expander.rs`：

```rust
//! `SynonymExpander` trait + `NoopExpander` 恒等实现。

use locifind_search_backend::{ExpandedSearchIntent, SearchIntent};

/// 把 `SearchIntent` 扩展为 `ExpandedSearchIntent`。
///
/// 实现需保证：未命中词典的 keyword 产出 singleton group（`synonyms` 为空），
/// 后端拿到 singleton 时行为与原 `SearchBackend::search(base)` byte-equal。
pub trait SynonymExpander: Send + Sync + std::fmt::Debug {
    fn expand(&self, intent: SearchIntent) -> ExpandedSearchIntent;
}

/// 恒等实现：把 intent 包成全 singleton 的 `ExpandedSearchIntent`。
/// 用于测试 / 关闭场景 / 词典加载失败的 fallback。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExpander;

impl SynonymExpander for NoopExpander {
    fn expand(&self, intent: SearchIntent) -> ExpandedSearchIntent {
        ExpandedSearchIntent::identity(intent)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use locifind_search_backend::{FileSearchIntent, SearchIntent};

    fn intent_with_keywords(kws: Vec<&str>) -> SearchIntent {
        // 实际构造按 SearchIntent::FileSearch 真实字段。占位 builder:
        // 真实代码中按仓内 FileSearchIntent::minimal_with_keywords(...) 等工厂构造。
        SearchIntent::FileSearch(FileSearchIntent {
            keywords: Some(kws.into_iter().map(String::from).collect()),
            ..FileSearchIntent::default()
        })
    }

    #[test]
    fn noop_returns_identity_singleton_groups() {
        let intent = intent_with_keywords(vec!["工作汇报", "ppt"]);
        let expanded = NoopExpander.expand(intent);
        assert_eq!(expanded.keyword_groups.len(), 2);
        assert!(expanded.is_identity());
        assert_eq!(expanded.keyword_groups[0].head, "工作汇报");
        assert!(expanded.keyword_groups[0].synonyms.is_empty());
        assert_eq!(expanded.keyword_groups[1].head, "ppt");
    }

    #[test]
    fn noop_on_non_search_intent_produces_empty_groups() {
        // Refine / Clarify / FileAction 等 variant 走 search_keywords() -> None
        let intent = intent_with_keywords(vec![]);
        let expanded = NoopExpander.expand(intent);
        assert!(expanded.keyword_groups.is_empty());
    }
}
```

> `FileSearchIntent::default()` 需要 `FileSearchIntent` impl 了 `Default`。若仓内未实现，按真实必需字段在测试里用 `FileSearchIntent { keywords: ..., language: None, ... }` 直接字段初始化。

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-harness synonym::expander::tests -- --exact 2>&1 | head -30
```

预期：模块未找到（`synonym` 未在 lib.rs 导出，Task 6 才做）。先用 `cargo test -p locifind-harness --no-run 2>&1` 看编译错误。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

临时在 `packages/harness/src/lib.rs` 顶部加 `pub mod synonym;`（Task 6 会正式做）：

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness synonym::expander::tests
```

预期：全过；2 test pass。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/synonym/mod.rs \
        packages/harness/src/synonym/expander.rs \
        packages/harness/src/lib.rs
git commit -m "feat(harness): 新增 SynonymExpander trait + NoopExpander(BETA-11 Task 2)"
```

---

## Task 3：YAML 加载 + 词典 lint（构造期 fail-fast）

**Files:**
- Create: `packages/harness/src/synonym/yaml.rs`
- Modify: `packages/harness/Cargo.toml`

- [ ] **Step 1: Write the failing test**

修改 `packages/harness/Cargo.toml`，`[dependencies]` 加：

```toml
serde_yaml = "0.9"
```

新文件 `packages/harness/src/synonym/yaml.rs`（仅 lint 相关部分；展开/cap 在 Task 4 / 5）：

```rust
//! YAML 词典加载 + lint。BETA-11 仅校验结构，不做语言扩展。

use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

const SUPPORTED_VERSION: u32 = 1;
const MAX_ALIASES_PER_GROUP: usize = 8;

#[derive(Debug, Error)]
pub enum ExpanderError {
    #[error("读取词典文件 {path:?} 失败: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("解析 YAML {path:?} 失败: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("词典 {path:?} version={got},仅支持 version=1")]
    UnsupportedVersion { path: PathBuf, got: u32 },
    #[error("词典 {path:?} language={got},此文件期望 language={expected}")]
    LanguageMismatch {
        path: PathBuf,
        expected: &'static str,
        got: String,
    },
    #[error("词典 {path:?} 组 head={head:?}: aliases 数量 {n} > 上限 {MAX_ALIASES_PER_GROUP}")]
    TooManyAliases {
        path: PathBuf,
        head: String,
        n: usize,
    },
    #[error("词典 {path:?} 组 head={head:?} 含跨语言 alias {alias:?}(expected {expected})")]
    CrossLanguageAlias {
        path: PathBuf,
        head: String,
        alias: String,
        expected: &'static str,
    },
    #[error("词典 {path:?} 组 head={head:?} 内出现重复词 {dup:?}")]
    DuplicateWithinGroup {
        path: PathBuf,
        head: String,
        dup: String,
    },
    #[error("词典 {path:?} 词 {word:?} 在多个组中作为 head 或 alias 出现")]
    DuplicateAcrossGroups { path: PathBuf, word: String },
}

#[derive(Debug, Deserialize)]
struct RawDict {
    version: u32,
    language: String,
    groups: Vec<RawGroup>,
}

#[derive(Debug, Deserialize)]
struct RawGroup {
    head: String,
    aliases: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    domain: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedDict {
    pub(crate) language: &'static str,
    pub(crate) groups: Vec<ParsedGroup>,
}

#[derive(Debug, Clone)]
pub(crate) struct ParsedGroup {
    pub(crate) head: String,
    pub(crate) aliases: Vec<String>,
}

pub(crate) fn parse_dict_str(
    yaml: &str,
    path: &Path,
    expected_lang: &'static str,
) -> Result<ParsedDict, ExpanderError> {
    let raw: RawDict = serde_yaml::from_str(yaml).map_err(|source| ExpanderError::Parse {
        path: path.to_path_buf(),
        source,
    })?;
    if raw.version != SUPPORTED_VERSION {
        return Err(ExpanderError::UnsupportedVersion {
            path: path.to_path_buf(),
            got: raw.version,
        });
    }
    if raw.language != expected_lang {
        return Err(ExpanderError::LanguageMismatch {
            path: path.to_path_buf(),
            expected: expected_lang,
            got: raw.language,
        });
    }

    let mut seen_words: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut groups = Vec::with_capacity(raw.groups.len());
    for g in raw.groups {
        if g.aliases.len() > MAX_ALIASES_PER_GROUP {
            return Err(ExpanderError::TooManyAliases {
                path: path.to_path_buf(),
                head: g.head,
                n: g.aliases.len(),
            });
        }
        let all: Vec<&str> = std::iter::once(g.head.as_str())
            .chain(g.aliases.iter().map(String::as_str))
            .collect();
        let mut intra: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for w in &all {
            if !intra.insert(*w) {
                return Err(ExpanderError::DuplicateWithinGroup {
                    path: path.to_path_buf(),
                    head: g.head.clone(),
                    dup: (*w).into(),
                });
            }
            if has_cross_language_char(w, expected_lang) {
                return Err(ExpanderError::CrossLanguageAlias {
                    path: path.to_path_buf(),
                    head: g.head.clone(),
                    alias: (*w).into(),
                    expected: expected_lang,
                });
            }
        }
        for w in &all {
            if !seen_words.insert((*w).into()) {
                return Err(ExpanderError::DuplicateAcrossGroups {
                    path: path.to_path_buf(),
                    word: (*w).into(),
                });
            }
        }
        groups.push(ParsedGroup {
            head: g.head,
            aliases: g.aliases,
        });
    }
    Ok(ParsedDict {
        language: expected_lang,
        groups,
    })
}

fn has_cross_language_char(word: &str, expected_lang: &str) -> bool {
    let has_ascii_alpha = word.chars().any(|c| c.is_ascii_alphabetic());
    let has_cjk = word.chars().any(is_cjk);
    match expected_lang {
        "zh" => has_ascii_alpha,
        "en" => has_cjk,
        _ => false,
    }
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x4E00..=0x9FFF |   // CJK Unified Ideographs
        0x3400..=0x4DBF |   // CJK Extension A
        0x20000..=0x2A6DF | // CJK Extension B
        0x3000..=0x303F |   // CJK 符号与标点
        0xFF00..=0xFFEF     // 全角形式
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::path::PathBuf;

    fn p() -> PathBuf {
        PathBuf::from("/tmp/test.yaml")
    }

    #[test]
    fn accepts_well_formed_zh_dict() {
        let yaml = r#"
version: 1
language: zh
groups:
  - head: 工作汇报
    aliases: [述职, 年度总结]
  - head: 截图
    aliases: [截屏, 屏幕截图]
"#;
        let d = parse_dict_str(yaml, &p(), "zh").unwrap();
        assert_eq!(d.language, "zh");
        assert_eq!(d.groups.len(), 2);
        assert_eq!(d.groups[0].head, "工作汇报");
        assert_eq!(d.groups[0].aliases, vec!["述职", "年度总结"]);
    }

    #[test]
    fn rejects_too_many_aliases() {
        let yaml = r#"
version: 1
language: zh
groups:
  - head: 工作汇报
    aliases: [a1, a2, a3, a4, a5, a6, a7, a8, a9]
"#;
        match parse_dict_str(yaml, &p(), "zh") {
            Err(ExpanderError::TooManyAliases { n, .. }) => assert_eq!(n, 9),
            other => panic!("expected TooManyAliases, got {other:?}"),
        }
    }

    #[test]
    fn rejects_cross_language_alias_in_zh_dict() {
        let yaml = r#"
version: 1
language: zh
groups:
  - head: 合同
    aliases: [协议, contract]
"#;
        match parse_dict_str(yaml, &p(), "zh") {
            Err(ExpanderError::CrossLanguageAlias { alias, .. }) => assert_eq!(alias, "contract"),
            other => panic!("expected CrossLanguageAlias, got {other:?}"),
        }
    }

    #[test]
    fn rejects_cjk_in_en_dict() {
        let yaml = r#"
version: 1
language: en
groups:
  - head: slides
    aliases: [slideshow, 幻灯片]
"#;
        match parse_dict_str(yaml, &p(), "en") {
            Err(ExpanderError::CrossLanguageAlias { alias, .. }) => assert_eq!(alias, "幻灯片"),
            other => panic!("expected CrossLanguageAlias, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_within_group() {
        let yaml = r#"
version: 1
language: zh
groups:
  - head: 截图
    aliases: [截图, 截屏]
"#;
        match parse_dict_str(yaml, &p(), "zh") {
            Err(ExpanderError::DuplicateWithinGroup { dup, .. }) => assert_eq!(dup, "截图"),
            other => panic!("expected DuplicateWithinGroup, got {other:?}"),
        }
    }

    #[test]
    fn rejects_duplicate_across_groups() {
        let yaml = r#"
version: 1
language: zh
groups:
  - head: 截图
    aliases: [截屏]
  - head: 屏幕快照
    aliases: [截屏]
"#;
        match parse_dict_str(yaml, &p(), "zh") {
            Err(ExpanderError::DuplicateAcrossGroups { word, .. }) => assert_eq!(word, "截屏"),
            other => panic!("expected DuplicateAcrossGroups, got {other:?}"),
        }
    }

    #[test]
    fn rejects_wrong_version() {
        let yaml = r#"
version: 2
language: zh
groups: []
"#;
        match parse_dict_str(yaml, &p(), "zh") {
            Err(ExpanderError::UnsupportedVersion { got, .. }) => assert_eq!(got, 2),
            other => panic!("expected UnsupportedVersion, got {other:?}"),
        }
    }
}
```

> 若 harness 已用 `thiserror`，复用现有依赖。否则在 `Cargo.toml` 加 `thiserror = "1"`。

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-harness synonym::yaml::tests 2>&1 | head -30
```

预期：编译失败（模块未引入）或 test 未找到。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。在 `packages/harness/src/synonym/mod.rs` 已声明 `pub mod yaml;`。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness synonym::yaml::tests
```

预期：全过；7 test pass。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs \
        packages/harness/src/synonym/mod.rs \
        packages/harness/Cargo.toml
git commit -m "feat(harness): 词典 YAML 加载 + 7 项 lint(BETA-11 Task 3)"
```

---

## Task 4：`YamlSynonymExpander` 双向展开 + 语言判定

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`

- [ ] **Step 1: Write the failing test**

在 `yaml.rs` 现有内容下追加（保留 Task 3 的 `parse_dict_str` + 7 项 lint test）：

```rust
use locifind_search_backend::{ExpandedSearchIntent, KeywordGroup, SearchIntent};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use crate::synonym::expander::SynonymExpander;

/// YAML 词典实现的 `SynonymExpander`。
#[derive(Debug)]
pub struct YamlSynonymExpander {
    /// head/alias -> 该组所有成员（含 head）。
    zh_index: HashMap<String, Arc<[String]>>,
    en_index: HashMap<String, Arc<[String]>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeywordLang {
    Zh,
    En,
    /// 混合（含 ASCII 字母 + 含 CJK / `-` / `_` 之类标识符）或无意义。不扩。
    Skip,
}

impl YamlSynonymExpander {
    /// 从两份 YAML 文件加载（zh + en）。
    pub fn from_paths(zh: &Path, en: &Path) -> Result<Self, ExpanderError> {
        let zh_yaml = fs::read_to_string(zh).map_err(|source| ExpanderError::Io {
            path: zh.to_path_buf(),
            source,
        })?;
        let en_yaml = fs::read_to_string(en).map_err(|source| ExpanderError::Io {
            path: en.to_path_buf(),
            source,
        })?;
        Self::from_str(&zh_yaml, zh, &en_yaml, en)
    }

    /// 从字符串构造（测试便利 + 上面 `from_paths` 复用）。
    pub fn from_str(
        zh_yaml: &str,
        zh_path: &Path,
        en_yaml: &str,
        en_path: &Path,
    ) -> Result<Self, ExpanderError> {
        let zh = parse_dict_str(zh_yaml, zh_path, "zh")?;
        let en = parse_dict_str(en_yaml, en_path, "en")?;
        Ok(Self {
            zh_index: build_index(&zh.groups),
            en_index: build_index(&en.groups),
        })
    }

    fn lookup(&self, lang: KeywordLang, keyword: &str) -> Option<&Arc<[String]>> {
        match lang {
            KeywordLang::Zh => self.zh_index.get(keyword),
            KeywordLang::En => self.en_index.get(keyword),
            KeywordLang::Skip => None,
        }
    }
}

impl SynonymExpander for YamlSynonymExpander {
    fn expand(&self, intent: SearchIntent) -> ExpandedSearchIntent {
        let groups = match intent.search_keywords() {
            None => Vec::new(),
            Some(kws) => kws
                .iter()
                .map(|kw| self.expand_one(kw))
                .collect::<Vec<_>>(),
        };
        ExpandedSearchIntent {
            base: intent,
            keyword_groups: groups,
        }
    }
}

impl YamlSynonymExpander {
    fn expand_one(&self, keyword: &str) -> KeywordGroup {
        let lang = classify(keyword);
        match self.lookup(lang, keyword) {
            None => KeywordGroup::singleton(keyword),
            Some(group_members) => {
                // head 位是命中词，其余按词典原顺序追加（不含命中词本身）。
                let synonyms: Vec<String> = group_members
                    .iter()
                    .filter(|w| w.as_str() != keyword)
                    .cloned()
                    .collect();
                KeywordGroup {
                    head: keyword.to_string(),
                    synonyms,
                }
            }
        }
    }
}

fn build_index(groups: &[ParsedGroup]) -> HashMap<String, Arc<[String]>> {
    let mut idx = HashMap::new();
    for g in groups {
        let members: Arc<[String]> = std::iter::once(g.head.clone())
            .chain(g.aliases.iter().cloned())
            .collect::<Vec<String>>()
            .into();
        for w in members.iter() {
            idx.insert(w.clone(), Arc::clone(&members));
        }
    }
    idx
}

fn classify(keyword: &str) -> KeywordLang {
    let has_ascii_alpha = keyword.chars().any(|c| c.is_ascii_alphabetic());
    let has_cjk = keyword.chars().any(is_cjk);
    let has_other_ascii = keyword
        .chars()
        .any(|c| c == '-' || c == '_' || (c.is_ascii() && !c.is_ascii_alphanumeric() && !c.is_whitespace()));
    let only_digits_or_symbols = !has_ascii_alpha && !has_cjk;

    if only_digits_or_symbols {
        return KeywordLang::Skip;
    }
    if has_ascii_alpha && (has_cjk || has_other_ascii) {
        // synthetic-place / synthetic-place-笔记
        return KeywordLang::Skip;
    }
    if has_cjk && !has_ascii_alpha {
        return KeywordLang::Zh;
    }
    if has_ascii_alpha && !has_cjk {
        return KeywordLang::En;
    }
    KeywordLang::Skip
}

#[cfg(test)]
mod expand_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use locifind_search_backend::{FileSearchIntent, SearchIntent};
    use std::path::PathBuf;

    fn zh_yaml() -> &'static str {
        r#"
version: 1
language: zh
groups:
  - head: 工作汇报
    aliases: [述职, 年度总结]
  - head: 截图
    aliases: [截屏, 屏幕截图]
"#
    }

    fn en_yaml() -> &'static str {
        r#"
version: 1
language: en
groups:
  - head: slides
    aliases: [slideshow, presentation]
"#
    }

    fn expander() -> YamlSynonymExpander {
        YamlSynonymExpander::from_str(
            zh_yaml(),
            &PathBuf::from("zh.yaml"),
            en_yaml(),
            &PathBuf::from("en.yaml"),
        )
        .unwrap()
    }

    fn intent_with(kws: Vec<&str>) -> SearchIntent {
        SearchIntent::FileSearch(FileSearchIntent {
            keywords: Some(kws.into_iter().map(String::from).collect()),
            ..FileSearchIntent::default()
        })
    }

    #[test]
    fn classify_pure_chinese_is_zh() {
        assert_eq!(classify("工作汇报"), KeywordLang::Zh);
    }

    #[test]
    fn classify_pure_english_is_en() {
        assert_eq!(classify("slides"), KeywordLang::En);
    }

    #[test]
    fn classify_hyphenated_identifier_is_skip() {
        assert_eq!(classify("synthetic-place"), KeywordLang::Skip);
        assert_eq!(classify("synthetic-place-笔记"), KeywordLang::Skip);
    }

    #[test]
    fn classify_digits_only_is_skip() {
        assert_eq!(classify("2024"), KeywordLang::Skip);
    }

    #[test]
    fn zh_keyword_expands_with_head_first() {
        let e = expander();
        let out = e.expand(intent_with(vec!["工作汇报"]));
        assert_eq!(out.keyword_groups.len(), 1);
        let g = &out.keyword_groups[0];
        assert_eq!(g.head, "工作汇报");
        assert_eq!(g.synonyms, vec!["述职", "年度总结"]);
    }

    #[test]
    fn zh_alias_as_input_makes_alias_the_head() {
        let e = expander();
        let out = e.expand(intent_with(vec!["述职"]));
        let g = &out.keyword_groups[0];
        assert_eq!(g.head, "述职");
        // head 位是命中词；其余按词典原顺序（不含命中词）
        assert_eq!(g.synonyms, vec!["工作汇报", "年度总结"]);
    }

    #[test]
    fn en_keyword_expands_via_en_index() {
        let e = expander();
        let out = e.expand(intent_with(vec!["slides"]));
        let g = &out.keyword_groups[0];
        assert_eq!(g.head, "slides");
        assert_eq!(g.synonyms, vec!["slideshow", "presentation"]);
    }

    #[test]
    fn miss_in_dict_returns_singleton() {
        let e = expander();
        let out = e.expand(intent_with(vec!["完全不存在的词"]));
        assert!(out.keyword_groups[0].is_singleton());
    }

    #[test]
    fn mixed_identifier_is_singleton() {
        let e = expander();
        let out = e.expand(intent_with(vec!["synthetic-place"]));
        assert!(out.keyword_groups[0].is_singleton());
    }

    #[test]
    fn multi_keyword_intent_preserves_order() {
        let e = expander();
        let out = e.expand(intent_with(vec!["工作汇报", "synthetic-place", "slides"]));
        assert_eq!(out.keyword_groups.len(), 3);
        assert_eq!(out.keyword_groups[0].head, "工作汇报");
        assert!(out.keyword_groups[0].synonyms.len() == 2);
        assert!(out.keyword_groups[1].is_singleton());
        assert_eq!(out.keyword_groups[2].head, "slides");
        assert!(out.keyword_groups[2].synonyms.len() == 2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-harness synonym::yaml 2>&1 | head -40
```

预期：编译失败（`from_paths` 等新符号未定义）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness synonym::yaml
```

预期：lint 7 test + expand_tests 9 test 共 16 test pass。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(harness): YamlSynonymExpander 双向展开 + 语言判定(BETA-11 Task 4)"
```

---

## Task 5：运行期 cap（单 query 总扩展词数 ≤ 32）

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`

- [ ] **Step 1: Write the failing test**

在 `expand_tests` 模块追加：

```rust
#[test]
fn cap_truncates_tail_when_total_exceeds_32() {
    // 构造 6 组每组 8 alias 共 6 * 9 = 54 词,超 32 截断
    let mut yaml = String::from("version: 1\nlanguage: zh\ngroups:\n");
    for i in 0..6 {
        yaml.push_str(&format!("  - head: head-{i}\n    aliases: ["));
        let aliases: Vec<String> = (0..8).map(|j| format!("a-{i}-{j}")).collect();
        yaml.push_str(&aliases.join(", "));
        yaml.push_str("]\n");
    }
    // 这份 yaml 含 ASCII 字母,无法过 zh 语言 lint。仅 cap 单元测试可改用 from_str 直接喂构造好的 expander —— 用直接构造 mock。

    // 简化:跳过 yaml,直接断言 cap 行为通过 cap_keyword_groups helper。见下。
}

#[test]
fn cap_keyword_groups_truncates_synonyms_tail_by_total_budget() {
    use locifind_search_backend::KeywordGroup;
    let groups = vec![
        KeywordGroup {
            head: "h1".into(),
            synonyms: (0..20).map(|i| format!("s1-{i}")).collect(),
        },
        KeywordGroup {
            head: "h2".into(),
            synonyms: (0..20).map(|i| format!("s2-{i}")).collect(),
        },
    ];
    // 总词数 = 2 + 20 + 20 = 42,cap 32 → 截掉尾部 10 词
    let (capped, warn) = cap_keyword_groups(groups, 32);
    let total: usize = capped.iter().map(|g| 1 + g.synonyms.len()).sum();
    assert_eq!(total, 32);
    assert!(warn);
}

#[test]
fn cap_keyword_groups_under_budget_is_passthrough() {
    use locifind_search_backend::KeywordGroup;
    let groups = vec![KeywordGroup {
        head: "h1".into(),
        synonyms: vec!["s1".into(), "s2".into()],
    }];
    let (capped, warn) = cap_keyword_groups(groups, 32);
    assert_eq!(capped.len(), 1);
    assert_eq!(capped[0].synonyms.len(), 2);
    assert!(!warn);
}
```

> 第一个 test 仅占位说明，删除该 test，保留下面 2 个直接测 `cap_keyword_groups` 的 test。

在 `yaml.rs` `impl YamlSynonymExpander` 区域之上添加：

```rust
const RUNTIME_KEYWORD_CAP: usize = 32;

/// 截断扩展结果至 `cap` 词。
/// 返回 (截断后的 groups, 是否触发截断 warn)。
pub(crate) fn cap_keyword_groups(
    mut groups: Vec<KeywordGroup>,
    cap: usize,
) -> (Vec<KeywordGroup>, bool) {
    let total: usize = groups.iter().map(|g| 1 + g.synonyms.len()).sum();
    if total <= cap {
        return (groups, false);
    }
    let mut budget = cap;
    let mut new_groups = Vec::with_capacity(groups.len());
    for g in groups.drain(..) {
        if budget == 0 {
            break;
        }
        // head 必保
        let head = g.head;
        budget = budget.saturating_sub(1);
        let take = budget.min(g.synonyms.len());
        let synonyms: Vec<String> = g.synonyms.into_iter().take(take).collect();
        budget -= take;
        new_groups.push(KeywordGroup { head, synonyms });
    }
    (new_groups, true)
}
```

并在 `impl SynonymExpander for YamlSynonymExpander::expand` 内，将构造 `groups` 后改为：

```rust
let groups = match intent.search_keywords() {
    None => Vec::new(),
    Some(kws) => kws.iter().map(|kw| self.expand_one(kw)).collect::<Vec<_>>(),
};
let (groups, _warn_truncated) = cap_keyword_groups(groups, RUNTIME_KEYWORD_CAP);
// _warn_truncated 在 Task 8 接到 Tracer 后通过 SynonymExpandEvent 上报
ExpandedSearchIntent {
    base: intent,
    keyword_groups: groups,
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-harness synonym::yaml::expand_tests::cap 2>&1 | head -30
```

预期：编译失败（`cap_keyword_groups` 未定义）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness synonym::yaml
```

预期：lint 7 + expand 9 + cap 2 = 18 test pass。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(harness): 同义词运行期 cap 32 词 + 截断 warn(BETA-11 Task 5)"
```

---

## Task 6：harness lib.rs 模块导出

**Files:**
- Modify: `packages/harness/src/lib.rs`

- [ ] **Step 1: Write the failing test**

新增 `packages/harness/src/synonym/mod.rs` 外部使用测试（如已在 Task 2 临时添加 `pub mod synonym;`，本 task 是正式整理 + 加 re-export）。在 `lib.rs` 现有 `pub mod` 声明区域：

```rust
pub mod synonym;
pub use synonym::{ExpanderError, NoopExpander, SynonymExpander, YamlSynonymExpander};
```

`ExpandedSearchIntent` / `KeywordGroup` 已通过 `locifind_search_backend` re-export 链可见，harness 用户直接 `use locifind_search_backend::{ExpandedSearchIntent, KeywordGroup};`，本 plan 不在 harness 再 re-export 一次（避免类型重复路径）。

无新 test。直接做 verify。

- [ ] **Step 2: Run test to verify it fails**

跳过（仅模块导出，无新行为测试）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness
```

预期：全过；harness 全部既有 test + 新增 synonym 共 18 test。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/lib.rs
git commit -m "feat(harness): synonym 模块在 lib.rs 正式导出(BETA-11 Task 6)"
```

---

## Task 7：`SearchBackend::search_expanded` default method

**Files:**
- Modify: `packages/search-backends/common/src/lib.rs`

- [ ] **Step 1: Write the failing test**

在 common crate 现有 SearchBackend test 区域（或新建 `expanded_backend_tests` 模块）：

```rust
#[cfg(test)]
mod search_expanded_default_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::{ExpandedSearchIntent, KeywordGroup};

    // 仅依靠 default impl 的假后端,期望 search_expanded 退化为调 search(&base)
    #[derive(Debug, Default)]
    struct FakeBackend {
        called_with: std::sync::Mutex<Option<SearchIntent>>,
    }

    impl SearchBackend for FakeBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn is_available(&self) -> bool {
            true
        }
        fn search<'a>(
            &'a self,
            intent: &'a SearchIntent,
            cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            *self.called_with.lock().unwrap() = Some(intent.clone());
            Box::pin(async move { backend_stream_from_results(Vec::new(), cancel) })
        }
    }

    #[tokio::test]
    async fn default_search_expanded_falls_back_to_search() {
        let backend = FakeBackend::default();
        let intent = SearchIntent::FileSearch(FileSearchIntent {
            keywords: Some(vec!["工作汇报".into()]),
            ..FileSearchIntent::default()
        });
        let expanded = ExpandedSearchIntent {
            base: intent.clone(),
            keyword_groups: vec![KeywordGroup {
                head: "工作汇报".into(),
                synonyms: vec!["述职".into()],
            }],
        };
        let _stream = backend
            .search_expanded(&expanded, CancellationToken::new())
            .await;
        // 默认实现把 expanded.base 喂给 search()
        assert_eq!(
            backend.called_with.lock().unwrap().as_ref(),
            Some(&intent),
        );
    }
}
```

> 若 common crate 现在没有 `tokio` 作 dev-dep，按已有测试惯例处理（参考 `packages/search-backends/common/Cargo.toml [dev-dependencies]`）。

在 `pub trait SearchBackend` 内（紧邻 `fn search`）追加：

```rust
/// 接受同义词扩展后的搜索意图。默认实现 fallback 到 `search(&expanded.base)`，
/// 丢弃 group 信息。支持同义词的后端覆盖此方法。
fn search_expanded<'a>(
    &'a self,
    expanded: &'a crate::ExpandedSearchIntent,
    cancel: CancellationToken,
) -> BackendSearchFuture<'a> {
    self.search(&expanded.base, cancel)
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-search-backend search_expanded_default 2>&1 | head -30
```

预期：编译失败（`search_expanded` 未定义）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-search-backend --all-targets -- -D warnings
cargo test -p locifind-search-backend
```

预期：全过；新 1 test pass，既有 test 不破。

- [ ] **Step 5: Commit**

```bash
git add packages/search-backends/common/src/lib.rs
git commit -m "feat(common): SearchBackend 加 search_expanded default method(BETA-11 Task 7)"
```

---

## Task 8：Spotlight 覆盖 `search_expanded` + `keyword_predicate_expanded`

**Files:**
- Modify: `packages/search-backends/spotlight/src/lib.rs`

- [ ] **Step 1: Write the failing test**

定位 `packages/search-backends/spotlight/src/lib.rs:387 fn keyword_predicate(keyword: &str) -> String`。新增配套函数 `keyword_predicate_expanded(group: &KeywordGroup) -> String`。

在 spotlight crate test 区域追加：

```rust
#[test]
fn singleton_group_predicate_is_byte_equal_to_keyword_predicate() {
    use locifind_search_backend::KeywordGroup;
    let g = KeywordGroup::singleton("工作汇报");
    let expanded = keyword_predicate_expanded(&g);
    let original = keyword_predicate("工作汇报");
    assert_eq!(expanded, original);
}

#[test]
fn multi_group_predicate_or_joins_all_members_across_three_fields() {
    use locifind_search_backend::KeywordGroup;
    let g = KeywordGroup {
        head: "工作汇报".into(),
        synonyms: vec!["述职".into(), "年度总结".into()],
    };
    let pred = keyword_predicate_expanded(&g);
    // 3 个字段 × 3 个词 = 9 个 CONTAINS[cd] 项
    let count = pred.matches("CONTAINS[cd]").count();
    assert_eq!(count, 9);
    assert!(pred.contains("\"工作汇报\""));
    assert!(pred.contains("\"述职\""));
    assert!(pred.contains("\"年度总结\""));
}

#[test]
fn multi_group_predicate_escapes_injection() {
    use locifind_search_backend::KeywordGroup;
    let g = KeywordGroup {
        head: "x".into(),
        synonyms: vec!["a\" || (1==1) || \"".into()],
    };
    let pred = keyword_predicate_expanded(&g);
    // 沿用 escape_predicate_string,内嵌双引号被转义
    assert!(!pred.contains("|| (1==1) ||"));
}
```

实现 `keyword_predicate_expanded`（紧邻 `keyword_predicate`）：

```rust
fn keyword_predicate_expanded(group: &locifind_search_backend::KeywordGroup) -> String {
    // singleton 优化:与 keyword_predicate(head) byte-equal
    if group.is_singleton() {
        return keyword_predicate(&group.head);
    }
    let mut parts: Vec<String> = Vec::with_capacity(group.all().len() * 3);
    for w in group.all() {
        let escaped = escape_predicate_string(w);
        parts.push(format!("kMDItemDisplayName CONTAINS[cd] \"{escaped}\""));
        parts.push(format!("kMDItemTextContent CONTAINS[cd] \"{escaped}\""));
        parts.push(format!("kMDItemFSName CONTAINS[cd] \"{escaped}\""));
    }
    format!("({})", parts.join(" || "))
}
```

修改 `build_predicate` / `translate` 区域（具体函数名以仓内为准；从现读到的代码看 keyword 在 `constraints.keywords` 处通过 `builder.and(keyword_predicate(keyword))` 累加）：新增一条只供 `search_expanded` 调用的等价函数 `build_predicate_expanded(expanded: &ExpandedSearchIntent, ...) -> String`，区别是 keyword 段改用 `keyword_predicate_expanded(group)`。

实现 `impl SearchBackend for SpotlightBackend` 覆盖 `search_expanded`：

```rust
fn search_expanded<'a>(
    &'a self,
    expanded: &'a locifind_search_backend::ExpandedSearchIntent,
    cancel: locifind_search_backend::CancellationToken,
) -> locifind_search_backend::BackendSearchFuture<'a> {
    // 把扩展后的 keyword groups 翻成 OR 谓词;其它字段(time/size/location/file_type 等)沿用 base
    Box::pin(async move {
        match self.translate_expanded(expanded) {
            Ok(query) => {
                let output = run_mdfind(&self.mdfind_path, &query, self.timeout, &cancel)
                    .map_err(|e| /* 既有错误映射 */)?;
                /* 既有结果端 exclude/normalize 流程 */
            }
            Err(e) => /* 既有错误映射 */,
        }
    })
}
```

> 实施细节按 spotlight `search()` 现有结构镜像 — 将 `translate(intent)` 路径在 `translate_expanded` 复用，仅 keyword 段切换函数。本 task 实施者需先读完 `spotlight/src/lib.rs` 现有 `search` 实现（约 80-110 行）再决定是抽公共助手还是双轨实现。**推荐**：抽 `build_constraints_predicate_with_keyword<F>(constraints, keyword_fn: F)` 这种泛型，让 `translate` 和 `translate_expanded` 共用主体。

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-spotlight 2>&1 | head -40
```

预期：编译失败（`keyword_predicate_expanded` 未定义）+ Spotlight `SearchBackend` impl 缺 `search_expanded`（无 — 默认 impl 在 Task 7 提供，但本 task 要 override）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。完整覆盖 `search_expanded` 并加 fixture 端到端 test：

```rust
#[test]
fn fixture_end_to_end_expands_工作汇报_to_述职_then_matches() {
    // 见 PROTO-05A fixture infra;创建 sandbox 含 述职.ppt;构造
    // ExpandedSearchIntent { base: FileSearch{keywords:["工作汇报"]}, keyword_groups: [head="工作汇报", synonyms=["述职"]] }
    // 跑 spotlight backend.search_expanded(...).await
    // 验证结果含 述职.ppt
    //
    // 实施: 复用 spotlight 现有 fake-mdfind.sh 测试模式 + 桩输出
}
```

> fixture 端到端 test 若 mdfind 真机依赖过重，至少跑出 mdfind 调用的查询字符串验证含 "述职" + "工作汇报" 即可。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-spotlight --all-targets -- -D warnings
cargo test -p locifind-spotlight
```

预期：spotlight 既有 test + 新 3-4 test 全过。

- [ ] **Step 5: Commit**

```bash
git add packages/search-backends/spotlight/src/lib.rs
git commit -m "feat(spotlight): 覆盖 search_expanded + OR 谓词翻译(BETA-11 Task 8)"
```

---

## Task 9：Tracer `SynonymExpandEvent`

**Files:**
- Modify: `packages/harness/src/tracing.rs`

- [ ] **Step 1: Write the failing test**

在 `tracing.rs` 现有 test 模块追加：

```rust
#[derive(Default)]
struct MockSynonymHook {
    events: Arc<Mutex<Vec<SynonymExpandEvent>>>,
}

impl TracingHook for MockSynonymHook {
    fn on_tool_call(&self, _: &ToolCallEvent) {}
    fn on_tool_result(&self, _: &ToolResultEvent) {}
    fn on_error(&self, _: &ToolErrorEvent) {}
    fn on_synonym_expand(&self, event: &SynonymExpandEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[test]
fn tracer_dispatches_synonym_expand_to_hooks() {
    let mock = MockSynonymHook::default();
    let events = Arc::clone(&mock.events);
    let tracer = Tracer::with_hooks(vec![Box::new(mock)]);

    tracer.on_synonym_expand(&SynonymExpandEvent {
        head: "工作汇报".into(),
        group: vec!["工作汇报".into(), "述职".into()],
        source: "zh.yaml".into(),
        truncated: false,
    });
    let recorded = events.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].head, "工作汇报");
}

#[test]
fn noop_hook_skips_synonym_expand() {
    NoopHook.on_synonym_expand(&SynonymExpandEvent {
        head: "x".into(),
        group: vec!["x".into()],
        source: "zh.yaml".into(),
        truncated: false,
    });
    // 不 panic 即可
}

#[test]
fn json_lines_hook_writes_synonym_expand_event() {
    let buf: Vec<u8> = Vec::new();
    let hook = JsonLinesHook::new(buf);
    hook.on_synonym_expand(&SynonymExpandEvent {
        head: "工作汇报".into(),
        group: vec!["工作汇报".into(), "述职".into()],
        source: "zh.yaml".into(),
        truncated: false,
    });
    // 内部 buf 在 Arc<Mutex<W>> 里;暴露一个 helper 或借用 Drop trait 检查
    // 实施: 用 JsonLinesHook::into_writer() 或 #[cfg(test)] 提供 dump()
    // 简化: 验证不 panic 即可
}
```

修改 `tracing.rs`：

1. 加新 event 类型：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynonymExpandEvent {
    pub head: String,
    pub group: Vec<String>,  // 含 head 在 [0]
    pub source: String,      // "zh.yaml" / "en.yaml" / "noop"
    pub truncated: bool,     // 运行期 cap 是否触发
}
```

2. `TracingHook` trait 加默认空实现的方法：

```rust
pub trait TracingHook: Send + Sync {
    fn on_tool_call(&self, event: &ToolCallEvent);
    fn on_tool_result(&self, event: &ToolResultEvent);
    fn on_error(&self, event: &ToolErrorEvent);
    /// 同义词扩展事件。默认空实现，老 hook 零修改。
    fn on_synonym_expand(&self, _event: &SynonymExpandEvent) {}
}
```

3. `Tracer` 加分发方法：

```rust
impl Tracer {
    pub fn on_synonym_expand(&self, event: &SynonymExpandEvent) {
        for hook in &self.hooks {
            hook.on_synonym_expand(event);
        }
    }
}
```

4. `NoopHook` 显式 impl `on_synonym_expand` 空（一致性）。

5. `JsonLinesHook<W>` impl `on_synonym_expand`：

```rust
fn on_synonym_expand(&self, event: &SynonymExpandEvent) {
    self.log("synonym_expand", event);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-harness tracing 2>&1 | head -30
```

预期：编译失败（`SynonymExpandEvent` 未定义、`on_synonym_expand` 缺）。

- [ ] **Step 3: Write minimal implementation**

合并到 Step 1。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness tracing
```

预期：既有 2 test + 新 3 test 全过。

- [ ] **Step 5: Commit**

```bash
git add packages/harness/src/tracing.rs
git commit -m "feat(harness): Tracer 加 SynonymExpandEvent(BETA-11 Task 9)"
```

---

## Task 10：词典初版 zh.yaml + en.yaml + tauri.conf.json bundle

**Files:**
- Create: `resources/synonyms/zh.yaml`
- Create: `resources/synonyms/en.yaml`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`

- [ ] **Step 1: 写词典 zh.yaml**

新文件 `resources/synonyms/zh.yaml`，按 spec §5.1 5 大桶填 ~60 组。以下为骨架，实施者完成全部条目（每组对应至少 1 个真实想得到的 demo query 才纳入）：

```yaml
version: 1
language: zh

groups:
  # —— office 汇报（~12 组）——
  - head: 工作汇报
    aliases: [述职, 年度总结, 季度汇报, 月度汇报]
    domain: office
  - head: 周报
    aliases: [周总结]
    domain: office
  - head: 简历
    aliases: [个人简历]
    domain: office
  - head: 总结
    aliases: [复盘]
    domain: office
  # ... 其余 8 组,实施时补全

  # —— 文件类型(~8)——
  - head: 幻灯片
    aliases: [演示文稿]
    domain: file_type
  # ...

  # —— media(~8)——
  - head: 截图
    aliases: [截屏, 屏幕截图]
    domain: media
  - head: 照片
    aliases: [相片, 图片]
    domain: media
  - head: 视频
    aliases: [影片, 录像]
    domain: media
  # ...

  # —— 文档管理(~5)——
  - head: 合同
    aliases: [协议]
    domain: document
  - head: 发票
    aliases: [票据]
    domain: document
  # ...

  # —— 个人/家庭(~5)——
  - head: 笔记
    aliases: [札记]
    domain: personal
  - head: 设计稿
    aliases: [设计文件]
    domain: design
  # ...
```

要求：实施者需补全到 ~60 组，每组 PR commit message 列「demo query 用例」。

- [ ] **Step 2: 写词典 en.yaml**

新文件 `resources/synonyms/en.yaml`，~40 组：

```yaml
version: 1
language: en

groups:
  # —— file type(~8)——
  - head: slides
    aliases: [slideshow, presentation]
    domain: file_type
  - head: spreadsheet
    aliases: [excel sheet]
    domain: file_type
  - head: document
    aliases: [doc]
    domain: file_type

  # —— media(~8)——
  - head: screenshot
    aliases: [screen capture, screencap]
    domain: media
  - head: photo
    aliases: [picture, pic]
    domain: media
  - head: video
    aliases: [movie, clip]
    domain: media

  # —— document(~5)——
  - head: contract
    aliases: [agreement]
    domain: document
  - head: invoice
    aliases: [receipt]
    domain: document
  - head: resume
    aliases: [cv]
    domain: document

  # —— personal(~4)——
  - head: note
    aliases: [memo]
    domain: personal
  - head: mockup
    aliases: [wireframe]
    domain: design
  # ...
```

补全到 ~40 组。

- [ ] **Step 3: 修改 tauri.conf.json bundle.resources**

`apps/desktop/src-tauri/tauri.conf.json`，将现有 `"bundle"` 节扩为：

```json
"bundle": {
  "active": true,
  "targets": ["app"],
  "icon": [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/128x128@2x.png",
    "icons/icon.icns",
    "icons/icon.ico"
  ],
  "resources": {
    "../../resources/synonyms/zh.yaml": "synonyms/zh.yaml",
    "../../resources/synonyms/en.yaml": "synonyms/en.yaml"
  }
}
```

> Tauri 2 `bundle.resources` 支持 map 形态（源:目标）。路径相对 `src-tauri/` 目录。Apply 到 .app 后位于 `Contents/Resources/synonyms/{zh,en}.yaml`，运行期 `AppHandle::path().resource_dir()` + `synonyms/zh.yaml` 解析。

- [ ] **Step 4: 验证两份词典能被 lint 通过**

新建一个临时单测（或现场跑），喂仓内真实词典文件：

```bash
cargo test -p locifind-harness -- --nocapture <<EOF
# 或者写一个 #[ignore] 标记的集成 test,手动 cargo test -- --ignored
EOF
```

替代方案——加一个集成 test `packages/harness/tests/synonym_dict.rs`：

```rust
#[test]
fn shipped_dicts_pass_lint() {
    use locifind_harness::YamlSynonymExpander;
    use std::path::PathBuf;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf();
    let zh = root.join("resources/synonyms/zh.yaml");
    let en = root.join("resources/synonyms/en.yaml");
    let _ = YamlSynonymExpander::from_paths(&zh, &en).expect("仓内词典必须 lint pass");
}
```

```bash
cargo fmt --check
cargo clippy -p locifind-harness --all-targets -- -D warnings
cargo test -p locifind-harness shipped_dicts_pass_lint
```

预期：lint pass。

- [ ] **Step 5: Commit**

```bash
git add resources/synonyms/zh.yaml \
        resources/synonyms/en.yaml \
        apps/desktop/src-tauri/tauri.conf.json \
        packages/harness/tests/synonym_dict.rs
git commit -m "feat(resources): 同义词词典 zh ~60 组 + en ~40 组 + tauri bundle(BETA-11 Task 10)"
```

---

## Task 11：desktop `SearchDeps` 加 `synonym_expander` 字段 + main.rs 启动加载

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing test**

参考第 33 阶段 `SearchDeps::new(...)` 签名扩展。`search.rs` test 区域加：

```rust
#[test]
fn search_deps_holds_synonym_expander() {
    use locifind_harness::{NoopExpander, SynonymExpander};
    use std::sync::Arc;
    let deps = SearchDeps::new(
        /* ... 既有 6 个 Arc ... */
        Arc::new(NoopExpander) as Arc<dyn SynonymExpander>,
    );
    // 取得 expander 引用,验证非空
    let _ = deps.synonym_expander();
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-desktop search_deps_holds_synonym_expander 2>&1 | head -30
```

预期：编译失败（`SearchDeps::new` 参数不匹配 / `synonym_expander()` 未定义）。

- [ ] **Step 3: Write minimal implementation**

修改 `search.rs::SearchDeps`：

```rust
pub struct SearchDeps {
    // ... 既有字段 ...
    synonym_expander: Arc<dyn SynonymExpander>,
}

impl SearchDeps {
    pub fn new(
        // ... 既有 6 参数 ...
        synonym_expander: Arc<dyn SynonymExpander>,
    ) -> Self {
        Self {
            // ... 既有字段映射 ...
            synonym_expander,
        }
    }

    pub(crate) fn synonym_expander(&self) -> &Arc<dyn SynonymExpander> {
        &self.synonym_expander
    }
}
```

同步更新 **search.rs 中所有现有 `SearchDeps::new(...)` 调用点**（含约 12 个 test 内构造，见第 33 阶段 `grep` 结果），全部追加 `Arc::new(NoopExpander)` 作末位参数。

修改 `apps/desktop/src-tauri/src/main.rs`：

```rust
// 1. 新增引入
use locifind_harness::{NoopExpander, SynonymExpander, YamlSynonymExpander};

// 2. 抽函数(放 main.rs 末尾或 synonym_resources.rs 模块)
fn build_synonym_expander(app: &tauri::AppHandle) -> Arc<dyn SynonymExpander> {
    let (zh, en) = resolve_synonym_paths(app);
    match YamlSynonymExpander::from_paths(&zh, &en) {
        Ok(e) => Arc::new(e),
        Err(err) => {
            eprintln!("synonym: 词典加载失败,退到 noop: {err}");
            Arc::new(NoopExpander)
        }
    }
}

fn resolve_synonym_paths(app: &tauri::AppHandle) -> (PathBuf, PathBuf) {
    // 优先 Tauri resource_dir(.app 打包态);失败 fallback workspace 根(dev 模式)
    if let Ok(resource_dir) = app.path().resource_dir() {
        let zh = resource_dir.join("synonyms/zh.yaml");
        let en = resource_dir.join("synonyms/en.yaml");
        if zh.exists() && en.exists() {
            return (zh, en);
        }
    }
    // dev fallback: workspace 根 resources/
    let workspace_root = std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            // 从 src-tauri/ 起向上找 workspace 根(含 Cargo.toml 顶层)
            std::iter::successors(Some(cwd), |p| p.parent().map(Path::to_path_buf))
                .find(|p| p.join("Cargo.toml").exists() && p.join("packages").exists())
        })
        .unwrap_or_else(|| PathBuf::from("."));
    (
        workspace_root.join("resources/synonyms/zh.yaml"),
        workspace_root.join("resources/synonyms/en.yaml"),
    )
}

// 3. 在 setup hook 内调用并装进 SearchDeps:
//    let expander = build_synonym_expander(&app.handle());
//    let deps = SearchDeps::new(..., expander);
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-desktop --all-targets -- -D warnings
cargo test -p locifind-desktop
```

预期：所有既有 desktop test（46 个）+ 新 1 test 全过。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(desktop): SearchDeps 加 synonym_expander + main.rs 加载词典(BETA-11 Task 11)"
```

---

## Task 12：desktop `search_impl` 接入 `expand()` + Tracer event

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: Write the failing test**

在 search.rs test 区域加：

```rust
#[tokio::test]
async fn search_impl_expands_intent_before_backend_call() {
    // 用一份测试 zh.yaml(in-memory)+ FakeCapturingBackend,
    // 验证 backend.search_expanded 收到的 keyword_groups 含同义词
    //
    // 实施: 复用第 30/31 阶段的 FakeCapturingBackend(若存在);
    // 否则简化为断言 expanded.keyword_groups.len() == 1 且 synonyms 非空
}

#[tokio::test]
async fn search_impl_emits_synonym_expand_trace_event() {
    // LOCIFIND_TRACE=/tmp/xxx 或者注入 MockSynonymHook
    // 跑 search_impl("找一份工作汇报的ppt"),验证 hook.events 至少 1 条
}

#[tokio::test]
async fn search_impl_noop_expander_emits_no_synonym_event() {
    // 注入 NoopExpander,验证 hook.events == 0
}
```

> 实施者按仓内 fake backend / mock hook 工厂用法选择。若需要新建测试 fake，参考 search.rs 现有 `FakeCapturingBackend` 模式。

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p locifind-desktop search_impl_expands 2>&1 | head -30
```

预期：编译失败 / 行为不符（`search_impl` 仍直接调 `backend.search`）。

- [ ] **Step 3: Write minimal implementation**

修改 `search_impl`（约 search.rs:139 起）。定位 backend 调用点（route 选好后），改为：

```rust
// 之前: 直接 backend.search(&effective, cancel.clone())
// 之后:
let expanded = deps.synonym_expander().expand(effective.clone());

// 发 trace event(对每个非 singleton group)
if !expanded.is_identity() {
    let source = guess_source(&expanded);  // 简化:zh.yaml/en.yaml 二选,见 SynonymExpandEvent 注释
    for group in &expanded.keyword_groups {
        if !group.is_singleton() {
            deps.tracer().on_synonym_expand(&SynonymExpandEvent {
                head: group.head.clone(),
                group: group.all().into_iter().map(String::from).collect(),
                source: source.clone(),
                truncated: false,  // 当前实现 cap 在 yaml.rs::expand() 内吞掉 truncated,留 BETA-11 升级
            });
        }
    }
}

let stream = backend.search_expanded(&expanded, cancel.clone()).await;
```

> trace event 的 `truncated` 当前留 false 是简化（Task 5 cap_keyword_groups 已返回 bool 但 `expand()` 吞掉了）。BETA-11 收尾若有时间，在 `YamlSynonymExpander` 内挂一个 last_truncated 标记 / 改 `SynonymExpander::expand` 返回 `(ExpandedSearchIntent, ExpansionStats)`。本 plan 不强制做。

> `guess_source` 简化：检测 group 中是否含 CJK，是则 "zh.yaml" 否则 "en.yaml"。

- [ ] **Step 4: Run test to verify it passes**

```bash
cargo fmt --check
cargo clippy -p locifind-desktop --all-targets -- -D warnings
cargo test -p locifind-desktop
```

预期：既有 46 + 新 3 test 全过。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "feat(desktop): search_impl 接入 expander + 发 synonym_expand trace(BETA-11 Task 12)"
```

---

## Task 13：docs/manual-test-scenarios.md BETA-11 节

**Files:**
- Modify: `docs/manual-test-scenarios.md`

- [ ] **Step 1: 追加 BETA-11 节**

在 `docs/manual-test-scenarios.md` 末尾追加：

````markdown
## BETA-11：同义词关键词扩展

> 关联：[spec](./superpowers/specs/2026-05-30-synonym-keyword-expansion-design.md) / [plan](./superpowers/plans/2026-05-30-synonym-keyword-expansion.md)

### 准备 fixture

在 `~/Desktop/locifind-beta11-fixtures/` 下创建：

- `述职.ppt`
- `屏幕截图 2024-01-01.png`
- `foo.pptx`
- `购房协议.pdf`
- `bar.jpg`
- `synthetic-place-note.md`

执行 `mdimport ~/Desktop/locifind-beta11-fixtures/` 让 Spotlight 索引。

### Scenario 清单

| # | query | 期望 | 验证什么 |
|---|---|---|---|
| 1 | `找一份工作汇报相关的ppt` | 命中 `述职.ppt` | zh office 桶（用户原 case）|
| 2 | `找最近的截图` | 命中 `屏幕截图 2024-01-01.png` | zh media 桶 |
| 3 | `find a slideshow` | 命中 `foo.pptx` | en file_type 桶 |
| 4 | `找合同` | 命中 `购房协议.pdf` | zh document 桶 |
| 5 | `find a photo` | 命中 `bar.jpg` | en media 桶 |
| 6 | `找 synthetic-place 的笔记` | 命中精确（不扩到无关项）| hyphenated 标识符不被误扩 |
| 7 | `LOCIFIND_TRACE=/tmp/beta11.jsonl npm run tauri dev` + 跑 #1 | `/tmp/beta11.jsonl` 含 `"tag":"synonym_expand"` 一行 | 可解释 |
| 8 | 移除 `Contents/Resources/synonyms/zh.yaml`（或停 dev 改名再起）+ 跑 #1 | #1 退化为不命中 | NoopExpander fallback 路径 |
````

- [ ] **Step 2: Commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "docs(beta-11): 同义词扩展手测 scenario 8 case(BETA-11 Task 13)"
```

---

## Task 14：收尾验证（ci.sh + evals byte-equal）

**Files:** none（仅验证）

- [ ] **Step 1: 跑 ci.sh 全套**

```bash
bash scripts/ci.sh
```

预期：全过。

- [ ] **Step 2: 跑 parser-only evals 确认 byte-equal**

```bash
cargo run -p locifind-evals --bin evals 2>&1 | tail -20
```

预期：`pass 472 / partial 26 / fail 2`，variant 命中 99.6%。

- [ ] **Step 3: 跑 hybrid evals 确认不掉**

```bash
cargo run -p locifind-evals --bin evals -- --with-fallback --hybrid 2>&1 | tail -20
```

预期：`pass 480 / partial 18 / fail 2`，rescued_to_pass 8/9。

- [ ] **Step 4: 真机手测（按 Task 13 scenario 清单走一遍）**

代理无法点 Tauri 窗口；交用户驱动。

`LOCIFIND_TRACE=/tmp/beta11.jsonl npm run tauri dev`，跑 8 case。结果记到 STATUS.md 收工日志。

- [ ] **Step 5: 收工 commit（仅 STATUS / ROADMAP 更新）**

按 CONVENTIONS §3 收工流程：
1. STATUS.md 顶部追加本会话日志 + 当前 task 改为「BETA-11 done」
2. ROADMAP.md 新增 BETA-11 / BETA-11A / BETA-11B / BETA-11C 4 条 task
3. 单次中文 commit 落库

---

## 出场标准（对应 spec §7）

- [ ] `bash scripts/ci.sh` 全过
- [ ] `cargo run -p locifind-evals` parser-only **pass 472 / partial 26 / fail 2 byte-equal**
- [ ] `cargo run -p locifind-evals --with-fallback --hybrid` **pass 480 不掉**
- [ ] 手测 scenario 1-8 全过（scenario 1 为用户原 case，必过）
- [ ] trace JSONL 包含 `synonym_expand` event（scenario 7）
- [ ] 词典缺失时 NoopExpander 退化路径生效（scenario 8）

---

## 修订记录

| 日期 | 修订 |
|---|---|
| 2026-05-30 | v0.1：初稿（Claude Code Opus 4.7，writing-plans 流程） |
