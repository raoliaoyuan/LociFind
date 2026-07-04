#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::needless_pass_by_value
)]

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand};
use locifind_evals::variant_name;
use locifind_intent_parser::hybrid::IntentDraft;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "fixtures")]
#[command(about = "LociFind 合成测试 fixture 生成器", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// 生成的根目录
    #[arg(short, long, default_value = "tests/fixtures/files")]
    dir: PathBuf,
}

#[derive(Subcommand)]
enum Commands {
    /// 生成 fixture
    Generate,
    /// 清理已生成的 fixture
    Clean,
    /// 生成 MVP-25 evals v0.5 fixture JSON
    GenerateEvalsV05 {
        /// 输出 JSON 文件
        #[arg(short, long, default_value = "packages/evals/fixtures/v0.5/cases.json")]
        output: PathBuf,
    },
    /// BETA-13：把 _authoring/*.json 分片确定性汇编为 coverage-cases.json
    AssembleCoverage {
        /// 分片目录（每片为 case JSON 数组）
        #[arg(long, default_value = "packages/evals/fixtures/v0.9/_authoring")]
        shards: PathBuf,
        /// 输出汇编文件
        #[arg(
            short,
            long,
            default_value = "packages/evals/fixtures/v0.9/coverage-cases.json"
        )]
        output: PathBuf,
    },
    /// BETA-13：合并 v0.5（500，逐字）+ coverage（手标 ground-truth）→ v0.9 cases.json（1000）
    GenerateEvalsV09 {
        /// v0.5 基底 fixture
        #[arg(long, default_value = "packages/evals/fixtures/v0.5/cases.json")]
        base: PathBuf,
        /// 覆盖驱动手标 case
        #[arg(
            long,
            default_value = "packages/evals/fixtures/v0.9/coverage-cases.json"
        )]
        coverage: PathBuf,
        /// 输出 JSON 文件
        #[arg(short, long, default_value = "packages/evals/fixtures/v0.9/cases.json")]
        output: PathBuf,
    },
    /// BETA-24：生成 lora-aug-keywords fixture（模板 + 手写汇编 + train/heldout 切分）
    GenerateLoraAugKeywords {
        /// 手写 seed 分片目录（每片为 `AugSeed` JSON 数组）
        #[arg(
            long,
            default_value = "packages/evals/fixtures/lora-aug-keywords/v1/_authoring"
        )]
        handwritten: PathBuf,
        /// 训练份输出
        #[arg(
            long,
            default_value = "packages/evals/fixtures/lora-aug-keywords/v1/cases.json"
        )]
        output_train: PathBuf,
        /// held-out 份输出（永不进训练）
        #[arg(
            long,
            default_value = "packages/evals/fixtures/lora-aug-keywords/v1/heldout-cases.json"
        )]
        output_heldout: PathBuf,
    },
}

struct FileDef {
    path: &'static str, // 相对根目录的路径
    size_mb: u64,
    modified_days_ago: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvalCase {
    id: String,
    query: String,
    language: String,
    expected_intent: Value,
}

#[derive(Default)]
struct EvalBuilder {
    cases: Vec<EvalCase>,
    variant_lang_counts: HashMap<(&'static str, &'static str), usize>,
}

const FIXTURES: &[FileDef] = &[
    // 7.1 中文 — 文件搜索相关
    FileDef {
        path: "Desktop/synthetic-budget.pptx",
        size_mb: 5,
        modified_days_ago: 1,
    }, // 昨天编辑的 ppt
    FileDef {
        path: "Downloads/synthetic-excel-recent.xlsx",
        size_mb: 2,
        modified_days_ago: 2,
    }, // 最近三天修改的 Excel
    FileDef {
        path: "Downloads/synthetic-video-large.mp4",
        size_mb: 150,
        modified_days_ago: 0,
    }, // 下载目录中大于 100MB 的视频
    FileDef {
        path: "Documents/合成-预算-2026.docx",
        size_mb: 1,
        modified_days_ago: 0,
    }, // 名字里有"预算"
    FileDef {
        path: "Desktop/synthetic-word-doc.docx",
        size_mb: 1,
        modified_days_ago: 0,
    }, // 桌面上的 word
    FileDef {
        path: "Documents/2025/synthetic-presentation-2025.pptx",
        size_mb: 10,
        modified_days_ago: 400,
    }, // 2025 年的 ppt
    FileDef {
        path: "Documents/合成-会议纪要-001.md",
        size_mb: 1,
        modified_days_ago: 0,
    }, // "会议纪要"开头
    FileDef {
        path: "Downloads/synthetic-received-last-week.pdf",
        size_mb: 3,
        modified_days_ago: 5,
    }, // 上周收到的 pdf
    FileDef {
        path: "Documents/synthetic-recent-markdown.md",
        size_mb: 1,
        modified_days_ago: 3,
    }, // 最近一周访问过的 md
    FileDef {
        path: "Documents/synthetic-old-archive.zip",
        size_mb: 20,
        modified_days_ago: 30,
    }, // 2026-05-01 之前 (假设今天是 2026-05-25)
    // 7.2 中文 — 媒体搜索相关
    FileDef {
        path: "Music/Eric Clapton - Wonderful Tonight.mp3",
        size_mb: 5,
        modified_days_ago: 10,
    },
    FileDef {
        path: "Music/周华健 - 朋友.flac",
        size_mb: 30,
        modified_days_ago: 40,
    },
    FileDef {
        path: "Music/周华健 - 花心.mp3",
        size_mb: 8,
        modified_days_ago: 20,
    },
    FileDef {
        path: "Pictures/Screenshots/Screenshot 2026-05-24 10-00-00.png",
        size_mb: 2,
        modified_days_ago: 1,
    },
    FileDef {
        path: "Movies/synthetic-movie-long.mp4",
        size_mb: 1200,
        modified_days_ago: 5,
    }, // > 1GB
    // 7.3 英文 — 文件搜索相关
    FileDef {
        path: "Downloads/synthetic-large-file.bin",
        size_mb: 150,
        modified_days_ago: 0,
    },
    FileDef {
        path: "Desktop/synthetic-image.jpg",
        size_mb: 1,
        modified_days_ago: 0,
    },
    // 7.4 中英混合相关
    FileDef {
        path: "Documents/budget-plan.pptx",
        size_mb: 4,
        modified_days_ago: 0,
    },
];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.dir;

    match cli.command.unwrap_or(Commands::Generate) {
        Commands::Generate => generate(&root)?,
        Commands::Clean => clean(&root)?,
        Commands::GenerateEvalsV05 { output } => generate_evals_v05(&output)?,
        Commands::AssembleCoverage { shards, output } => assemble_coverage(&shards, &output)?,
        Commands::GenerateEvalsV09 {
            base,
            coverage,
            output,
        } => generate_evals_v09(&base, &coverage, &output)?,
        Commands::GenerateLoraAugKeywords {
            handwritten,
            output_train,
            output_heldout,
        } => generate_lora_aug_keywords(&handwritten, &output_train, &output_heldout)?,
    }

    Ok(())
}

