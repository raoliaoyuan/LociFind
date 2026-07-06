# Beta 出场报告（草稿骨架）

> 评估人：Claude Code (Opus 4.8)
> 日期：2026-07-06（骨架起草；真机取数待补）
> 阶段：**B：Beta** → **V：1.0** 切换
> 状态：**草稿 / 前置骨架** —— parser-only 口径数据已落，标 `TODO(真机)` 的格子待双平台基准画像真机复跑后填入即可定稿。

本报告按 [ROADMAP §9 出场报告模板](../../ROADMAP.md#9-出场报告模板) 撰写，逐项对照 [§6.3 B 阶段出场指标](../../ROADMAP.md#63-b-阶段出场beta) 与 [§6.5 不可回归约束](../../ROADMAP.md#65-不可回归约束codex-审阅-nice-to-have-14-落地)，checklist 对照 [§8 B→V 切换](../../ROADMAP.md#8-阶段切换-checklist)。

> **为何先出骨架**：B 出场的代码/质量线已就绪（parser-only v0.9 = 994/6/0 = 99.4%，>90% 阈值 ✅；§6.5 回归全程零豁免）。真正未闭合的全是**外部真机条件**（双平台基准画像复跑取数、子集测试库跑数、安装包真机装机、100GB 索引资源占用实测）。本骨架把「还差哪几格真机数」钉成清单，下次上机照填即可定稿出场。

---

## 1. 环境

| 项 | macOS 基准机 | Windows 基准机 |
|---|---|---|
| 型号 | `TODO(真机)`（基准画像 16GB / 512GB SSD） | `TODO(真机)`（v0.9.14 装机验证机，16GB） |
| OS | `TODO(真机)`（macOS 14 / 15 / 26 择一记录） | `TODO(真机)`（Windows 11） |
| Rust 工具链 | rust-toolchain.toml pin | 同 |
| 系统索引状态 | `TODO(真机)`（Spotlight 索引须健康——MVP-27 曾遇 server disabled，须换健康机） | `TODO(真机)`（Windows Search 已索引 + 可选 Everything 在跑） |
| 模型 GGUF | `TODO(真机)`（BETA-08 v1，main-v1-q4_k_m.gguf 940MB） | `TODO(真机)`（BETA-09(a) 模型分发后可实测 fallback） |
| 安装形态 | `TODO(真机)`（DMG，待 BETA-10 macOS CI 产物） | v0.9.14 NSIS 安装包（[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.14) + Scoop [scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind)） |

**关键说明**：本骨架的准确率/回归数据均为 **parser-only 口径**（intent-parser 层，平台无关，双平台 byte-equal，MVP-28 已验证 0pp 差距）。子集命中率（音乐/PDF/OCR）、索引资源占用、安装可用性属**后端执行 + 真机形态**范畴，须真机取数。

## 2. 数据集版本

| 数据集 | 版本 | 数量 | 来源 |
|---|---|---|---|
| Beta evals | v0.9 | 1000 条 | [`packages/evals/fixtures/v0.9/`](../../packages/evals/fixtures/) |
| MVP evals（回归基线） | v0.5 | 500 条 | [`packages/evals/fixtures/v0.5/cases.json`](../../packages/evals/fixtures/v0.5/cases.json) |
| P evals（回归基线） | v0.1 | 50 条 | [`packages/search-backends/common/tests/fixtures/cases.json`](../../packages/search-backends/common/tests/fixtures/cases.json) |
| 音乐 metadata 子集 | v0.9 音乐分桶 | `TODO(真机)` | BETA-13 音乐子集 + 测试音频库 |
| Office/PDF 内容子集 | v0.9 文档分桶 | `TODO(真机)` | BETA-13 文档内容子集 + 测试文档库 |
| OCR 子集 | v0.9 OCR 分桶 | `TODO(真机)` | BETA-13 OCR 子集 + 测试图片库 |
| 同义词召回评测（BETA-15A） | v0 | corpus 100 / cases 42 | `packages/evals/fixtures/synonym-recall/` |

## 3. 准确率

### 3.1 Beta evals v0.9（1000 条，parser-only）

| 口径 | parser-only（双平台 byte-equal） |
|---|---|
| pass（字段严格匹配） | **994 / 1000 = 99.4%** |
| partial | 6 / 1000 = 0.6% |
| fail | **0 / 1000 = 0.0%** |
| valid_intent（schema 合法率） | 100%（parser 产出全部 schema-valid） |

> 来源：[gap-inventory §3.5](./beta-14-gap-inventory-2026-07-04.md#35-追记2026-07-04-ixx-会话两轮收割--四项口径拍板落地) + [clarify options 决策](./beta-14-clarify-options-decision-2026-07-06.md)。演进轨迹：51.4% → 88.1%（BETA-13 收束）→ 92.7%（三刀）→ 95.2%（时间簇+keywords）→ 97.7%（四项口径拍板）→ 98.5%（2026-07-06 clarify options 方案 A）→ **99.4%**（同日老账收割 6 组：synthetic-artist ×4 / no mkv / 碳中和 compound + d3 ft 对齐 ×2 / music 目录 hint / 几个G sort）。variant 分桶：Clarify **67/0/0**、FileSearch 494/5/0、MediaSearch 189/1/0、FileAction 124/0/0、Refine 120/0/0。

### 3.2 按语言分桶（parser-only 严格匹配，§6.3 无独立语言阈值，沿用 §6.2 #2 ≥85% 参照）

| language | pass | 总数 | 严格匹配率 | 判定 |
|---|---|---|---|---|
| en | 328 | 330 | **99.4%** | ✅ |
| mixed | 170 | 171 | **99.4%** | ✅ |
| zh | 496 | 499 | **99.4%** | ✅ |

> parser-only 平台无关取数（2026-07-06 本机）；真机复跑时按同口径复核。

### 3.3 子集命中率（§6.3 硬指标，须真机 + 测试库）

| 子集 | 阈值 | 统计口径 | 实测 | 判定 |
|---|---|---|---|---|
| 音乐 artist / title 准确率 | > 85% | Top-1 命中 | `TODO(真机)` | — |
| Office / PDF 内容 | > 80% | Top-5 命中 | `TODO(真机)` | — |
| OCR | > 75% | Top-5 命中 | `TODO(真机)` | — |

### 3.4 剩余 6 条 partial 归因（不影响 >90% 出场线）

| 簇 | 条数 | 性质 |
|---|---|---|
| ~~clarify options 结构差异~~ | ~~8~~ → **0** | **已清（2026-07-06 方案 A）**：Clarify 桶 67/0/0，Class B 决策清账 |
| ~~可确定性收割的老账/零星~~ | ~~9~~ → **0** | **已清（2026-07-06 同日）**：synthetic-artist ×4（`by` 小写连字符 artist）/ no mkv（裸 no 窄路径）/ 碳中和（compound 占位符保全）+ d3 ft 标注对齐 ×2 / music 目录（mixed hint 形态）/ 几个G（抽象 size → size_desc） |
| v0.5 老账（标注锁定/自身不一致） | 5 | markdown ft（schema-10）、上个月**下载的**动词歧义（schema-14）、项目归档里 location（schema-46a）、downloads hint 双语形态 ×2（sort-059/061）——改 v0.5 标注吃 §6.5 豁免额度，攒批处理 |
| 已记录两难 | 1 | 备份文件（d5-zh-039：「备份文件」整词 vs「备份」，与「临时文件」保留惯例互斥，parser 规则无法两全） |

明细见 [gap-inventory §3.5](./beta-14-gap-inventory-2026-07-04.md) + [clarify options 决策](./beta-14-clarify-options-decision-2026-07-06.md)。**fail=0**，剩余 6 partial 全为 v0.5 标注锁定项/两难项，不阻塞出场线。

## 4. 性能

### 4.1 规则解析路径（沿用 §6.2 #3 参照 p95 < 500ms；B 阶段 §6.3 无独立解析延迟指标）

| 档位 | 平台 | p95 | 判定 | 备注 |
|---|---|---|---|---|
| parser-only | macOS | `TODO(真机)` | — | MVP-28 曾测 0.050ms，v0.9 复跑取数 |
| parser-only | Windows | `TODO(真机)` | — | MVP-28 曾测 0.277ms |

### 4.2 后台索引资源占用（§6.3 硬指标，须 100GB 真机测）

| 指标 | 阈值 | 统计口径 | 实测 | 判定 |
|---|---|---|---|---|
| 后台索引 CPU 占用 | < 15% | 索引 100GB 测试库平均 | `TODO(真机)` | — |
| 后台索引内存占用 | < 1GB | RSS 峰值 | `TODO(真机)` | — |

## 5. 回归对比（§6.5 不可回归约束）

| eval 集 | 上一阶段基线（MVP-28） | 本次（parser-only） | 判定 | 豁免 |
|---|---|---|---|---|
| P evals v0.1（50 条） | pass+partial = 48/50（96%） | `TODO(取数)` | — | — |
| MVP evals v0.5（500 条） | 472/26/2（parser-only） | **495/5/0**（+23 pass、fail 归零；全程逐 case 零回归） | ✅ 不低于基线 | 0 条 |
| Beta evals v0.9（1000 条） | —（本阶段新增） | 994/6/0（99.4%，含 2026-07-06 clarify 方案 A + 老账收割） | — | — |
| BETA-15A 同义词召回（42 条） | 门槛通过（≥70%/≤5%） | `synonym_recall_gate` ✅ | ✅ | 0 条 |

> **§6.5 关键结论（可先落）**：v0.5 从 MVP-28 的 472/26/2 提升到 **495/5/0**（pass +23、fail 2→0），历轮收割全程逐 case 对比零 pass→partial/fail 移动。**Beta 阶段对 MVP eval 不仅不回归，且净提升**，0 豁免（§6.5 累计豁免额度 25 条，本阶段用 0）。P eval v0.1 复跑取数后补格。

## 6. 失败 / 警告 / 已知问题

1. **真机取数缺口（本骨架主缺口）** — 子集命中率（音乐/PDF/OCR）、索引资源占用、安装可用性与性能 p95 均须真机复跑。代码/质量层已就绪，缺口纯属外部条件。
2. ~~clarify options 结构口径~~ — **已拍板并落地（2026-07-06 方案 A）**，见 §3.4 与[决策备忘](./beta-14-clarify-options-decision-2026-07-06.md)；Class B 决策清零。
3. **模型 fallback 的 Windows 实测缺口** — 承接 MVP-28（BETA-09(a)），本机无 GGUF，代码路径平台无关、macOS 已达标。
4. **macOS DMG 产物 CI 未建** — BETA-10 剩余项，Windows 侧 v0.9.14 已出包，macOS DMG CI 待下次触碰 macOS 侧。
5. **v0.9.14 真机装机验证未跑** — cycle 9 六场景 + BETA-10A「下载→放行→装→可用」+ Scoop 装机路径，随下次上机（[manual-test-scenarios](../manual-test-scenarios.md)）。

## 7. 出场指标 checklist（§6.3）

| # | 指标 | 阈值 | 实测 | 判定 |
|---|---|---|---|---|
| 1 | 总体 evals 通过率 | > 90% | parser-only **99.4%**（994/1000）；双平台 byte-equal（MVP-28 已验 0pp） | ✅（真机复跑确认口径） |
| 2 | 音乐 artist / title | > 85% | `TODO(真机)` | ⏳ |
| 3 | Office / PDF 内容 Top-5 | > 80% | `TODO(真机)` | ⏳ |
| 4 | OCR Top-5 | > 75% | `TODO(真机)` | ⏳ |
| 5 | macOS DMG 下载可装可用 + Gatekeeper 绕行文档化 | 通过 | 文档 ✅（[install.md](../install.md)）；DMG 产物 + 真机装 `TODO(真机)` | ⏳ |
| 6 | Windows 安装包下载可装可用 + SmartScreen 说明文档化 | 通过 | 文档 ✅（[install.md](../install.md)）；v0.9.14 出包 ✅；真机装 `TODO(真机)` | ⏳ |
| 7 | 一键删除索引/日志/模型/配置 | 全部可用 | 代码就绪（`CleanupTargets`）；手动验证 `TODO(真机)`（cycle 9 场景） | ⏳ |
| 8 | 后台索引 CPU 占用 | < 15% | `TODO(真机)` | ⏳ |
| 9 | 后台索引内存占用 | < 1GB | `TODO(真机)` | ⏳ |

**§6.5 不可回归**：✅（见 §5，0 豁免，v0.5 净提升）。

**汇总（骨架态）**：9 项中 **#1 质量线已达标**（真机复跑仅为确认双平台口径）、**#5/#6 文档层已就绪、产物/真机装待补**、**#2/#3/#4/#7/#8/#9 待真机取数**。**无任何指标判定为 ✗**——全部 ⏳ 均为外部真机条件，非代码缺陷。

## 8. 下一阶段风险与准备

**BETA-14 评测结论（骨架态）：B 阶段代码/质量层出场指标达标**（parser-only 99.4% > 90%、fail=0、§6.5 净提升 0 豁免）。B→V 正式切换仍受以下 [§8 checklist](../../ROADMAP.md#8-阶段切换-checklist) 项 gating，均非代码：

- ⏳ **双平台基准画像真机复跑**：填 §3.3/§4 全部 `TODO(真机)` 格 + checklist #2/#3/#4/#7/#8/#9。
- ⏳ **安装可用性真机验证**：macOS DMG CI（BETA-10）+ 双平台真机装机（v0.9.14 cycle 9 + BETA-10A + Scoop）。
- ⏳ **公开内测反馈整理**（§8 B→V 列）：依赖设计伙伴/首个真实部署获取（护城河 P0，ROADMAP §5）。
- ✅ ~~clarify options 结构口径拍板~~（Class B，2026-07-06 方案 A 拍板并落地，Class B 清零）。

**已识别风险（沿用 [ROADMAP §7](../../ROADMAP.md#7-风险地图)）**：本地模型低配机延迟（BETA-09a）；100GB 索引资源占用真机首测可能暴露调度回退需求（BETA-07 已做调度，须实测确认 <15%/<1GB）；跨平台单测卫生债（MVP-28 §7.4 记录）。

## 9. 结论（骨架态）

> **B 阶段代码/质量层出场线达标（parser-only 99.4% > 90%、fail=0、回归净提升）；正式出场判定待双平台真机复跑填格 + 安装真机验证 + 内测反馈。**

主要依据（已确立）：
- v0.9 evals parser-only **994/6/0 = 99.4%**（> 90% 阈值，fail 归零；含 2026-07-06 clarify options 方案 A 清 8 条 + 老账收割 9 条）。
- §6.5 回归：v0.5 从 472/26/2 → **495/5/0**（净提升 +23 pass / fail 归零），全程逐 case 零回归，**0 豁免**。
- 分发文档层就绪（[install.md](../install.md) SmartScreen/Gatekeeper/校验/源码构建 + [渠道评估](./beta-10-distribution-channels-2026-07-04.md)）；Windows v0.9.14 安装包 + Scoop bucket 已上线。
- 清除路径代码就绪（`CleanupTargets` + [PRIVACY.md](../../PRIVACY.md) 落盘清单对齐）。

待闭环（全为外部真机条件，非代码）：**§3/§4/§7 的 `TODO(真机)` 格 + macOS DMG CI + 双平台装机 + 公开内测反馈**。建议：代码/质量层判定通过，真机取数随下次上机批量填格定稿，同时并行推进设计伙伴获取以解锁内测反馈项。

下一步：见 [STATUS.md 下一步](../../STATUS.md)。
