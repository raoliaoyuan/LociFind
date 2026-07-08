# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（最新发版 **v0.9.20**——含 BETA-53 本机 MCP 服务，待 CI 出包 + 用户 Windows 真机测）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**BETA-56 短 CJK 查询（≤2 字）检索兜底 done（2026-07-08）**——BETA-55 把最后保存者写进 `author` 后，真机搜 2 字人名「燎原」仍 0 命中（`documents_fts`/`music_fts` trigram tokenizer，<3 字符生不成 3-gram 必 0；2 字人名/常用词/短编号同受限）。修 = `DocumentIndex::query`/`MusicIndex::query` **短查询 metadata LIKE 兜底**：无 `fts_match` 且 query 全词 <3 字纯 alnum/CJK → `LIKE '%词%'` 匹配 metadata 列（doc: title/author/file_name；music: 加 artist/album；**不扫 body**），长短混合仍走 FTS。desktop 搜索与 daemon MCP 共用 `LocalIndexBackend`——纯短查询在 `fts_match_from_groups` 被剥空 → 回退 base keyword → `query` 兜底，一改两受益。indexer +4 / local-index +1 端到端全绿 + clippy `-D warnings`/fmt 净；**真机 MCP 实锤**：新建 FTS-only daemon 挂 `dc:creator=燎原 饶`（正文/文件名均不含）语料，HTTP MCP 搜「燎原」0→命中 `案卷2024.docx`、「违约金」FTS 仍命中、「张三」0。上一里程碑 **BETA-54 数字/编号检索 gap done**（代码层，生效待桌面 app 出新版）。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-08（最新）**：**BETA-56 短 CJK 查询（≤2 字）检索兜底 done**（详见同名会话日志）。承接 BETA-55（最后保存者进 `author`）真机暴露：搜 2 字人名「燎原」0 命中——`documents_fts`/`music_fts` 用 trigram tokenizer（为支持中文任意子串），代价是 <3 字符查询生不成 3-gram、必然 0 命中（db.rs 注释已明载）；2 字人名 / 常用词（合同/发票/预算）/ 短编号同受限，语义臂对内容词有兜底、对人名/编号类无能为力。**修法**（方案 1 短查询 LIKE 兜底）：`DocumentIndex::query`/`MusicIndex::query` 新增分支——无 `fts_match` 且 query 经 whitespace 切分后 **全部** 词 <3 字符且纯 alnum/CJK 时，改走 `LIKE '%词%'` 子串匹配 metadata 列（doc: title/author/file_name；music: 加 artist/album；**不扫 body**——正文全表 LIKE 慢且噪声高、内容词由语义臂兜底），长短混合查询保持 FTS。共享判据 `db::short_metadata_like_terms`（`char::is_alphanumeric` 对 CJK 表意字亦 true；含符号病态输入如 `a" OR b` 不触发、保 `fts_sanitize` 零回归）。desktop 搜索与 daemon MCP 共用 `LocalIndexBackend`——纯短查询在 `fts_match_from_groups` 被剥空 → fts=None → 回退 base keyword → `query` 兜底，**一处修两路径皆受益**。**验证**：indexer +4（doc author/title/file_name 命中 + body 不扫 + doc_type 过滤 + music 元数据）/ local-index +1 端到端（走 `search_expanded` 生产路径）全绿，全 crate clippy `-D warnings`/fmt 净；**真机 MCP 达成**：新建 daemon FTS-only（`--model-path` 传 dummy.gguf → stub embedder → FTS-only）挂含 `dc:creator=燎原 饶`（正文/文件名均不含该串）的手工 docx 语料，HTTP MCP 搜「燎原」0→命中 `案卷2024.docx`、「违约金」正文 FTS 仍命中、「饶燎原」（author 非连续）/「张三」0。

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：**v0.9.18**（BETA-47/48/49/50）+ **v0.9.19**（BETA-51/52）双平台各 success、changelog 齐（[v0.9.19](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.19)）；并发机制累计稳。**Windows 首轮真机 6 项通过**（[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；macOS 真机待跑。
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

### 2026-07-08 — Claude Code (Opus 4.8) — BETA-56 短 CJK 查询（≤2 字）检索兜底

**承接**：BETA-55 把 docx 最后保存者写进 `author` 后，真机搜 2 字人名「燎原」仍 0 命中。诊断：`documents_fts`/`music_fts` 用 **trigram** tokenizer，<3 字符生不成 3-gram 必 0（db.rs 注释已明载）；2 字人名/常用词/短编号同受限。链路确认——纯短查询在 daemon `search_expanded` 里被 `fts_match_from_groups` 剥空 → fts=None → 回退 base keyword → `DocumentQuery.text`，**单一收敛点 = `DocumentIndex::query`**（desktop 与 MCP 两路径皆经此）。
**修法**（方案 1）：`DocumentIndex::query`/`MusicIndex::query` 加短查询 metadata LIKE 兜底——无 `fts_match` 且 query 全词 <3 字纯 alnum/CJK → `LIKE '%词%'` 匹配 metadata 列（doc: title/author/file_name；music: 加 artist/album；不扫 body），长短混合仍走 FTS。共享判据抽 `db::short_metadata_like_terms`（`char::is_alphanumeric` 覆盖 CJK；含符号病态输入不触发保零回归）。
**结果**：indexer 186（+4）/ local-index 27（+1 端到端）/ server 93 全绿；touched crate clippy `-D warnings`/fmt 净；locifindd 从 worktree 编译净。**真机 MCP 达成**：dummy.gguf → stub embedder → FTS-only daemon 挂手工 docx（`dc:creator=燎原 饶`、正文/文件名不含该串），HTTP MCP 搜「燎原」0→命中、「违约金」FTS 仍命中、「饶燎原」/「张三」0。ROADMAP 登 BETA-56（done）。**详录**：[session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-08 — Claude Code (Opus 4.8) — BETA-54 数字/编号检索 gap 修复 + Codex↔MCP 接线

**承接**：用户带 Codex 截图问「Codex 感觉绕过 MCP 直连索引库、是不是有问题」。诊断链：① 读 `search.rs`/`doc_db.rs` 纠正自己的 tokenizer 误判（FTS 是 **trigram**、数字本可子串命中）；② 查 `~/.codex/config.toml` + 全局状态实锤——**Codex 从没真正连上 MCP**（用户贴的 Claude 风格 `mcpServers` JSON 没进 Codex 的 TOML，只有 `node_repl` 一个 server），「绕行」= 无工具时自救。
**接线修复**：`setx LOCIFIND_MCP_TOKEN` + `codex mcp add locifind-local --url http://127.0.0.1:8766/mcp --bearer-token-env-var LOCIFIND_MCP_TOKEN`（codex-cli 0.142.5 原生支持 HTTP MCP）；端到端验证 `/health` 200 + `initialize`/`tools/list`/`tools/call search 仙本那`=3 命中 + `audit.jsonl` 新增 `action:"search"` 铁证。途中踩 token 轮换（旧 `1901d35a…` 401）+ 面板 token 不可复看 + `settings.json` 双路径分叉，最终用户 reset 取新 token 打通。
**BETA-54 修复**：真机复现真正的检索层 bug——`search 150138`/`440307` 0 命中但 `准考证` 命中同文档。根因 intent-parser `extract_en_residual_keywords`（`file_search.rs`）把纯数字 token 一律当 signal 剥；修 = `is_incidental_number`（<6 位才剥、≥6 位保留为字面 keyword）。desktop UI 与 daemon 共用 `parse` 一改两受益。242 单测全绿；重编 locifindd 挂含号码语料到 `:8788` 真机 MCP 实搜 `150138`/`440307`/`15013866763`/`440307201312314812`→命中、`2024` 仍 0（阈值有效）、`仙本那` 对照命中。**生效待桌面 app 出新版**（现跑 8766 仍旧码）。
**附带**：两 MCP token bug（token UX / 双 settings 路径）派后台会话 `task_7260d343`·`task_06f499be`；ROADMAP 登 BETA-54（done 代码层）。**详录**：[session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-07 V — Claude Code (Opus 4.8) — 桌面「本机 MCP 服务」BETA-53 S2/S3 code-done

**承接**：接上轮 S1（`attach_readonly` 只读挂载地基），用户「按推荐执行」→ 一并做 S2/S3 推到 code-done + 补端到端闸门 + 收工。
**产出**：① **server**——`DaemonConfigFile::personal_local(roots, token)`（桌面多 root 变体、全权 admin、`allow_full_read`）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内、桌面侧免直依赖 axum）+ **真 socket 起停集成测试**（`/health` 200 · `/mcp` 无 token 401 · shutdown 5s 内优雅返回，5× 稳定）。② **桌面 `mcp_service.rs`**——`McpServiceState` + 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`），复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`（同步拿端口占用错误）、随机 64-hex token、oneshot 优雅关停、开关态+token 持久化 settings.json、enabled 时自启。③ **前端 `McpPane.tsx`**——开关 / 运行状态〔地址·挂载条数·语义臂〕/ token 复制 / Claude Code 配置片段复制 / 重置令牌 / 安全提示；工具菜单入口〔`open-prefs-mcp`〕+ 选项页第八 tab。
**关键决策**：内嵌复用（非子进程）；roots 仅供 `list_collections` 展示（读取面由索引 db 边界天然约束）；安全红线只绑 127.0.0.1 + token 必填随机 + 暴露面 UI 明示 + 重置即踢连接。
**结果**：server lib 93 pass（+2）/ desktop 174 pass（+3）/ clippy `-D warnings`〔server·desktop·daemon〕/ fmt / tsc+vite 全绿；三方许可补 `getrandom`；设计文档 + ROADMAP BETA-53 标 code-done。
**真机验证达成（同会话续跑）**：功能 §2/§3/§4（harness + 对实跑 app curl）+ computer-use 驱动 dev app GUI 全流程 + 语义路径 B（`semantic-recall` 构建 harness：`semantic=true` + 中文 query 命中英文文档跨语言召回）三维均通过 → BETA-53 转 done；仅余可选「真 Claude Code 进程实连」（协议已 curl 验过）。
**发版**：bump **v0.9.20**（tauri.conf.json + Cargo.toml + Cargo.lock；含 BETA-53 本机 MCP 服务），推 `v0.9.20` tag 触发 release-windows.yml；Release changelog 补 MCP 服务用法 + 模型放置指引，待用户 Windows 真机测。

