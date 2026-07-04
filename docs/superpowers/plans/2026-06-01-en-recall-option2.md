# 英文召回 option 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 清零 BETA-15A 召回评测集 en 桶残留 3 个 FAIL case（cover letter / style guide 复合多词键 + minutes variant 漂移），en 召回 80%→100%、假阳保持 0%、v0.5 evals parser-only 不回归（pass≥472）。

**Architecture:** 两处独立修复。Fix B 改 parser `media_search.rs`：时长词（minute(s)/hour(s)/分钟/小时）从裸子串匹配改为需数字上下文才算强媒体信号，消除 "minutes" 的 variant 漂移。Fix A 改 harness `synonym/yaml.rs`：`expand` 的「有 keyword」分支新增多词键覆盖——扫描 query 中出现的多词词典键，若其包含某个已抽 keyword 则用多词键的组替换该 keyword 组（去重保序）。

**Tech Stack:** Rust（locifind-intent-parser / locifind-harness / locifind-evals），regex，serde_yaml。

设计来源：[docs/superpowers/specs/2026-06-01-en-recall-option2-design.md](../specs/2026-06-01-en-recall-option2-design.md)。

---

## File Structure

- `packages/intent-parser/src/parsers/media_search.rs` — Fix B：拆分 `has_strong_media_signal`，新增 `has_numeric_duration` helper。
- `packages/harness/src/synonym/yaml.rs` — Fix A：新增 `multiword_keys` / `apply_multiword_override` 方法 + `dedup_groups_by_head` free fn，改 `expand`。
- 验证：`packages/evals/src/bin/synonym_recall.rs`（既有）+ `packages/evals/src/bin/evals.rs`（既有），无需改源，只跑。

---

## Task 1: Fix B — 时长词需数字上下文（parser media_search）

**Files:**
- Modify: `packages/intent-parser/src/parsers/media_search.rs:125-147`（`has_strong_media_signal`）
- Modify: 同文件新增 `has_numeric_duration` helper（紧邻 `has_explicit_size_threshold`，约 :119 之后）
- Test: 同文件 `#[cfg(test)] mod tests_screenshot_time_and_stopwords`（追加测试）

- [ ] **Step 1: 写失败测试**

在 `mod tests_screenshot_time_and_stopwords`（文件末尾该模块内）追加：

```rust
    #[test]
    fn bare_minutes_does_not_trigger_media_v_option2() {
        // "minutes"（会议纪要）无数字上下文 → 不应漂移到 media_search。
        let intent = crate::parse("where are the minutes from the October all-hands");
        assert!(
            matches!(intent, SearchIntent::FileSearch(_)),
            "裸 minutes 应为 FileSearch，实际 {intent:?}"
        );
    }

    #[test]
    fn numeric_duration_still_triggers_media_v_option2() {
        use super::has_strong_media_signal;
        // 有数字上下文的时长词仍是强媒体信号。
        assert!(has_strong_media_signal("songs longer than 5 minutes"));
        assert!(has_strong_media_signal("找时长超过 3 小时的视频")); // "3 小时"
        assert!(has_strong_media_signal("clips under 30 minutes"));
        // 裸时长词不再是强信号。
        assert!(!has_strong_media_signal("where are the minutes"));
        assert!(!has_strong_media_signal("the minutes from the meeting"));
        assert!(!has_strong_media_signal("会议分钟纪要")); // 裸"分钟"无数字
        // 真·强媒体词不受影响。
        assert!(has_strong_media_signal("audio recording"));
        assert!(has_strong_media_signal("找一首歌"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser bare_minutes_does_not_trigger_media_v_option2 numeric_duration_still_triggers_media_v_option2 2>&1 | tail -20`
Expected: FAIL —— `bare_minutes...` 现得 MediaSearch；`numeric_duration...` 中 `the minutes from the meeting` / `会议分钟纪要` 现返回 true（裸 contains 命中）。

- [ ] **Step 3: 新增 `has_numeric_duration` helper**

在 `has_explicit_size_threshold`（约 :119）之后插入：

```rust
/// 时长词仅在前置数字时才算媒体信号（区分"5 minutes 的视频"与"会议 minutes 纪要"）。
/// 复用与 [`has_explicit_size_threshold`] 同款的数字+单位模式。
fn has_numeric_duration(lower: &str) -> bool {
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"\d+\s*(?:分钟|小时|minutes?|hours?)").expect("regex")
    });
    re.is_match(lower)
}
```

- [ ] **Step 4: 从 STRONG 移除时长词并改判定**

把 `has_strong_media_signal`（:125-147）改为：

