# BETA-13 — evals 扩到 1000 条（覆盖驱动 v0.9）— 设计

> 日期：2026-06-03
> 作者：Claude Code (Opus 4.8)
> 阶段：B / B5（Beta 出场前置；§6 出场指标「总体 evals 通过率 > 90%」依赖本评测集）
> 类型：**新建 v0.9 评测集（1000 条）**，v0.5 原样保留；可能伴随少量低风险 parser 修复
> 依赖：BETA-04（多源融合）/ BETA-05（Ranker）已 done；引用 [search-intent-schema.md](../../search-intent-schema.md) 为标注权威信源

## 1. 背景与目标

ROADMAP §6 出场指标把「**总体 evals 通过率 > 90%（BETA-13 v0.9 evals，1000 条）**」列为 Beta 出场硬指标。当前评测集只有 **v0.5 = 500 条**（[packages/evals/fixtures/v0.5/cases.json](../../../packages/evals/fixtures/v0.5/cases.json)），parser-only baseline `472 pass / 26 partial / 2 fail`（94.4% pass）。

但 v0.5 是**程序化生成**的（[fixtures.rs](../../../packages/evals/src/bin/fixtures.rs) 模板 + `target_for` 配额）——其 `expected_intent` 由 `file_intent(...)` 等 helper **与 query 用同一套 spec 数据配对生成、全程不调 parser**。这套机制保证了 consistency 与规模，但有一个本质局限（见 §3）：**生成器的"标准答案"本质是"作者心里、且被调到让 parser 能过的输出"，因此几乎不暴露 parser 的真实缺口**。

**目标**：建一套 **v0.9 = 1000 条** 的评测集，其中新增 500 条为**覆盖驱动、独立标注的 ground-truth**，系统性覆盖 Beta 阶段落地的新能力（同义词扩展、跨范畴多类型、内容检索、音乐 metadata 等）与已知薄弱区，**真实暴露 parser 在这些能力上的解析缺口**，并据此：

1. 给出 v0.9 的 parser-only baseline（pass/partial/fail），对照 §6 的 >90% 目标。
2. 暴露的缺口分两路处置：低风险、评测守门下可修的**顺手修**；设计性 / 大改动的**登记 ROADMAP 另开 task**。
3. 作为后续 parser / 词典 / 模型升级的回归锚点（与 v0.5 并存）。

## 2. 范围（已与用户对齐）

三处范围决策经 brainstorming AskUserQuestion 确认：

| 维度 | 决策 |
|---|---|
| 扩充重点 | **覆盖驱动**——重点打 Beta 新能力与已知缺口，而非按比例均匀放大 |
| 标准答案机制 | **Opus 撰写 + 独立标注**（依 schema 语义 + 功能设计意图，独立于 parser 当前行为）+ 对抗式标签校验 pass |
| 集合形态 | **超集**：v0.9 = v0.5 的 500 条（逐字保留）+ 500 条新覆盖 case |
| Gap 处置 | **低风险顺手修**（评测守门）+ **设计性登记**（ROADMAP 另开 task） |
| 流程 | 完整 superpowers 流程（本 spec → plan → subagent 驱动实现 + 双审） |

### 不做（YAGNI 闸口）

- **不跑真 Spotlight / mdfind / 模型端到端**：v0.9 与 v0.5 一致，仍是 **parser-only 确定性评测**。双平台真机 evals 属 Class A（需 macOS 真机 + 完整 Spotlight 索引），用本数据集但不在本 task 内跑。
- **不对 coverage case 做模板化生成**：coverage 是手工标注的 ground-truth，模板化会重新落回"答案跟着 parser 走"的陷阱（那是被否决的"按比例扩容"路线）。
- **不重排 / 不改 v0.5**：v0.5 的 472/26/2 byte-equal 锚点必须岿然不动。
- **不扩词典**：手维护词典 > 200 组才触发 BETA-15B；本 task 只衡量、不扩词。
- **不做大改动 parser 重构**：暴露的设计性缺口登记另开 task，本 task 只做低风险修复。

## 3. 核心洞察：为什么覆盖驱动必须独立标注

v0.5 生成器的数据流（简化）：

```
spec 数据(kind/time/loc/size/kw) ──┬──▶ query  字符串拼接
                                   └──▶ expected_intent  file_intent(...) 拼装
                                        （不经过 parser）
```

