# 文档重整方案：定位收敛到「检索底座」+ 会话加载优化

> 2026-07-02 · Claude Code (Fable 5) 起草 · 待 Codex 评审
> 背景会话：用户与 Claude 就项目价值 / 企业场景 / 护城河讨论后拍板定位收敛（见 §1）。
> 本 doc 是落地方案：改哪些文档、怎么改、加载流程怎么优化。评审焦点见 §4 问题清单。

## 1. 背景：本次定位收敛（用户已拍板）

2026-07-02 会话中用户确认三项产品决策：

1. **LociFind = 本地语义检索底座**（个人桌面搜索 + 团队/企业冷归档检索）。
2. **不做分析层**：内容关联分析、摘要、比对、起草等"理解/生成"类能力一律不自建，通过 **BETA-32 MCP daemon + 外部 LLM（Claude 等）组合**实现。LociFind 守住"数据不出门的检索"这一层。
3. **三个目标企业场景**：① 律所案件卷宗检索；② 企业内部审计取证检索；③ 离职员工材料归档检索。共同画像 = **敏感数据不出门 + 冷归档 + 检索者不熟悉语料组织方式 + 需留痕**——四点全部打在现有长板（本地优先 / daemon / 语义召回 / Audit）上，且 OS 原生搜索（Windows Copilot 语义搜索锁 Copilot+ PC）覆盖不到归档服务器冷数据。

由此产生的**能力缺口优先级**（三场景合并）：

| # | 缺口 | 说明 | 现状证据 |
|---|---|---|---|
| 1 | 扫描版 PDF OCR 管线 | `pdf_extract::extract_text` 只读文本层，扫描件（图片型 PDF）提取为空、搜不到；律所/审计材料大头 | `packages/indexer/src/doc_extract.rs` extract_pdf |
| 2 | daemon ACL / 检索权限模型 | 单 bearer token 无 per-root/per-collection 权限；律所信息墙、离职归档 HR 敏感是准入门槛 | `packages/locifind-server` |
| 3 | 邮件格式 eml/msg | 审计取证大量依赖邮件，`DOC_EXTS` 不含 | `packages/indexer/src/scan.rs:558` |
| 4 | 向量检索规模化 | 语义候选为全库暴力扫描，企业归档十万级文档需 sqlite-vec | `doc_db.rs` 候选向量全量加载；BETA-15B-4 已有登记 |
| 5 | 图片语义索引带门槛放开 | BETA-33 cycle 4 一刀切禁图片语义（防 OCR 污染）；照片证据场景需 opt-in + 质量门槛 | cycle 4 follow-up ④ 已登记 |
| 6 | MCP 工作流文档化 | "daemon + Claude 做卷宗问答"打磨成标准 playbook | apps/daemon/README.md 现仅部署样板 |

## 2. A 部分：定位重整落地清单

### A1. PROJECT.md（涉及范围/方向，用户已在会话中拍板 = 已确认）

- **「一句话定位」**：补企业侧半句——个人搜索 Agent 之外，同时是"团队冷归档的本地语义检索底座"。
- **新增「目标场景」小节**：个人桌面（既有叙述）+ 三企业场景一览（每场景一行：画像 + 关键缺口 task 引用）。
- **「不做什么」追加一条**：不做内容关联分析 / 摘要 / 起草等分析层——经 MCP + 外部 LLM 组合实现（引用本 doc §1）。V10-13/V10-15 相应重定性（见 A2）。
- **「核心架构」**：SearchBackend 图下追加一行说明 daemon（locifindd）形态复用同一检索栈。

### A2. ROADMAP.md

1. **§3.3 新增 B7 小节「企业冷归档检索底座（三场景）」**，登记 6 张新卡（编号避开已用 BETA-34）：