```rust
pub(crate) fn has_strong_media_signal(lower: &str) -> bool {
    const STRONG: &[&str] = &[
        "歌",
        "音乐",
        "audio",
        "song",
        "录音",
        "录像",
        "截图",
        "截屏",
        "screenshot",
        "screenshots",
        "截的",
        "截了",
    ];
    // 时长词（分钟/小时/minute(s)/hour(s)）仅在前置数字时算强信号，
    // 避免"minutes"（会议纪要）等内容词被误判为媒体（variant 漂移）。
    STRONG.iter().any(|s| lower.contains(s)) || has_numeric_duration(lower)
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p locifind-intent-parser bare_minutes_does_not_trigger_media_v_option2 numeric_duration_still_triggers_media_v_option2 2>&1 | tail -20`
Expected: PASS（2 tests）。

- [ ] **Step 6: 探查 office-02 修后 parser keyword（为 Task 3 召回做准备）**

Run: `cargo run -q -p locifind-cli -- --intent-only "where are the minutes from the October all-hands" 2>/dev/null`
Expected: `intent: file_search`，`keywords` 含 `"minutes"`（与 office-01 抽首个内容名词同理）。若 keywords 不含 minutes，记录实际输出供 Task 3 判断是否需 gazetteer 补齐（理论上 minutes 为首个内容名词应被抽出）。

- [ ] **Step 7: 全 crate 测试 + fmt + clippy**

Run: `cargo test -p locifind-intent-parser 2>&1 | tail -5 && cargo fmt -p locifind-intent-parser --check && cargo clippy -p locifind-intent-parser --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 全过，无回归。

- [ ] **Step 8: Commit**

```bash
git add packages/intent-parser/src/parsers/media_search.rs
git commit -m "fix(parser): 时长词需数字上下文才算强媒体信号（minutes 不再漂移到 media）"
```

---

## Task 1B: minutes case 第二层根因 — copula/aux 停词（执行中发现）

> **发现来源**：Task 1 Step 6 探查显示，修 media 漂移后 `where are the minutes from the October all-hands` 解析为 file_search 但 `keywords:["are"]`——停词 "are"（3 字符，未被 `len<3` 过滤）作为首个存活 token 被抽出，挤掉真正的内容名词 "minutes"。无此修复，recall-en-office-02 仍漏召回（expand 走 keyword 分支得 singleton "are"）。属 minutes case 必要的第二层修复，与上次会话英文停词工作同源。

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs`（关键词 stop 列表，约 :446-455，`where/when/how/did/need/want/save` 之后）
- Test: `packages/intent-parser/src/parsers/file_search.rs` 测试模块（追加）或 media_search 同类集成测试

- [ ] **Step 1: 写失败测试**

在 file_search.rs 的测试模块追加（若无合适模块，新增 `#[cfg(test)] mod tests_copula_stopwords`）：

```rust
    #[test]
    fn copula_are_not_extracted_as_keyword_en_recall() {
        // "are"（3 字符 copula）不应作内容 keyword，应跳到真正的内容名词 "minutes"。
        let intent = crate::parse("where are the minutes from the October all-hands");
        let SearchIntent::FileSearch(fs) = intent else {
            panic!("应为 FileSearch，实际 {intent:?}");
        };
        let kws = fs.keywords.unwrap_or_default();
        assert!(
            kws.iter().any(|k| k == "minutes"),
            "keywords 应含 minutes，实际 {kws:?}"
        );
        assert!(
            !kws.iter().any(|k| k == "are"),
            "keywords 不应含 are，实际 {kws:?}"
        );
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser copula_are_not_extracted_as_keyword_en_recall 2>&1 | tail -20`
Expected: FAIL —— 当前 keywords=["are"]。

- [ ] **Step 3: 在 stop 列表加入 copula/aux 词**

在 file_search.rs 的 excluded stop 列表（疑问词 `where/when/how` + 动词 `did/need/want/save/saved` 之后）追加：

```rust
        // B3.5/en-recall option2：copula/助动词（≥3 字符，未被 len<3 过滤）不应作内容
        // keyword —— 否则 "where are the minutes" 抽到 "are" 而非 "minutes"，漏召回。
        "are",
        "was",
        "were",
        "been",
        "being",
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-intent-parser copula_are_not_extracted_as_keyword_en_recall 2>&1 | tail -20`
Expected: PASS。

- [ ] **Step 5: 全 crate 测试 + fmt + clippy + v0.5 evals 不回归**

