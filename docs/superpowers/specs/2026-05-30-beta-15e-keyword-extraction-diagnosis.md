# BETA-15E — 自然中文 query keyword 抽取 — Phase 1 诊断 + 方案建议

> 日期：2026-05-30
> 作者：Claude Code (Opus 4.8)
> 阶段：B / B6（BETA-15D 真机手测暴露的 BETA-15 / parser 层 gap）
> 类型：systematic-debugging Phase 1 诊断（无代码改动）+ 方案推荐，作为 BETA-15E brainstorming 的输入
> 状态：诊断完成，待 brainstorming → writing-plans → 实现

## 1. 问题

BETA-15D（macOS 26 谓词回归）修复后真机手测发现：**parser 对自然中文 query 不产出名词短语 keyword**，导致 BETA-15 同义词关键词扩展特性对自然 query 实际不触发，BETA-15D 复合谓词修复无法用自然语言验证。

实测（`cargo run -q -p locifind-cli -- --intent-only "<query>"`）：

| query | keywords | 说明 |
|---|---|---|
| `找一份工作汇报相关的ppt` | **None** | 头号 demo，只剩 ext ppt/pptx |
| `找一份工作汇报相关的幻灯片` | **None** | 同义词词典自带 demo |
| `工作汇报 ppt` / `找工作汇报的ppt` / `包含工作汇报的ppt` | **None** | |
| `述职`（裸） | **None**（ext 也 None → match-all） | |
| `找文件名包含工作汇报的ppt` | `["工作汇报"]` | 唯一能提词的生硬 phrasing |

## 2. 根本原因（Phase 1）

`packages/intent-parser/src/parsers/file_search.rs::extract_filesearch_keywords`（line 232）**对中文只认 3 个显式结构模式**，不做通用中文 keyword 抽取：

1. 引号包裹：`「X」` / `"X"`（`extract_bracketed_word`）
2. 后缀短语：`名字里有 / 名字里包含 / 名字是 / 文件名包含` + X（`extract_after_phrase`）
3. `找最近的 X Y` 显式结构（`extract_zui_jin_de_keyword`，要求字面"最近的" + 空白分隔 token）

英文路径有 `extract_english_token_keyword`，但中文路径止于上述 3 模式。代码注释（line 262-264）明确这是**刻意保守设计**：「不做通用中文 token 扫描，否则会把"查找昨天编辑过的"这种"动词+时间"短语误判为 keyword」。

`找一份工作汇报相关的ppt` 三模式都不匹配 → `keywords=None`。

## 3. 为何 parser 路线（方案 A）高风险

- 中文无词边界，通用名词短语抽取需分词/词典，"找一份工作汇报相关的ppt" 里 X 的左右边界（剥"找一份"、"相关的"）难做稳。
- evals v0.5 的 fixture 很可能对大量自然 query 期望 `keywords=None`；加抽取会翻这些 case → parser-only 472/26/2 回归。STATUS 第 17 阶段曾因放宽 keyword 抽取触发 **−28 当场回退**，最终窄化到显式模式才稳——直接印证此风险。

## 4. 推荐方案 B：harness 层词典 gazetteer（大幅降险）

我们已有一份**精选同义词词典**（`resources/synonyms/{zh,en}.yaml`，正是关心的名词短语集合）。让 `SynonymExpander` **扫描原始 query 文本**，凡命中词典 head/alias 子串，就把它注入为一个 keyword group —— 在「扩展 parser 已给的 keyword」之外**新增这条注入路径**。

数据流：`找一份工作汇报相关的ppt` → parser 给 `keywords=None, ext=ppt` → expander gazetteer 扫到 "工作汇报"（词典 head）→ 注入 group `{工作汇报, 述职, 年度总结, …}` → `search_expanded` → BETA-15D 的 Q1 多词 glob 复合谓词 → 真机命中 `述职.ppt`（已由 BETA-15D 诊断阶段实测确认该复合谓词被 mdfind 接受且精确命中）。

| 维度 | 方案 B |
|---|---|
| parser / evals 472/26/2 | **完全不动 → 零 parser-eval 回归风险** |
| 过度抽取 | 词典精选多字名词短语 → 低（增益偏召回） |
| 改动面 | `SynonymExpander::expand` 签名需加 `raw_query: &str`（实测当前签名 `expand(&self, intent) -> ExpandedSearchIntent`，**拿不到原始 query**）→ 改 trait + `IdentityExpander`/`YamlSynonymExpander` 两个 impl + desktop `search.rs` 调用处透传 query；YamlSynonymExpander 加 gazetteer 扫描（最长匹配优先 / 去重 / 与既有 keyword 合并）+ 测试 |
| 工作量 | 中等（~半天），远低于方案 A |
| 配套 | 建议同时落 **BETA-15A 同义词召回评测集**（30~50 case）守召回质量；本方案不碰既有 parser evals |

**关键洞察**：BETA-15 当初只「扩展 parser 给的 keyword」，却假设 parser 会给名词短语 keyword（parser 实际不给）。方案 B 把缺口补在「已拥有词典」的 harness 层，绕开 parser 回归雷区。

## 5. 待 brainstorming 决策点

1. gazetteer 匹配策略：最长匹配 / 全部命中？命中多个词典词时如何取舍（避免噪声）。
2. 注入的 keyword group 与 parser 已有 keyword（少数 phrasing 有）如何合并去重。
3. `expand` 签名改动 vs 新增 `expand_with_query`（向后兼容 IdentityExpander / 测试）。
4. 是否需要 query 预处理（剥离扩展名词、停用词）再扫，降低误命中。
5. 验收门：BETA-15A 召回集 + evals parser-only 472/26/2 byte-equal（方案 B 不动 parser，自然成立）+ 真机 `找一份工作汇报相关的ppt → 述职.ppt`。

## 6. 边界

- 不改 spotlight backend（BETA-15D 已 done 且正确）。
- 不在诊断阶段动 parser；若 brainstorming 选方案 A，需独立评估 evals 回归。
- 本文档为诊断 + 输入，正式设计走 brainstorming 产出 design doc。
