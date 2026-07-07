# LociFind 项目 Roadmap

> 状态：**v1.0**（已采纳 Codex + Gemini 双轨审阅意见）。
> 本文件是项目的**长期任务地图**。
> 审阅记录：[Codex](./docs/reviews/2026-05-25-roadmap-codex.md) + [Gemini](./docs/reviews/2026-05-25-roadmap-gemini.md)；v0.1 → v1.0 修订摘要见本文件最后一节。

## 0. 本文件的位置

| 文档 | 内容 | 节奏 | 更新者 |
|---|---|---|---|
| [PROJECT.md](./PROJECT.md) | 项目目标、定位、原则、架构 | 月级 | 用户主导 |
| **[ROADMAP.md](./ROADMAP.md)（本文件）** | **任务级长期地图：4 阶段任务清单、依赖、估时、验收、里程碑、风险** | **周级** | **任一工具，需经评审** |
| [STATUS.md](./STATUS.md) | 当前阶段、当前 task ID、下一步、会话日志 | 每次会话 | 收工时由当前工具更新 |
| [CONVENTIONS.md](./CONVENTIONS.md) | 协作规则、收工流程、编码规范 | 月级 | 用户主导 |
| [docs/](./docs/) | 详细产品/技术/IP/风险计划 + 设计文档 + 审阅记录 | 按需 | 谨慎修改 |

**任何工具新会话开始时**，按顺序读：PROJECT.md → STATUS.md → ROADMAP.md → CONVENTIONS.md。这 4 份在一起回答："这是什么项目 / 做到哪了 / 接下来要做什么 / 怎么协作"。

## 1. Task 卡片字段约定

```
ID         独立稳定标识，如 PROTO-05 / MVP-12 / BETA-03 / V10-08
状态       not_started | in_progress | done | blocked | dropped
模块       源码 / 文档路径（packages/xxx, docs/xxx, apps/xxx）
依赖       其他 task ID 列表（DAG，无循环）
估时       原型期：hours/days；MVP 期：days；Beta/1.0：days/weeks
验收       客观可观察的判定标准（不能是"做完了"）
负责工具   可选；若某 task 明显适合某工具（如 Mac LoRA → 任何能跑 MLX 的工具）
```

ID 命名：`PROTO-NN` / `MVP-NN` / `BETA-NN` / `V10-NN`，两位数序号，预留间隔便于插入。原审阅期间补充的 task 用字母后缀（如 `PROTO-04A` / `MVP-07A`），表示"插在某 task 之后但不打破已有编号"。

**并行约束**（Codex 审阅 should-have #11 落地）：

每个子阶段在表后用单独小节列出"哪些 task 可并行 / 哪些有隐藏依赖"。task 表的"依赖"列只记直接技术依赖；间接 UX/逻辑依赖在并行约束小节说明。

## 2. 阶段总览

| 阶段 | 时长 | 当前状态 | 入场条件 | 出场条件（硬指标） | 演示价值 |
|---|---|---|---|---|---|
| **P：技术原型** | 1-2 周（7-10 工作日） | ✅ **已完成**（11/11 task） | Schema v1.0 + Trait v0.1 已采纳审阅 | macOS CLI 跑通 schema §7.1-§7.4 共 30 条用例，端到端准确率 ≥ 80%，简单查询 < 500ms | 内部技术验证（**variant 命中 92%、CLI 4ms ≪ 500ms**） |
| **M：MVP** | 3-5 周 | 🔄 **进行中**（M1 12/12 ✅、M2 3/3 ✅、M3 4/4 ✅、M4 7/7 ✅、M5 2/4） | P 出场条件全达成 | 双平台 Tauri 应用；三套 SearchBackend；500 条 evals ≥ 85%；双平台 evals 通过率差 < 5% | 早期用户内测 |
| **B：Beta** | 8-12 周 | 🔄 已开工（BETA-08/09/17 模型侧 + **BETA-01/01A/02 本地索引 + BETA-03 图片 OCR + BETA-04 多源融合 + BETA-05 Ranker + BETA-06 Audit + BETA-07 索引调度 done 2026-06-02，整栈真机验证通过**；正式切换仍待 §8 非代码长周期项） | M 出场条件全达成 | 多源索引可用；1000 条 evals ≥ 90%；macOS DMG + Windows 安装包经 GitHub Releases 可外发（未签名安装口径文档化 + 包管理器渠道，2026-07-04 开源免费拍板） | 公开测试用户 + 企业冷归档场景验证（B7 并行子线，不进出场指标） |
| **V：1.0** | 4-6 月 | 未开始 | B 出场条件全达成 | 插件系统、本地活动洞察、隐私 UI、自动更新、崩溃恢复全部就绪；LICENSE / Third-party Notices / 隐私说明齐全（商标注册已随 2026-07-04 开源免费拍板取消） | 正式发布 |

详细出场指标见 §6；并行启动的长周期事项见 §5。

## 3. 任务清单

### 3.1 P 阶段：技术原型（macOS 优先）

目标：**在 macOS 上跑通"自然语言 → SearchIntent → mdfind → 结果"的最小闭环**。

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 | 验收 |
|---|---|---|---|---|---|---|
| **PROTO-00** | 设计阶段（Schema v1.0 + Trait v0.1 + Codex 审阅 + ROADMAP v1.0） | done | docs/ | — | 完成于 2026-05-25 | 见 [STATUS.md 审阅记录](./STATUS.md) |
| **PROTO-01** | Rust workspace 骨架 + lint gate | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-01)） | — | 0.5d | ✅ Rust 1.95 stable；workspace 含 locifind-search-backend + locifind-intent-parser 两个占位 crate；rust-toolchain.toml + rustfmt.toml pin 版本；scripts/ci.sh 含 fmt + clippy(-D warnings) + build + test，全套通过 |
| **PROTO-02** | SearchIntent serde 类型 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-02)） | PROTO-01 | 1d | ✅ 全部类型落地（5 个 intent 变体 + 9 个公共类型）；50 条 fixture 用例（schema §7 全部）反序列化通过；round-trip 一致；`deny_unknown_fields` 在 internally-tagged enum 内部 struct 上生效（关键边界条件验证） |
| **PROTO-03** | JSON Schema 文件落地 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-03)） | PROTO-01 | 0.5d | ✅ `docs/schema/search-intent.v1.json` 已从 schema md §5 抽出；common crate 增加 schema/serde 交叉测试，50 条 fixture 全过，非法样本覆盖 limit、未知字段、file_action 条件、target index、clarify 空问题；`bash scripts/ci.sh` 通过 |
| **PROTO-04** | SearchBackend trait + 公共类型 | done | packages/search-backends/common | PROTO-02 | 0.5d | ✅ trait + SearchResult + SearchError（含 `UnsupportedIntent`）+ BackendKind + ImplementationStatus 已落地；BackendRegistry 生产链剔除 stub 的单元测试通过；`bash scripts/ci.sh` 通过 |
| **PROTO-04A** | macOS location resolver v0.1 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-04a)） | PROTO-02 | 0.5d | ✅ `LocationResolver` trait 已落地；macOS resolver 支持 `下载/桌面/文稿/图片/影片/音乐/截屏` hint；截屏目录优先读取 `com.apple.screencapture`，失败 fallback 到 `~/Desktop` 与 `~/Pictures/Screenshots`；`bash scripts/ci.sh` 通过 |
| **PROTO-05** | SpotlightBackend 首版 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-05)） | PROTO-04, PROTO-04A, PROTO-05A | 3d | ✅ `SpotlightBackend` 已实现 `SearchBackend`；`mdfind` 结构化参数调用、超时 kill、结果端 exclude 过滤、基础 metadata 补全已落地；fixture #1-#30 查询翻译测试与 shell 注入防护单元测试通过；`bash scripts/ci.sh` 通过 |
| **PROTO-05A** | 合成测试 fixture | done | tests/fixtures（或 packages/evals/fixtures） | PROTO-01 | 1d | ✅ 幂等 Rust 生成器落地；覆盖 18+ 类合成文件；Spotlight 索引验证通过；reindex.sh 就绪 |
| **PROTO-06** | 规则解析器 v0.1（规则解析，不含模型 fallback） | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-06)） | PROTO-02 | 3d | ✅ v0.1 实现 5 路径（file_search / media_search / file_action / refine / clarify）；50 条 fixture 按 variant 命中 46/50 = 92%（≥ 80%）；§3.5 Clarify 触发规则覆盖 unsafe_action / ambiguous_time / ambiguous_location / ambiguous_action；不调用 LLM；字段级精确匹配留到 PROTO-08 evals 硬判定 |
| **PROTO-07** | 顶层 CLI binary | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-07)） | PROTO-05, PROTO-06 | 1d | ✅ `locifind-cli "查找昨天编辑过的 ppt"` 可端到端调用 SpotlightBackend；`--json` / `--intent-only` / `--onlyin` / `--help` 已落地；退出码 0/1/2/3/4 区分成功、有无结果、clarify、未支持 intent、backend/系统错误；`bash scripts/ci.sh` 通过 |
| **PROTO-08** | evals v0.1（50 条用例落库） | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-08)） | PROTO-02, PROTO-06, PROTO-05A | 1d | ✅ `cargo run -p locifind-evals --bin evals` 输出 Pass/Partial/Fail 报告（总览 + variant 分桶 + language 分桶）；`--case N` / `--json` / `--only-failures` 支持；fixture 路径走 PROTO-05A；当前评测：variant 命中 92%（46/50），字段级精确匹配 42%（21/50）；`bash scripts/ci.sh` 通过 |
| **PROTO-09** | 原型出场评测 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#proto-09)） | PROTO-05, PROTO-06, PROTO-07, PROTO-08 | 0.5d | ✅ `docs/reviews/proto-exit.md` 落库：variant 命中 46/50 = 92%（≥ 80%）；CLI 端到端 release 4ms（远低于 500ms p95 阈值）；6/8 §6.1 指标 ✅ + 1/8 ⚠️（端到端 mdfind 留 M0 demo）；结论：**P 出场通过，推荐进 M 阶段** |

**P 阶段任务依赖图（v1.0 更新）**

```
PROTO-01 ─┬→ PROTO-02 ─┬→ PROTO-04 → PROTO-05 ──┬→ PROTO-07 → PROTO-09
          │            ├→ PROTO-04A ────────────┘             ↑
          │            ├→ PROTO-06 ──────────────────────────┤
          │            └→ PROTO-08 ──────────────────────────┘
          ├→ PROTO-03（与 PROTO-02 并行）
          └→ PROTO-05A（与 PROTO-02/03/04/04A/06 全并行）
```

**关键路径**：PROTO-01 → PROTO-02 → PROTO-04 → PROTO-05 → PROTO-07 → PROTO-09，约 7-8 天。
**并行机会**：PROTO-02 / PROTO-03 并行；PROTO-05A 与多数任务并行；PROTO-06 与 PROTO-04+05 并行；PROTO-08 与 PROTO-06 并行。
**预期总工期**：8-10 工作日（按 1 人/单工具串行；多工具协作可压缩到 6-7 天）。

> v0.1 关键路径估为 6 天，v1.0 修订为 7-8 天（Codex 审阅指出，补齐 fixture 与 resolver 后更接近真实工期）。

### 3.2 M 阶段：MVP（macOS + Windows 双平台）

目标：**同一份 Tauri 应用在 macOS 与 Windows 上跑通；三套 SearchBackend；基础 Harness；本地小模型**。

#### M1 子阶段：Harness 基础设施

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| MVP-01 | Tool Registry | done | packages/harness | PROTO-09 | 1d |
| MVP-02 | Schema Validator service | done | packages/harness | MVP-01 | 0.5d |
| MVP-03 | Policy Engine（权限分级 L0–L5） | done | packages/harness | MVP-01 | 1.5d |
| MVP-04 | Tool Loop Controller（最大步数 / 超时 / 取消） | done | packages/harness | MVP-01 | 1d |
| MVP-05 | Intent Router | done | packages/harness | MVP-01 | 1d |
| MVP-06 | Context Memory（多轮 / target_ref / refine 合并） | done | packages/harness | MVP-01 | 2d |
| MVP-07 | Streaming 抽象 | done | packages/harness | MVP-04 | 1d |
| **MVP-07A** | **SearchBackend v0.2：async + streaming 迁移** | done | packages/search-backends/{common,spotlight,windows-search,everything} | MVP-04, MVP-07, MVP-11, MVP-12 | 3d |
| MVP-08 | Tracing / Hooks | done | packages/harness | MVP-01 | 1d |
| MVP-09 | Capability Discovery | done | packages/harness | MVP-01 | 1d |
| MVP-10 | Fallback Chain | done | packages/harness | MVP-09 | 1d |
| **MVP-10A** | **FileActionTool（open/locate/copy/move/rename，delete 禁用）** | done | packages/harness + platform/{macos,windows} | MVP-03, MVP-06 | 2d |

**MVP-07A 验收**：三个真实 backend（Spotlight / WindowsSearch / Everything）暴露统一 `async fn search(...) -> impl Stream<Item = Result<SearchResult, _>>`；支持取消信号；CLI 与 UI 都能消费；旧同步接口经显式 sunset 删除或保留兼容层（二选一并文档化）。

**MVP-10A 验收**：`open` / `locate` 在 macOS / Windows 上可用；`copy` / `move` / `rename` 经 Policy Engine 确认后执行；`delete` 在 schema 层与 Policy 层双重禁用；target_ref 越界、批量阈值（默认 10）、路径冲突、跨卷 move 均有测试；schema §7.6 用例 #36-#40 端到端通过。

**M1 验收**：refine 合并语义（schema §3.4）单元测试 ≥ 95% 覆盖率；Context Memory 对 schema §7.5、§7.8 #43 / #45 / #46 用例契约测试通过。

#### M2 子阶段：Windows 平移

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| MVP-11 | WindowsSearchBackend（OLE DB / SystemIndex SQL） | done（2026-05-31，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#mvp-11)） | packages/search-backends/windows-search | PROTO-04 | 3d |
| MVP-12 | EverythingBackend（ES CLI 优先） | done（**执行层 Windows 11 真机实测 2026-05-31**：装 ES CLI（winget voidtools.Everything.Cli），EsCliExecutor spawn es.exe 端到端；真机修 1 bug——误加的 `-path` 把搜索项当路径吞掉致 0 结果，已移除） | packages/search-backends/everything | PROTO-04 | 2d |
| MVP-13 | 跨平台 location resolver（Known Folders / Spotlight 截屏配置） | done（platform/windows 原 windows-rs `SHGetKnownFolderPath` 从未在 Windows 编译过——撞 `unsafe_code=forbid` + API 误用；2026-05-31 改用 dirs crate，零 unsafe，首次 Windows 编译通过） | platform/{macos,windows} | MVP-09 | 2d |

**M2 验收**：30 条 P 阶段用例在 Windows 上端到端跑通；检测到 Everything 时自动切换到 EverythingBackend；macOS / Windows evals 结果差距 < 5%。

#### M3 子阶段：本地模型

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| MVP-14 | llama.cpp 集成 | done（llama-cpp-4 + candle + stub 多后端，stub 默认） | packages/model-runtime | PROTO-09 | 2d |
| MVP-15 | 模型常驻进程 | done | packages/model-runtime | MVP-14 | 1d |
| MVP-16 | Prompt 设计 + 5-10 few-shot | done | packages/intent-parser | MVP-14 | 2d |
| MVP-17 | 模型 fallback（规则解析不足时调用） | done（端到端 wiring + GBNF 基础设施 + v0.3 hybrid 架构全部落地；1.5B 模型在 v0.5 evals 上无净收益，质量提升归 BETA-08 LoRA） | packages/intent-parser | MVP-16 | 1d |

**M3 验收**：本地 Qwen2.5-1.5B Q4_K_M 推理首次加载 < 10s（实测 6.5s ✅），常驻态查询 < 3s（实测 warm 1.6s p95 ✅）；模型输出 JSON 合法率 > 98%（v0.2 实测 90.4%、v0.3 hybrid 58.3% 曾未达 → **BETA-08 v1 LoRA 已闭合：fallback valid_intent 比 8.3% → 100%（86/86），≥ 98% 达标 ✅**，参见 [beta-08-lora-v1.md](./docs/reviews/beta-08-lora-v1.md) / [mvp-17-fallback-evals.md](./docs/reviews/mvp-17-fallback-evals.md)）。

#### M4 子阶段：桌面应用

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| MVP-18 | Tauri 2 应用骨架（macOS + Windows） | done | apps/desktop | MVP-01, MVP-11 | 2d |
| MVP-19 | 搜索框 UI + 流式结果列表 | done（2026-05-28，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#mvp-19)） | apps/desktop | MVP-18, MVP-07 | 3d |
| MVP-20 | 全局快捷键（macOS ⌥Space / Windows Ctrl+Space） | done | apps/desktop | MVP-18 | 1d |
| MVP-21 | 后端状态指示 + 错误降级提示 | done | apps/desktop | MVP-10 | 1d |
| MVP-22 | 设置页 + 隐私管理页 | done | apps/desktop | MVP-18 | 2d |
| MVP-23 | macOS Full Disk Access 引导 | done | platform/macos | MVP-22 | 1d |
| MVP-24 | Windows 索引位置加入引导 | done（macOS stub，Windows 真检测 待真机） | platform/windows | MVP-22 | 1d |

#### M5 子阶段：MVP 出场

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| MVP-25 | evals 扩到 500 条 | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#mvp-25)） | packages/evals | PROTO-08, MVP-17 | 3d |
| MVP-26 | 跨平台一致性测试 | done（2026-06-01，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#mvp-26)） | tests/ | MVP-11, MVP-13, MVP-25 | 1d |
| MVP-27 | 性能测试（响应时间分布） | done（parser 50us / CLI 7ms / 搜索 20ms 但 Spotlight server disabled 需复测） | tests/ | MVP-19 | 1d |
| MVP-28 | MVP 出场评测 | done（2026-06-01，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#mvp-28)） | — | MVP-25, MVP-26, MVP-27 | 1d |

**M 阶段预期总工期**：3-5 周（按多工具协作）。

**M 阶段并行约束**（Codex 审阅 should-have #11 落地）：

- **可早启**：MVP-18 桌面骨架（独立于 backend）；M1 与 M2 / M3 之间无强依赖。
- **隐藏依赖（task 表"依赖"列未必体现）**：
  - **MVP-19 流式结果列表**：除 MVP-18 / MVP-07 外，**实际依赖 MVP-07A async/streaming 迁移完成** + 至少一个 stream-ready backend（推荐 SpotlightBackend 先升）。
  - **MVP-21 后端状态指示**：依赖 MVP-09 Capability Discovery + MVP-10 Fallback Chain。
  - **MVP-22 设置 / 隐私页**：可与 backend 并行开发，**但 Full Disk Access 引导（MVP-23）依赖平台权限检测能力**（属 MVP-09 的一部分）。
  - **文件操作 UI**（在 MVP-19 内）：依赖 MVP-10A FileActionTool。
- **强串行**：MVP-26 跨平台一致性测试必须等 MVP-11 / MVP-13 / MVP-25 全部 done。

### 3.3 B 阶段：Beta

目标：**多源本地索引可用；开源安装包可外发**（2026-07-04 开源免费拍板：签名分发口径作废）。

#### B0：开源发布审查（原「法务与安全审查」，2026-07-04 开源免费拍板 re-scoped）

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| **BETA-00** | **开源发布审查（LICENSE / Third-party Notices / 隐私说明 / 商标使用规范 / 仓库脱敏）** | **done（2026-07-04）**：LICENSE 双许可 ✅ + Everything 条款核查 ✅（不构成再分发，[third-party-licenses.md](./docs/third-party-licenses.md)）+ PRIVACY.md ✅ + **仓库脱敏 ✅**（语料全合成核实 / 图片无 EXIF / 密钥工作区+924 commits 历史双零命中 / 个人用户名 148 处等长替换清零、受改 crate 测试全绿，报告 [beta-00-repo-sanitization-2026-07-04.md](./docs/reviews/beta-00-repo-sanitization-2026-07-04.md)）。**仓库已转公开（2026-07-04 同日，orphan 首发）**：公开 `raoliaoyuan/LociFind` 自单 commit `bc47473` 起步、私有 `LociFind-archive` 冻结保全史（报告 §4 执行记录）——BETA-00 无开放项 | docs/ | MVP-28 | 完成于 2026-07-04 |

**BETA-00 验收**（2026-07-04 开源口径重写，原律师签字版 EULA / Privacy Policy 要求作废）：LICENSE-MIT + LICENSE-APACHE 入库 ✅；`docs/third-party-licenses.md` 与实际依赖差异为 0；**Everything 再分发条款核查 ✅**（2026-07-04：现状即「检测用户自装」——运行期 spawn `es.exe`、零 voidtools 二进制入库，不构成再分发；voidtools License 本身 MIT 风格宽松，核查记录见 [third-party-licenses.md](./docs/third-party-licenses.md)）；**隐私说明入库 ✅**（[PRIVACY.md](./PRIVACY.md)，2026-07-04：无遥测 / 唯一联网点 = 用户触发的模型下载 / 落盘清单 / 卸载清理路径 / daemon 形态数据边界）；Apple/Microsoft/voidtools 商标使用位置经审查（不暗示背书，[IP §7.3](./docs/LociFind知识产权保护计划书.md) 该节仍有效）；**公开仓库前脱敏**——BETA-44 真实语料、个人路径、git 历史敏感信息核查。**此 task 是 BETA-10 / BETA-10A 分发的前置**。

