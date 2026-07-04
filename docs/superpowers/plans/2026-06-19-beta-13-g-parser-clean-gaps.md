# BETA-13-G parser 干净缺口修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修三块干净 parser 缺口（截图内容子句路由 / 中文+英文类型名词→file_type / artist 自然措辞抽取），全程 v0.5 byte-equal 守护，把 v0.9 parser-only 通过率从 726 抬升而零回归。

**Architecture:** 全部改动在 `packages/intent-parser/src/parsers/`，对 `media_search.rs`（路由 + artist 抽取）与 `file_search.rs`（类型名词映射 + 关键词）做增量规则扩展。核心纪律：**每条规则只对 v0.5 不存在的新形态生效**，每个 task 完成立即重跑 v0.5 evals 确认 473 pass 且结果逐字节不变。

**Tech Stack:** Rust（rule-based parser，regex crate、OnceLock 缓存正则），评测工具 `cargo run -p locifind-evals --bin evals`。

**关联 spec：** [docs/superpowers/specs/2026-06-19-beta-13-g-parser-clean-gaps-design.md](../specs/2026-06-19-beta-13-g-parser-clean-gaps-design.md)

---

## 评测与 byte-equal 基线（每个 task 反复用）

跑通过率：
```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 2>/dev/null | tail -3
```
当前基线：v0.9 = **726 pass / 225 partial / 49 fail**；其中 v0.5 子集 = **473 pass / 25 partial / 2 fail**。

**byte-equal 闸门**（核心安全网，每个 task 改完必跑）：
```bash
# 在动手前先存基线一次（仅第一次）：
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-baseline.json
# 每次改完比对：v0.5 段必须逐字节相同
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | diff /tmp/v05-baseline.json - && echo "V0.5 BYTE-EQUAL OK"
```
任何 task 若 `diff` 非空即视为回归，必须收紧触发条件直到 v0.5 完全不变。

每个 task 收尾统一三件：`cargo fmt --check`、`cargo clippy -p locifind-intent-parser -- -D warnings`、`cargo test -p locifind-intent-parser`。

---

## Task 1: Fix 1 — 截图内容子句路由（媒体 → file_search）

**背景（实测）：** 文档内容子句（`正文里写着销售额下滑的报告`）**已经**走 file_search 且关键词正确；只有**截图**内容子句被强媒体词 `截图` 拽进 media_search 且关键词脏。本 task 只处理截图分支。

代表性失败（coverage，实测 expected vs actual）：
- `截图里写着已支付的` → 期望 `file_search{file_type:screenshot, keywords:[已支付]}`；实际 `media_search{media_type:screenshot, keywords:[里写着已支付的]}`。
- `截图里写着订单已发货的那张` → 期望 keywords `[订单已发货]`。
- `聊天截图里提到周五开会的` → 期望 keywords `[周五开会]`。
- `the screenshot that says payment successful` → 期望 `file_search{file_type:screenshot, keywords:[payment successful]}`。
- `screenshots that mention error 404` → keywords `[error 404]`。

**Files:**
- Modify: `packages/intent-parser/src/parsers/media_search.rs`（`is_media_query` 加内容子句闸门；新增 `detect_content_clause`）
- Modify: `packages/intent-parser/src/parsers/file_search.rs`（确保截图词→`file_type=screenshot` + 内容子句关键词干净抽取）
- Test: 同文件 `#[cfg(test)]` 模块

- [ ] **Step 1: 写失败测试（路由 + 字段）**

在 `media_search.rs` 测试模块加：
```rust
#[test]
fn content_screenshot_routes_to_file_search() {
    use locifind_search_backend::{FileType, SearchIntent};
    for (q, kw) in [
        ("截图里写着已支付的", "已支付"),
        ("截图里写着订单已发货的那张", "订单已发货"),
        ("聊天截图里提到周五开会的", "周五开会"),
    ] {
        let intent = crate::parse(q);
        let SearchIntent::FileSearch(fs) = intent else {
            panic!("{q} 应路由 file_search，实际 {intent:?}");
        };
        assert_eq!(fs.file_type, Some(vec![FileType::Screenshot]), "{q}");
        assert_eq!(fs.keywords, Some(vec![kw.to_owned()]), "{q}");
    }
}

#[test]
fn screenshot_without_content_clause_stays_media() {
    // byte-equal 守护：纯「截图 + 时间」无内容子句仍走 media（与 v0.5 一致）
    use locifind_search_backend::SearchIntent;
    assert!(matches!(crate::parse("最近三天的截图"), SearchIntent::MediaSearch(_) | SearchIntent::FileSearch(_)));
    // 注：上一条期望见决策清单（v0.5↔coverage 冲突项，本 task 不强求；此断言仅防 panic）
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser content_screenshot_routes_to_file_search -- --nocapture`
Expected: FAIL（当前路由 media_search）