/// BETA-13：把 `_authoring/*.json` 分片确定性汇编为 `coverage-cases.json`。
///
/// 分片按文件名升序拼接，片内保持原序——纯确定性。每片须为 case JSON 数组。
fn assemble_coverage(shards: &Path, output: &Path) -> Result<()> {
    let mut shard_paths: Vec<PathBuf> = fs::read_dir(shards)
        .with_context(|| format!("无法读分片目录 {}", shards.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    shard_paths.sort();

    let mut assembled: Vec<EvalCase> = Vec::new();
    for shard in &shard_paths {
        let cases =
            load_eval_cases(shard).with_context(|| format!("分片 {} 解析失败", shard.display()))?;
        assembled.extend(cases);
    }

    write_eval_cases(output, &assembled)?;
    println!(
        "已汇编 coverage: {} ({} cases from {} shards)",
        output.display(),
        assembled.len(),
        shard_paths.len(),
    );
    Ok(())
}

/// BETA-24：keywords 补全训练样本的种子——手写分片与模板生成共用此形态。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AugSeed {
    id: String,
    query: String,
    /// parser 会丢、期望模型补回的内容词（构造期已知）
    missing_keywords: Vec<String>,
}

/// 种子 → eval Case JSON。`expected_intent` = parser draft ⊕ 补齐 keywords：
/// 其余字段继承 draft（时间/排序 parser 本就处理对；language 已锁死不归模型管），
/// 保证 patch 精确等于 keywords 填充，且与 `apply_patch` 并集语义（draft 在前）同序。
/// 返回 None = parser 对该 query 不触发 keywords 待填（推理期到不了模型，不进数据集）。
fn aug_case_from_seed(seed: &AugSeed) -> Option<Value> {
    let draft = IntentDraft::from_query(&seed.query);
    if !draft.fillable_fields.contains(&"keywords") {
        return None;
    }
    let variant = variant_name(&draft.intent).to_owned();
    // SearchIntent 是 internally-tagged enum，必序列化为 JSON object，下面两处 None
    // 实为 dead path（clippy::expect_used 在本 bin 非测试码 denied，故用 `.ok()?` 而非 expect）。
    let mut intent_json = serde_json::to_value(&draft.intent).ok()?;
    let obj = intent_json.as_object_mut()?;
    let mut kws: Vec<String> = obj
        .get("keywords")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    for w in &seed.missing_keywords {
        if !kws.contains(w) {
            kws.push(w.clone());
        }
    }
    if kws.is_empty() {
        return None; // 连补齐后都没有 keywords 的种子无训练价值
    }
    obj.insert("keywords".to_owned(), serde_json::json!(kws));
    let language = obj
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    Some(serde_json::json!({
        "id": seed.id,
        "query": seed.query,
        "language": language,
        "variant": variant,
        "expected_intent": intent_json,
    }))
}

/// 程序模板种子：模板 × 合成词表，索引步进采样（确定性、无 RNG）。
/// 全部合成词，无真实文件名/路径（datasets README 强制项）。
fn template_seeds() -> Vec<AugSeed> {
    const ZH_CONTENT: &[&str] = &[
        "会议纪要",
        "周报",
        "体检报告",
        "财务预算",
        "项目复盘",
        "培训材料",
        "实验数据",
        "装修合同",
        "旅行攻略",
        "简历模板",
        "课程笔记",
        "年度总结",
        "需求文档",
        "采购清单",
        "发布计划",
        "调研报告",
        "用户手册",
        "测试报告",
        "工作总结",
        "面试记录",
    ];
    const ZH_TAIL: &[&str] = &["运维", "架构", "验收", "季度", "客户", "合规"];
    const ZH_TIME: &[&str] = &["2025年", "去年", "上个月", "最近一周"];
    const EN_CONTENT: &[&str] = &[
        "annual budget",
        "research paper",
        "onboarding checklist",
        "marketing plan",
        "design review",
        "meeting notes",
        "release plan",
        "expense report",
        "user survey",
        "product spec",
    ];
    const EN_TAIL: &[&str] = &["roadmap", "compliance", "handover"];
    const ZH_TOPIC: &[&str] = &[
        "毕业旅行",
        "夏天的雨",
        "旧城改造",
        "深夜电台",
        "山间徒步",
        "海边日落",
        "童年回忆",
        "城市夜景",
    ];
    const EN_TOPIC: &[&str] = &[
        "rainy nights",
        "long road trips",
        "quiet mornings",
        "summer festivals",
        "city lights",
    ];

    let mut seeds = Vec::new();
    let mut n = 0usize;
    let mut push = |seeds: &mut Vec<AugSeed>, query: String, missing: Vec<&str>| {
        n += 1;
        seeds.push(AugSeed {
            id: format!("aug-tpl-{n:03}"),
            query,
            missing_keywords: missing.into_iter().map(str::to_owned).collect(),
        });
    };

    // 模板 1（问题 4 同型）：「{time}的{content}文件名包含{tail}」→ parser 短路丢 content
    for (i, content) in ZH_CONTENT.iter().enumerate() {
        let time = ZH_TIME[i % ZH_TIME.len()];
        let tail = ZH_TAIL[i % ZH_TAIL.len()];
        push(
            &mut seeds,
            format!("{time}的{content}文件名包含{tail}"),
            vec![content],
        );
    }
    // 模板 2（同型异构，与模板 1 区分形态）：「找{content}相关的文件名包含{tail}的」
    // parser 短路抓 tail（文件名包含…）丢前置 content → 触发 keywords 待填。
    for (i, content) in ZH_CONTENT.iter().enumerate() {
        let tail = ZH_TAIL[(i + 2) % ZH_TAIL.len()];
        push(
            &mut seeds,
            format!("找{content}相关的文件名包含{tail}的"),
            vec![content],
        );
    }
    // 模板 3（英文文件名包含同型）：「{content} with filename containing {tail}」
    // parser 抓结构化的 tail，丢前置内容短语 content → 触发 keywords。
    for (i, content) in EN_CONTENT.iter().enumerate() {
        let tail = EN_TAIL[i % EN_TAIL.len()];
        push(
            &mut seeds,
            format!("{content} with filename containing {tail}"),
            vec![content],
        );
    }
    // 模板 4（媒体中文）：「播放{topic}相关的歌」→ parser 给空 keywords 待填。
    for topic in ZH_TOPIC {
        push(&mut seeds, format!("播放{topic}相关的歌"), vec![topic]);
    }
    // 模板 5（媒体英文）：「play some songs about {topic}」
    for topic in EN_TOPIC {
        push(
            &mut seeds,
            format!("play some songs about {topic}"),
            vec![topic],
        );
    }
    seeds
}

/// 按 id 升序排序后索引步进切分：每 5 条取 1 条进 held-out（~20%，确定性）。
fn split_train_heldout(cases: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut sorted: Vec<Value> = cases.to_vec();
    sorted.sort_by(|a, b| {
        a["id"]
            .as_str()
            .unwrap_or("")
            .cmp(b["id"].as_str().unwrap_or(""))
    });
    let (mut train, mut heldout) = (Vec::new(), Vec::new());
    for (i, c) in sorted.into_iter().enumerate() {
        if i % 5 == 4 {
            heldout.push(c);
        } else {
            train.push(c);
        }
    }
    (train, heldout)
}

/// BETA-24：模板种子 + 手写分片 → 切分写出 train/heldout。
/// 校验种子 id 全局唯一（模板 + 手写分片合并后）。撞 id → Err，fail-fast。
fn check_unique_ids(seeds: &[AugSeed]) -> Result<()> {
    let mut ids: Vec<&str> = seeds.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    anyhow::ensure!(ids.len() == before, "种子 id 重复");
    Ok(())
}

fn generate_lora_aug_keywords(
    handwritten: &Path,
    output_train: &Path,
    output_heldout: &Path,
) -> Result<()> {
    // 1. 模板种子 + 手写分片种子（分片按文件名升序，片内原序——同 assemble_coverage）
    let mut seeds = template_seeds();
    if handwritten.is_dir() {
        let mut shard_paths: Vec<PathBuf> = fs::read_dir(handwritten)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        shard_paths.sort();
        for p in &shard_paths {
            let shard: Vec<AugSeed> = serde_json::from_str(&fs::read_to_string(p)?)
                .with_context(|| format!("解析手写分片 {} 失败", p.display()))?;
            seeds.extend(shard);
        }
    }
    // 2. id 唯一性硬断言（手写分片 id 撞模板 id 时 fail-fast，防静默产错误数据）
    check_unique_ids(&seeds)?;
    // 3. 种子 → case；不触发 keywords 的丢弃并逐条打印（no silent caps）
    let mut cases = Vec::new();
    let mut dropped = Vec::new();
    for seed in &seeds {
        match aug_case_from_seed(seed) {
            Some(c) => cases.push(c),
            None => dropped.push(format!("{} | {}", seed.id, seed.query)),
        }
    }
    if !dropped.is_empty() {
        eprintln!("⚠️ {} 条种子不触发 keywords 待填，已丢弃：", dropped.len());
        for d in &dropped {
            eprintln!("  - {d}");
        }
    }
    // 4. 切分 + 写出
    let (train, heldout) = split_train_heldout(&cases);
    if let Some(dir) = output_train.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(output_train, serde_json::to_string_pretty(&train)? + "\n")?;
    fs::write(
        output_heldout,
        serde_json::to_string_pretty(&heldout)? + "\n",
    )?;
    eprintln!(
        "✅ lora-aug-keywords：seeds={}，cases={}（dropped {}），train={} → {}，heldout={} → {}",
        seeds.len(),
        cases.len(),
        dropped.len(),
        train.len(),
        output_train.display(),
        heldout.len(),
        output_heldout.display()
    );
    Ok(())
}

/// BETA-13：v0.9 = v0.5（逐字保留）+ coverage（手标 ground-truth）的确定性合并。
///
/// 纯确定性：v0.5 段保持原文件序在前，coverage 段保持原文件序在后，无排序/时间戳/随机。
/// 校验：id 全局唯一；coverage 每条 `expected_intent` 能反序列化为合法 `SearchIntent`。
fn generate_evals_v09(base: &Path, coverage: &Path, output: &Path) -> Result<()> {
    let base_cases =
        load_eval_cases(base).with_context(|| format!("读 v0.5 基底失败：{}", base.display()))?;
    let coverage_cases = load_eval_cases(coverage)
        .with_context(|| format!("读 coverage 失败：{}", coverage.display()))?;
    let (base_n, coverage_n) = (base_cases.len(), coverage_cases.len());

    let merged = merge_v09_cases(base_cases, coverage_cases)?;

    write_eval_cases(output, &merged)?;
    println!(
        "已生成 evals v0.9 fixture: {} ({} cases = {base_n} base + {coverage_n} coverage)",
        output.display(),
        merged.len(),
    );
    Ok(())
}

/// 合并 v0.5 + coverage 的纯逻辑（便于单测）：校验 coverage schema 合法性 + 全局 id 唯一，
/// 确定性拼接（base 在前、coverage 在后，各自保持入参序）。
fn merge_v09_cases(base: Vec<EvalCase>, coverage: Vec<EvalCase>) -> Result<Vec<EvalCase>> {
    // coverage 段逐条校验 expected_intent 为合法 SearchIntent（schema 门）。
    for case in &coverage {
        serde_json::from_value::<locifind_search_backend::SearchIntent>(
            case.expected_intent.clone(),
        )
        .with_context(|| {
            format!(
                "coverage case {} 的 expected_intent 不是合法 SearchIntent",
                case.id
            )
        })?;
    }

    let mut merged: Vec<EvalCase> = Vec::with_capacity(base.len() + coverage.len());
    merged.extend(base);
    merged.extend(coverage);

    // id 全局唯一断言。
    let mut seen = std::collections::HashSet::new();
    for case in &merged {
        if !seen.insert(case.id.as_str()) {
            anyhow::bail!("v0.9 case id 冲突：{}", case.id);
        }
    }
    Ok(merged)
}

/// 读取 eval case JSON 数组。
fn load_eval_cases(path: &Path) -> Result<Vec<EvalCase>> {
    let text = fs::read_to_string(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let cases: Vec<EvalCase> = serde_json::from_str(&text)
        .with_context(|| format!("无法解析 {} 为 case 数组", path.display()))?;
    Ok(cases)
}

/// 写出 eval case JSON 数组（pretty + 末尾换行，与 v0.5 一致）。
fn write_eval_cases(output: &Path, cases: &[EvalCase]) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("无法创建 fixture 目录")?;
    }
    let json = serde_json::to_string_pretty(cases)?;
    fs::write(output, format!("{json}\n")).context("无法写入 fixture JSON")
}