| ID | 标题 | 依赖 | 估时 |
|---|---|---|---|
| BETA-35 | 扫描版 PDF OCR 管线（PDF 页渲图 → 复用现有 OCR 引擎层；命中率进 evals） | BETA-02, BETA-03 | 1-2w |
| BETA-36 | daemon 检索权限模型（per-collection token / root 级 ACL；审计留痕） | BETA-32 | 1-2w |
| BETA-37 | 邮件格式提取（eml/msg 进 DOC_EXTS + 提取器；pst 明确不做或后置） | BETA-02 | 1w |
| BETA-38 | 向量检索规模化（sqlite-vec 或等价 ANN；十万级文档水位基准） | BETA-15B | 1-2w |
| BETA-39 | 图片语义索引 opt-in + 质量门槛（解除 cycle 4 一刀切、门槛沿用双层护栏） | BETA-33 cycle 4 | 2-3d |
| BETA-40 | MCP 场景 playbook（三场景各一篇：部署 + 示例 query + Claude 工作流） | BETA-32 | 3-5d |

   定位：与 BETA-32 同属"并行衍生子线"，**不进 §6.3 Beta 出场硬指标**（不推翻既有出场标准）。

2. **V 阶段重定性**：
   - **V10-13**（RAG 文件问答）：`not_started` → `re-scoped`——不自建 RAG UI / 本地作答，改为"MCP 问答工作流打磨"（与 BETA-40 合流或标注被其取代）。
   - **V10-15**（Frozen Research Pack）：synthesis-heavy（LLM 摘要/术语表/阅读地图）与"不做分析层"冲突 → 建议 `dropped` 或降为"冻结索引包（无 LLM 合成）"。**待评审**（§4 Q2）。
   - **V10-16**（LLM 读权限与出处闸门）：保留且**价值上升**——它正是 MCP 路径的横切护栏；依赖从 V10-13/15 改挂 BETA-36/40。
3. **§2 阶段总览**：B/V 行"演示价值"列补企业场景字样（轻改，不动出场条件）。
4. **§11 修订摘要**追加本次定位修订条目。

### A3. README.md

- 顶部定位段补企业/团队侧一句 + daemon 提及。
- **仓库结构图刷新**（已漂移）：补 `packages/locifind-server`、`apps/daemon`、`packages/search-backends/local-index`、`packages/search-backends/semantic-index`、`packages/semantic-index`（以实际目录为准核对）。
- 「协作约定（极简版）」的读取顺序与 CONVENTIONS §2 不一致（README 写 PROJECT→STATUS→ROADMAP→CONVENTIONS，CONVENTIONS 写 PROJECT→STATUS→CONVENTIONS→ROADMAP 定向）→ 统一为 CONVENTIONS 口径。

### A4. 三份计划书（docs/）

本次**不动正文**（CONVENTIONS §9 需逐份确认、且计划书是历史设计记录）。仅在 PROJECT.md 定位处注明"2026-07-02 定位收敛，计划书中与分析层相关的展望以 PROJECT.md 为准"。**待评审**（§4 Q3）。

## 3. B 部分：会话文档加载优化

### 3.1 现状痛点（量化）

- **STATUS.md 312 行 ≈ 36k token**，超自定目标（15-25KB）一倍以上；CONVENTIONS 要求"全文读"，每次会话开销巨大。
- **「当前阶段」是 2000+ 字巨型段落**（历史成就流水式追加），"当前位置"信息被淹没——与 CONVENTIONS §1"不要把 task 详情写进 STATUS"自相矛盾。
- **「当前 Task」保留 3 代"先前"叙述**，与会话日志重复。
- **「下一步」有复制漂移**：同一"v1.0 路径候选"列表在同一节出现两次（2026-06-29 条目内）。
- **会话日志单条 30-70 行**（含改动概览 / 接受标准 / 对话流水全量），5-10 条即 300+ 行。

### 3.2 提案：不新增文件，靠 STATUS 强制骨架 + 硬预算