- [ ] **Step 3: 实现内容子句检测 + 闸门**

在 `media_search.rs` 加纯函数（识别「内容子句」+ 抽取干净内容短语）：
```rust
/// BETA-13-G：检测「内容子句」——用户按内容/正文文字搜（非按文件名/类型）。
/// 命中返回干净内容短语（剥除子句引导词、容器尾巴）。仅用于截图分支重路由 +
/// file_search 关键词注入。中文：里写着/写着/写了/里提到/提到/里面有 X（后接「的…」结尾）。
/// 英文：that says / says / mention(s) / shows X。
pub(crate) fn detect_content_clause(input: &str) -> Option<String> {
    use regex::Regex;
    use std::sync::OnceLock;
    static RE_ZH: OnceLock<Regex> = OnceLock::new();
    let re_zh = RE_ZH.get_or_init(|| {
        // 引导词后到「的」前的内容；末尾「的那张/的报错/的提示/的」等容器尾巴在捕获组外
        Regex::new(r"(?:里写着|里写了|写着|写了|里提到|提到|里面有|里面提到)\s*([^，。,]+?)(?:的那张|的那个|的报错|的提示|的错误提示|的)?$")
            .expect("regex")
    });
    if let Some(cap) = re_zh.captures(input.trim()) {
        let s = cap[1].trim();
        if !s.is_empty() { return Some(s.to_owned()); }
    }
    static RE_EN: OnceLock<Regex> = OnceLock::new();
    let re_en = RE_EN.get_or_init(|| {
        Regex::new(r"(?i)(?:that says|says|that mentions?|mentions?|that shows?|shows?)\s+(.+?)\s*$")
            .expect("regex")
    });
    if let Some(cap) = re_en.captures(input.trim()) {
        let s = cap[1].trim();
        if !s.is_empty() { return Some(s.to_owned()); }
    }
    None
}

/// 是否为截图查询（强截图词）。
fn has_screenshot_word(lower: &str) -> bool {
    ["截图", "截屏", "screenshot", "screenshots"].iter().any(|w| lower.contains(w))
}
```

在 `is_media_query` 顶部（strong-media 判定之前）加闸门：
```rust
pub(crate) fn is_media_query(lower: &str) -> bool {
    // BETA-13-G：截图 + 内容子句 → 按内容搜，交 file_search（file_type=screenshot + keywords）。
    // 仅截图分支；纯「截图+时间/size」无内容子句不受影响（保持 v0.5）。
    if has_screenshot_word(lower) && detect_content_clause(lower).is_some() {
        return false;
    }
    // ……（其余原逻辑不变）
```
> 注：`is_media_query` 收的是 `lower`；`detect_content_clause` 对中文大小写无关、英文用 `(?i)`，传 lower 即可。

- [ ] **Step 4: file_search 侧——截图词映射 + 内容子句关键词**

确认 `file_search.rs` 的类型词映射含 `截图/screenshot → FileType::Screenshot`（grep 已见 line 1039-1040 含 screenshot；若是别处类型表，对齐之）。再在 `parse_file_search` 的关键词抽取里，对命中 `detect_content_clause` 的查询用其返回值作为 keywords（剥离 `截图里写着` 等引导词），避免落入通用残留抽取产生脏词。最小实现：在 `parse_file_search` 开头：
```rust
// BETA-13-G：内容子句查询——关键词直接取干净内容短语（截图等内容截图重路由至此）。
if let Some(content) = super::media_search::detect_content_clause(input) {
    // 仍照常解析 file_type（截图/文档类型词）、时间、location；仅 keywords 用 content。
    // 实现：在最终组装 keywords 处优先用 vec![content]，其余字段走原流程。
}
```
> 实现细节按 file_search 现有结构落位：目标是 `keywords = Some(vec![content])`，`file_type` 由类型词映射给出（截图→screenshot）。`sort` 保持 `modified_desc`（与 coverage 期望一致）。

