# BETA-24 LoRA 重训含 keywords 补全样本 + MediaSearch 内容词覆盖 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重训 Qwen3-0.6B LoRA 使模型能补全 keywords 字段（问题 4 最后一公里），并启用 MediaSearch 臂内容词覆盖检测。

**Architecture:** 方案 A——数据是唯一训练变量。先落地 MediaSearch 臂触发器（纯 parser 函数 + 媒体噪声词表 + fire-rate 误触发门），再新造 keywords 补全训练数据（程序模板 + Claude 手写，`expected_intent = parser draft ⊕ 补齐 keywords`，held-out 切片永不进训练），复用既有 `build_lora_dataset → prepare_main_data.py → mlx-lm LoRA → GGUF Q4_K_M` 管线重训，三层验收（held-out 量化 / v0.9 零回退 / 真机问题 4）。

**Tech Stack:** Rust（intent-parser / evals）、Python（mlx-lm 0.29.1, llama.cpp 工具链）、bash。

**对应 spec:** [2026-06-13-beta-24-lora-retrain-keywords-design.md](../specs/2026-06-13-beta-24-lora-retrain-keywords-design.md)

**硬红线（每个 task 的验证步都要守）：**
- `parse()` 本体零改动 → v0.5 473 / v0.9 726 parser-only byte-equal 不动。
- 每 task 完成跑 `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace`（platform-macos 在 Windows 的 2 个预存失败不适用本机；macOS 上应全绿）。
- 训练超参 / 基座（Qwen3-0.6B + v1 配方）不动。

---

### Task 1: MediaSearch 臂启用内容词覆盖检测（媒体噪声词表）

**Files:**
- Modify: `packages/intent-parser/src/fallback.rs`（`MEDIA_COVERAGE_NOISE_WORDS` 常量 + `has_uncovered_content` 媒体分支 + `analyze_structural_omissions` MediaSearch 臂 + 测试）

**背景：** BETA-23 砍掉媒体臂的原因是 FileSearch 剥离词表未剥媒体框架词（songs by / 一首 / 无损 / 时长），v0.9 媒体模板误触发 +4.7%。本 task 用媒体专用噪声词表解决。现有测试 `media_search_keywords_omission_disabled`（fallback.rs:771）固化了「禁用」决策，本 task 把它翻转为「真遗漏触发」。

- [ ] **Step 1: 探针选定正例 query**

媒体臂正例必须是「parser 真漏内容词」的查询。先用临时测试探针确认候选 query 的 parser 行为（看 keywords/artist/title/genre 是否覆盖了内容词）：

```bash
cd /Users/alice/Work/LocalFind
cat > /tmp/probe.rs <<'EOF'
// 临时探针：加到 fallback.rs tests 模块末尾跑一次后删除
#[test]
fn probe_media_candidates() {
    for q in [
        "找几首关于毕业旅行的歌",
        "play some indie tracks about rainy nights",
        "关于夏天的雨的歌曲",
        "songs about long road trips",
    ] {
        let parsed = parse_with_signals(q);
        eprintln!("q={q} intent={:?}", parsed.intent);
        eprintln!("  residual={:?}", crate::parsers::file_search::residual_content_segments(q));
    }
}
EOF
# 手动把上面测试体粘进 fallback.rs tests 模块，然后：
cargo test -p locifind-intent-parser probe_media_candidates -- --nocapture
```

从输出中选 2 个正例（内容词不在任何 covered 字段里、解析为 MediaSearch）：一个中文一个英文。**跑完删除探针测试。** 若上述候选全被 parser 覆盖，调整 query（加更生僻的合成主题词如「毕业旅行」「旧城改造」）直到找到真遗漏样本。

- [ ] **Step 2: 改写测试（失败先行）**

把 `media_search_keywords_omission_disabled`（fallback.rs:767-784）整体替换为（`<正例zh>` / `<正例en>` 用 Step 1 选定的 query 替换）：

```rust
    /// BETA-24：MediaSearch 臂启用 keywords 覆盖检测——parser 真漏内容词的
    /// 媒体查询应触发（媒体框架噪声词表已叠加剥离，BETA-23 +4.7% 误触发由词表解决）。
    #[test]
    fn media_search_keywords_omission_fires_on_true_omission() {
        for q in ["<正例zh>", "<正例en>"] {
            let parsed = parse_with_signals(q);
            assert!(
                matches!(parsed.intent, SearchIntent::MediaSearch(_)),
                "前提：{q} 应解析为 MediaSearch，实际 intent={:?}",
                parsed.intent
            );
            let missing = analyze_structural_omissions(&parsed);
            assert!(
                missing.contains(&"keywords"),
                "媒体臂真遗漏应触发 keywords，q={q} missing={missing:?}"
            );
        }
    }

    /// BETA-24：媒体框架词（songs by / 一首 / 无损 / 时长…）不得令媒体臂误触发。
    /// 反例集来自 BETA-23 review 实测误触发样本 + 常见媒体模板。
    #[test]
    fn media_framework_words_do_not_trigger_keywords_omission() {
        let mut failed: Vec<String> = Vec::new();
        for q in [
            "songs by Adele",
            "时长不到3分钟的歌曲",
            "找邓紫棋的歌曲",
            "周杰伦的歌",
            "来一首无损音质的单曲",
            "play some music",
            "高品质的专辑",
            "最近添加的playlist",
        ] {
            let parsed = parse_with_signals(q);
            let missing = analyze_structural_omissions(&parsed);
            if missing.contains(&"keywords") {
                failed.push(format!("query={q} missing={missing:?}"));
            }
        }
        assert!(failed.is_empty(), "媒体框架词误触发：{failed:#?}");
    }
```

注意：`covered_queries_do_not_trigger_keywords_omission`（fallback.rs:744）反例集中的媒体样本（songs by Adele 等）与新反例测试重叠，**保留原测试不动**（它守的是整体反例集，语义仍成立）。