#### B1：本地索引

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-01 | 音乐 metadata 索引（artist/title/album/duration/format） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-01)） | packages/indexer | MVP-28 | 5d |
| BETA-02 | Office/PDF 内容索引（FTS5） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-02)） | packages/indexer | MVP-28 | 7d |
| **BETA-01A** | **全盘音频索引（发现层全盘枚举，超越固定 Music 目录）** | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-01a)） | packages/indexer + packages/search-backends/local-index | BETA-01, MVP-12 | 3-4d |
| BETA-03 | OCR（macOS Vision / Windows.Media.Ocr / Tesseract 兜底） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-03)） | packages/indexer + packages/search-backends/local-index + apps/desktop | MVP-28 | 7d |
| **BETA-28** | **索引预算与分层新鲜度（按目录热度分档：light metadata / FTS / embedding / OCR；避免对 cold long-tail 全量预处理）** | not_started（2026-06-25 登记，源于 Claude × Codex 联合规划，借鉴 [LLM Wiki 文章](https://axk51013.medium.com/rethinking-agent-harness-part4-llm-wiki-%E5%8F%96%E4%BB%A3-rag-041629319804) Data 维度反例：cold content 反伤）。**价值主张**：把"对所有文件做所有事"换成「按 root 热度分档做多深」——默认 light（仅 metadata + 文件名 FTS）、热区 full（embedding + OCR）、按需 promote；用户可见档位 + 系统自适应。**衔接**：BETA-07 后台索引调度（加 per-root budget 字段）、BETA-15B 语义索引（embedding 入 full 档）、BETA-03 OCR（OCR 入 full 档）、设置页索引目录（per-root 档位 UI）、BETA-27 索引目录配置（per-root 档位继承根设置）。**主要风险**：漏召回（用户搜了 light 档目录下的内容词）；缓解 = 失败时自动 promote 该 root 到 full 档 + 通知用户 + BETA-30 失败样本箱记录信号。 | packages/indexer + packages/search-backends/local-index + apps/desktop | BETA-07, BETA-15B-1, BETA-27 | 1-2 weeks |
| **BETA-42** | **【bug】trigram FTS 2 字 CJK 关键词 AND 组合必然 0 命中（"判决 违约金"类查询误报「错误：未找到结果」）** | done（2026-07-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-42)） | packages/indexer + packages/intent-parser + packages/search-backends/local-index | — | 0.5d（含单测） |

> **BETA-01A 背景**（spike `spike-disk-wide-audio` 2026-06-02 真机验证）：现状 `reindex` 仅扫固定 `dirs::audio_dir()`，用户音频散落 OneDrive/下载等处时扫到 0 条。方案 = **发现/提取/存储三层拆分**：发现用 Everything `es.exe ext:`（Win，307ms 枚举全盘 1249 文件）/ Spotlight `mdfind`（macOS），提取仍用 lofty，存储复用 `MusicIndex`（spike 已加 `index_paths(&[PathBuf])`）。**搜索跨目录命中已验证 ✅**。
>
> **BETA-01A 三处必做设计**（spike 实测暴露）：① **跳过"仅在线"占位符**（查 `FILE_ATTRIBUTE_OFFLINE`/`RECALL_ON_DATA_ACCESS`）——实测 302/1249=24% OneDrive 占位符 `os error 395` 读取被拒，且会触发水合下载；占位符只存文件名不读标签；② **并行提取**（rayon）——单线程实测 244ms/文件 × 上千 = 5 分钟；③ **file_name 进 FTS**——实测标签覆盖仅 ~21%（多为教学/音效 mp3），按文件名搜需命中本地索引。思路可推广至 BETA-02 文档全盘索引。
>
> **BETA-01A 验收**：reindex 覆盖全盘音频（非仅 Music 目录）；仅在线占位符跳过、不触发下载、计入 skipped 统计；跨目录 artist/文件名查询真机命中；并行提取 p50 较单线程显著下降；fmt/clippy/全 workspace test 零回归。

#### B2：多源融合

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-04 | Result Normalizer（+ 可选"最近使用文件"快捷通道） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-04)） | packages/result-normalizer + packages/search-backends/local-index | BETA-01, BETA-02, MVP-10A | 3d |
| BETA-05 | Ranker（多源合并、BM25 + 启发式） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-05)） | packages/ranker | BETA-04 | 5d |
| BETA-06 | Audit Log | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-06)） | packages/harness | MVP-10A | 2d |
| BETA-07 | Index Scheduler（后台低优先级） | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-07)） | packages/indexer + apps/desktop | BETA-01, BETA-02 | 3d |

> **BETA-04 注**（Gemini 审阅 nice-to-have #4 落地）：如 Beta 进度领先，可在 Result Normalizer 中加入"最近使用文件"快捷通道（基于系统 Recent Items / Jump Lists），作为本地活动洞察（V10-02）的早期预览，**但不包含主题摘要等隐私敏感分析**。该预览功能默认关闭，需用户主动开启。

#### B3：模型升级

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-08 | LoRA 数据集生成 + MLX 微调 | done（2026-05-28，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-08)） | training/ + packages/evals/src/bin/build_lora_dataset.rs | MVP-25 | 2 weeks |
| BETA-09 | 模型量化与跨平台部署 | done（2026-06-01，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-09)） | training/ + packages/model-runtime + packages/evals | BETA-08 | 1 week |
| **BETA-17** | **基座模型选型实验（bake-off：Qwen2.5-1.5B 基线 vs Qwen3-0.6B vs Qwen3-1.7B）** | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-17)） | training/ + packages/model-runtime + packages/evals | BETA-08 | 3-5d |

**BETA-17 验收**：
- **候选**：Qwen2.5-1.5B（v1 基线）/ Qwen3-0.6B / Qwen3-1.7B，均 Apache 2.0，统一 Q4_K_M。
- **同配方训练**：三者用相同 LoRA 配方（mask-prompt + nonempty oversample 8×，超参对齐 v1）在 v0.5-patch 数据集上各训一份 adapter，复用 `training/mlx-lora/scripts/` 管线。
- **统一评测**：各跑 v0.5 evals `--with-fallback --hybrid` 全 500 case，对比 **pass / partial / fail / rescued_to_pass / regressed / 字段精确匹配 / p50·p95 fallback 延迟 / 常驻内存**。
- **Qwen3 关键约束**：dual-mode 必须 **non-thinking 模式**（`enable_thinking=False`）——本任务要快速产结构化 JSON 补丁，thinking 会暴涨延迟、是负担。spec 期确认 llama.cpp（当前 llama-cpp-sys-4 0.3.0）+ mlx-lm（0.29.1）对 Qwen3 架构的支持（各跑一次最小推理验证）。
- **判定**：若 Qwen3-0.6B 保住 v1 的 pass 数（净降 ≤2）→ 它是弱硬件场景更优解（更快更小），建议设为弱硬件默认；若 0.6B 掉质量 → Qwen3-1.7B 作"新一代质量更稳"升级。延迟需在目标弱硬件（如本次 Intel 核显笔记本，CPU 后端）实测对比。
- **关联**：与 BETA-09「弱硬件上模型 fallback 可选/降级 + 能力感知路由」联动——bake-off 结论直接决定弱硬件默认是"小模型"还是"纯 parser"。
- **2026-06-01 实测判定结果**：**分支①命中** —— Qwen3-0.6B 准确率逐项对等且更小更快，已推荐为弱硬件默认。**后续 backlog**：(a) **Windows 弱硬件延迟复核**——winner GGUF 传 Windows 走 BETA-09a Vulkan 流程实测 p50/p95，判定 3000ms 绝对达标；(b) **winner 接成默认推理基座 wiring**（改 `LOCIFIND_MODEL_PATH` 默认 / 能力感知选模型 + 过 evals 回归门）——本会话刻意不做（spec out of scope）；(c) **>1B 候选（Qwen3-1.7B / Qwen3.5-2B）按需补测**；(d) Qwen3.5 小尺寸为多模态 VLM，<1B 段未找到纯文本 Qwen3.5，如需新一代再追踪官方纯文本小 dense 发布。

#### B3.5：搜索召回增强（同义词族）

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| **BETA-11** | **同义词关键词扩展（手维护 YAML 词典）** | **dropped（2026-07-04 盘点：与 BETA-15 重复登记，工作已在 BETA-15 done 2026-05-30 全部交付**——`SynonymExpander`/`YamlSynonymExpander`/词典/desktop 接线/Spotlight `search_expanded` 均在仓，本行验收即 BETA-15 验收） | packages/harness + resources/synonyms + packages/search-backends/spotlight + apps/desktop | MVP-28 | ~~2d~~ 0（重复卡） |
| **BETA-11A** | **同义词召回评测集（独立 30~50 case fixture + query + 期望命中）** | **dropped（2026-07-04 盘点：= BETA-15A done 2026-05-30**；`fixtures/synonym-recall` 42 case + `synonym_recall` binary 在仓） | packages/evals | BETA-11 | ~~2d~~ 0（重复卡） |
| **BETA-11B** | **同义词召回升级（embedding 索引 或 LoRA 在线扩词，二选一）** | **dropped（2026-07-04 盘点：= BETA-15B**——已升格为旗舰「本地语义召回层」，embedding 路径胜出并大部分交付，见 BETA-15B 行） | packages/harness + packages/model-runtime | BETA-11, BETA-11A, BETA-08 | 见 BETA-15B |
| **BETA-11C** | **WindowsSearchBackend / EverythingBackend 覆盖 `search_expanded`** | **dropped（2026-07-04 盘点：= BETA-15C done 2026-05-31**；两后端 `search_expanded` OR 谓词翻译 + 单测在仓实测确认） | packages/search-backends/{windows-search,everything} | BETA-11, MVP-11, MVP-12 | ~~2d~~ 0（重复卡） |
| **BETA-11D** | **用户级持久化同义词库 + 人审闸门（运行态 feedback 学习）** | done（2026-06-13，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-11d)） | packages/harness + apps/desktop + `app_config_dir/user-synonyms.yaml` | BETA-11（共享 SynonymExpander trait，= BETA-15 已 done） | 完成于 2026-06-13 |
| **BETA-18** | **parser 多类型查询支持（"pdf和doc" / "图片和视频" 不再丢类型）** | done（2026-06-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-18)） | packages/search-backends/common + packages/intent-parser + 3 backends | — | 1-2d |
| **BETA-19** | **跨范畴多类型查询均衡展示（「图片和视频」少数派类型不再被碾压不可见）** | done（2026-06-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-19)） | packages/ranker + apps/desktop + packages/search-backends/common + packages/intent-parser | BETA-18 | 1d |

> **注（2026-07-04）**：以下 BETA-11/11A/11B/11C 验收段保留作历史记录——四卡均为 BETA-15/15A/15B/15C 的重复登记（已 dropped，见上表），验收内容已由对应 BETA-15 族卡片交付。

**BETA-11 验收**：spec [docs/superpowers/specs/2026-05-30-synonym-keyword-expansion-design.md](./docs/superpowers/specs/2026-05-30-synonym-keyword-expansion-design.md) §6 全部测试通过 + §7 全部 8 条手测 scenario 通过；`bash scripts/ci.sh` 全过；**v0.5 evals parser-only baseline byte-equal `pass 472 / partial 26 / fail 2`** + **hybrid `pass 480` 不掉**（关键不回归 guard，见 spec §6.2）；harness 新增 `SynonymExpander` trait + `YamlSynonymExpander` + `NoopExpander`；`SearchBackend` trait 加 `search_expanded` default method；SpotlightBackend 覆盖；初始词典 ~60 zh + ~40 en = ~100 组（spec §5）；混合字符 keyword（标识符如 `synthetic-place`）走 NoopExpand 不被误扩；trace `synonym_expand` event 落 JSONL。

**BETA-11A 验收**：独立同义词召回评测集 ≥ 30 case（query → 期望命中合成 fixture）；report 包含召回率 / 假阳率分桶；BETA-11 当前手维护词典在此集召回 ≥ 70%、假阳 ≤ 5%；为 BETA-11B 升级提供定量 baseline。

**BETA-11B 升级时点判定**（spec §10）：手维护词典 > 200 组、或出现可重复的"用户原 case 找不到"反馈、或 BETA-11A 召回率连续两次评测无法继续提升时进 BETA-11B。embedding 路径与 LoRA 在线扩词路径二选一在进入时再评估（要素：评测召回率提升幅度 / 推理延迟 p95 / 模型常驻额外内存 / 跨平台部署复杂度）。

**BETA-11C 验收**：WindowsSearchBackend `search_expanded` 在 SystemIndex SQL 翻成等价 OR 谓词；EverythingBackend `search_expanded` 在 ES 查询语法翻成等价 OR；spec §7 8 条手测在 Windows 真机端到端通过；macOS / Windows 同 query 召回差异 < 5pp（与 §6.2 跨平台一致性 guard 对齐）。

**BETA-11D 验收**：

- **持久化**：用户词典本地落盘（SQLite 或 `~/.locifind/user-synonyms.yaml`，二选一在 spec 期决定），重启 app 不丢；不上传不同步（守 PROJECT.md "本地优先" 原则）；支持导出 / 导入 yaml 半手动跨设备
- **双层叠加**：SynonymExpander 优先查用户词典再查系统词典（BETA-11）；冲突时用户词典覆盖；trace `synonym_expand` event 新增 `source: "user" | "system"` 字段
- **触发 UX**：query 命中数 ≤ 阈值（默认 0）时，UI 弹"扩展搜索?"对话框；候选词由 BETA-11B 在线生成（未做则候选空，用户手输）；用户勾选后立即重查并返回结果；勾选完成后 UI 问"是否记住此映射?"二次确认才沉淀
- **撤销 / 编辑 / 查看**：设置页新增"我的同义词"页，支持查看全部用户词条、编辑 aliases、删除整组、批量导出 / 导入；删除即时生效（不需重启）
- **卸载集成**：BETA-12 卸载流程需清掉用户词典文件（或对应 SQLite 表）
- **隐私**：用户词典不进 trace（默认场景；`LOCIFIND_TRACE` 开启时可见，便于 dev 调试）；Privacy Policy 文案新增"用户同义词库" 一条；不混入任何 telemetry 上报路径
- **反向污染防护**：单次新建词条 `aliases.len() ≤ 8`（与 BETA-11 系统词典 lint 一致）；不允许把 ASCII 标识符（`synthetic-place` 类）建为 head 或 alias（同 BETA-11 §4.2 NoopExpand 规则）；user 词典里同一 head 多次教学时合并 + 去重，不无限增长
- **目标 case 覆盖**：spec §7 手测追加 BETA-11D 场景:用户搜"友商竞争分析"零命中 → 弹候选（LLM 生成或空）→ 用户勾选 `[AWS, Azure, 产品分析, 功能洞察]`+ 确认 → 命中合成 fixture `aws计算产品分析.md` + `Azure功能洞察.md`；重启 app 后再搜同 query → 零延迟直接命中（走 user 词典）

**B3.5 并行约束**：BETA-11 完全独立于 B1 / B2（不依赖索引或多源融合，纯 query 时中间件）；BETA-11A 与 BETA-11 顺序串行（先有能力再衡量）；BETA-11B 强依赖 BETA-11A 有评测 baseline + BETA-08 LoRA 训练管线（若走 LoRA 在线扩词路径）；BETA-11C 与 BETA-11B 可并行（覆盖现有手维护词典的跨后端就够，不必等 embedding 升级）；**BETA-11D 强依赖 BETA-11（共享 trait），可选依赖 BETA-11B（候选生成质量直接影响 BETA-11D 的"教学成本"——LLM 候选好用户勾几下就行，LLM 没接用户得手输每个 alias）；BETA-11D 与 BETA-11C 无依赖可并行**。

**BETA-18 背景与验收**：2026-06-01 BETA-09(a) 会话用户在 Windows 桌面 app 手测「找 pdf和doc文件」只返回 docx——根因 `packages/intent-parser/src/parsers/file_search.rs:89` `match_extensions` 用 `.find()` 只取**第一个**命中的扩展名别名，词典 `EXTENSION_ALIASES` 里 `doc` 排在 `pdf` 前 → pdf 被静默丢弃（`SearchIntent.extensions` 是 `Vec<String>` 本就支持多扩展名，缺口纯在抽取层）。**修复**：扩展名抽取累加所有命中别名 + 合并去重；多类型 file_type 不一致时取 None 只保留 extensions；关键词抽取同步排除所有命中扩展名词；确认后端翻译多扩展名为 OR 谓词。**验收**：v0.5 evals parser-only 不退化（pass≥472）；新增单测覆盖「pdf和doc」「图片和视频」「word 或 ppt」+ 单类型回归；走 spec/plan 流程。