`expected_intent` 与 `query` 同源，且历史上被调校到"让 parser 能过"（472/26/2 = 94% pass，26 partial + 2 fail 是已知残差）。**用同样机制再生成 500 条，仍只会大多 pass——无法暴露新能力上的缺口**，与"覆盖驱动"初衷相悖。

覆盖驱动要求 ground-truth **独立于 parser 当前行为**：

```
query ──(Opus 按 schema 语义 + 功能设计意图标注)──▶ expected_intent（应然）
parser(query) ──▶ actual_intent（实然）
            比对 actual vs expected：
              一致 → Pass
              variant 一致、字段差 → Partial（字段级缺口）
              variant 不一致 → Fail（路由缺口）
```

这里 expected_intent 是「这条 query **应该**解析成什么」（依据 [schema doc §3-§7](../../search-intent-schema.md) 与各 BETA task 的设计意图），而非「parser 现在解析成什么」。**失败的 case = 真实 gap**，正是 BETA-13 的核心产出。

### 标注的"应然 vs 约定"两轴纪律（关键）

并非所有字段都该按"应然语义"标——否则会产生大量与缺口无关的噪声 partial（如 sort 默认值差异）。标注分两轴：

- **语义轴（query 显式表达的字段）**：按意图标，**这是要暴露 gap 的地方**。
  例：`上个月的发票` → `modified_time = {relative, last_month}` + `keywords=["发票"]`；`音乐和视频` → `file_search` + `file_type=["audio","video"]`（BETA-18/19 跨范畴路由意图）。
- **约定轴（query 未显式表达、但有惯例默认的字段）**：**follow v0.5/parser 既有约定**，避免无意义 partial。
  例：file_search 未提排序 → `sort = "modified_desc"`（v0.5 既有约定）；含 size 约束且语义指向大文件 → `sort = "size_desc"`；`language` 回显 case 语言；`schema_version` 恒 `"1.0"`。
  约定清单见 §6「标注约定表」，由 v0.5 既有 pattern + schema doc 提炼，标注者据此查表而非读 parser 代码。

> 一句话：**语义轴暴露缺口，约定轴消除噪声**。两者都不读 parser 源码——语义轴读 schema 意图，约定轴查约定表。

## 4. 架构与产物形态

### 4.1 文件布局

```
packages/evals/fixtures/
  v0.5/cases.json            # 500 条，原样不动（byte-equal 锚点）
  v0.9/
    coverage-cases.json      # ★ 500 条手工标注 ground-truth（本 task 核心产物，可读可审）
    cases.json               # 1000 条 = v0.5 500（逐字）+ coverage 500，merge 生成、提交入库
    README.md                # v0.9 说明 + baseline 报告 + gap 清单
```

- **`coverage-cases.json`** 是真正的人工策展产物（Opus 标注 + 校验），单独成文件便于 review / diff / 维护。
- **`cases.json`** 是合并构建物，提交入库以便 `load_cases("v0.9")` / `--fixtures v0.9` 直接可跑，且 CI 可校验其 = v0.5 + coverage 的确定性合并。

### 4.2 合并器（生成器扩展）

[fixtures.rs](../../../packages/evals/src/bin/fixtures.rs) 新增子命令 `generate-evals-v09`：

1. 读 `v0.5/cases.json`（500 条，**逐字保留 id 与字段顺序**）。
2. 读 `v0.9/coverage-cases.json`（500 条，id 形如 `v09-<domain>-<lang>-NNN`，全集内唯一）。
3. 确定性拼接（v0.5 在前、coverage 在后，各自内部保持文件序）→ 写 `v0.9/cases.json`（1000 条）。
4. 断言：总数 == 1000；id 全局唯一；coverage 段每条 `expected_intent` 能反序列化为合法 `SearchIntent`。

合并器**纯确定性**（无时间戳 / 随机 / 排序抖动），保证可复现 + 可进回归门。

### 4.3 case JSON 形态（标注契约）

每条 coverage case：

```json
{
  "id": "v09-synonym-zh-001",
  "query": "上个月改过的发票",
  "language": "zh",
  "expected_intent": {
    "intent": "file_search",
    "schema_version": "1.0",
    "language": "zh",
    "keywords": ["发票"],
    "modified_time": { "type": "relative", "value": "last_month" },
    "sort": "modified_desc"
  }
}
```

**`expected_intent` 必须是序列化形态**（与 `compare_json` 比对的 actual 一致）：