- [ ] **Step 3: 跑测试确认失败**

```bash
cargo test -p locifind-intent-parser media_search_keywords -- --nocapture
```

预期：`media_search_keywords_omission_fires_on_true_omission` FAIL（媒体臂还没启用，missing 不含 keywords）；`media_framework_words_do_not_trigger_keywords_omission` PASS（臂关着当然不触发——它在 Step 4 后继续守住）。

- [ ] **Step 4: 实现**

在 `COVERAGE_NOISE_WORDS`（fallback.rs:93）后新增常量（**按字符数降序排列**，与 covered 值同理防短词先替碎长词）：

```rust
/// BETA-24：MediaSearch 臂覆盖检测专用——媒体框架噪声词，在 FileSearch
/// 词表基础上叠加剥离。BETA-23 实测正是这些词未剥导致 +4.7% 误触发。
/// 按字符数降序排列（先替长词，防短词先替碎长词留残渣）。
const MEDIA_COVERAGE_NOISE_WORDS: &[&str] = &[
    // 英文（≥3 字母才可能构成 content run；两字母词如 by 不必列）
    "playlists", "playlist", "lossless", "duration", "minutes", "albums",
    "artist", "tracks", "listen", "songs", "music", "album", "track",
    "song", "play",
    // 中文
    "播放列表", "高品质", "高音质", "歌单", "歌曲", "单曲", "专辑", "音乐",
    "一首", "几首", "两首", "三首", "无损", "音质", "品质", "时长", "分钟",
    "小时", "播放", "听听", "的歌", "歌",
];
```

`has_uncovered_content`（fallback.rs:131）在噪声词剥离处加媒体分支——在 `COVERAGE_NOISE_WORDS` 循环之后、`ZH_CONTAINER_NOUNS` 循环之前插入：

```rust
        if matches!(intent, SearchIntent::MediaSearch(_)) {
            for w in MEDIA_COVERAGE_NOISE_WORDS {
                s = s.replace(w, " ");
            }
        }
```

`analyze_structural_omissions` 的 MediaSearch 臂（fallback.rs:256-258）把「禁用」注释块替换为：

```rust
            // BETA-24：媒体臂启用内容词覆盖检测。BETA-23 被砍的原因（FileSearch
            // 剥离词表未剥媒体框架词、v0.9 误触发 +4.7%）由 MEDIA_COVERAGE_NOISE_WORDS
            // 叠加剥离解决；误触发门见 fire-rate 报告（Task 2）。
            if has_uncovered_content(&parsed.query, &parsed.intent) {
                missing.push("keywords");
            }
```

注意：MediaSearch 臂的 match 绑定是 `ms`，`has_uncovered_content` 用 `&parsed.query` / `&parsed.intent`（与 FileSearch 臂 fallback.rs:232 同款调用）。`ms` 若因此只剩 time/size/sort/location 检查使用，无需改绑定。

- [ ] **Step 5: 跑测试确认通过**

```bash
cargo test -p locifind-intent-parser
```

预期：全绿（含两个新测试 + 既有 `covered_queries_do_not_trigger_keywords_omission` + `problem4_compound_query_triggers_keywords_omission`）。

- [ ] **Step 6: parser-only byte-equal 硬门**

```bash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 | tail -5
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 | tail -5
```

预期：v0.5 **pass 473** / v0.9 **pass 726**，与改动前完全一致（本 task 只碰触发器路径，`parse()` 零改动，机械保证；此步是验证而非调试）。

- [ ] **Step 7: fmt + clippy + 全 workspace test + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add packages/intent-parser/src/fallback.rs
git commit -m "feat(parser): BETA-24 MediaSearch 臂启用内容词覆盖检测（媒体噪声词表）"
```

---

### Task 2: fire-rate 误触发门（v0.9 before/after 量化 + 词表调优）

**Files:**
- 无代码改动预期；若超门需 Modify: `packages/intent-parser/src/fallback.rs`（收紧 `MEDIA_COVERAGE_NOISE_WORDS`）

- [ ] **Step 1: 采集 before/after 触发集**

```bash
cd /Users/alice/Work/LocalFind
git stash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --fire-rate > /tmp/beta24-fire-before.txt
git stash pop
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --fire-rate > /tmp/beta24-fire-after.txt
diff /tmp/beta24-fire-before.txt /tmp/beta24-fire-after.txt
```

before 应与 BETA-23 实测一致：269/1000 (26.9%)、omission.keywords=2（不一致则停下查原因）。fire-rate 输出末行 `triggered ids:` 含完整 id 列表，diff 即得新增触发 id。

- [ ] **Step 2: 逐条审新增触发**

对每个新增 id，从 fixture 取 query 与 expected_intent，人工判定「真遗漏 vs 误触发」：

```bash
for id in <新增id列表>; do
  python3 -c "