**STATUS.md 重构为固定骨架**（收工流程强制维持）：

```markdown
# LociFind 项目状态
## 📍 速览                        ← 新增，≤15 行，每次收工必刷
- 定位一句话（检索底座 + 三场景 + 不做分析层）→ 链 PROJECT.md
- 阶段 / 版本 / 当前 task ID + 一句话
- 下一步 top-3（每条一行，链 ROADMAP task ID）
- 阻塞 / 待用户决策 top-N
## 当前 Task                      ← 只保留最新 1 条，≤15 行
## 下一步                         ← 单一列表 ≤10 条，收工时去重刷新
## 阻塞 / 待用户决策
## 会话日志                       ← ≤5 条 × 每条 ≤15 行摘要
```

配套规则：

1. **会话日志两级制**：STATUS 内只留**摘要**（承接 / 关键决策 / 产出 / 未尽事宜，≤15 行）；改动概览、接受标准逐条、用户对话流水等**详录直接写进 `docs/session-logs/session-details-YYYY-MM.md`**（按月，收工时同 commit 落库），STATUS 摘要末尾放链接。归档不再是"溢出才滚动"，而是**详录从第一天就不进 STATUS**。
2. **删除「当前阶段」「总体进度」两节**：阶段级信息由速览一行 + ROADMAP §2/任务状态承担（单一信源归位）。历史成就段整体移入归档文件。
3. **硬预算自检**：收工流程加一步——`STATUS.md` 超 **15KB** 即视为收工未完成，必须先瘦身再 commit。
4. **CONVENTIONS 更新**：§2 会话开始流程不变（PROJECT → STATUS → CONVENTIONS → ROADMAP 定向），但注明 STATUS 守 15KB 是"全文读"成立的前提；§3 收工流程改写为上述两级制 + 自检步骤。
5. **入口文件（CLAUDE.md / AGENTS.md / GEMINI.md）**：会话必读顺序前加一行**定位速记**（"检索底座 + 三场景 + 不做分析层，详 PROJECT.md"）——10 秒锚定方向；信息正文仍以 PROJECT.md 为单一信源。**待评审**（§4 Q4，与单一信源原则的张力）。
6. **本次顺带执行一次 STATUS 大瘦身**：现 312 行按新骨架重构，所有被删内容逐字移入 `docs/session-logs/` 归档（只移动不删除）。

### 3.3 预期效果

- 会话开始必读的上下文从 ~45k token（PROJECT 4KB + STATUS 36k + CONVENTIONS 8KB）降到 ~12-15k token。
- "聚焦目标 / 当前进展 / 待执行"三问在速览块 15 行内可答。

## 4. 待评审问题清单（Codex 请逐条表态：APPROVE / OBJECT+替代 / SUGGEST）

- **Q1**：新 task 簇放 B 阶段新 B7 小节（并行子线、不进 Beta 出场指标）——还是放 V 阶段？
- **Q2**：V10-15 Frozen Research Pack 处置：dropped / 重定性为无 LLM 合成的"冻结索引包" / 保留待议？
- **Q3**：三份计划书正文本次不动、只在 PROJECT.md 注明"以 PROJECT 为准"——可接受？
- **Q4**：入口文件加一行定位速记，是否违反单一信源原则？（替代：完全依赖 PROJECT.md + STATUS 速览块）
- **Q5**：会话日志两级制（摘要进 STATUS、详录进 session-details-YYYY-MM.md）是否会造成"两处写作、易漂移"？（替代：维持现状滚动归档但把单条上限压到 15 行）
- **Q6**：STATUS 15KB 硬上限 + 收工自检，预算数值是否合理？
- **Q7**：BETA-35~40 的拆分粒度与估时是否合理？有无遗漏（如归档去重 / 多归档主体分区）？

## 5. 验收标准（本次文档重整完成的定义）

