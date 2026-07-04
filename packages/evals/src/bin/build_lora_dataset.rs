#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

//! BETA-08 `LoRA` 数据集生成器：从 v0.5 fixture cases 转出训练 JSONL。
//!
//! 用法：
//! ```text
//! cargo run --release --bin build_lora_dataset -- \
//!     --input packages/evals/fixtures/v0.5/cases.json \
//!     --output training/datasets/v0.5-patch/v0/
//! ```
//!
//! 详见 docs/superpowers/specs/2026-05-27-beta-08-lora-design.md

use clap::Parser;
use locifind_evals::{parse_cases_str, variant_name, Case};
use locifind_intent_parser::hybrid::{build_hybrid_prompt, IntentDraft};
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "build_lora_dataset", about = "BETA-08 LoRA 数据集生成器")]
struct Args {
    /// 输入 fixture 路径，例如 packages/evals/fixtures/v0.5/cases.json
    #[arg(long)]
    input: PathBuf,
    /// 输出目录，将写入 cases.jsonl 和 meta.json
    #[arg(long)]
    output: PathBuf,
    /// BETA-24：只保留 `fillable_fields` 含该字段的 case（如 keywords）
    #[arg(long)]
    require_fillable: Option<String>,
    /// meta.json 的 `dataset_name`（默认沿用 v0.5-patch）
    #[arg(long, default_value = "v0.5-patch")]
    dataset_name: String,
}

#[derive(Debug, Default)]
struct Stats {
    total_cases: usize,
    skipped_variant_mismatch: usize,
    empty_patch: usize,
    nonempty_patch: usize,
    skipped_not_fillable: usize,
    by_fillable_field: BTreeMap<String, usize>,
}

