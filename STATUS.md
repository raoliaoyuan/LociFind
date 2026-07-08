# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（最新发版 **v0.9.21**——含 BETA-53 本机 MCP 服务 + MCP token 两修〔reset 自动重启 + token 持久化分叉〕，待 CI 出包 + 用户 Windows 真机测）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**2026-07-08 修复本机 MCP 服务 token 持久化分叉**（BETA-53 follow-up）——`update_settings` 全量覆写用旧快照把后端带外写的 token 冲成 null（双写者覆盖，非双数据目录）→ 401 静默失效；修为写盘前合并回磁盘现值的 MCP 两字段，+3 测试，编译验证靠 CI。BETA-53 主体仍 **done**：接 S1 只读挂载地基：server 加 `personal_local` 多 root 构造器 + `serve_bound`（真 socket 起停集成测试）；桌面 `mcp_service.rs` 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`）复用桌面 embedder + 只读挂载 index.db、`127.0.0.1:8766`+随机 token+自启+持久化；前端 `McpPane.tsx`（开关/token 复制/配置片段/重置/安全提示）+ 工具菜单入口。验证 server 93 / desktop 174 / clippy·fmt·tsc+vite 全绿。**真机验证达成 → done**（功能 §2/§3/§4 + GUI 全流程 + **语义路径 B**〔`semantic-recall` 构建：`semantic=true` + 中文 query 命中英文文档跨语言召回〕三维均通过，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)；仅余可选「真 Claude Code 进程实连」，协议已 curl 验过）。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-08（最新）**：**修复本机 MCP 服务 token 持久化分叉**（BETA-53 follow-up，详见同名会话日志 + [详录](docs/session-logs/session-details-2026-07.md)）。排查 Codex 接 MCP 时的矛盾态——服务在跑并持一个 token、磁盘 settings.json 却显示 `mcp_service_token:null`/`enabled:false`。**根因非双数据目录**（`mcp_service.rs` 与 UI 均用 `settings_file_path` = `app_config_dir/settings.json`，同一文件；`LociFind\` 无 settings.json 本属正常），实为**双写者覆盖**：MCP 开关/token 由后端**带外**写盘，偏好表单 `update_settings` 全量覆写时用弹窗挂载期的旧快照把后端刚写的 token 冲成 null，而运行中 axum server 仍持内存旧 token → 401 静默失效、外部 client 无感退回 grep。**修**：`update_settings` 写盘前先读磁盘现值、合并回 `mcp_service_enabled`/`mcp_service_token`（磁盘成唯一信源、表单永不动这两字段）；`merge_backend_managed_mcp_fields` + 可测 `update_settings_at` 内核。**测试 +3**：settings clobber 回归 / 首存无文件 / mcp_service status↔磁盘 token 一致守卫。本机无 MSVC 工具链，编译/clippy 验证靠 CI；doc_markdown·field_reassign 已按 CI pedantic 核对。未 bump 版本、随下个发版携带。

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：**v0.9.18**（BETA-47/48/49/50）+ **v0.9.19**（BETA-51/52）双平台各 success、changelog 齐（[v0.9.19](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.19)）；**v0.9.20**（BETA-53 本机 MCP 服务）；**v0.9.21**（MCP token 两修：reset 自动重启 + token 持久化分叉，仅桌面）；并发机制累计稳。**Windows 首轮真机 6 项通过**（[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；macOS 真机待跑。
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

### 2026-07-08 — Claude Code (Opus 4.8) — 修复本机 MCP 服务 token 持久化分叉

**承接**：2026-07-08 Codex 接 MCP 排查（memory `mcp-token-ux-dual-settings-bug`）暴露矛盾态——运行态持 token（`/health` 200、旧 token 401）但磁盘 settings.json 显示 token=null/enabled=false。
**根因**：**非**双数据目录（后端与 UI 同写 `app_config_dir/settings.json`）；实为**双写者覆盖**——MCP token/enabled 后端带外写盘，偏好表单 `update_settings` 全量覆写时用挂载期旧快照把其冲成 null，运行中 axum server 仍持内存旧 token → 401 静默失效。
**产出**：[settings.rs](apps/desktop/src-tauri/src/settings.rs) `update_settings` 改为写盘前读磁盘、合并回后端带外管理的 `mcp_service_enabled`/`mcp_service_token`（`merge_backend_managed_mcp_fields` + 可测 `update_settings_at` 内核），磁盘成 MCP 两字段唯一信源；`settings.rs` +2 测试（clobber 回归 / 首存无文件）、[mcp_service.rs](apps/desktop/src-tauri/src/mcp_service.rs) +1 测试（status↔磁盘 token 一致守卫）；doc_markdown·field_reassign 已按 CI pedantic 核对。
**未尽事宜**：本机无 MSVC 工具链无法本地 `cargo test`/clippy，编译验证靠 CI；未 bump 版本，随下个发版携带。（详录 → docs/session-logs/session-details-2026-07.md）

### 2026-07-08 — Claude Code (Opus 4.8) — MCP 令牌重置 UX 小修（发版后）

**承接**：任务据「Codex 接 MCP 排查」印象报「面板只弹一次 token + 缺重置按钮」→ 复现发现二者早在 e1f3048（2026-07-07）已具备（token 随 3s 轮询常驻、重置按钮在列），任务描述来自旧装机版。用户拍板：把唯一真实缺口——`reset_token` 停服务后需手动重启——**改为自动重启**。
**产出**：`mcp_service.rs` `reset_token` 记录重置前运行态，停服务（踢旧连接，§5.2）+ 轮换 token 后，**若原本在跑则自动 `start()` 复用新 token 重启**（旧 token 立即 401、新 token 立即 200，免手动重开）；停止态则仅换 token。`McpPane.tsx` 重置提示文案同步；补停止态轮换 `#[tokio::test]`。
**结果**：`cargo check --tests`〔locifind-desktop〕绿、`cargo test mcp_service` 4 pass（含新测）；前端仅改一处中文提示串。playbook §4 / ROADMAP BETA-53 同步。本 reset 小修与上条 401 分叉修复（同日并行会话）已一并并入 main（异文件、互不冲突）。
**待验**：运行态自动重启的真机 401/200（需构建/起 app，本轮未做——复用已验的 start()/stop() 原语 + 单测覆盖停止态）。

### 2026-07-07 V — Claude Code (Opus 4.8) — 桌面「本机 MCP 服务」BETA-53 S2/S3 code-done

**承接**：接上轮 S1（`attach_readonly` 只读挂载地基），用户「按推荐执行」→ 一并做 S2/S3 推到 code-done + 补端到端闸门 + 收工。
**产出**：① **server**——`DaemonConfigFile::personal_local(roots, token)`（桌面多 root 变体、全权 admin、`allow_full_read`）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内、桌面侧免直依赖 axum）+ **真 socket 起停集成测试**（`/health` 200 · `/mcp` 无 token 401 · shutdown 5s 内优雅返回，5× 稳定）。② **桌面 `mcp_service.rs`**——`McpServiceState` + 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`），复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`（同步拿端口占用错误）、随机 64-hex token、oneshot 优雅关停、开关态+token 持久化 settings.json、enabled 时自启。③ **前端 `McpPane.tsx`**——开关 / 运行状态〔地址·挂载条数·语义臂〕/ token 复制 / Claude Code 配置片段复制 / 重置令牌 / 安全提示；工具菜单入口〔`open-prefs-mcp`〕+ 选项页第八 tab。
**关键决策**：内嵌复用（非子进程）；roots 仅供 `list_collections` 展示（读取面由索引 db 边界天然约束）；安全红线只绑 127.0.0.1 + token 必填随机 + 暴露面 UI 明示 + 重置即踢连接。
**结果**：server lib 93 pass（+2）/ desktop 174 pass（+3）/ clippy `-D warnings`〔server·desktop·daemon〕/ fmt / tsc+vite 全绿；三方许可补 `getrandom`；设计文档 + ROADMAP BETA-53 标 code-done。
**真机验证达成（同会话续跑）**：功能 §2/§3/§4（harness + 对实跑 app curl）+ computer-use 驱动 dev app GUI 全流程 + 语义路径 B（`semantic-recall` 构建 harness：`semantic=true` + 中文 query 命中英文文档跨语言召回）三维均通过 → BETA-53 转 done；仅余可选「真 Claude Code 进程实连」（协议已 curl 验过）。
**发版**：bump **v0.9.20**（tauri.conf.json + Cargo.toml + Cargo.lock；含 BETA-53 本机 MCP 服务），推 `v0.9.20` tag 触发 release-windows.yml；Release changelog 补 MCP 服务用法 + 模型放置指引，待用户 Windows 真机测。

