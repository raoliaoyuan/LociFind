# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中（三个真机报错修复随 **v0.9.34 双平台已发布 ✅**；BETA-63 + 语义臂逐条件补丁 v0.9.33 已发布；v0.9.27~31 详见会话日志）；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**2026-07-21 三个真机报错修复**——OCR 超大图缩放（`win_ocr.ps1` 按 `MaxImageDimension` 缩放） / docx-pptx zip EOCD 误报（Office 锁文件跳过 + 加密 CFB 可读提示） / 卸载默认保留索引（`uninstall-hooks.nsh` index.db 与模型同等保留），详见会话日志。**v0.9.34 双平台已发布 ✅**（2026-07-21）。
- **下一步 top-3**：① **设计伙伴/首个真实部署主动获取**（护城河 P0，ROADMAP §5；BETA-40 真实内网证据/BETA-44 语料扩充均以此为前提）；② **macOS 真机整体待跑**（出场线 Class A 唯一剩项；**v0.9.23 macOS DMG 已产出、具备真机测试前提**；Windows 真机 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)）；③ BETA-53 可选复核：真 Claude Code 进程连 `~/.claude/settings.json` 走一遍（[playbook](docs/reviews/beta-53-mcp-service-manual-verify.md)）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-21（最新）**：**三个真机报错追踪修复**（详见会话日志）——① OCR「Image dimensions are too large」：`win_ocr.ps1` 解码时按 `OcrEngine.MaxImageDimension` 缩放；② docx/pptx 索引「invalid Zip archive: Could not find EOCD」：Office 锁文件 `~$*` 跳过 + 加密 CFB 容器给可读 detail；③ 卸载重装丢索引：NSIS hook 把 index.db（+ -wal/-shm）纳入与模型同等的默认保留。**v0.9.34 双平台已发布 ✅**（2026-07-21，changelog 已补）。**未尽**：真机复测留用户（本机无 Rust/PowerShell/NSIS 工具链，全程只经人工审查 + CI 把关，CI 过程中确实揪出 2 个真问题，详会话日志）。

## 下一步

1. **BETA-53 剩余真机项**（功能级 + 真机 GUI 全流程已验，[报告](docs/reviews/beta-53-mcp-service-verify-2026-07-07.md)：harness 跑通 §2/§3/§4 + computer-use 驱动 dev app 实点——菜单/tab 路由·开关联动后端起停·token/配置片段复制·自启·旧设置迁移·对实跑 app curl 全通过）：**仅剩** ① 真 Claude Code 进程实连（协议已 curl 验过）、② 语义命中（`semantic-recall` 构建路径 B）——均依赖用户。
2. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
3. **真机验证剩余项**（Windows 10 项已过，[报告](docs/reviews/beta-manual-verify-2026-07-07-windows.md)：BETA-47/50/51/52/29〔v1+v2〕/33〔单实例锁·设置流〕 + 基础搜索 + BETA-12 卸载·升级）——**Windows 仅剩**：BETA-49 音乐发现不越界（依赖目录配置）、BETA-43 出处/`read_document`/审计导出（[playbooks README](docs/playbooks/README.md) 第 8/9 条，需 daemon + 外部 LLM；**其中 `read_document` 正斜杠 root round-trip bug 本轮已修**）、BETA-33 cycle 9 WSearch 状态条 / 全库-概貌口径差、**BETA-12 新增场景 4b/6（卸载默认保留索引，manual-test-scenarios 已补）**；**macOS 整体待跑**（按 [manual-test-scenarios](docs/manual-test-scenarios.md)）。
4. **发版进度**：…→ **v0.9.33**（BETA-63 补充：语义召回臂逐条件 AND/OR，2026-07-20 双平台已发布 ✅ 含 changelog）→ **v0.9.34**（三个真机报错修复：OCR 超大图/docx-zip EOCD/卸载保留索引，2026-07-21 双平台已发布 ✅ 含 changelog）。并发机制累计稳。
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

### 2026-07-21 — Claude Code (Sonnet 5) — 三个真机报错追踪修复：OCR 超大图 / docx-zip EOCD / 卸载默认清索引

**承接**：用户连续反馈三个真机问题。①Windows OCR 报「Image dimensions are too large」；②docx/pptx 索引报「invalid Zip archive: Could not find EOCD」；③新版本安装（用户澄清特指「先卸载旧版再装新版」这条手动路径）不该丢索引数据。**修复**：①`win_ocr.ps1` 解码超过 `OcrEngine.MaxImageDimension` 的图时用 `BitmapTransform` 等比缩放再识别，不再整图计 failed（此前 design 文档明写"v1 不做缩放"，本轮补齐）；②定位两个独立根因——Office 打开文档期间的隐藏锁文件 `~$*.docx` 被扫描器当真文档解析必然报 EOCD（`scan.rs::is_office_lock_file` 按 `~$` 前缀跳过，音乐/图片轮共用同一扫描骨架故一并生效）、加密/CFB 容器文档报错文本对用户不可读（`doc_extract.rs::zip_open_err_detail` 按 CFB 文件签名给可读 detail，未动 xlsx/calamine 路径避免误伤合法老版 .xls）；③用户确认要保留的场景是手动卸载重装（非覆盖安装升级，那条 `$UpdateMode` 分支早已保留）——`uninstall-hooks.nsh` 把 index.db（+ -wal/-shm）纳入与模型同等的默认保留（同一 MessageBox 问一次、静默默认保留），应用内「隐私页卸载清理」全删语义不变（用户主动清空数据的显式动作）。**踩坑核查**：另有用户提出"Claude Code MCP 配置 `type` 应为 local/remote"，经 WebFetch 官方文档（code.claude.com/docs/en/mcp）核实为误解——实际取值 stdio/http/sse/ws，local/remote 只是文档里的概念性说法非字段值，未改动代码。**未尽事宜**：真机复测（Windows 超大图 OCR / Office 锁文件 / 手动卸载重装保留索引三项）留用户，本机无对应环境跑不了。（同日追加：push 后 `ci.yml` **真揪出 2 个问题**——① `office_lock_file_tests` 里用反斜杠字面量模拟 Windows 路径的用例在 ubuntu 上失败（`\` 只在 Windows 是分隔符，Unix 构建 `Path::file_name()` 把整段路径当单一分量），拆成 `#[cfg(windows)]` 专属用例修复；② 一处 `assert!` 超 rustfmt 100 列宽被 `cargo fmt --check` 拦下，手动改成多行。两轮修复后 CI 转绿，证明"本机无法编译测试"这条免责声明不是走过场——`is_office_lock_file` 的测试代码本身有真 bug，若没有 CI 兜底会直接把坏测试合进 main。拍板发 **v0.9.34 并双平台发布成功**——Windows 16m34s / macOS 7m52s 均绿，产物齐〔`LociFind_0.9.34_x64-setup.exe` / `LociFind_0.9.34_aarch64.dmg` / `LociFind_aarch64.app.tar.gz`〕，Release 说明经 `gh release edit` 补全 changelog；待用户真机复测确认。）