fn generate_evals_v05(output: &Path) -> Result<()> {
    let mut builder = EvalBuilder::default();
    builder.add_schema_seed_cases()?;
    builder.add_class1_synonym_cases();
    builder.fill_to_targets();
    builder.assert_targets()?;
    builder.write(output)?;
    println!(
        "已生成 evals v0.5 fixture: {} ({} cases)",
        output.display(),
        builder.cases.len()
    );
    Ok(())
}

impl EvalBuilder {
    fn add_schema_seed_cases(&mut self) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct SeedCase {
            id: String,
            title: String,
            intent: Value,
        }

        let seeds: Vec<SeedCase> = serde_json::from_str(include_str!(
            "../../../search-backends/common/tests/fixtures/cases.json"
        ))?;
        for seed in seeds {
            let language = seed
                .intent
                .get("language")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            self.add(
                format!("v05-schema-{}", seed.id),
                clean_query(&seed.title),
                &language,
                seed.intent,
            );
        }
        Ok(())
    }

    fn add_class1_synonym_cases(&mut self) {
        let sort_synonyms = [
            ("最大的", "zh"),
            ("最大", "zh"),
            ("最重", "zh"),
            ("体积最大", "zh"),
            ("biggest", "en"),
            ("largest", "en"),
        ];
        for (synonym, language) in sort_synonyms {
            let (file_query, media_query) = match language {
                "zh" => (
                    format!("找下载目录里{synonym}的 ppt"),
                    format!("找{synonym}的视频"),
                ),
                _ => (
                    format!("find the {synonym} ppt in downloads"),
                    format!("find the {synonym} video"),
                ),
            };
            self.add(
                file_id("class1-sort"),
                file_query,
                language,
                file_intent(
                    language,
                    None,
                    Some("presentation"),
                    Some(json!({"hint":"下载"})),
                    None,
                    None,
                    None,
                    Some("size_desc"),
                    None,
                ),
            );
            self.add(
                media_id("class1-sort"),
                media_query,
                language,
                media_intent(
                    language,
                    "video",
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("size_desc"),
                    None,
                ),
            );
        }

        let week_synonyms = [
            ("一周内", "zh", "last_7_days"),
            ("本周", "zh", "this_week"),
            ("近一周", "zh", "last_7_days"),
            ("this week", "en", "this_week"),
            ("past 7 days", "en", "last_7_days"),
        ];
        for (synonym, language, value) in week_synonyms {
            let (file_query, media_query) = match language {
                "zh" => (
                    format!("{synonym}编辑过的 ppt"),
                    format!("找{synonym}修改的视频"),
                ),
                _ => (
                    format!("find ppt modified {synonym}"),
                    format!("find videos modified {synonym}"),
                ),
            };
            let time = Some(json!({"type":"relative","value":value}));
            self.add(
                file_id("class1-week"),
                file_query,
                language,
                file_intent(
                    language,
                    None,
                    Some("presentation"),
                    None,
                    time.clone(),
                    None,
                    None,
                    Some("modified_desc"),
                    None,
                ),
            );
            self.add(
                media_id("class1-week"),
                media_query,
                language,
                media_intent(
                    language,
                    "video",
                    None,
                    None,
                    None,
                    time,
                    None,
                    None,
                    Some("modified_desc"),
                    None,
                ),
            );
        }

        let size_synonyms = [
            ("几个 G", "zh", 1.0, "GB"),
            ("200 MB 以上", "zh", 200.0, "MB"),
            (">100MB", "mixed", 100.0, "MB"),
            ("大文件", "zh", 100.0, "MB"),
        ];
        for (synonym, language, value, unit) in size_synonyms {
            let (file_query, media_query) = if language == "mixed" {
                (
                    format!("find 下载目录 里的 {synonym}"),
                    format!("find video {synonym}"),
                )
            } else {
                (
                    format!("找下载目录里{synonym}的文件"),
                    format!("找{synonym}的视频"),
                )
            };
            let size = Some(json!({"type":"greater_than","value":value,"unit":unit}));
            self.add(
                file_id("class1-size"),
                file_query,
                language,
                file_intent(
                    language,
                    None,
                    None,
                    Some(json!({"hint":"下载"})),
                    None,
                    None,
                    size.clone(),
                    Some("size_desc"),
                    None,
                ),
            );
            self.add(
                media_id("class1-size"),
                media_query,
                language,
                media_intent(
                    language,
                    "video",
                    None,
                    None,
                    None,
                    None,
                    None,
                    size,
                    Some("size_desc"),
                    None,
                ),
            );
        }
    }

    fn fill_to_targets(&mut self) {
        self.fill_file_search();
        self.fill_media_search();
        self.fill_file_action();
        self.fill_refine();
        self.fill_clarify();
    }

    fn add(&mut self, id: String, query: String, language: &str, expected_intent: Value) {
        let variant = expected_intent
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let variant = normalized_variant(variant);
        if self.bucket_count(variant, language) >= target_for(variant, language) {
            return;
        }
        self.variant_lang_counts
            .entry((variant, lang_static(language)))
            .and_modify(|count| *count += 1)
            .or_insert(1);
        self.cases.push(EvalCase {
            id: self.unique_id(&id),
            query,
            language: language.to_owned(),
            expected_intent,
        });
    }

    fn unique_id(&self, prefix: &str) -> String {
        format!("{prefix}-{:03}", self.cases.len() + 1)
    }

    fn bucket_count(&self, variant: &'static str, language: &str) -> usize {
        self.variant_lang_counts
            .get(&(variant, lang_static(language)))
            .copied()
            .unwrap_or(0)
    }

    fn write(&self, output: &Path) -> Result<()> {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).context("无法创建 eval fixture 目录")?;
        }
        let json = serde_json::to_string_pretty(&self.cases)?;
        fs::write(output, format!("{json}\n")).context("无法写入 eval fixture JSON")
    }

    fn assert_targets(&self) -> Result<()> {
        let total = self.cases.len();
        if total < 500 {
            anyhow::bail!("v0.5 fixture 数量不足：{total}");
        }
        for variant in [
            "file_search",
            "media_search",
            "file_action",
            "refine",
            "clarify",
        ] {
            for language in ["zh", "en", "mixed"] {
                let actual = self.bucket_count(normalized_variant(variant), language);
                let target = target_for(normalized_variant(variant), language);
                if actual != target {
                    anyhow::bail!(
                        "bucket {variant}/{language} 数量不匹配：actual={actual}, target={target}"
                    );
                }
            }
        }
        Ok(())
    }
}

