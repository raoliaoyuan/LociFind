# 搜索召回修复：有关键词时 Everything 并列 fan-out

> 状态：设计已对齐，待写实现计划。
> 日期：2026-06-04（macOS）
> 承接：BETA-04 fan-out 多源搜索、BETA-15A/E 召回、Everything es.exe 路径修复（同会话）。

## 1. 背景与问题

Windows 真机反馈：三个搜索后端（`search.local` / `search.windows` / `search.everything`）
都可用（界面均绿），但内容查询召回不全——只命中 `search.local` 覆盖的部分目录，意图显示
`file_search via search.local`；同一查询单独用 Everything 结果完整得多。

**根因（代码坐实）**：路由把后端分两类，**Everything 被排除在内容 fan-out 之外、仅作零结果兜底**：

- `route_search_fanout`（`intent_router.rs:189-198`）内容分支只纳 **content-capable** 后端
  （local/windows），用 `backend_indexes_content` 过滤掉 Everything（纯文件名后端）。
- Everything 只在 `route_filename_fallback` 里，作为 `run_fanout_merge_with_fallback`
  （`fanout_merge.rs:116`：`if content.total > 0 { return }`）的**「content 干净零结果才触发」**兜底。

于是只要 `local`/`windows` 有任何**部分结果**（`content.total > 0`），Everything 的全盘文件名
召回就被整个跳过 → 召回不全。`via search.local` 是合并后的主源（local 有结果、windows 零结果）。

**关键约束（决定方案）**：`main.rs:263` 注释明确——**无内容关键词的查询**落到 Everything 才会
命中 match-all 垃圾结果；**有内容关键词时，Everything 用关键词做文件名匹配是合理的全盘召回，
不是噪声**。这正是「为什么当初把 Everything 设成兜底」的原因，也指明了安全的修复边界。

## 2. 目标与非目标

**目标**：内容查询（有关键词）时让 Everything **并列**参与 fan-out，恢复全盘召回，使 LociFind
的召回接近「单独用 Everything」。

**非目标**：
- **不让无关键词查询并列 Everything**（保留现状，避免 match-all 垃圾——`main.rs:263` 安全区）。
- 不动 `matches` / Ranker 排序逻辑（Ranker 已有 Filename 1.0 权重 + 多源加成，天然处理）。
- 不动 Everything 后端查询构造、不动 es.exe 路径解析（同会话已修）。
- 不移除现有零结果兜底（`route_filename_fallback`）——保留作边缘场景双保险。
- 不对 Everything 结果额外限流（依赖 Ranker 相关性排序 + 后端 limit + 总 limit）。

## 3. 核心设计（方案 A）

改 `route_search_fanout` 的内容分支：

```
当 expanded_needs_content(expanded) 为 true 时：
  selected = candidates 中 content-capable 者（local/windows，按 id 序）
  若 expanded.keyword_groups 非空（有内容关键词）：
    selected 追加 candidates 中非 content-capable 者（纯文件名后端 = Everything）
  若 selected 非空 → 返回 selected（并列 fan-out）
否则维持现状（首位单选）
```

### 决策 1：并列条件用 `keyword_groups 非空`，而非 `needs_content`

`needs_content = requires_content(base) || keyword_groups 非空`，范围更宽。并列 Everything
的条件**收紧为 `keyword_groups 非空`**——精确卡在「有内容关键词」安全区：有关键词 → Everything
文件名匹配合理、不 match-all。`needs_content` 为 true 但 `keyword_groups` 为空的情形（如某些
纯 base 内容需求）不并列 Everything（保守）。

### 决策 2：命中即并列，不限流（承接细节确认）

Everything 与 content 后端一起查、结果归一化合并去重（同文件多源合并保留富信息）。排序交给
现有 Ranker：`relevance = 0.5·文件名匹配 + 0.3·match_type 权重(Filename 1.0/…/Content 0.7)
+ 0.2·多源加成`——相关文件（文件名含关键词）靠前、多源命中更前、不相关的被总 limit 截断。
不额外限流。

### 决策 3：保留零结果兜底（承接细节确认）

`route_filename_fallback` + `run_fanout_merge_with_fallback` 的「content 零结果 → Everything」
兜底**保留不动**，作为「无关键词查询 / content 与并列 Everything 都零结果」的双保险。有关键词
查询此时 Everything 已在并列集里，兜底自然不重复触发（content.total 通常 > 0 或已含 Everything 结果）。

## 4. 跨平台

- **macOS**：无纯文件名后端（Everything 仅 Windows）→ 并列追加集为空 → `selected` = content only
  = 现状，**零行为变化**。Spotlight 是 content-capable，不受影响。
- **Windows**：Everything 可用时并列；不可用（未装 es.exe）时 candidates 不含它 → 并列集 = content
  only → 退回现状（不恶化）。

## 5. 实现位置

仅 `packages/harness/src/intent_router.rs` 的 `route_search_fanout` 内容分支。
复用现有 `backend_indexes_content`、`expanded_needs_content`、`available_search_tools_supporting`。
`search.rs` 的 fan-out 触发（`route_search_fanout` 返回 ≥2 → fan-out）与 `run_fanout_merge_with_fallback`
均不改——并列集 ≥2 时自然走 fan-out 合并。

## 6. 不变量与护栏

- **无关键词查询零行为变化**：并列只在 `keyword_groups 非空` 时发生 → 纯类型/扩展名查询不变、
  无 match-all 回归。
- **macOS byte-equal**：无 filename 后端 → 并列集 = content only → 现有 fan-out 测试不变。
- **现有路由/fan-out 测试**：`route_search_fanout` 既有用例（内容查询返回 content 后端、纯文件名
  返回单首位）需按新增「有关键词并列 filename 后端」更新断言（**设计内预期改动**）。
- `fmt` + `clippy --workspace -D warnings` 0；全 workspace 测试零回归（macOS 形态）。

## 7. 验证

- **harness 单测**（`intent_router.rs`，可在 macOS 跑——用 mock 后端注入 content + filename 能力）：
  (a) 有关键词内容查询 → 并列集含 content + filename 后端；(b) 无关键词查询 → 不含 filename 后端；
  (c) 无 filename 后端（macOS 形态）→ 并列集 = content only（不变）；(d) Everything 不可用 → 不并列。
- **fanout_merge 既有测试**：并列后 content 轮已含 Everything，确认零结果兜底不重复（行为合理）。
- 全 workspace 测试 + `fmt`/`clippy` 0；macOS 形态零回归。
- **真机验证（用户）**：装 es.exe 的 Windows 上，有关键词查询召回接近单独用 Everything；意图不再
  只 `via search.local`，sources 含 everything。

## 8. 已接受的限制

- 有关键词查询结果集变大（全盘文件名匹配），依赖 Ranker 把相关结果排前 + 总 limit 截断；极端宽
  关键词可能召回很多（但都是文件名含该词的合理结果）。
- 真机召回完整性需 Windows + Everything 验证（macOS 无法复现 Everything 路径），单测覆盖路由逻辑、
  真机覆盖端到端召回。