#### B4：分发

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-10 | macOS DMG 开源分发（原「签名 + 公证 + Stapler」，2026-07-04 开源免费拍板 re-scoped） | **in_progress（2026-07-04 文档层 done）**：Gatekeeper 放行文档（含 macOS 15 系统设置新路径 / 右键打开 / xattr）+ ad-hoc 签名说明入 [install.md](./docs/install.md)；Homebrew 评估 done（自建 tap 先行、官方 cask 待知名度，[渠道评估](./docs/reviews/beta-10-distribution-channels-2026-07-04.md)）。**DMG 产物 CI done（2026-07-06）**：[.github/workflows/release-macos.yml](.github/workflows/release-macos.yml) 镜像 release-windows.yml（macos-14 Apple Silicon + aarch64-apple-darwin + 同款 --locked 守门 + model-fallback/semantic-recall features + Gatekeeper 放行 releaseBody），与 windows 并行同吃 v* tag 挂同一 Release；本机仅 YAML 校验，**下次 macOS 发版首验**（Metal 编译/DMG 打包/tauri-action）。**剩余**：真机放行验证（§6.3 指标，随 macOS 发版） | platform/macos + docs | BETA-00, MVP-28 | 剩 0.5d |
| BETA-10A | Windows 安装包开源分发（原「MSIX 签名」，2026-07-04 开源免费拍板 re-scoped；原 BETA-11，2026-05-30 重编号） | **in_progress（2026-07-04 分发链路 done）**：SmartScreen 放行 + SHA256 校验 + 升级/卸载说明入 [install.md](./docs/install.md)、README 安装节、Release body 挂链；**v0.9.14 首个公开 Release 经 GitHub Releases 外发**（release-windows.yml 新仓库跑通、NSIS hook 首次真实构建通过）；**Scoop 渠道上线**（[scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind) bucket）；winget 待 BETA-14 后稳定期（[渠道评估](./docs/reviews/beta-10-distribution-channels-2026-07-04.md)）。**剩余**：真机「下载→放行→安装→可用」+ Scoop 装机路径验证（随下次上机 / cycle 9，§6.3 指标） | platform/windows + docs | BETA-00, MVP-28 | 剩 0.3d（真机） |
| BETA-12 | 卸载流程（删索引 / 模型 / 日志 / 保留配置）。卸载需清 `app_config_dir/user-synonyms.yaml`（BETA-11D 用户同义词库）。 | **done（2026-07-04）**：双层清理同一份清单——① Windows NSIS 卸载 hook（`apps/desktop/src-tauri/nsis/uninstall-hooks.nsh`，`$UpdateMode` 守卫升级不清数据、settings.json 保留）；② 应用内 `uninstall_cleanup` 命令 + 隐私页「卸载清理」（覆盖 macOS 无卸载器 / 便携版；模型句柄 `unload()` 释放 GGUF 占用、索引/嵌入/下载并发守卫、逐项报告）。搜索历史随「日志」口径一并清（查询词属敏感数据）。闸门：uninstall 单测 ×5 + hook 在位/守卫在位校验；NSIS hook 首次真实构建随下次发版 CI，真机验证归 cycle 9（[manual-test-scenarios BETA-12 节](docs/manual-test-scenarios.md)，场景 5「升级零数据损失」为发版阻断）。 | apps/desktop + platform/* | MVP-22, BETA-01-03 | ~~2d~~ 0.5d（AI 全程） |

#### B5：Beta 出场

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-13 | evals 扩到 1000 条 | done（2026-06-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13)） | packages/evals | BETA-04, BETA-05 | 完成于 2026-06-03 |
| BETA-14 | Beta 出场评测 + 内测招募文档 | not_started（**§6「总体 evals」51.4%→…→88.1%（7-04 收束）→92.7%（7-04 三刀）→95.2%（7-04 IX 时间表达簇 + keywords 小刀：before/after 绝对日期 + 年月 + 英文月名 + 区间 + 这周/这个月/新增/做的/最近拍 + created→created_desc 翻转收窄〔仅相对时间+显式创建词，Before/After 保持 modified_desc〕、月份名/序数/数字词停用 + 报告 sole-keep + 又 分隔 + 预算表 compound + 比X还大 size + 图片内容子句整尾短语 + "the word" 框架名词消歧 + 标题词不作时间；标注离群对齐 3 条均 5:1 以上锚点）→**97.7%**（7-04 X 四项口径拍板落地：英文复数归一〔装配终点 + minutes/news/series 例外 + report sole-keep〕、language 降出严格匹配〔judge 跳过，分语言统计不变〕、clarify question 核实既定实现、ext-ft 标注对齐 6 条 + G15 谓词扩展〔in the <kw> / 句首 documents 里 / 位置义 pictures 抑制〕+ 几百KB→<1MB 启发）。**v0.9 = 977/23/0、v0.5 = 490/10/0**，四轮逐 case 全程零回归，parser 新测 ×15 + judge 测 ×1，详[复盘 §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)。BETA-13-G1~G16 + 7-04 全部拍板已落地。→**98.5%**（7-06 clarify options 方案 A：按 reason 挂标准 options〔Unknown 除外〕+ parser/标注双向对齐，8 条 clarify partial 全清、Clarify 桶 67/0/0，Class B 决策清零）→**99.4%**（7-06 同日老账收割 9 条：songs by 小写连字符 artist〔synthetic-artist ×4〕/ 碳中和 compound 占位符保全 / 裸 no+字面扩展名窄路径 / music 目录 mixed hint / 几个G 抽象 size→size_desc / d3 ft 标注对齐 ×2）。**v0.9 = 994/6/0、v0.5 = 495/5/0**，逐 case 零回归，parser 新测 +5+4。剩 6 partial 全为 v0.5 标注锁定项（markdown ft / 下载动词歧义 / 项目归档 location / downloads hint 双语 ×2）+ 备份文件两难，不阻塞出场线。**BETA-14 出场报告骨架 [docs/reviews/beta-exit.md](docs/reviews/beta-exit.md) 已落**（parser-only 全填，真机格标 TODO）。出场判定余双平台基准画像真机复跑 + BETA-10/10A 分发前置** | — | BETA-10, BETA-10A, BETA-12, BETA-13 | 3d |
| BETA-13-G1 | parser 自然语言关键词抽取（名词短语，如「工作汇报/年度预算/marketing plan」）| **done**（零依赖跨度剥离：中文剥信号词取 CJK 段 + 英文短语抽取 + mixed 合并；容器名词整段丢弃）| packages/intent-parser | BETA-13 | 1-2 week |
| BETA-13-G2 | 音乐 metadata 措辞鲁棒性（artist/genre/album/title/duration 多样自然措辞，「周杰伦的歌/找邓紫棋的歌曲」抽不出 artist）| **done**（自由 CJK/EN artist + genre 新字段 + album/title 抽取 + quality high + 时长 less_than/中文数字 + 音频路由）| packages/intent-parser | BETA-13 | 3-5d |
| BETA-13-G3 | 中文类型词→file_type（「表格/幻灯片/演示文稿」等非字面扩展名的类型词映射）| **done**（范畴词 alias 拆分=仅 file_type 不带扩展名；补表格/照片/音频/代码/可执行/slides；多 file_type 按 query 语序）| packages/intent-parser | BETA-13 | 2d |
| BETA-13-G4 | refine 标记词覆盖扩展（「只要/换成/再加上/去掉/不限…了」等自然 refine 措辞，现仅认「只看/排除/清空」）| done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g4)） | packages/intent-parser | BETA-13 | 2-3d |
| BETA-13-G5 | file_action 自然识别（「打开第一个/删除这些」在无显式列表上下文时被误路由 file_search）| **done**（delete 动作 + 定位/显示措辞 + 路径目标 + 指示代词→首个 + 多序数 Indices + 全部；含 env path 的 dest 因机器相关不可参测）| packages/intent-parser | BETA-13 | 2-3d |
| BETA-13-G6 | 显式排序词覆盖（「按名字排序」等被忽略退回默认）| **done**（按名字±倒序 NameAsc/NameDesc + 方向×维度 created/accessed/modified asc/desc）| packages/intent-parser | BETA-13 | 1d |
| BETA-13-G7 | clarify 触发阈值（设计性）| **done**（用户决策：中度阈值 + 精确 5 类 reason；unsafe/action/location/time/type/unknown，有具体约束即不拦，「昨天的 pdf」仍走真搜索）| packages/intent-parser | BETA-13 | 需用户决策 |
| BETA-13-G8 | parser 干净缺口第二轮（截图内容子句路由 + 中/英文类型名词→file_type + artist 自然措辞修缮）| done（2026-06-19，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g8)） | packages/intent-parser | BETA-13 | 完成于 2026-06-19 |
| BETA-13-G9 | parser 近邻 follow-up（size 区间 + 内容截图多关键词）| done（2026-06-19，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g9)） | packages/intent-parser | BETA-13 | 完成于 2026-06-19 |
| BETA-13-G10 | coverage 标注对齐 v0.5 契约 + screenshot parser 微修 | done（2026-06-19，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g10)） | packages/intent-parser, packages/evals | BETA-13 | 完成于 2026-06-19 |

| BETA-13-G11 | 标注规范决策 A/B/C/D 落地（跨范畴多类型路由 + 多类型 ext 规则 + 排序边界 + 无上下文动作安全语义）| done（2026-06-19，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g11)） | packages/intent-parser, packages/evals | BETA-13 | 完成于 2026-06-19 |
| BETA-13-G12 | parser 自然语言剩余缺口 backlog（A/B/C/D 落地暴露，纯 parser 可修、改 coverage 对齐会凑指标故未动）| **done（2026-07-04 收束**：末项 ④ documents/pictures 消歧已由 G15 交付；「剩 2 fail = §1.1 契约冲突」经用户拍板 re-baseline 消化——coverage 改标 MediaSearch（schema 本有 sort/size/location/created_time 可无损表达）+ 顺手修 2 个 media 路径缺口〔`sorted by X` 短语漏进 screenshot keywords → 整短语剥除；`bigger than` 不入 `parse_size` GT 正则〕各带回归单测、v0.5 零暴露。历史 cycle 记录如下）（2026-06-19，**已清干净低风险项 +10 pass**：① **英文复数类型词** `pdfs`/`archives` 入 lexicon（word_present 词边界使 `pdf` 不匹配 `pdfs`）；② **cross-category keyword 残留** `都要`/`我全都要`/`但是不要` 等入停用词；③ **否定/排除多类型** `negation_split` 边界（`images but not videos`/`要图片不要视频`/`…但是不要截图`→`exclude_file_type` + 正向 file_type 只取标记前段）+ 5 条 coverage 单元素数组→标量对齐。**v0.9 807→817 pass、fail 16→15、v0.5 byte-equal 零回归、workspace 775 passed**。**【2026-06-19 续，+5 pass】再清干净小项 4 个**：⑤ **`不含 mkv`→exclude_extensions**（否定段字面扩展名 token，新 helper `negated_literal_extensions`，与类型词→exclude_file_type 区分；**en `no mkv` 推后**——需裸 `no` 标记 byte-equal 风险 + ground-truth 自相矛盾〔en-020 有 audio 扩展名列表而 zh-020 无〕，属 coverage re-baseline）；⑥ `音乐和图片文件都列一下`→加 `都列` 框架词剥离（`文件都列` 残留消除）；⑦ `按 size`/`by size`（英文 size 词）→ file_search decide_sort + refine 双路径 size_desc + `size` 入 EN_STOPWORDS；⑧ **exclude+约束→file_search（决策 C 同构）**：`文档和图片，排除压缩包`→file_search 带 exclude_file_type（refine 加约束门 `is_fresh_positive_then_exclude`：前向 `排除TYPE` + 排除前有正向类型才转 file_search；裸 `排除视频`/尾置 `把 ppt 也排除掉` 仍 refine）+ negation_split 复用 `排除`/`exclude` 标记。**v0.9 817→822 pass、fail 15→14、§6 80.7%→82.2%；v0.5 byte-equal 零回归（500 case 0 diff）、workspace 779 passed、4 条新单测**。**【2026-06-19 续，+4 pass｜标注决策 E/F/G】**用户逐条拍板剩余三条产品语义并落地：**E**（§3.2，改 parser）`has_visual_media_with_abstract_modifier` 加 `has_quantity_degree_modifier`（数量/程度修饰 `几个/些/短/some/a few` + 视觉媒体→media_search）→ `找几个视频`/`短视频`/`some videos` 转 pass；**F**（§3.4，改 parser）`detect_vague_clarify` 加 `bare_relative_time_only`（剥前导动词+尾「的」后精确等于单时间词→clarify(ambiguous_type)）→ `昨天的` 转 pass，`昨天的 pdf`/`昨天的视频` 不误触发；**G**（en-020，改 coverage）删除 en-020 多余 audio 扩展名列表对齐规则 B + zh-020（裸 `no` 标记按决策推后、仍 partial）。**v0.9 822→826 pass、fail 14→10、§6 82.2%→82.6%；v0.5 byte-equal 零回归（500 case 0 diff）、workspace 780 passed、3 条新单测、clippy/fmt 净**。**【2026-06-20 续，+5 pass｜③′ file_action 误路由 done】**`移动到文档文件夹`/`重命名为 终稿`/`把第1、3、5个复制到U盘` 转 file_action（file_action.rs 3 处：① 抽到 destination/new_name 但无显式目标→默认 `last_results Index{1}`，门控避免裸动作词误判；② 多序数 `第1、3、5个`→Indices；③ U盘/优盘/usb→`/Volumes/USB`）+ coverage d6 三条 `Documents` destination `/Users/me/Documents`→`~/Documents` 对齐 v0.5 约定（顺带 zh-020/en-004 partial→pass）。**v0.9 826→831 pass、fail 10→7、partial 164→162、§6 82.6%→83.1%；v0.5 byte-equal 零回归（500 case 0 diff）、workspace 全绿、clippy/fmt 净、5 条新单测**。**【2026-06-20 续，+4 pass｜②′ image+约束 误路由 done】**`创建于上个月的图片`/`桌面上 smaller than 1MB 的图片`/`截图目录里的图片` 转 file_search：① media_search image-only 守护（视觉媒体仅 image 无 video→file，§1.1 决策 image 是 carve-out；coverage 0 条 image→media、v0.5 唯一 image 锚点即 file）；② `截图目录/文件夹/夹`=location（screenshot_dir_is_location 提至 common，media 路由 + file_search file_type 双抑制 Screenshot + 新 LocationAlias 截图目录→截图）；③ file_search 补缺口：`创建`→created_time、LT 前缀 size 正则（smaller/less than/小于/不到…，v0.5 零暴露）、`less_than`→size_asc sort；④ coverage 对齐：zh-015 sort modified_desc→created_desc（v0.5 22 条 created 锚点全 created_desc，离群标注对齐）+ 更新 BETA-19 单测（image→file）。**v0.9 831→835 pass、fail 7→4、partial 162→161、§6 83.1%→83.5%；v0.5 byte-equal 零回归（500 case 0 diff）、workspace 全绿、clippy/fmt 净**。**剩余项**：④ **`documents`/`pictures` 类型义 vs 位置义消歧**（v0.5 ~24 条 location 锚点，高风险，宜单独 task）。**注**：剩 2 fail = §1.1 `screenshots…sorted`/`videos…sorted by size` 契约冲突，需评测集 re-baseline 非纯 parser）| packages/intent-parser | BETA-13-G11 | 2-3d |
| BETA-13-G13 | 过期 hybrid fallback 修复（首次跑 v0.9 hybrid 暴露模型反伤出厂质量）| done（2026-06-20，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g13)） | packages/intent-parser + apps/desktop + packages/evals | BETA-13-G12 | 完成于 2026-06-20 |
| BETA-13-G14 | 评测集 re-baseline 走向 §6 90%（决策清单 + 分组执行）| **done（2026-07-04 收束**：剩余 4 fail 经用户三项拍板全部消化——① v05-39a-039「把这些 pdf 复制到桌面」Refine→FileAction（与逐字相同的 39b-040 及 G5 拍板对齐）；② v05-45b-047「排除压缩包合并后」合并终态→Refine（单轮无状态评测下结构性不可测，合并归 harness 层）；③ v09-d5-en-024/025 §1.1 冲突 → coverage 改标 MediaSearch。**v0.9 = 881/119/0（§6 87.7%→88.1%）、variant 100%；v0.5 = 475/25/0**，锁定基线累计改 2 条（§6.5 豁免 ≤25 上限内），收束记录见[决策清单](docs/reviews/beta-13-rebaseline-decisions.md)顶部。距 90% 剩 1.9pp 全在 partial，路径 = BETA-29 草稿 UI 或新一轮缺口盘点。历史 cycle 记录如下）（2026-06-20）：用 v0.5 500 锁定基线逐条证伪 ~71 争议 partial（file_type 32+location 20+sort 19），重分类 Group A（coverage 标错对齐 v0.5）/ B（parser 缺口）/ C（真产品决策），产出 [决策清单](docs/reviews/beta-13-rebaseline-decisions.md)。**Group A 第 1 刀 done（待提交）**：coverage shards 12 条逐字段从 v0.5 锚点推导（file_type 显式类型词→document ×7、sort created/accessed→时间维度 + ext ×5）→ assemble-coverage→generate-evals-v09。**v0.9 835→847（精确 12 partial→pass）、§6 83.5%→84.7%、v0.5 byte-equal 0 变化、0 回归、v09_integrity 通过**。剩余 Group A 耦合 parser bug 归 B；A3 触 v0.5 base 排除。**Group B 三刀 done（+12，各独立 commit + TDD + byte-equal 闸门）**：B1 content-clause 文档类型名词→document（中文复合「劳动合同/协议文件」+ 英文 contract/report/agreement/resume/study notes 内容子句门控，+7）；B3 size 单位扩展（个G/bare g/gigs，+3）；B2「X文件夹」作 location（图片/影片文件夹 mirror screenshot_dir + Image 抑制，+2）。**v0.9 835→859、§6 83.5%→85.9%、全程 v0.5 byte-equal 0、0 回归、192 lib tests**。**Group C 决策 done（除 C1，+5）**：调研反转——C2 多类型 parser 多已正确给数组、拖累项是 documents/pictures 当 location（=C1）；C3 screenshot+time 是 coverage 错标 v0.5（6 条 created_desc）。落地 C3 screenshot→created_desc 对齐（+2）+ 用户拍板 oldest=created_asc（SORT_ALIAS，v0.5 零锚点，+1）+ 都找=数组（coverage d2-zh-035 ft 数组 + ext 删〔G11 多类型→ext=None〕+ parser frame-word「三种都找」，+1）。**v0.9 835→863、§6 83.5%→86.3%、全程 v0.5 byte-equal 0、0 回归、193 lib tests**。**诚实边界**：决策清单的 A~26/B~18 是标注方向条数，实翻少（耦合多字段）。**纯 parser/coverage 近见顶 ~86%；跨 90% 唯一剩余路径 = C1/G15**| packages/evals + packages/intent-parser | BETA-13-G13 | 进行中 |
| BETA-13-G15（C1）| `documents`/`pictures` 类型义 vs 位置义上下文消歧 | done（2026-06-20，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g15（c1）)） | packages/intent-parser | BETA-13-G14 | 完成于 2026-06-20 |
| BETA-13-G16（C2+backlog）| 多类型 ext 约定（C2）+ 相邻字段 backlog（keyword 泄漏 + opened 时间维度）| done（2026-06-20，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-13-g16（c2+backlog）)） | packages/intent-parser + packages/evals | BETA-13-G15 | 完成于 2026-06-20 |

#### B6：产品体验增强（演示能力）

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| BETA-15 | 同义词关键词扩展（手维护 YAML 词典 + harness 中间件） | done（2026-05-30，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15)） | packages/harness/src/synonym/ + apps/desktop + resources/synonyms/ | — | 完成于 2026-05-30 |
| BETA-15A | 同义词召回定量评测集（30~50 case fixture + 期望命中） | done（2026-05-30，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15a)） | packages/evals | BETA-15 | 完成于 2026-05-30 |
| BETA-15B | 词典升级为 embedding 索引 或 LoRA 在线扩词（二选一时评估） | not_started（父卡：2026-06-15 用户选「进取档」升格为旗舰「本地语义召回层」——按意思 / 跨语言模糊召回做差异化主打；拆 4 子项 BETA-15B-1..4）。**子周期全文详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15b-旗舰线子周期)**：BETA-15B-1 语义召回纵切 MVP（文档臂 embedding + BLOB 向量 + SemanticIndexBackend + 加权 RRF hybrid + macOS/Windows 真机验通）done；BETA-15B-2 向量索引后台预热 + 解耦调度 done；BETA-15B-3 融合调优（簇 A-1 相似度下限 / A-2 权重 w*=10 / A-3 FTS 置信度路由 / A-4 query 语种路由 / A-5 VEC top-1 cosine 阈值路由 ⭐ 首破 spec §5；簇 B 列迁移 + worker panic 兜底）done；BETA-15B-4 跨平台 + 拓宽 not_started；BETA-15B-5 / BETA-15B-6 见下独立行；BETA-15B-7 embedding 模型跨族探针（bge-m3 / qwen3-8b、暴露 pooling 硬编码 bug）done；BETA-15B-8 pooling type detection 修复 done；BETA-15B-9 llama-cpp-4 升级 + qwen3-8b 全零排查 done；BETA-15B-7-v2 bake bge-m3 生产 done；BETA-15B-10 bge-m3 baseline 重锚 + cosine sweep + 长文本扩量 done；BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 对照（sweep best OVERALL 0.900 / crosslang 0.725 复活字面 spec 双过）done；BETA-15B-11-v2 bake embeddinggemma-300m 生产（313MB、无 trade-off 全面提升）done。**当前 follow-up 抓手**：① cosine_threshold 在 embeddinggemma 上 sweep & bake（best 0.882）② baseline.json 切 embeddinggemma ③ BETA-15B-11-v3 prefix API 接 model-runtime（+0.013~0.026）④ 模型分发 UX。 | packages/harness + packages/indexer + packages/search-backends/semantic-index + apps/desktop + training/ | BETA-15, BETA-15A, **BETA-26（done）** | 旗舰线（多 cycle，见状态） |
| **BETA-15B-5** | **语义召回可信化（可解释 v1）：段落高亮 + 来源标注 + 置信档位** | done（2026-06-21，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15b-5)） | packages/search-backends/semantic-index（explain 纯模块）+ apps/desktop（命令 + 前端 UI） | BETA-15B-1, BETA-20（预览面板） | 完成于 2026-06-22 |
| **BETA-15B-6** | **持久化语义召回质量评测集 + baseline（质量调优前置设施）+ v2 扩量 + v3 二次扩量** | done（2026-06-21，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15b-6)） | packages/evals（semantic_quality 模块 + binary + 测试 + fixtures）+ packages/spike-retrieval（转正） | BETA-26, BETA-15A, BETA-15B-1 | 完成于 2026-06-22 |
| BETA-15C | WindowsSearchBackend / EverythingBackend 覆盖 `search_expanded` | done（2026-05-31，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15c)） | packages/search-backends/* + packages/harness + apps/desktop | BETA-15, MVP-26 | 完成于 2026-05-31 |
| BETA-15E | 自然中文 query 产出名词短语 keyword（解锁同义词特性 + 让 BETA-15D 复合谓词修复在真机可被验证）。**done（2026-05-30）**：方案 B=harness 层词典 gazetteer——`SynonymExpander::expand` 加 `query` 参数，仅当 parser 无 keyword 时扫 query 对 zh+en 索引做子串匹配，用 `parse(候选)` 重解析守护跳过类型/媒体词，取最长（并列取首现）注入单个内容词 group。**不动 parser/spotlight → evals parser-only 472/26/2 byte-equal（实跑确认）**。inline 执行 3 task + 每 task fmt/clippy/test 门 + ci.sh 全过；harness 测试 +5。诊断 [.../beta-15e-keyword-extraction-diagnosis.md](docs/superpowers/specs/2026-05-30-beta-15e-keyword-extraction-diagnosis.md)、设计 [.../beta-15e-gazetteer-design.md](docs/superpowers/specs/2026-05-30-beta-15e-gazetteer-design.md)、计划 [.../beta-15e-gazetteer.md](docs/superpowers/plans/2026-05-30-beta-15e-gazetteer.md) | done（详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15e)） | packages/harness/src/synonym + apps/desktop（不改 parser/spotlight） | BETA-15、BETA-15D（真机手测暴露） | 完成于 2026-05-30 |
| BETA-15D | Spotlight backend 谓词形态适配 macOS 26+ NSPredicate parser 回归 | done（2026-05-30，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-15d)） | packages/search-backends/spotlight + apps/locifind-cli | BETA-15（真机验证暴露） | 完成于 2026-05-30 |
| **BETA-20** | **结果预览面板（Quick Look：选中结果显示文本片段 / 图片缩略图 / PDF 首页 / 音频元数据+播放 / OCR 命中高亮）** | done（2026-06-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-20)） | apps/desktop（前端预览面板为主）+ packages/search-backends/local-index（预览数据源） | BETA-02, BETA-03, BETA-04 | 3-4d |
| **BETA-21** | **隐私 / 索引数据可视化轻量版（"索引了什么 / 日志在哪 / 一键清除"信任面板，V10-03 隐私 UI 早期预览）** | done（2026-06-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-21)） | apps/desktop（privacy.rs + PrivacyPage）+ packages/indexer（clear_index）+ packages/search-backends/local-index（clear） | BETA-06, BETA-07 | 2-3d |
| **BETA-22** | **搜索历史 + 保存的搜索 / 智能文件夹（本地持久化，可一键重跑）** | done（2026-06-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-22)） | apps/desktop（history.rs + SearchView + PrivacyPage）+ packages 无改动 | MVP-19 | 2-3d |
| **BETA-23** | **模型 fallback 接入桌面搜索默认流程（触发器扩展 + hybrid 编排 + feature 隔离）** | done（2026-06-13，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-23)） | packages/intent-parser + packages/model-runtime + apps/desktop + CI（release-windows.yml） | MVP-17, BETA-17 | 完成于 2026-06-13 |
| **BETA-24** | **LoRA 重训含 keywords 补全样本（问题 4 最后一公里）+ MediaSearch 内容词覆盖** | done（2026-06-13，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-24)） | training/mlx-lora + packages/evals + packages/intent-parser | BETA-23 | 完成于 2026-06-13 |
| **BETA-27** | **可配置本地索引目录 + 排除规则（通配符，参考 Everything 目录定义）** | done（2026-06-18，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-27)） | apps/desktop + packages/indexer | BETA-07（reindex 调度）, BETA-21（隐私面板复用）, BETA-15B-2（后台调度承接大目录） | 重估中（中等，初估 ~2-3d） |
| **BETA-26** | **本地语义检索质量探针（BETA-15B embedding 路径去风险，spike）** | done（2026-06-15，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-26)） | packages/spike-retrieval（throwaway）+ model-runtime embed() | BETA-02, BETA-03, BETA-15A | 完成于 2026-06-15 |
| **BETA-25** | **model-fallback 构建的动态库打包（静态链接路线）** | done（2026-06-13，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-25)） | apps/desktop + packages/model-runtime + CI | BETA-23 | 完成于 2026-06-13 |
| **BETA-29** | **查询意图可编辑草稿（搜索前/后展示 Search Intent 关键字段，用户一键修正类型 / 时间 / 排序后重跑）** | **in_progress（v1 done 2026-07-04、v2 done 2026-07-04 II）**：核心闭环已交付——`Started` 事件回带完整 intent JSON（三条执行路径统一）、新命令 `search_with_intent`（serde 强校验跳过 parser 直接执行、仅收 file_search/media_search、成功后照常 record 支撑后续 Refine）、意图条「调整 ▾」折叠面板（关键词/扩展名 chips 可增删 + 类型/时间/排序下拉 + 重跑、未编辑字段零丢失）；测试 ×5、手测清单入 [manual-test-scenarios BETA-29 节](docs/manual-test-scenarios.md)（随 cycle 9）。**v2 done（2026-07-04 II）**：① 草稿保存进 BETA-22 saved searches——`SavedSearch.intent: Option<Value>`（serde default 向后兼容旧文件）+ `save_search` 增 `intent` 可选参（保存时即走 `validate_draft_intent` 强校验闸门，与 search_with_intent 同口径）+ 草稿面板「保存草稿…」（与重跑共用 `buildDraft()` 保证所存即所跑）+ 带 ⚙ 角标 chip 重跑走 `search_with_intent`；③ 搜索前预览草稿——新命令 `preview_intent`（parser + Refine 合并、**不含模型 fallback**、只解析零 tool call）+ 搜索框 ⚙ / Shift+Enter 入口 + 预览意图条 + 复用草稿面板（「按此条件搜索」）、动作/澄清类提示后直接普通搜索；desktop 新测 ×3（history round-trip/闸门 + preview 零执行）、手测清单入 [manual-test-scenarios BETA-29 v2 节](docs/manual-test-scenarios.md)（随 cycle 9）。**剩余**：② 修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，卡随 BETA-30）。（2026-06-25 登记，源于 Claude × Codex 联合规划，借鉴 [LLM Wiki 文章](https://axk51013.medium.com/rethinking-agent-harness-part4-llm-wiki-%E5%8F%96%E4%BB%A3-rag-041629319804) Harness + Task 维度）。**价值主张**：把 parser 长尾问题（BETA-13 §6=87.7% 距 90% 差 2.3pp、纯 parser 见底）从「再训练 / 再标注」转移到「让用户一键修正」的产品化解法；BETA-13 剩余的 §1.1 video / screenshot+sort 等契约冲突可走草稿 UI 而非 re-baseline 评测集。**衔接**：Agent Harness（暴露 Search Intent JSON 到前端）、Search Intent schema、Ranker、BETA-13 eval pipeline（草稿修正可入 BETA-30 失败样本箱）、BETA-22 saved searches（草稿可保存）。**主要风险**：UI 复杂打断搜索流；缓解 = 默认折叠「展开高级」+ 一键 chips 修正常见字段（类型 / 时间 / 排序）+ 不打开高级时不影响主流程。 | apps/desktop + packages/harness + packages/intent-parser | BETA-13, BETA-22 | 1-2 weeks |
| **BETA-30** | **本地失败样本箱（低置信 / 零结果 / 用户快速改搜的 query 本地入箱，可标注转私有 eval / ranker 反馈）** | not_started（2026-06-25 登记，源于 Claude × Codex 联合规划，借鉴 [LLM Wiki 文章](https://axk51013.medium.com/rethinking-agent-harness-part4-llm-wiki-%E5%8F%96%E4%BB%A3-rag-041629319804) Task + Data 维度）。**价值主张**：Beta 之后真正稀缺的不是更多规则，是真实用户找不到什么。把 long-tail query 转成可观测可迭代的私有 eval、不需要云端 telemetry，符合本地优先原则。**衔接**：BETA-22 搜索历史、BETA-06 Audit、Ranker、BETA-13 eval pipeline（导出后入 private eval）、BETA-11C 用户同义词库（标注路径 → 同义词候选）、BETA-29 查询意图草稿（修正后行为入箱）。**主要风险**：隐私（日志含查询词和路径）、反馈偏置、日志膨胀；缓解 = 本地保存 + 用户可清空 + 显式导出后才与 evals pipeline 集成 + 只记录用户触达的 hot failures 不全量。 | apps/desktop + packages/evals + packages/ranker | BETA-22, BETA-06 | 1-2 weeks |
| **BETA-31** | **Windows 模型分发 UX 增强（双平台 onboarding 扩 3-step + GUI 一键下载 + example queries 内嵌）** | done（2026-06-27，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-31)） | apps/desktop/src-tauri + apps/desktop/src | BETA-15B-11-v2 | 完成于 2026-06-27 |
| **BETA-32** | **团队归档 MCP daemon（headless 服务，包装 hybrid 检索给 Claude Code / MCP 客户端用、单一固定集合、内网局域网部署、bearer auth）** | done（2026-06-29，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-32)） | packages/locifind-server + apps/daemon + packages/indexer + packages/evals + .github/workflows | — | 完成于 2026-06-29 |
| **BETA-33** | **桌面菜单栏重构 + 选项对话框（参考 Everything：7 个下拉菜单 + 模态「选项」窗）** | **in_progress（2026-07-03 Claude Code、cycle 1..7c + cycle 9 done）⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐**。**cycle 9 done（2026-07-03、六刀全清，代码层完、真机验证随下次发版按 [manual-test-scenarios「BETA-33 cycle 9」节](docs/manual-test-scenarios.md)）**：① embed 失败 degrade 根修——`TextEmbedder::is_ready()` 默认探测方法 + `EmbeddingModelHandle` 状态机实现 + `SemanticIndexBackend::is_available()` live 探测，feature 关/模型缺失/加载失败时必败语义臂**路由期退出** fanout（旧 `is_available` 只查句柄存在是「dev 版恒报 embedding 模型不可用」真根因）+ fanout 零结果报错优先非语义臂错误；② 单实例锁（tauri-plugin-single-instance、注册在全插件之前、二开聚焦既有窗口；licenses 顺带补 BETA-27 漏登记的 dialog）；③ `check_windows_search_indexed` 真做（`sc query WSearch` STATE 数字码 locale 无关解析、zh-CN 真机实测 + onboarding 第 1 步状态条首个真实消费点）；④ StatusIndicator/EmbedStatus 文案重写（`lib/model-status.ts` 单一信源：EmbedStatus 类型 ×3 / embedStatusLine ×2 / 顶栏第三套文案收拢、tooltip 不拼 raw 路径/Rust 错误串）；⑤「本地索引」全库 vs 概貌口径统一（`IndexStatus.db_totals` 结构化 + 概貌卡差值显式提示、隐私面板全库语义不动）；⑥ useAppSettings hook 抽取 + 删 SettingsPage（-538 行）+ `/settings` 路由（cycle 3 起已无导航入口）。新增 11 单测、六 crate 全测零回归。**剩余 cycle 10+**：PrivacyPage 迁「隐私」分类（~1d）/ 上下文 enable/disable（~0.5d）/ 子菜单 ▸ 展开 + 后端接通（~2d）/ dev feature-full script（需装 cmake、~0.5d）/ macOS 原生菜单 BETA-34（~3-5d）。**此前**（2026-07-02、cycle 1+2+3+3v2+3v3+3v4+4+5+6v4+7a+7b+7c done、v0.9.4~v0.9.7 已发 + v0.9.7 真机验证 (a)-(j) 9 GO / 1 FAIL（(d) window.confirm、已随 7c 修）+ cycle 7c 合 bump v0.9.8 待 tag）：菜单栏骨架 + 关于对话框 + 事件总线 + 全局快捷键 + Alt 访问键已落地。**cycle 7-a done（v0.9.7、Codex APPROVED with suggestions ⭐⭐⭐）**：v0.9.6 首次真机验证锁定"C 主 + B 副 + 数据源不一致 + 进度可视化断层"（用户报"添加后不显示"= 覆盖语义系统默认三夹消失的心智冲突、非真 bug、代码工作正常）。索引 pane UX 打磨——(1) C 修法：加"覆盖语义系统默认已隐藏"大字醒目提示条 + checkbox 增强绿描边加粗 + 概貌"目录"改「生效目录」+ tooltip；(2) B 修法：picker 后新目录 flash 1.5s 高亮 + scrollIntoView（Codex SUGGEST 9） + `⏳ 待应用` 琥珀 chip + `hasUnsavedChanges` sticky 底部 warn 提示 + 关闭前 window.confirm 二次确认；(3) 数据源统一（Codex APPROVED 2 选 (a)）：概貌 latestTime 引用 indexOverview + `prevIndexing` useEffect 完成时强刷 IndexStatus；(4) 进度可视化（Codex OBJECT 3 · SUGGEST 4/5）：新 `IndexStatus.current_phase` + `IndexProgress::on_phase(phase)` trait method 默认 no-op + `StatusProgressBridge::on_phase` impl + local-index `reindex_with_progress_inner` 每 phase 前调（MusicDiscovery 时同步清 current_root）+ 前端 phase chip 拼进 `indexStatusLine`（🎵 扫描音乐（Everything 全盘发现，请稍候）/ 📄 扫描文档 / 🖼 扫描图片）+ indeterminate 动画进度条（不做百分比）。改动 7 文件。本机 cargo 全测过 indexer 121 / local-index 22 / desktop bin 140 / desktop::search::index_status 9（含新 `phase_bridge_*` 单测）/ clippy 干净 / fmt 干净 / tsc 干净。**cycle 7-b done（v0.9.7 同版）**：子路径排除（相对 root path glob）—— 新 `RootExclude { root, patterns }` struct + `AppSettings.root_excludes` + `normalize_root_key(path)` 跨平台归一 helper（Windows 反斜杠→\+小写、Unix 正斜杠+保留大小写、trim 尾部分隔符）+ indexer `ExcludeFilter` struct（basename 全局 + per_root 相对路径 GlobSet 双层）+ `from_basename_set(&GlobSet)` 兼容构造（**Codex OBJECT 2 保 BETA-27 basename-only byte-for-byte**）+ `build(exclude_globs, root_excludes, normalize)` 生产构造（Codex SUGGEST 3 边界：以 `/**` 结尾 pattern 自动补目录本身让 walkdir 剪枝生效）+ Windows 分隔符归一（rel `\` → `/` 再喂 globset）+ 3 个 `_with_filter_and_progress` 方法（Music/Doc/Image）+ local-index `reindex_scoped_with_filter_and_progress` + desktop `read_index_config_with_filter` + `perform_reindex` 走 filter 版本 + 前端 `RootRow` 加折叠区「子路径排除 ▸ (N)」+ hint 通配符（`**`/`*`/`?`）说明 + 移除 root 时同步删该 root 的 root_excludes 条目（无孤儿）。改动 8 文件。本机 cargo 全测过：indexer 127（+6 新 ExcludeFilter：dir_itself / windows_separator / empty_root_short_circuits / walkdir_per_root / per_root_no_leak / from_basename_set 等价）/ desktop::settings 17（+5 新：`root_excludes_default_is_empty` / `root_exclude_serde_round_trip` / `normalize_root_key_windows_equivalence` / `normalize_root_key_empty_returns_empty` / `old_settings_without_root_excludes_parses_ok`）/ desktop bin 145 / clippy 干净（顺手删旧 `is_excluded_dir` 无 caller fn + 修 `derivable_impls` + `redundant_closure`）/ fmt 干净 / tsc 干净。**cycle 7-c done（2026-07-02、bump v0.9.8 待 tag、Codex SUGGEST 6/7/8/10 全落地 + 真机验证发现的 (d) confirm 失效顺带修）**：① 单目录重扫——`perform_reindex` 抽 `perform_reindex_for_roots(status, db, settings_path, roots_override)`、override 只换扫描 roots、exclude_globs / root_excludes / OCR / progress bridge 仍从 settings live-read（不给绕过排除配置的旁路留口子）+ 新 tauri command `reindex_root`（校验目录存在 + spawn_blocking + 完成接语义 worker、与全量 reindex 同构）+ RootRow「重扫」按钮（`⏳ 待应用` pending root 不显示、索引中禁用）；② 打开目录——**复用既有 `open_path` 命令**（FileActionTool 策略 + audit 口径一致、Windows `cmd start` 对目录开 Explorer / macOS `open`；与 design doc 的 plugin-shell 方案偏差 = 零新依赖、目标一致）+ RootRow「打开」按钮；③ 移除目录二次确认——单选「仅从索引配置移除」（默认）vs「移除并清除索引记录」、文案明确**不删除磁盘上的任何原文件**、清除失败时不移除配置（避免"以为清了其实没清"）；`MusicIndex::purge_under_root` + `DocumentIndex::purge_under_root` SQL 落 indexer 存储层、与 `stats_under_root` 共用新抽 `root_glob_predicate` / `root_glob_params` 边界 helper（**统计口径 = 清除口径**）、FTS 同事务同步删、document_vectors 外键级联；新 tauri command `purge_root_from_db` 薄封装；④ **(d) window.confirm 失效修复**——v0.9.7 真机验证发现 wry/WebView2 生产装机版 `window.confirm` 不弹窗直接放行、cycle 7-a 关闭守卫静默丢改动（对照实验排除 hasUnsavedChanges 状态问题）；新通用 in-DOM `ConfirmModal` 组件替换、关闭守卫与移除确认共用、Esc 在弹窗打开时只关弹窗不穿透外层守卫（详记忆 tauri-webview2-window-confirm-noop）。改动 8 文件；本机全测过：indexer 130（+3 purge 单测含 Codex SUGGEST 10 外键级联）/ local-index 22 / desktop bin 145 / clippy `-D warnings` 干净 / fmt 干净 / tsc 干净。**cycle 8 done（2026-07-02 Claude Code Opus 4.7、待 bump v0.9.9 一起 tag）**：快速入门 6 步重构（Win 6 / Mac 5，兑现 BETA-31 cycle 2 UX 扩展 pending 的 Everything 引导 + 生成模型下载入口）—— 新 4 组件 `apps/desktop/src/components/onboarding/{OnboardingShell,EverythingCheckStep,IndexRootsStep,FirstIndexStep}.tsx` + 改 5 文件（OnboardingWin/Mac 页 + [menu-events.ts](apps/desktop/src/lib/menu-events.ts) 新 `open-prefs-indexing` action + App.tsx 传 initialCategory + [PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) 新 `initialCategory?: Category` prop）。步骤：① Windows 索引 / Mac FDA；② Everything CLI winget 命令（复制按钮 + voidtools fallback + `get_backend_status` 前端 filter `search.everything` + 3s 自动重检）；③ 嵌入模型（复用 ModelDownloadStep kind='embedding'）；④ 生成模型 Qwen3-0.6B（复用 ModelDownloadStep kind='generation'）；⑤ 索引目录（内嵌只读 `get_effective_index_roots` 列表 + 「打开索引选项…」触发 `open-prefs-indexing` → PreferencesDialog 直接停在「索引」分类）；⑥ 首次索引（reindex + 实时 fts/semantic 进度条 + 行内 2×2 示例卡 + **不阻塞「完成」按钮**）。**零后端命令新增**（复用 `get_backend_status` / `get_effective_index_roots` / `reindex` / `get_index_status` / `open_windows_indexing_options`）；useShouldShowOnboarding 判定不变（第 3 步完成时仍调 `complete_onboarding('model_download')`）。dev 真机验证反馈两轮微调：① UX 紧凑化（shell padding 36/40/48→18/36/20、stepperDot 22→20、title fontSize 24→20、subtitle 13→12.5、children marginBottom 28→16、button padding 12/28→9/22、各步骤内部 padding/lineHeight 全线收一档）；② shell 加 `skipAction` 槽（primary 左侧 ghost）+ 每步都填「跳过此步」+ Step 3/4 隐藏 ModelDownloadStep 内嵌 skip 避免双重按钮；③ 顺带清 PreferencesDialog 索引概貌顶部 3 emoji（📄🖼🎵）+ RootRow 分类计数改中文（"文档 N · 图片 N · 音乐 N"）。tsc + vite build 双绿；BETA-31 cycle 2 UX pending 至此关闭；`check_windows_search_indexed` 仍是 stub 留 cycle 9。**cycle 1+2 改动概览（3 新文件 + 6 改、单次 commit）**：① 新 [MenuBar.tsx](apps/desktop/src/components/MenuBar.tsx) ~325 行（7 个下拉 + 28 菜单项含分隔线 + 全局快捷键 Ctrl+N/F/P/D/Shift+C/, + Alt+F/E/V/S/B/T/H 访问键 + hover 切换 + 点空白/Esc 关闭 + aria）；② 新 [AboutDialog.tsx](apps/desktop/src/components/AboutDialog.tsx) ~75 行（模态弹窗 + getVersion + GitHub 链接）；③ 新 [lib/menu-events.ts](apps/desktop/src/lib/menu-events.ts) ~45 行（MenuAction 10 种 + emit/listen helpers + CustomEvent `locifind:menu`）；④ [App.tsx](apps/desktop/src/App.tsx) +3/-12 删 NavLink 换 MenuBar；⑤ [styles.css](apps/desktop/src/styles.css) +179/-24 菜单栏 + 关于对话框样式；⑥ [SearchView.tsx](apps/desktop/src/SearchView.tsx) +60 inputRef + menu listener bridge 9 actions；⑦ tauri.conf.json + Cargo.toml + Cargo.lock 三处 0.8.9 → 0.9.0。**接通状态**：✅ 14 menu item + 6 快捷键 + 7 Alt 访问键 + 选项对话框；🚧 cycle 4+ 待：导出/列控制/排序/范围/跨语言/高级语法/管理保存/重建索引/索引状态/模型 ▸/后端 ▸/打开日志数据目录/键盘快捷键/用户手册/反馈/context-aware enable。**cycle 3 改动概览（v0.9.1、1 新文件 + 5 改、单次 commit）**：① 菜单栏紧凑化（[styles.css](apps/desktop/src/styles.css) `.app-header` min-height 28→22px / `.menu-bar-title` font-size 0.82→0.78rem + padding 0.2 0.55→0.08 0.32rem + 去 margin / `.menu-access-key` font-size 0.75→0.72rem + opacity 0.6→0.55，视觉密度对齐 Everything）；② 新 [PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) ~540 行：模态卡片 760×560 + 标题栏 + 左侧 4 分类树（常规 / 语义召回 / 索引 / 隐私与记录）+ 右侧分类对应 Pane（GeneralPane / SemanticPane / IndexingPane / PrivacyPane）+ 底部 取消/应用/确定；复用全部 SettingsPage 后端 tauri command（`get_settings` / `update_settings` / `get_audit_log` / `get_index_status` / `get_model_status` / `embedding_model_status` / `get_effective_index_roots` / `reindex` / `clear_audit_log`，**zero backend change**）；③ [lib/menu-events.ts](apps/desktop/src/lib/menu-events.ts) 加 `open-prefs` MenuAction；④ [MenuBar.tsx](apps/desktop/src/components/MenuBar.tsx)「工具→选项」与 Ctrl+, 改 emit("open-prefs") 不 navigate("/settings")；⑤ [App.tsx](apps/desktop/src/App.tsx) 加 showPrefs state + onMenuAction listener + `<PreferencesDialog />` 条件渲染；⑥ styles.css 新增 `.prefs-*` 系列 ~250 行（backdrop / 760×560 dialog / 160px 左栏分类树 / 右栏 scroll pane / 表单 form 控件 / footer 按钮 / audit table）；⑦ 旧 `/settings` 路由 + SettingsPage 保留作 fallback（**零回归保底**，cycle 4 才考虑删）；⑧ bump tauri.conf.json + Cargo.toml + Cargo.lock 三处 0.9.0 → 0.9.1。**cycle 3 v2 hotfix（v0.9.2、commit `318fdd3`、7 文件 +98/-14）**：本地首次 `npm run tauri dev` 真机验证 v0.9.1 发现 3 处交互 bug、当场修 2 个。① **Esc 键关闭修复** = 4 版试错定位（v1 `document.addEventListener` ❌ / v1.1 `window+capture+useRef` ❌ / v1.2 React onKeyDown+dialog root autoFocus ❌ / **v2 GO** = React `onKeyDownCapture` 挂 backdrop 元素 + `dialogRef.current?.focus()` 挂载自动获焦，React 合成事件层不受 Windows Sogou IME 拦截）；② **点遮罩关闭修复** = dialog 从 760×560 缩到 `min(720px, calc(100vw-80px)) × min(520px, calc(100vh-100px))` + 加 `outline: none`，默认 800×600 窗口下遮罩 ≥40px 可点；③ **Ctrl+, 打不开对话框** = Sogou IME 拦 `,` 键、无代码 fix、登记 cycle 4 follow-up 换绑 Ctrl+; 或 Ctrl+Shift+,；顺带 4 项 follow-up 登记（dev 版 `cargo run --no-default-features` 关掉 semantic-recall+model-fallback feature → 搜索恒报「embedding 模型不可用」；embed 报错整个搜索不 degrade；顶栏灯只判 feature+文件不判 load；npm 11 approve-scripts esbuild 加 `allowScripts` 到 package.json）；顺带 tauri-build 重生 `windows-schema.json` 一并入 commit。**cycle 3 v2 真机验证 GO 项（computer-use 驱动 GUI 截图 + zoom 双源）**：菜单栏紧凑度对齐 Everything / 工具→选项 弹模态 / 4 分类切换 / 索引 pane 显示真数据（3 系统默认路径 + 「上次索引 音乐 34185 / 文档 187 / 图片 4687」） / 隐私 pane 8 条 audit 真记录 / 取消 / × / **Esc** / **点遮罩** 全过。**cycle 3 v3（v0.9.3）**：暴露语义原始 cosine + 前端「相似度」列 + 按相似度排序（走 MergedResult.semantic_cosine 传给 SearchResultJson、避开 42 个 SearchResult 构造点、只动 3 个 MergedResult 构造点、真机 GO）。**cycle 3 v4（v0.9.4）**：Qwen3-0.6B 一键下载（model_download.rs 重构 ModelKind 枚举 + 独立命名空间事件 + useModelDownload/ModelDownloadStep 参数化 kind + PreferencesDialog 常规 pane 嵌入 not_found 下载按钮、~400 MB 从 unsloth/Qwen3-0.6B-GGUF 拉、Embedding 兼容 emit 老无 ns 事件不破 v0.9.3 前端） + 段落 vs 文档 cosine label 明示。**收工前追加（待 v0.9.5 出货）**：① Ctrl+, → Ctrl+; 换绑绕 Sogou IME（`,` 老键 fallback、双键 both emit `open-prefs`、真机秒验）；② StatusIndicator 顶栏灯口径修正（额外查 `embedding_model_status`、语义灯 ready 绿 / loading 蓝 / not_found 琥珀 / failed 红 / unavailable 灰、轮询 30s → 10s、与选项对话框「语义召回」pane 状态源统一）。**cycle 4 done（v0.9.5 待收工提交）**：OCR 乱码 + 图片语义污染防治双层门槛——A 层 `is_embed_worthy` v2（`MEANINGFUL_CHAR_RATIO_FLOOR=0.6` CJK+拉丁占非空白 ≥60% 才嵌）+ B 层 `embed_pending` / `explain_semantic_hit_impl` / `purge_short_body_vectors` 三处「图片 doc_type 一律跳过语义索引」+ 段落级 `EXPLAIN_MIN_SCORE 0.30→0.45` + `passage_worth_embedding` 段级门槛（字数 ≥8 + meaningful ratio ≥60%）。修复 v0.9.4 用户搜「作文」踩到 QQ 表情包 `face-3-efdc54.png` OCR 乱码段落级虚高 0.62 bug。改动 5 文件、本机 cargo 全测过（indexer 115/115、semantic-index 16/16、clippy `-D warnings` 干净、桌面 crate check 干净）；顺手修 Rust 1.96 新 lint `map_unwrap_or` 2 处（非本 cycle 引入）；已 commit `6e6d008` 未 tag。**cycle 5 done（v0.9.5 同版、一起 tag 待收工提交）**：索引目录概貌 + 分类统计 UI——后端新 tauri command `get_index_overview` + `DocumentIndex::stats_under_root` + `MusicIndex::stats_under_root`（SQL GLOB 3-OR 前缀边界跨 Windows / Unix 分隔符、图片按 `doc_type IN IMAGE_EXTS` 拆分）；前端 IndexingPane 顶部加 `.prefs-overview-card`（6 单元格）+ 新 `RootRow` 组件让每目录行展示 `📄 X · 🖼 Y · 🎵 Z` + 人性化时间；useEffect 跟 `settings.index_roots` fetch + 监听 `indexStatus.indexing` 从 true 转 false 时自动刷新。改动 7 文件；indexer 119/119、clippy `-D warnings` 全 workspace 干净、tsc 干净、fmt 干净。**未做 follow-up**：① size 字段（需 schema migration + upsert 补 size + 走 std::fs::metadata、~2h）；② 单目录重扫 `reindex_root(path)` command（~2h）；③ 打开目录 plugin-shell 按钮（~30 分钟）；④ 移除目录时同步 purge 该 root 下 DB 条目（~1h）。**cycle 6 v4 done（bump v0.9.6、待 tag ⭐⭐⭐）**：追加系统默认目录 checkbox + FTS 索引进度可视化——① **AppSettings 加 `include_system_defaults: bool`**（默 false 保旧覆盖语义、零回归）+ 新 `resolve_index_roots_tagged(raw, include_defaults) -> Vec<(PathBuf, bool)>` 主 API + 3 分支覆盖（空 raw / 覆盖 / 追加去重）+ `get_effective_index_roots` / `get_index_overview` 都加 `include_system_defaults: Option<bool>` 参数 + 抽 `read_effective_inputs` helper；② **IndexStatus 加 `current_root: Option<String>` + `fts_progress: Option<(u64, u64)>`** 字段 + `fts_begin` / `fts_finish` 生命周期 fn + `StatusProgressBridge` impl `locifind_indexer::IndexProgress` trait（on_file 累加 scanned/indexed + 父目录更新 current_root）；③ **local-index 新 `reindex_scoped_with_progress` API**（音乐 Everything 发现分支保 `index_paths` 无进度、fallback + 文档 + 图片全走 `_with_progress` 变体、`perform_reindex` 走它 + fts_begin/finish 包夹）；④ **前端 PreferencesDialog IndexingPane 统一按 effectiveRoots 渲染**（自定义项显示「移除」、系统默认项显示「系统默认」tag）+ `index_roots.length > 0` 时暴露 checkbox「☐ 同时索引系统默认目录」 + `indexStatusLine` 索引中显示「⏳ 正在索引：📁 current_root　已扫描 N · 已入库 M」+ SettingsPage fallback 页 interface 也加字段防 spread 丢字段；⑤ 顺手删旧 `resolve_index_roots(raw)` wrapper + `fts_set_current_root` 占位 fn（都无 caller、YAGNI）。改动 6 文件 +250/-70。**本机全测过**：indexer 119 / local-index 22 / desktop bin 141（3 ignored Windows 真机 e2e）/ desktop::settings 12（含新 3：tagged 3 branches + dedup + default）/ desktop::search::index_status 8（含新 1：桥闭环）/ clippy `-D warnings` 干净 / fmt 干净 / tsc 干净。**修复的 UX gap**：加自定义目录后系统默认 3 目录消失（override 语义 UX 坑、方案 B checkbox opt-in 追加）+ 索引进度只有静态"正在后台索引…"、看不到当前目录 + 数字。**cycle 7-N 路线**：cycle 7 embed 报错 degrade 到其他 3 后端（~30 分钟）+ dev feature-full script（~0.5d、需装 cmake）→ cycle 8 抽 `useAppSettings()` hook + 删 SettingsPage 旧路由（~1d）→ cycle 9 PrivacyPage 迁「隐私」分类（~1d）→ cycle 10 上下文 enable/disable（无选中项时灰、空 query 时灰，~0.5d）→ cycle 11 子菜单 ▸ 展开（视图→列/排序、工具→模型/后端，~1d）+ 后端接通（重建索引 + 打开日志/数据目录 + 用户手册/反馈 URL，需 plugin-shell + 3 tauri command，~1d） + StatusIndicator/EmbedStatus 文案 UX 整体重写（backend detail 字符串前端拼句、~0.5d）→ cycle 12 macOS 原生菜单（用 Tauri tauri::menu::Menu API，~3-5d、可独立 BETA-34）。**cycle 3 接受标准**：本机 cargo + node 均不可用（[[ci-ubuntu-first-run-lint-gaps]]）、CI ci.yml 兜底；真机正确性留 v0.9.1 release 装机后用户验证：(a) 菜单栏视觉密度对齐 Everything 截图；(b) 点「工具→选项」或 Ctrl+, 弹模态对话框、不再跳 `/settings` 长页；(c) 左侧 4 分类切换右侧表单内容；(d) 改字段点「应用」绿字「设置已保存」；点「确定」保存并关；(e) Esc / 点遮罩 / 右上 × 都能关闭；(f) 现有所有搜索 / 索引 / 预览 / 历史 / 保存的搜索 / 旧 `/settings` 路由 零回归。**cycle 1+2 接受标准**：本机 cargo + node 均不可用（[[ci-ubuntu-first-run-lint-gaps]]）、CI 兜底；真机正确性留 v0.9.0 release 装机后用户验证 (a) 顶部菜单栏 7 个下拉可点击展开 + hover 切换；(b) Alt+F 等访问键展开对应下拉；(c) Ctrl+N 清搜索框 + Ctrl+F 聚焦 + Ctrl+P 切预览面板；(d) 选中结果后 Ctrl+Shift+C 复制路径 + 状态栏「已复制路径：xxx」；(e) 工具→选项 跳现 `/settings` 长页面；(f) 帮助→关于 弹版本 v0.9.0；(g) 现有所有搜索/索引/预览/历史/保存的搜索 零回归。**承接（原 not_started 卡片描述）**：源于用户「请参考 Everything 菜单栏重新规划」需求，**范围 = 路径 C**：先纯前端 React `<MenuBar>` 组件 + `<PreferencesDialog>` modal、Windows 优先；macOS 用 Tauri `tauri::menu::Menu` 原生顶栏留 BETA-34（后续）。**菜单清单（7 个下拉）**：① 文件(新建搜索 Ctrl+N / 打开 Enter / 在资源管理器中显示 Ctrl+Enter / 复制路径 Ctrl+Shift+C / 导出结果 CSV/JSON / 退出) ② 编辑(撤销/重做/剪切/复制/粘贴/全选 + 查找 Ctrl+F) ③ 视图(列 ▸ / 排序 ▸ / 预览面板 Ctrl+P / 状态指示 / 快捷键横条显隐) ④ 搜索(重置 Esc / 搜索范围 ▸ / 跨语言匹配 / 搜索历史 / 清空历史 / 高级语法帮助) ⑤ 书签(保存当前 Ctrl+D / 管理 + 动态列出 saved_searches 前 N 条直接点击运行) ⑥ 工具(重建索引 / 索引状态 / 我的同义词 / 隐私与数据 / 模型 ▸ / 搜索后端 ▸ / 打开日志目录 / 打开数据目录 / **选项 Ctrl+,**) ⑦ 帮助(快速入门 = Onboarding / 键盘快捷键 / 用户手册 = docs / 反馈与报告 bug = GitHub Issues / 关于 LociFind)。**「选项」对话框结构**（参考 Everything）：左侧分类树 + 右侧表单 + 底部确定/取消/应用；分类 = 常规(界面/首页/搜索/结果/视图/上下文菜单/字体颜色/快捷键含 global_shortcut) / 历史 / 索引(目录 = index_roots / 排除规则 = exclude_globs / 调度) / **模型与召回**(LociFind 特有：model_path / similarity_floor / semantic_weight / enable_model_fallback) / **后端**(LociFind 特有：Spotlight/WindowsSearch/Everything/Semantic 启停) / 隐私(tracing 开关 / 数据根 / 清空各类) / 高级(env 开关含 LOCIFIND_ENABLE_EMBED)。**承接关系**：吃掉现有 `/settings`、`/privacy`、`/synonyms` 三个独立路由的内容到「工具」菜单 + 选项对话框；保留 `/` 搜索主路由 + 兼容现有 NavLink（or 完全替代）。**估时**：~1-2w（前端骨架 + 路由收编 + 选项对话框分类树 + 14 个分类的表单迁移 + 真机手测）。**验收**：(a) 顶部菜单栏 7 个下拉可展开 + 键盘 alt+首字母唤出；(b) Ctrl+, 打开选项对话框；(c) 现有所有 settings 字段迁入选项对话框对应分类、保存重启持久化；(d) Windows v0.9.0 真机验证菜单点击 / 键盘导航 / 选项保存 / 现有功能（搜索 / 索引 / 预览 / 历史 / 保存的搜索）零回归；(e) macOS 原生菜单留 BETA-34。**风险**：① CSS 模拟菜单与 webview 默认右键 / Tauri context menu 冲突需排查；② alt+首字母在 webview 里不一定被 Tauri 截获、需测试；③ 选项对话框 14 个分类一次性出工作量大、可拆 v1（核心 8 分类）+ v2（其余 6 分类）。**对应 spec / plan**：本次会话已对齐方案、下次会话开 cycle 时 spec 落 `docs/superpowers/specs/2026-XX-XX-beta-33-desktop-menubar-revamp-design.md`。 | apps/desktop/src + apps/desktop/src-tauri | BETA-31-v3 收尾 | 1-2 weeks |

**BETA-15D 验收**：
- 复合 keyword + extension 谓词在 macOS 26+ 真机 mdfind 上接受不 reject
- BETA-15 spec §7 scenario 1（`找一份工作汇报相关的ppt → 述职.ppt`）在真机端到端命中
- 既有 evals v0.5 parser-only / hybrid 不回归（472/26/2 + 480 维持）

**B6 升级判定**：手维护词典 > 200 组、或出现可重复的"用户原 case 找不到"反馈时启动 BETA-15B。

#### B7：企业冷归档检索底座（三场景，并行衍生子线）

> 2026-07-02 定位收敛后登记（方案与 Codex 评审全文见 [docs/reviews/doc-realign-retrieval-foundation.md](./docs/reviews/doc-realign-retrieval-foundation.md)，Codex 结论 APPROVE with required adjustments、修正意见已全部合入本小节）。
> 目标场景：① 律所案件卷宗检索；② 企业内部审计取证检索；③ 离职员工材料归档检索。共同画像 = 敏感数据不出门 + 冷归档 + 检索者不熟悉语料组织方式 + 需留痕。
> **红线：本小节不修改 §6.3；BETA-14 / §6.3 仍是 Beta 出场依据。B7 与 BETA-32 同属并行衍生子线，不进 Beta 出场硬指标、不阻塞 B→V 切换。**
> 分析层（内容关联分析 / 摘要 / 比对 / 起草）**不自建**——经 BETA-32 daemon + 外部 LLM（MCP 工作流）组合实现；V10-13/15/16 已相应重定性（见 §3.4）。

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| **BETA-35** | **扫描版 PDF OCR 管线（PDF 页渲染成图 → 复用 OcrEngine 层；三场景共同第一缺口）** | done（2026-07-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-35)） | packages/indexer + packages/search-backends/local-index + apps/desktop | BETA-02, BETA-03 | ~~1-2w~~ → 1 天（AI 全程） |
| **BETA-36** | **daemon 检索权限模型 + 归档集合（collection）模型** | done（2026-07-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-36)） | packages/locifind-server + apps/daemon | BETA-32 | ~~1.5-2.5w~~ → 1 天（AI 全程） |
| **BETA-37** | **邮件格式提取（eml 正文 + 附件 + headers 基础字段；msg 拆 BETA-37b 后置）** | done（2026-07-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-37)） | packages/indexer | BETA-02 | ~~1-1.5w~~ → 1 天（AI 全程） |
| **BETA-37b** | **msg（Outlook OLE/CFB）提取——BETA-37 拆出后置：无 fixture 验收靶、Rust crate 生态弱（msg_parser 久未更新），等真实样本/需求再评估 crate 或 shell-out 方案** | not_started | packages/indexer | BETA-37 | 2-4d |
| **BETA-38** | **向量检索规模化 + 文档身份/去重策略（内存缓存优化暴力 + 文件身份 hash 去重，十万级水位）** | done（2026-07-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-38)） | packages/indexer + packages/search-backends/semantic-index + packages/evals | BETA-15B | 1-2w |
| **BETA-39** | **图片语义索引 opt-in + 质量门槛（解除 BETA-33 cycle 4 一刀切）** | done（2026-07-03，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-39)）。**2026-07-04 III 追加：daemon 默认开**（用户拍板）——`ServerConfig.embed_images` daemon 默认 true / 桌面 opt-in 不变、`--disable-image-semantics` 逃生舱、启动期 purge 镜像桌面语义；顺带修语义臂空洞（`SemanticIndexBackend` 扩展接受图片类 MediaSearch + `IMAGE_EXTS` 过滤——`截图/照片` 问法此前语义臂直接跳过）；enterprise eval O-09 顶位命中实证 2 字 CJK 词图片内容可达：[beta-40-enterprise-eval-2026-07-04.md §7](docs/reviews/beta-40-enterprise-eval-2026-07-04.md) | packages/indexer + apps/desktop + packages/locifind-server + apps/daemon + packages/search-backends/semantic-index | BETA-33 | ~~2-3d~~ → 1 天（AI 全程）；daemon 默认开追加 0.5 天 |
| **BETA-40** | **MCP 场景 playbook（三场景各一篇：部署 + 权限配置 + 示例 query + LLM 工作流；吸收原 V10-13）** | **in_progress**（2026-07-03 文档面 done：[docs/playbooks/](./docs/playbooks/README.md)；2026-07-03 Codex 补三场景本机 smoke 证据：[enterprise-semantic-daemon-test-report-2026-07-03.md](docs/reviews/enterprise-semantic-daemon-test-report-2026-07-03.md)，真实模型加载 + daemon-mode smoke 3/3 + locifindd e2e 9/9 + 三场景 MCP 查询命中。**2026-07-04 Claude Code 口径修正 + 底座补齐**：7-03 的"semantic 命中"实为 FTS 字面命中——当时 daemon `document_vectors` 恒空、候选链无语义后端；本轮修 4 根因（中文 CMap PDF panic 降级 OCR / daemon 图片轮 / daemon embed pass+语义臂+RRF 融合 / WinRT 正斜杠路径），补 `index_failures` 失败留痕，真实模型端到端复验语义召回真实生效：[beta-40-ingest-semantic-gap-fix-2026-07-04.md](docs/reviews/beta-40-ingest-semantic-gap-fix-2026-07-04.md)。**2026-07-04 II Claude Code：enterprise eval 自动化 done**——`packages/evals` enterprise 模块 + `enterprise_scenarios` binary（queries.tsv 21 case → 真 locifindd collection 模式 → 逐 subject token 信息墙双断言 + top-K 命中）+ `enterprise_scenarios_gate` 回归门（fixture 完整性常跑 CI + env 门控端到端 `--require-all`）；真实模型 baseline 全过（同日 III 扩至 **22/22**，含图片语义 case O-09）、首轮暴露并修复 csv/tsv 不在 `DOC_EXTS` 的覆盖缺口；daemon 加 `--semantic-weight` 旋钮、权重 10 vs 3 排名逐 case 一致 → 融合权重问题结案维持默认：[beta-40-enterprise-eval-2026-07-04.md](docs/reviews/beta-40-enterprise-eval-2026-07-04.md)。**同日 IV：桌面 UI 消费 `extraction_failures()` done**（「选项 → 索引」新「未能索引的文件」区块，7-04 修复报告遗留项全部消化）。**剩验收第二条严格口径**：至少一场景在用户真实内网/归档目录走通并回填 `docs/reviews/beta-40-<场景>-evidence.md`；本机合成材料证据不替代真实内网证据） | apps/daemon + docs/ | BETA-32, BETA-36, BETA-41 | ~~3-5d~~ 文档面 0.5 天；本机 smoke + 语义底座 + eval 自动化 done；真实内网证据待用户 |
| **BETA-41** | **企业场景评测语料 fixture（三场景合成语料 + 检索 query 子集，BETA-35/37/38 共同验收基线）** | done（2026-07-02，详见[归档](docs/session-logs/ROADMAP-archive-2026-07.md#beta-41)）。2026-07-03 Codex 另补独立原始材料包 [test-materials/enterprise-scenarios-raw](test-materials/enterprise-scenarios-raw/) 与真实格式生成脚本 [generate_enterprise_real_format_materials.py](scripts/generate_enterprise_real_format_materials.py)，覆盖 DOCX/PPTX/XLSX/PDF/JPG/PNG/扫描 PDF/EML/MD/TXT；本轮报告指出 PDF/JPG/PNG/OCR 落库仍需专项。 | packages/evals + test-materials/ | BETA-13 | ~~1w~~ → 1 天（AI 全程）；扩展材料 0.5 天 |
| **BETA-43** | **MCP 出处/权限闸门先导（V10-16 先导提前：结果强制带出处 / 禁全文读取时片段级返回 / 审计导出合规报告）** | **done（2026-07-04）**：四条验收全达——① `search` 命中新增 `snippet`（关键词上下文窗口，字符级匹配避开 trigram 2 字 CJK 盲区）+ `pages`（复用 BETA-35 `document_passages` 命中回页）；② 新 MCP tool `read_document` + `CollectionConfig.allow_full_read`（TOML 缺省 false 禁全文 / legacy 合成 true）——禁全文时 `full=true` → Denied + audit denied 留痕、片段模式仅返回命中窗口 + 页级摘录、内容全取索引 db 不触磁盘原文件；③ `GET /admin/audit/report?format=md\|csv&subject=&collection=&from=&to=`（audit.jsonl → 人读合规报告，不要求客户 parse jsonl）+ audit 扩 `read` 动作 / `path` / `read_mode` 字段；④ e2e ×3 覆盖出处字段完整性 / 片段模式不吐全文 + read 留痕 / 禁全文拒绝 + denied 留痕 + 报告导出（非 admin 403 / 坏 format 400）。途中修复 search 结果 `\\?\` 规范化路径查库落空（镜像 desktop `lookup_candidates`）。server 88 / daemon 8+e2e 13 全绿；扫描件页级定位真机验证归入 BETA-33 cycle 9 清单（playbook 验证清单第 8/9 条）。V10-16 主卡保留隐私 UI 集成与全量策略收口。 | packages/locifind-server + apps/daemon | BETA-36, BETA-40 | ~~3-5d~~ → 1 天（AI 全程） |
| **BETA-44** | **enterprise eval 扩容 22 → 50 case（越权负样本 / 跨语言别名 / 近重复干扰 / 低清复扫件优先；不为凑数加新格式）** | **done（2026-07-04）**：queries.tsv 扩至 **53 case**、真实模型（embeddinggemma-300m + 当日 BETA-43 代码 locifindd）首跑 **53/53 全过 `--require-all`**——正样本绝大多数 top-1、最深第 3 位；11 条越权负样本缺省检索零跨集合泄漏 + 显式越权全拒；既有 22 case 排名与 baseline 一致零回归。新增 31 case：越权负样本 +8（法务/审计/HR/技术继任跨 subject 矩阵 + 同场景利益冲突墙）/ 跨语言别名 +4（Northridge/Northfield/Morningstar/Project Orion）/ 近重复干扰 +4（草稿 vs 签署版按措辞取对版本 + 版本对全召回）/ 低清复扫 +3 / 零覆盖材料消化 12（li.si 首获正样本）；新增合成材料 4 份（签署版和解协议 / 北原尽调清单 / 银行回单与 NDA 低清复扫）。`.msg` 仍挂 BETA-37b、PST 不做（验收 ③）；`enterprise_scenarios_gate` fixture 完整性测试零改动对 53 case 全过——闸门随 TSV 自然生长（验收 ④）。报告：[beta-44-enterprise-eval-expansion-2026-07-04.md](docs/reviews/beta-44-enterprise-eval-expansion-2026-07-04.md)。真实语料 case 随设计伙伴落地滚动补充。**2026-07-07 II 追加：离线闸门加固**——`Expectation::AccessDenied` 携机读墙目标（queries.tsv 11 条越权改 `ACCESS_DENIED:<相对路径>`）+ `enterprise_scenarios_gate` 两条常跑断言（每声明 collection 有 ≥1 case 演练〔无死 collection〕+ 每条越权墙目标非空洞〔真实存在且落未授权 collection〕）；运行期不消费墙目标、真机 `--require-all` 零回归；lib 67 / gate 6 全绿。把「信息墙有没有真被测到」从人工审阅转为常跑 CI 机器可查。 | packages/evals + test-materials/ | BETA-41 | ~~2-4d~~ → 0.5 天（AI 全程） |
| **BETA-53** | **桌面「本机 MCP 服务」开关（BETA-32 个人变体：本机索引经 MCP 暴露给本机 LLM 客户端）** | **code-done（2026-07-07 V）；剩真机验证**：用户诉求「让 Claude Code 经 MCP 检索本机文件」→ 澄清为 BETA-32 headless 检索的**个人单机变体**（非 BETA-43 企业信息墙——单库无 collection/token）。设计 [desktop-local-mcp-service-design.md](docs/reviews/desktop-local-mcp-service-design.md)：**内嵌**（非起子进程）`packages/locifind-server` 复用桌面已加载检索栈、**只读挂载**桌面 index.db（零重索引、语义白送）、工具菜单开关 + `127.0.0.1:8766` + 随机 token；安全红线只绑 127.0.0.1 + token 必填 + 暴露面知情。**S1-S3 code-done（2026-07-07 V）**。**S1**：`ServerCtx::attach_readonly(config, embedder)`（开现有 db 不跑首索引、按 embedder 探针挂语义臂、读现有 doc_count）+ 单测。**S2**：server 加 `DaemonConfigFile::personal_local(roots, token)`（多 root 桌面变体、全权 admin、allow_full_read）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内、真 socket 起停集成测试）；桌面 `mcp_service.rs`——`McpServiceState` + `start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token` 四命令，复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`（同步拿端口占用错误）、随机 64-hex token、oneshot 优雅关停、开关态+token 持久化 settings.json、enabled 时自启。**S3**：`preferences/McpPane.tsx`（开关 / 运行状态〔地址·挂载条数·语义臂〕/ token 复制 / Claude Code 配置片段复制 / 重置令牌 / 安全提示）+ 工具菜单入口〔`open-prefs-mcp`〕+ 选项页第八 tab。安全红线：只绑 127.0.0.1 + token 必填随机 + 暴露面 UI 明示 + 重置即踢连接。验证：server lib 93 pass（+2：personal_local / serve_bound 端到端）/ desktop 174 pass（+3：token·roots·status）/ clippy `-D warnings`〔server·desktop·daemon〕/ fmt / tsc+vite 全绿。剩：**真机验证**（带 semantic-recall 构建 + Claude Code 实连跑 search/read_document round-trip、依赖用户；照 [验证 playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。途中修 daemon 正斜杠 root → `read_document` round-trip bug（root 入口 `normalize_root` 归一原生分隔符 + 单测，commit 9b55a1c）。 | packages/locifind-server + apps/desktop | BETA-32, BETA-36 | S1-S3 done；剩真机验证 |

**BETA-35 验收**：① 图片型 PDF 页渲染 → OCR → 可检索；② 页码/来源映射保留（命中能回到具体页，取证可用）；③ 失败页记录不静默丢；④ 命中预览可展示 OCR 段落。BETA-41 扫描 PDF 子集命中率进 evals report；文本层 PDF 路径 byte-equal 不回归。

**BETA-36 验收**：① bearer token 升级为 per-collection / per-root 权限模型（常数时间比较沿用）；② collection 概念落地：root 分组、归档主体（案件/员工/审计项目）边界、显示名、只读态、审计标签——否则 ACL 只能按路径打补丁；③ audit 留痕含 subject（谁查了什么）；④ 越权访问返 403 + audit 记录，e2e 覆盖。**→ 2026-07-03 落地**：四条全达——① 多 token 逐条 `subtle::ct_eq` + `CollectionGrant`（`"*"`/列表）；② `CollectionConfig`（id/display_name/subject_kind/roots 多根/read_only/audit_tags）+ per-collection 独立 index.db 物理信息墙；③ `audit.jsonl` 每条含 subject（token→subject 映射），query 明文默认开、`[audit] log_query=false` 降级；④ REST 非 admin 403 + MCP 越权 tool error（未知/未授权同文案防探测）都留 denied 记录，e2e `e2e_infowall_search_scoped_and_denied` / `e2e_admin_403_and_audit_trail` / `e2e_read_only_collection_reindex_409` 覆盖。

**BETA-37 验收**：eml/msg 进 DOC_EXTS + 提取器（正文 + from/to/date/subject headers 基础字段 + 附件递交现有提取管线）；**pst 明确不在本卡范围**（后置，防审计取证场景误期望）；BETA-41 邮件子集命中率进 evals。**→ 2026-07-02 落地**：eml 全链路 done（headers 零 schema 变更映射 + 头块进 body；附件深度 1 并入 body）；msg 拆 BETA-37b 后置（spec §7 Q2 用户拍板）；email/attachment 桶命中率待 enterprise 向量 bootstrap（Mac）后由 `semantic_quality --fixture-set enterprise` 出报告。

**BETA-38 验收**：① 十万级文档向量检索水位基准（p95 延迟 + 内存）对比现暴力扫描（**cycle 4 达成**——实测 10万×1024：暴力全量重载 p95 ~900ms vs 进程级缓存 ~174ms、5×+，常驻向量 ~390MB；缓存后转 cosine 计算受限、sub-second 交互可用，未触及 ANN 门槛；[报告](./docs/reviews/beta-38-scaling-benchmark.md)）；② doc identity 策略定义并落库（**cycle 1-3 达成**——文件原始字节 FNV-1a hash 落 `documents.content_hash`，副本关系 SQL 可还原 `SELECT path FROM documents WHERE content_hash=?` 供审计取证，索引期同身份只嵌一次不丢任何 path、结果期合并不被副本刷屏）；③ 现有语义召回质量 evals 不回归（缓存/去重不改 cosine 语义，仅合并真副本；indexer/semantic-index/desktop/server 全测零回归）。

**BETA-39 验收**：设置项 opt-in（默认关）；开启后图片走双层质量门槛（沿用 A 层 meaningful_ratio + 段级门槛）入语义索引；关闭时行为与 BETA-33 cycle 4 现状 byte-equal；已知污染 case（QQ 表情包乱码 OCR）仍被挡。**→ 2026-07-03 落地**：四条全达——① `AppSettings.enable_image_semantics` 默认 false + 「选项→索引」pane checkbox；② 文档级门槛 = 字数 20 + 图片专属 ratio 0.75（`is_image_embed_worthy`），段级门槛 = `explain_passages_with_ratio(0.75)`（双层）；③ 三处调用点（`embed_pending` / `purge_short_body_vectors` / `explain_semantic_hit_impl`）默认参数路径零改动 + `embed_pending_embed_images_branches` / `purge_keep_worthy_images_branches` 断言关闭态 byte-equal；④ 单测 `is_image_embed_worthy_blocks_cjk_heavy_noise` + `explain_with_image_ratio_blocks_cjk_heavy_noise_segment` 覆盖 QQ 表情包 case（ratio≈0.63/0.65/0.67）仍被挡。实现修订：purge 的 `keep_worthy_images` 启动期由设置动态判（关→清全部图片向量恢复一刀切态、开→仅清不过 0.75 门槛的），非「关闭后保留已嵌」（守 byte-equal 验收）。

**BETA-40 验收**：三场景 playbook 各一篇（部署拓扑 + 权限配置 + 10 条示例 query + Claude/MCP 客户端工作流示例）；至少一个场景在真机/内网环境走通并留证据。

**BETA-41 验收**：三场景合成语料（扫描 PDF / 邮件 / 附件 / 跨语言别名 / 近重复材料）+ 相关性标注 + query 子集；隐私红线沿用「合成集入仓做 CI 门控」方案（BETA-15B-6 同款）。**→ 2026-07-02 落地**：四条全达——① scenario 三值齐（lawfirm 34 / audit 35 / offboarding 35）；② 五桶各 10 case + 文件层实体文件（9 扫描 PDF 其中判决书 ×3 为文件级近重复、6 eml 其中 2 封带 base64 附件 part、4 近重复 md/txt、1 文本层 PDF 对照守 BETA-27）；③ graded 1-3 标注 + 10 个 dup_group（BETA-38 doc identity 靶）；④ 隐私红线机器可查（`enterprise_recall_fixtures_integrity` 常跑：非 example.com/org 邮箱域名直接 fail）。**命中率报告**待向量 bootstrap（Mac）后由 `semantic_quality --fixture-set enterprise` 出、`enterprise_recall_gate` 同步转真跑；BETA-35 扫描 PDF OCR 端到端 = `real_pdf.rs` `--ignored` 三层测试（常跑二分守卫已本机验证：9 份全进扫描分支、文本层走原路径）。

**BETA-43 验收**：① MCP search 结果强制含出处（collection + path + 页码/段落定位，扫描件能回到页——复用 BETA-35 来源映射）；② collection 级 `allow_full_read` 策略——禁全文时读取类工具仅返回命中片段 + 有限上下文窗口，不吐全文；③ 审计导出：`audit.jsonl` → 人读合规报告（按 subject / collection / 时间范围过滤，md 或 csv），不要求客户自行 parse jsonl；④ e2e 覆盖禁全文越界拒绝与出处字段完整性。

**BETA-44 验收**：① `queries.tsv` 扩至 ≥50 case、`enterprise_scenarios --require-all` 全过；② 新增 case 优先覆盖：越权负样本（HR/法务/审计不同 subject）、跨语言/别名召回、近重复干扰下排名稳定、低清复扫件 OCR；③ `.msg` 相关 case 仍挂 BETA-37b（等真实样本），PST 不做；④ 报告沉淀 `docs/reviews/`，`--require-all` 闸门随 TSV 自然生长。

**B 阶段预期总工期**：8-12 周（B6 / B7 不上关键路径）。

#### B8：cycle 9 真机反馈（2026-07-06 v0.9.15 首测三条，用户拍板逐条登记）

> 来源：用户 v0.9.15 Windows 装机真机测试首批反馈。三项拍板（2026-07-06）：卸载默认保模型可勾选删 / 本地发现后**复制**进默认目录 / ①②当场做、③下会话。不进 §6.3 出场指标。

| ID | 标题 | 状态 | 模块 | 依赖 | 估时 |
|---|---|---|---|---|---|
| **BETA-45** | **模型本地发现 + 卸载默认保留模型**（真机反馈①：重装后被迫重下 ~700MB） | **done（2026-07-06 代码层）**：(a) NSIS 卸载 hook 改「模型默认保留」——MessageBox 询问删否（默认/静默 `/SD IDNO` = 保留）、保留经 models 同卷 Rename 暂存→整删→移回（敏感派生数据零遗漏）、$UpdateMode 守卫不变、闸门测试加 Rename models + `/SD IDNO` 断言；应用内「卸载清理」仍全删含模型（§6.3 指标不受影响）。(b) 下载 UI 前两级本地发现——默认路径已有完整文件（≥100MB）→ 直接就绪跳过下载；否则 everything crate 新公开 `find_files_named`/`es_cli_available`（复用 es.exe 两段式定位 + UTF-8 导出解码）按**精确文件名**全盘发现候选（绝不 `*.gguf` 泛搜、防错模型 ucrtbase abort），「使用此文件」经 `import_local_model` 复制进默认目录（校验文件名白名单 + ≥100MB + `.partial`→rename 原子落盘 + 与下载共用 in-flight 守卫与 done event，前端状态机零改动）。**v0.9.16 真机首验（同日）**：发现 UI 工作（Everything 命中 artifacts/ 下模型）；但暴露**下载卡死链**——HF 连接阶段长挂占满守卫 300s、取消无效（cancel 只在 chunk loop 检查）、前端切步重挂回 idle 与后端守卫脱节（无取消入口）、取消被误报「下载失败」。**修复四刀（v0.9.17 随包）**：tokio::select 取消竞速（连接阶段也即刻生效）+ connect_timeout 15s + **hf-mirror.com 镜像兜底**（PRIVACY 同步）+ 新命令 model_download_in_flight（前端 mount 恢复下载中态）+ invoke 拒绝路径过滤取消 reason；测试 +2。另：everything crate 引用忘加平台 gate 致 v0.9.16 macOS CI E0433、已加 shim（cfg windows 对 + 非 Windows 恒降级）。**卸载保模型弹窗待真机验证** | apps/desktop + packages/search-backends/everything | — | 0.5d |
| **BETA-46** | **默认零索引 + 目录列表 UX**（真机反馈②：未经同意不应索引系统目录；路径截断看不全） | **done（2026-07-06 代码层）**：`resolve_index_roots_tagged` 新语义——系统三夹**仅当 `include_system_defaults=true`** 时纳入、与 `index_roots` 空否解耦（空+false = **零索引**）；checkbox 常显（覆盖语义 banner 退役）；onboarding Step 5 / 设置页空态文案改「默认不索引、请添加」；目录路径退役 ellipsis 截断改完整显示（自然换行 + title hover）。settings 测试改四分支 + desktop 168 全过。**行为变化注意**：旧装机若 index_roots 为空，升级后停止索引系统三夹（需重新勾选或添加目录）——beta 阶段接受、随 cycle 9 复测确认。**v0.9.16 真机首验（同日）**：零索引/checkbox 常显生效；但路径换行修法在单行 flex 下被统计列+按钮挤成逐字断行——**二轮改三行卡片式**（行1 完整路径+标签 / 行2 统计+上次索引 / 行3 操作按钮，卡片下边框分隔，v0.9.17 随包）。**空态提示与升级行为待复测** | apps/desktop | — | 0.5d |
| **BETA-47** | **选项页重构**（真机反馈③：拆 tab = 常规 / 索引（本地索引配置）/ Everything（检测+开关）/ 语义召回（含模型下载与管理）/ Windows（系统集成）/ 隐私与记录 / 杂项；PreferencesDialog.tsx 1579 行拆文件） | **done（2026-07-06 代码层）**：(a) `enable_everything` 设置（默认开、旧配置 serde default 零回归）+ **三处 es.exe 调用点全门控**——搜索后端条件注册（关闭需重启，与 model_path 口径一致）、索引期音乐全盘发现（live-read；local-index 新 `reindex_scoped_with_filter_progress_and_discovery` 变体、`false` 时跳过发现器直走 MusicScan + phase 级回归测试；macOS Spotlight 发现不受控）、BETA-45 模型本地发现（live-read、关闭时 `everything_available=false` 零候选）；新命令 `check_everything_available`（检测与开关独立、非 Windows cfg shim 恒 false〔v0.9.16 E0433 口径〕）。(b) 七 tab 落地，Everything（检测 3s 轮询 + 开关 + winget/官网安装引导）与 Windows（WSearch 检测 + 打开系统索引选项）两 tab 仅 Windows 显示（`navigator.platform`）；**模型管理归位**：生成模型 fallback/状态/下载/路径覆盖从「常规」迁入「语义召回」；杂项收同义词入口、隐私与记录补完整隐私面板入口（跳转走未保存守卫）。(c) PreferencesDialog.tsx **1579→513 行**，面板拆 `components/preferences/` 九文件（shared/ConfirmModal/General/Indexing/Everything/Semantic/Windows/Privacy/Misc）。desktop 171 / local-index 24 全过 + tsc/vite/clippy/fmt 净。**真机待验**：Everything tab 两态 + 开关关闭后行为（随下次发版） | apps/desktop + packages/search-backends/local-index | BETA-45, BETA-46 | 1-2d |
| **BETA-48** | **前端 AppSettings 缺 `embedding_model_path` 字段**（BETA-47 会话顺带发现：Rust `AppSettings` 有该字段而前端 interface 没有，`update_settings` 全量覆写时 serde default 会把用户手工写进 settings.json 的该值静默冲掉） | **done（2026-07-06）**：前端 interface 补字段透传 + 「语义召回」tab 一并暴露「语义模型路径覆盖」输入框（与生成模型路径覆盖对称、placeholder 标默认 `embeddinggemma-300m-q8_0.gguf` 路径）。tsc/vite 过 | apps/desktop | — | 0.5d |
| **BETA-50** | **OCR 数字校正变体**（2026-07-06 真机踩坑：用户搜 `150138` 找不到准考证 PNG——诊断实锤 Windows OCR 把 `15013866763` 识成 `1 S013866763`〔5→S + 空格拆组〕，图已入库、trigram 正常、正确号码在索引里不存在） | **done（2026-07-06 同轮沉淀）**：indexer `digit_correction_variants`（易错字母经典五对 S→5/O→0/I·l→1/B→8/Z→2 + 跨单空格分组合并如 `789 803 810`→`789803810`；保守规则宁漏勿误——真数字 ≥4 且易错 ≤2、纯数字须分组 ≥2 且 ≥6 位、链 ≤64 字符、变体 ≤16 条去重）+ `finalize_ocr_text` 统一收口（normalize 后**原文保留**、变体以〔OCR数字校正〕行追加正文尾——预览可见即解释"为什么命中"、trigram 子串两态可搜）；WinRT/Tesseract 两引擎与扫描 PDF 逐页管线（`recognize` choke point）自动共享。测试：真机 case 四连（手机号拆组/`1234S6`/会议号合并/身份证紧邻链）+ 保守反例五连 + 去重上限 + doc_db FTS e2e（`150138`/`15013866763`/`S013866763` 均命中）。indexer 182 全过、local-index/desktop/server 零回归。**生效条件**：存量图片 mtime skip、需清空索引重建；locifindd 下次构建重编 | packages/indexer | — | 0.5d |
| **BETA-49** | **音乐发现按 roots 过滤**（BETA-47 会话顺带发现 + 2026-07-06 用户拍板**方案 A**：Everything/Spotlight 全盘发现与 BETA-46「默认零索引、未经同意不索引」语义冲突——发现结果越界入库、空 music_roots 也 spawn es.exe 全盘枚举） | **done（2026-07-06）**：local-index 三处发现分支统一——① 发现结果经 `filter_discovered_to_roots` 按生效音乐 roots 过滤后才 `index_paths`（Windows 大小写/分隔符归一、root+分隔符判界防 `D:\Music2` 误挂 `D:\Music`；发现器纯做加速，收录盘外音乐 = 把该目录加进索引目录）；② **roots 为空直接跳过发现器**（零索引不 spawn es.exe，顺带消除单测/「文档-only 重建」路径的全盘枚举）。旧库越界记录不主动清（沿用「生效目录之外 N 条」UI 提示 + 移除目录 purge 口径）。行为变更测试面：改写 `reindex_uses_discovery_paths_filtered_to_roots` + 计数 mock（空 roots 零调用）+ 纯函数边界测试；UI 文案「全盘发现」→「快速发现（仅限所选目录）」。local-index 26 全过 + desktop 全量 exit 0 + clippy/fmt 净。**遗留（低优）**：发现路径不经 exclude_globs/per-root 排除（root 内 `node_modules` 下音频经发现器仍入库、目录扫描则剪枝——罕见场景、暂不修） | packages/search-backends/local-index + apps/desktop | BETA-46 | 0.5d |

| **BETA-51** | **设置统一入口**（v0.9.18 后真机反馈：「我的同义词」设置整页进入后无关闭/返回入口、回不到搜索主界面；同类独立设置页应统一收进选项对话框） | **done（2026-07-07，随 v0.9.19）**：「我的同义词」`/synonyms` + 「隐私与数据」`/privacy` 两独立整页整体收编进选项对话框 tab——新增 `SynonymsPane`（内联「杂项」tab、增删改+导入/导出）+ `PrivacyPane` 折叠完整隐私内容（索引概览/数据位置/一键清除/卸载清理，与操作记录同 tab）；删两路由与两页文件（App.tsx）、`handleNavigate`/`useNavigate` 退役；工具菜单「我的同义词/隐私与数据」改 `open-prefs-misc`/`open-prefs-privacy` 事件打开对应 tab。tsc/vite 净、desktop 171 测试零回归。**真机待验**：菜单/设置进出可正常返回 | apps/desktop | — | 0.5d |
| **BETA-52** | **语义召回模型管理增强**（v0.9.18 后真机反馈：语义召回只显示"已就绪"、看不到实际用哪个模型；需可指定+检测可用性+自动发现，为切换更强本地/局域网可信模型铺路） | **done（2026-07-07，随 v0.9.19）**：① `EmbedStatus::Ready` 携 `active_path`（状态行显示「已就绪（当前模型：xxx.gguf）」；生成模型 detail 本就含路径）；② 新命令 `probe_model_file`（纯文件校验：存在/gguf 后缀/体积下限，不加载）+ 语义/生成两路径覆盖框「检测」按钮；③ 新命令 `discover_gguf_models`（everything crate 新 `find_files_by_extension`、`ext:gguf` 全盘发现 + 8MB 下限 + 体积降序 + 上限 60）+「扫描本机 gguf 模型」列表每项「设为语义/设为生成」回填路径覆盖并自动检测——**只回填不复制不加载**（错架构误载可能 crash、交用户判断+检测+重启验真）；非 Windows/Everything 关时提示手动填写。clippy `-D warnings`（修 `unnecessary_sort_by`）/tsc/vite/171 测试 全绿。**真机待验**：当前模型显示 / 检测 / 自动发现列表 | apps/desktop + packages/search-backends/everything | — | 1d |

#### 代码整洁 / 技术债 backlog（非关键路径，随手可做）

> 2026-06-03 "消冗余 + 上下文优化"梳理产生的可选后续项（均不阻塞功能）。已完成部分见 STATUS 同日会话日志 + [docs/session-logs/](../docs/session-logs/)。

| ID | 内容 | 风险 | 状态 |
|---|---|---|---|
| CLEAN-1 | search.rs 代码层进一步拆子模块（`file_actions` / `index_status` / `fanout` 等；当前已拆出 `search/tests.rs`，主文件 1558 行） | 低-中（纯重构、需调可见性 + 补 import，churn 大） | **done（2026-06-03）**：search.rs **1558 → 637 行**，按签名/区块整体迁出三子模块——`fanout.rs`(341，BETA-04/18/19 多源+均衡)、`file_actions.rs`(495，open/locate/confirm/cancel + record_audit)、`index_status.rs`(130，BETA-07 reindex/状态)。`#[tauri::command]` 包装 + main.rs 所引类型（SearchDeps/ReindexStats/IndexStatus/perform_reindex）留 search.rs，避免 tauri 命令宏跨模块失效；迁出项提 `pub(crate)` + parent `pub(crate) use ::*` 重导出，使 `super::*`(tests) 与 `search::X`(main) 均解析；中置 `use TargetRefError/FileActionError` 随用迁入。逻辑零改动（仅移动 + 可见性/导入调整）。desktop 72 单测全过、全 workspace clippy(`-D warnings`)+test 零回归（platform-macos 2 预存除外）。 |
| CLEAN-2 | 翻译层重复收拢到 common：`relative_time_bounds`（spotlight + windows-search）、`media_derived_file_types` + `media_common_constraints`（everything + windows-search） | 中（需三后端翻译单测护航，注意各后端时间/单位语法差异） | **done（2026-06-03）**：三函数 + 顺带发现的 `CommonConstraints` 结构体（三后端各一份完全相同）收拢到 common 单一信源，后端 `use` 引入；语法相关的 `add_*_constraints`（CommandBuilder/SqlBuilder/QueryBuilder）留原处不可合并。清理孤儿 import（`Location`/`RelativeTime`）。全 workspace clippy(`-D warnings`)+test 零回归（platform-macos 2 预存除外），后端 fixture 翻译单测全过。 |
| CLEAN-3 | model-runtime 纳入 workspace lints（补 `[lints] workspace = true`，修可能浮现的 warning） | 低 | **done（2026-06-03）**：加 `[lints] workspace = true`，修浮现的 16 类 warning（`missing_debug_implementations` 手写/derive、`needless_pass_by_value` → `ModelLoadParams` 加 `Copy`、`uninlined_format_args`、`must_use`、`doc_markdown`、`float_cmp`/`#[ignore]` reason/测试模块 allow）。clippy(`-D warnings`) 0。 |
| CLEAN-4 | 后端测试里重复的 `noop_waker + block_on` 抽公共 test helper（或改用 `futures_executor::block_on`） | 低 | **done（2026-06-03）**：7 处逐字节相同的手写 `block_on`（三后端各 1 + harness 4）全删，改用 `futures-executor::block_on`（dev-dependency，同 futures-rs 上游、零新传递依赖）。licenses 文档已登记。 |
| CLEAN-5 | 抑制被 `catch_unwind` 兜底的提取器 panic 的 stderr 刷屏（BETA-07 启动自动索引时 pdf-extract 0.10 对畸形 PDF panic 约 10 条/轮，污染 dev 日志；panic 已计 failed 不崩，仅打印噪声） | 低（线程局部标志 + 一次性 panic hook，零新依赖、不动 unsafe 约束） | **done（2026-06-03）**：scan.rs `catch_extract` 内置位线程局部 `IN_CATCH_EXTRACT`，`install_quiet_extract_panic_hook`（`Once` 进程级一次安装）的 hook 仅在该标志置位时跳过默认 hook 打印、其余 panic 照常打印。顺序路径 + rayon 并行路径均成立（panic 与置位同线程）。新增标志复位单测；indexer 76 单测全过、全 workspace clippy(`-D warnings`)+test 零回归（platform-macos 2 预存除外）。**后续选项（②，未做）**：换更鲁棒的 PDF 文本提取以**降低 panic 率本身**——候选 `lopdf` 直读（纯 Rust、轻、可控但需自写文本流解析）/ `pdfium-render`（覆盖最广但引入 pdfium 原生大依赖，与"无新重依赖优先"冲突）/ 跟进 `pdf-extract` 上游修复版本。评估门槛：畸形 PDF panic 率、二进制体积、是否破 `unsafe_code=forbid`。当前噪声已消、坏 PDF 仍计 failed 不进 index.db，故 ② 仅"提升覆盖率"边际收益，非紧急。 |
| CLEAN-6 | **ROADMAP 已完成 task 卡片压缩归档**（2026-07-02 登记：本文件已膨胀至 ~222KB、pre-commit hook 230KB 预警线就位。修法 = P/M 阶段与 B 阶段已 done 的巨型卡片压缩为"一行摘要 + 归档链接"，全文逐字移入 `docs/session-logs/ROADMAP-archive-*.md`；验收 = 压缩后 ROADMAP < 120KB、所有 task ID 仍可检索、归档只移动不删除） | 低（纯文档移动，需保 task ID 可检索） | **done（2026-07-03 Claude Code）**：62 张 done 巨型卡片行压缩为「ID + 标题 + done 摘要 + 归档链接」一行（模块/依赖/估时列保留）+ BETA-15B 父卡（not_started）内嵌的 15B-1..11-v2 全 done 子周期日志压为逐 ID 一行摘要 + 21 处 done 任务历史 background 注记搬迁，全文逐字入 [`docs/session-logs/ROADMAP-archive-2026-07.md`](docs/session-logs/ROADMAP-archive-2026-07.md)。**ROADMAP 242041 → 117234 字节（-51.6%，< 120KB 验收达成）**；160 个 task ID 全部前后可 grep（in_progress BETA-33/13-G12/13-G14/40 与 not_started 卡未动；BETA-16/16A、BETA-31-v2/-v4、15B-Y 等 done 卡内嵌引用随母卡进归档仍可 grep）。归档只移动不删除、历史一字不丢。 |

> 已完成（2026-06-03）：跨后端重复小函数（`validate_search_path`/`is_excluded`/`result_id`）+ 图片扩展名白名单（indexer/local-index/desktop 三副本）收拢到单一信源；删未用依赖（tokio/tracing/anyhow）+ 死代码 echo；STATUS 369KB→14KB + ROADMAP 定向读取。**注**：indexer 索引侧扩展名白名单与 common 搜索侧 `extensions_for_file_type` **语义不同、不可合并**（经 Codex 交叉验证确认）。

### 3.4 V 阶段：1.0

目标：**正式发布所需的最后一公里**。

| ID | 标题 | 状态 | 模块 | 估时 |
|---|---|---|---|---|
| V10-01 | 插件系统（Plugin SDK） | not_started | packages/harness + docs | 3 weeks |
| V10-02 | 本地活动洞察模块（**工作时间分布 + 工作主题摘要**；最近打开如已在 BETA-04 下沉则此处不再实现） | not_started | packages/indexer + apps/desktop | 3 weeks |
| V10-03 | 隐私 / 权限管理 UI | not_started | apps/desktop | 2 weeks |
| V10-04 | 自动更新机制（Sparkle / Squirrel） | not_started | platform/* | 2 weeks |
| V10-05 | 崩溃恢复 | not_started | apps/desktop | 1 week |
| V10-06 | 多语言扩展（界面 i18n） | not_started | apps/desktop | 1 week |
| V10-07 | 企业管理策略（如需要） | not_started | apps/desktop + docs | 2 weeks |
| V10-08 | 开源法务文档完整化（LICENSE 双许可 / Third-party Notices / 隐私说明；原律师版 Privacy / Terms / EULA 已随 2026-07-04 开源免费拍板取消） | in_progress（LICENSE 已入库 2026-07-04） | docs | 数天（大幅缩量） |
| V10-09 | ~~商标注册完成（中美）~~ | **dropped**（2026-07-04 开源免费拍板：不注册商标，撞名风险接受、全程用完整品牌名） | — | — |
| V10-10 | 官网与用户文档 | not_started | （仓库外） | 持续 |
| V10-11 | 1.0 发布 evals + 性能基准 | not_started | packages/evals | 2 weeks |
| V10-12 | 1.0 正式发布 | not_started | — | — |
| V10-13 | **MCP 文件问答工作流（re-scoped：不自建 RAG UI / 本地模型作答；检索+问答经 BETA-32 daemon + 外部 LLM 客户端组合实现，本卡并入 BETA-40 playbook）** | **re-scoped → 并入 BETA-40**（2026-07-02 定位收敛「不做分析层」，详 [doc-realign 方案](./docs/reviews/doc-realign-retrieval-foundation.md)；原 2026-06-03"RAG over local results 本地作答"方案作废） | docs + apps/daemon | 并入 BETA-40 |
| V10-14 | **通用自定义规则过滤（power-user：正则 + 熵 + 路径 + 扩展名，自助式内容/文件名模式匹配）** | not_started（2026-06-15 登记，源于"能否找 AWS 凭证"讨论；**不现在动手、不挤占进取档 BETA-15B**）。**定位=通用过滤器,非 secret scanner**：让 power-user 自定义规则指向自己关心的模式;**不对外宣称"找密码/凭证"**——那是另一个 job(模式审计 vs 按意思找文件)、差异化弱且规则维护重,该用 `gitleaks`/`trufflehog`/`detect-secrets` 专业工具补位。**隐私硬约束(必守)**：① 命中敏感路径(dotfile/`.env`/源码/`.pem`)需**显式 opt-in + 二次确认**(当前 indexer 特意跳过这些);② 命中**只存位置+规则名,绝不存明文密钥/密码**(避免 index.db 变成第二份泄漏面);③ 该类规则索引可独立清除;④ 守 local-first 不外发。**边界**：本质是 secret-scanning 的近邻,扩张前须过 PROJECT.md「防范围蔓延」判定。 | apps/desktop + packages/indexer + packages/intent-parser | BETA-22(保存搜索同源) | 重估中（非关键路径） |
| V10-15 | **Frozen Index Pack（re-scoped：冻结检索包，无内置 LLM 合成——显式 pin 资料夹的冻结快照 + 原文索引 + 来源映射 + 文件/段落 ID + mtime/hash 失效检测 + 可导出给 MCP daemon 的检索上下文）** | **re-scoped**（2026-07-02 定位收敛「不做分析层」：原 Frozen Research Pack 的 LLM 合成部分——摘要 / 术语表 / 阅读地图——**外置**给外部 LLM 经 MCP 工作流生成，不内置；「冷归档可复现、可留痕、可交接」价值保留。原 2026-06-25 登记背景与 LLM Wiki 借鉴见 [doc-realign 方案](./docs/reviews/doc-realign-retrieval-foundation.md) §6 Q2） | apps/desktop + packages/search-backends/semantic-index + packages/harness | V10-16, BETA-36 | 2-3 weeks（合成外置后缩量） |
| V10-16 | **MCP/LLM 读取权限与出处闸门（所有经 daemon/MCP 暴露给外部 LLM 的读取必过策略：哪些 collection/目录可读 / 是否允许全文 / 答案必须带出处引用）** | not_started（2026-06-25 登记；**2026-07-02 重定性**：从"本地 LLM 功能护栏"改为"**MCP 路径横切护栏**"、价值上升——律所信息墙 / 审计留痕 / 离职归档 HR 敏感的企业准入门槛。**衔接**：BETA-06 Audit、BETA-36 collection ACL、BETA-40 playbook、V10-03 隐私 UI。**缓解**：per-root/per-collection 策略（敏感目录默认禁）+ 答案强制出处引用。**2026-07-04 护城河规划**：先导部分（出处强制 / 片段级返回 / 审计导出）提前拆 **BETA-43** 进 B7，本卡保留隐私 UI 集成（V10-03）与全量策略收口，详 [moat-plan-2026-07-04.md](./docs/reviews/moat-plan-2026-07-04.md)） | packages/locifind-server + apps/daemon + packages/harness | BETA-36, BETA-40, V10-03 | 2 weeks（BETA-43 先导拆出后缩量） |

**V 阶段预期总工期**：4-6 个月。

## 4. 里程碑

| 里程碑 | 触发条件 | 价值 |
|---|---|---|
| **M0：设计定稿** | Schema v1.0 + Trait v0.1 + Codex 审阅落地 | 内部对齐技术路径（**已完成 2026-05-25**） |
| **M1：原型 demo** | PROTO-09 通过 | 内部演示，验证"AI Search for humans"概念 |
| **M2：双平台 MVP** | MVP-28 通过 | 早期用户内测；可邀请 5-10 人试用 |
| **M3：Beta 内测** | BETA-14 通过；安装包经 GitHub Releases 外发（2026-07-04 开源口径，原「签名安装包」作废） | 50-100 人公开测试；收集真实查询样本（脱敏） |
| **M4：1.0 公开发布** | V10-12 | 正式发布；仓库主页 / GitHub Pages 上线（商店提交已随开源免费拍板取消——需开发者账号） |

## 5. 长周期事项时间线（独立于代码节奏）

| 事项 | 启动时间 | 完成时间 | 启动负责 | 说明 |
|---|---|---|---|---|
| ~~注册 Apple Developer Program~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：不签名不公证，DMG 走 GitHub Releases + Gatekeeper 绕行文档（BETA-10 re-scoped） |
| ~~采购 Windows OV/EV 代码签名证书~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：接受 SmartScreen 未知发布者提示 + 说明文档（BETA-10A re-scoped） |
| ~~注册核心域名（locifind.ai / .app / .dev）~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：GitHub 仓库 + GitHub Pages 足够 |
| ~~提交 LociFind 商标申请（中国 + 美国）~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：撞名风险接受；全程用完整品牌名 LociFind 降低风险 |
| ~~评估第二梯队商标申请~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：随商标申请一并取消 |
| 建立 Third-party Notices 台账 | **PROTO-01 起强制**（引入第一个依赖时） | 持续维护 | 任一工具 | 引入 / 移除依赖时**当场**更新 [docs/third-party-licenses.md](./docs/third-party-licenses.md)；收工时检查"新依赖是否登记"作为 commit 前置 |
| 简短隐私说明 + LICENSE（原「Privacy Policy / EULA 草案」，2026-07-04 开源免费拍板 re-scoped：律师版 EULA 取消） | Beta 外发前 | 数天 | 任一工具 + 用户 | LICENSE-MIT / LICENSE-APACHE 已入库（2026-07-04）；隐私说明归 BETA-00 |
| ~~Apple Notarization 流程演练~~ | — | — | — | **已取消（2026-07-04 开源免费拍板）**：不公证 |
| **获取设计伙伴 / 首个真实部署**（律所 / 审计 / 离职归档任一场景） | **B7 收尾期（2026-07 起）** | 持续 | 用户 | 护城河 P0（详 [moat-plan-2026-07-04.md](./docs/reviews/moat-plan-2026-07-04.md)）：BETA-40 真实内网证据、BETA-44 真实语料、场景词表积累均以此为前提；性质是**主动获取**而非被动等待环境 |

> 这些事项**不阻塞代码进度**但**阻塞分发**。Beta 外发前必须全部就绪。

## 6. 度量指标（按阶段）

> 由 Codex 审阅 must-fix #6 落地：每个性能 / 准确率指标必须明确**四要素 — 数据集 / 统计口径 / 运行环境 / 排除项**。
> 由 Gemini 审阅 should-have #3 落地：性能指标必须明确**硬件画像**。

**统一基准硬件画像**（除非另注，下列性能指标均基于此画像）：

- **macOS 基准机**：Apple Silicon M 系列（M1/M2/M3 任一），16GB RAM，512GB SSD，macOS 14+，Spotlight 索引已完成
- **Windows 基准机**：x86-64 8 核 CPU（Intel 12 代 / Ryzen 5000 级或更新），16GB RAM，512GB SSD，Windows 11，Windows Search 索引已完成

低于基准画像的硬件不进入出场判定（但应在 issue tracker 记录"低配机器观测数据"）。

### 6.1 P 阶段出场（原型）

| 指标 | 阈值 | 数据集 | 统计口径 | 排除项 |
|---|---|---|---|---|
| 端到端准确率（NL → SearchIntent JSON → 结果集） | ≥ 80% | PROTO-08 v0.1 evals（47 条 + PROTO-05A 合成 fixture） | 严格匹配判定（intent 正确 + 关键字段一致） | — |
| 规则解析路径响应（NL → SearchIntent） | p95 < 500ms | 47 条用例 | p95，不含 mdfind 执行 | 冷启动、首次 JIT |
| CLI 端到端简单查询响应 | p95 < 1500ms | §7.1-§7.4 共 30 条 | p95 | 冷启动、Spotlight 首次索引 |
| 在真实 macOS 14+ 环境运行无 panic | 100% | PROTO-09 烟雾测试 | 无 panic / 无 unwrap 触发 | — |
| [trait §4.2](./docs/search-backend-trait.md) 实测验证清单 | 全勾选 | trait §4.2 | 逐项确认 | — |
| Schema §3.5 Clarify 触发规则单元测试 | 100% 通过 | 触发规则表 | — | — |
| Stub backend 不进入生产 fallback 链 | 集成测试断言通过 | 测试用例 | — | — |

### 6.2 M 阶段出场（MVP）

| 指标 | 阈值 | 数据集 | 统计口径 | 运行环境 | 排除项 |
|---|---|---|---|---|---|
| 简单文件搜索生成合法 SearchIntent JSON | ≥ 90% | MVP-25 v0.5 evals（500 条） | schema 校验通过率 | 双平台 | — |
| 中文 / 英文 / 中英混合查询解析正确 | ≥ 85%（各语言子集分别） | MVP-25 按语言分桶 | 严格匹配 | 双平台 | — |
| 简单查询响应（规则解析路径） | p95 < 500ms | MVP-25 标记为 simple 的用例 | p95 | 基准画像 | 冷启动 |
| 复杂查询响应（含模型 fallback） | p95 < 3000ms | MVP-25 标记为 model 的用例 | p95 | 基准画像，模型常驻 | 模型首次加载 |
| SearchBackend 调用成功率 | > 95% | MVP-26 跨平台一致性测试 | 按 backend 分桶 | 双平台 | `UnsupportedIntent` 不计为失败 |
| **macOS / Windows evals 通过率差距** | **< 5 个百分点** | MVP-25 同份 evals 双平台跑 | 直接差值 | 双平台基准画像 | — |
| 模型输出 JSON 合法率 | > 98% | MVP-25 触发模型的用例 | schema 校验通过率 | 基准画像 | — |
| 文件操作权限策略 | 100% 通过安全 evals | MVP-25 安全子集（含 schema §7.6/§7.7） | 必须 100% | — | — |
| Tauri 应用流畅运行 | 启动 < 3s / 操作响应 < 100ms | MVP-27 | p95 | **基准画像** | 应用首次启动 |
| Stub backend 不进入生产 fallback 链 | 集成测试断言通过 | — | — | — | — |

### 6.3 B 阶段出场（Beta）

| 指标 | 阈值 | 数据集 | 统计口径 | 运行环境 | 排除项 |
|---|---|---|---|---|---|
| 总体 evals 通过率 | > 90% | BETA-13 v0.9 evals（1000 条） | 严格匹配 | 双平台基准画像 | — |
| 音乐 artist / title 准确率 | > 85% | BETA-13 音乐子集 | Top-1 命中 | 基准画像 + 测试音频库 | — |
| Office / PDF 内容 Top 5 命中率 | > 80% | BETA-13 文档内容子集 | Top-5 命中 | 基准画像 + 测试文档库 | — |
| OCR Top 5 命中率 | > 75% | BETA-13 OCR 子集 | Top-5 命中 | 基准画像 + 测试图片库 | — |
| macOS DMG 经 GitHub Releases 下载可装可用（2026-07-04 开源口径，替代原 Notarization 指标） | 真机安装成功 + Gatekeeper 绕行步骤文档化 | BETA-10 产物 | 通过 / 失败 | macOS 基准机 | — |
| Windows 安装包经 GitHub Releases 下载可装可用（2026-07-04 开源口径，替代原 MSIX SmartScreen 指标） | 真机安装成功 + SmartScreen 说明文档化 | BETA-10A 产物 | 通过 / 失败 | Windows 基准机 | — |
| 一键删除索引 / 日志 / 模型 / 配置 | 全部可用 | 手动验证 | 是 / 否 | 双平台 | — |
| 后台索引 CPU 占用 | < 15% | 索引 100GB 测试库 | 平均 | **基准画像** | 索引完成阶段 |
| 后台索引内存占用 | < 1GB | 同上 | RSS 峰值 | 基准画像 | — |

### 6.4 V 阶段（1.0 发布）

| 指标 | 阈值 | 数据集 / 验证方式 |
|---|---|---|
| 自动更新机制 | 端到端通过 | V10-04 演练（macOS Sparkle / Windows Squirrel） |
| 崩溃恢复测试 | 模拟 5 类崩溃场景全恢复 | V10-05 测试 |
| 测试矩阵 | macOS 14 / 15 / 26 × Windows 10 / 11 全部通过 | V10-11 v1.0 evals |
| 法务文档 | LICENSE 双许可 / Third-party Notices / 隐私说明 全部就绪（2026-07-04 开源口径，律师签字取消） | V10-08 |
| ~~商标~~ | 已取消（2026-07-04 开源免费拍板） | ~~V10-09~~ |
| 插件系统 | SDK 文档 + 至少 1 个示例插件 | V10-01 |
| 本地活动洞察模块 | 默认关闭、可选开启、隐私边界文档化 | V10-02 + privacy-security.md |

### 6.5 不可回归约束（Codex 审阅 nice-to-have #14 落地）

每阶段出场必须**重跑所有前序阶段 eval**，并满足：

- 前序 eval 通过率 **不低于上一阶段出场报告**。
- 任何下降必须在出场报告中明确豁免理由（如"v0.5 改了 X 字段，3 条用例预期重新校准"）。
- 豁免数累计不得超过该 eval 集总数的 5%。

实现：`packages/evals` 提供 `--regression-check <prev-report.json>` 选项；阶段出场报告必须包含回归对比表。

## 7. 风险地图

按发生概率 × 影响 排序，每条带触发条件与缓解措施。

| 风险 | 概率 | 影响 | 触发条件 | 缓解 | 应对窗口 |
|---|---|---|---|---|---|
| **本地模型在低配机器上慢** | 高 | 中 | MVP 测试机内存 8GB / CPU 老 | 规则优先 + 模型常驻 + 默认 4-bit；提供"轻量模式"关闭模型 | MVP 出场前 |
| **macOS Spotlight 排除目录导致结果缺失** | 中 | 中 | 用户搜不到明显存在的文件 | 引导授权 Full Disk Access；UI 明确告知结果范围；fallback 到自建轻量索引 | MVP-23 |
| **Windows Search 被企业策略禁用** | 中 | 中 | 企业 / 政府机环境 | fallback 到 Everything；无 Everything 时提示用户 | MVP-11 |
| **跨平台开发复杂度爆炸** | 中 | 高 | platform-specific 代码渗透到通用层 | 严格隔离 platform/* 与 search-backends/{spotlight,windows-search,everything}；通用层不 import 平台 API | 持续，CR 时严查 |
| **Apple Notarization 失败** | 中 | 中 | Hardened Runtime / entitlements 配置错 | Beta 启动前演练（见 §5）；维护 notarization 错误排查 runbook | Beta 启动前 |
| **IP 风险：暗示 Apple/MS/voidtools 背书** | 低 | 高 | 营销文案中误用 | 文案过 [IP 计划书 §7.3](./docs/LociFind知识产权保护计划书.md) 与 [风险清单 §3.2](./docs/LociFind项目注意事项与风险清单.md) | Beta 外发前 |
| **训练数据混入真实用户敏感信息** | 中 | 高 | 直接抓取查询日志 | 严格用合成数据 + 手工标注；[训练数据版本化](./training/datasets/README.md)；commit 前 hook 校验 | LoRA 启动前 |
| **Agent 误删用户文件** | 低 | 极高 | MVP 错误开放 delete | MVP 禁用 delete；写操作必须显式确认 + 移到回收站；删除批量阈值 clarify | MVP-03 |
| ~~商标 LOCI 同名冲突阻挡注册~~（2026-07-04 开源免费拍板后不再申请注册；残余风险 = 他人抢注同名，接受） | 低 | 低 | 他人注册同名商标并主张权利 | 全程用完整品牌 LociFind；开源先发时间戳即公开在先使用证据 | — |
| **本地活动洞察的隐私误判** | 低（V10） | 高 | 用户认为是监控 | 默认关闭；UI 明确说明数据本地；不收集网络访问 | V10-02 |
| **系统搜索索引延迟 / 排序差异导致 eval flake**（Codex #13） | 高 | 中 | CI 上 47/500/1000 条 eval 间歇失败 | PROTO-05A fixture 预热；测试启动前等待 Spotlight 索引完成；backend 测试分层（翻译单元测试 vs 实机 smoke）；Top-K 容忍；报告中记录 OS / 索引状态 | PROTO-08 起持续 |
| **应用商店 / 系统平台隐私政策收紧**（Gemini #5） | 中 | 中 | Apple 新增 Privacy Manifest 项 / Microsoft Store 政策变更 | Beta 启动前重读 App Store Review Guidelines；订阅 Apple / Microsoft 开发者新闻；BETA-00 法务审查覆盖最新政策 | Beta 启动前 + 1.0 前 |
| **模型授权在不同司法辖区的商用限制**（Gemini #5） | 低 | 中 | 中国 / 美国 / 欧盟对模型商业使用差异 | 模型 license 台账（IP §7.5）按区标注；BETA-09 跨平台部署前确认；如 Qwen license 收紧准备备用基座模型清单 | Beta 启动前 |

## 8. 阶段切换 checklist

每个阶段切换必须由当前会话工具在 [STATUS.md](./STATUS.md) 留记录，并勾选下方对应 §6 出场指标。

### P → M 切换

- [ ] §6.1 全部指标达成
- [ ] PROTO-09 评测报告 `docs/reviews/proto-exit.md` 落库（按 §6 报告模板）
- [ ] [§5 长周期事项](#5-长周期事项时间线独立于代码节奏) 中 P 阶段第 0 天事项已启动

### M → B 切换

- [ ] §6.2 全部指标达成
- [ ] **§6.5 不可回归约束**：47 条 P eval 通过率不低于 PROTO-09 报告
- [ ] MVP-28 评测报告 `docs/reviews/mvp-exit.md` 落库
- [ ] **BETA-00 开源发布审查已启动**（LICENSE 双许可已入库 2026-07-04 ✅；剩 Everything SDK 条款核查 + 隐私说明 + 仓库脱敏）
- [x] ~~商标申请 / Apple Developer 账号 / Windows 签名证书~~ **已取消（2026-07-04 开源免费拍板）**

### B → V 切换

- [ ] §6.3 全部指标达成
- [ ] **§6.5 不可回归约束**：500 条 MVP eval + 47 条 P eval 通过率不低于上一阶段
- [ ] BETA-14 评测报告 `docs/reviews/beta-exit.md` 落库
- [ ] **BETA-00 开源发布审查完成**：LICENSE 双许可 / Third-party Notices / 隐私说明入库 + 仓库脱敏核查通过（律师签字要求已随 2026-07-04 开源免费拍板取消）
- [ ] 公开内测反馈整理完成

### V → 公开发布

- [ ] §6.4 全部指标达成
- [ ] **§6.5 不可回归约束**：1000 条 Beta eval + 全部前序 eval 通过率不低于 BETA-14
- [ ] 官网 / 文档 / 发布稿就绪

## 9. 出场报告模板

PROTO-09 / MVP-28 / BETA-14 / V10-11 的出场报告统一遵循下方结构（Codex 审阅 nice-to-have #16 落地）：

```markdown
# {阶段} 出场报告

> 评估人：{工具名}
> 日期：YYYY-MM-DD
> 阶段：{P / MVP / Beta / V1.0}

## 1. 环境
- macOS 基准机：{型号 / OS 版本 / 内存}
- Windows 基准机：{型号 / OS 版本 / 内存}
- 测试时 Spotlight / Windows Search 索引状态

## 2. 数据集版本
- evals 版本：v0.X，{N} 条
- fixture 版本：vY，覆盖 ...

## 3. 准确率
- 总体：{X%}（vs 阈值 {Y%}）
- 分桶：按 intent / 语言 / backend 分桶
- 失败用例列表与归因

## 4. 性能
- 各项 p50 / p95 / p99
- 与上一阶段对比

## 5. 回归对比（§6.5 不可回归约束）
- 前序 eval 通过率 vs 上一阶段
- 豁免列表与理由

## 6. 失败 / 警告 / 已知问题

## 7. 出场指标 checklist
- [ ] 逐项勾选 §6.X
- [ ] §6.5 不可回归

## 8. 下一阶段风险与准备
```

## 10. ROADMAP 维护规则

- **谁更新**：任一工具均可更新 task 状态、添加新 task、修改估时；**重大方向调整**（阶段范围变化、关键路径任务删除）需用户确认。
- **何时更新**：
  - 收工时：把本会话改动的 task 状态同步到本文件（与 STATUS.md 同步进行）。
  - 阶段切换时：完整勾选 §8 checklist 并产出 §9 模板的出场报告。
  - 引入新依赖 / 删除任务：当场更新依赖图。
- **不更新的内容**：当前 task 详情 / 当前 next step / 阻塞 → 这些在 STATUS.md，不要复述。
- **审阅**：v0.1 → v1.0 由 Codex + Gemini 并行审阅完成；之后的版本只在阶段切换时复审。

---

## 11. v0.1 → v1.0 修订摘要（来自 Codex + Gemini 双轨审阅）

完整审阅原文：
- [Codex 审阅](./docs/reviews/2026-05-25-roadmap-codex.md)（6 must-fix / 7 should-have / 3 nice-to-have / 2 out-of-scope）
- [Gemini 审阅](./docs/reviews/2026-05-25-roadmap-gemini.md)（0 must-fix / 3 should-have / 2 nice-to-have）

两份审阅**互补无冲突**，全部 must-fix 与 should-have 已采纳。

### Codex must-fix（全部修订）

| # | 修订点 | 落地位置 |
|---|---|---|
| 1 | P 阶段补合成 fixture 任务 | 新增 PROTO-05A；§3.1 依赖图更新 |
| 2 | P 阶段补 macOS location resolver | 新增 PROTO-04A；§3.1 依赖图更新；MVP-13 改为跨平台扩展 |
| 3 | Schema 用例口径统一为 47 条 | §3.1 PROTO-02/06/08 验收明确"47 条 + §7.1-§7.4 子集 30 条用于翻译实测" |
| 4 | MVP 补 SearchBackend async/streaming 迁移 | 新增 MVP-07A |
| 5 | MVP 补 FileActionTool | 新增 MVP-10A |
| 6 | §6 指标定义四要素 | §6 全面重写为四要素表格 + 统一基准硬件画像 |

### Codex should-have（全部修订）

| # | 修订点 | 落地位置 |
|---|---|---|
| 7 | PROTO-03 "字节一致"改为语义一致 + 交叉测试 | PROTO-03 验收 |
| 8 | PROTO-02/03 标注可并行 | PROTO-02 验收 + 依赖图 |
| 9 | PROTO-05 估时 2d → 3d | PROTO-05 估时列；§3.1 关键路径更新为 7-8 天 |
| 10 | PROTO-06 标题加"规则解析，不含模型 fallback" | PROTO-06 标题 + 验收 |
| 11 | MVP 增加并行约束小节 | §3.2 M5 末尾新增小节 + §1 字段约定 |
| 12 | Beta 阶段补依赖列 | §3.3 全表加依赖列 |
| 13 | 风险地图补"系统搜索 eval flake" | §7 新增一行 |

### Codex nice-to-have（全部采纳）

| # | 修订点 | 落地位置 |
|---|---|---|
| 14 | 每阶段"不可回归"指标 | §6.5 新增；§8 各切换 checklist 加约束 |
| 15 | PROTO-01 加 clippy / fmt lint gate | PROTO-01 标题 + 验收 |
| 16 | 出场报告模板化 | §9 新增 |

### Codex out-of-scope（保留原 ROADMAP 决策）

- #17 本地模型不提前到 P：PROTO-06 标题已明确不含模型 fallback。
- #18 P 阶段不做 Tauri UI：ROADMAP 一直未在 P 阶段安排 UI。

### Gemini should-have（全部修订）

| # | 修订点 | 落地位置 |
|---|---|---|
| 1 | Beta 前增加法务与安全审查 | §3.3 新增 BETA-00；§8 M→B / B→V checklist 加 BETA-00 前置 |
| 2 | 长周期事项补"全球商标评估"+ 强化 Third-party Notices 起点 | §5 新增一行 + 修订 Third-party Notices 行 |
| 3 | 性能基准明确硬件画像 | §6 顶部新增"统一基准硬件画像"小节 |

### Gemini nice-to-have（全部采纳）

| # | 修订点 | 落地位置 |
|---|---|---|
| 4 | 本地活动洞察"最近文件统计"可下沉到 Beta | BETA-04 加注释；V10-02 标题改为"工作时间分布 + 主题摘要" |
| 5 | 风险地图加"商店隐私政策收紧" + "模型授权地区差异" | §7 新增两行 |

### 2026-07-02 定位收敛修订（用户拍板 + Claude Code 起草 + Codex 评审）

- **定位收敛**：LociFind = 本地语义检索底座（个人桌面 + 团队/企业冷归档检索）；**不做分析层**（内容关联分析 / 摘要 / 比对 / 起草），经 BETA-32 MCP daemon + 外部 LLM 组合实现。目标企业场景三个：律所卷宗 / 内部审计 / 离职归档。
- §3.3 新增 **B7 小节**（BETA-35~41 七卡，并行衍生子线、不进 §6.3 出场指标、不阻塞 B→V）。
- §3.4 重定性三卡：**V10-13** re-scoped 并入 BETA-40；**V10-15** re-scoped 为 Frozen Index Pack（无内置 LLM 合成）；**V10-16** 重定性为 MCP/LLM 读取权限与出处闸门、依赖改挂 BETA-36/40。
- 方案与 Codex 评审全文：[docs/reviews/doc-realign-retrieval-foundation.md](./docs/reviews/doc-realign-retrieval-foundation.md)（Codex：APPROVE with required adjustments，修正意见已全部合入）。

### 2026-07-04 护城河规划修订（Codex 起草 + Claude Code 评审综合 + 用户拍板）

- 护城河规划落档：[docs/reviews/moat-plan-2026-07-04.md](./docs/reviews/moat-plan-2026-07-04.md)——核心命题「功能不构成壁垒（AI 时代天级可复制），评测资产 / 信任证据 / 客户侧沉淀 / 场景纵深才是」；护城河论述单一信源在该 doc，其他文档只挂引用。
- §3.3 B7 新增两卡：**BETA-43**（V10-16 先导：出处/权限闸门产品化）、**BETA-44**（enterprise eval 扩容 22→50 case）。
- §3.4 **V10-16** 标注先导部分提前拆 BETA-43、估时缩量。
- §5 新增「获取设计伙伴 / 首个真实部署」长周期事项（护城河 P0，主动获取）。
- 红线不变：B7（含新两卡）仍不进 §6.3 出场指标、不阻塞 B→V 切换。

### 2026-07-04 开源免费定位修订（用户拍板 + Claude Code 落地）

- **决策**：LociFind 走**开源免费**路线，MIT OR Apache-2.0 双许可（LICENSE-MIT / LICENSE-APACHE 入库，workspace `license` 字段同步）；放弃全部商业分发前置——商标注册 / 代码签名证书 / Apple Developer / 付费域名。
- **§5 长周期事项**：Apple Developer / Windows 证书 / 域名 / 商标（含第二梯队评估）/ Notarization 演练 5 项标记已取消；Privacy Policy / EULA 行 re-scoped 为「简短隐私说明 + LICENSE」；双平台真机 evals 与设计伙伴获取**不受影响**（质量验证与护城河 P0 与商业分发无关）。
- **task re-scope**：BETA-00 → 开源发布审查（LICENSE / Notices / 隐私说明 / 商标使用规范 / 仓库脱敏，律师协同取消，估时 2 weeks → 2-3d）；BETA-10 → macOS DMG 开源分发（GitHub Releases + Gatekeeper 绕行文档 + Homebrew cask 评估）；BETA-10A → Windows 开源分发（NSIS 产物 + SmartScreen 说明 + winget/Scoop 评估）；V10-08 缩量；V10-09 商标注册 dropped。
- **出场指标**：§6.3 两条分发指标改「GitHub Releases 下载可装可用 + 绕行/说明文档化」；§6.4 法务行改开源口径、商标行取消；§8 M→B / B→V checklist 同步。
- **新增开源前置检查**（归 BETA-00）：Everything SDK（voidtools License）再分发条款核查；BETA-44 真实语料 / 个人路径 / git 历史脱敏后方可公开仓库。

### 后续

ROADMAP v1.0 已发布；之后的修订（task 状态、估时、新增子 task）由各会话收工时同步，重大方向调整需用户确认；下一次完整审阅在 M→B 阶段切换时进行。