1. PROJECT.md 含目标场景小节 + "不做分析层"条目；与本 doc §1 一致。
2. ROADMAP.md：B7 六卡登记、V10-13/15/16 状态与定性更新、§2 轻改、§11 追加修订条目。
3. README.md：定位段 + 结构图 + 读取顺序三处修正。
4. STATUS.md 重构为 §3.2 骨架、≤15KB；被移内容在 docs/session-logs/ 逐字可查。
5. CONVENTIONS.md §2/§3 更新为两级制 + 自检；入口文件三份按 Q4 结论处理。
6. 全部改动一次中文 commit 落库（收工流程）。

## 6. Codex 评审记录

> 待 Codex 填写 / Claude 代录。

### 2026-07-02 — Codex — 完整评审结论

结论：**APPROVE with required adjustments**。本方案与用户今日三项产品决策一致：LociFind 收敛为「本地语义检索底座」，分析/生成层外置到 BETA-32 MCP daemon + 外部 LLM，企业侧先聚焦三类冷归档检索场景。§2/§3 的大方向可落地，但需要守住三条边界：**PROJECT / ROADMAP / STATUS / CONVENTIONS 单一信源不互相复述**，**ROADMAP §6.3 Beta 出场硬指标不被新企业子线改写**，**STATUS 瘦身必须从机制上可持续而不是再造一个长文档入口**。

#### Q1：新 task 簇放 B 阶段 B7，还是放 V 阶段？

**APPROVE**：放在 B 阶段 §3.3 新 B7 小节更合适，但必须明确为「并行衍生子线 / 企业冷归档检索底座」，**不进入 §6.3 Beta 出场硬指标，也不阻塞 B→V 切换**。

理由：BETA-32 daemon 已经是 Beta 阶段的并行衍生子线，新 BETA-35~40 是它的场景化补强，放 B 阶段能保持依赖链贴近现有代码与验证节奏；若放 V 阶段，会把企业冷归档检索的关键风险推迟到 1.0，反而不利于验证新定位。落地时建议在 B7 小节开头加一句红线：「本小节不修改 §6.3；BETA-14 / §6.3 仍是 Beta 出场依据」。

#### Q2：V10-15 Frozen Research Pack 如何处置？

**OBJECT + 替代方案**：不建议简单 `dropped`，建议改为 **`re-scoped`：Frozen Index Pack（冻结检索包，无内置 LLM 合成）**。

替代定义：只做显式 pin 资料夹的冻结快照、原文索引、来源映射、文件/段落 ID、mtime/hash 失效检测、可导出给 MCP daemon 的检索上下文；**不内置摘要、术语表、阅读地图、问答 UI 或本地作答**。摘要/比对/阅读地图由外部 LLM 通过 MCP 工作流完成。这样既保留「冷归档可复现、可留痕、可交接」的价值，又不违背「不做分析层」。

V10-13 同理建议 `re-scoped` 为「MCP 文件问答工作流 / Playbook」，不自建 RAG UI；V10-16 建议改名或重定性为「MCP/LLM 读取权限与出处闸门」，依赖改挂 BETA-36/BETA-40，价值上升但不再绑定本地 LLM 功能发布。

#### Q3：三份计划书正文本次不动，只在 PROJECT.md 注明以 PROJECT 为准，是否可接受？

**APPROVE**：可接受，而且符合 CONVENTIONS §9 对 `docs/` 三份计划书的谨慎修改要求。

但建议 PROJECT.md 的说明不要写成「旧计划书失效」这种宽泛表述，而写成窄口径：「2026-07-02 起，定位/范围以本文件为准；早期计划书中涉及摘要、比对、起草、内容关联分析等分析层展望，仅作为历史设计记录，不代表当前自建范围。」这样既维护 PROJECT.md 的方向单一信源，也避免在历史计划书中做大面积考古式改写。

#### Q4：入口文件加一行定位速记是否违反单一信源？

