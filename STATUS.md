# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**v0.9.16/17 双发版 + 真机反馈二轮修复 done**——v0.9.16 随包 BETA-45/46（macOS CI 踩 target-gated 依赖坑、shim 修复重跑过）；实测暴露下载卡死链 → 修复四刀 + **hf-mirror 镜像兜底** + 目录三行卡片布局，随 v0.9.17 发布（双平台一次过）。待用户 v0.9.17 真机验证。
- **下一步 top-3**：① v0.9.17 真机验证（用户进行中：下载取消/镜像兜底/三行布局/卸载保模型/零索引空态/clarify 英文追问/升级零损失）；② BETA-47 选项页重构（ROADMAP B8，含 Everything 开关 / 模型管理归位 / tab 拆分）；③ 设计伙伴/首个真实部署主动获取（护城河 P0）与双平台真机复跑填 [beta-exit.md](docs/reviews/beta-exit.md) TODO 格。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（clarify options 结构口径 2026-07-06 方案 A 拍板落地）。

## 当前 Task

**2026-07-06 III（最新）**：**v0.9.16/17 双发版 + 真机反馈二轮修复**。① v0.9.16 发版：macOS CI 首跑 E0433（everything crate target-gated 依赖被无条件引用）→ 平台 shim 修复 → dispatch 重跑 success（无须重打 tag）。② v0.9.16 装机实测回报 → 三项修复：**下载卡死链**（HF 连接阶段长挂占守卫 300s、取消无效、前端状态脱节 → select 取消竞速 + connect_timeout 15s + **hf-mirror 镜像兜底** + model_download_in_flight 前端感知 + PRIVACY 同步）、取消误报「下载失败」（invoke-catch 路径补过滤）、目录**三行卡片布局**（单行 flex 逐字断行 → 路径/统计/按钮分行）。③ v0.9.17 发版双平台一次 success（并发机制三连稳）、changelog 补全。desktop 170 全过 + tsc/vite/clippy/fmt 净。流程备忘：`gh run watch` 假退出 ×3，一律 `gh run view --json status` 轮询。详录 [session-details-2026-07](docs/session-logs/session-details-2026-07.md)。

## 下一步

1. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
2. **BETA-33 cycle 9 真机验证**：随下次发版装机，按 [manual-test-scenarios](docs/manual-test-scenarios.md) 跑六场景；本轮验证面另含 BETA-43（出处/`read_document`/审计导出，[playbooks README](docs/playbooks/README.md) 第 8/9 条）+ **BETA-12 卸载清理**（场景 5「升级零数据损失」为发版阻断；NSIS hook 首次真实构建即本次发版 CI）+ **BETA-29 意图草稿 v1（6 场景）+ v2（7 场景）**。
3. **v0.9.15 发版 done（并发首版）**：windows+macos 双 workflow 同 tag 并发均 success，[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.15) 含 exe + DMG（aarch64）+ changelog；macOS DMG CI 首验通过。用户真机测试进行中；**BETA-45/46 改动未随包**、待下次发版验证。
4. **BETA-10 剩余**：macOS DMG 产物 CI done 且 **v0.9.15 首验通过**；剩 macOS 真机放行验证（§6.3）；winget 待 BETA-14 后 / Homebrew tap 可启动（DMG CI 已跑通）。
5. **BETA-40 真实内网证据**：唯一剩余验收项，依赖 ①。
6. **剩余 6 条 partial**（不阻塞出场线，[beta-exit §3.4](docs/reviews/beta-exit.md)）：全为 v0.5 标注锁定项（markdown ft / 「上个月下载的」动词歧义 / 项目归档 location / downloads hint 双语 ×2，改标注吃 §6.5 豁免额度）+ 备份文件两难。parser 可确定性收割已见底。
7. **BETA-29 v2 余量**：修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，唯一剩余项）。
8. **V10-16 主卡**（隐私 UI 集成 + 全量策略收口）：BETA-43 先导拆出后缩量，待 V 阶段。

**流程备忘**：Windows 发版 = bump 版本（tauri.conf.json + Cargo.toml）→ 推 `v*` tag 触发 release-windows.yml → Release 说明含 changelog（CONVENTIONS §8）。Windows 编带 llama 的 locifindd 一律用 `scripts\build-locifindd-llama.bat`。

## 阻塞 / 待用户决策

- **Class A（外部条件，阻塞出场评测，不阻塞代码）**：仅剩 BETA-09(a)/MVP-26/28 双平台 evals——需 Windows 真机 + 完整 Spotlight 索引 macOS。~~Apple Developer / 证书 / 域名 / 商标~~ **已取消（2026-07-04 开源免费拍板**，分发改 GitHub Releases 开源口径，[ROADMAP §5](./ROADMAP.md)）。
- **Class B（产品决策，不阻塞 §6 出场线）**：**已全部清零**——7-04 X 四项拍板 + 7-06 clarify options 结构口径（方案 A：按 reason 定带不带 options、非 Unknown 一律挂、parser/标注双向对齐，[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）均落地。

## 会话日志

> 摘要 ≤5 条；全文与更早历史：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md) → [STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) → [STATUS-archive-through-2026-06-03.md](docs/session-logs/STATUS-archive-through-2026-06-03.md)。

