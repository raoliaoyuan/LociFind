# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（v0.9.27 发版并入 **BETA-60 检索+索引性能优化**〔搜索 fan-out 串行→并发 + 索引 WAL + 提取分块并行〕；**v0.9.28 热修 BETA-60 进度回退**——首版把 `on_file` 放并行块尾串行段致「一块内进度全冻、用户误判索引卡死」，改为并行提取阶段逐文件上报〔`IndexProgress` 本 Send+Sync〕；数据无损、真机崩溃恢复已验）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**2026-07-09 Codex 查身份证文件复盘 → BETA-58/59 分派两 CLI 并落地（并入 main 待发版）**——用户带 Codex 全程截图（手搓 HTTP 3m52s）问「工具/MCP 还能优化啥」。Claude Code 当项目经理拆两条不重叠赛道并行：**BETA-58 接入体验**（前端子代理：McpPane 客户端切换 + Codex `mcp add` 命令 + MSIX 重登警告 + curl 全 Accept 头）+ **BETA-59 PII 概念检索**（Codex CLI：索引时识别身份证〔GB 11643 校验〕/手机号、注入类型关键词到 FTS，「身份证」概念词召回）。Claude Code 复核双 diff、跑通 tsc/clippy/test。**已发 v0.9.26**（BETA-57+58+59 一并 bump、推 tag 触发 CI 双平台发布）。BETA-59 生效需重建索引。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；**v0.9.23 macOS DMG 已产出、具备真机测试前提**；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-09（最新²）**：**BETA-60 检索+索引性能优化（分派双赛道并行）**。用户真机反馈搜「身份证」1965ms（fan-out 四臂）+ 索引慢，要 Claude Code 当项目经理拆活给本地 Claude CLI 与 Codex CLI 并监督、下版问题都解决。诊断：搜索 fan-out **串行 await**（各后端耗时相加、Windows Search powershell+ADODB 冷启动最大头）；索引 walk 单线程 + 每文件事务 + **未开 WAL**（每文件 fsync）+ 提取/OCR 无并发。拆**文件不重叠**双赛道并行：**Track A 搜索并发化**（本地 Claude，`packages/harness`+`apps/desktop/fanout.rs`）= harness 抽纯 fuse 函数保语义 + desktop `spawn_blocking` 并发查各后端（求和→取最慢）；**Track B 索引提速**（Codex CLI，`packages/indexer`）= WAL+`synchronous=NORMAL` 消 fsync + `scan.rs` 128/块 rayon 并行提取（仿音乐 `index_paths` 三段式、限内存保进度、DB 写不并行、embedding 不动）。**复核**：逐段核 desktop 并发编排（取消/兜底/sources/errors 对齐）+ 令 Codex 把首版全量堆内存改分块；**集成校验** `cargo check -p locifind-desktop`〔server/backends 调用方随 `Send`/`Sync` bound 一起过〕+ harness 191/indexer 194/clippy/fmt 全绿。**搜索不需重建索引；WAL/并行提取下次构建生效**。待发 **v0.9.27**。**未尽**：Windows/Everything 常驻宿主、embedding batch/context 复用、fan-out 软超时留后续轮。详见会话日志。（BETA-58/59 详录见会话日志）

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：v0.9.18/19（BETA-47-52）→ **v0.9.20**（BETA-53 本机 MCP）→ **v0.9.21**（MCP token 两修）→ **v0.9.23**（并入 BETA-54 数字检索 + BETA-55 最后保存者；v0.9.22 中间态已折入，[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.23) 双平台齐）→ **v0.9.24**（并入 BETA-56 短 CJK 兜底）。并发机制累计稳。
5. **BETA-10 剩余**：macOS DMG 产物 CI done 且 **v0.9.15 首验通过**；剩 macOS 真机放行验证（§6.3）；winget 待 BETA-14 后 / Homebrew tap 可启动（DMG CI 已跑通）。
6. **BETA-40 真实内网证据**：唯一剩余验收项，依赖 ②。
7. **剩余 6 条 partial**（不阻塞出场线，[beta-exit §3.4](docs/reviews/beta-exit.md)）：全为 v0.5 标注锁定项（markdown ft / 「上个月下载的」动词歧义 / 项目归档 location / downloads hint 双语 ×2，改标注吃 §6.5 豁免额度）+ 备份文件两难。parser 可确定性收割已见底。
8. **BETA-29 v2 余量**：修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，唯一剩余项）。
9. **V10-16 主卡**（隐私 UI 集成 + 全量策略收口）：BETA-43 先导拆出后缩量，待 V 阶段。

