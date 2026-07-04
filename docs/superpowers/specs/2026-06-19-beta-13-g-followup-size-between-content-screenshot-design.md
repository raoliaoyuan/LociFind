# BETA-13-G 近邻 follow-up：size between + 内容截图多关键词（设计）

> 日期：2026-06-19 · 工具：Claude Code (Opus 4.8) · 阶段：B（Beta）
> 前序：[BETA-13-G8](2026-06-19-beta-13-g-parser-clean-gaps-design.md)（已合 main，v0.9 726→765）
> 来源：[标注冲突决策清单 §4](../../reviews/beta-13-g-annotation-conflicts.md) 登记的 parser 可修近邻缺口

## 1. 背景

BETA-13-G8 落地后 v0.9 parser-only = 765 pass / 199 partial / 36 fail。决策清单 §4 登记了 4 项「parser 真能修、不卡标注」的近邻缺口。本会话取其中 **byte-equal 安全、有 evals 增益**的两项（#1 size between、#2 内容截图多关键词）。

砍掉 #3（documents 被 location 误判，触 location parser、byte-equal 风险高）、#4（artist.rs 重构，0 evals 增益），YAGNI。

## 2. 目标与非目标

**目标**
1. `parse_size` 支持区间 size（`between A and B unit`）→ `SizeExpression::Between`。
2. 内容截图边缘变体（`同时出现`/`with both` + 多关键词拆分）路由至 file_search。

**非目标**
- 不碰 location parser（#3）、不做 artist.rs 重构（#4）。
- 不碰任何评测集 fixture。
- 不追求 90% 总体指标。

## 3. 约束
- **v0.5 byte-equal 铁律**：v0.5 = 473 pass，规范化逐 case 比对（`/tmp/v05check.py`，非裸 diff——reporter JSON 非确定，见 [memory]）必须零差异。
- Rust：`cargo fmt --check` + `cargo clippy --deny warnings` + `cargo test --workspace` 全绿。
- 改动限于 `packages/intent-parser/src/parsers/{file_search.rs, media_search.rs}`。

## 4. 设计

### 4.1 Fix #1 — size between（archives between-size）

实测：`archives between 10 and 100 MB` → 期望 `file_type=archive, size=Between{min:10,max:100,unit:MB}, keywords=null`（archives 已由 BETA-13-G8 从 keywords 剥除）。

`SizeExpression::Between{min,max,unit}` schema 已存在（[common/lib.rs:786](../../../packages/search-backends/common/src/lib.rs)）；`parse_size`（[file_search.rs:416](../../../packages/intent-parser/src/parsers/file_search.rs)）缺识别——当前 `between…and…` 不命中任何分支、返回 None。

**机制**：在 `parse_size` 返回 None 前加区间正则：
- 英文 `between\s*(\d+)\s*(?:and|-|~)\s*(\d+)\s*(unit)` → `Between{min, max, unit}`（单位取末位，两数共用）。
- 中文 `(\d+)\s*(?:到|至|-|~)\s*(\d+)\s*(unit)` → 同。
- min/max 顺序规范化（min ≤ max）。

**byte-equal 安全**：`between/到/至` size 形态当前恒返回 None；新增只影响原本 size=None 的查询。实现前 grep v0.5 确认无该形态需保持 None 的 case。

### 4.2 Fix #2 — 内容截图边缘变体（多关键词）

实测：
- `截图里同时出现订单号和金额的` → `file_type=screenshot, keywords=["订单号","金额"]`
- `screenshot with both order id and tracking number` → `file_type=screenshot, keywords=["order id","tracking number"]`

BETA-13-G8 的 `detect_content_clause` 引导词含 `里写着/写着/提到…`，但不含 `同时出现/with both`，故这两条仍误路由 media。且它们要求按 `和`/`and` **拆多关键词**。

**机制**：
1. **扩引导词**：`detect_content_clause`（media_search.rs）的中文正则加 `同时出现/同时包含`，英文加 `with both`，使截图 + 这些引导词命中重路由闸门（与 BETA-13-G8 截图路由同机制：`has_screenshot_word && detect_content_clause.is_some()` → file_search）。
2. **多关键词拆分（仅 both/同时 分支）**：定义「both 语义」标记集（`同时出现/同时包含/with both`）。当内容子句由 both 标记引导时，把捕获内容按 `和`/`\band\b` 拆成多个关键词；否则保持单关键词。
   - 落点：file_search 的内容子句关键词注入处（BETA-13-G8 已有该短路）。增加：若引导标记属 both 集 → split 内容为 `Vec`；否则维持单元素。
3. file_type=screenshot 由现有截图类型映射给出（不变）。

**关键边界（用户已确认）**：**仅 both/同时 标记才拆**。常规内容子句（`里面有提到甲方乙方的协议`→`["甲方乙方"]`，含字面但无「和」无 both 标记）保持单关键词——避免对所有含 `和` 的内容词乱拆破坏 BETA-13-G8 已通过的 case。

**byte-equal 安全**：`同时出现/with both` 在 v0.5 不出现（grep 确认）；闸门对 v0.5 惰性。

## 5. 验证策略

每个 fix 完成即：
1. `cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/v05-now.json && python3 /tmp/v05check.py` → **V0.5 BYTE-EQUAL OK**（473，0 差异）。
2. v0.9 → coverage fail 下降（预计 archives 1 + 截图 2 = 3）、无新增 fail。
3. `cargo fmt --check` + `cargo clippy --deny warnings` + `cargo test --workspace`。

预计 v0.9 765 → ~768。如实报告，不凑指标。

## 6. 风险

- **多关键词拆分误伤**：若 both 标记判定过宽，可能把单关键词内容子句拆碎。缓解=both 标记集严格限于 `同时出现/同时包含/with both`，对 BETA-13-G8 内容截图全集 + 含「和」的内容 case 逐条 byte-equal/单测核对。
- **size between 正则吞错数**：`between 10 and 100 MB` 两数 + 末位单位。缓解=单测覆盖、v0.5 byte-equal 闸门。

## 7. 实现单元（供 writing-plans 拆 task）
1. `parse_size` 加区间识别 + 单测（TDD）。
2. `detect_content_clause` 扩 both 引导词 + 多关键词拆分（仅 both 分支）+ 单测（TDD）。
3. 回归门（v0.5 byte-equal + v0.9 + fmt/clippy/test）。
