# BETA-13 同义词召回回归修复：gazetteer 升为召回核心内容词提取器

> 状态：设计已对齐，待写实现计划。
> 日期：2026-06-04（macOS）
> 承接：[BETA-15A 召回评测](./2026-05-30-beta-15a-synonym-recall-eval-design.md)、[BETA-15E gazetteer 设计](./2026-05-30-beta-15e-gazetteer-design.md)、[BETA-15E 多概念注入](./2026-06-01-beta-15e-multi-group-gazetteer-design.md)。

## 1. 背景与问题

BETA-13-G1（commit `e59e5ab`）增强了 parser 关键词抽取，把 v0.9 字段精确匹配从
514 抬到 726。但**落库时漏跑了 `synonym_recall_meets_gate`**（macOS 上），该门此后
一直红着：**recall 从 ≥70% 崩到 5.9%**（门槛 70%）。已二分确认回归由 `e59e5ab` 引入
（其父提交召回门 PASS）。

`run_recall`（`packages/evals/src/recall.rs`）管线是 `parse → expand → matches`，
`matches` 忠实 BETA-15D 双查询语义：**组内 OR、组间 AND**（即真实后端语义，非仅评测）。
崩塌有**两种叠加模式**（实测三个 case 坐实）：

| Query | parser keywords | 崩塌原因 |
|---|---|---|
| 找我们之前签的合同，跟乙方那份 | `["合同","乙方"]` | 「合同」同义词扩展正常（→协议/合约），但多抽的修饰语「乙方」成独立组，**组间 AND** 要求文件名也含「乙方」→ 碾压 |
| 我之前投递工作时做的简历在哪 | `["投递工作时","简历在哪"]` | **分词不净**：「简历」粘成「简历在哪」，既不在词典（无同义词扩展）又非文件名子串 → 漏 |
| find the contract we signed with the vendor last quarter | `["contract we signed","vendor","quarter"]` | 两者叠加：分词不净 + 多抽 vendor/quarter 三组 AND 碾压 |

**根因**：BETA-13 G1 让 parser 激进抽取自然语言 query 的多个 keyword，副作用是
①把修饰语/次要概念抽成独立组、被组间 AND 当硬约束碾压；②分词边界不净、keyword 带噪声，
既不触发同义词扩展又不匹配语料。而 BETA-13 前这些 query parser 不抽 keyword、走 gazetteer
兼底扫出**干净的核心内容词 + 同义词扩展**，召回正常。

**核心矛盾**：v0.9「字段精确匹配」要 parser 多抽、抽准；BETA-15A「召回」要干净核心词 +
同义词扩展 + 不被 AND 碾压。BETA-13 优化了前者、砸了后者。

## 2. 目标与非目标

**目标**：在**不回退 v0.9（726）**的硬约束下，把 recall 门从 5.9% 恢复到 **≥70%**、
假阳 ≤5%。

**非目标**：
- **不动 parser 关键词抽取**（保住 v0.9 字段精确匹配；v0.5/v0.9 是 parser-only evals、
  不经 expand，故召回侧任何改动天然 byte-equal 不影响二者）。
- 不动 `matches` 的组内 OR / 组间 AND 语义（忠实 BETA-15D 真实后端，改它影响假阳与后端
  查询构造契约）。
- 不动词典源（`resources/synonyms/{zh,en}.yaml`）、不动 recall fixtures（语料 + 标注），
  否则是掩盖而非修复。
- 不动后端 `search_expanded`（已支持单组 OR 跨字段）。

## 3. 核心设计

### 决策 1：gazetteer 从「兼底」升为「召回核心内容词提取器」

把 `YamlSynonymExpander::expand` 里 gazetteer 的触发条件，从「仅 parser 无 keyword 的
兜底分支」**扩展为「召回时总参与」**：

1. expand 拿到 intent（无论 parser 是否抽了 keyword）。
2. 对 **query 原文**扫 gazetteer 多概念（`gazetteer_lookup_multi`），提取所有在 ship
   词典里、过 `is_pure_content_term` 守护（排除类型/媒体词）的核心内容词。
3. 命中 ≥1 → 各 `expand_one` 后**合并成单个 OR 组**（`merge_or_group`，组内 OR）→
   **替代** parser 的 keyword 组。
4. gazetteer 零命中（核心词不在词典）→ **保留 parser keyword 组**（现状，不变）；
   parser 也无 keyword 时 → 退原 `bare_content_keyword` 兜底（不变）。

### 决策 2：命中即替代（而非叠加 / 启发式）

gazetteer 命中即用合并 OR 组**替代** parser keyword 组。