### 2026-07-06 III — Claude Code (Fable 5) — v0.9.16/17 双发版 + 真机反馈二轮修复

**承接**：用户拍板发 v0.9.16 → 装机实测回报下载卡死链等 → 逐条修复攒批 → 拍板发 v0.9.17。
**发版**：v0.9.16 macOS 首跑 E0433（target-gated 依赖坑）→ shim 修复 + dispatch 重跑 success；v0.9.17 双平台一次 success，并发机制三连稳。changelog 均补全。
**产出**：下载卡死链修复四刀（select 取消竞速〔连接阶段即刻生效〕+ connect_timeout 15s + hf-mirror 镜像兜底〔PRIVACY 同步〕+ model_download_in_flight 前端恢复下载态）；取消误报失败修复（invoke-catch 补过滤）；目录三行卡片布局（路径/统计/按钮分行）。BETA-45 真机首验：发现 UI 工作（Everything 命中 artifacts 模型）。
**结果**：desktop 170 全过、tsc/vite/clippy/fmt 净；[Release v0.9.17](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.17) exe+DMG 齐。
**未尽事宜**：v0.9.17 待用户验证（取消即刻生效/镜像/三行布局/卸载保模型弹窗/零索引空态/升级零损失）；`gh run watch` 假退出 ×3 → 一律 --json 轮询。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-06 II — Claude Code (Fable 5) — v0.9.15 并发发版 + cycle 9 真机反馈落地（BETA-45/46）

**承接**：用户拍板 push + 并发发版 → 真机测 v0.9.15 → 回报三条反馈 → 三项拍板后当场实现 ①②、③登记下会话。
**发版**：v0.9.15 双平台并发**双 success**——macOS DMG CI 首验通过（aarch64 DMG 产出）、并发同 Release 幂等追加成立；changelog 补全。踩坑：`gh run watch` 假退出 ×2，改 `--json status` 轮询。
**产出**：**BETA-45** 模型本地发现 + 卸载默认保模型（NSIS `/SD IDNO` + 同卷 Rename 暂存；everything `find_files_named`〔wfn: 精确名 + UTF-8 导出〕；discover/import 命令 + 白名单 + 原子落盘 + 复用下载 done event；ModelDownloadStep 发现 UI）；**BETA-46** 默认零索引（`resolve_index_roots_tagged` 三夹仅勾选纳入、空+false=零索引）+ checkbox 常显 + banner 退役 + 路径完整显示；**BETA-47** 选项页重构登记（ROADMAP 新 B8 小节）。
**结果**：desktop 168 / everything 15 / settings 四分支 / uninstall 闸门（+2 断言）全绿；tsc/vite/clippy `-D warnings`/fmt 净。根因诊断：反馈① = BETA-12 整目录删含模型；反馈② = 空 roots 兜底三夹旧语义。
**未尽事宜**：BETA-45/46 随下次发版真机验证（NSIS 弹窗须真装真卸）；升级行为变化（空 roots 老装机停索三夹）随 cycle 9 复测确认；BETA-47 下会话。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-06 — Claude Code (Opus 4.8 / Fable 5) — BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割至 99.4%

**承接**：用户问「本次会话该做什么」→ 读三份共享文档 + 定向读 ROADMAP §2/§6.3/§8 → 判定质量线已达标、卡口全在真机/对外；用户选「先看 ROADMAP 全局再定」→ 按建议做出场报告骨架 + clarify 分析 → 拍板方案 A → 就地实现 → 续推老账收割。
**产出**：① [beta-exit.md](docs/reviews/beta-exit.md) 骨架（§9 模板，parser-only 全填、真机格标 TODO，B→V checklist 必交付项）；② clarify options **方案 A** 拍板并落地（[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）——按 reason 定带不带 options、非 Unknown 一律挂、parser（`clarify_with`+`standard_options`）与标注（d6/d8 共 17 条）双向对齐，Class B 清零；③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符保全 / 裸 no+字面扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。
**结果**：**v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 230→235 测 + evals/harness/server 全 gate（28 suite 0 failed）+ clippy `-D warnings`/fmt 净。剩 6 partial 全为 v0.5 标注锁定项 + 备份文件两难，parser 收割见底。
**未尽事宜**：真机复跑填 beta-exit TODO 格；clarify en query 返中文 options 是既有 i18n 缺口（独立小卡）。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。