/// --require-fillable 过滤：不触发该字段待填的 case 推理期到不了模型，
/// 训进去反而教模型补没被要求的字段（BETA-24）。
fn line_passes_require_fillable(line: &JsonlLine, required: Option<&str>) -> bool {
    // 不用 Option::is_none_or（1.82 起才稳定，项目 MSRV 1.80）
    match required {
        None => true,
        Some(f) => line.fillable_fields.iter().any(|x| x == f),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in &digest {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn git_rev() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |s| s.trim().to_owned())
}

#[derive(Debug, Serialize)]
struct JsonlLine {
    prompt: String,
    completion: String,
    case_id: String,
    fillable_fields: Vec<String>,
    draft_variant: String,
}

/// 计算 draft 到 expected 的 top-level 字段 diff。
///
/// 规则（spec §4.2）：
/// - `intent` / `schema_version` 字段被剔除（hybrid 锁定 variant，patch 不应改这两个）
/// - draft 缺该字段 / 是 null → patch[字段] = expected[字段]
/// - draft[字段] == expected[字段] → 不进 patch
/// - draft[字段] != expected[字段] → patch[字段] = expected[字段]（整字段替换，不做嵌套 diff）
fn compute_patch(draft: &Map<String, Value>, expected: &Map<String, Value>) -> Map<String, Value> {
    let mut patch = Map::new();
    for (key, expected_val) in expected {
        if key == "intent" || key == "schema_version" {
            continue;
        }
        let draft_val = draft.get(key);
        let needs_fill = match draft_val {
            None => true,
            Some(v) if v.is_null() => true,
            Some(v) => v != expected_val,
        };
        if needs_fill {
            patch.insert(key.clone(), expected_val.clone());
        }
    }
    patch
}

/// 把 Case 转成一行 JSONL。
///
/// 返回 `Ok(None)` 表示 variant 错位（hybrid 锁定 variant，patch 无法救），该 case 不进数据集。
fn case_to_jsonl_line(case: &Case) -> anyhow::Result<Option<JsonlLine>> {
    let draft = IntentDraft::from_query(&case.query);
    let draft_variant = variant_name(&draft.intent).to_owned();

    if draft_variant != case.variant {
        return Ok(None);
    }

    let draft_json = serde_json::to_value(&draft.intent)?;
    let draft_obj = draft_json
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("case {}: draft 非 JSON object", case.id))?;
    let expected_obj = case
        .expected_intent
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("case {}: expected_intent 非 JSON object", case.id))?;

    let patch = compute_patch(draft_obj, expected_obj);

    debug_assert!(!patch.contains_key("intent"));
    debug_assert!(!patch.contains_key("schema_version"));

    let prompt = build_hybrid_prompt(&case.query, &draft);
    let completion = serde_json::to_string(&Value::Object(patch))?;
    let fillable_fields = draft
        .fillable_fields
        .iter()
        .map(|s| (*s).to_owned())
        .collect();

    Ok(Some(JsonlLine {
        prompt,
        completion,
        case_id: case.id.clone(),
        fillable_fields,
        draft_variant,
    }))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let input_bytes = fs::read(&args.input)
        .map_err(|e| anyhow::anyhow!("读取 {} 失败: {e}", args.input.display()))?;
    let source_sha256 = sha256_hex(&input_bytes);
    let input_str =
        std::str::from_utf8(&input_bytes).map_err(|e| anyhow::anyhow!("input 非 UTF-8: {e}"))?;

    let mut cases: Vec<Case> =
        parse_cases_str(input_str).map_err(|e| anyhow::anyhow!("解析 cases.json 失败: {e}"))?;
    cases.sort_by(|a, b| a.id.cmp(&b.id));

    let mut stats = Stats {
        total_cases: cases.len(),
        ..Stats::default()
    };
    let mut lines: Vec<String> = Vec::with_capacity(cases.len());

    for case in &cases {
        match case_to_jsonl_line(case)? {
            None => stats.skipped_variant_mismatch += 1,
            Some(line) => {
                if !line_passes_require_fillable(&line, args.require_fillable.as_deref()) {
                    stats.skipped_not_fillable += 1;
                    continue;
                }
                if line.completion == "{}" {
                    stats.empty_patch += 1;
                } else {
                    stats.nonempty_patch += 1;
                }
                for f in &line.fillable_fields {
                    *stats.by_fillable_field.entry(f.clone()).or_insert(0) += 1;
                }
                lines.push(serde_json::to_string(&line)?);
            }
        }
    }

    fs::create_dir_all(&args.output)?;

    let cases_path = args.output.join("cases.jsonl");
    let mut f = fs::File::create(&cases_path)?;
    for line in &lines {
        writeln!(f, "{line}")?;
    }

    let meta = serde_json::json!({
        "dataset_name": args.dataset_name,
        "version": "v0",
        "source": args.input.to_string_lossy(),
        "source_sha256": source_sha256,
        "license": "internal",
        "generation_method": "parser-diff",
        "generator_version": format!("build_lora_dataset@{}", git_rev()),
        "privacy_review_status": "synthetic-no-pii",
        "created_at": chrono::Utc::now().to_rfc3339(),
        "reviewer": "Claude Code",
        "stats": {
            "total_cases": stats.total_cases,
            "skipped_variant_mismatch": stats.skipped_variant_mismatch,
            "empty_patch": stats.empty_patch,
            "nonempty_patch": stats.nonempty_patch,
            "skipped_not_fillable": stats.skipped_not_fillable,
            "by_fillable_field": stats.by_fillable_field,
        }
    });
    let meta_path = args.output.join("meta.json");
    fs::write(&meta_path, serde_json::to_string_pretty(&meta)? + "\n")?;

    eprintln!(
        "✅ 写入 {} 行 cases.jsonl + meta.json 到 {}",
        lines.len(),
        args.output.display()
    );
    eprintln!(
        "   stats: total={}, skip_variant={}, skip_not_fillable={}, empty={}, nonempty={}",
        stats.total_cases,
        stats.skipped_variant_mismatch,
        stats.skipped_not_fillable,
        stats.empty_patch,
        stats.nonempty_patch
    );

    Ok(())
}

