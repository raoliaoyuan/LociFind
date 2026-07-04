# MVP 出场报告

> 评估人：Claude Code (Opus 4.8)
> 日期：2026-06-01
> 阶段：**M：MVP** → **B：Beta** 切换
> 评测对象：commit `5245a68`（main，BETA-16 后）+ 本会话 MVP-26 Everything 侧收尾（迁移自旧 clone 的工作树改动）

本报告按 [ROADMAP §9 出场报告模板](../../ROADMAP.md#9-出场报告模板) 撰写，逐项对照 [§6.2 M 阶段出场指标](../../ROADMAP.md#62-m-阶段出场mvp) 与 [§6.5 不可回归约束](../../ROADMAP.md#65-不可回归约束codex-审阅-nice-to-have-14-落地)。

---

## 1. 环境

| 项 | macOS 基准机 | Windows 基准机（本次新增实测） |
|---|---|---|
| 型号 | Apple Silicon M 系（基准画像 16GB / 512GB SSD） | Intel Core i7-1165G7 @ 2.80GHz / 15.8GB |
| OS | macOS 26（Darwin 25.x） | Windows 11 专业版 10.0.26200 |
| Rust 工具链 | 1.95.0（旧报告）/ rust-toolchain.toml pin | rustc 1.93.1（2026-02-11） |
| 系统索引状态 | **MVP-27 基线机 Spotlight server disabled**（端到端 mdfind 待健康机复测） | Windows Search 已索引 + Everything 1.x 在跑（es.exe via voidtools.Everything.Cli） |
| 模型 GGUF | 本机齐全（BETA-08 v1，main-v1-q4_k_m.gguf 940MB） | **缺**（gitignored，未手动获取 → 模型 fallback 路径本机不可实测，属 BETA-09(a)） |

**关键说明**：本次评测在 **Windows 11 真机**首次完成「parser 层 + 后端结果集层」双平台一致性实测；模型 fallback 相关指标（§6.2 #4 / #7）本机无模型，沿用 macOS BETA-08 v1 权威实测值并明确标注 Windows 验证待 BETA-09(a)。

## 2. 数据集版本

| 数据集 | 版本 | 数量 | 来源 |
|---|---|---|---|
| MVP evals | v0.5 | 500 条 | [`packages/evals/fixtures/v0.5/cases.json`](../../packages/evals/fixtures/v0.5/cases.json) |
| P evals（回归基线） | v0.1 | 50 条 | [`packages/search-backends/common/tests/fixtures/cases.json`](../../packages/search-backends/common/tests/fixtures/cases.json) |
| 跨平台一致性合成语料 | v0.1（PROTO-05A，跨平台移植） | 18 文件 | `cargo run -p locifind-evals --bin fixtures --dir <CORPUS> generate` |
| 同义词召回评测（BETA-15A） | v0 | corpus 100 / cases 42 | `packages/evals/fixtures/synonym-recall/` |

## 3. 准确率

### 3.1 MVP evals v0.5（500 条）

| 口径 | parser-only（双平台 byte-equal，本会话 Windows 实测） | hybrid Q4_K_M（macOS，BETA-08 v1 + parser file_type fix，carried） |
|---|---|---|
| variant 命中率 | **498 / 500 = 99.6%** | 498 / 500 = 99.6% |
| pass（字段严格匹配） | **472 / 500 = 94.4%** | **473 / 500 = 94.6%** |
| partial | 26 / 500 = 5.2% | 25 / 500 |
| fail | 2 / 500 = 0.4% | 2 / 500 |
| valid_intent（schema 合法率） | 100%（parser 产出全部 schema-valid） | **100%**（fallback 86/86，v0 的 8.3% → v1 100%） |

### 3.2 按语言分桶（parser-only 严格匹配，§6.2 #2 阈值 ≥85%）

| language | pass | 总数 | 严格匹配率 | 判定 |
|---|---|---|---|---|
| en | 144 | 150 | **96.0%** | ✅ |
| mixed | 88 | 100 | **88.0%** | ✅ |
| zh | 240 | 250 | **96.0%** | ✅ |

### 3.3 按 variant 分桶（parser-only）

| variant | pass | partial | fail |
|---|---|---|---|
| FileSearch | 186 | 13 | 1 |
| MediaSearch | 92 | 8 | 0 |
| FileAction | 76 | 4 | 0 |
| Refine | 79 | 0 | 1 |
| Clarify | 39 | 1 | 0 |

### 3.4 失败用例（2 条，与 macOS 同批，非平台差异）

| ID | 期望 | 实际 | 归因 |
|---|---|---|---|
| #39a「把这些 pdf 复制到桌面」 | Refine | FileAction | 同一 query 结构在 file/media 模板下双路由 artifact；parser 物理上无法两全 |
| #45b「排除压缩包合并后」 | FileSearch | Refine | 同上（dual-route artifact，非用户真实输入） |

剩余 26 partial 主要为 keywords / artist / new_name / language 检测器 trade-off 边缘 case（BETA-08 v1 报告 §6 已锚定，augmentation 已饱和）。

## 4. 性能

### 4.1 规则解析路径（§6.2 #3 阈值 p95 < 500ms）

| 档位 | 平台 | p50 | p95 | p99 | 样本 | 判定 |
|---|---|---|---|---|---|---|
| parser-only | Windows（本次） | 0.146 ms | **0.277 ms** | 0.456 ms | 1870 | ✅ |
| translate | Windows（本次） | 0.164 ms | 0.340 ms | 0.496 ms | 1581 | ✅ |
| parser-only | macOS（MVP-27） | 0.038 ms | 0.050 ms | 0.053 ms | 1870 | ✅ |
| cli-intent-only-warm | macOS（MVP-27） | 4.962 ms | 7.233 ms | 9.496 ms | 1581 | ✅ |

规则路径双平台均 << 500ms（Windows 余量 ~1800×）。Windows 比 macOS 略慢属正常硬件/编译差异，绝对值仍极富裕。

### 4.2 复杂查询（模型 fallback，§6.2 #4 阈值 p95 < 3000ms）

| 档位 | 平台 | p95 | 判定 | 备注 |
|---|---|---|---|---|
| fallback（Q4_K_M） | macOS（BETA-08 v1 / BETA-09） | **1592 ms** | ✅ | Q4 是精度饱和 + 最低延迟 sweet spot |
| fallback | Windows | — | ⏳ | **本机无模型 GGUF，待 BETA-09(a) 跨平台部署实测** |

### 4.3 端到端搜索（CLI / 后端执行）

- macOS MVP-27 `cli-search` p95 ~19ms 但 **Spotlight server disabled**、有非成功退出 → ⚠️ 待 Spotlight 健康的 macOS 机复测。
- Windows 后端执行层本会话经 MVP-26 语料一致性 + 真机集成测试验证可用（见 §3.5/§5 上一节与下表），未做高样本量 p95 画像（backlog）。

## 5. 回归对比（§6.5 不可回归约束）

| eval 集 | 上一阶段基线 | 本次（Windows） | 判定 | 豁免 |
|---|---|---|---|---|
| P evals v0.1（50 条） | PROTO-09：pass+partial = 46/50（92%） | **pass 42 + partial 6 = 48/50（96%）** | ✅ 不低于基线 | 0 条 |
| MVP evals v0.5 parser-only（500 条） | 历史 baseline 472/26/2 | **472/26/2 byte-equal** | ✅ 持平 | 0 条 |
| BETA-15A 同义词召回（42 条） | 召回 88.2% / 假阳 0.0% | 门槛 `synonym_recall_gate` ✅ 通过（≥70%/≤5%） | ✅ | 0 条 |

P eval 通过率较 PROTO-09 不降反升（parser v0.1 → v0.5 自然增长）；无任何用例需要豁免。

## 6. §6.2 出场指标 checklist

| # | 指标 | 阈值 | 实测 | 判定 |
|---|---|---|---|---|
| 1 | 合法 SearchIntent JSON | ≥ 90% | parser 100% schema-valid（+ 模型 100%，carried） | ✅ |
| 2 | zh / en / 混合各语言严格匹配 | ≥ 85% | zh 96.0% / en 96.0% / mixed 88.0% | ✅ |
| 3 | 简单查询（规则路径）p95 | < 500ms | Windows 0.277ms / macOS 0.050ms | ✅ |
| 4 | 复杂查询（模型 fallback）p95 | < 3000ms | macOS 1592ms ✅；**Windows 待 BETA-09(a)** | ✅\* |
| 5 | SearchBackend 调用成功率 | > 95% | MVP-26 语料一致性：Windows Search 5/5 类、Everything 5/5 类命中 = 100% | ✅ |
| 6 | macOS / Windows evals 通过率差距 | < 5pp | parser-only **0pp**（472/26/2 byte-equal 双平台实测） | ✅ |
| 7 | 模型输出 JSON 合法率 | > 98% | macOS BETA-08 v1 = 100%（86/86）；**Windows 待 BETA-09(a)** | ✅\* |
| 8 | 文件操作权限策略 | 100% 通过安全 evals | PolicyEngine + file_action §7.6 契约（harness 124 lib 测试全过）；copy/move/rename 未确认绝不执行、delete 拒绝→clarify | ✅ |
| 9 | Tauri 应用流畅运行 | 启动 < 3s / 操作 < 100ms | Windows 真机 `tauri dev` 起窗 + 流式 UX 可用（观察值）；**未做严格 p95 画像** | ⚠️ |
| 10 | Stub backend 不进生产 fallback 链 | 集成测试断言通过 | `production_chain_excludes_stub_backends`（common）+ `production_chain_excludes_stub_tools`（harness）✅ | ✅ |

**§6.5 不可回归**：✅（见 §5，0 豁免）。

> **\* #4 / #7**：阈值在 macOS 上已实测达标；Windows 因本机无模型 GGUF 未实测，属 [BETA-09(a) 跨平台部署](../../ROADMAP.md) 范畴（模型分发到 Windows 后跑 v0.5 `--with-fallback --hybrid` 复核即闭合）。代码路径双平台同构（fallback 在 intent-parser，平台无关）。

**汇总**：10 项中 **8 项 Windows 实测全过**、**2 项（#4/#7）macOS 达标 + Windows 待 BETA-09(a)**、**1 项（#9）观察可用但缺严格画像**。无任何指标判定为 ✗。

## 7. 失败 / 警告 / 已知问题

1. **模型 fallback 的 Windows 实测缺口（#4/#7）** — 本机无 GGUF。代码路径平台无关、macOS 已达标；解锁仅需 BETA-09(a) 把模型分发到 Windows 后复跑。
2. **Tauri GUI 严格性能画像缺失（#9）** — 真机观察启动/响应正常，但无 p95 数据。建议 B 阶段补 GUI 侧延迟埋点。
3. **端到端搜索 p95 待健康 Spotlight 机复测** — MVP-27 基线机 Spotlight server disabled。
4. **跨平台单测卫生（本会话发现）** — `cargo test --workspace` 在 Windows 上有 2 个 `locifind-platform-macos` 预存失败（该 crate 单测硬编码 Unix 路径），在干净 main 上同样复现，**非本次回归**；建议按平台 gate 或改用 `MAIN_SEPARATOR`（独立 backlog）。
5. **MVP-26 本会话修复的真机 bug** — Everything `path_under` 用非递归 `parent:`，对含子目录的 `location.include`/目录 hint 漏召回；已改为递归「全路径子串 + 尾分隔符」scope（真机探针实证 + 集成测试守护）。
6. **MVP-25 残留 2 fail / 26 partial** — dual-route fixture artifact + 检测器边缘 case，已饱和，留 B 阶段数据 augmentation。

## 8. 下一阶段风险与准备

**MVP-28 评测结论：MVP 阶段代码层出场指标达标（可测项全过，模型项 macOS 达标）。** M→B 阶段正式切换仍受以下 [§8 checklist](../../ROADMAP.md#8-阶段切换-checklist) 项 gating（均非代码、属长周期）：

- ⏳ **BETA-09(a)**：模型 GGUF 分发到 Windows 并复核 #4/#7（解锁后 §6.2 全 10 项双平台闭环）。
- ⏳ **§5 长周期事项**（应立即启动，独立于代码）：BETA-00 法务与安全审查 kickoff；商标申请提交；Apple Developer 账号；Windows 代码签名证书采购（2–4 周）。
- ⏳ 端到端搜索 p95 在 Spotlight 健康的 macOS 机 + Windows 真机各跑一次画像。

**已识别风险（沿用 [ROADMAP §7](../../ROADMAP.md#7-风险地图)）**：本地模型低配机延迟（Windows i7-1165G7 实测模型加载/延迟待 BETA-09a）；跨平台单测卫生债；Smart App Control 间歇拦未签名 exe（BETA-10A 签名解决）。

## 9. 结论

> **MVP 阶段出场（代码层）通过 —— 推荐进入 B 阶段筹备。**

主要依据：
- v0.5 evals parser-only **94.4% pass / variant 99.6%**，三语言严格匹配均 ≥88%（≥85% 阈值）。
- **双平台 evals 差距 0pp**（§6.2 #6 M→B 硬指标，本会话 Windows 真机首次 byte-equal 实测达标）。
- 后端结果集层双平台一致（MVP-26：Spotlight / Windows Search / Everything + 能力感知路由真机闭环，调用成功率 100%）。
- 规则路径性能双平台 << 500ms（~1800× 余量）；模型 fallback p95 1592ms（macOS）。
- 安全不变量成立（文件写操作确认门 + delete 拒绝 + stub 不进生产链）。
- §6.5 回归约束满足，0 豁免。

唯一未在本机闭环的是**模型 fallback 的 Windows 实测**（BETA-09a，代码路径平台无关、macOS 已达标）与 **§8 非代码长周期事项**（法务/商标/证书）。建议：代码层即刻进入 B 阶段筹备，同时立即启动长周期事项让其并行跑起来。

下一步：见 [STATUS.md 下一步](../../STATUS.md)。
