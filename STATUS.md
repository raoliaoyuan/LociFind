# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（最新发版 **v0.9.19**）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**桌面「本机 MCP 服务」BETA-53 S2/S3 code-done**——接 S1 只读挂载地基：server 加 `personal_local` 多 root 构造器 + `serve_bound`（真 socket 起停集成测试）；桌面 `mcp_service.rs` 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`）复用桌面 embedder + 只读挂载 index.db、`127.0.0.1:8766`+随机 token+自启+持久化；前端 `McpPane.tsx`（开关/token 复制/配置片段/重置/安全提示）+ 工具菜单入口。验证 server 93 / desktop 174 / clippy·fmt·tsc+vite 全绿。**功能级 round-trip 已验**（真实 index.db 拷贝：只绑 127.0.0.1·token 鉴权·三工具·search 命中+read_document·reset，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)）。剩 GUI/真 Claude Code/语义路径 B。
- **下一步 top-3**：① **BETA-53 剩余真机项**（功能 round-trip 已验；剩 GUI 视觉 + 真 Claude Code 实连 + 语义命中〔`semantic-recall` 构建〕，[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）；② 设计伙伴/首个真实部署主动获取（护城河 P0）；③ **macOS 真机整体待跑**（出场线 Class A 唯一剩项；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-07 V（最新）**：**桌面「本机 MCP 服务」BETA-53 S2/S3 code-done**（详见同名会话日志 + [设计](docs/reviews/desktop-local-mcp-service-design.md)）。接 S1 只读挂载地基：① **server**——`DaemonConfigFile::personal_local(roots, token)`（桌面多 root 变体、全权 admin、`allow_full_read`）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内）+ 真 socket 起停集成测试；② **桌面 `mcp_service.rs`**——`McpServiceState` + 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`），复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`（同步拿端口占用错误）、随机 64-hex token、oneshot 优雅关停、开关态+token 持久化 + enabled 时自启；③ **前端 `McpPane.tsx`**——开关/运行状态/token 复制/Claude Code 配置片段/重置/安全提示 + 工具菜单入口 + 选项页第八 tab。安全红线：只绑 127.0.0.1 + token 必填随机 + 暴露面 UI 明示 + 重置即踢连接。验证：server lib 93 / desktop 174 / clippy `-D warnings`〔server·desktop·daemon〕/ fmt / tsc+vite 全绿；三方许可补 getrandom。剩真机验证。

## 下一步

1. **BETA-53 剩余真机项**（功能级 round-trip 已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：真实 index.db 拷贝跑通 §2/§3/§4——只绑 127.0.0.1·无/错 token 401·三工具·真实 search 命中+read_document·reset 停+轮换）：剩 ① GUI 开关/复制的视觉呈现、② 真 Claude Code 进程实连、③ 语义命中（带 `semantic-recall` 构建路径 B）——照 [playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)。
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

### 2026-07-07 V — Claude Code (Opus 4.8) — 桌面「本机 MCP 服务」BETA-53 S2/S3 code-done

**承接**：接上轮 S1（`attach_readonly` 只读挂载地基），用户「按推荐执行」→ 一并做 S2/S3 推到 code-done + 补端到端闸门 + 收工。
**产出**：① **server**——`DaemonConfigFile::personal_local(roots, token)`（桌面多 root 变体、全权 admin、`allow_full_read`）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内、桌面侧免直依赖 axum）+ **真 socket 起停集成测试**（`/health` 200 · `/mcp` 无 token 401 · shutdown 5s 内优雅返回，5× 稳定）。② **桌面 `mcp_service.rs`**——`McpServiceState` + 四命令（`start/stop_mcp_service`/`mcp_service_status`/`reset_mcp_token`），复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`（同步拿端口占用错误）、随机 64-hex token、oneshot 优雅关停、开关态+token 持久化 settings.json、enabled 时自启。③ **前端 `McpPane.tsx`**——开关 / 运行状态〔地址·挂载条数·语义臂〕/ token 复制 / Claude Code 配置片段复制 / 重置令牌 / 安全提示；工具菜单入口〔`open-prefs-mcp`〕+ 选项页第八 tab。
**关键决策**：内嵌复用（非子进程）；roots 仅供 `list_collections` 展示（读取面由索引 db 边界天然约束）；安全红线只绑 127.0.0.1 + token 必填随机 + 暴露面 UI 明示 + 重置即踢连接。
**结果**：server lib 93 pass（+2）/ desktop 174 pass（+3）/ clippy `-D warnings`〔server·desktop·daemon〕/ fmt / tsc+vite 全绿；三方许可补 `getrandom`；设计文档 + ROADMAP BETA-53 标 code-done。
**未尽事宜**：**真机验证**——带 `semantic-recall` 构建启动 app → 开关 → Claude Code 实连跑 `search`/`read_document` round-trip（依赖用户）。

### 2026-07-07 IV — Claude Code (Opus 4.8) — daemon 正斜杠 root bug 修复 + 桌面本机 MCP 服务设计 & S1

**承接**：用户问「能否工具菜单开关 BETA-43」→ 澄清诉求实为「让 Claude Code 经 MCP 检索本机文件」= **BETA-32 个人变体（非 BETA-43）**。本机跑通独立 daemon 验证（FTS-only、search 内容命中准考证），走通中发现 `read_document` round-trip bug → 用户「先查 bug 再实现 A」。
**关键决策**：桌面「本机 MCP 服务」走**内嵌**（非起子进程）——复用桌面已加载检索栈、**只读挂载**桌面 index.db（零重索引、语义白送）；端口 **8766**、只绑 `127.0.0.1` + token。
**产出**：① daemon bug 修复（正斜杠 root → `documents.path` 混合分隔符 → `\\?\` canonicalize 路径 lookup 落空，修 = root 入口 `normalize_root` 归一 + 单测；正斜杠 root 实测 round-trip OK，commit 9b55a1c）；② [设计提案](docs/reviews/desktop-local-mcp-service-design.md)（3 阶段 S1-S3）；③ **S1 done**：`ServerCtx::attach_readonly`（开现有 db 不跑首索引、复用传入 embedder + 单测）。
**结果**：locifind-server 91 pass / clippy `-D warnings` / fmt 净；BETA-53 登 ROADMAP。
**未尽事宜**：S2（Tauri 起停命令 + 设置持久化）/ S3（React UI）下轮。

### 2026-07-07 III — Claude Code (Opus 4.8) — v0.9.18/19 Windows 真机验证（computer-use 驱动）

**承接**：用户「我已装好，帮忙测试」→ computer-use 驱动装机版桌面 App 做非破坏性功能验证 + 用户手验卸载/升级。
**产出**：[真机验证报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)——**首轮 6 项**：基础搜索回归（50 条/229ms）/ BETA-47 七 tab（Windows 平台 tab 显示）/ BETA-51 设置统一（同义词→杂项 tab、隐私→隐私与记录 tab、返回路径完整）/ BETA-52 模型管理（当前模型显示 + 检测「✓可用·313.3MB」+ 扫描 gguf 全盘 3 份）/ **BETA-50 OCR 数字校正**（搜 `150138` 命中准考证 PNG、命中片段高亮 +【OCR数字校正】追加行）/ BETA-12 卸载·升级零损失（用户手验）。**续验 4 项**（同日 computer-use）：BETA-29 v1（草稿面板字段一致 + 移除 chip 重跑、时间窗保留）+ v2（Shift+Enter 预览「尚未执行搜索」+ 按此条件搜索真执行）/ BETA-33 cycle 9 单实例锁（tasklist 仅 1 进程、既有窗口置前）+ 设置流关闭守卫（脏态提示 + 放弃确认、配置零改动）。
**未尽事宜**：Windows 仅剩 BETA-49（依赖目录配置）/ BETA-43（需 daemon+LLM）/ BETA-33 WSearch 状态条·口径差（需停服务/造口径差）；**macOS 真机整体待跑**（Class A 出场线剩双平台 evals）。

### 2026-07-07 II — Claude Code (Opus 4.8) — enterprise 评测闸门加固（防假绿越权断言）

**承接**：用户问「本会话该做什么」→ 判定代码线已随 v0.9.19 追平、剩余主线卡真机验证 + 设计伙伴（均需用户）；选 BETA-44 eval 扩容后核实**卡片早已 done**（53 case、真机 53/53）+ 新 case 无法本机验真 + 卡片反对凑数 → 改向加固离线闸门。
**关键决策**：不再造合成 case；把越权负样本从"裸 `ACCESS_DENIED`"升级为"带机读墙目标"，让"信息墙真被测到"成为常跑 CI 可查（不依赖真机/模型）。
**产出**：`enterprise.rs` `Expectation::AccessDenied{target}` + parser `ACCESS_DENIED:<路径>`（运行期不消费、真机 `--require-all` 零回归）；queries.tsv 11 条越权补非空洞墙目标；`enterprise_scenarios_gate` +2 断言（无死 collection + 墙目标非空洞）；evals/README 校正 22→53 计数 + TSV 格式。
**结果**：lib 67 / gate 6（含 2 新断言）pass、clippy `-D warnings`/fmt 净。
**未尽事宜**：本轮纯 evals/fixture 不影响发版；真机验证清单不变（v0.9.18/19 六场景仍待用户）。

### 2026-07-07 — Claude Code (Opus 4.8) — 选项设置统一 + 语义模型状态/检测/自动发现 + v0.9.18/19 双发版

**承接**：用户问「本会话该做什么」→ 判定 BETA-47/48/49/50 code-done 未随包、发 **v0.9.18**；随后真机反馈两问题（同义词整页无返回入口 / 语义召回看不到当前模型），拍板补自动发现后一起发 **v0.9.19**。
**产出**：**BETA-51 设置统一入口**——「我的同义词」「隐私与数据」两独立整页收编进选项对话框 tab（`SynonymsPane` 内联杂项、`PrivacyPane` 折叠完整隐私内容），删 `/synonyms`·`/privacy` 路由与两页文件、工具菜单改开对应 tab；**BETA-52 语义模型管理增强**——`EmbedStatus::Ready` 带 `active_path`（显示当前模型）+ `probe_model_file`「检测」按钮 + `discover_gguf_models`「扫描本机 gguf」自动发现（everything `find_files_by_extension`、每项设为语义/生成回填路径、只填不复制不加载）。
**关键记录**：本机工具链确认可用（vcvars + 入仓 libclang），非 llama 门控改动跑无 feature `cargo check/clippy` ~1.5min 即验证（旧 memory「本机无 linker」作废、已更正）。
**结果**：tsc/vite/clippy `-D warnings`（修 `unnecessary_sort_by`）/171 desktop 测试 全绿；v0.9.18 + v0.9.19 双平台各 success、changelog 齐。
**未尽事宜**：v0.9.18/19 随真机验证（设置统一返回 / 模型检测·自动发现 / OCR 数字校正 等）。

