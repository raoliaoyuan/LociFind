# BETA-13 screenshot parser follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 修两条 coverage 对齐暴露的干净 screenshot parser bug——英文词数字时间（`last three days`）+ media 路径 name 排序，全程 v0.5 byte-equal 守护。

**Architecture:** 纯增量规则扩展，改 `packages/intent-parser/src/parsers/{common.rs, media_search.rs}`。Fix A 给相对时间 match 加英文词形分支 + screenshot 抽取器 stop 数字词；Fix B 给 `parse_media_search` sort 块加 name 检测（镜像 file_search BETA-13-G6）。只命中 v0.5 不存在的形态。

**Tech Stack:** Rust，评测 `cargo run -p locifind-evals --bin evals`。

**关联 spec：** [docs/superpowers/specs/2026-06-19-beta-13-screenshot-parser-followup-design.md](../specs/2026-06-19-beta-13-screenshot-parser-followup-design.md)

---

## 基线与闸门

当前 v0.9：**774 pass / 200 partial / 26 fail**；v0.5 = 473 pass。基线 `/tmp/v05-baseline.json`（若无：`cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-baseline.json`）。

byte-equal 闸门（规范化，禁裸 diff）：
```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-now.json && python3 /tmp/v05check.py
```
v0.9 计数：
```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | python3 -c "import json,sys;from collections import Counter;print(dict(Counter(x['result']['type'] for x in json.load(sys.stdin))))"
```
每 task 收尾：`cargo fmt --check`、`cargo clippy -p locifind-intent-parser --all-targets -- -D warnings`、`cargo test -p locifind-intent-parser`。

---

## Task 1: Fix A — 英文词数字时间 + screenshot stop 数字词

**实测目标**：`screenshots from the last three days` → media{screenshot, created_time:last_3_days, sort:created_desc, 无 keyword}（id v09-d5-en-003），当前 partial（无 time、漏 keyword "three"、sort relevance）。

**Files:**
- Modify: `packages/intent-parser/src/parsers/common.rs`（相对时间 match，约 line 161-194，加英文词形）
- Modify: `packages/intent-parser/src/parsers/media_search.rs`（`extract_screenshot_keywords` 的 `stop_words` 加数字词）
- Test: 两文件测试模块

- [ ] **Step 1: 写失败测试**

media_search.rs 测试模块：
```rust
#[test]
fn en_word_number_time_screenshot_passes() {
    use locifind_search_backend::{MediaType, RelativeTime, SearchIntent, SortOrder, TimeExpression};
    let SearchIntent::MediaSearch(m) = crate::parse("screenshots from the last three days") else {
        panic!("应 media_search");
    };
    assert_eq!(m.media_type, MediaType::Screenshot);
    assert_eq!(m.created_time, Some(TimeExpression::Relative { value: RelativeTime::Last3Days }));
    assert_eq!(m.sort, Some(SortOrder::CreatedDesc));
    assert_eq!(m.keywords, None, "数字词 three 不应漏成 keyword");
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo test -p locifind-intent-parser en_word_number_time_screenshot_passes`
Expected: FAIL（created_time None / keywords 含 "three" / sort relevance）

- [ ] **Step 3a: common.rs 加英文词形时间分支**

在相对时间 match 的对应分支加英文词数字形（与既有 `last 3 days` 并列，纯 contains 增量）：
- Last3Days 分支加：`|| lower.contains("last three days") || lower.contains("past three days")`
- Last7Days 分支加：`|| lower.contains("last seven days") || lower.contains("past seven days")`
- Last14Days 分支加：`|| lower.contains("last two weeks") || lower.contains("past two weeks") || lower.contains("last fourteen days")`
- Last30Days 分支加：`|| lower.contains("last thirty days") || lower.contains("past thirty days")`

> 只增 contains 字符串，不动结构。en-003 仅需 `last three days`；其余词形为合理对称补全（v0.5 用数字形，词形零冲突）。

- [ ] **Step 3b: media_search.rs screenshot stop 加数字词**

`extract_screenshot_keywords` 的 `stop_words` 数组里（英文 stop words 区段）增补：
```rust
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
```
> 使 `three` 等不漏成 screenshot keyword。这些词作截图内容关键词概率极低，且 stop 比对大小写不敏感。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser en_word_number_time_screenshot_passes`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`
Run v0.9 → en-003 转 pass（pass 774→775）、无新增 fail