**流程备忘**：Windows 发版 = bump 版本（tauri.conf.json + Cargo.toml）→ 推 `v*` tag 触发 release-windows.yml → Release 说明含 changelog（CONVENTIONS §8）。Windows 编带 llama 的 locifindd 一律用 `scripts\build-locifindd-llama.bat`。

## 阻塞 / 待用户决策

- **Class A（外部条件，阻塞出场评测，不阻塞代码）**：仅剩 BETA-09(a)/MVP-26/28 双平台 evals——需 Windows 真机 + 完整 Spotlight 索引 macOS。~~Apple Developer / 证书 / 域名 / 商标~~ **已取消（2026-07-04 开源免费拍板**，分发改 GitHub Releases 开源口径，[ROADMAP §5](./ROADMAP.md)）。
- **Class B（产品决策，不阻塞 §6 出场线）**：**已全部清零**——最新一项「Everything 音乐全盘发现 vs 零索引语义」2026-07-06 拍板**方案 A（发现结果按 roots 过滤）**并当场落地（ROADMAP BETA-49）；此前 clarify options 等各项均已落地。

## 会话日志

> 摘要 ≤5 条；全文与更早历史：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md) → [STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) → [STATUS-archive-through-2026-06-03.md](docs/session-logs/STATUS-archive-through-2026-06-03.md)。

### 2026-07-09 — Claude Code (Opus 4.8) — v0.9.28 热修：BETA-60 索引进度回退（误判卡死）

**承接**：用户装 v0.9.27 后，索引中途硬关程序、重开感觉「进度卡死不动」。**现场取证**（Claude Code 直接在用户 Windows 机上 tasklist/sqlite3 只读排查）：装的确是 0.9.27；主库 `%APPDATA%\LociFind\index.db` **完好**——WAL 模式崩溃恢复成功、已 checkpoint 归零、documents=67 + passages=51 在，**我这轮 WAL 改动未致数据丢失**（一度 `sqlite3 -readonly` 读到 0 是没应用 WAL 的旧快照虚惊，WAL-aware `mode=ro` 读到 67）；无 desktop 派生的卡住 OCR 子进程；WAL 时间戳在重开后仍前进＝索引其实在跑。**真因**：BETA-60 Track B 把 `on_file` 放在「128/块并行提取完的块尾串行段」，一块处理期间进度计数器完全不动，块内有慢文件（大 PDF/图 OCR）就冻几十秒→误判卡死。**修**：`scan.rs` 把 `on_file` 下沉进并行 `par_iter` map、逐文件提取完即报（`IndexProgress` 本 `Send+Sync` 专为跨线程设计）；串行段不再重复 on_file；`EXTRACT_CHUNK` 128→64（进度解耦后分块仅限内存）。indexer 194 测试 + clippy/fmt 全绿。发 **v0.9.28**。（真机无限 hang 无证据：OCR 有 30s 超时、坏文件返 Err 已妥处）

### 2026-07-09 — Claude Code (Opus 4.8) — BETA-60 检索+索引性能优化（双赛道并行）