import json
cases = json.load(open('packages/evals/fixtures/v0.9/cases.json'))
c = next(c for c in cases if c['id'] == '$id')
print(c['id'], '|', c.get('query') or c.get('title'), '|', json.dumps(c['expected_intent'], ensure_ascii=False))
"
done
```

判定标准：expected_intent 的 keywords/artist/title/album/genre 里有 query 内容词而 parser 输出缺它 → 真遗漏（合格）；parser 已全覆盖、新触发纯因媒体框架词没剥干净 → 误触发。

- [ ] **Step 3: 误触发门判定与词表迭代**

门：**新增触发中的误触发 = 0**（对照 BETA-23 被砍时 +4.7% ≈ 媒体模板 47 条量级）。超门则把肇事框架词补进 `MEDIA_COVERAGE_NOISE_WORDS`，回 Step 1 重测。词表反复收不拢（迭代 3 轮仍有误触发）→ 按 spec §4.2 把媒体臂保持关闭（revert Task 1 的臂启用、保留词表与测试为禁用形态）并在 spec 验证后记如实登记，后续 task 照常进行（FileSearch 臂样本仍有效）。

- [ ] **Step 4: 把 fire-rate 数字记入 spec 验证后记 + commit（若有词表改动）**

在 spec `2026-06-13-beta-24-lora-retrain-keywords-design.md` 末尾追加「验证后记」节，记录 before/after fire-rate、新增触发数、误触发数：

```bash
git add packages/intent-parser/src/fallback.rs docs/superpowers/specs/2026-06-13-beta-24-lora-retrain-keywords-design.md
git commit -m "test(parser): BETA-24 媒体臂 fire-rate 误触发门通过（v0.9 before/after 量化）"
```

---

### Task 3: `load_cases` 支持任意路径（held-out 评测前置）

**Files:**
- Modify: `packages/evals/src/lib.rs:101-122`（`load_cases`）
- Test: 同文件 tests 模块

- [ ] **Step 1: 写失败测试**

在 `packages/evals/src/lib.rs` 的 tests 模块加：

```rust
    #[test]
    fn load_cases_accepts_json_path() {
        // 路径形态（以 .json 结尾）应按文件路径读取——held-out 评测依赖
        let path = format!("{}/fixtures/v0.5/cases.json", env!("CARGO_MANIFEST_DIR"));
        let by_path = load_cases(&path).expect("路径加载失败");
        let by_name = load_cases("v0.5").expect("版本名加载失败");
        assert_eq!(by_path.len(), by_name.len());
        assert_eq!(by_path[0].id, by_name[0].id);
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-evals load_cases_accepts_json_path
```

预期：FAIL，`未知 fixture 版本：…/fixtures/v0.5/cases.json`。

- [ ] **Step 3: 实现**

`load_cases` 的 match 在 `other => bail!` 前加一臂：

```rust
        // BETA-24：以 .json 结尾视为文件路径（held-out / 临时 fixture 评测）
        other if other.ends_with(".json") => std::fs::read_to_string(Path::new(other))
            .map_err(|e| anyhow::anyhow!("读取 fixture 路径 {other} 失败：{e}"))?,
```

- [ ] **Step 4: 跑测试确认通过 + commit**

```bash
cargo test -p locifind-evals
cargo fmt --check && cargo clippy -p locifind-evals --all-targets -- -D warnings
git add packages/evals/src/lib.rs
git commit -m "feat(evals): BETA-24 load_cases 支持 .json 路径（held-out 评测前置）"
```

---

### Task 4: fixtures 子命令 `generate-lora-aug-keywords`（模板生成 + 手写汇编 + train/heldout 切分）

**Files:**
- Modify: `packages/evals/src/bin/fixtures.rs`（新 Subcommand + 实现 + 测试）
- Create: `packages/evals/fixtures/lora-aug-keywords/v1/_authoring/`（目录，本 task 先放一个最小示例分片）

**核心设计（spec §3.1 实现细则）：** 合成 case 的 `expected_intent = parser draft ⊕ 补齐 keywords`——其余字段继承 draft（时间/排序等 parser 本就处理对，且 language 已锁死不归模型管），保证 patch 精确等于 keywords 填充、与 `apply_patch` 并集语义（draft 在前、补词在后）同序。手写样本因此只需 `{id, query, missing_keywords}` 三个字段（AugSeed），由汇编统一算 expected。不触发 keywords 的 seed **丢弃并逐条打印**（no silent caps）。

- [ ] **Step 1: 写失败测试**

在 `fixtures.rs` 末尾加 tests 模块：

```rust
#[cfg(test)]
mod lora_aug_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn aug_case_from_seed_problem4_shape() {
        // 问题 4 同型：parser 抽到「运维」丢「会议纪要」→ expected = draft ∪ missing（draft 在前）
        let seed = AugSeed {
            id: "aug-test-001".to_owned(),
            query: "2025年的会议纪要文件名包含运维".to_owned(),
            missing_keywords: vec!["会议纪要".to_owned()],
        };
        let case = aug_case_from_seed(&seed).expect("问题 4 同型应触发 keywords");
        assert_eq!(case["id"], "aug-test-001");
        assert_eq!(case["variant"], "FileSearch");
        let kws: Vec<String> = serde_json::from_value(
            case["expected_intent"]["keywords"].clone()
        ).unwrap();
        assert!(kws.contains(&"运维".to_owned()), "draft 词应保留: {kws:?}");
        assert!(kws.contains(&"会议纪要".to_owned()), "missing 词应补入: {kws:?}");
        let pos_draft = kws.iter().position(|k| k == "运维").unwrap();
        let pos_missing = kws.iter().position(|k| k == "会议纪要").unwrap();
        assert!(pos_draft < pos_missing, "并集语义 draft 在前: {kws:?}");
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
    fn template_seeds_are_deterministic() {
        let a = template_seeds();
        let b = template_seeds();
        assert_eq!(a.len(), b.len());
        assert!(a.iter().zip(&b).all(|(x, y)| x.id == y.id && x.query == y.query));
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
            heldout.iter().all(|c| !train_ids.contains(c["id"].as_str().unwrap())),
            "train/heldout 零重叠"
        );
        let (train2, heldout2) = split_train_heldout(&cases);
        assert_eq!(train, train2);
        assert_eq!(heldout, heldout2);
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-evals --bin fixtures lora_aug
```

预期：编译 FAIL（`AugSeed` / `aug_case_from_seed` / `template_seeds` / `split_train_heldout` 未定义）。

- [ ] **Step 3: 实现**

`fixtures.rs` 头部补 import（已有 `serde::{Deserialize, Serialize}`、`serde_json::Value`、`anyhow`）：

```rust
use locifind_evals::variant_name;
use locifind_intent_parser::hybrid::IntentDraft;
```

`Commands` enum 加：

```rust
    /// BETA-24：生成 lora-aug-keywords fixture（模板 + 手写汇编 + train/heldout 切分）
    GenerateLoraAugKeywords {
        /// 手写 seed 分片目录（每片为 AugSeed JSON 数组）
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
```

main 的 match 加对应臂调 `generate_lora_aug_keywords(&handwritten, &output_train, &output_heldout)?`。

核心实现（加在 `assemble_coverage` 附近）：

```rust
/// BETA-24：keywords 补全训练样本的种子——手写分片与模板生成共用此形态。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AugSeed {
    id: String,
    query: String,
    /// parser 会丢、期望模型补回的内容词（构造期已知）
    missing_keywords: Vec<String>,
}

/// 种子 → eval Case JSON。expected_intent = parser draft ⊕ 补齐 keywords：
/// 其余字段继承 draft（时间/排序 parser 本就处理对；language 已锁死不归模型管），
/// 保证 patch 精确等于 keywords 填充，且与 apply_patch 并集语义（draft 在前）同序。
/// 返回 None = parser 对该 query 不触发 keywords 待填（推理期到不了模型，不进数据集）。
fn aug_case_from_seed(seed: &AugSeed) -> Option<Value> {
    let draft = IntentDraft::from_query(&seed.query);
    if !draft.fillable_fields.contains(&"keywords") {
        return None;
    }
    let variant = variant_name(&draft.intent).to_owned();
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
        "会议纪要", "周报", "体检报告", "财务预算", "项目复盘", "培训材料",
        "实验数据", "装修合同", "旅行攻略", "简历模板", "课程笔记", "年度总结",
    ];
    const ZH_TAIL: &[&str] = &["运维", "架构", "验收", "季度", "客户", "合规"];
    const ZH_TIME: &[&str] = &["2025年", "去年", "上个月", "最近一周"];
    const EN_CONTENT: &[&str] = &[
        "annual budget", "research paper", "onboarding checklist",
        "marketing plan", "design review", "meeting notes",
    ];
    const EN_TAIL: &[&str] = &["roadmap", "compliance", "handover"];
    const ZH_TOPIC: &[&str] = &[
        "毕业旅行", "夏天的雨", "旧城改造", "深夜电台", "山间徒步", "海边日落",
    ];
    const EN_TOPIC: &[&str] = &["rainy nights", "long road trips", "quiet mornings"];

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
        push(&mut seeds, format!("{time}的{content}文件名包含{tail}"), vec![content]);
    }
    // 模板 2（复合内容词）：「找{time}的{content}和{tail}相关的文件」
    for (i, content) in ZH_CONTENT.iter().enumerate() {
        let time = ZH_TIME[(i + 1) % ZH_TIME.len()];
        let tail = ZH_TAIL[(i + 2) % ZH_TAIL.len()];
        push(&mut seeds, format!("找{time}的{content}和{tail}相关的文件"), vec![content, tail]);
    }
    // 模板 3（英文文件名包含同型）：「{content} files with {tail} in the name」
    for (i, content) in EN_CONTENT.iter().enumerate() {
        let tail = EN_TAIL[i % EN_TAIL.len()];
        push(&mut seeds, format!("{content} files with {tail} in the name"), vec![content]);
    }
    // 模板 4（媒体中文）：「找几首关于{topic}的歌」
    for topic in ZH_TOPIC {
        push(&mut seeds, format!("找几首关于{topic}的歌"), vec![topic]);
    }
    // 模板 5（媒体英文）：「play some songs about {topic}」
    for topic in EN_TOPIC {
        push(&mut seeds, format!("play some songs about {topic}"), vec![topic]);
    }
    seeds
}

