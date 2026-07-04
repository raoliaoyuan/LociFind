# BETA-15E — 同义词词典 gazetteer 注入 keyword — 设计

> 日期：2026-05-30
> 作者：Claude Code (Opus 4.8)
> 阶段：B / B6（承接 BETA-15D 真机手测暴露的 parser keyword gap）
> 类型：harness SynonymExpander 增强（方案 B：query gazetteer 注入 keyword group），**不动 parser、不动 spotlight backend**
> 输入：Phase 1 诊断 [2026-05-30-beta-15e-keyword-extraction-diagnosis.md](./2026-05-30-beta-15e-keyword-extraction-diagnosis.md)

## 1. 背景与目标

parser 对自然中文 query 不产出名词短语 keyword（`找一份工作汇报相关的ppt` → `keywords=None`），导致 BETA-15 同义词扩展不触发、BETA-15D 复合谓词修复无法用自然语言验证（详诊断文档）。

**目标**：自然中文 query 含同义词词典中的内容名词短语时（如"工作汇报"），由 `SynonymExpander` 扫描原始 query 命中该词并注入为 keyword group → 触发同义词扩展 → BETA-15D 双查询 → 真机命中 `述职.ppt`。

**核心约束**：**不改 parser、不改 spotlight backend** → evals parser-only **472/26/2 byte-equal** 自然成立。

## 2. 核心架构决策

### 决策 1：兼底触发（仅 parser 无 keyword 时）
`expand` 仅在 `intent.search_keywords()` 为 None/空时启用 gazetteer 扫描；parser 已给 keyword 则走原有 `expand_one` 扩展路径，gazetteer 不介入。最小干预、与现有行为零冲突。

### 决策 2：gazetteer 扫描 + 最长匹配单 group
- 扫描源：原始 query 文本，按 `intent.language` 选索引（zh→zh_index、en→en_index、mixed→两者）。
- 候选 = 索引 key（head + alias，`build_index` 已含双向）中**作为 query 子串出现**者。
- 多命中 → **取最长候选，只注入 1 个 group**（最具体；兼底语义下 query 通常只含一个核心名词短语；避免多词 AND 过度收窄）。长度并列时取 **query 中首现位置最靠前**者（确定性，便于 byte-equal 测试）。
- 命中后用现有 `expand_one(matched)` 产出完整同义词 group，作为唯一 keyword group 注入 `ExpandedSearchIntent`。

### 决策 3：重叠保护 = 重解析候选词（以 parser 为单一信源）
词典桶 2（文件类型：幻灯片/表格/文档…）、桶 3（媒体：截图/照片/视频…）含的是 parser 会消费为 `file_type`/`media_type`/`extensions` 的类型词；若把它们注入为文件名 keyword 会错误收窄。

**守护规则**：对每个 gazetteer 候选词 T，调用 `locifind_intent_parser::parse(T)`，**若结果 `file_type.is_some()` 或 `media_type.is_some()` 或 `extensions.is_some()` 或 variant 非 FileSearch → 跳过 T**（它是类型/媒体词，parser 已在主 query 中消费）；否则 T 是纯内容词，可注入。

实测验证（`--intent-only`）：

| 候选词 | parse 分类 | gazetteer |
|---|---|---|
| 工作汇报 / 述职 / 报告 / 合同 / 发票 / 笔记 | file_type=None media_type=None ext=None | **注入** ✓ |
| 幻灯片 | file_type=presentation, ext=[ppt,pptx] | 跳过 |
| 截图 | variant=media_search, media_type=screenshot | 跳过 |
| 视频 / 文档 | file_type=video / document | 跳过 |

优点：零词典改动、自动正确（含 报告/合同 等边界词）、以 parser 为类型判定单一信源、随 parser/词典演进自洽。代价：harness 新增 `intent-parser` 依赖（已确认 harness↔intent-parser 互不依赖，无环）。候选通常 0–2 个 → 0–2 次 parse 调用，性能可忽略。

### 决策 4：API —— expand 加 query 参数
`SynonymExpander::expand(&self, intent: SearchIntent) -> ExpandedSearchIntent` 改为 `expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent`。
- `IdentityExpander`：忽略 query（保持 identity 语义）。
- `YamlSynonymExpander`：用 query 做 gazetteer（仅兼底分支）。
- 调用处 `apps/desktop/src-tauri/src/search.rs:251`：透传 search 命令已持有的 query 字符串。
- 其余 harness 内部不变；`expand_one` / `cap_keyword_groups` / 索引结构复用。

