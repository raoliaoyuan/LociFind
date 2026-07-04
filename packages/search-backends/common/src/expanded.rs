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
/// 注：`SearchIntent` 含 `f64` 字段（`SizeExpression`），故只能 `PartialEq`，不实现 `Eq`。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
            .map(|kws| {
                kws.iter()
                    .map(|kw| KeywordGroup::singleton(kw.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            base,
            keyword_groups,
        }
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
