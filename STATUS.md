# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（**v0.9.29 热修真机栈溢出崩溃**——某台机装机后偶发崩，异常码 `0xC00000FD`〔`STATUS_STACK_OVERFLOW`〕、故障模块为 `locifind-desktop.exe` 自身：根因是 `pdf-extract`/`lopdf` 解析深层 PDF 递归过深撑爆 rayon 提取 worker 的默认 ~2 MiB 栈；栈溢出是 SEH 非 panic，现有 `catch_unwind`/`panic=unwind` 兜不住 → 给提取线程池设 `stack_size=64 MiB`；纯运行期修复、不需重建索引；v0.9.27 并入 BETA-60、v0.9.28 热修 BETA-60 两处索引回退）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**2026-07-09 Codex 查身份证文件复盘 → BETA-58/59 分派两 CLI 并落地（并入 main 待发版）**——用户带 Codex 全程截图（手搓 HTTP 3m52s）问「工具/MCP 还能优化啥」。Claude Code 当项目经理拆两条不重叠赛道并行：**BETA-58 接入体验**（前端子代理：McpPane 客户端切换 + Codex `mcp add` 命令 + MSIX 重登警告 + curl 全 Accept 头）+ **BETA-59 PII 概念检索**（Codex CLI：索引时识别身份证〔GB 11643 校验〕/手机号、注入类型关键词到 FTS，「身份证」概念词召回）。Claude Code 复核双 diff、跑通 tsc/clippy/test。**已发 v0.9.26**（BETA-57+58+59 一并 bump、推 tag 触发 CI 双平台发布）。BETA-59 生效需重建索引。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；**v0.9.23 macOS DMG 已产出、具备真机测试前提**；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-09（最新³）**：**v0.9.29 热修真机栈溢出崩溃（PDF 提取递归撑爆线程栈）**。用户报某台 Windows 装机后偶发崩溃，异常码 `0xC00000FD`（`STATUS_STACK_OVERFLOW`）、故障模块 `locifind-desktop.exe` 自身、偏移固定。诊断：索引期 `pdf-extract 0.10`（内部 `lopdf`，静态链进 exe）解析深层嵌套/畸形 PDF 时递归很深，BETA-60 新建的受限提取 rayon 线程池（[scan.rs](packages/indexer/src/scan.rs)）未设栈大小、worker 拿 std 默认 ≈2 MiB → 被撑爆。**关键**：栈溢出是 SEH 异常、**不是 panic**，`catch_extract` 的 `catch_unwind` 与 release 的 `panic="unwind"` 都兜不住，直接 abort 整个 app——完美吻合「偶发/单机/模块为 exe/偏移固定」。**修**：给 `extract_pool` 加 `stack_size=64 MiB`（新常量 `EXTRACT_STACK_SIZE`，并发已被 `EXTRACT_PARALLELISM=4` 限死、栈按需提交、开销可忽略）；改的是共享 `locifind-indexer` crate，desktop + `locifindd` 两侧同受益。indexer scan 测试 44/44 + clippy 净。**纯运行期修复、不需重建索引**，重构建即生效。待发 **v0.9.29**。**未尽**：若同机仍复现（真·成环递归），再上 PDF 提取子进程隔离；可向用户取触发 PDF 做回归 fixture。（BETA-59 独立 `entity` 列重构已并入本分支、详见会话日志）

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：…→ **v0.9.24**（BETA-56 短 CJK 兜底）→ **v0.9.25**（BETA-57 AND→OR 兜底）→ **v0.9.26**（BETA-57/58/59）→ **v0.9.27**（BETA-60 检索+索引性能）→ **v0.9.28**（热修 BETA-60 两处索引回退）→ **v0.9.29**（热修真机栈溢出崩溃：PDF 提取线程池加 64 MiB 栈）。**待推 `v0.9.29` tag 触发 CI 双平台发布**。并发机制累计稳。
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

### 2026-07-09 — Claude Code (Opus 4.8) — v0.9.29 热修：真机栈溢出崩溃（PDF 提取递归撑爆线程栈）