## 3. 组件改动清单

| 文件 | 改动 |
|---|---|
| `packages/harness/Cargo.toml` | 加 `locifind-intent-parser` 依赖 |
| `packages/harness/src/synonym/expander.rs` | `SynonymExpander::expand` 签名加 `query: &str`；`IdentityExpander` 忽略 |
| `packages/harness/src/synonym/yaml.rs` | `YamlSynonymExpander::expand` 加兼底 gazetteer：扫描 + 重解析守护 + 最长匹配 + `expand_one` 注入单 group；新增 `fn gazetteer_lookup(&self, query, lang) -> Option<String>` |
| `apps/desktop/src-tauri/src/search.rs` | `expand(...)` 调用透传 query |
| `expand` 的所有其它调用方（harness 单测、desktop 测试、evals 若有） | 同步改签名传 query（多数测试传原 query 或空串）|
| `resources/synonyms/*.yaml` | **不改** |
| parser / spotlight / evals 源 | **不改** |

## 4. 数据流（`找一份工作汇报相关的ppt`）

```
parse → FileSearch{ keywords:None, extensions:[ppt,pptx], file_type:presentation }
        │ (keywords 为空 → 触发 gazetteer)
expand(intent, "找一份工作汇报相关的ppt"):
  zh_index 子串命中候选: ["工作汇报", "总结"(若 query 含)…] → 取最长 "工作汇报"
  重解析守护: parse("工作汇报") → 无 file_type/media/ext → 内容词，保留
  expand_one("工作汇报") → group{head:工作汇报, synonyms:[述职,年度总结,季度汇报,月度汇报]}
  → ExpandedSearchIntent{ base, keyword_groups:[该组] }
        │
search_expanded → BETA-15D Q1: (FSName/DisplayName glob 工作汇报|述职|年度总结|…) && (ext ppt|pptx)
        │  + Q2: TextContent CONTAINS 各词
        → 命中 述职.ppt
```

## 5. 错误处理 / 边界

- gazetteer 无命中（或命中全被守护跳过）→ 返回 identity（与今天行为一致，纯扩展名/match-all 搜索不变）。
- query 为空 / parser 已给 keyword → 不扫描。
- 注入仍受既有 `cap_keyword_groups(RUNTIME_KEYWORD_CAP)` 约束（单 group，远低于 cap）。
- ContextMemory / Refine / FileAction 路径不受影响（expand 仅作用于 FileSearch/MediaSearch 的 keyword 缺口）。

## 6. 测试

harness 单测（`yaml.rs` + `expander.rs`）：
- gazetteer 命中内容词注入正确 group（`找一份工作汇报相关的ppt` → group head=工作汇报，synonyms 含述职）。
- 重解析守护：`找幻灯片`/`找截图`/`找视频`/`找文档` 不注入（类型/媒体词跳过）→ 返回 identity。
- 兼底触发：parser 已给 keyword 时 gazetteer 不介入（走原 expand_one）。
- 最长匹配：query 同时含短词与长词（如含"总结"与"工作汇报"重叠场景）取最长。
- 语言选择：en query 扫 en_index；mixed 扫两者。
- `IdentityExpander::expand(_, query)` 忽略 query 仍返回 identity。
- 注入 group 与 `expand_one(matched)` byte-equal。

## 7. 验证门

- `bash scripts/ci.sh` 全过（harness 测试 +N；desktop 编译适配新签名）。
- **evals v0.5 parser-only 472/26/2 byte-equal**（不动 parser；实跑确认）。
- evals hybrid 不回归（不沾 fallback 模块）。
- 真机（agent 可经 desktop 集成测试或后续 UI 手测验证）：`找一份工作汇报相关的ppt` 经 expand 产出 head=工作汇报 的 group；扩展 Q1 谓词命中 述职.ppt（谓词已由 BETA-15D 阶段实测确认）。

## 8. 非目标

- BETA-15A 同义词召回定量评测集（独立 backlog，建议随后跟进守召回质量，但不阻塞本次）。
- parser keyword 抽取改造（方案 A，本设计刻意规避）。
- en 词典召回调优 / 词典扩容（> 200 组触发 BETA-15B）。
- gazetteer 多 group 注入（本设计单 group；多概念 query 留后续按需）。