- **为何不叠加**：叠加会让组间 AND 更严（parser 脏组仍在）→ 更崩。
- **为何不启发式**（仅当 parser「不理想」才替代）：阈值难定、复杂；且 parser keyword
  本就干净时（query「找合同」→`["合同"]`），gazetteer 也扫到「合同」、替代后等价（无害）。
- **代价（有意权衡）**：`合同+乙方`→替代成「合同」OR 组、丢「乙方」约束 → 召回所有合同。
  但 recall 标注期望就是召回合同类文件（`f-xieyi-pdf`=甲乙合同），且取向已定召回优先。

### 决策 3：多核心词合并单 OR 组（承接 BETA-15E）

query 含多个独立核心内容词（如「找合同和协议」）时，`gazetteer_lookup_multi` 返回多个
非重叠键，`merge_or_group` 合并成**一个 OR 组**（组内 OR = 并集），避免多组 AND 碾压。
匹配语义「组间 AND、组内 OR」决定：多核心词必须合并成一个组才能 OR 召回。

## 4. 与 BETA-15E 的关系

本修复**吸收并扩展 BETA-15E**——BETA-15E 原是「parser 无 keyword 时 gazetteer 兼底
多概念注入」的独立特性，本设计把它的组件提升为**召回路径的核心机制**：

- `gazetteer_lookup_multi`：**已在 `feat-beta-15e` 分支实现**（commit `6b47d2d`，非重叠
  贪心选词），引入 main 复用。
- `merge_or_group`：BETA-15E Task 2 **未实现**（计划文档有代码），本修复新实现。
- `strip_leading_search_verbs`：harness 已存在（剥前导搜索动词），`gazetteer_lookup_multi`
  内部已用。
- **expand 的「有 keyword 时也扫 gazetteer + 命中即替代」逻辑：本修复新增**（BETA-15E 计划
  只改了 `_` 兜底分支，未触及「有 keyword」路径）。

故 BETA-15E 的挂起 Task 1 commit 在此被复用、复活；BETA-15E 不再作为独立特性单列。

## 5. 实现位置

仅 `packages/harness/src/synonym/yaml.rs`（纯召回侧）：

- 引入 `gazetteer_lookup_multi`（自 `feat-beta-15e`）+ 新增 `merge_or_group`。
- 改 `expand`：进入即扫 `gazetteer_lookup_multi`，命中 ≥1 → `merge_or_group` 替代；
  零命中 → 走 parser keyword（或原裸词兜底）。

## 6. 不变量与护栏

- **v0.5/v0.9 byte-equal**：parser-only、不经 expand → 天然不变（实现后再跑确认 473 / 726）。
- **recall 门恢复**：recall ≥ 70%、fp ≤ 5%。
- **harness 现有 expand 单测**：假定「parser 有 keyword → 直接用 parser 组」的断言，按新
  行为更新（**设计内预期改动，非回归**）；其余单测保持。
- **链顺序**：gazetteer 命中 ≥1 用之 → 零命中保留 parser keyword → parser 也无则裸词兜底。
- `fmt` + `clippy --workspace -D warnings` 0；无新依赖。

## 7. 验证

- **harness 单测**：(a)`合同+乙方`类经 expand 得单 OR 组（含协议/合约、不含乙方组）；
  (b)`简历在哪`类脏 keyword 被 gazetteer 干净词「简历」替代；(c) 词典未命中核心词时
  保留 parser keyword（不恶化）；(d) 多核心词合并单 OR 组；(e) 复用/更新
  `gazetteer_lookup_multi` 与 `merge_or_group` 单测。
- **recall 门**：`cargo test -p locifind-evals synonym_recall_meets_gate` 由 5.9% → ≥70%；
  `cargo run -p locifind-evals --bin synonym_recall` 报告分桶召回 + fp ≤5%。
- **parser-only 零回归**：`evals --fixtures v0.5` = 473/25/2、`--fixtures v0.9` = 726/225/49。
- `bash scripts/ci.sh` 全绿。

## 8. 已接受的限制

- **精确性降**：替代丢弃 parser 的修饰语约束（乙方/vendor/quarter）→ 召回域变宽。以
  召回优先取向 + recall 假阳门（≤5%）守护。
- **词典覆盖依赖**：query 核心词不在 ship 词典 → gazetteer 零命中、回退 parser keyword
  （不恶化，但该 case 召回不改善）。recall fixtures 核心词在 BETA-13 前可召回，说明覆盖到位。
- **真实搜索行为变化**：expand 是产品召回路径，自然语言 query 召回变好；不引入新假阳由
  recall fp 门 + 真机手测守。
