# BETA-11D 用户级持久化同义词库 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户教学自己的同义词映射，本地 YAML 持久化、即时生效、参与搜索召回扩展，并在零结果时引导教学。

**Architecture:** harness 新增 `UserIndex`（YAML 模型 + lint + 可变 CRUD）与 `LayeredSynonymExpander`（用户层覆盖系统层），现有 `expand()` 算法重构到「词典视图」抽象上以零行为变更复用（evals byte-equal 守门）。desktop 新增 `user_synonyms.rs`（共享 `Arc<RwLock<UserIndex>>` + Tauri 命令）+ 零命中触发 UX。

**Tech Stack:** Rust（harness / Tauri 后端，serde_yaml 已是 harness 依赖）、React/TypeScript（前端）、YAML 持久化（`app_config_dir/user-synonyms.yaml`）。

**设计文档：** [docs/superpowers/specs/2026-06-13-beta-11d-user-synonym-dictionary-design.md](../specs/2026-06-13-beta-11d-user-synonym-dictionary-design.md)

---

## 文件结构

**harness（`packages/harness/src/synonym/`）**
- Create `user.rs` — `UserGroup` / `UserIndex`（YAML 解析 + lint + CRUD + DictView impl）、`UserDictError`。
- Modify `yaml.rs` — 抽出 `DictView` trait + 把 `expand_one`/`gazetteer_lookup_multi`/`apply_multiword_override`/`expand` 主体参数化为自由函数；`YamlSynonymExpander` 实现 `DictView` 并委托。新增 `LayeredSynonymExpander`。
- Modify `mod.rs` / `lib.rs` — 导出 `UserGroup` / `UserIndex` / `UserDictError` / `LayeredSynonymExpander`。

**desktop（`apps/desktop/src-tauri/src/`）**
- Create `user_synonyms.rs` — 共享状态 `UserSynonymState`（`Arc<RwLock<UserIndex>>` + 路径）、文件 IO、6 个 Tauri 命令。
- Modify `main.rs` — 启动加载用户词典、构造 `LayeredSynonymExpander`、manage `UserSynonymState`、注册命令。
- Modify `search.rs` — `search_impl` 加可选 `adhoc` 参数（注入一次性 OR 组）；新命令 `search_with_adhoc_synonyms`。
- Modify `privacy.rs` — `PrivacyOverview` 加 `user_synonym_count` + 数据位置一行。

**前端（`apps/desktop/src/`）**
- Create `UserSynonymsPage.tsx`（或并入 `SettingsPage`）— 「我的同义词」管理 UI。
- Modify `SearchView.tsx` — 零命中触发 UX（提示 → 手输 → adhoc 重查 → 记住确认）。
- Modify `PrivacyPage.tsx` — 显示用户同义词库位置 + 组数。
- Modify `styles.css` — 复用既有 CSS 变量。

**文档**
- Modify `ROADMAP.md`（BETA-11D 状态 + BETA-12 checklist 补一条）、`STATUS.md`（收工时）、`docs/manual-test-scenarios.md`（BETA-11D 场景）、隐私文案 doc。

---

## Task 1: harness — `UserGroup` / `UserIndex` 模型 + YAML 解析 + lint

**Files:**
- Create: `packages/harness/src/synonym/user.rs`
- Modify: `packages/harness/src/synonym/mod.rs`

- [ ] **Step 1: 写失败测试**

在 `packages/harness/src/synonym/user.rs` 写：

```rust
//! BETA-11D 用户级持久化同义词词典：YAML 模型 + lint + 运行时可变 CRUD。
//!
//! 与系统词典（[`crate::synonym::yaml`]）的**唯一 schema 差异**：用户词典**允许组内跨语言 alias**
//! （目标 case「友商竞争分析 → AWS / Azure / 产品分析」需要），其余 lint 规则全部沿用系统层原语。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::synonym::yaml::{classify, KeywordLang, MAX_ALIASES_PER_GROUP};

const SUPPORTED_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UserDictError {
    #[error("解析用户词典 YAML 失败: {0}")]
    Parse(String),
    #[error("用户词典 version={got}，仅支持 version=1")]
    UnsupportedVersion { got: u32 },
    #[error("组 head={head:?}: aliases 数量 {n} 超过上限 {MAX_ALIASES_PER_GROUP}")]
    TooManyAliases { head: String, n: usize },
    #[error("组 head={head:?} 内出现重复词 {dup:?}")]
    DuplicateWithinGroup { head: String, dup: String },
    #[error("词 {word:?} 在多个组中作为 head 或 alias 重复出现")]
    DuplicateAcrossGroups { word: String },
    #[error("head 不能为空")]
    EmptyHead,
    #[error("不允许标识符样的词 {word:?}（含连字符/下划线/点等分隔符）")]
    IdentifierLike { word: String },
}

/// 用户词典单组（YAML 序列化形态）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserGroup {
    pub head: String,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawUserDict {
    version: u32,
    #[serde(default)]
    groups: Vec<UserGroup>,
}

#[derive(Serialize)]
struct OutUserDict<'a> {
    version: u32,
    groups: &'a [UserGroup],
}

/// 运行时可变的用户词典：`groups` 为权威态，索引每次变更后重建。
#[derive(Debug, Default, Clone)]
pub struct UserIndex {
    groups: Vec<UserGroup>,
    zh_index: HashMap<String, Arc<[String]>>,
    en_index: HashMap<String, Arc<[String]>>,
}

impl UserIndex {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// 从 YAML 文本解析 + 全量 lint。任一组非法则整体拒绝。
    pub fn from_yaml_str(yaml: &str) -> Result<Self, UserDictError> {
        let raw: RawUserDict =
            serde_yaml::from_str(yaml).map_err(|e| UserDictError::Parse(e.to_string()))?;
        if raw.version != SUPPORTED_VERSION {
            return Err(UserDictError::UnsupportedVersion { got: raw.version });
        }
        Self::from_groups(raw.groups)
    }

    /// 从组列表构造（lint + 建索引）。
    pub fn from_groups(groups: Vec<UserGroup>) -> Result<Self, UserDictError> {
        lint_groups(&groups)?;
        let (zh_index, en_index) = build_user_indices(&groups);
        Ok(Self {
            groups,
            zh_index,
            en_index,
        })
    }

    #[must_use]
    pub fn groups(&self) -> &[UserGroup] {
        &self.groups
    }

    /// 序列化为 YAML 文本（持久化用）。
    #[must_use]
    pub fn to_yaml_str(&self) -> String {
        serde_yaml::to_string(&OutUserDict {
            version: SUPPORTED_VERSION,
            groups: &self.groups,
        })
        .unwrap_or_else(|_| "version: 1\ngroups: []\n".to_owned())
    }
}

/// lint 一组用户词条（污染防护，复用系统层原语）。
fn lint_groups(groups: &[UserGroup]) -> Result<(), UserDictError> {
    let mut seen_words: HashSet<&str> = HashSet::new();
    for g in groups {
        if g.head.trim().is_empty() {
            return Err(UserDictError::EmptyHead);
        }
        if g.aliases.len() > MAX_ALIASES_PER_GROUP {
            return Err(UserDictError::TooManyAliases {
                head: g.head.clone(),
                n: g.aliases.len(),
            });
        }
        let members: Vec<&str> = std::iter::once(g.head.as_str())
            .chain(g.aliases.iter().map(String::as_str))
            .collect();
        let mut intra: HashSet<&str> = HashSet::new();
        for w in &members {
            if is_identifier_like(w) {
                return Err(UserDictError::IdentifierLike { word: (*w).into() });
            }
            if !intra.insert(*w) {
                return Err(UserDictError::DuplicateWithinGroup {
                    head: g.head.clone(),
                    dup: (*w).into(),
                });
            }
        }
        for w in &members {
            if !seen_words.insert(*w) {
                return Err(UserDictError::DuplicateAcrossGroups { word: (*w).into() });
            }
        }
    }
    Ok(())
}

/// 标识符样：含连字符 / 下划线 / 点（如 `synthetic-place`）。对齐 BETA-11 §4.2 NoopExpand 规则。
fn is_identifier_like(word: &str) -> bool {
    word.chars().any(|c| matches!(c, '-' | '_' | '.'))
}

/// 按各成员语言建立双向索引（与系统层 `build_index` 同语义，但允许组内跨语言成员）。
fn build_user_indices(
    groups: &[UserGroup],
) -> (HashMap<String, Arc<[String]>>, HashMap<String, Arc<[String]>>) {
    let mut zh = HashMap::new();
    let mut en = HashMap::new();
    for g in groups {
        let members: Arc<[String]> = std::iter::once(g.head.clone())
            .chain(g.aliases.iter().cloned())
            .collect::<Vec<String>>()
            .into();
        for w in members.iter() {
            match classify(w) {
                KeywordLang::Zh => {
                    zh.insert(w.clone(), Arc::clone(&members));
                }
                KeywordLang::En => {
                    en.insert(w.clone(), Arc::clone(&members));
                }
                KeywordLang::Skip => {}
            }
        }
    }
    (zh, en)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    #[test]
    fn parses_cross_language_group() {
        let yaml = "version: 1\ngroups:\n  - head: 友商竞争分析\n    aliases: [AWS, Azure, 产品分析]\n";
        let idx = UserIndex::from_yaml_str(yaml).unwrap();
        assert_eq!(idx.groups().len(), 1);
        assert_eq!(idx.groups()[0].head, "友商竞争分析");
        // 中文 head 入 zh_index，英文 alias 入 en_index，均指向同组。
        assert!(idx.zh_index.contains_key("友商竞争分析"));
        assert!(idx.en_index.contains_key("AWS"));
    }

    #[test]
    fn rejects_too_many_aliases() {
        let yaml = "version: 1\ngroups:\n  - head: h\n    aliases: [a1,a2,a3,a4,a5,a6,a7,a8,a9]\n";
        assert_eq!(
            UserIndex::from_yaml_str(yaml),
            Err(UserDictError::TooManyAliases {
                head: "h".into(),
                n: 9
            })
        );
    }

    #[test]
    fn rejects_identifier_like() {
        let yaml = "version: 1\ngroups:\n  - head: 报告\n    aliases: [synthetic-place]\n";
        assert_eq!(
            UserIndex::from_yaml_str(yaml),
            Err(UserDictError::IdentifierLike {
                word: "synthetic-place".into()
            })
        );
    }

    #[test]
    fn rejects_duplicate_across_groups() {
        let yaml = "version: 1\ngroups:\n  - head: 报告\n    aliases: [汇报]\n  - head: 文档\n    aliases: [汇报]\n";
        assert_eq!(
            UserIndex::from_yaml_str(yaml),
            Err(UserDictError::DuplicateAcrossGroups { word: "汇报".into() })
        );
    }

    #[test]
    fn yaml_roundtrip() {
        let yaml = "version: 1\ngroups:\n  - head: 报告\n    aliases: [汇报, report]\n";
        let idx = UserIndex::from_yaml_str(yaml).unwrap();
        let out = idx.to_yaml_str();
        let reparsed = UserIndex::from_yaml_str(&out).unwrap();
        assert_eq!(reparsed.groups(), idx.groups());
    }
}
```