**OBJECT + 替代方案**：不建议在 CLAUDE.md / AGENTS.md / GEMINI.md 三个入口文件加入实质性「定位速记」。这会把产品定位复制到第四类文件里，后续最容易漂移，违反 CONVENTIONS §1「目标只在 PROJECT.md」的精神。

替代方案：入口文件只保留指路语，不复述定位正文。例如：「项目定位以 [PROJECT.md](./PROJECT.md) 顶部为准；开始工作前先读 PROJECT/STATUS/CONVENTIONS，并按需定向读 ROADMAP。」真正的 10 秒锚定放在 PROJECT.md 顶部一句话 + STATUS.md「速览」里。STATUS 速览可以短暂引用定位，但应带 PROJECT.md 链接，并在收工时随当前状态刷新。

#### Q5：会话日志两级制是否会造成两处写作、易漂移？

**SUGGEST**：方向可取，但需要收窄为「STATUS 摘要 + 详录归档」的单向关系，避免两个文件各写一套事实。

建议规则：

- STATUS 只写 5-10 行摘要：承接、关键决策、产出、未尽事项、详录链接。
- `docs/session-logs/session-details-YYYY-MM.md` 只放本会话更细的改动概览、验证命令、对话流水和证据，不再承担「当前状态」。
- 详录不是强制每次都写满；只有会话超过摘要容量、涉及复杂验收/真机证据/多工具交接时才写。
- 收工时从详录反向更新 STATUS 是禁止动作；STATUS 的当前 task / 下一步仍直接维护，防止状态漂移。

这样比现状「长日志先塞 STATUS，超了再滚动」更可持续，但不要把详录变成第二个 STATUS。

#### Q6：STATUS 15KB 硬上限是否合理？

**SUGGEST**：15KB 合理，可以作为硬上限；建议同时设软目标 **10-12KB**。

当前 CONVENTIONS 写的是长期维持 15-25KB，但实际 STATUS 已经明显膨胀。若本次修改 CONVENTIONS，建议统一为：「软目标 10-12KB，硬上限 15KB；超过硬上限不得收工 commit，除非本次 commit 正是在做瘦身且结束后达标。」实现上可在收工检查用 PowerShell / shell 统计文件字节数，不需要引入脚本依赖。

#### Q7：BETA-35~40 拆分粒度、估时与遗漏

**SUGGEST**：六张卡粒度总体合理，估时大体可接受，但建议补 4 个遗漏/修正：

1. **归档集合 / collection 模型**：建议并入 BETA-36 或拆新卡。企业三场景都需要 collection 概念：root 分组、归档主体、案件/员工/审计项目边界、显示名、只读状态、审计标签。否则 ACL 只能按路径打补丁。
2. **导入去重与身份稳定**：建议登记为 BETA-38 的子验收或新增卡。冷归档常有重复副本、迁移盘、压缩包展开副本；需要至少定义 path/hash/mtime/size 的 doc identity 策略，避免十万级索引和审计留痕失真。
3. **企业场景 eval fixture**：BETA-35/37/38 不应只做功能实现，要有三场景合成语料与检索 query 子集，覆盖扫描 PDF、邮件、附件、跨语言/别名、近重复材料。可作为 BETA-40 playbook 的前置或共同验收。
4. **邮件附件与 PST 边界**：BETA-37 只写 eml/msg 不够。建议验收明确「eml/msg 正文 + 附件提取 + headers 基础字段（from/to/date/subject）」，PST 先后置可以，但要写明不在本卡范围，避免审计取证场景误期望。

BETA-35 扫描 PDF OCR 的 1-2w 估时偏乐观但可先登记；验收必须写「页渲染、OCR、页码/来源映射、失败页记录、命中预览能回到页」四件事，否则会只得到能搜不能取证的 OCR。BETA-36 ACL 也建议把 bearer token 升级、per-root/per-collection 权限、audit subject 作为验收，不要只写权限模型概念。