- 顶层含 `intent` / `schema_version: "1.0"`；`language` 回显 case 语言。
- `file_type` 单值写**标量字符串**（`"presentation"`），多值写**数组**（`["audio","video"]`）——见 [common/src/lib.rs `file_type_set`](../../../packages/search-backends/common/src/lib.rs)。
- 只写 query 真正表达 + 约定默认的字段；**不硬塞**与 query 无关字段（沿用 v0.5「占位符存在性决定字段」纪律）。

## 5. 覆盖矩阵（500 条）

按 Beta 能力与已知薄弱区分配；每域内再按 zh / en / mixed 三语切分（大体沿用 v0.5 的 zh 重、en 中、mixed 轻比例）。条数为目标，标注期 ±5 可调，总数锁 500。

| # | 覆盖域 | 对应能力 / 缺口 | 条数 | variant 倾向 |
|---|---|---|---|---|
| D1 | 同义词扩展 + 自然中文/英文关键词抽取 | BETA-15 / 15A / 15E（自然 query keyword 抽取曾是真实 gap） | 90 | file_search / media_search |
| D2 | 跨范畴多类型 + 强媒体词路由 | BETA-18 / 19（`pdf和doc`、`音乐和视频`、`截图和视频`） | 70 | file_search（多 file_type）|
| D3 | 内容检索语义（Office/PDF/OCR 命中正文/图片文字） | BETA-02 / 03 | 70 | file_search（keywords 落内容词）|
| D4 | 音乐 artist / title / album / genre / 时长 | BETA-01 / 01A | 60 | media_search（audio）|
| D5 | 时间 / 尺寸 / 位置 / 排序 自然表达变体 | 核心 schema §4 | 80 | file_search / media_search |
| D6 | file_action 确认流 + 多目标 + 写操作澄清 | MVP-19 / file-action 族 | 50 | file_action / clarify(UnsafeAction)|
| D7 | refine 上下文记忆（delta / clear） | refine 族 | 40 | refine |
| D8 | clarify 歧义（时间/位置/类型/操作模糊） | clarify 族 | 30 | clarify |
| D9 | 已修 bug 回归锚点 | `pdf和doc`(BETA-18) / dual-route / 强媒体跨范畴 | 10 | 混合 |
| | **合计** | | **500** | |

> 矩阵刻意**重 file_search / media_search / 跨范畴 / 内容检索**（Beta 的主战场与新能力），轻 refine/clarify（已较稳）。D9 把历史真机 bug 钉成永久回归锚点。

## 6. 标注约定表（约定轴查表依据）

提炼自 v0.5 既有 pattern + [schema doc §3-§4](../../search-intent-schema.md)，标注者据此填**未被 query 显式表达**的字段，消除噪声 partial：

| 字段 | 约定 |
|---|---|
| `schema_version` | 恒 `"1.0"` |
| `language` | 回显 case 语言（zh/en/mixed）|
| `sort`（file_search 默认）| `"modified_desc"` |
| `sort`（含 size 约束且语义指向大/小）| 大→`"size_desc"`、小→`"size_asc"` |
| `sort`（"最新/最近"显式）| `"modified_desc"`；"最早"→`"modified_asc"` |
| `file_type` / `extensions` | query 指明具体类型才填；纯"文件"不塞 file_type |
| Screenshot + 时间词 | 时间归 `created_time`（截图创建时间语义，沿用 v0.5）|
| `requires_confirmation`（file_action）| 写操作（rename/move/copy/delete）恒 `true` |
| `base_ref`（refine）| 恒 `"last_intent"` |
| Clarify `question` / `options` | 评测**忽略文本**（[lib.rs](../../../packages/evals/src/lib.rs) `is_clarify_question_equal` 恒真、options 只校验结构）；标注给占位即可，重点是 `reason` |

> 约定表是 spec 的活附录；标注期若发现 v0.5 还有未收录的稳定约定，补入此表并在 plan 记录。

## 7. 标注流程（subagent 驱动 + 对抗校验）