/// 按 id 升序排序后索引步进切分：每 5 条取 1 条进 held-out（~20%，确定性）。
fn split_train_heldout(cases: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut sorted: Vec<Value> = cases.to_vec();
    sorted.sort_by(|a, b| {
        a["id"].as_str().unwrap_or("").cmp(b["id"].as_str().unwrap_or(""))
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
    // 2. id 唯一性硬断言
    let mut ids: Vec<&str> = seeds.iter().map(|s| s.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    anyhow::ensure!(ids.len() == before, "种子 id 重复");
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
    fs::write(output_heldout, serde_json::to_string_pretty(&heldout)? + "\n")?;
    eprintln!(
        "✅ lora-aug-keywords：seeds={}，cases={}（dropped {}），train={} → {}，heldout={} → {}",
        seeds.len(), cases.len(), dropped.len(),
        train.len(), output_train.display(),
        heldout.len(), output_heldout.display()
    );
    Ok(())
}
```

注意：`fixtures.rs` 现有 import 若缺 `anyhow::Context`（`with_context`）需补；`fs` 为 `std::fs` 已有。

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p locifind-evals --bin fixtures
```

预期：4 个新测试全 PASS。若 `aug_case_from_seed_problem4_shape` 因 parser 行为与预期不符（如「运维」不在 draft keywords），用 `--nocapture` 打印 draft 调整断言前提——**不得为测试通过而改 parser**。

- [ ] **Step 5: 放最小手写示例分片 + 首跑生成器观察丢弃率**

创建 `packages/evals/fixtures/lora-aug-keywords/v1/_authoring/hw-smoke.json`（占位示例，Task 5 会替换为正式分片）：

```json
[
  {
    "id": "aug-hw-smoke-001",
    "query": "帮我找一下去年写的装修合同名字里带押金的",
    "missing_keywords": ["装修合同"]
  }
]
```

```bash
cargo run --release -p locifind-evals --bin fixtures -- generate-lora-aug-keywords
```

观察：模板存活率（cases/seeds）。模板 2/4/5 是「尝试性形态」，**部分被丢正常**；若某模板全军覆没（一条不剩），说明该形态 parser 全覆盖或不触发，在 Step 3 的模板里换形态（如内容词后加「相关的」、媒体模板换「关于X的歌曲推荐」）重试，目标模板部分存活 ≥60 条。

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt --check && cargo clippy -p locifind-evals --all-targets -- -D warnings && cargo test -p locifind-evals
git add packages/evals/src/bin/fixtures.rs packages/evals/fixtures/lora-aug-keywords/
git commit -m "feat(evals): BETA-24 lora-aug-keywords fixture 生成器（模板+手写汇编+train/heldout 切分）"
```

---

### Task 5: Claude 手写自然变体分片（防模板过拟合）

**Files:**
- Create: `packages/evals/fixtures/lora-aug-keywords/v1/_authoring/hw-{zh,en,mixed,media-zh,media-en,colloquial}.json`（6 片，替换 hw-smoke.json）
- Create: `packages/evals/fixtures/lora-aug-keywords/v1/README.md`

- [ ] **Step 1: 并行 dispatch 6 个撰写子 agent**

每片 ~30 条 AugSeed（`{id, query, missing_keywords}`），id 前缀 `aug-hw-{片名}-NNN`。给每个子 agent 的统一指令要点（参照 BETA-13 `ANNOTATION_GUIDE.md` 纪律精神，但本数据只标 missing_keywords 一个语义轴）：

- query 必须是**自然口语**，不得复刻 Task 4 模板句式（模板防过拟合正是本片存在的理由）；鼓励：语气词（帮我/麻烦/那个）、倒装、省略、错落的修饰语、中英混合。
- 每条 query 须包含 parser 大概率会丢的内容词（多内容词复合、「文件名包含」类短路、容器名词链「X的Y的Z」、媒体主题词），`missing_keywords` 列出期望模型补回的词（1-2 个）。
- **全合成词**：内容词用常见生活/办公主题（体检报告/装修合同/毕业旅行……），严禁真实人名外的真实专名、真实文件名、真实路径（艺术家名只用 v0.5 已有的合成惯例如「邓紫棋/周杰伦」级公开人名即可，避免生僻真实信息）。
- 分片主题：hw-zh（中文文件搜索）/ hw-en（英文文件搜索）/ hw-mixed（中英混合）/ hw-media-zh（中文媒体）/ hw-media-en（英文媒体）/ hw-colloquial（极口语化中文，含错别字近似表达）。

- [ ] **Step 2: 汇编 + 存活率检查**

```bash
rm packages/evals/fixtures/lora-aug-keywords/v1/_authoring/hw-smoke.json
cargo run --release -p locifind-evals --bin fixtures -- generate-lora-aug-keywords
```

观察丢弃列表：手写片存活率目标 ≥50%（手写无法预判 parser 行为，丢弃过半说明撰写指令理解偏了——把丢弃样本反馈给子 agent 重写该片）。**总 cases 目标 ≥300**（train ≥240 / heldout ≥60）；不足则补一轮撰写。

- [ ] **Step 3: 写 fixture README + commit**

`packages/evals/fixtures/lora-aug-keywords/v1/README.md`：

```markdown
# lora-aug-keywords v1

BETA-24 keywords 补全训练数据 fixture。**v0.9 全程不进训练**（评测集污染红线）。

- 生成：`cargo run --release -p locifind-evals --bin fixtures -- generate-lora-aug-keywords`
- 种子 = 程序模板（fixtures.rs `template_seeds`）+ `_authoring/hw-*.json` 手写分片（AugSeed：`{id, query, missing_keywords}`）
- `expected_intent = parser draft ⊕ 补齐 keywords`（其余字段继承 draft；与 apply_patch 并集语义同序）
- 不触发 keywords 待填的种子在生成期丢弃并打印
- `cases.json` = 训练份；`heldout-cases.json` = 验收量化份（id 升序每 5 取 1，**永不进训练**）
- 词表全合成，无真实文件名/路径/搜索词
```

```bash
git add packages/evals/fixtures/lora-aug-keywords/
git commit -m "feat(evals): BETA-24 手写自然变体分片 6 片 + fixture README"
```

---

### Task 6: `build_lora_dataset` 过滤旗标 + 生成 keywords-aug 训练 JSONL

**Files:**
- Modify: `packages/evals/src/bin/build_lora_dataset.rs`
- Create（生成产物，入库）: `training/datasets/lora-aug-keywords/v1/cases.jsonl` + `meta.json`

- [ ] **Step 1: 写失败测试**

在 `build_lora_dataset.rs` 的 `case_conversion_tests` 模块加：

```rust
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
        let line = case_to_jsonl_line(&covered).expect("不应报错").expect("variant 应匹配");
        assert!(
            !line.fillable_fields.iter().any(|f| f == "keywords"),
            "前提：该 case 不应有 keywords 待填，实际 {:?}",
            line.fillable_fields
        );
        assert!(!line_passes_require_fillable(&line, Some("keywords")));
        assert!(line_passes_require_fillable(&line, None));
    }
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-evals --bin build_lora_dataset require_fillable
```

预期：编译 FAIL（`line_passes_require_fillable` 未定义）。

- [ ] **Step 3: 实现**

`Args` 加两个旗标：

```rust
    /// BETA-24：只保留 fillable_fields 含该字段的 case（如 keywords）
    #[arg(long)]
    require_fillable: Option<String>,
    /// meta.json 的 dataset_name（默认沿用 v0.5-patch）
    #[arg(long, default_value = "v0.5-patch")]
    dataset_name: String,
```

过滤辅助函数 + main 循环接线：

```rust
/// --require-fillable 过滤：不触发该字段待填的 case 推理期到不了模型，
/// 训进去反而教模型补没被要求的字段（BETA-24）。
fn line_passes_require_fillable(line: &JsonlLine, required: Option<&str>) -> bool {
    required.is_none_or(|f| line.fillable_fields.iter().any(|x| x == f))
}
```

main 循环里 `Some(line)` 臂改为先过滤：

```rust
            Some(line) => {
                if !line_passes_require_fillable(&line, args.require_fillable.as_deref()) {
                    stats.skipped_not_fillable += 1;
                    continue;
                }
                // …（原 empty/nonempty 统计与 push 不动）
            }
```

`Stats` 加 `skipped_not_fillable: usize` 字段，meta.json 的 stats 与末尾 eprintln 各加该项；meta 的 `"dataset_name"` 改用 `args.dataset_name`。

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p locifind-evals --bin build_lora_dataset
```

- [ ] **Step 5: 生成数据集（训练份）**

```bash
cargo run --release -p locifind-evals --bin build_lora_dataset -- \
    --input packages/evals/fixtures/lora-aug-keywords/v1/cases.json \
    --output training/datasets/lora-aug-keywords/v1/ \
    --require-fillable keywords \
    --dataset-name lora-aug-keywords
```

检查 meta.json：`nonempty_patch` 应 = 全部行数（expected = draft ⊕ keywords，patch 必非空）；`skipped_not_fillable` 应 = 0（fixture 生成期已过滤，此处是双保险）；`by_fillable_field.keywords` = 行数。

- [ ] **Step 6: 回归 + commit**

```bash
cargo fmt --check && cargo clippy -p locifind-evals --all-targets -- -D warnings && cargo test -p locifind-evals
git add packages/evals/src/bin/build_lora_dataset.rs training/datasets/lora-aug-keywords/
git commit -m "feat(evals): BETA-24 build_lora_dataset --require-fillable + lora-aug-keywords v1 训练 JSONL"
```

---

### Task 7: `prepare_main_data.py` 多源混合（向后兼容）

**Files:**
- Modify: `training/mlx-lora/scripts/prepare_main_data.py`

- [ ] **Step 1: 实现（脚本小，直接改 + 自检断言即测试）**

加 argparse：`--keywords-aug <path>` 可选——**不传时行为与现状逐字节一致**（run_main_v1.sh / run_bakeoff.sh 的历史可复现性不破）。混合逻辑：

```python
import argparse

KEYWORDS_OVERSAMPLE = 2  # keywords-aug 重复倍数；按三桶大致均衡调（见下方打印）

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--keywords-aug", type=Path, default=None,
                    help="BETA-24：keywords 补全数据集 cases.jsonl（不传=v1 现状行为）")
    cli = ap.parse_args()
    # …（读 INPUT、转 chat、empty/nonempty 分桶、oversample 8× 全部不动）…
    keywords_records = []
    if cli.keywords_aug is not None:
        if not cli.keywords_aug.exists():
            print(f"❌ 未找到 keywords-aug: {cli.keywords_aug}", file=sys.stderr)
            return 1
        with cli.keywords_aug.open("r", encoding="utf-8") as f:
            kw_lines = [json.loads(line) for line in f if line.strip()]
        kw_minimal = [
            {"messages": [
                {"role": "user", "content": rec["prompt"]},
                {"role": "assistant", "content": rec["completion"]},
            ]}
            for rec in kw_lines
        ]
        assert all(not _is_empty_patch(_completion(r)) for r in kw_minimal), \
            "keywords-aug 不应含 empty patch（expected = draft ⊕ keywords 必非空）"
        keywords_records = kw_minimal * KEYWORDS_OVERSAMPLE
    all_records = empty + oversampled + keywords_records
    rng.shuffle(all_records)
    # …（写出与既有断言不动）…
    print(
        f"✅ main data prepared: empty={len(empty)}, "
        f"nonempty={len(nonempty)}×{NONEMPTY_OVERSAMPLE}={len(oversampled)}, "
        f"keywords={len(keywords_records) // max(KEYWORDS_OVERSAMPLE, 1)}×{KEYWORDS_OVERSAMPLE}"
        f"={len(keywords_records)}, total={len(all_records)}"
    )
```

均衡目标：三桶（empty ~443 / v0.5-nonempty ~440 / keywords-aug×K）各占总量 25%-40%。train ≥240 条时 K=2 → ~480，落在区间；生成量变化时调 `KEYWORDS_OVERSAMPLE` 常量。

- [ ] **Step 2: 双形态验证**

```bash
# 形态 1：不带旗标 = 现状逐字节一致
python3 training/mlx-lora/scripts/prepare_main_data.py
shasum training/mlx-lora/data/main/train.jsonl   # 记录
git stash && python3 training/mlx-lora/scripts/prepare_main_data.py && shasum training/mlx-lora/data/main/train.jsonl && git stash pop
# 两次 shasum 必须一致

# 形态 2：带旗标
python3 training/mlx-lora/scripts/prepare_main_data.py \
    --keywords-aug training/datasets/lora-aug-keywords/v1/cases.jsonl
# 检查打印的三桶数量落在 25%-40% 区间
```

- [ ] **Step 3: commit**

```bash
git add training/mlx-lora/scripts/prepare_main_data.py
git commit -m "feat(training): BETA-24 prepare_main_data 支持 keywords-aug 多源混合（默认行为不变）"
```

---

### Task 8: `run_beta24.sh` 训练脚本 + 训练跑通

**Files:**
- Create: `training/mlx-lora/scripts/run_beta24.sh`

- [ ] **Step 1: 写脚本**

以 `run_bakeoff.sh` 为底，差异：数据准备带 `--keywords-aug`、产物 slug 固定 `beta24-qwen3-0.6b`、评测段扩为 v0.5 + v0.9 双轨（with-fallback 留 Task 9 手动跑，脚本只到 parser-only baseline 落 JSON，避免训练脚本里塞 1000 条慢推理）：

```bash
#!/usr/bin/env bash
# BETA-24：keywords 补全重训。基座/超参完全对齐 BETA-17 winner（Qwen3-0.6B + v1 配方），
# 单一变量 = 训练数据并入 lora-aug-keywords。
set -euo pipefail

REPO_ID="mlx-community/Qwen3-0.6B-4bit"
SLUG="beta24-qwen3-0.6b"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
DATA_DIR="training/mlx-lora/data/main"
ADAPTER_DIR="training/mlx-lora/adapters/$SLUG"
SAFETENSORS_DIR="training/mlx-lora/fused/$SLUG-safetensors"
GGUF_F16="training/mlx-lora/fused/$SLUG-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/$SLUG-q4_k_m.gguf"

echo "==> [1/6] prepare data（v0.5-patch + lora-aug-keywords）"
python3 training/mlx-lora/scripts/prepare_main_data.py \
    --keywords-aug training/datasets/lora-aug-keywords/v1/cases.jsonl

echo "==> [2/6] mlx-lm lora train（v1 配方：1000 step / 16 layers / batch 4 / lr 1e-4 / mask-prompt / seed 42）"
python3 -m mlx_lm lora \
    --model "$REPO_ID" \
    --train \
    --data "$DATA_DIR" \
    --fine-tune-type lora \
    --num-layers 16 \
    --iters 1000 \
    --batch-size 4 \
    --learning-rate 1e-4 \
    --steps-per-report 50 \
    --steps-per-eval 200 \
    --adapter-path "$ADAPTER_DIR" \
    --mask-prompt \
    --seed 42

echo "==> [3/6] mlx-lm fuse → HF safetensors"
python3 -m mlx_lm fuse \
    --model "$REPO_ID" \
    --adapter-path "$ADAPTER_DIR" \
    --save-path "$SAFETENSORS_DIR" \
    --dequantize

echo "==> [4/6] convert_hf_to_gguf.py → fp16 GGUF"
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" \
    "$SAFETENSORS_DIR" \
    --outfile "$GGUF_F16" \
    --outtype f16

echo "==> [5/6] llama-quantize → Q4_K_M GGUF"
"$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$GGUF_Q4" Q4_K_M

echo "==> [6/6] parser-only baseline（v0.5 应 473 / v0.9 应 726）"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.5 --json > "training/mlx-lora/fused/$SLUG-v05-baseline.json"
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.9 --json > "training/mlx-lora/fused/$SLUG-v09-baseline.json"

echo "==> GGUF 体积 + sha256"
ls -lh "$GGUF_Q4"
shasum -a 256 "$GGUF_Q4"
echo "✅ [$SLUG] 训练完成；with-fallback 三层验收见 plan Task 9"
```

- [ ] **Step 2: 跑训练（~50 min，M5 Pro 量级）**

```bash
bash training/mlx-lora/scripts/run_beta24.sh 2>&1 | tee /tmp/beta24-train.log
```

预期：train loss 收敛到 ~0.01 量级（对照 v1：val 0.010）；GGUF ~378 MB。训练发散或 loss 异常 → 先查三桶混合比例（Task 7 打印），不要动超参。

- [ ] **Step 3: commit（脚本；GGUF/adapter 均 gitignore 不入库）**

```bash
git add training/mlx-lora/scripts/run_beta24.sh
git commit -m "feat(training): BETA-24 重训脚本（v1 配方 + keywords-aug 数据，单一变量）"
```

---

### Task 9: 三层验收评测（held-out / v0.9 / v0.5）+ 迭代协议

**Files:**
- 无代码改动预期；产出评测数字记入 spec 验证后记

- [ ] **Step 1: 第一层——held-out 合成集量化（核心新能力证据）**

```bash
ROOT=/Users/alice/Work/LocalFind; cd $ROOT
GGUF=training/mlx-lora/fused/beta24-qwen3-0.6b-q4_k_m.gguf
HELDOUT=packages/evals/fixtures/lora-aug-keywords/v1/heldout-cases.json

# parser-only 基线（对照：keywords 缺失应大量 partial）
DYLD_LIBRARY_PATH=$ROOT/target/release ./target/release/evals \
    --fixtures "$HELDOUT" --json > /tmp/beta24-heldout-parser.json
# with-fallback（真模型）
LOCIFIND_MODEL_PATH="$GGUF" DYLD_LIBRARY_PATH=$ROOT/target/release ./target/release/evals \
    --fixtures "$HELDOUT" --with-fallback --hybrid \
    --baseline /tmp/beta24-heldout-parser.json 2>&1 | tee /tmp/beta24-heldout-fallback.log
```

**门：with-fallback pass 率 ≥80%**（现役模型在此集上应接近 0%——空 patch `{}` 补不出 keywords）。同时记录 parser-only 基线 pass 率作对照（应显著低，证明增量来自模型）。

- [ ] **Step 2: 第二层——v0.9 全集（回归红线）**

```bash
# parser-only byte-equal（Task 8 已产 baseline JSON，pass 必须 = 726）
python3 -c "import json; d=json.load(open('training/mlx-lora/fused/beta24-qwen3-0.6b-v09-baseline.json')); print(d.get('summary') or {k:d[k] for k in d if 'pass' in str(k).lower()})"
# with-fallback before/after diff
LOCIFIND_MODEL_PATH="$GGUF" DYLD_LIBRARY_PATH=$ROOT/target/release ./target/release/evals \
    --fixtures v0.9 --with-fallback --hybrid \
    --baseline training/mlx-lora/fused/beta24-qwen3-0.6b-v09-baseline.json \
    2>&1 | tee /tmp/beta24-v09-fallback.log
# fire-rate（媒体臂启用后的最终形态留档）
DYLD_LIBRARY_PATH=$ROOT/target/release ./target/release/evals --fixtures v0.9 --fire-rate \
    | tee /tmp/beta24-v09-fire.txt
```

**门：regressions = 0**（红线，模型把 parser 已对的改错即阻断）；keywords/media 触发 case 净增益 ≥0，逐 case 列出 gains。**延迟同步记录**：触发 case p50/p95（≤3s 门，对照 BETA-23 实测 p95 601ms CPU / 本机 metal 应更低）。

- [ ] **Step 3: 第三层前半——v0.5 同口径**

```bash
LOCIFIND_MODEL_PATH="$GGUF" DYLD_LIBRARY_PATH=$ROOT/target/release ./target/release/evals \
    --fixtures v0.5 --with-fallback --hybrid \
    --baseline training/mlx-lora/fused/beta24-qwen3-0.6b-v05-baseline.json \
    2>&1 | tee /tmp/beta24-v05-fallback.log
```

**门：parser-only 473 byte-equal；with-fallback regressions=0、pass ≥480**（不低于 BETA-17 winner 的 480——keywords 训练不得挤掉既有 time/size 等补全能力）。

- [ ] **Step 4: 不达标迭代协议（最多 3 轮，每轮如实记录）**

- held-out <80%：dump 失败 case 的模型原始输出（evals `--case <id>` 复跑 + stderr），按失败模式补样本（哪类形态错补哪类）→ 调 `KEYWORDS_OVERSAMPLE` 或补手写片 → 重跑 Task 8。
- v0.9/v0.5 出现 regression：查模型把哪个字段改错——keywords 之外的字段错补优先怀疑训练数据 patch 含了多余字段（检查 meta.json by_fillable_field）；keywords 错补则收紧训练样本质量。
- 每轮在 spec 验证后记登记：轮次 / 改动 / 三层数字。3 轮仍不达 → 停，向用户汇报失败模式与选项（更多数据 / 接受更低门 / 换路线），不凑数。

- [ ] **Step 5: 达标后 commit 评测记录**

spec 验证后记写入三层最终数字 + 训练轮次：

```bash
git add docs/superpowers/specs/2026-06-13-beta-24-lora-retrain-keywords-design.md
git commit -m "test(evals): BETA-24 三层验收通过（held-out/v0.9/v0.5 数字登记）"
```

---

### Task 10: 模型登记 + 文档 + 真机问题 4 手测准备

**Files:**
- Create: `training/mlx-lora/releases/beta24.md`
- Modify: `training/mlx-lora/README.md`、`training/datasets/README.md`、`docs/manual-test-scenarios.md`

- [ ] **Step 1: releases/beta24.md（凭此可重建，沿用 v1.md/BETA-17 登记模式）**

```markdown
# BETA-24 重训（keywords 补全 + MediaSearch 覆盖）

- 基座：`mlx-community/Qwen3-0.6B-4bit`（同 BETA-17 winner，Apache 2.0）
- adapter：`training/mlx-lora/adapters/beta24-qwen3-0.6b/`
- Q4_K_M GGUF：`training/mlx-lora/fused/beta24-qwen3-0.6b-q4_k_m.gguf`
  - sha256：`<实测填写>`
- 训练数据：v0.5-patch/v0 + lora-aug-keywords/v1（KEYWORDS_OVERSAMPLE=<实际值>）
- 超参：v1 配方不动（1000 step / num-layers 16 / batch 4 / lr 1e-4 / mask-prompt / seed 42）
- 复现：`bash training/mlx-lora/scripts/run_beta24.sh`
- 三层验收：held-out <N>/<M>（<P>%）；v0.9 with-fallback pass <X> / regressed 0；v0.5 pass <Y> / regressed 0
- 部署：GGUF 改名放 `app 数据目录/models/qwen3-0.6b-q4_k_m.gguf`（desktop 期望文件名不变，零代码改动）
```

`<>` 占位在评测后记实测值填写后才 commit。

- [ ] **Step 2: README 两处状态更新**

- `training/mlx-lora/README.md` 顶部状态行加 BETA-24 节（一段：动机=keywords 空 patch、数据=唯一变量、指向 releases/beta24.md）。
- `training/datasets/README.md` 状态行加 lora-aug-keywords/v1。

- [ ] **Step 3: 手测场景登记**

`docs/manual-test-scenarios.md` 加 BETA-24 节（沿用 BETA-23 节格式）：

- 前置：新 GGUF 改名放置到 app 数据目录 `models/qwen3-0.6b-q4_k_m.gguf`（路径与放置方式照抄 BETA-23 节）；构建照抄 BETA-23 真机手测流程（release bundle + metal；**注意 BETA-25 动态库缺口未修，需照 BETA-23 手测时的手工补 Frameworks+rpath+重签步骤**）。
- 场景 1（问题 4 端到端）：搜「2025年的会议纪要文件名包含运维」→ 触发模型 →「模型补全」徽标 → 结果含「会议纪要」命中（对照合成 fixture 文件 `Documents/合成-会议纪要-001.md` 或用户真实同型文件）。
- 场景 2（媒体臂）：搜一条 Task 1 正例同型媒体查询 → 触发 → 补全主题词。
- 场景 3（回归）：BETA-23 的 4 个手测场景重过（状态三态/开关/无模型降级不受新模型影响）。

- [ ] **Step 4: 全量回归 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add training/mlx-lora/releases/beta24.md training/mlx-lora/README.md training/datasets/README.md docs/manual-test-scenarios.md
git commit -m "docs(training): BETA-24 模型登记 + README 状态 + 真机手测场景"
```

真机 GUI 手测由用户驱动（场景已登记）；收工时按 CONVENTIONS §3 更新 STATUS/ROADMAP（BETA-24 → done 或如实记录状态）并合并 feature 分支。

---

## 自审记录

- **Spec 覆盖**：§3 数据生成 → Task 4/5/6；§4 媒体臂 → Task 1/2；§5 训练 → Task 7/8；§6 三层验收 → Task 9 + Task 10 Step 3（真机）；§6.4 如实记录 → Task 9 Step 4 迭代协议。无遗漏。
- **类型一致性**：`AugSeed` / `aug_case_from_seed` / `template_seeds` / `split_train_heldout`（Task 4 定义、Task 5 消费）；`line_passes_require_fillable` / `skipped_not_fillable`（Task 6 内自洽）；`--keywords-aug` 旗标（Task 7 定义、Task 8 消费）；GGUF slug `beta24-qwen3-0.6b`（Task 8/9/10 一致）。
- **已知不确定点（计划内置应对）**：模板/手写存活率（Task 4 Step 5、Task 5 Step 2 的存活率检查与重写回路）；媒体臂正例选定（Task 1 Step 1 探针）；误触发收不拢的退路（Task 2 Step 3 保持关闭）；训练不达标（Task 9 Step 4 三轮协议）。