impl EvalBuilder {
    fn fill_file_search(&mut self) {
        let zh = [
            ("找{time}{loc}的{kind}", "modified"),
            ("查找{loc}{size}的{kind}", "size"),
            ("找{time}{loc}名字里有「{kw}」的文件", "keyword"),
        ];
        let en = [
            ("find {kind} modified {time} in {loc}", "modified"),
            ("find {kind} {size} in {loc}", "size"),
            ("find files containing {kw} in {loc}", "keyword"),
        ];
        let mixed = [
            ("找 {loc} 里 {time} 改过的 {kind}", "modified"),
            ("find {loc} 里的 {size} {kind}", "size"),
            ("找最近的 {kw} {kind}", "keyword"),
        ];
        self.fill_file_language("zh", &zh);
        self.fill_file_language("en", &en);
        self.fill_file_language("mixed", &mixed);
    }

    fn fill_file_language(&mut self, language: &str, templates: &[(&str, &str)]) {
        let kinds = kind_specs(language);
        let times = time_specs(language);
        let locations = location_specs(language);
        let sizes = size_specs(language);
        let keywords = keyword_specs(language);
        let mut i = 0usize;
        while self.bucket_count("file_search", language) < target_for("file_search", language) {
            let (template, mode) = templates[i % templates.len()];
            let kind = &kinds[i % kinds.len()];
            let time = &times[(i / 2) % times.len()];
            let loc = &locations[(i / 3) % locations.len()];
            let size = &sizes[(i / 5) % sizes.len()];
            let kw = keywords[(i / 7) % keywords.len()];
            // v0.5：模板占位符存在性决定字段是否填入（避免 fixture 硬塞与 query 不符的字段）
            let has_time = template.contains("{time}");
            let has_loc = template.contains("{loc}");
            let has_kind = template.contains("{kind}");
            let query = template
                .replace("{kind}", kind.query)
                .replace("{time}", time.query)
                .replace("{loc}", loc.query)
                .replace("{size}", size.query)
                .replace("{kw}", kw);
            let time_json = if has_time {
                Some(json!({"type":"relative","value":time.value}))
            } else {
                None
            };
            let loc_json = if has_loc {
                Some(json!({"hint": loc.hint}))
            } else {
                None
            };
            // file_type / extensions 只在 query 含 {kind} 时填入（keyword 模板用"...的文件"
            // 不指定具体类型，不应硬塞 file_type）
            let file_type_arg = if has_kind { kind.file_type } else { None };
            let intent = match mode {
                "size" => file_intent(
                    language,
                    None,
                    file_type_arg,
                    loc_json,
                    None,
                    None,
                    Some(json!({"type":"greater_than","value":size.value,"unit":size.unit})),
                    Some("size_desc"),
                    None,
                ),
                "keyword" => file_intent(
                    language,
                    Some(vec![kw.to_owned()]),
                    file_type_arg,
                    loc_json,
                    time_json,
                    None,
                    None,
                    Some("modified_desc"),
                    None,
                ),
                _ => file_intent(
                    language,
                    None,
                    file_type_arg,
                    loc_json,
                    time_json,
                    None,
                    None,
                    Some("modified_desc"),
                    None,
                ),
            };
            self.add(file_id("template"), query, language, intent);
            i += 1;
        }
    }