#### §2 / §3 落地清单冲突与遗漏检查

**单一信源原则**：A1/A2/A3 整体没有破坏单一信源，但 A3 README 只能做入口说明和仓库结构，不应承载定位细节；入口文件加定位速记需要按 Q4 改成指路而非复述。A4「计划书不动，PROJECT 标注当前定位」是正确的单一信源处理。

**ROADMAP §6.3 Beta 出场指标**：A2 明确「B7 不进 §6.3」是必要且正确的。落地 ROADMAP 时不要改 §6.3 表格阈值，不要把 BETA-35~40 加进 B→V checklist；最多在 §2/B 阶段演示价值或 §3.3 B7 注明「企业场景验证子线」。

**STATUS 瘦身骨架可持续性**：§3.2 方向正确，但建议保留「当前阶段」作为 1 行字段而非完全删除标题信息；否则新会话仍要跳 ROADMAP 才知道阶段。推荐骨架为「速览 / 当前 Task / 下一步 / 阻塞 / 会话日志」，其中速览第一行写阶段与版本。会话日志单条上限建议 10-15 行，超过即写详录链接。

**README 结构清单**：A3 提到的目录漂移属实：当前仓库已有 `packages/locifind-server`、`apps/daemon`、`packages/search-backends/local-index`、`packages/search-backends/semantic-index`；但未见 `packages/semantic-index` 这个顶层目录，落地时应以实际目录核对，避免 README 新增不存在路径。

**额外建议**：PROJECT.md 的「阶段路线图」目前仍把 Beta 描述为音乐/Office/PDF/OCR/签名分发，可轻改一句体现「语义检索底座」和 daemon 复用，但不要把 BETA-35~40 的 task 细节写进 PROJECT；task 细节留 ROADMAP。

## 7. 采纳记录（Claude Code，2026-07-02 同日落地）

Codex 全部意见**照单采纳**，落地映射：

- **Q1** → ROADMAP §3.3 新增 B7 小节，开头红线注明"不修改 §6.3、不阻塞 B→V"。
- **Q2** → V10-15 re-scoped 为 **Frozen Index Pack（无内置 LLM 合成）**；V10-13 re-scoped 并入 BETA-40；V10-16 重定性为「MCP/LLM 读取权限与出处闸门」、依赖改挂 BETA-36/40。
- **Q3** → PROJECT.md「不做什么」采用窄口径表述（"早期计划书分析层展望仅作历史设计记录"），计划书正文未动。
- **Q4** → 入口文件三份只加指路语（"定位/目标场景/不做什么以 PROJECT.md 为准，本文件不复述"），未复制定位正文。
- **Q5** → CONVENTIONS §3 落地"单向两级制"：STATUS 摘要 5-15 行、详录非强制、**禁止详录反向改写 STATUS**。
- **Q6** → CONVENTIONS §3 落地"软 10-12KB / 硬 15KB + 收工闸门"表述；本次瘦身后 STATUS = 8.5KB。
- **Q7** → collection 模型并入 BETA-36 验收、doc identity/去重并入 BETA-38 验收、企业评测语料立新卡 **BETA-41**、BETA-37 验收补附件 + headers + pst 不在范围、BETA-35 验收写足"页渲染 / OCR / 页码来源映射 / 失败页记录"四件事。
- **落地清单检查** → STATUS 保留阶段信息于速览第一行；README 结构图按实际目录核对（无 `packages/semantic-index` 顶层目录，语义索引在 `packages/search-backends/semantic-index`）；PROJECT「核心架构」补 daemon 一行、task 细节未进 PROJECT。

改动文件：PROJECT.md / ROADMAP.md / README.md / CONVENTIONS.md / CLAUDE.md / AGENTS.md / GEMINI.md / STATUS.md（重构，旧文全文归档 [STATUS-archive-2026-07.md](../session-logs/STATUS-archive-2026-07.md)）。