#[cfg(test)]
mod case_conversion_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use locifind_evals::Case;
    use serde_json::json;

    fn mk_case(id: &str, query: &str, expected: serde_json::Value) -> Case {
        let variant = expected
            .get("intent")
            .and_then(|v| v.as_str())
            .map_or("Unknown", |s| match s {
                "file_search" => "FileSearch",
                "media_search" => "MediaSearch",
                "file_action" => "FileAction",
                "refine" => "Refine",
                "clarify" => "Clarify",
                _ => "Unknown",
            })
            .to_owned();
        Case {
            id: id.to_owned(),
            query: query.to_owned(),
            language: "zh".to_owned(),
            variant,
            expected_intent: expected,
        }
    }

    #[test]
    fn require_fillable_filters_non_keywords_cases() {
        // fillable 不含 keywords 的 case 在 --require-fillable keywords 下应被滤掉
        let covered = mk_case(
            "test-3",
            "上周的pdf",
            json!({
                "intent": "file_search", "schema_version": "1.0", "language": "zh",
                "extensions": ["pdf"],
                "modified_time": {"type": "relative", "value": "last_week"},
                "sort": "modified_desc"
            }),
        );
        let line = case_to_jsonl_line(&covered)
            .expect("不应报错")
            .expect("variant 应匹配");
        assert!(
            !line.fillable_fields.iter().any(|f| f == "keywords"),
            "前提：该 case 不应有 keywords 待填，实际 {:?}",
            line.fillable_fields
        );
        assert!(!line_passes_require_fillable(&line, Some("keywords")));
        assert!(line_passes_require_fillable(&line, None));
    }

    #[test]
    fn variant_mismatch_returns_none() {
        // parser 对 "查找昨天编辑过的 ppt" 会判 FileSearch，fixture 故意标 media_search
        let case = mk_case(
            "test-1",
            "查找昨天编辑过的 ppt",
            json!({"intent": "media_search", "schema_version": "1.0", "media_type": "video"}),
        );
        let result = case_to_jsonl_line(&case).expect("should not error");
        assert!(result.is_none(), "variant 错位的 case 应返回 None");
    }

    #[test]
    fn variant_match_returns_some_with_correct_fields() {
        let case = mk_case(
            "test-2",
            "查找昨天编辑过的 ppt",
            json!({
                "intent": "file_search",
                "schema_version": "1.0",
                "language": "zh",
                "extensions": ["ppt", "pptx"],
                "file_type": "presentation",
                "modified_time": {"type": "relative", "value": "yesterday"},
                "sort": "modified_desc"
            }),
        );
        let result = case_to_jsonl_line(&case).expect("should not error");
        let line = result.expect("should be Some");
        assert_eq!(line.case_id, "test-2");
        assert_eq!(line.draft_variant, "FileSearch");
        assert!(line.prompt.contains("查找昨天编辑过的 ppt"));
        let _: serde_json::Value =
            serde_json::from_str(&line.completion).expect("completion is valid JSON");
    }
}

#[cfg(test)]
mod compute_patch_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::compute_patch;
    use serde_json::json;

    #[test]
    fn empty_patch_when_equal() {
        let draft = json!({"intent": "file_search", "language": "zh", "schema_version": "1.0"});
        let expected = json!({"intent": "file_search", "language": "zh", "schema_version": "1.0"});
        let patch = compute_patch(draft.as_object().unwrap(), expected.as_object().unwrap());
        assert!(patch.is_empty(), "完全相等时 patch 应为空");
    }

    #[test]
    fn fills_missing_field() {
        let draft = json!({"intent": "file_search", "language": "zh", "schema_version": "1.0"});
        let expected = json!({
            "intent": "file_search",
            "language": "zh",
            "schema_version": "1.0",
            "modified_time": {"type": "relative", "value": "yesterday"}
        });
        let patch = compute_patch(draft.as_object().unwrap(), expected.as_object().unwrap());
        assert_eq!(patch.len(), 1);
        assert_eq!(
            patch.get("modified_time").unwrap(),
            &json!({"type": "relative", "value": "yesterday"})
        );
    }

    #[test]
    fn replaces_different_value_whole_field() {
        let draft = json!({
            "intent": "file_search",
            "schema_version": "1.0",
            "sort": "relevance_desc"
        });
        let expected = json!({
            "intent": "file_search",
            "schema_version": "1.0",
            "sort": "modified_desc"
        });
        let patch = compute_patch(draft.as_object().unwrap(), expected.as_object().unwrap());
        assert_eq!(patch.len(), 1);
        assert_eq!(patch.get("sort").unwrap(), &json!("modified_desc"));
    }

    #[test]
    fn excludes_intent_and_schema_version_even_when_different() {
        // 防御性：即使 intent / schema_version 不同（理论上调用方应先 skip variant 错位 case），
        // compute_patch 自己也要剔除这两个字段。
        let draft =
            json!({"intent": "file_search", "schema_version": "1.0", "sort": "relevance_desc"});
        let expected =
            json!({"intent": "media_search", "schema_version": "1.0", "sort": "modified_desc"});
        let patch = compute_patch(draft.as_object().unwrap(), expected.as_object().unwrap());
        assert!(!patch.contains_key("intent"));
        assert!(!patch.contains_key("schema_version"));
        assert_eq!(patch.get("sort").unwrap(), &json!("modified_desc"));
    }
}
