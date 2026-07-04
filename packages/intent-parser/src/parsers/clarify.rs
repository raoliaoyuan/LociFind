//! `SearchIntent::Clarify` 构造 helper。
//!
//! 顶层 `parse()` 内的 clarify 触发判断仍在 lib.rs，作为 dispatcher 前置。
#![allow(clippy::pedantic)]

use locifind_search_backend::{Clarify, ClarifyReason, Language, SchemaVersion, SearchIntent};

pub(crate) fn clarify_unknown(language: Language, question: &str) -> SearchIntent {
    clarify_with(language, ClarifyReason::Unknown, question)
}

/// BETA-13-G7：以指定 `reason` 构造 clarify（question 仅作呈现，eval 不比对文案）。
pub(crate) fn clarify_with(
    language: Language,
    reason: ClarifyReason,
    question: &str,
) -> SearchIntent {
    SearchIntent::Clarify(Clarify {
        schema_version: SchemaVersion::V1,
        language: Some(language),
        reason,
        question: question.to_owned(),
        options: None,
    })
}