### 2026-07-20 — Claude Code (Sonnet 5) — BETA-63 多复合条件检索全局匹配模式（AND/OR 可选，移除 BETA-57 静默兜底）

**承接**：用户反馈「多条件检索返回大量不符合要求的结果」，要求梳理索引构建与检索命中逻辑并优化。**诊断**：`LocalIndexBackend::search_results_expanded`（[lib.rs](packages/search-backends/local-index/src/lib.rs)）里 BETA-57 遗留的「组间 AND 0 命中静默放宽为 OR」自动兜底——用户无感知地被扩大召回，只命中部分条件的结果混入。**关键决策**（用户三点拍板）：① All 模式 0 命中即 0、不再静默放宽；② 全局默认 All；③ 四个检索后端（local-index/windows-search/everything/spotlight）统一生效。**产出**：`packages/search-backends/common` 新增 `MatchMode` 枚举挂 `ExpandedSearchIntent.match_mode` 单一信源；四后端组间连接逻辑均改按 `match_mode` 取 AND/OR（结构性约束如扩展名/时间/大小/路径恒 AND、不受影响）；local-index 移除 `fts_or_relax_from_groups`。桌面端 `AppSettings.search_match_all_conditions`（默认 true）+「常规」面板下拉框 + live-read provider；daemon 新增 CLI `--match-any-condition`（无 settings.json、启动期一次性注入）；桌面内嵌 MCP 服务读同一份桌面设置。全 workspace `cargo test`/`clippy -D warnings`/`fmt` 净（daemon e2e 3 个失败经 `git stash` 对照基线确认系本机沙盒临时目录路径问题、与本次改动无关，详见 ROADMAP BETA-63 卡）。**未尽事宜**：无。（同日追加：拍板发 **v0.9.32 并双平台发布成功**——Windows 15m52s / macOS 10m25s 均绿，产物齐〔`LociFind_0.9.32_x64-setup.exe` / `LociFind_0.9.32_aarch64.dmg` / `LociFind_aarch64.app.tar.gz`〕，Release 说明经 `gh release edit` 补全 changelog；用户本地安装测试中。）

### 2026-07-20（续）— Claude Code (Sonnet 5) — BETA-63 补充：语义召回臂逐条件 AND/OR（v0.9.32 真机复盘）

**承接**：用户装 v0.9.32 真机测试「2025年 开发部 述职报告」，反馈"全部命中"仍只按"述职报告"一个条件过滤。**排查过程**：先用 `cargo run -p locifind-cli --intent-only --json` 本地重跑 parser 验证「季度 财务 报告」等词组，确认「报告」类通用容器名词按既有设计被丢弃、2 字 CJK 词受 trigram 限制——这两者是**既有已知限制、非本次引入**，用向用户报告；用户给出精确复现词组「开发部」+「述职报告」（均非短词/非通用词，理应正常进 keyword_groups）后，继续深挖到**真正根因**：`SemanticIndexBackend::search_expanded` 完全不消费 `keyword_groups`/`match_mode`，把整句关键词拼接成一个向量做相似度召回，RRF 融合时把只贴合其中一个条件的文档也带进最终结果，绕开 FTS 臂的严格 AND。**方案决策**（用户主导讨论权衡）：不是简单砍掉语义臂（会损失同义/跨语言模糊召回），改为**折中**——语义臂逐词组分别 embed 算相似度（组内同义词仍 OR-取最大值），按 `match_mode` 汇总多个条件（All 取最小值一票否决、Any 取最大值），单/零词组零变化。**产出**：`packages/search-backends/semantic-index` 新增 `search_results_expanded`，新增 2 测试用例（All 一票否决 both.txt / 单词组回归零变化），25 测试全绿；harness 191 + server 95 + desktop 183 无回归；clippy `-D warnings`/fmt 净。**未尽**：无。（同日追加：拍板发 **v0.9.33 并双平台发布成功**——Windows / macOS〔11m42s〕均绿，产物齐〔`LociFind_0.9.33_x64-setup.exe` / `LociFind_0.9.33_aarch64.dmg` / `LociFind_aarch64.app.tar.gz`〕，Release 说明经 `gh release edit` 补全 changelog；待用户用原始复现词组复测确认。）

> 更早历史（v0.9.31 及以前）已归档：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md)。

