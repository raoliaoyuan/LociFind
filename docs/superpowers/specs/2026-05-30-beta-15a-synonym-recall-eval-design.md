# BETA-15A — 同义词召回定量评测集 — 设计

> 日期：2026-05-30
> 作者：Claude Code (Opus 4.8)
> 阶段：B / B6（承接 [BETA-15](./2026-05-30-synonym-keyword-expansion-design.md) 同义词扩展 + [BETA-15E](./2026-05-30-beta-15e-gazetteer-design.md) gazetteer 注入）
> 类型：新建离线确定性召回评测 + CI 回归门，**不动 parser / spotlight / 词典源 / v0.5 fixtures**

## 1. 背景与目标

BETA-15 给 harness 加了同义词扩展（`SynonymExpander`），BETA-15E 进一步让自然中文 query 经 gazetteer 注入内容词 keyword group。但二者的验收都只有「手测 scenario + v0.5 evals 不回归」——**召回质量从未被定量衡量**：词典改一组、gazetteer 守护逻辑回退，召回率掉多少没有任何自动信号。

BETA-15 spec §10 与 BETA-15E spec §8 都把本评测集列为明确承接项（原 ROADMAP 编号 BETA-11A，重编号族下为 BETA-15A）。

**目标**：建一套**离线、确定性、可进 CI** 的同义词召回评测集，量化「当前手维护词典 + gazetteer」在一组合成 query 上的**召回率 / 假阳率**，给后续 embedding / LoRA 升级（BETA-15B）提供对比 baseline，并作为词典/扩展逻辑回退的回归门。

**核心约束**：不改 parser、不改 spotlight backend、不改词典源、不碰 v0.5 evals fixtures → 既有 evals parser-only **472/26/2 byte-equal** 自然成立。

## 2. 范围（已与用户对齐）

| 维度 | 决策 |
|---|---|
| 定位 | **CI 回归门 + baseline**（不是一次性报告）。退出码非 0 阻断 `scripts/ci.sh` |
| 测量路线 | **离线确定性模拟**——不跑 Spotlight / mdfind / 模型。隔离衡量「词典 + gazetteer + 扩展」层 |
| 覆盖 | **zh + en 全覆盖**，~40-50 case，覆盖内容词桶（zh office 汇报 / 文档管理 / 个人；en document / office / personal） |
| 管线 | 走真实 `parse → expand` 全链路（不直接喂 keyword），才能抓到 gazetteer 回归 |
| corpus | 手工标注合成文件集（非生成器产物），含期望命中文件 + 干扰文件 |

### 不做（YAGNI 闸口）

- 真 Spotlight 端到端召回（flaky + 仅 macOS + macOS 26 谓词 bug；后端正确性 BETA-15D 已验，本评测刻意隔离同义词层）
- embedding / LoRA 语义召回（BETA-15B）
- WindowsSearchBackend / EverythingBackend 召回（BETA-15C）
- 词典扩容（> 200 组触发 BETA-15B）
- 真实文件内容全文索引模拟（corpus content_terms 为轻量标注，非真文档正文）
- fixture 生成器（召回 case 需人工标注期望命中，手维护更诚实）

## 3. 架构与数据流

完整复用真实管线，只在最后一步用模拟替代 mdfind：

```
query ──parse──▶ SearchIntent ──expand(intent, query)──▶ keyword_groups
  (真 locifind_intent_parser::parse)
                 (真 YamlSynonymExpander::from_paths(ship 的 zh/en.yaml) + gazetteer)
                                                              │
                          ┌───────────────────────────────────┘
                          ▼
              对合成 corpus 做「忠实于 BETA-15D 的子串匹配」
                          ▼
              actual_hits  ──vs──  expected_hits（case 标注）
                          ▼
              召回率 / 假阳率（总 + 按桶 + 按语言分桶）
```

**关键性质**：必须走 `parse → expand` 全链路。若直接喂 keyword 给 `expand_one`，则只测了词典图、抓不到 BETA-15E gazetteer 在自然 query 上的触发回归——而 gazetteer 是本评测最该守护的新增能力。

## 4. 匹配模拟模型（忠实于 BETA-15D 双查询）