**承接**：用户报某台 Windows 装机后偶发崩溃，异常码 `0xC00000FD`（`STATUS_STACK_OVERFLOW`）、故障模块 `locifind-desktop.exe` 自身、偏移 `0xc4c842` 固定。**诊断**（读 indexer 提取链）：索引期 `pdf-extract 0.10`（内部 `lopdf`、静态链进 exe）解析深层嵌套/畸形 PDF 时递归很深；BETA-60 新建的受限提取 rayon 线程池（[scan.rs:~360](packages/indexer/src/scan.rs)）**未设栈大小**，worker 拿 std 默认 ≈2 MiB → 被撑爆。**关键洞察**：栈溢出是 SEH 异常、**不是 panic**——[scan.rs:616](packages/indexer/src/scan.rs) 注释只覆盖 panic 一路，`catch_extract` 的 `catch_unwind` 与 release `panic="unwind"`（[Cargo.toml:107](apps/desktop/src-tauri/Cargo.toml)）都拦不住，直接 abort 整个 app；完美吻合「偶发/单机/模块为 exe/偏移固定」。**产出**：给 `extract_pool` 加 `.stack_size(64 MiB)`（新模块级常量 `EXTRACT_STACK_SIZE` + 根因 doc 注释；并发已被 `EXTRACT_PARALLELISM=4` 限死、栈按需提交、开销可忽略）；改的是共享 `locifind-indexer` crate → desktop + `locifindd` 两侧同受益。indexer scan 测试 44/44 通过、clippy 净（常量移模块级避 `items_after_statements`）。**纯运行期修复、不需重建索引**，重构建即生效。bump **v0.9.29**（tauri.conf/Cargo.toml/lock）。**未尽事宜**：若同机仍复现（真·成环递归 64 MiB 也救不了），再上 PDF 提取**子进程隔离**（崩了只丢一个文件）；可向用户取触发崩溃的 PDF 做回归 fixture 坐实「深但有限 vs 成环」。待推 `v0.9.29` tag 触发 CI 双平台发布。

### 2026-07-09 — Claude Code (Opus 4.8) — v0.9.28 热修：BETA-60 索引进度回退（误判卡死）

**承接**：用户装 v0.9.27 后，索引中途硬关程序、重开感觉「进度卡死不动」。**现场取证**（Claude Code 直接在用户 Windows 机上 tasklist/sqlite3 只读排查）：装的确是 0.9.27；主库 `%APPDATA%\LociFind\index.db` **完好**——WAL 模式崩溃恢复成功、已 checkpoint 归零、documents=67 + passages=51 在，**我这轮 WAL 改动未致数据丢失**（一度 `sqlite3 -readonly` 读到 0 是没应用 WAL 的旧快照虚惊，WAL-aware `mode=ro` 读到 67）；无 desktop 派生的卡住 OCR 子进程；WAL 时间戳在重开后仍前进＝索引其实在跑。**真因**：BETA-60 Track B 把 `on_file` 放在「128/块并行提取完的块尾串行段」，一块处理期间进度计数器完全不动，块内有慢文件（大 PDF/图 OCR）就冻几十秒→误判卡死。**修①（进度冻结）**：`scan.rs` 把 `on_file` 下沉进并行 `par_iter` map、逐文件提取完即报（`IndexProgress` 本 `Send+Sync` 专为跨线程设计）；串行段不再重复 on_file；`EXTRACT_CHUNK` 128→64。**修②（子进程风暴，同轮真机续查暴露）**：发 v0.9.28 前用户首索引 50 文件卡「0/50」十分钟，现场查出 desktop 派生 **17 个 `pdftoppm.exe`** 并发——按核数并行提取时，多份扫描 PDF 各 spawn `pdftoppm`（一份一进程、200DPI 整份渲染）+ 逐页 OCR，子进程互抢打爆机器、整体更慢。加 `EXTRACT_PARALLELISM=4` 受限 rayon 线程池（`pool.install`）把重量级提取并发压到 4 路（取 min(4, 核数)）。indexer 194 测试 + clippy/fmt + `cargo check -p locifind-desktop` 全绿。（发 v0.9.28 前已取消不充分的首版构建、重发含两修的 v0.9.28；真机无限 hang 无证据：OCR/pdftoppm 均有超时、坏文件返 Err 已妥处）

### 2026-07-09 — Claude Code (Opus 4.8) — BETA-59 独立 `entity` 列重构（PII 类型词与展示正文隔离）

**承接**：BETA-59 首版 PII 类型词并入 `documents_fts.body`，遗留「正文无字面标签、仅命中注入词时 `snippet()` 回显关键词尾巴」的出处观感缺陷（非隐私/正确性）。**产出**：`documents_fts` 加末列 `entity`（列序 title=0/author=1/body=2/entity=3 不变），类型词改写 `entity`——`query` 裸 `MATCH` 自动跨列命中，`snippet()`/`preview` 固定 body 列**永不回显** entity；`pii.rs` `append_pii_keywords_for_fts`→`pii_entity_keywords`。**迁移不 bump schema**：`migrate_documents_fts_entity` 就地**拷 body 进新 4 列表再 drop+rename**（body 是正文唯一存处、不能仿 `migrate_music_fts` 从主表重建；entity 灌空待增量回填），升级无运行时崩（4 列 INSERT 只在迁移后跑）、老正文照常可搜，与既有两处"透明加列"同套路；bump 版本逼全库重建代价不划算故舍。indexer 195/197 + server 93 全绿、clippy `-D warnings`/fmt 净（+entity 命中/snippet 不回显/3 列老库迁移保 body 三测）。已 rebase 到 v0.9.29、与 BETA-60 WAL/分块提取同文件融合；**PR [#6](https://github.com/raoliaoyuan/LociFind/pull/6)**。