- [ ] **Step 5: 跑测试确认通过 + byte-equal**

Run: `cargo test -p locifind-intent-parser content_screenshot_routes_to_file_search`
Expected: PASS
Run byte-equal 闸门（见顶部）：`... diff /tmp/v05-baseline.json -` → 必须 `V0.5 BYTE-EQUAL OK`
Run: `cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 2>/dev/null | tail -3` → coverage fail 下降、无新增 fail

- [ ] **Step 6: fmt/clippy/test + commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/media_search.rs packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G Fix1：截图内容子句路由至 file_search + 干净关键词"
```

---

## Task 2: Fix 3a — 中文尾置类型名词 → file_type

**背景（实测）：** 一批 coverage 是 file_search 且关键词已对，**只缺 file_type**，因为尾置类型名词（`…的报告`/`…的表`）未映射：
- `找一份装修预算的表` → 期望 `spreadsheet`（现 None），keywords `[装修预算]` 已对。
- `找一下季度 KPI 的表` / `帮我找一下 budget 预算表` → `spreadsheet`。
- `正文里写着销售额下滑的报告` → `document`。
- `找一下里面写了不可抗力条款的合同` / `里面提到知识产权归属的协议` → `document`。
- `提到李娜的简历` / `内容包含期末考试范围的笔记` → `document`。
- `内容提到现金流的财务报表` → `spreadsheet`。

**byte-equal 陷阱（实测）：** v0.5 的 `报告` 全部出现在 `名字里有「合成报告」的文件` 里——**尾名词是「文件」，「报告」在引号 keyword 内**。因此规则**只映射查询尾部 head 名词**即安全（v0.5 那些尾名词是「文件」=不映射）。v0.5 无任何 `表` 字 case。

**类型名词映射表（尾置 head 名词 → file_type）：**
| 名词 | file_type |
|---|---|
| 表 / 表格 / 报表 / 财务报表 / 账本 | spreadsheet |
| 报告 / 合同 / 协议 / 简历 / 笔记 / 学习笔记 / 学习资料 / 资料 / 说明文件 / 单据 / 邮件草稿 / 文档 / 文件夹除外 | document |

> 注：`文档` 已映射（`内容里提到季度营收的文档` 已 pass），无需重复；新增的是 `报告/合同/协议/简历/笔记/资料/单据/草稿/表/报表/账本` 等。

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs`（类型名词映射 + 尾置 head 名词 gating）
- Test: 同文件测试模块

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn trailing_chinese_type_noun_maps_file_type() {
    use locifind_search_backend::{FileType, SearchIntent};
    let cases = [
        ("找一份装修预算的表", FileType::Spreadsheet, "装修预算"),
        ("内容提到现金流的财务报表", FileType::Spreadsheet, "现金流"),
        ("正文里写着销售额下滑的报告", FileType::Document, "销售额下滑"),
        ("找一下里面写了不可抗力条款的合同", FileType::Document, "不可抗力条款"),
        ("提到李娜的简历", FileType::Document, "李娜"),
    ];
    for (q, ft, kw) in cases {
        let SearchIntent::FileSearch(fs) = crate::parse(q) else { panic!("{q}") };
        assert_eq!(fs.file_type, Some(vec![ft]), "{q} file_type");
        assert_eq!(fs.keywords, Some(vec![kw.to_owned()]), "{q} keywords");
    }
}