- [ ] **Step 5: commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser --all-targets -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/common.rs packages/intent-parser/src/parsers/media_search.rs
git commit -m "BETA-13 screenshot follow-up Fix A：英文词数字时间 + screenshot stop 数字词"
```

---

## Task 2: Fix B — media 路径 name 排序

**实测目标**：`上个月的截图按名字排`（id v09-d5-zh-038）→ sort 应 name_asc（当前 created_desc）。注：keyword `["按名字排"]` 仍漏（CJK 抽取器留 follow-up），故本 case 仍 partial，本 task 只修 sort 字段 + media/file 路径一致。

**Files:**
- Modify: `packages/intent-parser/src/parsers/media_search.rs`（`parse_media_search` sort 决策块，约 line 993）
- Test: 同文件测试模块

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn media_path_honors_name_sort() {
    use locifind_search_backend::{SearchIntent, SortOrder};
    // 媒体查询显式「按名字排」→ name_asc（对齐 file_search BETA-13-G6）
    let SearchIntent::MediaSearch(m) = crate::parse("上个月的截图按名字排") else {
        panic!("应 media_search");
    };
    assert_eq!(m.sort, Some(SortOrder::NameAsc));
    // 倒序变体
    let SearchIntent::MediaSearch(m2) = crate::parse("视频按名字倒序排") else {
        panic!("应 media_search");
    };
    assert_eq!(m2.sort, Some(SortOrder::NameDesc));
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo test -p locifind-intent-parser media_path_honors_name_sort`
Expected: FAIL（sort=created_desc / relevance）

- [ ] **Step 3: 实现 media name sort**

在 `parse_media_search` 的 `let sort = if ...` 链**最前**加 name 检测（显式 name 排序优先于隐式 size/created）：
```rust
    let sort = if lower.contains("按名字")
        || lower.contains("按名称")
        || lower.contains("名字排")
        || lower.contains("名称排")
        || lower.contains("by name")
    {
        // BETA-13 follow-up：media 路径对齐 file_search BETA-13-G6 名称排序
        if lower.contains("倒序") || lower.contains("降序") || lower.contains("name desc") {
            Some(SortOrder::NameDesc)
        } else {
            Some(SortOrder::NameAsc)
        }
    } else if has_explicit_size_threshold(lower) || has_size_desc_sort_word(lower) {
        Some(SortOrder::SizeDesc)
    } else if /* …既有分支保持不变… */
```
> 把既有 `let sort = if has_explicit_size_threshold...` 改成在其前插入 name 分支，其余分支原样保留。词集与 file_search `explicit_sort_override`（file_search.rs:237-251）一致。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser media_path_honors_name_sort`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（v0.5 media 零 `名字/name`）
Run v0.9 → 无新增 fail；zh-038 sort 转正（仍 partial 因 keyword 漏，符合预期）

- [ ] **Step 5: commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser --all-targets -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/media_search.rs
git commit -m "BETA-13 screenshot follow-up Fix B：media 路径识别 name 排序（对齐 file_search）"
```

---

## Task 3: 整体回归门

- [ ] **Step 1: 全量验证**

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-now.json && python3 /tmp/v05check.py
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | python3 -c "import json,sys;from collections import Counter;print(dict(Counter(x['result']['type'] for x in json.load(sys.stdin))))"
```
Expected: fmt 净 / clippy 0 / test 全绿 / **V0.5 BYTE-EQUAL OK** / v0.9 pass=775（+1）、fail≤25、无新增 fail。

- [ ] **Step 2: 如实记录**（v0.9 774→775；zh-003/zh-038 keyword 漏抽仍为 follow-up）。

> 收工由用户「收工」时执行。

---

## Self-Review 检查
- **Spec 覆盖**：Fix A（Task 1）/ Fix B（Task 2）/ 验证（Task 3）全覆盖。
- **byte-equal**：两 task 含闸门；v0.5 对 `last <word> days` 与 media `名字/name` 已 grep 确认 0 命中。
- **类型一致**：`RelativeTime::Last3Days`/`TimeExpression::Relative`/`SortOrder::NameAsc/NameDesc/CreatedDesc`、`MediaType::Screenshot` 均现有枚举；测试用 `crate::parse` 端到端。
- **无占位符**：每步给具体 contains 串/数字词/sort 分支与断言。
- **已知局限明确**：zh-038 keyword 仍漏 → 仍 partial，Task 2 只修 sort（spec 已声明）。