    fn fill_media_search(&mut self) {
        for language in ["zh", "en", "mixed"] {
            let times = time_specs(language);
            let locations = location_specs(language);
            let sizes = size_specs(language);
            let media = media_specs(language);
            let mut i = 0usize;
            while self.bucket_count("media_search", language) < target_for("media_search", language)
            {
                let spec = &media[i % media.len()];
                let time = &times[(i / 2) % times.len()];
                let loc = &locations[(i / 3) % locations.len()];
                let size = &sizes[(i / 5) % sizes.len()];
                // v0.5：只在 query 模板含占位符时填入相应字段（之前模板硬塞所有字段会
                // 与 query 不一致，造成 partial diff）
                let has_time = spec.query.contains("{time}");
                let has_loc = spec.query.contains("{loc}");
                let query = spec
                    .query
                    .replace("{time}", time.query)
                    .replace("{loc}", loc.query)
                    .replace("{size}", size.query);
                let size_json = if spec.with_size {
                    Some(json!({"type":"greater_than","value":size.value,"unit":size.unit}))
                } else {
                    None
                };
                let time_json = if has_time {
                    Some(json!({"type":"relative","value":time.value}))
                } else {
                    None
                };
                let loc_json = if has_loc {
                    Some(json!({"hint": loc.hint}))
                } else {
                    None
                };
                // v0.5：Screenshot + 时间词 → created_time（语义上是"截图创建时间"），
                // 其他媒体类型按 modified_time。
                let is_screenshot = spec.media_type == "screenshot";
                let (created_json, modified_json) = if is_screenshot {
                    (time_json.clone(), None)
                } else {
                    (None, time_json.clone())
                };
                // sort 优先级：size > created (screenshot) > modified > relevance
                let sort = if spec.with_size {
                    Some("size_desc")
                } else if is_screenshot && time_json.is_some() {
                    Some("created_desc")
                } else if time_json.is_some() {
                    Some("modified_desc")
                } else {
                    Some("relevance_desc")
                };
                let intent = media_intent(
                    language,
                    spec.media_type,
                    spec.artist,
                    spec.title,
                    loc_json,
                    modified_json,
                    created_json,
                    size_json,
                    sort,
                    None,
                );
                self.add(media_id("template"), query, language, intent);
                i += 1;
            }
        }
    }

    fn fill_file_action(&mut self) {
        for language in ["zh", "en", "mixed"] {
            let mut i = 0usize;
            while self.bucket_count("file_action", language) < target_for("file_action", language) {
                let index = (i % 5) + 1;
                let (query, action, requires_confirmation, destination, new_name) =
                    action_case(language, i, index);
                let intent = json!({
                    "schema_version":"1.0",
                    "intent":"file_action",
                    "language":language,
                    "action":action,
                    "target_ref":{"source":"last_results","selector":{"type":"index","value":index}},
                    "requires_confirmation":requires_confirmation
                });
                let mut intent = intent;
                if let Some(dest) = destination {
                    intent["destination"] = json!(dest);
                }
                if let Some(name) = new_name {
                    intent["new_name"] = json!(name);
                }
                self.add(action_id("template"), query, language, intent);
                i += 1;
            }
        }
    }

    fn fill_refine(&mut self) {
        for language in ["zh", "en", "mixed"] {
            let mut i = 0usize;
            while self.bucket_count("refine", language) < target_for("refine", language) {
                let (query, delta, clear) = refine_case(language, i);
                let mut intent = json!({
                    "schema_version":"1.0",
                    "intent":"refine",
                    "language":language,
                    "base_ref":"last_intent",
                    "delta":delta
                });
                if let Some(clear) = clear {
                    intent["clear"] = json!(clear);
                }
                self.add(refine_id("template"), query, language, intent);
                i += 1;
            }
        }
    }

    fn fill_clarify(&mut self) {
        for language in ["zh", "en", "mixed"] {
            let mut i = 0usize;
            while self.bucket_count("clarify", language) < target_for("clarify", language) {
                let (query, reason, question, options) = clarify_case(language, i);
                self.add(
                    clarify_id("template"),
                    query,
                    language,
                    json!({
                        "schema_version":"1.0",
                        "intent":"clarify",
                        "language":language,
                        "reason":reason,
                        "question":question,
                        "options":options
                    }),
                );
                i += 1;
            }
        }
    }
}

struct KindSpec {
    query: &'static str,
    file_type: Option<&'static str>,
}

struct TimeSpec {
    query: &'static str,
    value: &'static str,
}

struct LocationSpec {
    query: &'static str,
    hint: &'static str,
}

struct SizeSpec {
    query: &'static str,
    value: f64,
    unit: &'static str,
}

struct MediaSpec {
    query: &'static str,
    media_type: &'static str,
    artist: Option<&'static str>,
    title: Option<&'static str>,
    with_size: bool,
}

fn target_for(variant: &'static str, language: &str) -> usize {
    match (variant, language) {
        ("file_search", "zh") => 100,
        ("file_search", "en") => 60,
        ("file_search", "mixed") => 40,
        ("media_search", "zh") => 55,
        ("media_search", "en") => 25,
        ("media_search", "mixed") => 20,
        ("file_action", "zh") => 35,
        ("file_action", "en") => 30,
        ("file_action", "mixed") => 15,
        ("refine", "zh") => 40,
        ("refine", "en") => 20,
        ("refine", "mixed") => 20,
        ("clarify", "zh") => 20,
        ("clarify", "en") => 15,
        ("clarify", "mixed") => 5,
        _ => 0,
    }
}

fn normalized_variant(intent: &str) -> &'static str {
    match intent {
        "file_search" | "FileSearch" => "file_search",
        "media_search" | "MediaSearch" => "media_search",
        "file_action" | "FileAction" => "file_action",
        "refine" | "Refine" => "refine",
        "clarify" | "Clarify" => "clarify",
        _ => "unknown",
    }
}

fn lang_static(language: &str) -> &'static str {
    match language {
        "zh" => "zh",
        "en" => "en",
        "mixed" => "mixed",
        _ => "unknown",
    }
}

fn clean_query(title: &str) -> String {
    let mut s = title.to_owned();
    if let Some(p) = s.find('（') {
        s.truncate(p);
    }
    if let Some(p) = s.find('(') {
        s.truncate(p);
    }
    for prefix in ["refine：", "refine: "] {
        if let Some(stripped) = s.strip_prefix(prefix) {
            s = stripped.to_owned();
        }
    }
    s.trim().to_owned()
}

fn file_intent(
    language: &str,
    keywords: Option<Vec<String>>,
    file_type: Option<&str>,
    location: Option<Value>,
    modified_time: Option<Value>,
    created_time: Option<Value>,
    size: Option<Value>,
    sort: Option<&str>,
    extensions: Option<Vec<&str>>,
) -> Value {
    let mut intent = json!({
        "schema_version":"1.0",
        "intent":"file_search",
        "language":language
    });
    if let Some(keywords) = keywords {
        intent["keywords"] = json!(keywords);
    }
    if let Some(extensions) = extensions {
        intent["extensions"] = json!(extensions);
    } else if let Some(file_type) = file_type {
        if let Some(exts) = default_extensions(file_type) {
            intent["extensions"] = json!(exts);
        }
    }
    if let Some(file_type) = file_type {
        intent["file_type"] = json!(file_type);
    }
    if let Some(location) = location {
        intent["location"] = location;
    }
    if let Some(modified_time) = modified_time {
        intent["modified_time"] = modified_time;
    }
    if let Some(created_time) = created_time {
        intent["created_time"] = created_time;
    }
    if let Some(size) = size {
        intent["size"] = size;
    }
    if let Some(sort) = sort {
        intent["sort"] = json!(sort);
    }
    intent
}