Run: `cargo test -p locifind-intent-parser 2>&1 | tail -5 && cargo fmt -p locifind-intent-parser --check && cargo clippy -p locifind-intent-parser --all-targets -- -D warnings 2>&1 | tail -5 && cargo run -q -p locifind-evals --bin evals 2>&1 | tail -8`
Expected: 全过；v0.5 evals parser-only pass≥472。

- [ ] **Step 6: Commit**

```bash
git add packages/intent-parser/src/parsers/file_search.rs
git commit -m "fix(parser): copula/助动词加入英文 keyword 停词（minutes case 第二层修复）"
```

---

## Task 2: Fix A — 多词键覆盖（harness synonym expander）

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`（`impl YamlSynonymExpander` 新增方法；`expand` 改写；文件级新增 `dedup_groups_by_head` free fn）
- Test: 同文件 `#[cfg(test)] mod expand_tests`（追加测试）

- [ ] **Step 1: 写失败测试**

在 `mod expand_tests` 内，扩充测试用 en 词典并新增 case。先把 `en_yaml()`（:580）替换为包含多词键的版本：

```rust
    fn en_yaml() -> &'static str {
        r"
version: 1
language: en
groups:
  - head: slides
    aliases: [slideshow, presentation]
  - head: cover letter
    aliases: [application]
  - head: style guide
    aliases: [branding, guidelines]
"
    }
```

然后追加测试：

```rust
    #[test]
    fn multiword_key_overrides_single_token_keyword() {
        // parser 抽单 token "cover"，query 含多词键 "cover letter" → 用多词键组覆盖。
        let e = expander();
        let out = e.expand(
            intent_with(vec!["cover"]),
            "find my cover letter for the Google position",
        );
        assert_eq!(out.keyword_groups.len(), 1);
        assert_eq!(out.keyword_groups[0].head, "cover letter");
        assert!(out.keyword_groups[0]
            .synonyms
            .contains(&"application".to_string()));
    }

    #[test]
    fn multiword_key_overrides_style_guide() {
        let e = expander();
        let out = e.expand(intent_with(vec!["style"]), "find the style guide for our brand assets");
        assert_eq!(out.keyword_groups[0].head, "style guide");
        assert!(out.keyword_groups[0]
            .synonyms
            .contains(&"branding".to_string()));
    }

    #[test]
    fn multiword_override_noop_when_key_absent_in_query() {
        // query 不含多词键 → 单 token keyword 组不变。
        let e = expander();
        let out = e.expand(intent_with(vec!["slides"]), "find slides about budgets");
        assert_eq!(out.keyword_groups[0].head, "slides");
        assert_eq!(out.keyword_groups[0].synonyms, vec!["slideshow", "presentation"]);
    }

    #[test]
    fn multiword_override_dedups_when_two_tokens_map_to_same_key() {
        // 两个 keyword 都被同一多词键包含 → 覆盖后去重为单组。
        let e = expander();
        let out = e.expand(
            intent_with(vec!["cover", "letter"]),
            "find my cover letter",
        );
        assert_eq!(out.keyword_groups.len(), 1);
        assert_eq!(out.keyword_groups[0].head, "cover letter");
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-harness -- multiword_key_overrides_single_token_keyword multiword_key_overrides_style_guide multiword_override 2>&1 | tail -20`
Expected: FAIL —— 当前 `expand_one("cover")` 不查多词键，head 仍是 "cover"（或错扩到 album-art 组）。

- [ ] **Step 3: 新增 `multiword_keys` 方法**

在 `impl YamlSynonymExpander` 内（`gazetteer_lookup` 之后，:316 附近）新增：

```rust
    /// 返回所有含空格的多词词典键（zh + en）。
    fn multiword_keys(&self) -> impl Iterator<Item = &str> {
        self.zh_index
            .keys()
            .chain(self.en_index.keys())
            .map(String::as_str)
            .filter(|k| k.contains(' '))
    }

    /// Fix A：多词键覆盖。对每个已抽 keyword 组，在 query 中寻找**包含该 keyword**、
    /// 通过 [`is_pure_content_term`] 守护、且字面出现在 query 中的多词词典键；命中则用
    /// 多词键的组替换该 keyword 组。多个候选取最长（字符数），并列取 query 首现靠前者。
    /// query 与键均按小写比较。
    fn apply_multiword_override(&self, query: &str, groups: &mut [KeywordGroup]) {
        let lower = query.to_lowercase();
        for slot in groups.iter_mut() {
            let kw = slot.head.to_lowercase();
            let mut best: Option<(usize, usize, &str)> = None; // (字符长, 首现位置, key)
            for key in self.multiword_keys() {
                let key_l = key.to_lowercase();
                if !key_l.contains(&kw) {
                    continue;
                }
                let Some(pos) = lower.find(&key_l) else {
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
                    best = Some((len, pos, key));
                }
            }
            if let Some((_, _, key)) = best {
                *slot = self.expand_one(key);
            }
        }
    }
```

