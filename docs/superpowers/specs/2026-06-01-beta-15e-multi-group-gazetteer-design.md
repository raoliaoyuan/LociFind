# BETA-15E 后续：gazetteer 兼底多概念注入设计

> 状态：设计已对齐，待写实现计划。
> 日期：2026-06-01（macOS）
> 承接：[BETA-15E gazetteer 设计](./2026-05-30-beta-15e-gazetteer-design.md)、[BETA-15A 同义词召回评测](./2026-05-30-beta-15a-synonym-recall-eval-design.md)。

## 1. 背景与问题

BETA-15E 在 harness 层加了 gazetteer 兼底：当 parser **没有**抽出任何 keyword 时，
`YamlSynonymExpander::expand` 扫词典，把 query 里命中的内容词注入为一个 keyword 组，
让自然语言 query（如「找一份工作汇报相关的ppt」）也能触发同义词召回。

**当前缺口**：`gazetteer_lookup` 只挑**最长的单个**词典内容键注入一个组。一条 query
里若有**多个独立词典内容概念**（如「找合同和报告」），只有较长的一个被注入，另一个被
**静默丢弃**。parser 已抽出 keyword 的路径本就按每 keyword 一组多 group 处理，缺口
**仅在 gazetteer 兼底这条路径**。

## 2. 目标与非目标

**目标**：gazetteer 兼底路径支持一条 query 里的多个独立内容概念，不再丢弃。

**非目标**：
- 不动 parser 已抽 keyword 的路径（已是多 group）。
- 不动后端：产物是**单个多成员组**，SpotlightBackend / WindowsSearch 的 `search_expanded`
  早已支持单组 OR 跨字段（BETA-15E 已 ship），无需改。
- 不动词典源（`resources/synonyms/{zh,en}.yaml`）。
- 不碰 parser / spotlight 谓词 / evals parser fixtures。

## 3. 核心设计决策

### 决策 1：多概念合并成**一个 OR 组**（而非多个 AND 组）

匹配语义是「**组间 AND、组内 OR**」（见 `packages/evals/src/recall.rs`，忠实 BETA-15D 双查询）。

- **若注入多个独立组**（组间 AND）：query「合同和报告」→「合同」组 + 「报告」组 →
  文件须**同时**含合同**和**报告 → 单文件极少两者皆是，召回趋近 0。**错误**。
- **若合并成一个 OR 组**（组内 OR）：把两个概念及各自同义词全并入一个组 →
  「合同 OR 协议 OR 报告 OR 总结 OR …」→ 文件含**任一**即命中 = 结果集并集。

中文「合同**和**报告」语义是"这两类都给我看"= 并集 = OR。**选合并成一个 OR 组**。

> 注：parser 抽 keyword 的路径用 AND（多 keyword 收窄），那条**不动**。gazetteer 兼底
> 的「和」是"两类都要"，与 parser keyword 的收窄语义不同，分别处理是有意为之。

### 决策 2：非重叠贪心选词

替换 `gazetteer_lookup`（返回单键）为返回**多个非重叠内容键**：

1. 扫 zh+en 索引键，对每个**出现在 query 中**、过 `is_pure_content_term` 守护的键，
   记录 `(字符长度, 首现字节位置, 字节跨度)`。
2. 按「长度降序、首现位置升序」排序候选。
3. 贪心逐个取：若其在 query 中的字节跨度与**已选键的跨度不重叠**则保留，否则跳过。

天然处理子串包含——「工作汇报」选中后，子串「汇报」跨度重叠被跳过，不注入噪声概念。
结果是一组互不重叠的内容键，按 query 首现位置排序。

### 决策 3：head 取 query 首现概念

合并组的 `head` = query 中**首现**的概念（如「合同和报告」→ head=合同），其余键的
head + 全部 synonyms 去重并入 `synonyms`。head 只影响 display / 排序，OR 成员全部参与
匹配。整组交给已有的 `cap_keyword_groups`（`RUNTIME_KEYWORD_CAP=32`）兜底截断。

## 4. 实现位置

仅 `packages/harness/src/synonym/yaml.rs`：

- `gazetteer_lookup(&self, query) -> Option<String>` → 改 / 新增
  `gazetteer_lookup_multi(&self, query) -> Vec<String>`（非重叠贪心，按首现排序）。
- `expand` 兼底分支：`gazetteer_lookup_multi` 命中 ≥1 → 各 `expand_one` 后并入单组
  （首现键作 head，其余 head+synonyms 去重并入），交 `cap_keyword_groups`；
  零命中 → 退 `bare_content_keyword`（单词兜底，**不动**）。

## 5. 不变量（守 recall 回归门）

- **单概念 query 与现状 byte-equal**：只命中一个键时，合并退化为原单组单概念，
  召回基线（总 100% / zh 100% / en 100% / 假阳 0%）不变。
- **链顺序不变**：多概念 gazetteer → 命中≥1 用之；零命中再退 `bare_content_keyword`。
- **假阳门 ≤5%**：合并 OR 组命中域变宽（任一概念命中即返回）；新增 recall case 须带
  干扰文件，确保假阳不越界。

## 6. 验证

- **harness 单测**：(a) 多概念合并成单 OR 组、(b) 子串重叠跳过、(c) 单概念退化
  byte-equal、(d) 首现顺序定 head、(e) 零命中退 bare fallback 不变。
- **BETA-15A recall fixtures 新增多概念 case**：如「找合同和报告」分别命中语料里的
  合同文件与报告文件 + 至少一个干扰文件守假阳；跑 `synonym_recall` 门 +
  `cargo test --workspace`。
- **evals v0.5 parser-only 472/26/2 零回归**（不沾 parser）。
- `bash scripts/ci.sh` 全绿。

## 7. 已接受的限制

- 合并 OR 组的命中域比单概念宽，理论假阳上升；以 recall 假阳门（≤5%）+ 干扰 case 守护。
- gazetteer 仍只在 parser **完全无 keyword** 时触发（与 BETA-15E 一致）；parser 抽了
  部分 keyword 的混合 query 不走此路径。