fn media_intent(
    language: &str,
    media_type: &str,
    artist: Option<&str>,
    title: Option<&str>,
    location: Option<Value>,
    modified_time: Option<Value>,
    created_time: Option<Value>,
    size: Option<Value>,
    sort: Option<&str>,
    keywords: Option<Vec<String>>,
) -> Value {
    let mut intent = json!({
        "schema_version":"1.0",
        "intent":"media_search",
        "language":language,
        "media_type":media_type
    });
    if let Some(artist) = artist {
        intent["artist"] = json!(artist);
    }
    if let Some(title) = title {
        intent["title"] = json!(title);
    }
    if let Some(location) = location {
        intent["location"] = location;
    }
    if let Some(modified_time) = modified_time {
        intent["modified_time"] = modified_time;
    }
    if let Some(created_time) = created_time {
        intent["created_time"] = created_time;
    }
    if let Some(size) = size {
        intent["size"] = size;
    }
    if let Some(sort) = sort {
        intent["sort"] = json!(sort);
    }
    if let Some(keywords) = keywords {
        intent["keywords"] = json!(keywords);
    }
    intent
}

fn default_extensions(file_type: &str) -> Option<Vec<&'static str>> {
    match file_type {
        "presentation" => Some(vec!["ppt", "pptx"]),
        "spreadsheet" => Some(vec!["xls", "xlsx"]),
        "document" => Some(vec!["pdf"]),
        "archive" => Some(vec!["zip"]),
        "video" => Some(vec!["mp4"]),
        _ => None,
    }
}

fn kind_specs(language: &str) -> Vec<KindSpec> {
    // v0.5 dual-route 修复：从 file_search 的 {kind} 集合移除 zh="视频" / en="videos"
    // —— 这两个语义媒体词与 media_specs 重叠会生成相同 query 但不同 expected variant，
    // 造成 fixture 内部 dual-route artifact（22 fail 中 20 都源于此）。
    // 保留 mixed="mp4"（扩展名，fixture 设计上故意区分"扩展名 → file_search"vs"媒体语义词 → media_search"）。
    match language {
        "en" => vec![
            KindSpec {
                query: "ppt",
                file_type: Some("presentation"),
            },
            KindSpec {
                query: "pdf",
                file_type: Some("document"),
            },
            KindSpec {
                query: "Excel",
                file_type: Some("spreadsheet"),
            },
            KindSpec {
                query: "zip files",
                file_type: Some("archive"),
            },
        ],
        "mixed" => vec![
            KindSpec {
                query: "ppt",
                file_type: Some("presentation"),
            },
            KindSpec {
                query: "PDF",
                file_type: Some("document"),
            },
            KindSpec {
                query: "Excel",
                file_type: Some("spreadsheet"),
            },
            KindSpec {
                query: "mp4",
                file_type: Some("video"),
            },
        ],
        _ => vec![
            KindSpec {
                query: "ppt",
                file_type: Some("presentation"),
            },
            KindSpec {
                query: "pdf",
                file_type: Some("document"),
            },
            KindSpec {
                query: "Excel",
                file_type: Some("spreadsheet"),
            },
            KindSpec {
                query: "zip",
                file_type: Some("archive"),
            },
        ],
    }
}

fn time_specs(language: &str) -> Vec<TimeSpec> {
    match language {
        "en" => vec![
            TimeSpec {
                query: "yesterday",
                value: "yesterday",
            },
            TimeSpec {
                query: "last week",
                value: "last_week",
            },
            TimeSpec {
                query: "past 7 days",
                value: "last_7_days",
            },
            TimeSpec {
                query: "this week",
                value: "this_week",
            },
            TimeSpec {
                query: "last month",
                value: "last_month",
            },
        ],
        "mixed" => vec![
            TimeSpec {
                query: "yesterday",
                value: "yesterday",
            },
            TimeSpec {
                query: "上周",
                value: "last_week",
            },
            TimeSpec {
                query: "past 7 days",
                value: "last_7_days",
            },
            TimeSpec {
                query: "本周",
                value: "this_week",
            },
        ],
        _ => vec![
            TimeSpec {
                query: "昨天",
                value: "yesterday",
            },
            TimeSpec {
                query: "上周",
                value: "last_week",
            },
            TimeSpec {
                query: "最近一周",
                value: "last_7_days",
            },
            TimeSpec {
                query: "本周",
                value: "this_week",
            },
            TimeSpec {
                query: "上个月",
                value: "last_month",
            },
        ],
    }
}

fn location_specs(language: &str) -> Vec<LocationSpec> {
    match language {
        "en" => vec![
            LocationSpec {
                query: "downloads",
                hint: "downloads",
            },
            LocationSpec {
                query: "desktop",
                hint: "desktop",
            },
            LocationSpec {
                query: "documents",
                hint: "documents",
            },
        ],
        "mixed" => vec![
            LocationSpec {
                query: "downloads",
                hint: "downloads",
            },
            LocationSpec {
                query: "桌面",
                hint: "桌面",
            },
            LocationSpec {
                query: "Documents",
                hint: "documents",
            },
        ],
        _ => vec![
            LocationSpec {
                query: "下载目录",
                hint: "下载",
            },
            LocationSpec {
                query: "桌面",
                hint: "桌面",
            },
            LocationSpec {
                query: "文稿",
                hint: "文稿",
            },
        ],
    }
}

fn size_specs(language: &str) -> Vec<SizeSpec> {
    match language {
        "en" => vec![
            SizeSpec {
                query: "over 100MB",
                value: 100.0,
                unit: "MB",
            },
            SizeSpec {
                query: "larger than 1GB",
                value: 1.0,
                unit: "GB",
            },
            SizeSpec {
                query: ">200MB",
                value: 200.0,
                unit: "MB",
            },
        ],
        "mixed" => vec![
            SizeSpec {
                query: ">100MB",
                value: 100.0,
                unit: "MB",
            },
            SizeSpec {
                query: "1 GB 以上",
                value: 1.0,
                unit: "GB",
            },
            SizeSpec {
                query: "大文件",
                value: 100.0,
                unit: "MB",
            },
        ],
        _ => vec![
            SizeSpec {
                query: "大于 100MB",
                value: 100.0,
                unit: "MB",
            },
            SizeSpec {
                query: "超过 1GB",
                value: 1.0,
                unit: "GB",
            },
            SizeSpec {
                query: "200 MB 以上",
                value: 200.0,
                unit: "MB",
            },
            SizeSpec {
                query: "大文件",
                value: 100.0,
                unit: "MB",
            },
        ],
    }
}

fn keyword_specs(language: &str) -> Vec<&'static str> {
    match language {
        "en" => vec![
            "synthetic-budget",
            "synthetic-report",
            "synthetic-plan",
            "synthetic-notes",
        ],
        "mixed" => vec!["budget", "synthetic-plan", "会议", "invoice"],
        _ => vec!["预算", "会议纪要", "合成报告", "synthetic-plan"],
    }
}

fn media_specs(language: &str) -> Vec<MediaSpec> {
    match language {
        "en" => vec![
            MediaSpec {
                query: "find videos modified {time} in {loc}",
                media_type: "video",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "find {size} videos in {loc}",
                media_type: "video",
                artist: None,
                title: None,
                with_size: true,
            },
            MediaSpec {
                query: "find screenshots from {time} in {loc}",
                media_type: "screenshot",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "find songs by synthetic-artist",
                media_type: "audio",
                artist: Some("synthetic-artist"),
                title: None,
                with_size: false,
            },
        ],
        "mixed" => vec![
            MediaSpec {
                query: "find {time} 的 video in {loc}",
                media_type: "video",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "找 {loc} 里的 {size} video",
                media_type: "video",
                artist: None,
                title: None,
                with_size: true,
            },
            MediaSpec {
                query: "find {time} 的截图",
                media_type: "screenshot",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "find synthetic-artist 的歌",
                media_type: "audio",
                artist: Some("synthetic-artist"),
                title: None,
                with_size: false,
            },
        ],
        _ => vec![
            MediaSpec {
                query: "找{time}{loc}的视频",
                media_type: "video",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "找{loc}{size}的视频",
                media_type: "video",
                artist: None,
                title: None,
                with_size: true,
            },
            MediaSpec {
                query: "找{time}截的 synthetic-receipt 截图",
                media_type: "screenshot",
                artist: None,
                title: None,
                with_size: false,
            },
            MediaSpec {
                query: "找 synthetic-artist 的歌",
                media_type: "audio",
                artist: Some("synthetic-artist"),
                title: None,
                with_size: false,
            },
        ],
    }
}