BETA-15D 的 Q1 对文件名用 `kMDItemFSName == "*kw*"cd` glob（大小写不敏感子串），Q2 对内容用 `kMDItemTextContent CONTAINS[cd]`（大小写不敏感子串）。模拟规则因此定为：

> 文件 F 命中 query Q ⟺ Q 经 `parse → expand` 得到的 `keyword_groups` 中，**每一个** group 都被 F 满足；某 group 被满足 ⟺ 该 group 的**任一**成员（head 或 synonym），大小写不敏感地作为子串出现在 F 的 `filename` 或某个 `content_terms[i]` 中。

即：**组内 OR、组间 AND**，忠实于 BETA-15 后端语义（spec §3「组内 OR、组间 AND」）与 BETA-15D 谓词形态。注意 expander 在 parser 抽到多个 keyword 时确实会产出多 group（如 `[工作汇报, ppt]`），故匹配函数必须正确实现组间 AND；而 BETA-15E gazetteer 兼底只注入单 group，单 group 下 AND 退化为「任一成员子串命中」。

匹配为纯函数 `fn matches(groups: &[KeywordGroup], file: &CorpusFile) -> bool`，无 I/O、无平台依赖、可单测。

> 注：keyword 缺失（gazetteer 也未命中）→ `keyword_groups` 为空 → 该 query 不命中任何文件（actual_hits 空）。这是合法状态，由 case 的 expected_hits 决定它算召回失败还是本就无期望命中。

## 5. Corpus 与 case fixture 格式

新目录 `packages/evals/fixtures/synonym-recall/`，两份 checked-in JSON。

### 5.1 `corpus.json` —— 合成文件全集

```json
[
  { "id": "f-zhishu-ppt",      "filename": "述职.ppt",       "content_terms": [] },
  { "id": "f-xiebao-pdf",      "filename": "购房协议.pdf",    "content_terms": [] },
  { "id": "f-distract-plan",   "filename": "项目计划.docx",   "content_terms": [] },
  { "id": "f-cv-en",           "filename": "my_cv.docx",     "content_terms": [] }
]
```

- 文件名刻意用**同义词/别名**而非 query 原词（如期望命中 `述职.ppt` 验证 query「工作汇报」经扩展才命中）。
- **干扰文件**（distractor）：名字含其它桶内容词或近似词，放进 corpus 但不在相关 case 的 expected_hits 里，用于度量假阳率。

### 5.2 `cases.json` —— 召回 case

```json
[
  {
    "id": "recall-zh-office-01",
    "query": "找一份工作汇报相关的ppt",
    "language": "zh",
    "bucket": "office",
    "expected_hits": ["f-zhishu-ppt"]
  }
]
```

- `bucket` ∈ {office, document, personal, ...}（用于分桶报告，对齐词典 domain）。
- `expected_hits`：该 query **应当**命中的文件 id 集合。
- 引用完整性：cases 中每个 file id 必须存在于 corpus（单测断言）。

## 6. 指标与报告

| 指标 | 定义 |
|---|---|
| **召回率 recall** | Σ(命中的 expected 文件数) / Σ(全部 expected 文件数)；总 + 按 bucket + 按 language 分桶 |
| **假阳率 false-positive** | Σ_case \|actual_hits \ expected_hits\| / Σ_case (\|corpus\| − \|expected_hits\|)。即在所有「(case, 非该 case 预期的 corpus 文件)」对中，被错误命中的比例。度量过度扩词 |

报告输出（与现有 evals 报告风格一致）：

- 总览：总 recall / 总 fp / case 数 / corpus 大小
- 分桶：按 bucket、按 language 的 recall
- `--only-failures`：列出召回未达标（漏命中）或引入假阳的 case 明细（query + expected + actual + 缺失/多余文件）
- `--json`：机器可读报告

指标计算为纯函数（`recall`、`false_positive_rate`），可单测边界（全命中 / 全漏 / 空 expected）。

## 7. CI 回归门

新增 bin `synonym_recall`，`scripts/ci.sh` 末尾调用。门槛对齐 ROADMAP BETA-15A 验收：