- [ ] **Step 4: 新增 `dedup_groups_by_head` free fn**

在文件中 `build_index`（:401）附近新增：

```rust
/// 按 head 去重 keyword 组，保留首现顺序（多词键覆盖后两 token 可能映射到同一组）。
fn dedup_groups_by_head(groups: &mut Vec<KeywordGroup>) {
    let mut seen = std::collections::HashSet::new();
    groups.retain(|g| seen.insert(g.head.clone()));
}
```

- [ ] **Step 5: 改写 `expand` 的「有 keyword」分支**

把 `expand`（:378-398）的 match 改为：

```rust
    fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent {
        let groups = match intent.search_keywords() {
            Some(kws) if !kws.is_empty() => {
                let mut gs = kws.iter().map(|kw| self.expand_one(kw)).collect::<Vec<_>>();
                self.apply_multiword_override(query, &mut gs);
                dedup_groups_by_head(&mut gs);
                gs
            }
            // 兼底：parser 无 keyword → 先查 gazetteer 词典内容词（BETA-15E），未命中再退到
            // 裸内容词兜底（让「英语」这类非词典裸词也能走内容搜索而非 match-all）。
            _ => self
                .gazetteer_lookup(query)
                .or_else(|| bare_content_keyword(query))
                .map(|matched| vec![self.expand_one(&matched)])
                .unwrap_or_default(),
        };
        let (groups, _warn_truncated) = cap_keyword_groups(groups, RUNTIME_KEYWORD_CAP);
        // _warn_truncated 在 Task 12 接到 Tracer 后通过 SynonymExpandEvent 上报
        ExpandedSearchIntent {
            base: intent,
            keyword_groups: groups,
        }
    }
```

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p locifind-harness -- multiword 2>&1 | tail -20`
Expected: PASS（4 个新 test + 既有 multi_keyword_intent_preserves_order 等不破）。

- [ ] **Step 7: 全 crate 测试 + fmt + clippy**

Run: `cargo test -p locifind-harness 2>&1 | tail -5 && cargo fmt -p locifind-harness --check && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 全过。

- [ ] **Step 8: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "fix(synonym): expand 多词键覆盖——单 token keyword 升级为多词词典键组"
```

---

## Task 3: 集成验证 — 召回评测清零 + v0.5 evals 不回归 + ci.sh

**Files:** 无源码改动（纯验证；若发现 office-02 仍漏，回到 Task 1/2 修，不在此 Task 加新逻辑）。

- [ ] **Step 1: 召回评测 en→100%**

Run: `cargo run -q -p locifind-evals --bin synonym_recall -- --only-failures 2>&1 | tail -20`
Expected: `en 100.0%` / 总召回 `100.0%` / 假阳 `0.0%`，无 `[FAIL]` 行。
若仍有 FAIL（特别是 recall-en-office-02）：回到 Task 1 Step 6 探查 parser keyword；若 parser 未抽 "minutes"，需评估是否在 Fix A 增加单词键覆盖路径（超出当前 spec，需回 spec 决策——本 Task 不擅自加）。

- [ ] **Step 2: v0.5 evals parser-only 不回归（硬门）**

Run: `cargo run -q -p locifind-evals --bin evals 2>&1 | tail -25`
Expected: parser-only **pass ≥ 472 / fail ≤ 2 / variant 命中 ≥ 99%**。
若 pass < 472：Fix B 引入回归 → 回退 Task 1 方案（收窄 `has_numeric_duration` 或还原 STRONG 列表），重跑直至 ≥472。

- [ ] **Step 3: 全套 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -15`
Expected: fmt + clippy(-D warnings) + build + test + synonym-recall 门全过。

- [ ] **Step 4: 无新增源码改动需提交则跳过 commit**

本 Task 通常无 commit（Task 1/2 已落库）。如 Step 1/2 触发回退修复，按修复内容补 commit。

---

## 验收对照（spec §4）

- en 召回 80%→100%、总召回→100%、假阳保持 0%（Task 3 Step 1）。
- v0.5 evals parser-only pass≥472（Task 3 Step 2 硬门）。
- 新增单测：harness 多词键覆盖 4 个 + parser 时长词数字上下文 2 个（Task 1/2）。
- `bash scripts/ci.sh` 全套绿（Task 3 Step 3）。