fn action_case(
    language: &str,
    index: usize,
    target_index: usize,
) -> (
    String,
    &'static str,
    bool,
    Option<&'static str>,
    Option<&'static str>,
) {
    let action = index % 5;
    match (language, action) {
        ("en", 0) => (
            format!("open the {target_index} result"),
            "open",
            false,
            None,
            None,
        ),
        ("en", 1) => (
            format!("show the {target_index} result in Finder"),
            "locate",
            false,
            None,
            None,
        ),
        ("en", 2) => (
            format!("copy the {target_index} result to desktop"),
            "copy",
            true,
            Some("~/Desktop"),
            None,
        ),
        ("en", 3) => (
            format!("move the {target_index} result to documents"),
            "move",
            true,
            Some("~/Documents"),
            None,
        ),
        ("en", _) => (
            format!("rename the {target_index} result to synthetic-final"),
            "rename",
            true,
            None,
            Some("synthetic-final"),
        ),
        ("mixed", 0) => (
            format!("open 第{target_index}个"),
            "open",
            false,
            None,
            None,
        ),
        ("mixed", 1) => (
            format!("在 Finder 显示第{target_index}个"),
            "locate",
            false,
            None,
            None,
        ),
        ("mixed", 2) => (
            format!("copy 第{target_index}个到 desktop"),
            "copy",
            true,
            Some("~/Desktop"),
            None,
        ),
        ("mixed", 3) => (
            format!("move 第{target_index}个到 Documents"),
            "move",
            true,
            Some("~/Documents"),
            None,
        ),
        ("mixed", _) => (
            format!("把第{target_index}个 rename 为 synthetic-final"),
            "rename",
            true,
            None,
            Some("synthetic-final"),
        ),
        (_, 0) => (format!("打开第{target_index}个"), "open", false, None, None),
        (_, 1) => (
            format!("在访达里显示第{target_index}个"),
            "locate",
            false,
            None,
            None,
        ),
        (_, 2) => (
            format!("把第{target_index}个复制到桌面"),
            "copy",
            true,
            Some("~/Desktop"),
            None,
        ),
        (_, 3) => (
            format!("把第{target_index}个移动到文稿"),
            "move",
            true,
            Some("~/Documents"),
            None,
        ),
        (_, _) => (
            format!("把第{target_index}个改名为 synthetic-final"),
            "rename",
            true,
            None,
            Some("synthetic-final"),
        ),
    }
}

fn refine_case(language: &str, index: usize) -> (String, Value, Option<Vec<&'static str>>) {
    match (language, index % 6) {
        ("en", 0) => (
            "show only pdf ones".to_owned(),
            json!({"extensions":["pdf"],"file_type":"document"}),
            None,
        ),
        ("en", 1) => (
            "only in downloads".to_owned(),
            json!({"location":{"hint":"downloads"}}),
            None,
        ),
        ("en", 2) => (
            "exclude videos".to_owned(),
            json!({"exclude_file_type":["video"]}),
            None,
        ),
        ("en", 3) => ("sort by size".to_owned(), json!({"sort":"size_desc"}), None),
        ("en", 4) => (
            "clear the location limit".to_owned(),
            json!({}),
            Some(vec!["location"]),
        ),
        ("en", _) => (
            "limit to last week".to_owned(),
            json!({"modified_time":{"type":"relative","value":"last_week"}}),
            None,
        ),
        ("mixed", 0) => (
            "只看 PDF ones".to_owned(),
            json!({"extensions":["pdf"],"file_type":"document"}),
            None,
        ),
        ("mixed", 1) => (
            "only downloads 里的".to_owned(),
            json!({"location":{"hint":"downloads"}}),
            None,
        ),
        ("mixed", 2) => (
            "排除 video".to_owned(),
            json!({"exclude_file_type":["video"]}),
            None,
        ),
        ("mixed", 3) => (
            "sort by size 倒序".to_owned(),
            json!({"sort":"size_desc"}),
            None,
        ),
        ("mixed", 4) => (
            "不限制 downloads 了".to_owned(),
            json!({}),
            Some(vec!["location"]),
        ),
        ("mixed", _) => (
            "只看 last week 的".to_owned(),
            json!({"modified_time":{"type":"relative","value":"last_week"}}),
            None,
        ),
        (_, 0) => (
            "只看 pdf".to_owned(),
            json!({"extensions":["pdf"],"file_type":"document"}),
            None,
        ),
        (_, 1) => (
            "只看下载目录里的".to_owned(),
            json!({"location":{"hint":"下载"}}),
            None,
        ),
        (_, 2) => (
            "排除视频".to_owned(),
            json!({"exclude_file_type":["video"]}),
            None,
        ),
        (_, 3) => ("按大小倒序".to_owned(), json!({"sort":"size_desc"}), None),
        (_, 4) => (
            "不限制下载目录了".to_owned(),
            json!({}),
            Some(vec!["location"]),
        ),
        (_, _) => (
            "只看上周修改的".to_owned(),
            json!({"modified_time":{"type":"relative","value":"last_week"}}),
            None,
        ),
    }
}

fn clarify_case(
    language: &str,
    index: usize,
) -> (String, &'static str, &'static str, Vec<&'static str>) {
    match (language, index % 4) {
        ("en", 0) => ("find recent".to_owned(), "ambiguous_time", "Which recent time range should I use?", vec!["today", "past 3 days", "past week", "past month"]),
        ("en", 1) => ("delete everything".to_owned(), "unsafe_action", "Delete is not supported in MVP. Show files instead?", vec!["show in Finder", "cancel"]),
        ("en", 2) => ("find files inside synthetic-place".to_owned(), "ambiguous_location", "Which location should I search?", vec!["all files", "downloads", "documents", "desktop"]),
        ("en", _) => ("copy all of them".to_owned(), "ambiguous_action", "Do you want to act on all previous results?", vec!["confirm all", "choose some", "cancel"]),
        ("mixed", 0) => ("找 recent 的".to_owned(), "ambiguous_time", "你说的「recent」是指哪个时间范围？", vec!["今天", "过去 3 天", "过去一周", "过去一个月"]),
        ("mixed", 1) => ("delete 全部".to_owned(), "unsafe_action", "删除操作 MVP 暂不支持。是否改为显示文件？", vec!["显示文件", "取消"]),
        ("mixed", 2) => ("找 synthetic-place 里的文件".to_owned(), "ambiguous_location", "没找到对应的目录。要不要在哪个范围内搜索？", vec!["全盘搜索", "下载", "文稿", "桌面"]),
        ("mixed", _) => ("copy 全部结果".to_owned(), "ambiguous_action", "要对上一轮的全部结果执行此操作吗？", vec!["确认全部", "只选择部分", "取消"]),
        (_, 0) => ("找最近的".to_owned(), "ambiguous_time", "你说的「最近」是指最近几天？", vec!["今天", "过去 3 天", "过去一周", "过去一个月"]),
        (_, 1) => ("全部删掉".to_owned(), "unsafe_action", "删除操作会移到回收站，且 MVP 暂不支持。是否改为在访达 / 资源管理器中显示，由你手动操作？", vec!["在访达/资源管理器中显示", "取消"]),
        (_, 2) => ("找 synthetic-place 里的文件".to_owned(), "ambiguous_location", "没找到对应的目录。要不要在哪个范围内搜索？", vec!["全盘搜索", "下载", "文稿", "桌面"]),
        (_, _) => ("把这些都复制到桌面".to_owned(), "ambiguous_action", "要对上一轮的全部结果执行此操作吗？请先确认目标文件列表。", vec!["确认全部", "只选择部分", "取消"]),
    }
}