- **召回率 ≥ 70%**、**假阳率 ≤ 5%**
- 未达标 → 退出码非 0 → 阻断 CI；词典改动 / gazetteer 守护回退导致召回掉到阈值下立刻被抓。
- **baseline 锚定**：首次实跑确定实测召回/假阳，写进 STATUS 与 README。CI 门槛阈值取「ROADMAP 验收下限（70% / 5%）」为准；若实测显著高于下限（如召回 90%），在报告中记录实测 baseline 供 BETA-15B 对比，但**门槛仍守 70%/5% 下限**（避免阈值过紧导致正常词典微调误报红）。
- 退出码语义：达标 0 / 未达标 1 / 加载或解析错误 2。

## 8. 组件与文件布局

| 文件 | 改动 |
|---|---|
| `packages/evals/fixtures/synonym-recall/corpus.json` | 新建（手工标注合成文件全集） |
| `packages/evals/fixtures/synonym-recall/cases.json` | 新建（手工标注召回 case + expected_hits） |
| `packages/evals/src/recall.rs`（或 lib 内新模块） | 召回评测核心：`CorpusFile` / `RecallCase` 类型 + `matches()` 纯函数 + `recall()` / `false_positive_rate()` 指标 + 报告聚合。可单测 |
| `packages/evals/src/bin/synonym_recall.rs` | 新建 bin：加载 ship 词典（`YamlSynonymExpander::from_paths`）→ 对每 case `parse + expand` → `matches` → 报告 + 门槛退出码。支持 `--json` / `--only-failures` |
| `packages/evals/Cargo.toml` | 加 `locifind-harness`（expander）依赖（intent-parser / common 已在依赖树） |
| `scripts/ci.sh` | 加一行 `cargo run -p locifind-evals --bin synonym_recall`（门槛退出码） |
| `packages/evals/README.md` | 新增「同义词召回评测 (BETA-15A)」节：用法 + 判定 + 门槛 |
| 词典路径解析 | bin 从 workspace 根 `resources/synonyms/{zh,en}.yaml` 读（evals 是 dev/CI 态，无 .app 打包态，路径解析比 desktop 简单） |

## 9. 测试

| 层 | case |
|---|---|
| `matches()` 纯函数单测 | 大小写不敏感子串命中 / 不命中 / 多成员 group 任一命中 / 空 group 不命中 / content_terms 命中 |
| 指标单测 | recall 全命中=1.0 / 全漏=0.0 / 分桶聚合；fp 无干扰=0 / 有错误命中>0 |
| fixture 完整性单测 | corpus.json / cases.json 反序列化通过；cases 引用的每个 file id 存在于 corpus；id 无重复 |
| 端到端 bin 冒烟 | 加载 ship 词典跑全 case，断言总召回 ≥ 70% 且假阳 ≤ 5%（与 CI 门同源，锁住 baseline） |

## 10. 回归 guard（reviewer 检查项）

下述应零改动：

- `packages/intent-parser/**`
- `packages/search-backends/spotlight/**`
- `packages/harness/src/synonym/**`（仅**消费** expander，不改其逻辑）
- `resources/synonyms/{zh,en}.yaml`（词典源不动；本评测衡量它，不改它）
- `packages/evals/fixtures/v0.5/**`（parser eval 数据集）

验证：`bash scripts/ci.sh` 全过；`cargo run -p locifind-evals --bin evals -- --fixtures v0.5` parser-only **472/26/2 byte-equal**。

## 11. 非目标

见 §2「不做」。重申：本评测刻意**隔离同义词层**，不验证后端正确性（BETA-15D 已验）、不验证 parser 准确率（v0.5 evals 已守）。它只回答一个问题：**当前词典 + gazetteer，在一组合成痛点 query 上，召回率和假阳率是多少，有没有回退。**

## 12. 升级路径

- 召回率连续两次评测无法继续提升、或词典 > 200 组 → 触发 BETA-15B（embedding / LoRA 在线扩词），用本评测集做前后对比。
- 多概念 query 多 group 注入（BETA-15E 非目标，留后续）落地后，corpus 加多概念 case 验证组间 AND 收窄（`matches()` 已实现组间 AND，无需改语义）。
- BETA-15C Windows 后端召回：可复用本 corpus + cases，匹配模拟换成各后端谓词语义。

## 13. 修订记录

| 日期 | 修订 |
|---|---|
| 2026-05-30 | v0.1：初稿（Claude Code Opus 4.8，brainstorming 流程） |
