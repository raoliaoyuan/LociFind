# BETA-13 screenshot parser follow-up：词数字时间 + media name sort（设计）

> 日期：2026-06-19 · 工具：Claude Code (Opus 4.8) · 阶段：B
> 来源：[决策清单 §1 更新](../../reviews/beta-13-g-annotation-conflicts.md) 由 coverage 对齐暴露的 screenshot parser bug
> 同分支前序：coverage 对齐 v0.5 契约（8 条 video/screenshot→media，commit 3f26486）

## 1. 背景

coverage 对齐 v0.5 契约后，3 条 screenshot 标注转为正确 media 形态，但 parser 字段 bug 使其停在 partial。本 follow-up 取其中**干净低风险**两项；中文 keyword 漏抽（脆弱 CJK 串手术、触 20 条 v0.5 screenshot 共享路径）留后续。

## 2. 目标与非目标

**目标**
- Fix A：英文 `last/past <数字词> days` 时间解析 + screenshot 抽取器 stop 数字词 → `screenshots from the last three days` 转 pass。
- Fix B：media 路径识别 `按名字排序/by name` → NameAsc/NameDesc（对齐 file_search BETA-13-G6），media 与 file 路径一致。

**非目标**
- 不改 `extract_screenshot_keywords` 的中文连续串残留逻辑（zh-003 `三天的截图` / zh-038 `按名字排` 漏抽）——脆弱、高 byte-equal 风险，留 follow-up。zh-038 本次只修 sort，仍 partial。
- 不碰 fixture（coverage 已在前序 commit 改好）。

## 3. 约束
- **v0.5 byte-equal**（473，规范化 `/tmp/v05check.py`）。两 fix 均只命中 v0.5 不存在的形态（v0.5 时间用数字/序数 `third one`、media 零 `名字/name`，已 grep 确认）。
- Rust：fmt + clippy(`--all-targets -D warnings`) + test 全绿。
- 改动限 `packages/intent-parser/src/parsers/`。

## 4. 设计

### 4.1 Fix A — 英文词数字时间 + 数字词 stop
**问题**：`screenshots from the last three days` → parser 未解析 `last three days`（"three" 是词非数字），漏成 keyword `["three"]` + sort=relevance_desc（无 time）。期望 media{screenshot, created_time:last_3_days, sort:created_desc, 无 keyword}。

**机制**：
1. 英文相对时间解析处（`last/past N days/weeks/months/years`）：在匹配数字前，先把英文数字词 `one..ten`（必要时 a/an→1）映射为数字，使 `last three days` 等价 `last 3 days` → 复用现有 `last_N_days` 归一（last_3_days）。落点在 time 解析（`common.rs` 的 `parse_time_fields` 或 file_search 的相对时间正则——实现时定位）。
2. `extract_screenshot_keywords`（media_search.rs）的 `stop_words` 增补英文数字词 `one/two/three/four/five/six/seven/eight/nine/ten`，使其不漏成 keyword。

**byte-equal 安全**：v0.5 无 `last <word> days` 形态（仅 `third one` 序数，不匹配）；数字词加入 screenshot stop 只影响截图查询、且这些词作内容词的概率极低。跑前 grep 复核 + 闸门。

### 4.2 Fix B — media 路径 name sort
**问题**：`上个月的截图按名字排` → media sort 给 created_desc，未认 `按名字排`。期望 sort=name_asc。

**机制**：`parse_media_search` 的 sort 决策块加 name 排序检测（对齐 file_search BETA-13-G6 的 NameAsc/NameDesc 词集：`按名字排序/按名字排/按文件名` + `倒序/降序`→NameDesc，否则 NameAsc；英文 `by name` + `desc/descending`）。置于 size_desc 等判定的合适优先级（显式 name 排序应优先于隐式 created_desc）。

**byte-equal 安全**：v0.5 media_search 零 `名字/name`（已 grep 确认）；新增只影响含 name 排序词的 media 查询。

**已知局限**：zh-038 的 keyword `按名字排` 仍漏（Fix 不碰 CJK 抽取器）→ zh-038 sort 转正但仍 partial。本 fix 价值=sort 字段正确 + media/file 路径一致。

## 5. 验证

每块完成即：v0.5 规范化 byte-equal 闸门（473）+ v0.9（en-003 转 pass、无新增 fail）+ fmt/clippy(`--all-targets`)/test。预计 v0.9 774→775（+1，en-003）；zh-038 仍 partial（sort 转正）。如实报告。

## 6. 实现单元（供 writing-plans）
1. 英文词数字→相对时间 + screenshot stop 增补 + 单测（TDD）。
2. media 路径 name sort 检测 + 单测（TDD）。
3. 回归门。