#[test]
fn quoted_keyword_noun_not_mapped() {
    // byte-equal 守护：v0.5 形态——尾名词是「文件」，报告在 keyword 内，不映射
    use locifind_search_backend::SearchIntent;
    let SearchIntent::FileSearch(fs) = crate::parse("找本周文稿名字里有「合成报告」的文件") else { panic!() };
    assert_eq!(fs.file_type, None, "尾名词为「文件」不应映射 document");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser trailing_chinese_type_noun_maps_file_type`
Expected: FAIL（file_type 为 None）

- [ ] **Step 3: 实现尾置类型名词映射**

在 `file_search.rs` 加纯函数并在 `parse_file_search` 组装 file_type 处调用（仅当现有 file_type 推导为空时填）：
```rust
/// BETA-13-G：查询尾部 head 名词 → file_type（仅当尾名词本身是类型名词时映射，
/// 避免「…「合成报告」的文件」这类把 keyword 内名词误当类型）。
/// 取「的」之后到结尾的最后一段名词匹配。
fn trailing_type_noun_file_type(input: &str) -> Option<locifind_search_backend::FileType> {
    use locifind_search_backend::FileType;
    let tail = input.trim().rsplit('的').next().unwrap_or("").trim();
    // 去掉量词/语气尾巴（份/个/吧/呢/。等）
    const SPREADSHEET: &[&str] = &["财务报表", "报表", "表格", "表", "账本"];
    const DOCUMENT: &[&str] = &[
        "报告", "合同", "协议", "简历", "学习笔记", "笔记", "学习资料", "资料",
        "说明文件", "单据", "邮件草稿", "草稿",
    ];
    if SPREADSHEET.iter().any(|w| tail == *w || tail.ends_with(w)) { return Some(FileType::Spreadsheet); }
    if DOCUMENT.iter().any(|w| tail == *w || tail.ends_with(w)) { return Some(FileType::Document); }
    None
}
```
> 关键 gating：用 `rsplit('的').next()` 取最后一个「的」之后的尾段；`名字里有「合成报告」的文件` 尾段是「文件」→ 不命中。`装修预算的表` 尾段是「表」→ spreadsheet。在 `parse_file_search` 里，仅当原 file_type 推导为 None 时用此结果。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser trailing_chinese_type_noun_maps_file_type quoted_keyword_noun_not_mapped`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（**特别核对** `「合成报告」` 7 条与 `表` 无 v0.5 冲突）
Run v0.9 → partial 下降、无新增 fail

- [ ] **Step 5: commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G Fix3a：中文尾置类型名词→file_type（gating 尾 head 名词）"
```

---

## Task 3: Fix 3b — 英文 head 类型名词 → file_type

**背景（实测）：**
- `a document about the marketing plan` / `show me the document about the project roadmap` / `find the document about machine learning` → `document`（keywords 已对）。
- `documents that mention quarterly revenue` / `documents that talk about data privacy` → `document`。
- `archives between 10 and 100 MB` → `archive`。
- `documents modified today` → `document`。

**byte-equal 陷阱（实测，关键）：** v0.5 的 `documents` 几乎都是 `… in documents` = **位置（Documents 文件夹）**，期望 file_type 由别的类型词决定或 None。因此英文规则**只在 head 位置**（`a/the document(s) about/that/whose/modified…` 在句首区域）映射，**绝不**对 `in documents` 的位置语义生效。

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs`
- Test: 同文件测试模块

- [ ] **Step 1: 写失败测试（含 byte-equal 守护）**

```rust
#[test]
fn english_head_type_noun_maps_file_type() {
    use locifind_search_backend::{FileType, SearchIntent};
    for (q, ft) in [
        ("a document about the marketing plan", FileType::Document),
        ("documents that mention quarterly revenue", FileType::Document),
        ("documents modified today", FileType::Document),
        ("archives between 10 and 100 MB", FileType::Archive),
    ] {
        let SearchIntent::FileSearch(fs) = crate::parse(q) else { panic!("{q}") };
        assert_eq!(fs.file_type, Some(vec![ft]), "{q}");
    }
}

#[test]
fn in_documents_is_location_not_type() {
    // byte-equal 守护：「in documents」是位置 Documents 夹，不得映射 file_type=document
    use locifind_search_backend::SearchIntent;
    let SearchIntent::FileSearch(fs) = crate::parse("find screenshots from this week in documents") else { return };
    assert_ne!(fs.file_type, Some(vec![locifind_search_backend::FileType::Document]),
        "in documents 不应判 document 类型");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser english_head_type_noun_maps_file_type`
Expected: FAIL

- [ ] **Step 3: 实现 head-gated 英文映射**

```rust
/// BETA-13-G：英文 head 类型名词 → file_type。仅句首区域的 a/the/this/that/复数裸词 +
/// document(s)/archive(s)/…，且其后接 about/that/whose/modified/含约束——区别于「in documents」位置义。
fn english_head_type_noun_file_type(lower: &str) -> Option<locifind_search_backend::FileType> {
    use locifind_search_backend::FileType;
    use regex::Regex;
    use std::sync::OnceLock;
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^(?:a |the |this |that |these |those |find |show me |list )*(documents?|archives?|spreadsheets?|presentations?)\b")
            .expect("regex")
    });
    // 「in documents」位置义：documents 前有 in/inside/under → 不映射
    if Regex::new(r"\b(?:in|inside|under)\s+documents?\b").ok()?.is_match(lower) {
        // 仍可能 head 另有类型词；但若整句仅此 documents，则交由原 location 逻辑
    }
    let cap = re.captures(lower.trim())?;
    let word = &cap[1];
    let ft = if word.starts_with("archive") { FileType::Archive }
        else if word.starts_with("spreadsheet") { FileType::Spreadsheet }
        else if word.starts_with("presentation") { FileType::Presentation }
        else { FileType::Document };
    Some(ft)
}
```
> gating 要点：正则 `^…(documents?|archives?…)` 锚定**句首**，故 `find screenshots … in documents`（documents 在句尾）不命中。在 `parse_file_search` 中仅当原 file_type 为 None 时采用。TDD 中按 byte-equal 结果微调（如发现某 v0.5 句首 documents 位置义，追加排除）。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser english_head_type_noun_maps_file_type in_documents_is_location_not_type`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（**逐条核对** v0.5 所有 `documents` 句首形态不被误判）
Run v0.9 → partial 下降、无新增 fail

- [ ] **Step 5: commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G Fix3b：英文 head 类型名词→file_type（排除 in documents 位置义）"
```

---

## Task 4: Fix 2 — artist 自然措辞抽取修缮

**背景（实测 20 条 coverage artist partial），三类病：**

1. **前缀污染**（regex 贪婪吞动词/位置/只取末 token）：
   - `找邓紫棋的歌曲` → 抽成 `找邓紫棋`（应 `邓紫棋`，须剥前置 `找`）。
   - `音乐目录里周杰伦的歌` → `里周杰伦`（应 `周杰伦`，须剥位置前缀）。
   - `Taylor Swift 的歌` → `Swift`（应 `Taylor Swift`，须取完整多 token）。
   - `周杰伦《范特西》专辑里的歌` → `专辑里`（应 `周杰伦`）。
2. **漏抽**（`的` 与 `歌` 之间夹修饰 / 英文名 + 中文「的歌」/ 视频）：
   - `王菲的爵士风格歌曲`→`王菲`、`周杰伦超过4分钟的无损歌曲`→`周杰伦`、`找五月天短于4分钟的歌`→`五月天`。
   - `Taylor Swift 的无损歌曲`→`Taylor Swift`、`Coldplay 超过5分钟的歌`→`Coldplay`。
   - `music videos by Adele`→`Adele`（+ media_type=video）、`Eason 的 music video`→`Eason`（+video）。
   - `周杰伦《范特西》专辑里的歌`/`薛之谦《绅士》专辑`/`找毛不易《消愁》`→`X《…》` 取 X。
   - `陈奕迅 浮夸 这首歌`→`陈奕迅`；`play the song Shape of You by Ed Sheeran`→`Ed Sheeran`。
3. **过抽**（应为 None 却抽出，多因 `叫 X 的歌` 中 X 是 title / quality 词）：
   - `找一首叫 七里香 的歌` → 误抽 `七里香`（应 None，七里香=title）。
   - `找一首叫 Hello 的歌` → 误抽 `Hello`（应 None）。
   - `找一些高品质的歌` → 误抽 `些高品质`（应 None，高品质=quality）。

**Files:**
- Modify: `packages/intent-parser/src/parsers/media_search.rs`（`extract_artist_by_structure` 重写 + `is_stopword_artist` 扩充 + `叫`-title 抑制 + 《》提取 + videos-by 路由/抽取）
- Test: 同文件测试模块

- [ ] **Step 1: 写失败测试（正例 + 反例）**

```rust
#[test]
fn artist_extraction_natural_phrasing() {
    use locifind_search_backend::SearchIntent;
    let positive = [
        ("找邓紫棋的歌曲", "邓紫棋"),
        ("音乐目录里周杰伦的歌", "周杰伦"),
        ("Taylor Swift 的歌", "Taylor Swift"),
        ("王菲的爵士风格歌曲", "王菲"),
        ("周杰伦超过4分钟的无损歌曲", "周杰伦"),
        ("找五月天短于4分钟的歌", "五月天"),
        ("Coldplay 超过5分钟的歌", "Coldplay"),
        ("薛之谦《绅士》专辑", "薛之谦"),
        ("陈奕迅 浮夸 这首歌", "陈奕迅"),
        ("play the song Shape of You by Ed Sheeran", "Ed Sheeran"),
    ];
    for (q, a) in positive {
        let intent = crate::parse(q);
        let artist = match &intent {
            SearchIntent::MediaSearch(m) => m.artist.clone(),
            _ => None,
        };
        assert_eq!(artist.as_deref(), Some(a), "{q} 应抽 artist={a}，得 {artist:?}");
    }
}

#[test]
fn artist_extraction_no_false_positive() {
    use locifind_search_backend::SearchIntent;
    for q in ["找一首叫 七里香 的歌", "找一首叫 Hello 的歌", "找一些高品质的歌"] {
        if let SearchIntent::MediaSearch(m) = crate::parse(q) {
            assert_eq!(m.artist, None, "{q} 不应抽 artist，得 {:?}", m.artist);
        }
    }
}

#[test]
fn music_video_by_artist_routes_video() {
    use locifind_search_backend::{MediaType, SearchIntent};
    for (q, a) in [("music videos by Adele", "Adele"), ("Eason 的 music video", "Eason")] {
        let SearchIntent::MediaSearch(m) = crate::parse(q) else { panic!("{q}") };
        assert_eq!(m.artist.as_deref(), Some(a), "{q}");
        assert_eq!(m.media_type, MediaType::Video, "{q} media_type");
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser artist_extraction_natural_phrasing artist_extraction_no_false_positive music_video_by_artist_routes_video`
Expected: FAIL

- [ ] **Step 3: 实现（分步在 `extract_artist_by_structure` 与配套）**

要点（按 TDD 红→绿逐条加，最终满足上述断言；保持对 v0.5 既有 artist 锚点不变）：
1. **前置剥离**：抽取前剥掉句首动词 `找/搜/搜索/帮我找/想找/我想找/找一下/找找/列出` 与位置前缀 `…目录里/…文件夹里/…里`（取最后一个位置词后的片段）。
2. **`叫 X` 抑制**：若命中 `叫\s*X\s*的歌` 形态，X 归 title，**artist 返回 None**（修 七里香/Hello 过抽）。
3. **中文夹修饰**：正则放宽为 `([\p{Han}]{2,4}|[A-Za-z][\w\- ]*?)(?:《[^》]*》)?(?:专辑)?(?:[^的]{0,8})?的(?:[^歌]{0,6})?(?:歌曲|歌|音乐)`——允许 `的` 后夹 `无损/爵士风格` 等修饰；但候选 X 经 `is_stopword_artist` 过滤（加入 `高品质/些高品质/无损/高清/一些/一首` 等）。
4. **英文完整名**：`X 的歌` 里 X 为英文时取**完整连续大写 token 串**（`Taylor Swift`），不止末 token；`(?:songs?|tracks?|music)\s+by\s+X` 与 `the song .+ by X` 都取 X。
5. **《》提取**：`X《…》` / `X《…》专辑` → artist=X（`薛之谦《绅士》专辑`、`找毛不易《消愁》`）。
6. **`X 浮夸 这首歌`**：`^X\s+\S+\s*这首歌` → artist=X（`陈奕迅`）。
7. **video 路由 + 抽取**：`music videos by X` / `X 的 music video` / `X的音乐视频` → 在 `is_media_query` 命中 media（含 video 词），`detect_media_type` 判 video，`extract_artist` 同样抽 X。确认 `has_audio_metadata_signal`/路由让这些进 media（`music video` 含 `music` 可能已命中；逐条验证）。

> 实现纪律：每加一条规则即重跑 v0.5 byte-equal。v0.5 有 4 条 artist partial + 若干 pass 锚点（如 `张学友`/`Ed Sheeran` 系列），**任何改动后 v0.5 必须逐字节不变**。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser artist_extraction_natural_phrasing artist_extraction_no_false_positive music_video_by_artist_routes_video`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（**核心风险点**：artist 规则与 v0.5 共享，必须零回归）
Run v0.9 → artist partial 下降、无新增 fail

- [ ] **Step 5: commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/media_search.rs
git commit -m "BETA-13-G Fix2：artist 自然措辞抽取（剥前缀/完整EN名/夹修饰/《》/video，抑制叫-title 过抽）"
```

---

## Task 5: 标注冲突决策清单文档

**Files:**
- Create: `docs/reviews/beta-13-g-annotation-conflicts.md`

- [ ] **Step 1: 生成冲突清单数据**

```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null > /tmp/v09-after.json
```
用脚本从 `/tmp/v09-after.json` 提取仍失败的 case，按以下三类归并（query + v0.5 现判 + coverage 现判）：
1. **v0.5↔coverage 契约冲突**：`video/截图 + size/time`（v0.5 media、coverage file）——列形态汇总 + 计数。
2. **d9 多类型标注自相矛盾**：`pdf 和 doc`→null vs `pdf 和图片`→array 等，列矛盾对。
3. **其余标注边界**：location-vs-keyword、`备份文件` 切分、否定 `exclude_*` 触发等。

- [ ] **Step 2: 写决策清单文档**

文档结构（每条给「保留 v0.5 / 改判 coverage / 分流」三选项 + 影响计数 + byte-equal 代价）：
```markdown
# BETA-13-G parser 无解缺口：标注冲突决策清单
> 这些缺口 parser 改不动（动则破 v0.5 byte-equal 或标注自相矛盾），需用户逐条拍板后另起任务（涉及评测集 re-baseline）。
## 一、v0.5↔coverage 契约冲突：video/截图 + size/time
（表格：形态 | 样例 query | v0.5 现判 | coverage 现判 | 影响数 | 选项）
## 二、d9 多类型标注自相矛盾
## 三、其余标注边界
## 附：本会话干净 fix 已吃掉的部分（前后通过率对照）
```

- [ ] **Step 3: commit**

```bash
git add docs/reviews/beta-13-g-annotation-conflicts.md
git commit -m "BETA-13-G：标注冲突决策清单（parser 无解缺口，待用户拍板）"
```

---

## Task 6: 整体回归门

- [ ] **Step 1: 全量验证**

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | diff /tmp/v05-baseline.json - && echo "V0.5 BYTE-EQUAL OK"
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 2>/dev/null | tail -3
```
Expected: fmt 净 / clippy 0 / test 全绿 / **v0.5 byte-equal OK** / v0.9 pass 数 > 726、fail ≤ 49（无新增）。

- [ ] **Step 2: 记录实测增益**

把 v0.9 前后通过率（726 → 实测）如实记入决策清单文档「附」节，**不凑指标**。

> 收工（STATUS/ROADMAP/最终 commit）由用户说「收工」时按 CONVENTIONS §3 执行，不在本计划内。

---

## Self-Review 检查

- **Spec 覆盖**：Fix 1（Task 1）/ Fix 2（Task 4）/ Fix 3（Task 2+3，按中/英文与实测重新切分）/ 决策清单（Task 5）/ 验证（Task 6）全覆盖。
- **byte-equal**：每个改动 task 都含 `/tmp/v05-baseline.json` diff 闸门 + 已逐条核对已知陷阱（`「合成报告」`、`in documents`、v0.5 artist 锚点）。
- **类型一致**：`detect_content_clause`（pub(crate) in media_search）被 file_search 调用；`FileType` 枚举值（Screenshot/Spreadsheet/Document/Archive/Presentation）与 schema 一致；测试用 `crate::parse` 端到端入口。
- **无占位符**：每个实现步给了具体函数/正则/映射表/真实 query。artist Task 3 因 regex 须 TDD 迭代，给了分步要点 + 完整断言驱动，符合 rule-based parser 域的合理粒度。