1. **撰写**：按覆盖域派 Opus 子 agent 并行撰写 + 标注（每 agent 吃透对应 BETA task 设计意图 + schema 相关小节 + 约定表）。产出该域的 case JSON 数组。
2. **对抗式标签校验**：第二批子 agent 对每条 case **独立复核** label——① JSON 合法且能反序列化为 `SearchIntent`（schema 合法性）；② variant 正确；③ 语义轴字段符合 query 意图、无过/欠标；④ 约定轴字段符合约定表。**校验者默认质疑**，对存疑 case 标记 `disputed` 并给理由。
3. **裁决**：`disputed` case 由我（必要时升级给用户）裁决——改标 / 保留 / 丢弃补新。**目标是 label 可信，而非凑满 500**：宁可丢弃可疑 case 也不污染 baseline。
4. **机械门**：合并后跑 `coverage_cases_are_schema_valid` 测试（每条 expected_intent serde 反序列化通过）。
5. **抽样人审**：从 500 条随机抽一批交用户 spot-check（标注质量背书）。

## 8. baseline 测量与 gap 处置

1. 合并出 1000 条后，`cargo run -p locifind-evals --bin evals -- --fixtures v0.9` 出 parser-only baseline（pass/partial/fail + variant 混淆矩阵 + 按域统计）。
2. **分类缺口**：
   - **Partial**（variant 对、字段差）→ 字段级抽取缺口（如自然 query 不抽 keyword、时间词漏解析）。
   - **Fail**（variant 错）→ 路由缺口（如跨范畴未路由 file_search、写操作未进 file_action）。
3. **处置（用户决策 #3）**：
   - **低风险顺手修**：根因清晰、改动局部、**v0.5 parser-only 472/26/2 不回归** + 新增单测守门的，直接修。
   - **设计性登记**：需 schema 改动 / 大重构 / 跨模块的，登记 ROADMAP 另开 task，baseline 报告记为 known gap。
4. **达标判定**：修复 + 登记后重测，**目标 v0.9 总体 pass > 90%**。若达标，§6 该指标可标记满足；若因大量设计性 gap 暂未达标，**如实报告达成率 + gap 清单**（不为凑指标而强标 case 让其 pass——那等于自欺）。

## 9. 验收标准

- [ ] `v0.9/cases.json` 恰 **1000 条** = v0.5 500（逐字）+ coverage 500；id 全局唯一。
- [ ] `coverage-cases.json` 500 条全部能反序列化为合法 `SearchIntent`（CI 测试 `coverage_cases_are_schema_valid` 守门）。
- [ ] **v0.5 parser-only 仍 472/26/2 byte-equal**（v0.5 未被触碰）。
- [ ] `generate-evals-v09` 合并确定性（重跑产物逐字节一致）。
- [ ] v0.9 parser-only baseline 已测并写入 `v0.9/README.md`：总体 + 按 variant + 按覆盖域 pass/partial/fail；known gap 清单。
- [ ] 低风险 gap 已修（每个修复 v0.5 不回归 + 新增针对性单测）；设计性 gap 已登记 ROADMAP。
- [ ] 全 workspace `cargo test` 零回归（platform-macos 2 预存 Windows 失败除外）；`fmt --check` ✅；`clippy --workspace --all-targets -D warnings` 0。
- [ ] README（packages/evals）+ v0.9/README.md 更新用法；ROADMAP BETA-13 → done；STATUS 收工段。
- [ ] 抽样 case 经用户 spot-check 背书。

## 10. 风险与缓解

| 风险 | 缓解 |
|---|---|
| **误标产生假失败**污染 baseline | §7 对抗式校验 + schema-valid 机械门 + 抽样人审；语义/约定两轴纪律 |
| coverage 在约定字段上偏离 parser 惯例 → 噪声 partial | §6 约定表统一查表；校验者按表复核约定轴 |
| 合并产物不确定 / id 冲突 | 合并器纯确定性 + id 全局唯一断言 |
| pass 率 < 90% 触发大量 parser 改动 | 范围闸：仅低风险修；设计性 gap 登记另开 task（用户决策 #3 已授权）|
| 为凑 90% 而把可疑 case 标成 pass | §8 明确禁止；如实报告达成率优先于指标 |
| 500 条手标工作量大 | subagent 并行撰写 + 按域分批；coverage-cases.json 可分批合入 |

## 11. 承接 / 后续

- **Class A 双平台 evals**：拿 v0.9 在 macOS 真机 + Windows 跑端到端，验「双平台差距 < 5pp」（M→B 硬指标）。本 task 备好数据集，真机跑留 Class A 会话。
- **BETA-14 Beta 出场评测**：依赖本 task 的 v0.9 baseline。
- **登记的设计性 gap**：随后续 parser 迭代消化。
- **BETA-15B**：v0.9 同义词 / 召回相关 case 为 embedding/LoRA 升级提供对比锚点（与 BETA-15A 召回集互补）。