fn file_id(prefix: &str) -> String {
    format!("v05-file-{prefix}")
}

fn media_id(prefix: &str) -> String {
    format!("v05-media-{prefix}")
}

fn action_id(prefix: &str) -> String {
    format!("v05-action-{prefix}")
}

fn refine_id(prefix: &str) -> String {
    format!("v05-refine-{prefix}")
}

fn clarify_id(prefix: &str) -> String {
    format!("v05-clarify-{prefix}")
}

fn generate(root: &Path) -> Result<()> {
    println!("正在生成 fixture 到: {}", root.display());
    fs::create_dir_all(root).context("无法创建根目录")?;

    for def in FIXTURES {
        let path = root.join(def.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("无法创建子目录")?;
        }

        if path.exists() {
            // 简单幂等检查：如果大小一致且已存在，跳过（或者可以检查 mtime，但简单起见直接覆盖或跳过）
            // 这里选择跳过，如果需要重新生成可以 --clean
            println!("跳过已存在文件: {}", def.path);
            continue;
        }

        create_synthetic_file(&path, def.size_mb)?;
        set_file_mtime(&path, def.modified_days_ago)?;
        println!("已生成: {}", def.path);
    }

    Ok(())
}

fn clean(root: &Path) -> Result<()> {
    if root.exists() {
        println!("正在清理 fixture 目录: {}", root.display());
        fs::remove_dir_all(root).context("清理失败")?;
    }
    Ok(())
}

fn create_synthetic_file(path: &Path, size_mb: u64) -> Result<()> {
    // 跨平台：`File::set_len` 扩展文件到指定大小（NTFS / APFS 上为稀疏，按零读取），
    // 取代原 macOS 专用的 `dd if=/dev/zero`。
    let file = fs::File::create(path).context("无法创建文件")?;
    if size_mb > 0 {
        let bytes = size_mb
            .checked_mul(1024 * 1024)
            .context("文件大小溢出 u64")?;
        file.set_len(bytes).context("设置文件大小失败")?;
    }
    Ok(())
}

fn set_file_mtime(path: &Path, days_ago: i64) -> Result<()> {
    // 跨平台：`File::set_modified`（std，自 1.75 稳定），取代原 macOS 专用的 `touch -t`。
    let dt: DateTime<Utc> = Utc::now() - Duration::days(days_ago);
    let mtime: std::time::SystemTime = dt.into();

    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .context("打开文件以设置 mtime 失败")?;
    file.set_modified(mtime).context("设置 mtime 失败")?;

    Ok(())
}

#[cfg(test)]
mod lora_aug_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn aug_case_from_seed_rejects_populated_keywords_problem4_shape() {
        // BETA-13-G13：fill-empty-only 取代旧 BETA-24「追加已填 keywords」。
        // 问题 4 同型（parser 已抽到「运维」、仅丢「会议纪要」）现不再标 keywords fillable
        // → aug_case_from_seed 丢弃该种子（返回 None）。追加已填 keywords 的训练目标已废弃
        // （实测在受闸评测集净伤害 14、净收益 0：模型把 file_type 词回声进已对 keywords）。
        // keyword 补全的合法训练样本收敛到媒体臂 parser 留空 keywords 的情形，见
        // aug_case_from_seed_media_arm_keywords_equals_missing。
        let seed = AugSeed {
            id: "aug-test-001".to_owned(),
            query: "2025年的会议纪要文件名包含运维".to_owned(),
            missing_keywords: vec!["会议纪要".to_owned()],
        };
        assert!(
            aug_case_from_seed(&seed).is_none(),
            "parser 已抽出 keywords 的种子应被丢弃（fill-empty-only）"
        );
    }

    #[test]
    fn aug_case_from_seed_rejects_covered_query() {
        // parser 已全覆盖的 query 不触发 keywords → 应被丢弃（返回 None）
        let seed = AugSeed {
            id: "aug-test-002".to_owned(),
            query: "上周的pdf".to_owned(),
            missing_keywords: vec![],
        };
        assert!(aug_case_from_seed(&seed).is_none());
    }

    #[test]
    fn aug_case_from_seed_media_arm_keywords_equals_missing() {
        // 媒体臂 draft keywords 为空 → 补入的 missing 即全部 keywords（与文件臂「追加」路径不同）
        let seed = AugSeed {
            id: "aug-test-media".to_owned(),
            query: "播放毕业旅行相关的歌".to_owned(),
            missing_keywords: vec!["毕业旅行".to_owned()],
        };
        let case = aug_case_from_seed(&seed).expect("媒体内容词遗漏应触发 keywords");
        assert_eq!(case["variant"], "MediaSearch");
        let kws: Vec<String> =
            serde_json::from_value(case["expected_intent"]["keywords"].clone()).unwrap();
        assert_eq!(
            kws,
            vec!["毕业旅行".to_owned()],
            "媒体臂 keywords 应等于 missing"
        );
    }

    #[test]
    fn check_unique_ids_rejects_collision() {
        // 手写分片 id 撞模板 id → fail-fast（防静默产出错误训练数据的关键防线）
        let seeds = vec![
            AugSeed {
                id: "aug-tpl-001".to_owned(),
                query: "a".to_owned(),
                missing_keywords: vec![],
            },
            AugSeed {
                id: "aug-tpl-001".to_owned(),
                query: "b".to_owned(),
                missing_keywords: vec![],
            },
        ];
        assert!(check_unique_ids(&seeds).is_err(), "撞 id 应报错");
        // 唯一 id 不报错
        let ok = vec![AugSeed {
            id: "aug-tpl-001".to_owned(),
            query: "a".to_owned(),
            missing_keywords: vec![],
        }];
        assert!(check_unique_ids(&ok).is_ok());
    }

    #[test]
    fn template_seeds_are_deterministic() {
        let a = template_seeds();
        let b = template_seeds();
        assert_eq!(a.len(), b.len());
        assert!(a
            .iter()
            .zip(&b)
            .all(|(x, y)| x.id == y.id && x.query == y.query));
        // id 唯一
        let mut ids: Vec<&str> = a.iter().map(|s| s.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), a.len(), "模板 seed id 必须唯一");
    }

    #[test]
    fn split_train_heldout_no_overlap_and_deterministic() {
        let cases: Vec<Value> = (0..50)
            .map(|i| serde_json::json!({"id": format!("aug-{i:03}")}))
            .collect();
        let (train, heldout) = split_train_heldout(&cases);
        assert_eq!(train.len() + heldout.len(), cases.len());
        assert_eq!(heldout.len(), 10, "20% held-out");
        let train_ids: std::collections::HashSet<&str> =
            train.iter().map(|c| c["id"].as_str().unwrap()).collect();
        assert!(
            heldout
                .iter()
                .all(|c| !train_ids.contains(c["id"].as_str().unwrap())),
            "train/heldout 零重叠"
        );
        let (train2, heldout2) = split_train_heldout(&cases);
        assert_eq!(train, train2);
        assert_eq!(heldout, heldout2);
    }
}