在 `packages/harness/src/synonym/mod.rs` 末尾加：

```rust
pub mod user;
pub use user::{UserDictError, UserGroup, UserIndex};
```

注：本任务依赖 `yaml.rs` 暴露 `classify` / `KeywordLang` / `MAX_ALIASES_PER_GROUP` 为 `pub(crate)`——在 Task 2 一并放开（Task 2 重构 yaml.rs 时改可见性）。**为让 Task 1 可独立编译，先在 yaml.rs 把这三项改为 `pub(crate)`**（见下 Step 3）。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-harness synonym::user 2>&1 | tail -20`
Expected: 编译失败（`classify` / `KeywordLang` / `MAX_ALIASES_PER_GROUP` 不可见）。

- [ ] **Step 3: 放开 yaml.rs 三项可见性**

在 `packages/harness/src/synonym/yaml.rs`：
- `const MAX_ALIASES_PER_GROUP: usize = 8;` → `pub(crate) const MAX_ALIASES_PER_GROUP: usize = 8;`
- `enum KeywordLang {` → `pub(crate) enum KeywordLang {`
- `fn classify(keyword: &str) -> KeywordLang {` → `pub(crate) fn classify(keyword: &str) -> KeywordLang {`

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-harness synonym::user 2>&1 | tail -20`
Expected: 5 个测试 PASS。

- [ ] **Step 5: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5
git add packages/harness/src/synonym/user.rs packages/harness/src/synonym/mod.rs packages/harness/src/synonym/yaml.rs
git commit -m "feat(beta-11d): 用户词典 UserIndex 模型 + YAML 解析 + lint（允许跨语言 alias）"
```

---

## Task 2: harness — 重构 expand 到 `DictView` 抽象（零行为变更）

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`

**目标：** 把 `YamlSynonymExpander::expand` 的算法抽到自由函数 + `DictView` trait，使 Task 3 的 `LayeredSynonymExpander` 复用同一份逻辑。**`YamlSynonymExpander` 输出必须逐字节不变**——由既有单测 + Task 12 的 evals byte-equal 守门。

- [ ] **Step 1: 加 `DictView` trait + 把方法改为视图自由函数**

在 `yaml.rs` 加 trait（放在 `YamlSynonymExpander` 定义附近）：

```rust
/// 扩展算法所需的词典只读视图。`YamlSynonymExpander`（系统层）与 `LayeredSynonymExpander`
/// （双层）各自实现，使一套扩展算法复用于两种词典形态。
pub(crate) trait DictView {
    /// 按语言查 keyword 所在组的全体成员（含 head）。
    fn lookup(&self, lang: KeywordLang, keyword: &str) -> Option<Arc<[String]>>;
    /// 全部索引键（zh + en），供 gazetteer 扫描。顺序不影响结果（下游确定性排序）。
    fn all_keys(&self) -> Vec<String>;
    /// 含空格的多词键，供多词覆盖。
    fn multiword_keys(&self) -> Vec<String>;
}
```

把现有 `expand_one` / `gazetteer_lookup_multi` / `apply_multiword_override` 从 `impl YamlSynonymExpander` 移出为**自由函数**，签名首参改为 `view: &dyn DictView`。逐函数改写（逻辑完全不变，仅把 `self.lookup`/`self.zh_index.keys()...` 换成 view 调用）：

```rust
fn expand_one(view: &dyn DictView, keyword: &str) -> KeywordGroup {
    let lang = classify(keyword);
    match view.lookup(lang, keyword) {
        None => KeywordGroup::singleton(keyword),
        Some(group_members) => {
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

fn gazetteer_lookup_multi(view: &dyn DictView, query: &str) -> Vec<String> {
    let mut cands: Vec<(usize, usize, usize, String)> = Vec::new();
    for key in view.all_keys() {
        let Some(pos) = query.find(key.as_str()) else {
            continue;
        };
        if !is_pure_content_term(&key) {
            continue;
        }
        cands.push((key.chars().count(), pos, pos + key.len(), key));
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut chosen: Vec<(usize, usize, String)> = Vec::new();
    for (_, start, end, key) in cands {
        let overlaps = chosen.iter().any(|(s, e, _)| start < *e && *s < end);
        if !overlaps {
            chosen.push((start, end, key));
        }
    }
    chosen.sort_by_key(|(s, _, _)| *s);
    chosen.into_iter().map(|(_, _, k)| k).collect()
}

fn apply_multiword_override(view: &dyn DictView, query: &str, groups: &mut [KeywordGroup]) {
    let lower = query.to_lowercase();
    let multiword: Vec<String> = view.multiword_keys();
    for slot in groups.iter_mut() {
        let kw = slot.head.to_lowercase();
        let mut best: Option<(usize, usize, String)> = None;
        for key in &multiword {
            let key_l = key.to_lowercase();
            if !key_l.split_ascii_whitespace().any(|w| w == kw.as_str()) {
                continue;
            }
            let Some(pos) = lower.find(&*key_l) else {
                continue;
            };
            if !is_pure_content_term(key) {
                continue;
            }
            let len = key.chars().count();
            let better = match best {
                None => true,
                Some((blen, bpos, _)) => len > blen || (len == blen && pos < bpos),
            };
            if better {
                best = Some((len, pos, key.clone()));
            }
        }
        if let Some((_, _, key)) = best {
            *slot = expand_one(view, &key);
        }
    }
}
```

加共享的顶层算法：

```rust
/// 双视图共享的扩展算法主体（原 `YamlSynonymExpander::expand` 逻辑，逐字节等价）。
pub(crate) fn expand_with_view(
    view: &dyn DictView,
    intent: SearchIntent,
    query: &str,
) -> ExpandedSearchIntent {
    let gaz = gazetteer_lookup_multi(view, query);
    let groups = if let Some(merged) =
        merge_or_group(gaz.iter().map(|k| expand_one(view, k)).collect())
    {
        vec![merged]
    } else {
        match intent.search_keywords() {
            Some(kws) if !kws.is_empty() => {
                let mut gs = kws.iter().map(|kw| expand_one(view, kw)).collect::<Vec<_>>();
                apply_multiword_override(view, query, &mut gs);
                dedup_groups_by_head(&mut gs);
                gs
            }
            _ => bare_content_keyword(query)
                .map(|matched| vec![expand_one(view, &matched)])
                .unwrap_or_default(),
        }
    };
    let (groups, _warn_truncated) = cap_keyword_groups(groups, RUNTIME_KEYWORD_CAP);
    ExpandedSearchIntent {
        base: intent,
        keyword_groups: groups,
    }
}
```

- [ ] **Step 2: `YamlSynonymExpander` 实现 `DictView` 并委托**

删除 `YamlSynonymExpander` 内已移出的 `expand_one`/`gazetteer_lookup_multi`/`apply_multiword_override`/`multiword_keys` 方法（保留 `from_paths`/`from_str`/`lookup`）。加：

```rust
impl DictView for YamlSynonymExpander {
    fn lookup(&self, lang: KeywordLang, keyword: &str) -> Option<Arc<[String]>> {
        match lang {
            KeywordLang::Zh => self.zh_index.get(keyword).cloned(),
            KeywordLang::En => self.en_index.get(keyword).cloned(),
            KeywordLang::Skip => None,
        }
    }
    fn all_keys(&self) -> Vec<String> {
        self.zh_index
            .keys()
            .chain(self.en_index.keys())
            .cloned()
            .collect()
    }
    fn multiword_keys(&self) -> Vec<String> {
        self.zh_index
            .keys()
            .chain(self.en_index.keys())
            .filter(|k| k.contains(' '))
            .cloned()
            .collect()
    }
}
```

把 `impl SynonymExpander for YamlSynonymExpander` 的 `expand` 改为：

```rust
fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent {
    expand_with_view(self, intent, query)
}
```

（删除原内联实现；同时删除旧的 `fn lookup(&self, ...)` 私有方法，已被 DictView impl 取代。）

- [ ] **Step 3: 跑既有 harness 测试确认零回归**

Run: `cargo test -p locifind-harness 2>&1 | tail -20`
Expected: 全部既有 synonym 测试 PASS（包括 `expand_gazetteer_*` / `expand_keeps_parser_keyword_when_gazetteer_misses` 等）。

- [ ] **Step 4: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5
git add packages/harness/src/synonym/yaml.rs
git commit -m "refactor(beta-11d): expand 算法抽到 DictView 抽象（YamlSynonymExpander 行为不变）"
```

---

## Task 3: harness — `LayeredSynonymExpander`（用户层覆盖系统层）

**Files:**
- Modify: `packages/harness/src/synonym/user.rs`
- Modify: `packages/harness/src/synonym/mod.rs`、`packages/harness/src/lib.rs`

- [ ] **Step 1: 给 `UserIndex` 加 `DictView` 所需的查找 + 写测试**

在 `user.rs` 给 `UserIndex` 加（供 layered 查询）：

```rust
impl UserIndex {
    pub(crate) fn lookup(
        &self,
        lang: crate::synonym::yaml::KeywordLang,
        keyword: &str,
    ) -> Option<Arc<[String]>> {
        use crate::synonym::yaml::KeywordLang;
        match lang {
            KeywordLang::Zh => self.zh_index.get(keyword).cloned(),
            KeywordLang::En => self.en_index.get(keyword).cloned(),
            KeywordLang::Skip => None,
        }
    }
    pub(crate) fn keys(&self) -> impl Iterator<Item = &String> {
        self.zh_index.keys().chain(self.en_index.keys())
    }
}
```

在 `user.rs` 加 `LayeredSynonymExpander` + 测试：

```rust
use std::sync::RwLock;

use locifind_search_backend::{ExpandedSearchIntent, SearchIntent};

use crate::synonym::expander::SynonymExpander;
use crate::synonym::yaml::{expand_with_view, DictView, KeywordLang, YamlSynonymExpander};

/// 双层同义词扩展器：用户层（可变）覆盖系统层（只读）。冲突 keyword 用用户组替换系统组。
#[derive(Debug)]
pub struct LayeredSynonymExpander {
    system: YamlSynonymExpander,
    user: Arc<RwLock<UserIndex>>,
}

impl LayeredSynonymExpander {
    #[must_use]
    pub fn new(system: YamlSynonymExpander, user: Arc<RwLock<UserIndex>>) -> Self {
        Self { system, user }
    }
}

impl DictView for LayeredSynonymExpander {
    fn lookup(&self, lang: KeywordLang, keyword: &str) -> Option<Arc<[String]>> {
        // 用户层优先（替换语义）。
        if let Ok(user) = self.user.read() {
            if let Some(hit) = user.lookup(lang, keyword) {
                return Some(hit);
            }
        }
        self.system.lookup(lang, keyword)
    }
    fn all_keys(&self) -> Vec<String> {
        let mut keys = self.system.all_keys();
        if let Ok(user) = self.user.read() {
            keys.extend(user.keys().cloned());
        }
        keys.sort();
        keys.dedup();
        keys
    }
    fn multiword_keys(&self) -> Vec<String> {
        let mut keys = self.system.multiword_keys();
        if let Ok(user) = self.user.read() {
            keys.extend(user.keys().filter(|k| k.contains(' ')).cloned());
        }
        keys.sort();
        keys.dedup();
        keys
    }
}

impl SynonymExpander for LayeredSynonymExpander {
    fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent {
        expand_with_view(self, intent, query)
    }
}
```

测试（追加到 `user.rs` 的 `mod tests`）：

```rust
#[test]
fn layered_user_only_term_expands_via_gazetteer() {
    use crate::synonym::yaml::YamlSynonymExpander;
    use locifind_search_backend::{FileSearch, SchemaVersion, SearchIntent};
    use std::path::Path;
    use std::sync::RwLock;

    // 系统层用真实词典（保证 from_str 成功）；用户层加一个系统没有的词。
    let system = YamlSynonymExpander::from_str(
        "version: 1\nlanguage: zh\ngroups: []\n",
        Path::new("zh.yaml"),
        "version: 1\nlanguage: en\ngroups: []\n",
        Path::new("en.yaml"),
    )
    .unwrap();
    let user = Arc::new(RwLock::new(
        UserIndex::from_yaml_str(
            "version: 1\ngroups:\n  - head: 友商竞争分析\n    aliases: [AWS, Azure]\n",
        )
        .unwrap(),
    ));
    let exp = LayeredSynonymExpander::new(system, user);
    let intent = SearchIntent::FileSearch(FileSearch {
        schema_version: SchemaVersion::V1,
        language: None,
        keywords: None,
        extensions: None,
        file_type: None,
        location: None,
        modified_time: None,
        created_time: None,
        accessed_time: None,
        size: None,
        exclude_extensions: None,
        exclude_file_type: None,
        sort: None,
        limit: None,
    });
    let out = exp.expand(intent, "友商竞争分析");
    assert_eq!(out.keyword_groups.len(), 1);
    let g = &out.keyword_groups[0];
    assert_eq!(g.head, "友商竞争分析");
    assert!(g.synonyms.contains(&"AWS".to_owned()));
    assert!(g.synonyms.contains(&"Azure".to_owned()));
}

#[test]
fn layered_empty_user_matches_system_only() {
    use crate::synonym::expander::SynonymExpander;
    use crate::synonym::yaml::YamlSynonymExpander;
    use locifind_search_backend::{FileSearch, SchemaVersion, SearchIntent};
    use std::path::Path;
    use std::sync::RwLock;

    let zh = "version: 1\nlanguage: zh\ngroups:\n  - head: 工作汇报\n    aliases: [述职]\n";
    let en = "version: 1\nlanguage: en\ngroups: []\n";
    let intent = || {
        SearchIntent::FileSearch(FileSearch {
            schema_version: SchemaVersion::V1,
            language: None,
            keywords: Some(vec!["工作汇报".to_owned()]),
            extensions: None,
            file_type: None,
            location: None,
            modified_time: None,
            created_time: None,
            accessed_time: None,
            size: None,
            exclude_extensions: None,
            exclude_file_type: None,
            sort: None,
            limit: None,
        })
    };
    let sys_only = YamlSynonymExpander::from_str(zh, Path::new("zh.yaml"), en, Path::new("en.yaml"))
        .unwrap()
        .expand(intent(), "工作汇报");
    let layered = LayeredSynonymExpander::new(
        YamlSynonymExpander::from_str(zh, Path::new("zh.yaml"), en, Path::new("en.yaml")).unwrap(),
        Arc::new(RwLock::new(UserIndex::empty())),
    )
    .expand(intent(), "工作汇报");
    assert_eq!(layered.keyword_groups, sys_only.keyword_groups, "用户层空时与系统层逐字节一致");
}
```

注：`expand_with_view` / `DictView` / `KeywordLang` / `YamlSynonymExpander.lookup` 需在 `yaml.rs` 为 `pub(crate)`。Task 2 已把 `DictView`/`expand_with_view` 设为 `pub(crate)`；`YamlSynonymExpander` 的 `lookup`（DictView 方法）已是 trait 方法可见。

- [ ] **Step 2: 跑测试确认通过**

Run: `cargo test -p locifind-harness synonym::user 2>&1 | tail -20`
Expected: 含两个 layered 测试在内全 PASS。

- [ ] **Step 3: 导出**

`packages/harness/src/synonym/mod.rs` 的 `pub use user::...` 补 `LayeredSynonymExpander`：

```rust
pub use user::{LayeredSynonymExpander, UserDictError, UserGroup, UserIndex};
```

`packages/harness/src/lib.rs:52` 的 `pub use synonym::{...}` 补：

```rust
pub use synonym::{
    ExpanderError, LayeredSynonymExpander, NoopExpander, SynonymExpander, UserDictError,
    UserGroup, UserIndex, YamlSynonymExpander,
};
```

- [ ] **Step 4: fmt + clippy + 全 harness 测试 + 提交**

```bash
cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5
cargo test -p locifind-harness 2>&1 | tail -5
git add packages/harness/src/synonym/user.rs packages/harness/src/synonym/mod.rs packages/harness/src/lib.rs
git commit -m "feat(beta-11d): LayeredSynonymExpander 双层扩展（用户覆盖系统）"
```

---

## Task 4: harness — `UserIndex` 运行时 CRUD（add/update/remove/合并）

**Files:**
- Modify: `packages/harness/src/synonym/user.rs`

- [ ] **Step 1: 写失败测试**

追加到 `user.rs` `mod tests`：

```rust
#[test]
fn add_or_merge_dedups_same_head() {
    let mut idx = UserIndex::empty();
    idx.add_or_merge("报告", vec!["汇报".into()]).unwrap();
    idx.add_or_merge("报告", vec!["汇报".into(), "周报".into()]).unwrap();
    assert_eq!(idx.groups().len(), 1, "同 head 合并不新增组");
    assert_eq!(idx.groups()[0].aliases, vec!["汇报", "周报"], "合并去重保序");
}

#[test]
fn add_rejects_too_many_after_merge() {
    let mut idx = UserIndex::empty();
    idx.add_or_merge("h", vec!["a1".into(), "a2".into(), "a3".into(), "a4".into()]).unwrap();
    let err = idx
        .add_or_merge("h", vec!["a5".into(), "a6".into(), "a7".into(), "a8".into(), "a9".into()])
        .unwrap_err();
    assert!(matches!(err, UserDictError::TooManyAliases { .. }));
    // 失败后原状态不变（仍 4 个 alias）。
    assert_eq!(idx.groups()[0].aliases.len(), 4);
}

#[test]
fn update_replaces_aliases() {
    let mut idx = UserIndex::empty();
    idx.add_or_merge("报告", vec!["汇报".into()]).unwrap();
    idx.update("报告", vec!["周报".into(), "月报".into()]).unwrap();
    assert_eq!(idx.groups()[0].aliases, vec!["周报", "月报"]);
}

#[test]
fn remove_drops_group() {
    let mut idx = UserIndex::empty();
    idx.add_or_merge("报告", vec!["汇报".into()]).unwrap();
    assert!(idx.remove("报告"));
    assert!(idx.groups().is_empty());
    assert!(!idx.remove("不存在"));
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-harness synonym::user 2>&1 | tail -20`
Expected: 编译失败（`add_or_merge`/`update`/`remove` 未定义）。

- [ ] **Step 3: 实现 CRUD**

在 `impl UserIndex` 加（每个写操作先在候选上 lint，成功才落内存 + 重建索引——保证失败原子回滚）：

```rust
/// 添加或合并到同名 head 组（合并去重保序）。校验失败则不改原状态。
pub fn add_or_merge(&mut self, head: &str, aliases: Vec<String>) -> Result<(), UserDictError> {
    let head = head.trim().to_owned();
    let mut candidate = self.groups.clone();
    if let Some(g) = candidate.iter_mut().find(|g| g.head == head) {
        for a in aliases {
            let a = a.trim().to_owned();
            if !a.is_empty() && a != g.head && !g.aliases.contains(&a) {
                g.aliases.push(a);
            }
        }
    } else {
        let aliases: Vec<String> = aliases
            .into_iter()
            .map(|a| a.trim().to_owned())
            .filter(|a| !a.is_empty())
            .collect();
        candidate.push(UserGroup { head, aliases });
    }
    self.replace_groups(candidate)
}

/// 替换某 head 组的 aliases（head 不存在则等价新增）。
pub fn update(&mut self, head: &str, aliases: Vec<String>) -> Result<(), UserDictError> {
    let head = head.trim().to_owned();
    let aliases: Vec<String> = aliases
        .into_iter()
        .map(|a| a.trim().to_owned())
        .filter(|a| !a.is_empty())
        .collect();
    let mut candidate = self.groups.clone();
    match candidate.iter_mut().find(|g| g.head == head) {
        Some(g) => g.aliases = aliases,
        None => candidate.push(UserGroup { head, aliases }),
    }
    self.replace_groups(candidate)
}

/// 删除某 head 组，返回是否删到。
pub fn remove(&mut self, head: &str) -> bool {
    let before = self.groups.len();
    self.groups.retain(|g| g.head != head);
    let removed = self.groups.len() != before;
    if removed {
        let (zh, en) = build_user_indices(&self.groups);
        self.zh_index = zh;
        self.en_index = en;
    }
    removed
}

/// 用候选组列表替换全量（先 lint，成功才提交）。
fn replace_groups(&mut self, candidate: Vec<UserGroup>) -> Result<(), UserDictError> {
    let next = Self::from_groups(candidate)?;
    *self = next;
    Ok(())
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-harness synonym::user 2>&1 | tail -20`
Expected: 全 PASS。

- [ ] **Step 5: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5
git add packages/harness/src/synonym/user.rs
git commit -m "feat(beta-11d): UserIndex 运行时 CRUD（add/update/remove，失败原子回滚）"
```

---

## Task 5: desktop — `user_synonyms.rs` 共享状态 + 文件 IO + 命令

**Files:**
- Create: `apps/desktop/src-tauri/src/user_synonyms.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`（声明模块）

- [ ] **Step 1: 写模块（含纯逻辑单测）**

`apps/desktop/src-tauri/src/user_synonyms.rs`：

```rust
//! BETA-11D 用户级持久化同义词库。
//!
//! 用户词条持久化到 app 配置目录 `user-synonyms.yaml`（与 `settings.json` 同目录），运行时由
//! [`UserSynonymState`] 持有的 `Arc<RwLock<UserIndex>>` 维护——与 [`crate::search::SearchDeps`] 里
//! 的 `LayeredSynonymExpander` **共享同一把锁**，故管理命令改完搜索路径立即可见、零重启。
//!
//! **隐私**：用户词条属用户数据，只存自身配置目录、不外发、不调 `Tracer`（默认不进 trace）。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use locifind_harness::{UserGroup, UserIndex};
use tauri::{AppHandle, Manager, State};

/// 共享用户词典状态（manage 进 Tauri；与 LayeredSynonymExpander 共享同一 Arc）。
pub struct UserSynonymState {
    pub index: Arc<RwLock<UserIndex>>,
    path: PathBuf,
}

impl UserSynonymState {
    #[must_use]
    pub fn new(index: Arc<RwLock<UserIndex>>, path: PathBuf) -> Self {
        Self { index, path }
    }
}

/// `user-synonyms.yaml` 路径（best-effort，仅解析不创建目录）。隐私面板复用。
pub(crate) fn user_synonyms_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|p| p.join("user-synonyms.yaml"))
}

/// 写入用路径：确保配置目录存在。
fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let mut path = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法获取配置目录: {e}"))?;
    if !path.exists() {
        fs::create_dir_all(&path).map_err(|e| format!("无法创建配置目录: {e}"))?;
    }
    path.push("user-synonyms.yaml");
    Ok(path)
}

/// 启动加载：文件缺失 / 损坏 → 空词典（best-effort，不阻塞搜索）。
#[must_use]
pub fn load_user_index(path: &Path) -> UserIndex {
    fs::read_to_string(path)
        .ok()
        .and_then(|c| UserIndex::from_yaml_str(&c).ok())
        .unwrap_or_else(UserIndex::empty)
}

/// 隐私面板用：读取用户词典组数（best-effort，缺库/损坏返回 0）。
pub(crate) fn group_count_at(path: &Path) -> usize {
    load_user_index(path).groups().len()
}

/// 持久化当前内存词典到磁盘。
fn persist(state: &UserSynonymState, app: &AppHandle) -> Result<(), String> {
    let path = store_path(app)?;
    let yaml = state
        .index
        .read()
        .map_err(|_| "用户词典锁中毒".to_owned())?
        .to_yaml_str();
    fs::write(&path, yaml).map_err(|e| format!("写入用户词典失败: {e}"))
}

#[tauri::command]
pub fn get_user_synonyms(state: State<'_, UserSynonymState>) -> Result<Vec<UserGroup>, String> {
    Ok(state
        .index
        .read()
        .map_err(|_| "用户词典锁中毒".to_owned())?
        .groups()
        .to_vec())
}

#[tauri::command]
pub fn add_user_synonym(
    app: AppHandle,
    state: State<'_, UserSynonymState>,
    head: String,
    aliases: Vec<String>,
) -> Result<Vec<UserGroup>, String> {
    {
        let mut idx = state.index.write().map_err(|_| "用户词典锁中毒".to_owned())?;
        idx.add_or_merge(&head, aliases).map_err(|e| e.to_string())?;
    }
    persist(&state, &app)?;
    get_user_synonyms(state)
}

#[tauri::command]
pub fn update_user_synonym(
    app: AppHandle,
    state: State<'_, UserSynonymState>,
    head: String,
    aliases: Vec<String>,
) -> Result<Vec<UserGroup>, String> {
    {
        let mut idx = state.index.write().map_err(|_| "用户词典锁中毒".to_owned())?;
        idx.update(&head, aliases).map_err(|e| e.to_string())?;
    }
    persist(&state, &app)?;
    get_user_synonyms(state)
}

#[tauri::command]
pub fn delete_user_synonym(
    app: AppHandle,
    state: State<'_, UserSynonymState>,
    head: String,
) -> Result<Vec<UserGroup>, String> {
    let changed = {
        let mut idx = state.index.write().map_err(|_| "用户词典锁中毒".to_owned())?;
        idx.remove(&head)
    };
    if changed {
        persist(&state, &app)?;
    }
    get_user_synonyms(state)
}

#[tauri::command]
pub fn export_user_synonyms(state: State<'_, UserSynonymState>) -> Result<String, String> {
    Ok(state
        .index
        .read()
        .map_err(|_| "用户词典锁中毒".to_owned())?
        .to_yaml_str())
}

#[tauri::command]
pub fn import_user_synonyms(
    app: AppHandle,
    state: State<'_, UserSynonymState>,
    yaml_text: String,
) -> Result<Vec<UserGroup>, String> {
    // 整份校验：任一组非法则拒绝、内存不变。
    let parsed = UserIndex::from_yaml_str(&yaml_text).map_err(|e| e.to_string())?;
    {
        let mut idx = state.index.write().map_err(|_| "用户词典锁中毒".to_owned())?;
        *idx = parsed;
    }
    persist(&state, &app)?;
    get_user_synonyms(state)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn load_missing_or_corrupt_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_user_index(&dir.path().join("nope.yaml")).groups().is_empty());
        let bad = dir.path().join("bad.yaml");
        std::fs::write(&bad, b"\t not yaml :::").unwrap();
        assert!(load_user_index(&bad).groups().is_empty(), "损坏文件退空");
    }

    #[test]
    fn persist_roundtrip_and_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user-synonyms.yaml");
        let mut idx = UserIndex::empty();
        idx.add_or_merge("友商竞争分析", vec!["AWS".into(), "Azure".into()]).unwrap();
        std::fs::write(&path, idx.to_yaml_str()).unwrap();
        let loaded = load_user_index(&path);
        assert_eq!(loaded.groups().len(), 1);
        assert_eq!(loaded.groups()[0].head, "友商竞争分析");
        assert_eq!(group_count_at(&path), 1);
    }
}
```

`apps/desktop/src-tauri/src/main.rs` 加模块声明（与其他 `mod` 同处）：

```rust
mod user_synonyms;
```

- [ ] **Step 2: 跑测试确认通过**

Run: `cargo test -p locifind-desktop user_synonyms 2>&1 | tail -20`
Expected: 2 个测试 PASS。

- [ ] **Step 3: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-desktop && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5
git add apps/desktop/src-tauri/src/user_synonyms.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(beta-11d): desktop user_synonyms 共享状态 + 文件 IO + 6 命令"
```

---

## Task 6: desktop — main.rs 接线（LayeredSynonymExpander + manage + 注册命令）

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: 改 `build_synonym_expander` 产出共享 user 索引**

把 `main.rs:188` 的 `build_synonym_expander` 改为返回 expander + 共享 user 索引：

```rust
/// 构造双层同义词扩展器：系统层加载失败退到 noop；同时返回与之共享的用户词典 Arc。
fn build_synonym_expander(
    app: &tauri::AppHandle,
) -> (Arc<dyn SynonymExpander>, Arc<std::sync::RwLock<locifind_harness::UserIndex>>) {
    use locifind_harness::{LayeredSynonymExpander, UserIndex};
    let user_path = user_synonyms::user_synonyms_path(app);
    let user_index = user_path
        .as_deref()
        .map(user_synonyms::load_user_index)
        .unwrap_or_else(UserIndex::empty);
    let user = Arc::new(std::sync::RwLock::new(user_index));

    let (zh, en) = resolve_synonym_paths(app);
    let expander: Arc<dyn SynonymExpander> = match YamlSynonymExpander::from_paths(&zh, &en) {
        Ok(system) => Arc::new(LayeredSynonymExpander::new(system, Arc::clone(&user))),
        Err(err) => {
            eprintln!("synonym: 系统词典加载失败，退到 noop: {err}");
            Arc::new(NoopExpander)
        }
    };
    (expander, user)
}
```

- [ ] **Step 2: 在 setup 接线 manage + 用同一 Arc 构造 state**

在 `main.rs` 构造 `SearchDeps` 的位置（调用 `build_synonym_expander` 处），改为：

```rust
let (synonym_expander, user_index) = build_synonym_expander(&app_handle);
// ... 现有 SearchDeps::new(..., synonym_expander) 保持 ...

// BETA-11D：用户词典管理状态与 expander 共享同一把锁。
let user_synonyms_path = user_synonyms::user_synonyms_path(&app_handle)
    .unwrap_or_else(|| std::path::PathBuf::from("user-synonyms.yaml"));
app.manage(user_synonyms::UserSynonymState::new(
    Arc::clone(&user_index),
    user_synonyms_path,
));
```

（`app_handle` / `app` 的确切变量名以现有 setup 闭包为准；`build_synonym_expander` 现在返回 tuple，调用点解构即可。）

- [ ] **Step 3: 注册命令**

在 `tauri::generate_handler![...]` 列表追加（与 `record_search` 等同处）：

```rust
user_synonyms::get_user_synonyms,
user_synonyms::add_user_synonym,
user_synonyms::update_user_synonym,
user_synonyms::delete_user_synonym,
user_synonyms::export_user_synonyms,
user_synonyms::import_user_synonyms,
```

- [ ] **Step 4: 编译 + 提交**

Run: `cargo build -p locifind-desktop 2>&1 | tail -15`
Expected: 编译通过。

```bash
cargo fmt -p locifind-desktop && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5
git add apps/desktop/src-tauri/src/main.rs
git commit -m "feat(beta-11d): main.rs 接线 LayeredSynonymExpander + manage UserSynonymState + 注册命令"
```

---

## Task 7: desktop — `search_with_adhoc_synonyms`（零命中临时重查，不落盘）

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: 写测试（adhoc 注入产出含 alias 的 OR 组、不写磁盘）**

在 `apps/desktop/src-tauri/src/search/tests.rs` 末尾加（断言注入函数行为；纯逻辑，不跑后端）：

```rust
#[test]
fn inject_adhoc_group_overrides_same_head() {
    use locifind_search_backend::{ExpandedSearchIntent, FileSearch, KeywordGroup, SchemaVersion, SearchIntent};
    let base = SearchIntent::FileSearch(FileSearch {
        schema_version: SchemaVersion::V1,
        language: None,
        keywords: Some(vec!["友商竞争分析".into()]),
        extensions: None,
        file_type: None,
        location: None,
        modified_time: None,
        created_time: None,
        accessed_time: None,
        size: None,
        exclude_extensions: None,
        exclude_file_type: None,
        sort: None,
        limit: None,
    });
    let expanded = ExpandedSearchIntent {
        base,
        keyword_groups: vec![KeywordGroup::singleton("友商竞争分析")],
    };
    let out = super::inject_adhoc_group(expanded, "友商竞争分析", vec!["AWS".into(), "Azure".into()]);
    assert_eq!(out.keyword_groups.len(), 1);
    assert_eq!(out.keyword_groups[0].head, "友商竞争分析");
    assert_eq!(out.keyword_groups[0].synonyms, vec!["AWS", "Azure"]);
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop inject_adhoc_group 2>&1 | tail -10`
Expected: 编译失败（`inject_adhoc_group` 未定义）。

- [ ] **Step 3: 实现注入 + 命令**

在 `search.rs` 加自由函数：

```rust
/// BETA-11D：把一次性 adhoc 同义词组注入 ExpandedSearchIntent（不落盘）。覆盖同 head 组，
/// 否则插到最前。用于零命中教学的「立即重查但不沉淀」。
pub(crate) fn inject_adhoc_group(
    mut expanded: ExpandedSearchIntent,
    head: &str,
    aliases: Vec<String>,
) -> ExpandedSearchIntent {
    use locifind_search_backend::KeywordGroup;
    let mut synonyms = Vec::new();
    for a in aliases {
        let a = a.trim().to_owned();
        if !a.is_empty() && a != head && !synonyms.contains(&a) {
            synonyms.push(a);
        }
    }
    let group = KeywordGroup {
        head: head.to_owned(),
        synonyms,
    };
    if let Some(slot) = expanded.keyword_groups.iter_mut().find(|g| g.head == head) {
        *slot = group;
    } else {
        expanded.keyword_groups.insert(0, group);
    }
    expanded
}
```

`search_impl`（现有内部函数）加可选参数 `adhoc: Option<(String, Vec<String>)>`，并把 `search.rs:276` 的扩展步骤改为：

```rust
let expanded = {
    let e = deps.synonym_expander().expand(effective.clone(), &query);
    match adhoc {
        Some((ref head, ref aliases)) => inject_adhoc_group(e, head, aliases.clone()),
        None => e,
    }
};
```

现有调用 `search_impl` 的入口（`search` 命令路径）传 `None`。新增命令（与 `search` 命令同形态，复用其 streaming 机制）：

```rust
/// BETA-11D：零命中教学——用 adhoc 同义词立即重查（不写用户词典）。
#[tauri::command]
pub async fn search_with_adhoc_synonyms(
    query: String,
    head: String,
    aliases: Vec<String>,
    deps: tauri::State<'_, SearchDeps>,
    on_event: tauri::ipc::Channel<SearchEvent>,
) -> Result<(), String> {
    // 复用 search 命令的执行体，传入 adhoc 注入。
    run_search_impl(query, Some((head, aliases)), &deps, on_event).await
}
```

> 实现注：现有 `search` 命令内部的执行体抽成 `run_search_impl(query, adhoc, deps, on_event)`，
> `search` 命令调用时传 `None`，`search_with_adhoc_synonyms` 传 `Some(...)`。确切的 channel /
> 事件类型（`SearchEvent` / `Channel`）以现有 `search` 命令签名为准，照搬即可。

在 main.rs `generate_handler!` 追加 `search::search_with_adhoc_synonyms`。

- [ ] **Step 4: 跑测试确认通过 + 编译**

Run: `cargo test -p locifind-desktop inject_adhoc_group 2>&1 | tail -10 && cargo build -p locifind-desktop 2>&1 | tail -10`
Expected: 测试 PASS + 编译通过。

- [ ] **Step 5: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-desktop && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/search/tests.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat(beta-11d): search_with_adhoc_synonyms 临时重查（注入 adhoc OR 组不落盘）"
```

---

## Task 8: desktop — 隐私面板集成（用户词典位置 + 组数）

**Files:**
- Modify: `apps/desktop/src-tauri/src/privacy.rs`

- [ ] **Step 1: 写测试**

在 `privacy.rs` 的 `mod tests` 加（仿现有 search_history 计数断言）：

```rust
#[test]
fn overview_includes_user_synonym_count() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("user-synonyms.yaml");
    std::fs::write(&path, "version: 1\ngroups:\n  - head: 报告\n    aliases: [汇报]\n").unwrap();
    assert_eq!(crate::user_synonyms::group_count_at(&path), 1);
}
```

- [ ] **Step 2: 在 `PrivacyOverview` 加字段 + 数据位置**

`PrivacyOverview` struct 加：

```rust
/// BETA-11D：用户同义词库组数。
pub user_synonym_count: usize,
```

`get_privacy_overview` 计算（经 `user_synonyms::user_synonyms_path` + `group_count_at` 单一信源），并在数据位置列表追加一行「用户同义词库」（路径 + 大小，仿 search_history 那行）。

- [ ] **Step 3: 跑测试 + 编译 + 提交**

Run: `cargo test -p locifind-desktop privacy 2>&1 | tail -10`
Expected: PASS。

```bash
cargo fmt -p locifind-desktop && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5
git add apps/desktop/src-tauri/src/privacy.rs
git commit -m "feat(beta-11d): 隐私面板显示用户同义词库位置 + 组数"
```

---

## Task 9: 前端 — 「我的同义词」管理页

**Files:**
- Create: `apps/desktop/src/UserSynonymsPage.tsx`
- Modify: `apps/desktop/src/App.tsx`（路由 / tab）、`apps/desktop/src/styles.css`

- [ ] **Step 1: 写管理页组件**

`apps/desktop/src/UserSynonymsPage.tsx`（调 6 个命令；内联输入、不可用 `window.prompt`）：

```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface UserGroup { head: string; aliases: string[]; }

export function UserSynonymsPage() {
  const [groups, setGroups] = useState<UserGroup[]>([]);
  const [head, setHead] = useState("");
  const [aliasText, setAliasText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [ioText, setIoText] = useState("");

  const refresh = () =>
    invoke<UserGroup[]>("get_user_synonyms").then(setGroups).catch((e) => setError(String(e)));
  useEffect(() => { void refresh(); }, []);

  const splitAliases = (t: string) =>
    t.split(/[,，\s]+/).map((s) => s.trim()).filter(Boolean);

  const add = async () => {
    setError(null);
    try {
      const next = await invoke<UserGroup[]>("add_user_synonym", {
        head: head.trim(), aliases: splitAliases(aliasText),
      });
      setGroups(next); setHead(""); setAliasText("");
    } catch (e) { setError(String(e)); }
  };

  const del = async (h: string) => {
    try { setGroups(await invoke<UserGroup[]>("delete_user_synonym", { head: h })); }
    catch (e) { setError(String(e)); }
  };

  const doExport = async () => setIoText(await invoke<string>("export_user_synonyms"));
  const doImport = async () => {
    setError(null);
    try { setGroups(await invoke<UserGroup[]>("import_user_synonyms", { yamlText: ioText })); }
    catch (e) { setError(String(e)); }
  };

  return (
    <div className="user-synonyms-page">
      <h2>我的同义词</h2>
      <p className="hint">为查询词添加同义词，搜索时会一并扩展召回。改动立即生效。</p>
      <div className="add-row">
        <input placeholder="词（如 友商竞争分析）" value={head} onChange={(e) => setHead(e.target.value)} />
        <input placeholder="同义词，逗号或空格分隔" value={aliasText} onChange={(e) => setAliasText(e.target.value)} />
        <button onClick={add} disabled={!head.trim()}>添加</button>
      </div>
      {error && <div className="error">{error}</div>}
      <ul className="group-list">
        {groups.map((g) => (
          <li key={g.head}>
            <strong>{g.head}</strong>
            <span className="aliases">{g.aliases.join("、")}</span>
            <button onClick={() => del(g.head)}>删除</button>
          </li>
        ))}
        {groups.length === 0 && <li className="empty">还没有自定义同义词</li>}
      </ul>
      <details className="io">
        <summary>导入 / 导出（YAML）</summary>
        <textarea value={ioText} onChange={(e) => setIoText(e.target.value)} rows={8} />
        <div>
          <button onClick={doExport}>导出到上方</button>
          <button onClick={doImport} disabled={!ioText.trim()}>从上方导入</button>
        </div>
      </details>
    </div>
  );
}
```

- [ ] **Step 2: 接入导航**

在 `App.tsx` 把 `UserSynonymsPage` 接入设置区导航（仿现有 PrivacyPage / SettingsPage 的 tab 切换），加一个「我的同义词」入口。`styles.css` 用既有 CSS 变量补 `.user-synonyms-page` / `.group-list` / `.add-row` 等样式（参考现有页面的间距/颜色变量，不引入新色值）。

- [ ] **Step 3: 类型检查 + 构建**

Run: `cd apps/desktop && npm run build 2>&1 | tail -15`
Expected: tsc + vite build 通过。

- [ ] **Step 4: 提交**

```bash
git add apps/desktop/src/UserSynonymsPage.tsx apps/desktop/src/App.tsx apps/desktop/src/styles.css
git commit -m "feat(beta-11d): 前端「我的同义词」管理页（增删 + 导入导出）"
```

---

## Task 10: 前端 — 零命中触发 UX（提示 → 手输 → adhoc 重查 → 记住）

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`、`apps/desktop/src/styles.css`

- [ ] **Step 1: 加触发开关读取 + 零命中提示状态**

在 `SearchView.tsx`：
- 新增设置项「搜索无结果时提示添加同义词」（默认 true）——若 settings 已有结构则加一个 bool 字段；否则用 localStorage `suggestSynonymOnEmpty`（默认开）。
- 搜索完成（结果流结束）后，若**结果数 === 0** 且开关开 且 query 非空 → 显示零命中提示条。

- [ ] **Step 2: 实现两段式教学 UI**

提示条内联流程（不可用 `window.prompt`）：

```tsx
// 状态
const [emptyPrompt, setEmptyPrompt] = useState<{ query: string; head: string } | null>(null);
const [teachAliases, setTeachAliases] = useState("");
const [askRemember, setAskRemember] = useState<{ head: string; aliases: string[] } | null>(null);

// 结果流结束回调里：
if (resultCount === 0 && suggestOn && query.trim()) {
  setEmptyPrompt({ query, head: extractedKeyword ?? query.trim() });
}

// 用户填好 aliases 点「扩展搜索」：立即重查但不落盘
const runAdhoc = async () => {
  if (!emptyPrompt) return;
  const aliases = teachAliases.split(/[,，\s]+/).map((s) => s.trim()).filter(Boolean);
  if (aliases.length === 0) return;
  await invoke("search_with_adhoc_synonyms", {
    query: emptyPrompt.query, head: emptyPrompt.head, aliases,
    onEvent: channel, // 复用现有 SearchView 的结果 Channel
  });
  setAskRemember({ head: emptyPrompt.head, aliases });
  setEmptyPrompt(null); setTeachAliases("");
};

// 重查出结果后问「是否记住此映射?」
const remember = async () => {
  if (!askRemember) return;
  await invoke("add_user_synonym", { head: askRemember.head, aliases: askRemember.aliases });
  setAskRemember(null);
};
```

UI：`emptyPrompt` 非空 → 渲染「没找到结果。为「{head}」添加同义词扩展搜索?」+ head 可编辑 input + aliases input + 「扩展搜索」按钮；`askRemember` 非空 → 渲染「是否记住此映射?」+「记住」/「不记住（关闭）」两按钮。`styles.css` 用既有变量补样式。

> 复用注意：`search_with_adhoc_synonyms` 的结果通过与 `search` 命令**同一个 `Channel<SearchEvent>`** 回流，前端结果渲染逻辑不变——只是换了触发命令。确切的 channel 变量名以现有 `SearchView` 的 search 调用为准。

- [ ] **Step 3: 类型检查 + 构建**

Run: `cd apps/desktop && npm run build 2>&1 | tail -15`
Expected: 通过。

- [ ] **Step 4: 提交**

```bash
git add apps/desktop/src/SearchView.tsx apps/desktop/src/styles.css
git commit -m "feat(beta-11d): 零命中触发 UX（提示→手输→adhoc 重查→记住确认）"
```

---

## Task 11: 文档 — 隐私文案 / ROADMAP / 手测场景 / licenses

**Files:**
- Modify: 隐私页教育文案（`PrivacyPage.tsx` 或对应 doc）、`ROADMAP.md`、`docs/manual-test-scenarios.md`、`docs/third-party-licenses.md`（如需）

- [ ] **Step 1: 隐私文案**

在隐私页教育性文案 + 隐私政策 doc 新增一条：「**用户同义词库**：你添加的同义词存于本机 `user-synonyms.yaml`，不上传、不同步，可在「我的同义词」页查看 / 删除 / 导出。」

- [ ] **Step 2: ROADMAP BETA-12 补 checklist**

在 `ROADMAP.md` BETA-12 卡片描述里追加一条：「卸载需清 `app_config_dir/user-synonyms.yaml`（BETA-11D 用户同义词库）。」

- [ ] **Step 3: 手测场景**

在 `docs/manual-test-scenarios.md` 加 BETA-11D 节，覆盖：
1. 设置页「我的同义词」增 / 删 / 导入导出，列表即时刷新；
2. lint 错误内联提示（如别名超 8 个、标识符样词被拒）；
3. **目标 case 端到端**：搜「友商竞争分析」零结果 → 提示 → 手输 `AWS, Azure, 产品分析, 功能洞察` → 扩展搜索出结果 → 「记住」→ **重启 app** → 再搜「友商竞争分析」零延迟直接命中；
4. 隐私页可见「用户同义词库」位置 + 组数。

- [ ] **Step 4: licenses 检查**

确认未引入新依赖（`serde_yaml` 已是 harness 依赖；`tempfile` 已是 desktop dev-dep）。若 `cargo tree` 显示新增传递依赖则登记 `docs/third-party-licenses.md`，否则跳过。

Run: `git diff --stat`（确认仅文档改动）

- [ ] **Step 5: 提交**

```bash
git add ROADMAP.md docs/manual-test-scenarios.md apps/desktop/src/PrivacyPage.tsx
git commit -m "docs(beta-11d): 隐私文案 + BETA-12 卸载 checklist + 手测场景"
```

---

## Task 12: 全量验证（evals byte-equal + workspace 零回归）

**Files:** 无（仅运行验证）

- [ ] **Step 1: evals parser-only byte-equal 硬门**

Run:
```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5 2>&1 | tail -5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9 2>&1 | tail -5
```
Expected: v0.5 = `473`、v0.9 = `726`（与重构前逐字节一致；如有偏移说明 Task 2 重构破了 byte-equal，回查 `expand_with_view`）。

> 确切的 evals 调用方式以 `packages/evals` README / 现有 STATUS 记录为准（本仓 evals 为 parser-only 确定性）。

- [ ] **Step 2: 全 workspace fmt + clippy + test**

Run:
```bash
cargo fmt --check 2>&1 | tail -5
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
cargo test --workspace 2>&1 | tail -20
```
Expected: fmt 无输出；clippy 0 警告；test 零回归（仅 platform-macos 2 个预存 Windows 失败除外）。

- [ ] **Step 3: 前端构建**

Run: `cd apps/desktop && npm run build 2>&1 | tail -10`
Expected: tsc + vite build 通过。

- [ ] **Step 4: 最终提交（若有 fmt 调整）**

```bash
git add -A && git commit -m "chore(beta-11d): 全量验证通过（evals byte-equal + workspace 零回归）" || echo "无待提交改动"
```

---

## 自审记录

- **spec 覆盖**：A 持久化（Task 1/5）/ B 双层（Task 2/3/6）/ C 管理 UI（Task 5/9）/ D 触发 UX（Task 7/10）；隐私（Task 8/11）；污染防护（Task 1 lint，三入口共用）；卸载 checklist（Task 11）；测试（各 task TDD + Task 12）；目标 case（Task 11 手测）。全部有对应 task。
- **类型一致**：`UserIndex` / `UserGroup` / `UserDictError` / `LayeredSynonymExpander` / `UserSynonymState` / `DictView` / `expand_with_view` / `inject_adhoc_group` 跨 task 命名一致。
- **占位符**：无 TBD / TODO；每段代码完整可落地。少数「以现有签名为准」处（search 命令 channel、App.tsx 导航、setup 变量名）已显式标注实现注，非占位。