**承接**：用户真机反馈搜「身份证」1965ms（fan-out local+semantic+windows+everything 四臂）+ 索引慢，要 Claude Code 当项目经理分派本地 Claude CLI 与 Codex CLI 并监督、尽量不打扰、下版问题都解决。**诊断**（两 Explore 代理并行摸链路）：搜索 fan-out `fanout_merge.rs` 串行 await 各后端耗时**相加**、Windows Search 每查 spawn powershell+ADODB 冷启动是最大单头；索引 `scan.rs` walk 单线程 + 每文件事务 + **未开 WAL**（每文件 fsync）+ 提取/OCR 无并发。**分派**（文件不重叠双赛道并行）：Track A 搜索并发化（本地 Claude 子代理，harness+desktop）、Track B 索引提速（Codex CLI `codex exec --sandbox workspace-write`，indexer），两者禁碰 git/STATUS/ROADMAP。**产出**：A = harness 抽纯 fuse 函数（语义逐字节等价、daemon 路径改调它）+ desktop `concurrent_collect` 用 `spawn_blocking`+`block_on` 并发查各后端按原序回收（求和→取最慢）；B = 文件型库开 WAL+`synchronous=NORMAL`（内存库兜底）+ `scan.rs` 128/块 rayon 并行提取（串行预检→并行提取→串行写库/进度）。**复核**：逐段核 desktop 并发编排取消/兜底/顺序对齐；Codex 首版全量堆内存 → 令其改分块（限内存尖峰+保实时进度）；集成 `cargo check -p locifind-desktop`〔server/backends 调用方随 `Send`/`Sync` bound 一起过〕+ harness 191/indexer 194/clippy/fmt 全绿。**未尽**：Windows/Everything 常驻宿主、embedding batch/context 复用（涉 RAM/共享 runtime 风险高本轮不做）、fan-out 软超时。搜索不需重建索引、WAL/并行提取下次构建生效。待发 **v0.9.27**。

### 2026-07-09 — Claude Code (Opus 4.8) — 分派两 CLI 落地 BETA-58/59（MCP 接入体验 + PII 概念检索）

**承接**：用户带 Codex 查身份证文件全程截图，要求 Claude Code 当项目经理、把优化任务分派给本地 Claude CLI 与 Codex CLI 并监督完成，尽量不打扰用户、下版测试时问题都解决。**关键决策**：拆两条**文件不重叠**赛道并行——前端（TS，`apps/desktop`）分本地 Claude 子代理、Rust（`packages/**`）分 Codex CLI（`codex exec --sandbox workspace-write`），两者禁碰 STATUS/ROADMAP 与 git，收工归并由 Claude Code 统一做。**产出**：BETA-58（接入体验）+ BETA-59（PII 概念检索）均 done、并入 main 待发版（详见「当前 Task」+ ROADMAP）；Claude Code 复核双 diff、补 curl 的 PowerShell 提示 + doc_db 注入点权衡注释、跑通 tsc/clippy/test。**未尽事宜**：待推 **v0.9.26**（含 BETA-57/58/59）触发 CI 双平台发布；BETA-59 生效需重建索引；后续可提 `entity` 独立 FTS 列消除 snippet 注入词回显边缘。

### 2026-07-09 — Claude Code (Opus 4.8) — BETA-57 多词查询组间 AND→OR 召回兜底

**承接**：用户经 MCP 查体检材料，报「`体检 体检报告 健康检查 健康体检` 泛查 0 命中、单词能命中」。诊断纠偏：并非我起初判的 `fts_sanitize` 短语化（那条只在无词组的 raw-text 兜底触发、生产不走），真因是 `fts_match_from_groups` **组间 AND**——parser 拆多词组、缺任一词即整条结构性归零。`爱康`(2 字正文词)另属 BETA-56 兜底不扫 body 的已知边界 + 验证 daemon 跑 dummy.gguf 语义臂死，非本次范围。
**产出（方案 A：AND 优先 + 0 命中 OR 兜底）**：`search_results_expanded`（desktop+MCP 收敛点）AND 空且 ≥2 有效词组时经新 `fts_or_relax_from_groups` 放宽组间 OR 重试一次；零精确性回归（仅空时触发）；抽 `sanitized_group_terms` 消重。local-index +2 测试（单元 + 端到端复现体检报告场景 + AND 命中不受影响对照），29 全绿 / clippy `-D warnings` / fmt 净。查询侧改动**不需重建索引**。
**收尾（同日续）**：重编 `locifindd`〔release+llama-cpp，4m46s〕+ desktop NSIS〔6m40s〕→ **bump v0.9.25**（tauri.conf/Cargo.toml/lock）→ **真机 MCP 验证达成**：另编 FTS-only stub daemon〔dummy.gguf〕挂体检语料，`健康检查` 单搜 0（缺席）/ 多词 `体检 体检报告 健康检查 健康体检`〔含缺席词、旧 AND 必 0〕经 OR 兜底命中，audit.jsonl 三条 results 佐证。待推 `v0.9.25` tag 触发 CI 发布。分析层（「总结健康状态」）仍是外部 LLM 的活、LociFind 只管检索（范围不变）。


