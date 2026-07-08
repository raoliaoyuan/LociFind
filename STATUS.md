# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（最新 **v0.9.23 双平台已发布**——含 BETA-53 本机 MCP + MCP token 两修 + **BETA-54 数字/编号检索** + **BETA-55 索引 Office 最后保存者**〔cp:lastModifiedBy 进 author FTS〕；BETA-54/55 **装机版真机验证达成**〔`15013866` 命中、author 带最后保存者〕；**BETA-56 短 CJK ≤2 字兜底已并入 main、待下个发版**）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**2026-07-08 Codex 接 MCP 排查 → BETA-54/55 + v0.9.23 双平台发布 done**——用户观察「Codex 绕过 MCP 直连库」实为 Codex 从没挂上（Claude 风格 JSON 没进 Codex TOML；配法修好后现稳走 MCP，途中踩 token 轮换 / MSIX 环境变量需注销重登）。真机暴露并修两 gap：**BETA-54 数字检索**（intent-parser 保留 ≥6 位数字串）+ **BETA-55 索引最后保存者**（`cp:lastModifiedBy` 进 author FTS）。三分支（main BETA-54 / origin token 两修 / release）**收敛为单一 main** + **v0.9.23 双平台发布**（CI 修 clippy `manual_range_contains` + fmt 遗留后全绿）。并发会话另修 MCP token 两 bug（已并 main）+ 短 CJK ≤2 字兜底（BETA-56，memory `cjk-short-query-trigram-like-fallback`）。上一里程碑 BETA-53 本机 MCP 服务 done（v0.9.20）。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；**v0.9.23 macOS DMG 已产出、具备真机测试前提**；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-08（最新）**：**Codex 接本机 MCP 排查 → BETA-54/55 + v0.9.23 双平台发布**（详见会话日志 + [详录](docs/session-logs/session-details-2026-07.md)）。用户带 Codex 截图问「是不是绕过 MCP 直连库」→ 实锤 **Codex 从没挂上 MCP**（贴的 Claude 风格 `mcpServers` JSON 没进 Codex 的 TOML `[mcp_servers]`），「绕行」= 无工具时自救。修接线（`codex mcp add --url` + `--bearer-token-env-var`），端到端 + audit 铁证验证走 MCP；踩坑 token 轮换 / **MSIX 包吃 setx 新 token 须注销重登**（重启 app/explorer 都不够）。真机暴露两 gap 并修：**BETA-54 数字检索**（`file_search.rs` intent-parser 无条件剥数字 → `is_incidental_number` 保留 ≥6 位；desktop+MCP 共用 `parse` 一改两受益）+ **BETA-55 索引最后保存者**（`doc_extract.rs` `read_core_props` 加抽 `cp:lastModifiedBy` 经 `combine_authors` 并入 author FTS；xlsx 另开 zip 补 core props）。**收敛三分支为单一 main**（main BETA-54 / origin token 两修 / release 线岔开 → cherry-pick playbook + 强推 origin/main）。**v0.9.23 双平台发布**（本机出 Windows 装机版真机验 `15013866` 命中 + author 带最后保存者 → tag → CI 修 clippy `manual_range_contains` + fmt 遗留〔token 两修合并时漏的〕→ 全绿 → Windows setup + macOS DMG〔npm ERESOLVE flake 重跑〕齐发）。测试：intent-parser 242 / indexer doc_extract 25 全绿。派生 task：短 CJK 兜底（done，BETA-56）/ token UX / npm lockfile 发版隐患。

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：v0.9.18/19（BETA-47-52）→ **v0.9.20**（BETA-53 本机 MCP）→ **v0.9.21**（MCP token 两修）→ **v0.9.23 双平台已发布**（并入 BETA-54 数字检索 + BETA-55 最后保存者；v0.9.22 为中间态已折入）——[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.23) 含 Windows setup + macOS DMG，CI 全绿。并发机制累计稳。
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

### 2026-07-08 — Claude Code (Opus 4.8) — Codex↔MCP 接线 + BETA-54/55 + v0.9.23 双平台发布

**承接**：用户带 Codex 截图问「是否绕过 MCP」→ 实锤 Codex 从没挂上（Claude JSON 没进 Codex TOML）；修接线后稳走 MCP（详见「当前 Task」）。
**BETA-54 数字检索**：`file_search.rs` `extract_en_residual_keywords` 无条件剥纯数字 → `is_incidental_number`（<6 位才剥），desktop+MCP 共用 `parse` 一改两受益；242 测试。
**BETA-55 索引最后保存者**：`doc_extract.rs` `read_core_props` 加抽 `cp:lastModifiedBy` 经 `combine_authors` 并入 author FTS，xlsx 另开 zip 补 core props；doc_extract 25 pass。生效需清空索引重建。
**发布**：三分支收敛为单一 main（cherry-pick playbook + 强推 origin/main）；本机出 Windows 装机版真机验（`15013866` 命中 / author 带最后保存者）→ **v0.9.23 tag → 双平台发布**；CI 修 clippy `manual_range_contains` + fmt 遗留后全绿，macOS npm ERESOLVE flake 重跑过。**收尾**：清后台 worktree（2 个已并入的删了）+ **BETA-56 短 CJK 兜底 cherry-pick 并入 main**（indexer +4/local-index +1，本机 fmt/clippy/test 全过；待下个发版）。派生 task：短 CJK（done BETA-56）/ token UX / npm lockfile。

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


