# STATUS 归档（2026-07）

> 本文件承接 [STATUS-archive-2026-06.md](./STATUS-archive-2026-06.md)。
> **2026-07-02 II 会话（文档重整：定位收敛 + STATUS 瘦身骨架落地，详 [docs/reviews/doc-realign-retrieval-foundation.md](../reviews/doc-realign-retrieval-foundation.md)）将当时的 STATUS.md 全文逐字归档于此**，随后 STATUS.md 重构为「速览 / 当前 Task / 下一步 / 阻塞 / 会话日志摘要」固定骨架。
> 后续会话的详录写入 session-details-YYYY-MM.md、溢出摘要滚动追加到本文件。

---
### 2026-07-09 — Claude Code (Opus 4.8) — BETA-59 独立 `entity` 列重构（PII 类型词与展示正文隔离）

**承接**：BETA-59 首版 PII 类型词并入 `documents_fts.body`，遗留「正文无字面标签、仅命中注入词时 `snippet()` 回显关键词尾巴」的出处观感缺陷（非隐私/正确性）。**产出**：`documents_fts` 加末列 `entity`（列序 title=0/author=1/body=2/entity=3 不变），类型词改写 `entity`——`query` 裸 `MATCH` 自动跨列命中，`snippet()`/`preview` 固定 body 列**永不回显** entity；`pii.rs` `append_pii_keywords_for_fts`→`pii_entity_keywords`。**迁移不 bump schema**：`migrate_documents_fts_entity` 就地**拷 body 进新 4 列表再 drop+rename**（body 是正文唯一存处、不能仿 `migrate_music_fts` 从主表重建；entity 灌空待增量回填），升级无运行时崩（4 列 INSERT 只在迁移后跑）、老正文照常可搜，与既有两处"透明加列"同套路；bump 版本逼全库重建代价不划算故舍。indexer 195/197 + server 93 全绿、clippy `-D warnings`/fmt 净（+entity 命中/snippet 不回显/3 列老库迁移保 body 三测）。已 rebase 到 v0.9.29、与 BETA-60 WAL/分块提取同文件融合；**PR [#6](https://github.com/raoliaoyuan/LociFind/pull/6)**。

---
### 2026-07-09 — Claude Code (Opus 4.8) — BETA-60 检索+索引性能优化（双赛道并行）

**承接**：用户真机反馈搜「身份证」1965ms（fan-out local+semantic+windows+everything 四臂）+ 索引慢，要 Claude Code 当项目经理分派本地 Claude CLI 与 Codex CLI 并监督、尽量不打扰、下版问题都解决。**诊断**（两 Explore 代理并行摸链路）：搜索 fan-out `fanout_merge.rs` 串行 await 各后端耗时**相加**、Windows Search 每查 spawn powershell+ADODB 冷启动是最大单头；索引 `scan.rs` walk 单线程 + 每文件事务 + **未开 WAL**（每文件 fsync）+ 提取/OCR 无并发。**分派**（文件不重叠双赛道并行）：Track A 搜索并发化（本地 Claude 子代理，harness+desktop）、Track B 索引提速（Codex CLI `codex exec --sandbox workspace-write`，indexer），两者禁碰 git/STATUS/ROADMAP。**产出**：A = harness 抽纯 fuse 函数（语义逐字节等价、daemon 路径改调它）+ desktop `concurrent_collect` 用 `spawn_blocking`+`block_on` 并发查各后端按原序回收（求和→取最慢）；B = 文件型库开 WAL+`synchronous=NORMAL`（内存库兜底）+ `scan.rs` 128/块 rayon 并行提取（串行预检→并行提取→串行写库/进度）。**复核**：逐段核 desktop 并发编排取消/兜底/顺序对齐；Codex 首版全量堆内存 → 令其改分块（限内存尖峰+保实时进度）；集成 `cargo check -p locifind-desktop`〔server/backends 调用方随 `Send`/`Sync` bound 一起过〕+ harness 191/indexer 194/clippy/fmt 全绿。**未尽**：Windows/Everything 常驻宿主、embedding batch/context 复用（涉 RAM/共享 runtime 风险高本轮不做）、fan-out 软超时。搜索不需重建索引、WAL/并行提取下次构建生效。待发 **v0.9.27**。

---
### 2026-07-09 — Claude Code (Opus 4.8) — 分派两 CLI 落地 BETA-58/59（MCP 接入体验 + PII 概念检索）

**承接**：用户带 Codex 查身份证文件全程截图，要求 Claude Code 当项目经理、把优化任务分派给本地 Claude CLI 与 Codex CLI 并监督完成，尽量不打扰用户、下版测试时问题都解决。**关键决策**：拆两条**文件不重叠**赛道并行——前端（TS，`apps/desktop`）分本地 Claude 子代理、Rust（`packages/**`）分 Codex CLI（`codex exec --sandbox workspace-write`），两者禁碰 STATUS/ROADMAP 与 git，收工归并由 Claude Code 统一做。**产出**：BETA-58（接入体验）+ BETA-59（PII 概念检索）均 done、并入 main 待发版（详见「当前 Task」+ ROADMAP）；Claude Code 复核双 diff、补 curl 的 PowerShell 提示 + doc_db 注入点权衡注释、跑通 tsc/clippy/test。**未尽事宜**：待推 **v0.9.26**（含 BETA-57/58/59）触发 CI 双平台发布；BETA-59 生效需重建索引；后续可提 `entity` 独立 FTS 列消除 snippet 注入词回显边缘。

---
### 2026-07-09 — Claude Code (Opus 4.8) — BETA-57 多词查询组间 AND→OR 召回兜底

**承接**：用户经 MCP 查体检材料，报「`体检 体检报告 健康检查 健康体检` 泛查 0 命中、单词能命中」。诊断纠偏：并非我起初判的 `fts_sanitize` 短语化（那条只在无词组的 raw-text 兜底触发、生产不走），真因是 `fts_match_from_groups` **组间 AND**——parser 拆多词组、缺任一词即整条结构性归零。`爱康`(2 字正文词)另属 BETA-56 兜底不扫 body 的已知边界 + 验证 daemon 跑 dummy.gguf 语义臂死，非本次范围。
**产出（方案 A：AND 优先 + 0 命中 OR 兜底）**：`search_results_expanded`（desktop+MCP 收敛点）AND 空且 ≥2 有效词组时经新 `fts_or_relax_from_groups` 放宽组间 OR 重试一次；零精确性回归（仅空时触发）；抽 `sanitized_group_terms` 消重。local-index +2 测试（单元 + 端到端复现体检报告场景 + AND 命中不受影响对照），29 全绿 / clippy `-D warnings` / fmt 净。查询侧改动**不需重建索引**。
**收尾（同日续）**：重编 `locifindd`〔release+llama-cpp，4m46s〕+ desktop NSIS〔6m40s〕→ **bump v0.9.25**（tauri.conf/Cargo.toml/lock）→ **真机 MCP 验证达成**：另编 FTS-only stub daemon〔dummy.gguf〕挂体检语料，`健康检查` 单搜 0（缺席）/ 多词 `体检 体检报告 健康检查 健康体检`〔含缺席词、旧 AND 必 0〕经 OR 兜底命中，audit.jsonl 三条 results 佐证。待推 `v0.9.25` tag 触发 CI 发布。分析层（「总结健康状态」）仍是外部 LLM 的活、LociFind 只管检索（范围不变）。

---
### 2026-07-08 — Claude Code (Opus 4.8) — Codex↔MCP 接线 + BETA-54/55 + v0.9.23 双平台发布

**承接**：用户带 Codex 截图问「是否绕过 MCP」→ 实锤 Codex 从没挂上（Claude JSON 没进 Codex TOML）；修接线后稳走 MCP。
**BETA-54 数字检索**：`file_search.rs` `extract_en_residual_keywords` 无条件剥纯数字 → `is_incidental_number`（<6 位才剥），desktop+MCP 共用 `parse` 一改两受益；242 测试。
**BETA-55 索引最后保存者**：`doc_extract.rs` `read_core_props` 加抽 `cp:lastModifiedBy` 经 `combine_authors` 并入 author FTS，xlsx 另开 zip 补 core props；doc_extract 25 pass。生效需清空索引重建。
**发布**：三分支收敛为单一 main（cherry-pick playbook + 强推 origin/main）；本机出 Windows 装机版真机验（`15013866` 命中 / author 带最后保存者）→ **v0.9.23 tag → 双平台发布**；CI 修 clippy `manual_range_contains` + fmt 遗留后全绿，macOS npm ERESOLVE flake 重跑过。**收尾**：清后台 worktree（2 个已并入的删了）+ **BETA-56 短 CJK 兜底 cherry-pick 并入 main**（indexer +4/local-index +1，本机 fmt/clippy/test 全过；待下个发版）。派生 task：短 CJK（done BETA-56）/ token UX / npm lockfile。

---
### 2026-07-08 — Claude Code (Opus 4.8) — 修复本机 MCP 服务 token 持久化分叉

**承接**：2026-07-08 Codex 接 MCP 排查（memory `mcp-token-ux-dual-settings-bug`）暴露矛盾态——运行态持 token（`/health` 200、旧 token 401）但磁盘 settings.json 显示 token=null/enabled=false。
**根因**：**非**双数据目录（后端与 UI 同写 `app_config_dir/settings.json`）；实为**双写者覆盖**——MCP token/enabled 后端带外写盘，偏好表单 `update_settings` 全量覆写时用挂载期旧快照把其冲成 null，运行中 axum server 仍持内存旧 token → 401 静默失效。
**产出**：[settings.rs](../../apps/desktop/src-tauri/src/settings.rs) `update_settings` 改为写盘前读磁盘、合并回后端带外管理的 `mcp_service_enabled`/`mcp_service_token`（`merge_backend_managed_mcp_fields` + 可测 `update_settings_at` 内核），磁盘成 MCP 两字段唯一信源；`settings.rs` +2 测试（clobber 回归 / 首存无文件）、[mcp_service.rs](../../apps/desktop/src-tauri/src/mcp_service.rs) +1 测试（status↔磁盘 token 一致守卫）；doc_markdown·field_reassign 已按 CI pedantic 核对。
**未尽事宜**：本机无 MSVC 工具链无法本地 `cargo test`/clippy，编译验证靠 CI；未 bump 版本，随下个发版携带。

---
### 2026-07-08 — Claude Code (Opus 4.8) — MCP 令牌重置 UX 小修（发版后）

**承接**：任务据「Codex 接 MCP 排查」印象报「面板只弹一次 token + 缺重置按钮」→ 复现发现二者早在 e1f3048（2026-07-07）已具备（token 随 3s 轮询常驻、重置按钮在列），任务描述来自旧装机版。用户拍板：把唯一真实缺口——`reset_token` 停服务后需手动重启——**改为自动重启**。
**产出**：`mcp_service.rs` `reset_token` 记录重置前运行态，停服务（踢旧连接，§5.2）+ 轮换 token 后，**若原本在跑则自动 `start()` 复用新 token 重启**（旧 token 立即 401、新 token 立即 200，免手动重开）；停止态则仅换 token。`McpPane.tsx` 重置提示文案同步；补停止态轮换 `#[tokio::test]`。
**结果**：`cargo check --tests`〔locifind-desktop〕绿、`cargo test mcp_service` 4 pass（含新测）；前端仅改一处中文提示串。playbook §4 / ROADMAP BETA-53 同步。本 reset 小修与上条 401 分叉修复（同日并行会话）已一并并入 main（异文件、互不冲突）。
**待验**：运行态自动重启的真机 401/200（需构建/起 app，本轮未做——复用已验的 start()/stop() 原语 + 单测覆盖停止态）。

### 2026-07-07 V — Claude Code (Opus 4.8) — 桌面「本机 MCP 服务」BETA-53 S2/S3 code-done

**承接**：接上轮 S1（`attach_readonly` 只读挂载地基），用户「按推荐执行」→ 一并做 S2/S3 推到 code-done + 补端到端闸门 + 收工。
**产出**：① **server**——`DaemonConfigFile::personal_local(roots, token)`（桌面多 root 变体、全权 admin、`allow_full_read`）+ `app::serve_bound(listener, ctx, shutdown)`（axum 封装在 server 内）+ **真 socket 起停集成测试**（`/health` 200 · `/mcp` 无 token 401 · shutdown 5s 内优雅返回）。② **桌面 `mcp_service.rs`**——`McpServiceState` + 四命令，复用桌面 embedder + 只读挂载 index.db、bind `127.0.0.1:8766`、随机 64-hex token、oneshot 优雅关停、持久化 + enabled 时自启。③ **前端 `McpPane.tsx`**——开关/运行状态/token 复制/配置片段/重置/安全提示 + 工具菜单入口 + 选项页第八 tab。
**结果**：server lib 93 / desktop 174 / clippy `-D warnings` / fmt / tsc+vite 全绿；三方许可补 `getrandom`。**真机验证达成**：功能 §2/§3/§4 + computer-use GUI 全流程 + 语义路径 B 三维均通过 → BETA-53 转 done。**发版 v0.9.20**（含 BETA-53 本机 MCP 服务）。

### 2026-07-07 IV — Claude Code (Opus 4.8) — daemon 正斜杠 root bug 修复 + 桌面本机 MCP 服务设计 & S1

**承接**：用户问「能否工具菜单开关 BETA-43」→ 澄清诉求实为「让 Claude Code 经 MCP 检索本机文件」= **BETA-32 个人变体（非 BETA-43）**。本机跑通独立 daemon 验证（FTS-only、search 内容命中准考证），走通中发现 `read_document` round-trip bug → 用户「先查 bug 再实现 A」。
**关键决策**：桌面「本机 MCP 服务」走**内嵌**（非起子进程）——复用桌面已加载检索栈、**只读挂载**桌面 index.db（零重索引、语义白送）；端口 **8766**、只绑 `127.0.0.1` + token。
**产出**：① daemon bug 修复（正斜杠 root → `documents.path` 混合分隔符 → `\\?\` canonicalize 路径 lookup 落空，修 = root 入口 `normalize_root` 归一 + 单测；正斜杠 root 实测 round-trip OK，commit 9b55a1c）；② [设计提案](../reviews/desktop-local-mcp-service-design.md)（3 阶段 S1-S3）；③ **S1 done**：`ServerCtx::attach_readonly`（开现有 db 不跑首索引、复用传入 embedder + 单测）。
**结果**：locifind-server 91 pass / clippy `-D warnings` / fmt 净；BETA-53 登 ROADMAP。
**未尽事宜**：S2（Tauri 起停命令 + 设置持久化）/ S3（React UI）下轮。

---
### 2026-07-07 III — Claude Code (Opus 4.8) — v0.9.18/19 Windows 真机验证（computer-use 驱动）

**承接**：用户「我已装好，帮忙测试」→ computer-use 驱动装机版桌面 App 做非破坏性功能验证 + 用户手验卸载/升级。
**产出**：[真机验证报告](../reviews/beta-manual-verify-2026-07-07-windows.md)——**首轮 6 项**：基础搜索回归（50 条/229ms）/ BETA-47 七 tab（Windows 平台 tab 显示）/ BETA-51 设置统一（同义词→杂项 tab、隐私→隐私与记录 tab、返回路径完整）/ BETA-52 模型管理（当前模型显示 + 检测「✓可用·313.3MB」+ 扫描 gguf 全盘 3 份）/ **BETA-50 OCR 数字校正**（搜 `150138` 命中准考证 PNG、命中片段高亮 +【OCR数字校正】追加行）/ BETA-12 卸载·升级零损失（用户手验）。**续验 4 项**（同日 computer-use）：BETA-29 v1（草稿面板字段一致 + 移除 chip 重跑、时间窗保留）+ v2（Shift+Enter 预览「尚未执行搜索」+ 按此条件搜索真执行）/ BETA-33 cycle 9 单实例锁（tasklist 仅 1 进程、既有窗口置前）+ 设置流关闭守卫（脏态提示 + 放弃确认、配置零改动）。
**未尽事宜**：Windows 仅剩 BETA-49（依赖目录配置）/ BETA-43（需 daemon+LLM）/ BETA-33 WSearch 状态条·口径差（需停服务/造口径差）；**macOS 真机整体待跑**（Class A 出场线剩双平台 evals）。

---
### 2026-07-07 II — Claude Code (Opus 4.8) — enterprise 评测闸门加固（防假绿越权断言）

**承接**：用户问「本会话该做什么」→ 判定代码线已随 v0.9.19 追平、剩余主线卡真机验证 + 设计伙伴（均需用户）；选 BETA-44 eval 扩容后核实**卡片早已 done**（53 case、真机 53/53）+ 新 case 无法本机验真 + 卡片反对凑数 → 改向加固离线闸门。
**关键决策**：不再造合成 case；把越权负样本从"裸 `ACCESS_DENIED`"升级为"带机读墙目标"，让"信息墙真被测到"成为常跑 CI 可查（不依赖真机/模型）。
**产出**：`enterprise.rs` `Expectation::AccessDenied{target}` + parser `ACCESS_DENIED:<路径>`（运行期不消费、真机 `--require-all` 零回归）；queries.tsv 11 条越权补非空洞墙目标；`enterprise_scenarios_gate` +2 断言（无死 collection + 墙目标非空洞）；evals/README 校正 22→53 计数 + TSV 格式。
**结果**：lib 67 / gate 6（含 2 新断言）pass、clippy `-D warnings`/fmt 净。
**未尽事宜**：本轮纯 evals/fixture 不影响发版；真机验证清单不变（v0.9.18/19 六场景仍待用户）。

---
### 2026-07-07 — Claude Code (Opus 4.8) — 选项设置统一 + 语义模型状态/检测/自动发现 + v0.9.18/19 双发版

**承接**：用户问「本会话该做什么」→ 判定 BETA-47/48/49/50 code-done 未随包、发 **v0.9.18**；随后真机反馈两问题（同义词整页无返回入口 / 语义召回看不到当前模型），拍板补自动发现后一起发 **v0.9.19**。
**产出**：**BETA-51 设置统一入口**——「我的同义词」「隐私与数据」两独立整页收编进选项对话框 tab（`SynonymsPane` 内联杂项、`PrivacyPane` 折叠完整隐私内容），删 `/synonyms`·`/privacy` 路由与两页文件、工具菜单改开对应 tab；**BETA-52 语义模型管理增强**——`EmbedStatus::Ready` 带 `active_path`（显示当前模型）+ `probe_model_file`「检测」按钮 + `discover_gguf_models`「扫描本机 gguf」自动发现（everything `find_files_by_extension`、每项设为语义/生成回填路径、只填不复制不加载）。
**关键记录**：本机工具链确认可用（vcvars + 入仓 libclang），非 llama 门控改动跑无 feature `cargo check/clippy` ~1.5min 即验证（旧 memory「本机无 linker」作废、已更正）。
**结果**：tsc/vite/clippy `-D warnings`（修 `unnecessary_sort_by`）/171 desktop 测试 全绿；v0.9.18 + v0.9.19 双平台各 success、changelog 齐。
**未尽事宜**：v0.9.18/19 随真机验证（设置统一返回 / 模型检测·自动发现 / OCR 数字校正 等）。

---
### 2026-07-06 VI — Claude Code (Fable 5) — BETA-50 OCR 数字校正（真机准考证误识诊断 + 沉淀）

**承接**：用户问「为什么搜 150138 找不到准考证 PNG 内容」→ 实机诊断 index.db：图已入库、trigram 子串匹配正常，根因 = Windows OCR 把 5 识成 S（`15013866763` → `1 S013866763`）+ 空格拆组 → 用户拍板「现在就做」索引端校正。
**产出**：indexer `digit_correction_variants`（易错字母 S/O/I·l/B/Z → 数字 + 跨单空格分组合并；保守规则：真数字 ≥4 且易错 ≤2、纯数字分组 ≥2 且 ≥6 位）+ `finalize_ocr_text` 收口（**原文保留**、变体以〔OCR数字校正〕行追加，trigram 子串两态可搜）；两 OCR 引擎 + 扫描 PDF 逐页管线共享。
**结果**：indexer 182（+5：真机 case 四连 / 保守反例 / doc_db FTS e2e）、local-index 26、desktop + server 全量 exit 0；clippy/fmt 净。
**未尽事宜**：随下次发版生效；存量图片 mtime skip、需清空索引重建才带变体；locifindd 下次构建须重编（indexer 变更）。

---
### 2026-07-06 V — Claude Code (Fable 5) — BETA-48 修复 + BETA-49 音乐发现按 roots 过滤

**承接**：BETA-47 收工后用户指示继续处理两条顺带发现 → BETA-48 直接修、发现语义经 AskUserQuestion 拍板方案 A 后当场落地。
**关键决策**：音乐全盘发现改**按生效 roots 过滤入库**（发现器纯做加速、越界不入库；空 roots 连 es.exe 都不 spawn）——BETA-46 零索引语义对齐，BETA-01A「全盘入库」废弃；旧库越界记录不主动清（沿用「生效目录之外」提示 + purge 口径）。
**产出**：local-index 三处发现分支统一过滤 + `filter_discovered_to_roots` 纯函数；BETA-48 `embedding_model_path` 前端透传 + 语义 tab 路径覆盖 UI；文案「全盘发现」→「快速发现（仅限所选目录）」。
**结果**：local-index 26（+2，含改写的行为变更测试）/ desktop 全量 exit 0；clippy（清 2 条 doc 缩进）/fmt/tsc/vite 净。
**未尽事宜**：音乐发现不越界随 BETA-47 真机一并验证。

---
### 2026-07-06 IV — Claude Code (Fable 5) — BETA-47 选项页重构（七 tab + Everything 开关 + 拆文件）

**承接**：用户问「本次会话该做什么」→ 判定 BETA-47 为唯一标注「下会话」的主卡 → 用户拍板直接开工。
**产出**：① `enable_everything` 设置 + 三处 es.exe 门控（搜索后端条件注册〔重启生效〕/ 音乐全盘发现〔live、关闭回退目录扫描〕/ 模型本地发现〔live〕）+ `check_everything_available` 检测命令（与开关独立、非 Windows shim 恒 false）；② 七 tab（常规/索引/Everything/语义召回/Windows/隐私与记录/杂项，平台 tab 仅 Windows 显示；模型管理从常规迁入语义召回）；③ PreferencesDialog 1579→513 行、面板拆 `preferences/` 九文件。
**结果**：desktop 171（+1）/ local-index 24（+1，phase 级回归测试）全过；tsc/vite/clippy/fmt 净。
**未尽事宜**：BETA-47 真机验证随下次发版；顺带发现两条——**BETA-48**（前端 AppSettings 缺 `embedding_model_path`，UI 保存冲掉手工值，已登 ROADMAP B8）+ Everything 全盘发现 vs 零索引语义张力（进 Class B 待拍板）。

---
### 2026-07-06 III — Claude Code (Fable 5) — v0.9.16/17 双发版 + 真机反馈二轮修复

**承接**：用户拍板发 v0.9.16 → 装机实测回报下载卡死链等 → 逐条修复攒批 → 拍板发 v0.9.17。
**发版**：v0.9.16 macOS 首跑 E0433（target-gated 依赖坑）→ shim 修复 + dispatch 重跑 success；v0.9.17 双平台一次 success，并发机制三连稳。changelog 均补全。
**产出**：下载卡死链修复四刀（select 取消竞速〔连接阶段即刻生效〕+ connect_timeout 15s + hf-mirror 镜像兜底〔PRIVACY 同步〕+ model_download_in_flight 前端恢复下载态）；取消误报失败修复（invoke-catch 补过滤）；目录三行卡片布局（路径/统计/按钮分行）。BETA-45 真机首验：发现 UI 工作（Everything 命中 artifacts 模型）。
**结果**：desktop 170 全过、tsc/vite/clippy/fmt 净；[Release v0.9.17](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.17) exe+DMG 齐。
**未尽事宜**：v0.9.17 待用户验证（取消即刻生效/镜像/三行布局/卸载保模型弹窗/零索引空态/升级零损失）；`gh run watch` 假退出 ×3 → 一律 --json 轮询。详录 → [session-details-2026-07.md](./session-details-2026-07.md)。

---
### 2026-07-06 II — Claude Code (Fable 5) — v0.9.15 并发发版 + cycle 9 真机反馈落地（BETA-45/46）

**承接**：用户拍板 push + 并发发版 → 真机测 v0.9.15 → 回报三条反馈 → 三项拍板后当场实现 ①②、③登记下会话。
**发版**：v0.9.15 双平台并发**双 success**——macOS DMG CI 首验通过（aarch64 DMG 产出）、并发同 Release 幂等追加成立；changelog 补全。踩坑：`gh run watch` 假退出 ×2，改 `--json status` 轮询。
**产出**：**BETA-45** 模型本地发现 + 卸载默认保模型（NSIS `/SD IDNO` + 同卷 Rename 暂存；everything `find_files_named`〔wfn: 精确名 + UTF-8 导出〕；discover/import 命令 + 白名单 + 原子落盘 + 复用下载 done event；ModelDownloadStep 发现 UI）；**BETA-46** 默认零索引（`resolve_index_roots_tagged` 三夹仅勾选纳入、空+false=零索引）+ checkbox 常显 + banner 退役 + 路径完整显示；**BETA-47** 选项页重构登记（ROADMAP 新 B8 小节）。
**结果**：desktop 168 / everything 15 / settings 四分支 / uninstall 闸门（+2 断言）全绿；tsc/vite/clippy `-D warnings`/fmt 净。根因诊断：反馈① = BETA-12 整目录删含模型；反馈② = 空 roots 兜底三夹旧语义。
**未尽事宜**：BETA-45/46 随下次发版真机验证（NSIS 弹窗须真装真卸）；升级行为变化（空 roots 老装机停索三夹）随 cycle 9 复测确认；BETA-47 下会话。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-06 — Claude Code (Opus 4.8 / Fable 5) — BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割至 99.4%

**承接**：用户问「本次会话该做什么」→ 读三份共享文档 + 定向读 ROADMAP §2/§6.3/§8 → 判定质量线已达标、卡口全在真机/对外；用户选「先看 ROADMAP 全局再定」→ 按建议做出场报告骨架 + clarify 分析 → 拍板方案 A → 就地实现 → 续推老账收割。
**产出**：① [beta-exit.md](../reviews/beta-exit.md) 骨架（§9 模板，parser-only 全填、真机格标 TODO，B→V checklist 必交付项）；② clarify options **方案 A** 拍板并落地（[决策备忘](../reviews/beta-14-clarify-options-decision-2026-07-06.md)）——按 reason 定带不带 options、非 Unknown 一律挂、parser（`clarify_with`+`standard_options`）与标注（d6/d8 共 17 条）双向对齐，Class B 清零；③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符保全 / 裸 no+字面扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。
**结果**：**v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 230→235 测 + evals/harness/server 全 gate（28 suite 0 failed）+ clippy `-D warnings`/fmt 净。剩 6 partial 全为 v0.5 标注锁定项 + 备份文件两难，parser 收割见底。
**未尽事宜**：真机复跑填 beta-exit TODO 格；clarify en query 返中文 options 是既有 i18n 缺口（独立小卡）。详录 → [session-details-2026-07.md](./session-details-2026-07.md)。

### 2026-07-04 VIII — Claude Code (Fable 5) — §6 缺口盘点 + 三刀收割跨过 90% 出场线

**承接**：STATUS 下一步 ③（§6 90% 出场线评估，BETA-14 前决策）；用户指令"盘点 1.9pp 缺口"后认可三刀路线。
**关键发现**：119 partial 按根因重切出两个未盘过的大簇（media title 残段 17 / Refine delta 16）——「纯 parser 已见底」结论被推翻，无需 BETA-29 换口径即可过线。
**产出**：① media title 兜底只收"点名"（`is_descriptor_segment`，质量/流派词 lexicon 单一来源）+ artist 停词「时长」；② refine 设值 scope 限定 / artist 兜底 / sort 附加解耦 / 多字段 clear 语序 + `from last`→modified 撤销；③ file_action rename 混排介词（v0.5 3 条转正）/ 目的地路径与 target 分离 / external drive 映射；④ 标注对齐 17 条（d7×12 对齐 v0.5 主流与 wire 标量约定、d6×5 `~/Desktop` 机器无关化），均为 v0.9 coverage 分片、v0.5 未动。
**结果**：v0.9 = **927/73/0（92.7%）**、v0.5 = **478/22/0**；三轮逐 case 对比零回归。新增单测 15；intent-parser 215 / evals 全 gate / server 88 / desktop 165 全绿；clippy/fmt 净。
**未尽事宜**：3 项口径拍板（language / Clarify 文案 / 复数归一，入 Class B）；时间表达簇 ~17 条按反馈驱动排期；出场判定余双平台真机复跑。复盘：[beta-14-gap-inventory-2026-07-04.md](../reviews/beta-14-gap-inventory-2026-07-04.md)。

### 2026-07-04 VII — Claude Code — BETA-12 卸载流程 + BETA-29 意图草稿 v1

**承接**：7-04 VI 后 STATUS 下一步 ③（BETA-12 无阻塞代码卡）+ 用户点名接开 BETA-29。
**产出**：① BETA-12 双层清理——NSIS 卸载 hook（`apps/desktop/src-tauri/nsis/uninstall-hooks.nsh` 经 installerHooks 挂载；`$UpdateMode` 守卫升级不清数据、settings.json 保留，变量语义从 CLI 内嵌 NSIS 模板实证）+ 应用内 `uninstall_cleanup`（删索引/模型/日志/审计/搜索历史/user-synonyms.yaml；两模型句柄新增 `unload()` 释放 GGUF 句柄；索引/嵌入/下载并发守卫）+ 隐私页「卸载清理」二段确认 + 使用手册 FAQ；② BETA-29 v1——`SearchEvent::Started` 新增 `intent_json`（三条执行路径统一）、`search_impl` 拆出 `run_resolved_search`、新命令 `search_with_intent`（deny_unknown_fields 强校验、仅收 file_search/media_search、record 支撑后续 Refine）、意图条「调整 ▾」折叠草稿面板（chips + 类型/时间/排序下拉 + 重跑）。
**验证**：desktop 165 全绿（新增 11：uninstall ×5 + unload ×1 + BETA-29 ×5）；clippy(`-D warnings`)/fmt 净；前端 tsc + vite build 净。NSIS hook 首次真实构建随下次发版 CI（路径错打包会大声失败、另有仓内在位闸门测试）。
**未尽事宜**：BETA-12/29 真机验证归 cycle 9（manual-test-scenarios 两节）；BETA-29 v2 余量登记在 ROADMAP 卡片。

### 2026-07-04 VI — Claude Code — B7 收账四连：BETA-43 + BETA-44 + BETA-13 收束 + LLVM 正式化

**承接**：7-04 V 护城河规划落库后的执行轮（moat-plan P1-P2 代码项 + STATUS 下一步 ③④⑤⑥）。
**关键决策**（用户拍板）：BETA-13 re-baseline 三项——39a-039 改标 FileAction（与逐字同句的 39b-040 及 G5 拍板对齐）、45b-047 合并终态改标 Refine（单轮无状态评测结构性不可测）、§1.1 冲突 2 条 coverage 改标 MediaSearch（schema 本有 sort/size 等字段可无损表达，零 parser/v0.5 风险）。
**产出**：① BETA-43 全套（`snippet`/`pages` 出处、`read_document` + `allow_full_read`、`/admin/audit/report`、audit 扩 read/path/read_mode、e2e ×3、`\\?\` 路径归一修复）；② BETA-44（queries.tsv 22→53、新材料 4 份、真实模型 53/53 `--require-all` 首跑全过，报告 [beta-44-enterprise-eval-expansion](../reviews/beta-44-enterprise-eval-expansion-2026-07-04.md)）；③ BETA-13 收束（v0.9 881/119/0、v0.5 475/25/0、§6.5 豁免 2 条记录在案；media 路径 `sorted by` keywords 泄漏 + `bigger than` size 丢失两缺口修复带单测）；④ `scripts/build-locifindd-llama.bat` 入仓 + daemon README §2.5 / scripts README；⑤ ROADMAP 清账：BETA-11/11A/11B/11C dropped（= BETA-15/15A/15B/15C 重复登记）、BETA-13-G12/G14 done 收口。
**验证**：server 88 / daemon 8+e2e 13 / intent-parser 200 / evals 全 suite / desktop 154 全绿；全 workspace clippy(`-D warnings`)/fmt 净（platform-macos 2 预存除外）。
**未尽事宜**：设计伙伴获取（P0 外部）；BETA-43 扫描件回页真机验证归 cycle 9；§6 90% 剩 1.9pp。

### 2026-07-04 V — Claude Code — 护城河规划评审与落库

**承接**：用户携 Codex 五层护城河方案（场景/格式/权限审计/评测/MCP 生态）请求评审并综合成完整规划。
**关键决策**（用户拍板"推荐集"）：护城河论述单一信源落 [moat-plan-2026-07-04.md](../reviews/moat-plan-2026-07-04.md)；评审三处结构性修正——功能清单≠护城河（本项目周级卡天级完成即证据，壁垒在评测资产/信任证据/客户侧沉淀/场景纵深）、MCP 是分发渠道非护城河、补漏签名分发信任 + 客户侧状态沉淀两层。
**产出**：moat-plan doc；ROADMAP B7 新卡 **BETA-43**（V10-16 先导：出处强制/片段级返回/审计导出）+ **BETA-44**（eval 扩容 22→50）各带验收、V10-16 标注拆卡、§5 新增「获取设计伙伴/首个真实部署」（P0 主动项）、§11 修订摘要；STATUS 下一步重排（设计伙伴升 P0）；CONVENTIONS §3 加「踩坑→fixture→闸门」收工检查。B7 红线不变。
**未尽事宜**：PROJECT.md 定位句（"权限网关"半句）待后议；BETA-43/44 已于 7-04 VI 消化。

### 2026-07-04 IV — Claude Code — 桌面 UI 消费 extraction_failures()

**承接**：7-04 修复报告遗留 4（STATUS 下一步 ②）。
**产出**：后端 `get_extraction_failures` command（`index_failures` 表 → 路径 + 原因 + rfc3339 时间倒序，db 不存在返空）；前端「选项 → 索引」新「未能索引的文件」区块（默认折叠显条数 / 展开滚动列表 / 零失败不渲染 / 打开与 reindex 完成自动刷新）。
**验证**：desktop 154 全绿、clippy/fmt/tsc/vite build 净；命令层薄封装，留痕全周期由 indexer 既有测试守护。
**未尽事宜**：7-04 遗留项全部消化；BETA-40 仅剩真实内网证据（外部依赖）。

### 2026-07-04 III — Claude Code — daemon 默认开图片语义 + 语义臂 MediaSearch 空洞修复

**承接**：用户拍板 BETA-39 方案 (a)（报告 §5 建议）。
**产出**：① `ServerConfig.embed_images`（daemon 默认 true / 桌面 opt-in 不变）+ `locifindd --disable-image-semantics` + 首次索引与 reindex 透传 + 启动期 purge 镜像桌面语义（关 → 清全部图片向量）；② 评测集扩 22 case（新 O-09：PNG 进 offboarding collection，OCR 文本全为 2 字 CJK 词）；③ **O-09 首跑 miss 暴露语义臂真空洞**——`截图/照片` 问法被 parser 路由 MediaSearch、`SemanticIndexBackend` 只接 FileSearch → 扩展 `query_spec` 接受图片类 MediaSearch + `IMAGE_EXTS` 候选过滤（音频/视频不接、桌面 opt-in 关时零变化）；④ playbooks/README 部署提示同步。
**验证**：端到端 **22/22 全过**（`--require-all`、O-09 顶位、其余 21 case 排名与前 baseline 逐条一致）；semantic-index 23（+2 新单测 stub OCR/embedder）/ desktop 154 / server 66 / daemon 8+e2e 9 / evals 全绿；clippy/fmt 净。
**未尽事宜**：真实内网证据（待用户）；2 字 CJK 泛词收尾（LIKE 兜底不再紧迫、需要时另立卡）。

### 2026-07-04 II — Claude Code — enterprise eval 自动化 + csv/tsv 覆盖缺口修复

**承接**：STATUS 下一步 ①（7-04 修复后的可重复回归缺口）。
**产出**：① `packages/evals` 新 enterprise 模块 + `enterprise_scenarios` binary（queries.tsv → 运行时生成合规 collection config → 真 locifindd → 逐 subject token MCP search → top-K 命中 / 越权双断言 → 报告 + `--require-all` 闸门）+ `enterprise_scenarios_gate`（3 条 fixture 完整性常跑 CI + env 门控端到端）；② daemon 新 `--semantic-weight` 旋钮（`ServerConfig` 贯通 RRF 融合、默认不变）；③ 首轮 20/21 暴露 csv/tsv 不在 `DOC_EXTS` → 补纯文本提取（桌面同享）；④ 示例 config token 补齐 ≥32 字符（原样照抄会被 daemon 拒启）。
**关键结论**：修复后真实模型两轮 **21/21 全过**（15 条命中第 1 位、3 条越权负样本零泄漏 + 全拒）；权重 10 vs 3 逐 case 排名一致 → daemon 语义融合权重问题结案、维持默认。2 字 CJK 泛词评估：FTS 双路径结构性 0 命中、唯一不可达组合 = 图片 OCR + 纯 2 字词，修法待拍板。
**验证**：indexer 177 / desktop 154 / server 66 / daemon 8+e2e 9 / local-index / semantic-index / evals 全绿，clippy/fmt 净。报告：[beta-40-enterprise-eval-2026-07-04.md](docs/reviews/beta-40-enterprise-eval-2026-07-04.md)。
**未尽事宜**：真实内网证据（待用户）、2 字 CJK 修法拍板、桌面 UI 消费 `extraction_failures()`。


### 2026-07-04 — Claude Code — PDF/JPG/PNG/OCR 落库专项 + daemon 语义臂补齐

**承接**：STATUS 下一步 ①（7-03 报告 §5.1 遗留）。
**关键发现**：daemon 语义检索此前从未生效——`document_vectors` 恒空（embed pass 只有桌面端跑）、检索候选链只有 FTS 臂；**7-03 报告的三场景"semantic 命中"实为 FTS 字面命中**。
**产出**：4 根因修复——① pdf-extract 遇中文 CMap panic → `extract_pdf` 内层 catch 降级 OCR 管线；② daemon 首次索引 + reindex 补图片 OCR 轮；③ daemon 补 embed pass、候选链装 `SemanticIndexBackend`、`SearchTool` 走桌面同款 RRF 融合；④ WinRT OCR 正斜杠路径归一。另新增 `index_failures` 文件级失败留痕表（成功重扫/磁盘删除自动清除）+ daemon 启动期依赖探测日志。新增 2 个 CI 安全回归测试。报告：[beta-40-ingest-semantic-gap-fix-2026-07-04.md](../reviews/beta-40-ingest-semantic-gap-fix-2026-07-04.md)。
**验证**：indexer / server(66) / daemon e2e(9/9) / local-index / semantic-index / harness / desktop(154) 全绿、clippy 净；真实模型 + 企业材料端到端：UniGB PDF 落库入语义、7 集合向量全写、图片可搜（`现场交付照片` 顶位命中 JPG）、`项目交接` 从 degraded 转命中。
**未尽事宜**：2 字 CJK 词 trigram 限制（BETA-42 关联）、daemon 语义权重待评测数据、桌面 UI 消费 `extraction_failures()`。

### 2026-07-03 X — Codex — 企业三场景材料与 semantic daemon 测试闭环

**承接**：用户要求先了解律所、内部审计、离职员工三种目标场景，再规划测试、生成真实格式材料，并在本机有模型后直接开展 semantic daemon 测试。  
**产出**：新增 [enterprise-scenario-test-plan.md](../reviews/enterprise-scenario-test-plan.md)、[test-materials/enterprise-scenarios-raw](../../test-materials/enterprise-scenarios-raw/)、[generate_enterprise_real_format_materials.py](../../scripts/generate_enterprise_real_format_materials.py)、[enterprise-smoke-test-2026-07-03.md](../reviews/enterprise-smoke-test-2026-07-03.md)、[enterprise-semantic-daemon-test-report-2026-07-03.md](../reviews/enterprise-semantic-daemon-test-report-2026-07-03.md)。材料覆盖 DOCX/PPTX/XLSX/PDF/JPG/PNG/扫描 PDF/EML/MD/TXT。  
**关键修复**：补齐 LLVM/Clang 20.1.8 开发环境；`packages/model-runtime/Cargo.toml` 的 `llama-cpp` feature 透传 `llama-cpp-4/mtmd`，解决 Windows `common_*` 链接缺符号；`packages/evals/src/mcp_client.rs` 适配 daemon 当前 stateless MCP JSON framing。  
**验证**：`cargo build -p locifindd --features locifind-model-runtime/llama-cpp` 通过；`cargo test -p locifind-evals --features semantic-recall --test daemon_mode_smoke -- --nocapture` 3/3 通过；`cargo test -p locifindd --test e2e -- --nocapture` 9/9 通过；手工 MCP 查询命中律所 `违约金 条款`、审计 `收款账户 不一致`、离职交接 `Lighthouse` / `双层鉴权`、HR `保密协议`。  
**未尽事宜**：PDF/JPG/PNG/OCR 落库链路仍需专项；泛词 `handover` / `项目交接` 可 degraded；`.tmp` 下 LLVM 仅作本机临时依赖。

### 2026-07-03 IX — Claude Code — CLEAN-6 瘦身 + BETA-33 cycle 9 六刀全清

**产出**：ROADMAP 242KB→118KB，62 张 done 巨卡归档；BETA-33 cycle 9 完成 embed degrade 根修、单实例锁、WSearch 探测、状态文案单一信源、口径统一、useAppSettings hook 与旧路由删除。
**验证**：indexer / semantic-index / desktop / harness / local-index / server 全测零回归，clippy/fmt/tsc/vite build 净。

### 2026-07-03 VIII — Claude Code — BETA-38 cycle 4 十万级规模化基准

**产出**：`bench_semantic` 与 scaling evals，十万向量缓存基准报告；缓存 p95 明显优于暴力重载，BETA-38 整卡 done。详见 [beta-38-scaling-benchmark.md](../reviews/beta-38-scaling-benchmark.md)。

### 2026-07-03 VII — Claude Code — BETA-38 cycle 1-3

**产出**：`documents.content_hash`、文件原始字节身份 hash、索引期同身份向量复用、semantic backend 进程级 `VectorCache` 与结果去重。详见 [ROADMAP-archive-2026-07.md](./ROADMAP-archive-2026-07.md)。

### 2026-07-03 VI — Claude Code — BETA-42 trigram FTS 短词修复

**产出**：过滤纯 CJK 短词进入 FTS AND，避免 2 字中文短词导致 0 命中；local-index/server/desktop 回归通过。

### 2026-07-03 V — Claude Code (Sonnet 5) — v0.9.11 装机验证全过 + 发现 BETA-42（trigram FTS 短词 bug）

**承接**：用户手动测试 GUI 时机器意外死机（Kernel-Power 41，历史上非首次，与本会话此前的多屏截屏/overlay 授权操作时间相关但不能排除机器自身稳定性问题，此后避免走该自动化路径）。改用隔离 git worktree（checkout v0.9.11 tag，不碰同仓库另一会话 BETA-39 未提交改动）跑 `real_pdf --ignored`（9 份扫描 PDF OCR 全过、0 失败页）+ `real_eml`（6 封全过），等价验证 (c)(e)(g) 引擎层；剩余 UI 呈现（预览徽章/eml 头块/附件分段/pdftoppm 绿标）改为文字指引，用户手动操作全部确认通过。
**意外发现 BETA-42**：验证中用户报"判决 违约金"搜索报红色「错误：未找到结果」；代码走查确认根因——`documents_fts` trigram 分词器要求词项 ≥3 字符（indexer 单测注释早已知晓此限制），但 `extract_zh_residual_keywords` 切中文关键词只要求 ≥2 字，2 字词与其他词 AND 组合时结构性 0 命中，前端把"全链路零结果"渲染成「错误：」样式（`SearchEvent::Error` 既有设计非崩溃，但用词易误导）。已登记 ROADMAP BETA-42（commit `f7d9a9d`），三候选修法待拍板，代码未动。
**验证结论**：v0.9.11 本体功能稳定可用，16 项验证全通过；BETA-42 是长期存在的 FTS 特性缺口、非本版引入，不影响本次结论。
**未尽事宜**：BETA-42 修法拍板（下一步 top-1）；BETA-40 真机走通留证据仍待用户内网环境。

### 2026-07-03 IV — Claude Code (Fable 5 / Opus 4.8) — BETA-39 图片语义索引 opt-in（B7 能力卡 done ⭐）

**承接**：会话中途电脑死机重启、切 Opus 4.8 续做；开场四问 AskUserQuestion 全采推荐（图片专属门槛 0.75 / 关闭时启动期 purge 依设置动态判 / 段落级 explain 同步放开 + 图片专属段级门槛 / UI 放「选项→索引」pane）。
**产出**：解除 BETA-33 cycle 4 图片一刀切。`enable_image_semantics` 默认关；`is_image_embed_worthy`（0.75 门槛）+ 三处调用点参数化（`embed_pending(embed_images)` / `purge_short_body_vectors(keep_worthy_images)` 启动期依设置动态判 / `explain_passages_with_ratio(0.75)`）；`SearchDeps` image_semantics_provider live-read；「选项→索引」checkbox +「需重新索引生效」副文案。indexer 171 + semantic-index 18 + desktop 147 单测 + tsc + clippy/fmt 全绿。
**关键决策**：图片专属门槛 0.75 卡在已知污染 case（QQ 表情包 ratio≈0.63）上界之上、真文字截图（>0.8）零误伤；purge 的 keep_worthy_images 启动期动态判而非「关闭后保留已嵌」，守 byte-equal 验收。
**未尽事宜**：待用户 commit 收工；开启后不自动补嵌（受 `LOCIFIND_ENABLE_EMBED=1` 门 + 需重扫触发，随 BETA-31-v2 真修）。

### 2026-07-03 II — Claude Code (Fable 5) — v0.9.11 发布 + BETA-36 daemon 权限/collection 模型全落地（B7 第四张卡 done ⭐）

**承接**：用户拍板「先发 v0.9.11，然后开 BETA-36」。
**v0.9.11**：bump + tag → Release workflow ✅ → notes 已补（pdftoppm 兜底修复 + eml 索引；装机清单 (c)(e) 承接 + (f)(g) 新项）。
**BETA-36 产出**：spec 4 问全采推荐 + 5 cycle（TOML 配置模型 / 多 token+principal 穿透 / per-collection 独立 db 物理信息墙 / search 越权 Denied + list_collections / audit.jsonl + /admin/audit + admin 门 403）；e2e 8 条含信息墙/403+audit/只读 409。全量描述见 [ROADMAP BETA-36 卡片](../../ROADMAP.md)。
**关键决策**：物理信息墙（per-collection 独立 db，越权无查询面）；未知/未授权 collection 同文案防探测；audit.jsonl 与 ops tracing 两套规则各守其职（前者默认记 query 明文供取证、后者永不记）。
**意外收获**：e2e MCP helper 按 rmcp stateless 纯 JSON framing 重写，解除 BETA-32 遗留 2 条 `#[ignore]`。
**踩坑**：全 workspace 测试重跑 LNK1104 = 上轮残留 locifind_desktop 测试进程锁 exe，杀进程即愈；PS 5.1 `Set-Content -Encoding utf8` 带 BOM 会让 Cargo 拒解析 TOML/JSON（版本 bump 改用 Edit 工具）。
**未尽事宜**：daemon reindex 仍是 stub；BETA-40 playbook 依赖已全齐；V10-16 出处闸门衔接 BETA-36 collection ACL。

---

## 滚动归档：会话日志摘要（从 STATUS.md 滚出）

### 2026-07-02 VII — Claude Code (Fable 5) — BETA-41 企业评测语料 fixture 全落地 ⭐ + v0.9.10 release notes 补齐

**承接**：v0.9.10 workflow 跑中、用户拍板 BETA-41 先行；spec 期 4 问（结构/入仓方式/规模/邮件格式）AskUserQuestion 全采推荐。
**产出**：spec + 5 cycle：语料 104 doc/50 case 五桶 + harness `--fixture-set enterprise` + 双门 + 文件层生成脚本与 9 扫描 PDF/6 eml/近重复副本 + real_pdf 三层测试；workflow 28578228626 ✅ 后 `gh release edit v0.9.10 --notes-file` 已补 changelog。
**关键决策**：① 不在本机装 poppler——装机验证清单第一项要验"未装→引导"初始态，装了会破坏它，OCR 端到端留 `--ignored` 给装机验证顺跑；② 向量 bootstrap 移 Mac（本机缺 cmake/VS Build Tools，与 BETA-31-v2 同阻塞），模型对齐 semantic-recall v5 的 bge-m3。
**踩坑**：.ps1 含中文必须带 UTF-8 BOM（PS 5.1 无 BOM 按 GBK 读、here-string 报误导性 terminator 错），已写记忆。
**未尽事宜**：向量 bootstrap + baseline；BETA-41 fixture OCR 端到端随 v0.9.10 装机验证跑。

### 2026-07-02 VI — Claude Code (Opus 4.7) — v0.9.10 bump + tag（BETA-35 分发）

**产出**：push commit `3a1727c`（BETA-35 全落地）→ origin/main；bump v0.9.10（tauri.conf.json + Cargo.toml）+ tag → 触发 Release Windows workflow。首次 workflow **failed**（Cargo.lock 未同步、CI `--locked` 校验挂）→ `cargo check -p locifind-desktop` 生成 Cargo.lock 新版 → `sync Cargo.lock for v0.9.10 bump` commit `3204f3b` → 删旧远程 tag + 移动 v0.9.10 到 3204f3b → 新 workflow run 28578228626 触发中。release notes 已起草到 scratchpad（新增功能 / pdftoppm 前置 / 装机清单 / 内部改动），workflow 完成后 `gh release edit v0.9.10 --notes-file` 补上。
**未尽事宜**：v0.9.10 装机验证（用户执行）；release notes edit（workflow 完成后）。

---

## 2026-07-02 II 整体归档：重构前 STATUS.md 全文

# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取（§2 阶段总览 + 当前阶段小节，不必全文）。
> **每次"收工"**：按 [CONVENTIONS §3 收工流程](./CONVENTIONS.md) 更新本文件和 ROADMAP。
>
> 本文件是**当前进度的单一信源**。全程任务地图在 [ROADMAP.md](./ROADMAP.md)。
> 不要在本文件复述 ROADMAP 中的 task 详情；用 task ID 引用即可。
>
> **历史会话日志（2026-06-03 及之前）已归档** → [docs/session-logs/](./docs/session-logs/)。本文件只保留最近约 5-10 条；
> 收工时若会话日志超出此量，把最旧的滚动追加到 [docs/session-logs/](./docs/session-logs/) 的归档文件，**保持本文件精简**。

---

## 当前阶段

**M（MVP）代码层全部 done；B（Beta）阶段进行中。** M1 12/12 ✅、M2 3/3 ✅、M3 4/4 ✅、M4 7/7 ✅、M5 4/4 ✅。

B 阶段已落地：BETA-09(a) Windows 跨平台一致性、BETA-17 基座选型 bake-off、BETA-01/01A（音乐+全盘音频索引）、BETA-02（Office/PDF 内容索引）、BETA-03（图片 OCR）、BETA-04（多源融合）、BETA-05（Ranker）、BETA-06（Audit）、BETA-07（后台索引调度）、BETA-18/19（跨范畴多类型 + 均衡展示）+ 强媒体词跨范畴路由、**BETA-20/21/22（B6 演示能力三项：结果预览面板 + 隐私信任面板 + 搜索历史/保存的搜索）全部 done**——**整栈真机验证通过**。**B1 本地索引除 macOS Vision OCR 已全部落地**（macOS Vision 留 Mac 会话）。**B6 演示能力三项全清**。**BETA-13 evals 扩到 1000 条 done**（v0.9 覆盖驱动评测集；baseline 量化出 parser 自然语言缺口、§6>90% 未达成，登记 BETA-13-G1~G7 parser backlog）。**BETA-23 模型 fallback 接入桌面默认流程 done**（2026-06-13，真机手测通过；keywords 补全待 BETA-24 重训）。**BETA-15B-8 model-runtime pooling type detection done**（infra 修复、bge-m3 CLS pooling 真水位 OVERALL 0.869 双过 spec 字面 0.864、infra 阻塞解除）。**BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查 done**（4-hypothesis ladder 全 FAIL、qwen3-8b 真水位仍未知、推断 fused Gated Delta Net × Last pooling × embedding-only 交互 8b-specific bug、issue body draft 入仓待用户 file；qwen3-0.6b 升 0.3.2 语义等价、vectors.json + vectors-qwen3-0.6b.json SHA256 一并变到 0315b8d0...）。**BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验 done ⭐⭐**（Branch I-a GO 双过 spec 字面：OVERALL **0.900** +0.036 / crosslang **0.725** +0.039 vs v5 bge-m3 baseline、no-prefix mode 单独也双过字面 = prefix 是 ROI 加分项不是必要条件）; **BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done ⭐⭐**（最窄 wiring 切换、单文件 diff < 20 行、OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026 vs v5 bge-m3 baseline T=0.70 全方面提升无 trade-off、分发净降 292 MB）。**BETA-31 Windows 模型分发 UX 增强 done ⭐**（双平台 onboarding 扩 3-step + GUI 一键下载 + 5 example queries、为邀请同事 Windows 真机手测扫清体验障碍、reqwest 0.12 stream 下载 + 进度 event + cancel + 重入 IN_FLIGHT guard + partial 清理；C2 frontend hook 修 cancel-race + listener leak 共 2 Important fix）。**BETA-32 团队归档 MCP daemon done ⭐**（与主线 BETA→1.0 并行的衍生子线、不阻塞 1.0 出场标准；新 `packages/locifind-server` lib + `apps/daemon` binary `locifindd` 走 rmcp 1.8 streamable-HTTP / bearer auth、复用 hybrid 检索栈 zero-touch、三平台 binary CI workflow `release-daemon.yml`、apps/daemon/README.md launchd / systemd / NSSM 部署样板齐备；红线 1-7 全过）。各 task 状态见 [ROADMAP §3.3](./ROADMAP.md)。

## 当前 Task

**最新（2026-07-02 Claude Code (Fable 5)）**：BETA-33 cycle 7 收官 —— **v0.9.7 真机验证 (a)-(j) 9 GO / 1 FAIL + cycle 7-c 三件套 done + (d) confirm 失效修复 done、bump v0.9.8 待 tag ⭐⭐⭐⭐⭐**。上午驱动装机版 GUI 全程截图跑完 7-a/7-b 验证清单：(a)(b)(c)(e)(f)(g)(h)(i)(j) 全 GO（(g)(h) 用 sqlite 直查双维度验证：新文件被剪 + 存量条目被 stale-prune 追溯清除、比验收标准更强；(j) node_modules 全库 0 条；(i) root_excludes 孤儿清理精确）；**唯一 FAIL = (d) 关闭前二次确认**——根因 wry/WebView2 生产环境 `window.confirm` 不弹窗直接放行（checkbox-only 对照实验排除状态问题、已写记忆 tauri-webview2-window-confirm-noop）。下午接续 cycle 7-c 三件套 + (d) 修复：单目录重扫（`perform_reindex_for_roots` roots_override + `reindex_root` command + RootRow「重扫」按钮）、打开目录（**复用既有 `open_path`**、比 design doc 的 plugin-shell 方案少一个依赖且策略/audit 口径一致）、移除目录二次确认（通用 in-DOM `ConfirmModal` 单选「仅移除配置」vs「移除并清除索引记录」、文案明确不删磁盘文件、同一组件顺带替换掉关闭守卫的 window.confirm）+ `purge_under_root` SQL 落存储层（与 `stats_under_root` 共用新抽 `root_glob_predicate` 边界 helper、FTS 同步删、向量外键级联 + 级联单测）。顺带发现 2 个非本 cycle 问题登记 follow-up：**无单实例锁**（两实例并发写同一 index.db / settings.json）+ 「本地索引」行全局口径 vs 概貌 root-scoped 口径并存易困惑。详见 [ROADMAP BETA-33 卡片](./ROADMAP.md) + 本日会话日志。

**先前（2026-07-01 Claude Code）**：BETA-33 cycle 7 三刀合一 v0.9.7 —— **cycle 7-a + 7-b done、v0.9.7 已发版 ⭐⭐⭐⭐⭐**。cycle 7-a 索引目录管理 UX 打磨（v0.9.6 首次真机验证锁定"C 主 + B 副 + 数据源不一致 + 进度可视化断层"后修法：大字警告条 + checkbox 增强 + flash 高亮 + pending badge + sticky 提示 + 关闭前二次确认 + indeterminate 动画进度条 + phase chip + `IndexStatus.current_phase` 字段 + `IndexProgress::on_phase` trait method）；cycle 7-b 子路径排除通配符（`AppSettings.root_excludes: Vec<RootExclude>` + `normalize_root_key` 跨平台归一 helper + `ExcludeFilter` 双层结构 + 兼容层保 BETA-27 basename-only byte-for-byte + glob 边界补目录本身 + Windows 分隔符归一 + 每 root 折叠区 UI）；**Codex APPROVED with suggestions**（1m26s、+44 -0 追加 §10、3 OBJECT 全采纳 + 10 SUGGEST 全合入 doc）。详见 [design doc](docs/reviews/beta-33-cycle-7-index-mgmt-ux.md) + 本日会话日志。

**先前（2026-06-30 Claude Code）**：BETA-31-v3「v0.8.0 真机 UX gap 修复集」—— **第 8 刀 cycle 6 v3 done + v0.8.8 真机验证 (a)(b)(c)(d) 全 GO ⭐⭐⭐**（同会话双段产出。① **v0.8.8 真机验证 4/4 GO**：computer-use 自动驱动 GUI 全程截图证据—— (a) 空 `index_roots` 显示 Music/Documents/Pictures 真系统默认 3 条、(b) 加 Downloads 系统默认列表消失走自定义+「移除」分支、(c) 移除最后一条系统默认列表重现且标签正确、(d) 搜「读后感」13 条 3127ms 4 后端融合 top 3 全是真相关内容（罗翔讲读后感 mp3 / 语文怎么学 docx / 柳林风声读后感 pptx）、证明 cycle 6 v2 effectiveRoots + cycle 5 EMBED_TRUNCATE_CHARS=600 + cycle 3 ranker 污染清理叠加效应；② **cycle 6 v3 fix done**：顺带发现设置页文案「语义召回模型未找到」+ 下载按钮误显示、但顶栏「语义召回」绿点亮 + 搜索确实拿到语义命中——根因 = `EmbeddingModelHandle::status()` 的 NotLoaded 分支一刀切返 NotFound、不查文件、与顶栏 `is_active()` = feature 开 + path.exists() 判定不一致；修法 = 单文件 +14/-3、NotLoaded 分支加 path.exists() 检查、存在 → Ready / 不存在 → NotFound、前端 EmbedStatus 类型 / 文案 / 下载按钮条件全部零改动；bump v0.8.9）；**第 7 刀 cycle 6 v2 done + cycle 5 真机验证 GO ⭐⭐⭐**（前序：① **cycle 5 真机验证 GO**：v0.8.7 + `LOCIFIND_ENABLE_EMBED=1` 重启后 worker 跑完 42 篇真文档零 crash、`worker_elapsed_ms=927571 summary="语义索引就绪 411 篇"`、含 docx/txt/pdf/xlsx/png/md/jpg 全类型 + 中英混合、ucrtbase 0xc0000409 不再复现、证明 cycle 5 `EMBED_TRUNCATE_CHARS=600` hotfix 完全有效；② **cycle 6 v2 fix done**：v0.8.7 真机驱动截图验证发现 cycle 6 三个 UX bug（effectiveRoots 数据源永远从 settings.json 文件读不跟前端 useState 同步 / 「系统默认」标签错误打到用户配置目录 / tab 切换不重 fetch）；修法 = settings.rs `get_effective_index_roots` 加 `Option<Vec<String>>` 参数、Some 直用 None 退到读文件保留兼容 + SettingsPage.tsx useEffect 监听 `settings.index_roots` 变化重 fetch + 传当前 useState 作参数；2 文件 +39/-19；bump v0.8.8）；**第 7 刀 cycle 6 设置页显示生效索引目录 done**（前序：用户报「索引配置里看不到当前已添加的目录信息」、当前 0 个时只显示泛指文案。修法 = 新 `get_effective_index_roots` tauri command + frontend useEffect fetch；UI 在 `index_roots=[]` 时显示 📂 + 系统默认 3 条具体路径 read-only。bump v0.8.7）；**第 6 刀 cycle 5 EMBED_TRUNCATE_CHARS 1200 → 600 hotfix done ⭐**（v0.8.5 用户跑 `LOCIFIND_ENABLE_EMBED=1` 复现 crash + per-doc 日志锁定触发文档 = `2下53单元归类复习（语文）.pdf` body_len_chars=**1200** 第 1 篇即崩。**根因**：BETA-26 锁定的 `EMBED_TRUNCATE_CHARS=1200` 与 BETA-15B-11-v2 切到 embeddinggemma-300m 后 context **仅 2048 token** 不兼容、中文 SentencePiece char-to-token ratio 1.5-2、1200 字符 → 2000-2400 token → **溢出 context** → llama-cpp ggml `abort/__fastfail` → ucrtbase 0xc0000409；BETA-31-v3 cycle 4 之前因路径 bug 模型从未真加载 / cycle 3 之前 worker 因空向量 vector_is_current 跳过、所以这 bug 一直没暴露、cycle 4 first 真跑到 embed() 立即崩。**修法**：改 1 个常量 1200 → 600（中文 ≈ 1000-1200 token、留 800+ token / 60% buffer）+ 详细注释。bump v0.8.6 待用户装机验证 worker 跑完 139 篇真文档不再 crash）；**第 5 刀 cycle 4 native crash 止血 done ⭐**（cycle 3 修 ranker 污染后 v0.8.4 装机用户首次看到 spawn_semantic_index worker 真正进 embed_pending 调 embedder.embed() 真文档、立即触发 llama-cpp/ggml native crash、整个进程被杀；Win 事件日志 5 次崩溃指纹一致 `Faulting module: ucrtbase.dll` + `Exception code: 0xc0000409` + `Fault offset: 0x00000000000a527e`、v0.8.3/v0.8.4 都崩 = 同一深层 bug、cycle 3 之前 worker 从未真跑到 embed() 所以没暴露；修法三件套：① spawn_semantic_index **默认禁** + env `LOCIFIND_ENABLE_EMBED=1` 才跑（短期止血、现有 369 真文档向量 + FTS + Everything + Windows Search 全部可用）、② indexer 加 tracing 依赖 + embed_pending per-doc info log（用户主动开 LOCIFIND_ENABLE_EMBED=1 重试时、locifind.log 最后一行锁定触发 crash 的文档 path + body_len、为下个 cycle 真修提供输入）、③ main.rs std::panic::set_hook 写日志 + 启动 dump 加 embed_pending_enabled 标志；bump v0.8.5、本机 cargo 不可用、CI 兜底）；**第 4 刀 cycle 3 ranker 污染修复 done**（bump v0.8.4）；**第 3 刀 cycle 2 日志栈 done**（bump v0.8.3）；**第 2 刀 cycle 1 路径 fix done**（bump v0.8.2）；第 1 刀 done（v0.8.1）；**第 2 刀 cycle 2 UX 扩展 pending**（Step 2 模型四入口、~1d）；BETA-31-v2 Windows GPU 推理 cycle 暂搁。

**BETA-33 桌面菜单栏重构 cycle 1+2+3+3v2+3v3+3v4+4+5+6v4+7a+7b+7c done（cycle 7c = 2026-07-02 Claude Code、bump v0.9.8 待 tag；cycle 7a+7b = 2026-07-01、v0.9.7 已发 + 真机验证 9 GO / 1 FAIL(d) 已随 7c 修、Codex APPROVED with suggestions⭐⭐⭐⭐⭐⭐⭐⭐⭐**：参考 Everything 7 个下拉菜单 + 关于对话框 modal + 事件总线 + 全局快捷键 + Alt+首字母访问键、纯前端 React 实现；**v0.9.1**：菜单栏紧凑化 + PreferencesDialog 模态选项卡片（左侧 4 分类树 + 右侧表单 + 底部 取消/应用/确定）；**v0.9.2 hotfix**：Esc 键 4 版试错定位到 Windows Sogou IME → React `onKeyDownCapture` 挂 backdrop 解决 + dialog 缩到 720×520 + 遮罩 ≥40px 可点；**v0.9.3**：暴露语义原始 cosine 走 `MergedResult.semantic_cosine` 传给 `SearchResultJson`、前端新增「相似度」列（真 cosine 0.30-0.90、可点头排序）；**v0.9.4**：Qwen3-0.6B 一键下载（model_download.rs 重构 ModelKind 枚举 + 独立命名空间事件 + useModelDownload/ModelDownloadStep 参数化 + PreferencesDialog 常规 pane 内嵌 not_found 下载按钮） + 预览面板 label 明示「段落级」vs「文档级」两种 cosine 粒度；**cycle 3 v4 收工前追加 2 修（待发未 bump、下一版本 v0.9.5 承）**：① Ctrl+, → Ctrl+; 换绑绕 Sogou IME（`,` 老键保留作 fallback，双键 both emit `open-prefs`）；② StatusIndicator 顶栏灯口径修正（额外查 `embedding_model_status`、semantic 灯颜色跟真 EmbedStatus 走：ready 绿 / loading 蓝 / not_found 琥珀 / failed 红 / unavailable 灰、10s 轮询、避免灯绿但搜索报 embedding 不可用的误导）；旧 `/settings` 路由 + SettingsPage 保留作 fallback。**cycle 4（v0.9.5 待提交）**：OCR 乱码 + 图片语义污染防治双层门槛落地——A 层 `is_embed_worthy` v2 加 CJK+拉丁字母有意义占比 60% ratio 挡明显噪声；B 层 `embed_pending` + `explain_semantic_hit_impl` + `purge_short_body_vectors` 三处一致地"图片 doc_type 一律跳过语义索引/清旧图片向量"彻底切掉整类 OCR 污染源；段落级 `EXPLAIN_MIN_SCORE 0.30 → 0.45` + `passage_worth_embedding` 段级门槛（字数 ≥8 + meaningful ratio ≥60%）双重防线。本机 cargo 全测过（indexer 115/115、semantic-index 16/16、clippy `-D warnings` 干净、桌面 crate check 干净）；顺手修 Rust 1.96 新 lint `map_unwrap_or` 2 处（discovery.rs / placeholder.rs、非本 cycle 引入）。**cycle 5（v0.9.5 同版）**：索引目录概貌 + 分类统计 UI 落地——后端新 `get_index_overview` tauri command（`DocRootStats` + `MusicRootStats` 各带 `stats_under_root(root)` fn、SQL GLOB 3-OR 前缀边界覆盖 Windows `\` 和 Unix `/`、图片按 `doc_type IN IMAGE_EXTS` 拆分、best-effort 打开失败返 0 计数）；前端「选项 → 索引」pane 顶部加 `.prefs-overview-card`（6 单元格：目录数 / 总条数 / 📄 文档 / 🖼 图片 / 🎵 音乐 / 上次索引）+ `RootRow` 组件让每目录行显示分类分总 + 人性化时间（"刚刚 / N 分钟前 / 今天 HH:MM / 昨天 / YYYY-MM-DD"）；跟 `settings.index_roots` 依赖 useEffect fetch + 监听 `indexStatus.indexing` 从 true 转 false 时自动重刷新概貌。cargo test indexer 119/119、clippy 全干净、tsc 干净。**cycle 6 v4（bump v0.9.6、待 commit + tag）**：追加系统默认目录 checkbox（方案 B 解决"加自定义后系统默认消失"UX 坑，`AppSettings.include_system_defaults: bool` 默 false 保旧覆盖语义、非空时暴露 checkbox）+ FTS 索引进度可视化（`IndexStatus.current_root` + `fts_progress: (scanned, indexed)`、`StatusProgressBridge` impl `IndexProgress` trait 桥、local-index 新 `reindex_scoped_with_progress` API）+ 前端目录列表统一 effectiveRoots 渲染 + `indexStatusLine` 索引中显示「📁 当前目录　已扫描 N · 已入库 M」。全测过：indexer 119 / local-index 22 / desktop bin 141 / desktop::settings 12（含新 3 tagged branches 单测）/ desktop::search::index_status 8（含新 1 桥闭环单测）/ clippy 干净 / tsc 干净 / fmt 干净。**cycle 7-a + 7-b（v0.9.7 已发、真机验证 9/10 GO）**：索引目录管理 UX 打磨（警告条 / pending badge / flash / phase chip / indeterminate 进度条）+ 子路径排除通配符（`root_excludes` + `ExcludeFilter` 双层）。**cycle 7-c（bump v0.9.8 待 tag）**：单目录重扫 `reindex_root` + 打开目录（复用 `open_path`）+ 移除目录二次确认可选 purge（`purge_under_root` 存储层 SQL + `root_glob_predicate` 共用边界 + 外键级联单测）+ **(d) window.confirm 失效修复**（in-DOM `ConfirmModal`、wry/WebView2 生产环境 confirm 是 no-op）。详见 ROADMAP BETA-33 卡片 / 本日会话日志。

**BETA-32 团队归档 MCP daemon = done，已合 main（2026-06-29 Claude Code，[PR #21](https://github.com/raoliaoyuan/LociFind/pull/21)、merge commit `35725db`、分支已删）+ Windows 真机自验 GO（2026-06-29、wiring 全过 + 2 bug 登记 follow-up）⭐ 与主线 BETA→1.0 并行的衍生子线、不阻塞 1.0 出场标准；为团队共享归档（设计稿 / 标书 / 财务底稿）提供 headless MCP 检索服务**。**同会话顺带 v0.8.0 GUI Windows 真机验证 GO**（4 后端 fan-out 含语义召回全跑通 + 3 桌面 UX bug + 2 操作层痛点登记 follow-up）。

**关键决策**：① 范围 = **headless MCP daemon only**（不动桌面 GUI / 不动 release-windows.yml / 不动 PROJECT / CONVENTIONS 核心原则）；② transport = rmcp 1.8 streamable-HTTP（spec 锁版本、stateful=true 复用 LLM client tools/list 缓存）；③ bearer auth = `secrecy::SecretString` + `subtle::ConstantTimeEq` 常数时间比较；④ 真机部署 = **DEFERRED 用户自验**（BETA-31 同款节奏，管理员一台机器起 daemon + 另一台 Claude Code 接、跑 5 example query）；⑤ 真模型 v0.9 评测 = **DEFERRED**（成本太高、用户 / CI 环境跑、留 follow-up cycle）。

**改动概览**（7 commit + T14 doc-sync）：
- C2a-f `25b6999` / `886a91f` / `2e1976a` / `4eff5a2` / `51c4b6d` / `d8847a1`：packages/locifind-server lib（ServerConfig + ServerCtx + bearer auth + Tool trait + Search/ListRoots tool + admin handlers + MCP adapter + app.rs Router、29 单测）
- C3a-b `4eff5a2` / `93943ad`：apps/daemon binary `locifindd`（CLI 9 flag + preflight 6 检 + lifecycle graceful shutdown + 首次全量索引）
- C4 `829ce68`：apps/daemon e2e 集成测试 x5（health / auth / list_roots / search / reindex 409）
- C5 `5341b4d`：evals 加 `--mode daemon` + top-K 闸门 + 3 daemon_mode_smoke 测
- C6 `e5821b0`：三平台 binary CI workflow `release-daemon.yml`（Mac arm/x86 + Windows + Linux + SHA256 checksums） + ROADMAP BETA-32 卡片登记
- C7 `60ab0c9`：T14 doc-sync（apps/daemon/README.md 部署样板 + STATUS + ROADMAP 改 done + app.rs 注释 fix）
- 收尾 `3dc0c6a`：/code-review ultra 找到 3 critical fix（dual-db wiring + reindex stub 不写 state + JSON-RPC error 检查）已 inline 合 main

**接受标准（spec §8.2 红线 1-9 全过）**：fmt 净 / clippy 0 warning / workspace test 927 passed 0 failed / locifind-server 29 passed / locifindd e2e 5 passed / evals 75 passed（含 daemon_mode_smoke 3）/ desktop frontend vite 347ms ✓ + cargo check 净 / 真机部署 DEFERRED 用户自验。

**承接**：上一个 cycle BETA-31 收尾后启动衍生子线、与主线 BETA→1.0（cosine sweep / baseline rewrite / 真机 evals）并行；不与任何 hybrid 路径冲突。

> **近邻已完成里程碑**（详情见「会话日志」/ [归档](docs/session-logs/STATUS-archive-2026-06.md)，此处不复述）：
> - BETA-31 Windows 模型分发 UX 增强 done（[PR #20](https://github.com/raoliaoyuan/LociFind/pull/20) merged、merge commit `1f04f51`）
> - BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done（[PR #19](https://github.com/raoliaoyuan/LociFind/pull/19) merged、merge commit `e3670dc`）
> - BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验 done（[PR #18](https://github.com/raoliaoyuan/LociFind/pull/18) merged、Branch I-a GO ⭐⭐）
> - BETA-15B-10 bge-m3 baseline 重锚 + cosine sweep & bake + 长文本扩量 + 截断解除 done（[PR #17](https://github.com/raoliaoyuan/LociFind/pull/17) merged）
> - BETA-15B-7-v2 hotfix BERT encode n_ubatch panic（[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16) merged）

## 阻塞 / 待用户决策

**Class A（需用户启动外部条件，解锁真实价值）**——详见 [ROADMAP §5 长周期事项](./ROADMAP.md)：

1. **BETA-09(a) 跨平台部署 / MVP-26 跨平台一致性 / MVP-28 出场评测** — 需 Windows 真机 + 完整 Spotlight 索引的 macOS 机器跑双平台 v0.5 evals（M→B 切换硬指标"双平台 evals 差距 <5pp"尚未物理跑过）。
2. **长周期事项** — Apple Developer Program 注册 / Windows OV·EV 证书采购 / locifind.ai·.app·.dev 域名 / 商标申请。不阻塞代码但阻塞 Beta 分发，宜尽早启动。

**Class B（产品决策）**：

1. **评测语料隐私方案 = 已拍板「混合」**（2026-06-21）：合成集入仓做 CI 门控主基准 + gitignored 真实 spike 本地校准。已由 BETA-15B-6 落地。**语义召回质量调优（RRF 权重 / 更大模型 / 下限精校）解锁**——baseline 已出（Phase D done），下一 cycle 调优起点 = [baseline 报告](docs/reviews/semantic-recall-quality-baseline.md)。

## 总体进度

完成 / 进行中 / 待办的 task 状态以 [ROADMAP.md](./ROADMAP.md) 中各 task 卡片的 `状态` 字段为准。本节只给阶段级摘要。

| 阶段 | 进度 | 备注 |
|---|---|---|
| **设计前置（PROTO-00）** | ✅ 完成 | Schema v1.0 + Trait v0.1 + Codex/Gemini 双轨审阅 + ROADMAP v1.0 |
| **P：技术原型** | ✅ 已完成 | 11/11 task；variant 命中 92%；出场报告 [docs/reviews/proto-exit.md](./docs/reviews/proto-exit.md) |
| **M：MVP** | ✅ 代码层全 done | M1 12/12、M2 3/3、M3 4/4、M4 7/7、M5 4/4。M→B 正式切换待 §8 非代码长周期项 + BETA-09(a) Windows 部署 |
| **B：Beta** | 🔄 进行中 | B1 本地索引除 macOS Vision OCR 全部落地；多源融合 / Ranker / Audit / 后台索引 / 跨范畴均衡 done。出场标准见 [ROADMAP §6.3](./ROADMAP.md) |

## 下一步

> 任务详情（依赖 / 估时 / 验收）在 [ROADMAP.md](./ROADMAP.md)，本节只给"下次会话可立即上手"的指向。**不复述已完成工作**——已完成项见「会话日志」+ ROADMAP 各 task 状态。

- **【当前焦点 — 2026-07-02 v0.9.8 tag + 装机验证 cycle 7-c ⭐】**：cycle 7-c + (d) confirm 修复已合入、bump v0.9.8 待 tag（推 `v0.9.8` tag 触发 release-windows.yml）。**装机验证清单**：(a) 单目录「重扫」只扫该目录且 root_excludes 仍生效、其他 root 计数不变；(b)「打开」拉起 Explorer；(c) 移除选「移除并清除索引记录」→ 概貌该目录计数归零、选「仅移除」→ 记录保留；(d) 带未保存改动点 ✕ / Esc / 取消 → **应用内 ConfirmModal** 出现（不再静默丢弃）、弹窗内 Esc 只关弹窗；(e) 移除确认弹窗文案明确"不删除磁盘文件"。**验证后接 cycle 8+ 候选**：① 无单实例锁（2026-07-02 真机验证发现、两实例并发写同一 index.db / settings.json、tauri-plugin-single-instance、~0.5d）；② embed 报错 degrade 到其他 3 后端（老 bug、~30 分钟）；③ 抽 `useAppSettings()` hook + 删 SettingsPage 旧路由（~1d）；④ 「本地索引」行全局口径 vs 概貌 root-scoped 口径统一（~0.5d）；⑤ StatusIndicator/EmbedStatus 文案重写（~0.5d）。

- **【前一焦点 — 2026-06-29 BETA-31-v3 v0.8.0 真机 UX gap 修复集第 1 刀 + v0.8.1 release ⭐】**：用户切到 Windows 真机后报 2 v0.8.0 UX bug（索引目录数量看不到 + 重启显示「尚未索引」）。改方案 B：① main.rs setup 加启动回填 `IndexStatus.last_summary`（`compute_index_totals` 走 SQLite COUNT、毫秒级）；② SettingsPage.tsx 改 4 处文案（标题加「当前 X 个」、空时默认目录提示、加目录后 5s toast、状态行分四档）；③ bump v0.8.1 + tag 触发 release-windows.yml 出新 NSIS。**5 UX bug 中本 cycle 修了 2 个**（前 2 项 UX gap），剩 3 个留 BETA-31-v3 后续子刀（顶栏灯 ≠ EmbedStatus / 设置页 NotLoaded→NotFound 渲染脱节 / Everything 检测失败 GUI 引导）+ embedding 冷启动 17s 留 BETA-31-v2 GPU cycle + Everything 服务模式留 follow-up。**v1.0 路径上的其他候选**（按优先级）：① **cosine_threshold 在 embeddinggemma 上 sweep & bake**（拿回 sweep best 0.882 +0.008、~1d）；② **baseline.json rewrite 切到 embeddinggemma 数据**（与桌面解耦、~0.5d）；③ **BETA-15B-11-v3 prefix API 接 model-runtime**（+0.013~+0.026 各桶加成、~1w）；④ **BETA-31-v2 Windows GPU 推理优化**（vulkan、~1-2w、spec 已起草入仓、cycle 待用户装 VS Build Tools + Vulkan SDK 后接续）；⑤ **BETA-31-v3 剩余 3 子刀**（顶栏灯 / NotLoaded 渲染 / Everything 引导）；⑥ **BETA-31-v4 模型 SHA256 签名验证**（~0.5d、原 v3 编号让位）；⑦ **评测扩量 crosslang 桶**（14 例校验运气、~1d）；⑧ **BETA-32 follow-up backlog**：T7 reindex 真扫盘 / preflight 检查 leftover / SearchHit.size 补字段 / check_token chars vs bytes / `--config` TOML 接通或删 flag / DaemonHandle drop 等 abort / 启动 banner 不被 log filter 吞 / mcp_client 与 e2e.rs helper 共享 / rmcp e2e helper 重写（lib done、helper 留 follow-up）。**v1.0 路径上的其他候选**（按优先级）：① **cosine_threshold 在 embeddinggemma 上 sweep & bake**（拿回 sweep best 0.882 +0.008、~1d）；② **baseline.json rewrite 切到 embeddinggemma 数据**（与桌面解耦、~0.5d）；③ **BETA-15B-11-v3 prefix API 接 model-runtime**（+0.013~+0.026 各桶加成、~1w）；④ **BETA-31-v2 Windows GPU 推理优化**（vulkan/cuda、~1-2w、需 Windows 真机）；⑤ **BETA-31-v3 模型 SHA256 签名验证**（~0.5d、增加下载安全性）；⑥ **评测扩量 crosslang 桶**（14 例校验运气、~1d）；⑦ **BETA-32 follow-up backlog（/code-review ultra 12 项）**：T7 reindex 真扫盘 / T6 #6 ServerCtx search_candidates cache / preflight 检查 documents.db.* leftover / doc_count 语义统一 / SearchHit.size 补字段 / check_token chars vs bytes / `--config` TOML 接通或删 flag / DaemonHandle drop 等 abort / 启动 banner 不被 log filter 吞 / mcp_client 与 e2e.rs helper 共享 / `release-windows.yml` 同款 `--locked` gap / rmcp `stateful_mode=false` 切 stateless 简化 e2e。
- **【BETA-13 这条线建议收束】**（§6=87.7%，距 90% 出场线差 ~2.3pp，纯 parser/coverage 在 d2/d3/d5 块基本见底、不可达）：剩 **4 fail** = 2 条 v0.5 基线（不可作为）+ 2 条 §1.1↔§3.1 契约冲突（`screenshots…sorted by name`/`videos…sorted by size`：screenshot/video+排序词归 media〔§1.1〕还是 file〔§3.1〕，**须评测集 re-baseline、动 v0.5 锁定基线，是产品决策非 parser**）；119 partial 散落各块无单一大块、逐条 ROI 低且多有 byte-equal 风险。**继续抬指标须用户拍板 §1.1 re-baseline，或转新特性**。
- **【本机可立即上手的代码层候选（不卡外部条件）】**：① **BETA-15B-11-v2 bake embeddinggemma-300m 推到生产**（**BETA-15B-11 follow-up 最高优**、~1-2w）；② **BETA-15B-11-v3 prefix API 接 model-runtime**（~1w）；③ **评测扩量**（~1d）；④ **模型分发 UX**（~1-2w）；⑤ **BETA-20 v2 预览增强**；⑥ **BETA-12 卸载流程**（~2d）；⑦ **BETA-15B-3 簇 A 余项**：原始 query 入 schema；⑧ **BETA-15B-4**（Windows GPU + 媒体/OCR 臂 + sqlite-vec）；⑨ **BETA-03 macOS Vision OCR**（Mac 会话）；⑩ **BETA-28 索引预算与分层新鲜度**（~1-2w）；⑪ **BETA-29 查询意图可编辑草稿**（~1-2w）；⑫ **BETA-30 本地失败样本箱**（~1-2w）；⑬ **BETA-33 桌面菜单栏重构 + 选项对话框**（**2026-07-02 cycle 7a+7b+7c done、v0.9.7 已发 + v0.9.8 待 tag**）——剩余 cycle 8+：① embed 报错 degrade 到其他 3 后端（老 bug、~30 分钟）；② dev feature-full script（需装 cmake、~0.5d）；③ 抽 `useAppSettings()` hook + 删 SettingsPage 旧路由（~1d）；④ PrivacyPage 迁分类（~1d）；⑤ 上下文 enable/disable（~0.5d）；⑥ 子菜单 ▸ 展开（~1d）；⑦ 后端接通 plugin-shell（~1d）；⑧ StatusIndicator/EmbedStatus 文案重写（~0.5d）；⑨ macOS 原生菜单 BETA-34（~3-5d）。详见 [ROADMAP §3.3 B6](./ROADMAP.md) BETA-33 卡片。
- **【byte-equal 闸门方法】**（改 coverage/parser 必走）：reporter JSON 非确定（HashMap 序 + elapsed_ms），用规范化逐 case 比对（`--json` 输出按 id 比 `actual_json`，v05-* 子集 0 变化即 byte-equal）。改 coverage 必走 shards→assemble-coverage→generate-evals-v09，**勿手改 coverage-cases.json**（会被静默回退）。详记忆 [[project-evals-coverage-pipeline-drift]]。
- **【待用户真机操作（已实现、待验证/调参）】**：① **语义相似度下限 bake**——装 v0.4.0 看分数分布，把「语义相似度下限」从 0.30 调到甜点值反馈数值，我 bake 为 `DEFAULT_SIMILARITY_FLOOR`（v0.3.0 已确认 0.30 偏松）；② 各特性真机 UI 手测见 [docs/manual-test-scenarios.md](./docs/manual-test-scenarios.md)（BETA-27 索引目录 / BETA-25 打包 / BETA-24 复合查询 / BETA-15B-2 暖机 / BETA-03/06/07 等，按特性组织）。
- **【每次开场先确认 Class A 哪条具备启动条件】**（详见上方「阻塞 / 待用户决策」）：Windows 真机双平台 evals / Apple Developer 注册 / 证书·域名·商标。决定具备条件再推。
- **【Windows 发版流程】**：推 `v*` tag 触发 `.github/workflows/release-windows.yml`；Release 说明须含 changelog（CONVENTIONS §8）；发版前先 bump app 内部版本号（tauri.conf.json + Cargo.toml）。
- **【代码层 Class B 残留】**：见 [ROADMAP §3.3 B 阶段](./ROADMAP.md) 未完成 task，残留多为外部依赖（GBNF 新版）或边际收益较低（Tier 2 LoRA / partial 精度）。RAG 文件问答已登记 V 阶段旗舰 **V10-13**（Beta 前不动手）。
- **【2026-06-25 Claude × Codex 联合规划 5 个新特性已登记 ROADMAP】**（借鉴 [LLM Wiki 文章](https://axk51013.medium.com/rethinking-agent-harness-part4-llm-wiki-%E5%8F%96%E4%BB%A3-rag-041629319804) 四维度 + sweet spot + 反例框架）：① **BETA-28 索引预算与分层新鲜度**（Data 反例护栏）；② **BETA-29 查询意图可编辑草稿**（parser 长尾产品化解法）；③ **BETA-30 本地失败样本箱**（私有 eval 闭环）；④ **V10-15 Frozen Research Pack**（LLM Wiki sweet spot 唯一入口、V 旗舰、配套 V10-16）；⑤ **V10-16 LLM 读权限与出处闸门**（ACL 反例护栏、横切所有 LLM 功能、与 V10-15 绑定发布）。详 ROADMAP §3.3 B6 / §3.4 V 阶段卡片。

## 会话日志

> 仅保留最近约 8-10 条；更早全部历史见 [docs/session-logs/STATUS-archive-through-2026-06-03.md](./docs/session-logs/STATUS-archive-through-2026-06-03.md)（含完整 78 条 + 滚动归档区 + 重构前的顶部摘要与当前 Task 历史，逐字保留）。**2026-07-01 II 收工已归档**：新 6 条（cycle 3 v2 + cycle 3 + 06-30 cycle 6 v3 + 06-30 cycle 1+2 + 06-30 cycle 6 v2 + 06-30 第 7 刀 cycle 6）滚到 [archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) 2026-07-01 滚动归档 II 批；本 STATUS 剩 6 条（cycle 7c / cycle 7a+7b / cycle 6 v4 / cycle 5 / cycle 4 / cycle 3 v3+v4）。

### 2026-07-02 — Claude Code (Fable 5) — v0.9.7 真机验证 (a)-(j) + BETA-33 cycle 7-c 三件套 + window.confirm 失效修复 + bump v0.9.8（待 tag ⭐⭐⭐⭐⭐）

**承接**：v0.9.7（`8ce7a65`）昨日已发已装机。用户「你直接帮忙真机验证」→ 我全程接管 GUI 驱动验证；验证毕用户拍板「接着开工 cycle 7-c（含把 (d) 的 confirm 机制一并修掉）」。

**上半场：v0.9.7 真机验证（9 GO / 1 FAIL）**。方法学 = 备份 settings.json → 重置为受控测试状态 + 独立夹具目录（`D:\LociFindVerify97` 含 `临时/`、`sub/backup/`、`node_modules/` + 唯一 token 文件）→ PowerShell 原生截图 + Win32 鼠标驱动装机版 GUI → sqlite3 直查 index.db 验证 → 全程完毕恢复原配置重启应用（用户存量索引 4887 文档+图片 / 34185 音乐核对无损）。**结果**：cycle 7-a (a) pending badge + sticky 提示 GO（flash 1.5s 动画未定格、不判失败）、(b) 警告条 + checkbox 强化 GO、(c) 勾选追加 + 可逆 + snapshot 精确 GO、(e) indeterminate 进度条 + 🎵 phase chip + 计数行 GO、(f) 完成后概貌/时间戳强刷 GO；cycle 7-b (g)(h) **双维度 GO**（排除生效后新增文件不入库 + 对照组入库；意外加分 = 存量条目被 stale-prune 追溯清除、排除是回溯生效的）、(i) root_excludes 孤儿清理 GO、(j) node_modules 全库 0 条 GO。**唯一 FAIL = (d)**：带未保存改动点 ✕ 静默关闭、无确认弹窗；checkbox-only 对照实验证明 `hasUnsavedChanges=true` 时仍放行 → 根因 = **wry/WebView2 生产装机版 `window.confirm` 是 no-op**（代码逻辑本身正确）；写入记忆 tauri-webview2-window-confirm-noop。**顺带发现**（非本 cycle 引入、登记 follow-up）：① 无单实例锁——用户实例运行时我误启第二实例成功、两实例并发写同一 index.db / settings.json；② 「本地索引」行（全局计数、音乐 34185）与概貌（root-scoped、音乐 2）两套口径并存易困惑。

**下半场：cycle 7-c 实施（Codex SUGGEST 6/7/8/10 全落地 + (d) 修复）**。
- **indexer 存储层**：[db.rs](packages/indexer/src/db.rs) 新 `root_glob_predicate(col)` + `root_glob_params(root)` 共享边界 helper、`MusicIndex::stats_under_root` 重构复用 + 新 `MusicIndex::purge_under_root`（同事务 FTS 同步删）；[doc_db.rs](packages/indexer/src/doc_db.rs) 同款重构 + `DocumentIndex::purge_under_root`（documents_fts 同步删、document_vectors 走既有 `ON DELETE CASCADE`）；+3 单测（music/doc 子树清除含 FTS 验证 + 兄弟前缀目录不误删 + 幂等 + **外键级联单测**）。
- **desktop 后端**：[index_status.rs](apps/desktop/src-tauri/src/search/index_status.rs) `perform_reindex` 抽 `perform_reindex_for_roots(…, roots_override)`（override 只换 roots、exclude/OCR/progress bridge 仍 settings live-read）；[main.rs](apps/desktop/src-tauri/src/main.rs) 新 command `reindex_root`（目录存在校验 + spawn_blocking + 完成接语义 worker、与全量同构）；[settings.rs](apps/desktop/src-tauri/src/settings.rs) 新 command `purge_root_from_db`（薄封装 + `PurgeSummary`）；两命令注册 invoke_handler。
- **前端**：[PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) 新通用 in-DOM `ConfirmModal`（标题 + 消息 + 可选单选组 + danger 样式）——`handleCloseWithGuard` 弃 window.confirm 改弹它；移除目录走 `requestRemoveRoot` 二次确认（「仅从索引配置移除」默认 vs「移除并清除索引记录」、文案明确不删磁盘文件、purge 失败不移除配置）；RootRow 加「打开」（复用既有 `open_path` 命令、FileActionTool 策略 + audit 口径一致、**与 design doc plugin-shell 方案的合理偏差 = 零新依赖**）+「重扫」按钮（pending root 不显示、索引中禁用）；Esc 在弹窗打开时只关弹窗不穿透外层守卫（Sogou IME workaround 路径不动）；[styles.css](apps/desktop/src/styles.css) `.prefs-confirm-*` 系列。
- **版本**：tauri.conf.json + Cargo.toml + Cargo.lock 三处 0.9.7 → **0.9.8**。

**接受标准（本机全测过）**：`cargo test -p locifind-indexer --lib` **130 passed**（+3）/ `-p locifind-local-index-backend --lib` 22 passed / `-p locifind-desktop --no-default-features --bins` **145 passed** 3 ignored / `cargo clippy … -D warnings` 干净 / `cargo fmt --check` 干净 / `npx tsc --noEmit` 干净。

**真机验证清单（v0.9.8 tag 装机后）**：(a) 单目录「重扫」只扫该目录且排除仍生效；(b)「打开」拉起 Explorer；(c) 移除选「清除」→ 概貌计数归零、选「仅移除」→ 记录保留；(d) 带未保存改动点 ✕ / Esc / 取消 → 应用内 ConfirmModal 出现、弹窗内 Esc 只关弹窗；(e) 弹窗文案明确"不删除磁盘文件"。

**用户对话流水**：① 用户问当前待执行任务 → 报 STATUS 摘要 → ② 「你直接帮忙真机验证」→ 全程 GUI 驱动跑 (a)-(j) + sqlite 验证 + 恢复现场 + 报告 9 GO / 1 FAIL → ③ 「接着开工 cycle 7-c（含把 (d) 的 confirm 机制一并修掉）」→ 读 design doc §5 + 源码 → 实施 indexer/desktop/前端三层 + bump v0.9.8 + 全测 → ④ 「收工」→ 本次 doc-sync + commit。

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 7-a + 7-b：索引目录管理 UX 打磨 + 子路径排除通配符 + bump v0.9.7（待 tag、Codex APPROVED with suggestions ⭐⭐⭐⭐⭐）

**承接**：v0.9.6 (`1781fd3`) 昨天刚发、cycle 6 v4 首次真机验证由本会话完成。用户反馈两个 UX 缺口 + 新需求「子路径通配符排除」：① 加了自定义目录后系统默认三夹（Music/Documents/Pictures）"消失"看似丢失（**实际是 override 覆盖语义正确行为**、cycle 6 v4 已加 include_system_defaults checkbox opt-in，但 UI 灰色小字用户没引起注意）；② 本地索引进度只显示静态"已扫描 0 · 已入库 0"，看不到当前扫哪个目录、也看不到 phase；③ 希望友好目录管理 + 支持子路径排除通配符。

**方法学**：**先规划、再复现诊断、再 Codex 评审、再实施**——按 CLAUDE.md 三工具协作节奏。
1. **规划阶段**：写 [docs/reviews/beta-33-cycle-7-index-mgmt-ux.md](docs/reviews/beta-33-cycle-7-index-mgmt-ux.md) design doc，拆三刀 7-a UX / 7-b 子路径排除 / 7-c 老 follow-up；
2. **诊断阶段**：state 注入法（改 settings.json 塞 Downloads + include_system_defaults=false）+ PowerShell 原生 System.Drawing.CopyFromScreen + Win32 SetCursorPos/mouse_event 驱动（DPI 125% 补偿；computer-use screenshot 对 msedgewebview2 mask 失效换 native 截图）→ 12 张 shot 观察 → 排除 A（真 bug）、锁 C 主 + B 副 + 数据源不一致 + 进度可视化断层（详 [doc §1.4](docs/reviews/beta-33-cycle-7-index-mgmt-ux.md)）；
3. **Codex 评审**：computer-use 拉起 Codex 桌面版 → prompt 让它评审 doc → **APPROVED with suggestions**（4 APPROVED + 3 OBJECT + 10 SUGGEST、1m26s、+44 -0 追加 §10）→ 回写 doc §1.5/§3/§4/§5/§8 D5 合入意见；
4. **实施**：cycle 7-a（前后端 7 文件）+ cycle 7-b（前后端 8 文件）连做。

**关键决策**：① **cycle 7 出货节奏 = 三刀合一 v0.9.7**（用户 AskUserQuestion 拍板；避免多次装机-反馈往返）；② **子路径排除语义 = 相对 root 的 path glob**（`临时/**` / `**/backup/**` / `*.old/*`；心智模型清晰、表达力强）；③ **Codex OBJECT 3 采纳**：不做 walkdir 预扫 count（大目录多次 IO 打在最怕的路径），indeterminate 动画进度条 + `已扫描 N · 已入库 M`；④ **Codex OBJECT 2 采纳**：保留旧 `index_dirs_excluding(&GlobSet)` API 委托新 `_with_filter` API，BETA-27 basename-only byte-for-byte 保护；⑤ **Codex OBJECT 1 采纳**：`root_excludes: Vec<RootExclude>` 而非 `HashMap<String, Vec<String>>`（未来 enabled/comment 易扩展、Windows 路径 key 更稳）；⑥ **Codex SUGGEST 9 采纳 → 替代 pending badge 主机制**：picker 后新目录 flash 高亮 + scrollIntoView，比 pending badge 更直觉；⑦ **cycle 7-c 留后续会话**（用户选"现在收工装机验 7-a+7-b"）。

**改动概览（cycle 7-a：7 文件 · cycle 7-b：8 文件 · 单次合并 commit）**：
- **cycle 7-a 后端**：[packages/indexer/src/progress.rs](packages/indexer/src/progress.rs) 新 `IndexPhase` enum + `on_phase(phase)` trait method 默认 no-op + `SpyProgress` 计数 phase + Cargo.toml 新 dep `serde` + `lib.rs` 导出 `IndexPhase`；[local-index/src/lib.rs](packages/search-backends/local-index/src/lib.rs) `reindex_with_progress_inner` 每 phase 前调 `on_phase`（MusicDiscovery / MusicScan / Doc / Image）；[apps/desktop/src-tauri/src/search/index_status.rs](apps/desktop/src-tauri/src/search/index_status.rs) `IndexStatus` 加 `current_phase` 字段 + `StatusProgressBridge::on_phase` impl（MusicDiscovery 时同步清 current_root）+ fts_begin/finish 清 phase
- **cycle 7-a 前端**：[PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) 新 `IndexPhase` type + `phaseChipLabel` + `initialSettings` snapshot + `hasUnsavedChanges` memo + `handleCloseWithGuard` 二次确认 + `flashPath`/`onFlash` 1.5s 计时器 + `RootRow` 新 `isPending`/`flash` props + 大字警告条 + checkbox 强化 + phase chip 拼进 `indexStatusLine` + 概貌"目录"改「生效目录」+ tooltip + `prevIndexing` useEffect 完成时强刷 indexStatus；[SettingsPage.tsx](apps/desktop/src/pages/SettingsPage.tsx) fallback `IndexStatus` 加 `current_phase`；[styles.css](apps/desktop/src/styles.css) `.prefs-warn-banner` + `.prefs-checkbox-strong` + `.prefs-root-tag.pending` + `@keyframes prefs-row-flash` + `.prefs-progress-indeterminate` + `@keyframes prefs-progress-slide`
- **cycle 7-b 后端**：[apps/desktop/src-tauri/src/settings.rs](apps/desktop/src-tauri/src/settings.rs) 新 `RootExclude { root, patterns }` struct + `AppSettings.root_excludes` + `normalize_root_key(path)` 跨平台归一 helper（Windows 反斜杠→\+小写、Unix 正斜杠+保留大小写、trim 尾部分隔符）+ 5 单测；[packages/indexer/src/scan.rs](packages/indexer/src/scan.rs) 新 `ExcludeFilter` struct（basename + per_root 双层）+ `from_basename_set(&GlobSet)` 兼容构造 + `build(exclude_globs, root_excludes, normalize)` 生产构造（`/**` 结尾自动补目录本身 pattern）+ `is_excluded_dir(entry, normalize)` Windows 分隔符归一 + 6 单测 + `run_incremental_index_with_filter_and_progress` + 3 个 `_with_filter_and_progress` 方法（Music/Doc/Image）+ Cargo.toml/lib.rs 导出；[local-index/src/lib.rs](packages/search-backends/local-index/src/lib.rs) 新 `reindex_scoped_with_filter_and_progress` + `_inner`（phase 通知同旧版）；[index_status.rs](apps/desktop/src-tauri/src/search/index_status.rs) 新 `read_index_config_with_filter` + `perform_reindex` 走 filter 版本
- **cycle 7-b 前端**：[PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) `AppSettings.root_excludes` + `RootExclude` interface + `RootRow` 加折叠区「子路径排除 ▸ (N)」+ 输入 + hint 说明通配符 + `excludesFor` / `updateExcludesFor` + `removeRoot`（同步删孤儿 root_excludes 条目）；[SettingsPage.tsx](apps/desktop/src/pages/SettingsPage.tsx) fallback `AppSettings` 加 `root_excludes`；[styles.css](apps/desktop/src/styles.css) `.prefs-btn.has-excludes` + `.prefs-root-excludes` + `.prefs-exclude-row` + `.prefs-exclude-add-row`
- **cycle 7-a doc 回写**：[docs/reviews/beta-33-cycle-7-index-mgmt-ux.md](docs/reviews/beta-33-cycle-7-index-mgmt-ux.md) §1.5 / §3 / §4 / §5 / §8 D5 融入 Codex OBJECT + SUGGEST；§10 保留 Codex 原评审
- **版本 bump**：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.9.6 → **0.9.7**

**接受标准（本机 cargo + tsc 全测过）**：
- ✅ `cargo test -p locifind-indexer --lib` **127 passed**（原 119 + 8 新：`noop_progress_on_phase_default_impl_is_no_op` / `spy_progress_records_phase_calls` + `ExcludeFilter` 6 单测 `from_basename_set_matches_old_behavior` / `build_appends_dir_itself_pattern` / `windows_separator_normalization` / `empty_root_excludes_short_circuits_to_basename` / `walkdir_prunes_matching_subtree_per_root` / `per_root_does_not_leak_across_roots`）
- ✅ `cargo test -p locifind-local-index-backend --lib` **22 passed**
- ✅ `cargo test -p locifind-desktop --no-default-features --bins` **145 passed** 3 ignored（原 140 + 5 新 settings 单测：`root_excludes_default_is_empty` / `root_exclude_serde_round_trip` / `normalize_root_key_windows_equivalence` / `normalize_root_key_empty_returns_empty` / `old_settings_without_root_excludes_parses_ok`）
- ✅ `cargo test -p locifind-desktop settings::` **17 passed**
- ✅ `cargo test -p locifind-desktop search::index_status::` **9 passed**（含新 `phase_bridge_sets_current_phase_and_clears_root_on_music_discovery`）
- ✅ `cargo clippy -p locifind-indexer -p locifind-local-index-backend -p locifind-desktop --no-default-features --lib --bins --tests -- -D warnings` 干净（顺手删旧 `is_excluded_dir` 无 caller fn + 修 `derivable_impls` + `redundant_closure_for_method_calls` + `single_match_else`）
- ✅ `cargo fmt --all -- --check` 干净
- ✅ `npx tsc --noEmit` (apps/desktop) 干净

**未做 / 已登记 follow-up**（cycle 7-c 下会话 + 其他 backlog）：
- ① **cycle 7-c 三件套**（Codex SUGGEST 6/7/8/10）：单目录重扫（抽 `perform_reindex_for_roots` 内部函数复用同套配置解析）+ 打开目录（tauri-plugin-shell open）+ 移除目录二次确认对话框（明确"不删除磁盘文件"）+ `DocumentIndex::purge_under_root` / `MusicIndex::purge_under_root` SQL 抽到存储层复用 `stats_under_root` 边界 helper + 外键级联单测
- ② embed backend 失败时不 degrade 到其他 3 后端（老 bug）
- ③ dev feature-full script（需装 cmake、`npm run tauri dev -- --features semantic-recall,model-fallback` 才有真语义能力）
- ④ 抽 `useAppSettings()` hook + 删 SettingsPage 旧路由（消除重复 ~120 行）
- ⑤ StatusIndicator/EmbedStatus 文案 UX 整体重写

**真机验证清单（v0.9.7 tag 装机后）**：**cycle 7-a**：(a) 加 Downloads → 新目录 flash 高亮 + `⏳ 待应用` badge + 底部 sticky "未保存"提示；(b) 大字警告条「已隐藏系统默认目录」出现 + checkbox 绿描边加粗；(c) 勾 checkbox → 警告条消失 + 系统默认追加；(d) 关闭对话框有未保存 → window.confirm 二次确认；(e) 立即索引 → indeterminate 进度条 + phase chip 切换（🎵 → 📄 → 🖼）；(f) 索引完成后顶部概貌"生效目录 N" + 上次索引强刷。**cycle 7-b**：(g) 展开 Downloads「子路径排除 ▸」→ 加 `临时/**` → 应用 → reindex → `Downloads/临时` 下条目不入库；(h) 加 `**/backup/**` → 任意深度 backup 子目录都剪；(i) 移除 root → 对应 root_excludes 条目自动清（无孤儿）；(j) 全局 `exclude_globs`（node_modules 等）仍生效。

**用户对话流水**：① 用户报"添加了目录后无法正常显示 + 看不到索引进度目录 + 需要子路径排除通配符"、请 Codex 一起规划 → ② 我诊断 v0.9.6 = 首次真机验证需先排查、写 design doc、AskUserQuestion 4 决策（版本 v0.9.6 / 节奏三刀合一 / glob 语义 / 交接方式）→ ③ 用户「授权诊断复现」→ state 注入 + PowerShell 驱动 GUI 12 shot 观察、锁定 C 主 + B 副 + 数据源不一致 + 进度断层 → ④ 用户选「先回写 doc（合入 Codex OBJECT/SUGGEST）再开工」→ ⑤ 用户「拉起 Codex 帮忙评审」→ computer-use 打开 Codex 桌面 + prompt 送出 + 等待 1m26s → Codex APPROVED with suggestions + 3 OBJECT + 10 SUGGEST 追加 §10 → ⑥ 回写 doc 融入 Codex 意见 → ⑦ 用户「开工实施」→ 连做 cycle 7-a（前后端 7 文件 + 3 新 test）+ cycle 7-b（前后端 8 文件 + 11 新 test）→ ⑧ 用户「现在就收工提交 7-a+7-b bump v0.9.7 装机验」→ 归档 + doc-sync + bump + 待 commit。

---

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 6 v4：追加系统默认目录 checkbox + FTS 索引进度可视化（bump v0.9.6、待 tag）⭐⭐⭐

**承接**：cycle 5 已 doc-sync 待发。用户在 Windows 真机 v0.9.5 之前版本反馈两个 UX 缺口：① 加了 Downloads 后系统默认 3 目录（Music/Documents/Pictures）**不再显示**、看似丢失（实际是 override 覆盖语义、旧行为）；② 索引进度只有静态"正在后台索引…（当前: 音乐 34185 / 文档 187 / 图片 4687）"、看不到当前正在扫哪个目录、也没有百分比 / 进度数字。用户「一起修复」——两个改动一起做进 v0.9.5 出货。

**关键决策**：① **问题 1 = 追加语义 opt-in checkbox**（方案 B，不破旧覆盖能力）：AppSettings 加 `include_system_defaults: bool` 默认 `false`（旧覆盖语义、零回归）；前端在 `index_roots.length > 0` 时显示 checkbox「☐ 同时索引系统默认目录」；勾上后追加 3 系统夹与自定义并列扫。② **问题 2 = 桥 `IndexProgress` trait 到 `IndexStatus`**：加 `current_root: Option<String>` + `fts_progress: Option<(u64, u64)>` 字段；新 `StatusProgressBridge` impl `locifind_indexer::IndexProgress`，每单文件回调 +1 scanned/indexed + 更新 current_root 到该文件父目录；桌面 `perform_reindex` 走 local-index 新加 `reindex_scoped_with_progress`。③ **音乐 Everything 发现分支不进度**：`index_paths` 无 progress hook、UI 音乐阶段（秒级 Everything 全盘扫）current_root 保上一状态、可接受体感 trade-off。④ **前端目录列表统一按 effectiveRoots 渲染**：自定义项显示「移除」、系统默认项显示「系统默认」tag——一致的视觉、消除"empty vs 非 empty 两套渲染"分支。⑤ **删旧 `resolve_index_roots(raw)` wrapper**（无剩余 caller）+ 删 `fts_set_current_root` 占位 fn（YAGNI，未来需要再加）。⑥ **不 bump 版本**（cycle 4/5 已 bump v0.9.5、cycle 6 v4 同版本合并出货）。

**改动概览（6 文件 +250/-70、双面改动）**：
- 后端 Rust：
  - [settings.rs](apps/desktop/src-tauri/src/settings.rs) +112/-16：AppSettings 加 `include_system_defaults: bool` 字段 + 默认 false + doc 注释；新 `system_default_roots()` + `resolve_index_roots_tagged(raw, include_defaults) -> Vec<(PathBuf, bool)>` 主 API（3 分支：空/覆盖/追加）；`get_effective_index_roots` / `get_index_overview` 加 `include_system_defaults: Option<bool>` 参数 + 抽 `read_effective_inputs` helper；删旧 `resolve_index_roots(raw)` wrapper；3 新单测（tagged 3 分支 + 去重 + 默认零回归）
  - [privacy.rs](apps/desktop/src-tauri/src/privacy.rs:167) 5 行：`resolve_index_roots(&settings.index_roots)` → `resolve_index_roots_tagged(&settings.index_roots, settings.include_system_defaults).map(|(p,_)| p)`
  - [search/index_status.rs](apps/desktop/src-tauri/src/search/index_status.rs) +85/-6：`IndexStatus` 加 `current_root: Option<String>` + `fts_progress: Option<(u64, u64)>` 字段；`fts_begin` / `fts_finish` 生命周期 fn；`StatusProgressBridge` struct impl `IndexProgress` trait（on_file 累加 + 父目录更新 current_root）；`perform_reindex` 走带 progress 变体 + fts_begin/finish 包夹；`read_index_config` 走 tagged 版；1 新单测（桥闭环 begin → on_file × 2 → finish）
  - [local-index/lib.rs](packages/search-backends/local-index/src/lib.rs) +75：新 `pub fn reindex_scoped_with_progress(roots, exclude, progress: &dyn IndexProgress)` API + 内部 `reindex_with_progress_inner`（音乐发现分支保 `index_paths`、fallback + 文档 + 图片全走 `_with_progress` 变体）
- 前端 TS：
  - [PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) +85/-40：`AppSettings` interface 加 `include_system_defaults: boolean`；`IndexStatus` interface 加 `current_root: string | null` + `fts_progress: [number, number] | null`；`IndexingPane` 目录列表改成统一按 `effectiveRoots` 渲染 + `index_roots.length > 0` 时暴露 checkbox；`indexStatusLine` memo 索引中显示「⏳ 正在索引：📁 current_root　已扫描 N · 已入库 M」；`get_effective_index_roots` / `get_index_overview` 调用都传 `includeSystemDefaults`；useEffect 依赖数组加 `settings.include_system_defaults`
  - [SettingsPage.tsx](apps/desktop/src/pages/SettingsPage.tsx) +6：fallback 页 `AppSettings` + `IndexStatus` interface 加对应字段（防 spread 时丢字段）；不加 UI（fallback 页保持简洁）

**接受标准（本机 cargo + tsc 全测过）**：
- ✅ `cargo test -p locifind-indexer --lib` 119 passed 0 failed（cycle 5 沿用）
- ✅ `cargo test -p locifind-local-index-backend --lib` 22 passed 0 failed
- ✅ `cargo test -p locifind-desktop --no-default-features --bins` 141 passed 3 ignored（Windows 真机 e2e 保留 ignored）
- ✅ `cargo test -p locifind-desktop settings::` 12 passed（含新 3：tagged 3 分支 + 去重 + 默认零回归）
- ✅ `cargo test -p locifind-desktop search::index_status::` 8 passed（含新 1：fts_progress_bridge_ticks_and_updates_current_root）
- ✅ `cargo clippy -p locifind-indexer -p locifind-local-index-backend -p locifind-desktop --no-default-features --lib --bins --tests -- -D warnings` 干净通过（顺手删了 2 处 dead_code：旧 wrapper + 占位 fn）
- ✅ `cargo fmt --all` 干净
- ✅ `npx tsc --noEmit`（apps/desktop）干净（0 error）
- 真机验证留 v0.9.5 release 装机后：(a) 加 Downloads 后选项对话框显示「1 自定义 + 0 系统默认」+ Downloads 有「移除」按钮 + 新出现 checkbox「☐ 同时索引系统默认目录」；(b) 勾上 checkbox → 显示「1 自定义 + 3 系统默认」+ Music/Documents/Pictures 追加进列表 + 各带「系统默认」tag；(c) 立即索引 → 状态区实时显示「⏳ 正在索引：📁 C:\Users\Alice\Downloads\某子目录　已扫描 234 · 已入库 187」+ 2 秒轮询滚动 update；(d) 索引完成后状态区回到"上次索引 …"稳态、current_root/fts_progress 清空；(e) 现有搜索 / 排除 / 移除 / 立即索引 零回归。

**未做 / 已登记 follow-up**：
- ① 音乐 Everything 发现阶段无 progress（可接受、秒级完成）；未来若切回 dir-scan 分支可复用同款 progress 桥
- ② phase-label（"扫描音乐（Everything 全盘发现）"）UI hint 保留占位空间，未来若加 3 phase 切换回调可用（cycle 6 v4 已删占位 fn、需要时再加）
- ③ 单目录重扫（`reindex_root(path)` 新 command 走现 reindex 单 root 参数、~2h）——cycle 5 老 backlog
- ④ 打开目录（用 plugin-shell reveal_item_in_dir、~30 分钟）——cycle 5 老 backlog
- ⑤ 移除目录后 purge 该 root 下条目（现在只从 settings.index_roots 删、DB 里旧条目还在等下次 reindex 回收）——cycle 5 老 backlog
- ⑥ embed backend 失败时不 degrade 到其他 3 后端——cycle 4 老 backlog
- ⑦ 会话日志本次归 6 条到 archive-2026-06.md、本 STATUS 剩 10 条

**用户对话流水**：① 用户看 v0.9.5 之前版本截图问「加自定义后默认 3 目录不显示 = bug？+ 索引进度看不到目录 + %」；② 我诊断 = 前者故意 override 语义 UX 坑、后者结构性缺口（IndexStatus 只有 indexing bool）+ 给方案；③ 用户「一起修复」；④ 我起 TaskCreate × 5 → 实施 → cargo/tsc/clippy/fmt 全过 → 用户「收工」；⑤ 归档 + doc-sync + 待 commit。

---

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 5：索引目录概貌 + 分类统计 UI（v0.9.5 一起 tag） ⭐⭐

**承接**：cycle 4 已 commit `6e6d008` 未 tag。用户接着提「选项的索引里面没有列举当前索引的目录，每个目录中索引内容的统计，我希望用户能够直观的看到当前索引内容的概貌，并对索引目录和内容进行管理」。方案敲定：后端一个 `get_index_overview` command 返回每 root 的分类分总（doc/image/music）+ 上次索引；前端「选项 → 索引」pane 顶部加概貌卡片 + 每目录行右侧展开分类统计与时间。发布节奏 = **cycle 4 先 commit 但不发 tag、cycle 5 做完一起 tag v0.9.5**（用户 AskUserQuestion 拍板）。

**关键决策**：① **暂不加 size 字段**（documents/music schema 无 size、加 migration 风险大、留 follow-up）；② SQL 用 **GLOB 前缀 3-OR** 匹配（`path = ?1 OR path GLOB root/* OR path GLOB root\*`）解决 Windows `\` 和 Unix `/` 双分隔符跨 OS 一致性、不依赖 `PRAGMA case_sensitive_like`；③ 图片按 `doc_type IN IMAGE_EXTS` 拆开、`doc_count = doc_stats.total - doc_stats.images`（非图片文档独立列）；④ **前端 fetch 时机**：跟 `effectiveRoots` 同款依赖 `settings.index_roots` + 额外**监听 indexStatus.indexing 从 true 转 false 时重 fetch**（一次全量索引完成后自动刷新概貌）；⑤ 不改 `IndexingPane` 内部现有 `+ 添加目录` / `排除目录` / `立即索引` 结构、只在头部加概貌卡片 + 现有 root 列表行加统计右列——最小改动、零回归风险；⑥ **不 bump 版本**（保持 v0.9.5 = cycle 4 + cycle 5 合并）。

**改动概览（cycle 5 新增改动、7 文件 +346/-17）**：
- [embed.rs](packages/indexer/src/embed.rs) 无改动（cycle 4 已完成、cycle 5 沿用）
- [doc_db.rs](packages/indexer/src/doc_db.rs) +55：新 `DocRootStats` struct（total / images / last_indexed_time）+ `stats_under_root(root)` fn（单一 SQL、GLOB 3-OR 前缀边界、图片按 `doc_type IN IMAGE_EXTS` 拆分）+ 2 单测（前缀边界含 Windows 分隔符 + 空 root）
- [db.rs](packages/indexer/src/db.rs) +50：新 `MusicRootStats` struct（total / last_indexed_time）+ `stats_under_root(root)` fn（同款 GLOB 3-OR）+ 2 单测
- [lib.rs](packages/indexer/src/lib.rs) 2 行：`pub use` 新 stats structs
- [settings.rs](apps/desktop/src-tauri/src/settings.rs) +95：新 `RootIndexOverview` struct（serialize 给前端）+ `get_index_overview(app, index_roots?)` tauri command（复用 `resolve_index_roots` 判 default mode、开一次 DocumentIndex + MusicIndex 复用连接、每 root 2 次 query_row、best-effort 失败返 0 计数、last_indexed 走 max(doc, music) → rfc3339）
- [main.rs](apps/desktop/src-tauri/src/main.rs) 1 行：`invoke_handler` 注册 `settings::get_index_overview`
- [PreferencesDialog.tsx](apps/desktop/src/components/PreferencesDialog.tsx) +120/-17：① `RootIndexOverview` TypeScript interface + `indexOverview` state；② useEffect 跟 `settings.index_roots` fetch + useRef 监听 `indexStatus.indexing` 从 true 转 false 重 fetch；③ 新 `formatIndexTime(iso)` 人性化时间（刚刚 / N 分钟前 / 今天 HH:MM / 昨天 / N 天前 / YYYY-MM-DD）；④ 新 `RootRow` 组件（路径 + 系统默认 tag + 中间统计 `📄 X · 🖼 Y · 🎵 Z` + 上次索引 + 可选移除按钮）；⑤ `IndexingPane` 顶部加 `.prefs-overview-card`（6 单元格：目录数 / 总条数 / 文档 / 图片 / 音乐 / 上次索引）
- [styles.css](apps/desktop/src/styles.css) +76：`.prefs-overview-*` 系列（border + 6 flex cell + border-left 分隔）+ `.prefs-root-stats` / `.prefs-root-time`（monospace tabular-nums、`min-width:0` + `ellipsis` 让长 CJK 路径不撑爆布局）

**接受标准（本机 cargo + tsc 全测过）**：
- ✅ `cargo test -p locifind-indexer --lib` 119 passed 0 failed（cycle 4 的 115 + cycle 5 新 4）
- ✅ `cargo clippy -p locifind-indexer -p locifind-semantic-index -p locifind-desktop --no-default-features --lib --tests -- -D warnings` 干净通过
- ✅ `cargo fmt --all` 干净（fmt 顺手修 settings.rs 一处单行）
- ✅ `cargo check -p locifind-desktop --no-default-features` 干净
- ✅ `npx tsc --noEmit`（apps/desktop）干净（0 error）
- 真机验证留 v0.9.5 release 装机后：(a) 打开「选项 → 索引」看到概貌卡片显示真实计数；(b) 每目录行显示分类分总与"N 分钟前"时间；(c) 加/删目录后 useEffect 触发重 fetch overview；(d) reindex 后 indexStatus.indexing false 时自动刷新；(e) 现有搜索 / 排除规则 / 添加目录 / 立即索引 零回归。

**未做 / 已登记 follow-up**：
- ① size 字段（needs schema migration + upsert 补 size：documents / music 表加 `size_bytes` INTEGER 字段 + 走 std::fs::metadata 索引期拿 size + 加显示。~2h 独立 cycle）
- ② 单目录重扫（`reindex_root(path)` 新 command 走现 reindex 单 root 参数、~2h）
- ③ 打开目录（用 plugin-shell reveal_item_in_dir、~30 分钟、需要给 IndexingPane 加"打开"按钮）
- ④ 移除目录后 purge 该 root 下条目（现在只从 settings.index_roots 删、DB 里旧条目还在等下次 reindex 回收；显式 purge_root(root) 更即时、~1h）
- ⑤ cycle 4 老登记：embed backend 失败时不 degrade 到其他 3 后端（~30 分钟）
- ⑥ 会话日志本次达 15 条、下次收工滚归档 6-7 条

**用户对话流水**：① cycle 4 commit 完询问节奏；② 用户 AskUserQuestion 选"先 commit cycle 4 不发 tag、cycle 5 做完一起 tag v0.9.5"；③ 我探 db schema 发现 size 字段没有、留 follow-up；④ 后端加 stats_under_root（doc + music 各 1 fn + 各 2 单测）+ get_index_overview command；⑤ 前端 IndexingPane 加概貌卡片 + RootRow 组件 + formatIndexTime helper + CSS 76 行；⑥ 全测过（indexer 119/119、clippy 干净、tsc 干净、fmt 干净）；⑦ 等收工提交（v0.9.5 tag 同时触发）。

---

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 4：OCR 乱码 + 图片语义污染防治双层门槛 + bump v0.9.5（待收工提交）⭐⭐

**承接**：v0.9.4 出货后用户在 Windows 真机搜「作文」发现 21-25 号返回 QQ 表情包缓存图 `face-3-efdc54.png` 等，OCR 乱码 body（「动 @ 河的，紉 0 三的的骶也为巴圈 0@ 动 0 @馅 00 邑罊．等寻多子蕙．0 扁」）预览面板展示「最相似段落 · 语义相似度 0.62（强相关）」严重误导——表格文档级 raw cosine 仅 0.16。用户指令：「我想搜索的时候能找到真正贴合意图的内容，应该怎样修复？」

**根因（分析归档）**：BETA-31-v3 cycle 3 加的 `is_embed_worthy` 只挡 `body.trim().chars().count() < 20`，对空/超短 body 有效、对**≥20 字的乱码 OCR** 无效。embeddinggemma-300m 对中文短文本 cosine baseline ≈ 0.4-0.6、乱码段与 query "作文" 都落在"中文均值方向"、段落级挑最像的 top-1 → 0.62 虚高；文档级取全 body 均值 → 0.16 真值。段落级 `EXPLAIN_MIN_SCORE=0.30` 又低到"任何段落都能被展示"。

**关键决策**：**双层门槛 A+B**（讨论中先只想做 A 层 meaningful_ratio 精细化、测试暴露用户实际 case CJK 占比 62.9% > 60% A 层挡不住、扩展加 B 层）：
- **A 层（`is_embed_worthy` v2）**：新加 `MEANINGFUL_CHAR_RATIO_FLOOR=0.6`、body 中 CJK+拉丁字母占非空白 <60% 视为乱码不入嵌入。挡"数字/符号大头"的明显噪声。
- **B 层（图片 doc_type 直接跳过语义索引）**：`embed_pending` 里图片 doc_type（`IMAGE_EXTS`）一律跳过、`explain_semantic_hit_impl` 图片 doc_type 直接返空、`purge_short_body_vectors` SQL join documents 一并清旧图片向量。彻底切掉整类"tesseract OCR 质量不稳"的语义污染。
- **段落级 `EXPLAIN_MIN_SCORE 0.30 → 0.45`** + **`passage_worth_embedding` 段级门槛**（字数 ≥8 且 meaningful ratio ≥60%）双重防线。
- **不动 SearchDeps**（本考虑加 floor_provider 走 preview 传参、评估后判定段落级/文档级两个语义就该分开、min_score 保持段级常量最简）。

**改动概览（5 文件、单次待提交 commit）**：
- [embed.rs](packages/indexer/src/embed.rs) +58/-2：`MEANINGFUL_CHAR_RATIO_FLOOR=0.6` const + `meaningful_char_ratio()` pub fn（CJK Unified Ideographs U+4E00–U+9FFF + Extension A U+3400–U+4DBF + 拉丁字母算「有意义」）+ `is_embed_worthy()` 升级双门槛 + 4 新单测（真中文/英文/中英混排过、数字符号大头不过 + 门槛先后关系）
- [explain.rs](packages/search-backends/semantic-index/src/explain.rs) +34/-6：`EXPLAIN_MIN_SCORE 0.30 → 0.45` + `passage_worth_embedding()` 段级门槛（字数 ≥8 + meaningful ratio ≥60%、比文档级 20 字宽松保留真短句「我有一只猫。」）+ explain_passages 循环内跳过不合格段 + 2 新单测（段级边界 + 噪声段被挡）+ 2 老单测调整（body 扩到段 ≥8 字触发真 embedding 而非门槛短路）
- [doc_db.rs](packages/indexer/src/doc_db.rs) +23/-15：`body_and_doctype_of()` 新 fn（1 次 JOIN 拿 body + doc_type、取代 `body_of`）+ `embed_pending` 图片 doc_type 跳过 + `purge_short_body_vectors` SQL 加 `JOIN documents d ON d.id = v.doc_id` + filter 加图片判定（幂等：老库脏图片向量启动自动清）
- [preview.rs](apps/desktop/src-tauri/src/search/preview.rs) +9/-1：`explain_semantic_hit_impl` 图片 doc_type 兜底返空（防旧图片向量仍在库时 UI 侧展示虚高段落级 cosine）
- [discovery.rs](packages/indexer/src/discovery.rs) + [placeholder.rs](packages/indexer/src/placeholder.rs) +2/-4：顺手修 Rust 1.96 新 lint `map_unwrap_or`（`.map(f).unwrap_or(false)` → `.is_ok_and(f)`、非本 cycle 引入但阻塞 clippy `-D warnings`）
- 版本 bump：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.9.4 → 0.9.5

**接受标准（本机 cargo 可跑）**：
- ✅ `cargo test -p locifind-indexer --lib` 115 passed 0 failed（含新 4 embed 单测 + 老 `purge_short_body_vectors` 单测在新 SQL 下仍通过）
- ✅ `cargo test -p locifind-semantic-index --lib` 16 passed 0 failed（含新 2 explain 单测 + 2 老单测新阈值下调整通过）
- ✅ `cargo clippy -p locifind-indexer -p locifind-semantic-index --lib --tests -- -D warnings` 干净通过（2 顺手 lint fix 后）
- ✅ `cargo check -p locifind-desktop --no-default-features` 干净通过（preview.rs `locifind_indexer::IMAGE_EXTS` resolve 正确）
- ✅ `cargo fmt --all` 干净
- 真机验证留 v0.9.5 release 装机后用户验证：(a) 搜「作文」不再返 QQ 表情包 `face-3-efdc54.png`；(b) 搜「作文」返真正的 `作文整合.docx` / `饶知新组作文已校.rar` / `4_董宇辉-小作文.pdf` 等真作文相关；(c) 图片 OCR 内容仍能通过 FTS 字面命中（例如搜图内文字准确出现的字面词）；(d) 现有所有搜索 / 索引 / 预览 / 历史 / 保存的搜索 零回归。

**未修 / 登记 follow-up**：
- ① embed backend 失败时不 degrade 到其他 3 后端（老 bug、v0.9.5 首选未做、~30 分钟）
- ② `useAppSettings()` hook 抽取 + 删 SettingsPage 旧路由（~1d）
- ③ StatusIndicator/EmbedStatus 类型 + 文案 UX 整体重写（~0.5d）
- ④ **图片 OCR 语义索引可开关 setting**（当前一刀切禁、未来含真文字截图的图片场景需要 opt-in setting `enable_image_ocr_semantic`，~2h）
- ⑤ 会话日志本次达 14 条、下次收工滚归档 5-6 条到 `docs/session-logs/STATUS-archive-2026-06.md`

**用户对话流水**：① 用户报 face-3-efdc54.png 段落级 0.62 强相关不合理；② 我诊断 = 段落级 vs 文档级粒度差 + 中文短文本 embedding baseline 高（三个层面）+ 给三条候选修法；③ 用户问「怎样修复能真正贴合意图」；④ 我给主推荐 = is_embed_worthy v2 + explain 门槛 + 显示 bug + 索引清理 + v0.9.5 落地，~4h；⑤ 用户「现在开工」；⑥ 实施 A 层测试暴露 A 层挡不住用户实际 case（CJK 63% > 60%）、决定扩到 A+B 层双门槛（B 层图片一刀切跳过语义、彻底止血）；⑦ 全测 + fmt + clippy + check 干净、bump v0.9.5、doc-sync；⑧ **等用户「收工」指令再 commit**。

---

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 3 v3 + v4 + 收工前追加 Ctrl+; 换绑 & 顶栏灯口径修 + bump v0.9.3/v0.9.4（v0.9.5 待发） ⭐⭐⭐⭐

**承接**：v0.9.2 hotfix 出货后同会话继续。用户真机验证 v0.9.2/v0.9.3 时先后反馈 3 组问题，本会话全部当场修完：① 匹配方式列 0.16 与预览栏 0.59 数值不一致 → v0.9.3；② 常规 pane「未找到模型」听起来像必需错误 + 预览 vs 结果表 cosine 混淆 → v0.9.4；③ Ctrl+, 打不开 + 顶栏灯口径 → 收工前追加（待 v0.9.5 出货）。

**关键决策**：① v0.9.3 走 **MergedResult 而非 SearchResult** 承载新 `semantic_cosine` 字段（避开 42 个 SearchResult 构造点、只动 3 个 MergedResult 构造点）；② v0.9.4 model_download.rs 重构成 **ModelKind 枚举**（Embedding + Generation 通用下载、独立 cancel/in-flight guard、独立命名空间事件、Embedding 兼容 emit 老无 ns 事件不破 v0.9.3 前端）；③ Ctrl+; 保留 Ctrl+, 老键作 fallback（英文 IME 场景仍能用、双键 both emit `open-prefs`）；④ StatusIndicator 走**双查**（`get_backend_status` + `embedding_model_status`），语义灯颜色专项覆写 backend.is_available 判定，与选项对话框「语义召回」pane 状态源统一；⑤ **不 bump v0.9.5**（用户明确要求积攒多个修复一起出）。

**改动概览（3 commit + 收工后 1 commit 待推）**：
- v0.9.3 [`aaaa314`](https://github.com/raoliaoyuan/LociFind/commit/aaaa314) 8 文件 +85/-14：MergedResult + fuse_rrf/merge_results / SearchResultJson / result_to_json / fanout.rs / SearchView「相似度」列 + COLUMN_PREFS_VERSION 3 迁移 / ranker MergedResult test helper / bump 三处
- v0.9.4 [`5bcd038`](https://github.com/raoliaoyuan/LociFind/commit/5bcd038) 9 文件 +315/-99：model_download.rs 重构 ModelKind + 4 tauri command / main.rs 注册新 2 command / useModelDownload 参数化 kind / ModelDownloadStep 参数化 kind + 每 kind 文案 / PreferencesDialog GeneralPane 嵌入 not_found 下载按钮 + 「启用模型 Fallback（可选）」文案改 / SearchView 预览面板 label 明示段落级 / bump 三处
- 收工前追加 3 文件 +40/-15 待推：MenuBar.tsx 「选项」shortcut Ctrl+, → Ctrl+; + keydown handler 双键兼容 / StatusIndicator.tsx 加 EmbedStatus 双查 + 语义灯颜色专项覆写（ready 绿 / loading 蓝 / not_found 琥珀 / failed 红 / unavailable 灰）+ 轮询 30s → 10s / STATUS + ROADMAP doc-sync
- v0.9.3 + v0.9.4 Release notes 已填 changelog + 手测清单

**真机验证 GO（v0.9.3 装机、用户驱动）**：
- ✅ 结果表「相似度」列显示 raw cosine 0.14-0.16 → 0.30-0.90 数量级（早期用户 5 结果分别 0.16 拥挤 → 装 v0.9.3 后见真相似度）
- ⚠️ 用户设 floor=0.55 时结果里出现「相似度 0.54」的行 → 分析后是**段落级** vs **文档级**粒度差（filter 用 doc-level ≥ 0.55、preview 显示 top passage cosine 可能 < 0.55、不是过滤 bug）→ v0.9.4 label 明示
- ⚠️ 常规 pane「未找到模型」文案吓人 → v0.9.4 加下载按钮 + 「可选」标注

**未修（登记 v0.9.5 及以后 follow-up）**：
- ③ embed backend 失败时不 degrade 到其他 3 后端（老 bug、需 trace fanout 错误传播、~30 分钟）
- ⑤ `useAppSettings()` hook 抽取 + 删 SettingsPage 旧路由（~1d、消除重复 ~120 行）
- ⑥ StatusIndicator/EmbedStatus 类型 + 文案 UX 整体重写（backend detail 字符串前端拼句、~0.5d）
- 会话日志归档下次收工做（本次已达 13 条、超约定上限）

**未尽事宜**：① **v0.9.3 + v0.9.4 CI 已跑完 GO**、[v0.9.3 release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.3) + [v0.9.4 release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.4) NSIS 已上架；② **收工前追加 Ctrl+; + StatusIndicator 待 v0.9.5 出货**（用户明确不 bump 本轮、积攒下轮 embed degrade + 其他多修复一起出）；③ memory 加 [[windows-sogou-ime-esc-workaround]] 前一段已存、本追加 Ctrl+; 是同 pattern 延伸不必再存；④ 首次本地 tauri dev + release build 工具链装机（Node/Rustup/MSVC BuildTools 17.14.35）为本会话最大基建投入、后续 UI 秒级 HMR 迭代能力已就绪。

---

### 2026-07-01 滚动归档 II：6 条移至 [archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md)（cycle 3 v2 hotfix + cycle 3 + 06-30 cycle 6 v3 + 06-30 cycle 1+2 + 06-30 cycle 6 v2 + 06-30 第 7 刀 cycle 6）

---
---

### 2026-07-01 滚动归档：6 条移至 [archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md)

_BETA-31-v3 cycle 1-5（第 1-6 刀）+ 2026-06-29 v0.8.0 UX gap 第 1 刀 共 6 条已滚动归档、逐字保留。_

---


> _2026-06-30 滚动归档（cycle 1 之前 17 条：2026-06-25 ~ 2026-06-29 BETA-15B-8/9/10/11/11-v2 / BETA-31 / BETA-32 / BETA-32 follow-up / 真机 GO / ci.yml 扩范围）+ 2026-06-24 BETA-15B-7 done + 2026-06-24 BETA-15B-6 v3 done + 2026-06-24 BETA-15B-6 v2 done + 2026-06-24 A-5 done + 2026-06-23 A-4 done + 2026-06-23 A-3 done × 2（短/full）+ 2026-06-22 A-2 done + 2026-06-22 收口两分支 / 2 × 2026-06-21 BETA-15B-5+6 / 2026-06-20 BETA-13-G16 / 2026-06-20 G15 / G14 / G14 第 1 刀 / G13 过期 hybrid fallback 修复共已滚动归档 → [docs/session-logs/STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md)。_

---

### 2026-07-02 滚动归档：STATUS 会话日志摘要 1 条（2026-07-02 I）

### 2026-07-02 I — Claude Code (Fable 5) — v0.9.7 真机验证 (a)-(j) + cycle 7-c + confirm 修复 + bump v0.9.8（待 tag）

**产出**：真机验证 9 GO / 1 FAIL（唯一 FAIL = (d) 关闭守卫，根因 wry/WebView2 生产环境 `window.confirm` 是 no-op，已写记忆）；cycle 7-c 三件套（单目录重扫 `reindex_root` + 打开目录复用 `open_path` + 移除目录二次确认可选 purge，`purge_under_root` 落存储层 + `root_glob_predicate` 共用边界 + 级联单测）+ (d) 修复（通用 in-DOM `ConfirmModal`）。indexer 130 / desktop 145 / clippy / fmt / tsc 全过。
**顺带发现（follow-up）**：无单实例锁（两实例并发写库）；「本地索引」全局 vs 概貌 root-scoped 两套口径。
**全文**：本文件上方「2026-07-02 II 整体归档」内。

### 2026-07-02 II — Claude Code (Fable 5) — 定位收敛文档重整 + 会话加载优化

**承接**：同日与用户的价值 / 企业场景 / 护城河讨论，用户拍板"检索底座 + 三企业场景 + 不做分析层（走 MCP+LLM）"。
**关键决策**：Codex 评审（APPROVE with required adjustments）全采纳——Q1 B7 放 B 阶段并行子线不进 §6.3；Q2 V10-15 re-scope 为 Frozen Index Pack（无内置 LLM 合成）而非 dropped、V10-13 并入 BETA-40、V10-16 重定性 MCP 横切护栏；Q4 入口文件只指路不复述定位；Q5 日志两级制收窄为单向（禁止详录反向改 STATUS）；Q6 软 10-12KB / 硬 15KB 收工闸门；Q7 补 collection 模型（并入 BETA-36）、doc identity/去重（并入 BETA-38）、企业评测语料（新卡 BETA-41）、BETA-37 附件+headers+pst 边界。
**产出**：① PROJECT / ROADMAP / README / CONVENTIONS / CLAUDE·AGENTS·GEMINI / STATUS 七类文件重整（会话初始化必读 ~42k → ~12k token、降约 70%），方案与评审全文 [doc-realign-retrieval-foundation.md](../reviews/doc-realign-retrieval-foundation.md)；② **收工闸门自动化**：scripts/hooks/pre-commit（STATUS >15KB 阻止 commit、`LOCIFIND_ALLOW_FAT_STATUS=1` 放行一次；ROADMAP >230KB 预警指向 CLEAN-6）+ `git config core.hooksPath scripts/hooks`（README/CONVENTIONS 已写启用说明）+ 三场景闸门测试全过；③ **Codex CLI headless 评审通道**：`npm i -g @openai/codex`（0.142.5）、认证复用 `~/.codex/auth.json`、`codex exec` 验证过（注意关 stdin：`$null | codex exec ...`、默认 read-only sandbox），CLAUDE.md 备注"headless 优先、GUI 兜底"；④ ROADMAP 膨胀登记 CLEAN-6（~222KB → 目标 <120KB）+ 入口文件过时"80KB"改 220KB。
**未尽事宜**：v0.9.8 tag + 装机验证仍是下一步 top-1；BETA-35 spec 留下会话。

---

### 2026-07-03 滚动归档 IV：STATUS 会话日志摘要 1 条（2026-07-02 VIII）

### 2026-07-02 VIII — Claude Code (Fable 5) — pdftoppm winget 检测 bug 修复 + BETA-41 OCR 端到端全过 ⭐

**承接**：用户 v0.9.10 装机验证报"poppler 装好但检测不到"。**根因**：winget portable 包把安装目录写进**注册表用户 PATH**，运行中的 LociFind 进程环境是启动时继承的旧值，`detect()` 只靠 PATH spawn → 3s 重检永假（cycle 6 "装完 3s 绿标"承诺在 winget 场景不成立）。
**修复**：`pdf_rasterizer.rs` 新 `resolve_pdftoppm()`——PATH 裸名失败后兜底探测已知安装位置（Windows：winget Links + `Packages/oschwartz10612.Poppler_*/…/Library/bin`；macOS：Homebrew 双前缀，GUI app 同病），命中用绝对路径 spawn；`PopplerPdfRasterizer` 存解析后 exe。desktop `check_pdftoppm_available` 委托 detect、零改动受益。本 shell（旧 PATH 无 poppler）实测兜底命中。**装机版 workaround**：重启 LociFind 即绿标；真修随下版分发。
**顺带**：poppler 落盘后 `real_pdf -- --ignored` 端到端全过——BETA-41 fixture 9 份扫描 PDF 12 页 OCR 全成功、0 失败页、关键词断言全命中（装机清单 (d) ✅）。indexer 157 单测 + clippy/fmt 全绿。

### 2026-07-03 滚动归档 III：STATUS 会话日志摘要 1 条（2026-07-02 VI）

### 2026-07-02 VI — Claude Code (Opus 4.7) — v0.9.10 bump + tag（BETA-35 分发）

**产出**：push commit `3a1727c`（BETA-35 全落地）→ origin/main；bump v0.9.10（tauri.conf.json + Cargo.toml）+ tag → 触发 Release Windows workflow。首次 workflow **failed**（Cargo.lock 未同步、CI `--locked` 校验挂）→ `cargo check -p locifind-desktop` 生成 Cargo.lock 新版 → `sync Cargo.lock for v0.9.10 bump` commit `3204f3b` → 删旧远程 tag + 移动 v0.9.10 到 3204f3b → 新 workflow run 28578228626 触发中。release notes 已起草到 scratchpad（新增功能 / pdftoppm 前置 / 装机清单 / 内部改动），workflow 完成后 `gh release edit v0.9.10 --notes-file` 补上。
**未尽事宜**：v0.9.10 装机验证（用户执行）；release notes edit（workflow 完成后）。

### 2026-07-03 滚动归档 II：STATUS 会话日志摘要 1 条（2026-07-02 V）

### 2026-07-02 V — Claude Code (Opus 4.7) — BETA-35 扫描版 PDF OCR 管线全落地（B7 首刀 done ⭐）

**承接**：v0.9.9 tag 已推、装机验证由用户执行中；用户选 B7 首刀 BETA-35。
**关键决策**（spec 期，§8 Q1/Q2/Q3 全采推荐）：① `unsafe_code = forbid` workspace lint 排除 pdfium/mupdf 全 FFI；唯一符合项目 shell-out pattern 且许可宽松（律所可用）= **pdftoppm（poppler，GPL-2/LGPL）**；mupdf / ghostscript 均 AGPL 律所红牌；② 扫描版检测走**整文档二分**（<100 chars 判扫描）；③ 页粒度落 `document_passages` 新表，文本层 PDF 不入新表，**BETA-27 byte-equal 自然成立**；④ 不顺带补 macOS Vision（BETA-03 gap 另开卡）。
**产出**：[spec doc](docs/superpowers/specs/2026-07-02-beta-35-scanned-pdf-ocr-pipeline-design.md)（9 节）+ 6 cycle 全落地：**cycle 1** PdfRasterizer trait + PopplerPdfRasterizer（RAII TempDir）+ 11 单测；**cycle 2** is_scanned_pdf 分支 + 6 单测；**cycle 3** pipeline 整合 + aggregate_page_ocr_results 8 单测；**cycle 4** IncrementalStore::Entry → ExtractedDoc + document_passages/document_failed_pages 三表原子 tx + 外键级联 + 5 CRUD 单测；**cycle 5** LocalIndexBackend 两 API + PreviewPayload scanned_pages/failed_pages + SearchView 命中卡"扫描版 · N 页 · M 段"徽章 + 第 N 页 OCR 段落 + 失败页红色列表；**cycle 6** `check_pdftoppm_available` tauri command + 新 PdftoppmCheckStep 组件（winget/brew 双平台复制 + 3s 自动重检）并入 OnboardingWin 第 2 步与 Everything 并列（步数不膨胀）+ `tests/real_pdf.rs` 集成骨架（LOCIFIND_TEST_PDF env 装机入口）。全程 tsc/vite/clippy `-D warnings`/fmt/156 indexer 单测/149 desktop 单测 全绿；4 条验收全通。**估时 ROADMAP 原 1-2w → 实际 AI 单日完成**。
**未尽事宜**：bump v0.9.10 + tag + 装机验证（下一步 top-1）；BETA-41 fixture 端到端 case；文本层 PDF byte-equal 装机复核。

### 2026-07-03 滚动归档：STATUS 会话日志摘要 1 条（2026-07-02 III）

### 2026-07-02 III — Claude Code (Opus 4.7) — 快速入门 6 步重构 + 概貌 UX 去 emoji（BETA-31 follow-up）

**承接**：用户提出「优化帮助菜单→快速入门，覆盖 Windows 索引 / Everything / 嵌入模型 / 语义理解模型 / 索引目录 / 其他必需初始化」；把 BETA-31 pending 里「Everything 检测失败 GUI 引导」+ 生成模型下载入口 + 索引目录内嵌 + 首次索引一起做进 Onboarding。
**关键决策**（AskUserQuestion 3 问全采推荐）：① Mac 自动跳过 Everything 步（5 步 vs Win 6 步、OnboardingMac/Win 分叉不硬统一）；② 索引目录用内嵌只读列表 + 跳「选项 → 索引」按钮（避免复现 PreferencesDialog 目录管理 UI，`initialCategory` prop 打开时自动切分类）；③ 首次索引不阻塞「完成」按钮（后台继续跑，符合[[tauri-vs-dirs-data-dir-path-mismatch]]等既有原则不引入长阻塞）。
**产出**：① 新 4 组件 [OnboardingShell](apps/desktop/src/components/onboarding/OnboardingShell.tsx) / [EverythingCheckStep](apps/desktop/src/components/onboarding/EverythingCheckStep.tsx) / [IndexRootsStep](apps/desktop/src/components/onboarding/IndexRootsStep.tsx) / [FirstIndexStep](apps/desktop/src/components/onboarding/FirstIndexStep.tsx)；② 改 5 文件（[Onboarding{Win,Mac}](apps/desktop/src/pages) + [menu-events](apps/desktop/src/lib/menu-events.ts) 新 `open-prefs-indexing` action + [App.tsx](apps/desktop/src/App.tsx) 传 initialCategory + [PreferencesDialog](apps/desktop/src/components/PreferencesDialog.tsx) 支持 initialCategory prop）；③ **零后端命令新增**，全部复用 get_backend_status / get_effective_index_roots / reindex / get_index_status / open_windows_indexing_options；④ dev 真机验证反馈两轮微调：全线 padding/字号/行高紧凑化 + shell 加 skipAction 槽（每步都有「跳过此步」）；⑤ 顺带清 PreferencesDialog 索引概貌 3 emoji（📄🖼🎵）与 RootRow 分类计数 emoji（改中文）。tsc + vite build 双绿。
**未尽事宜**：bump v0.9.9 + tag + 装机验证（含 v0.9.8 cycle 7-c 5 点 + 快速入门 6 点）；`check_windows_search_indexed` 仍是 stub、Step 1 无绿标反馈（登记 cycle 9 候选）。

### 2026-07-03 I — Claude Code (Fable 5) — BETA-37 邮件格式提取全落地（B7 能力卡第三张 done ⭐）

**承接**：用户拍板「直接开 BETA-37」；spec 期 4 问 AskUserQuestion 全采推荐（mail-parser crate / msg 后置 BETA-37b / headers 零 schema 变更 / 附件并入 body 不单独成行）。
**产出**：spec + `email_extract.rs` + DOC_EXTS 接线 + `real_eml.rs` 常跑集成 + fixture Subject 前置修复 + licenses/README/ROADMAP 同步。
**关键决策**：附件不单独成 documents 行（防磁盘上不存在的幽灵 path 搅乱增量回收/打开动作）；深度限 1 防嵌套邮件炸弹；`full_encoding` 开历史 charset（企业归档常见 GBK 邮件）。
**踩坑**：PowerShell 双引号串里 `"$var?="` 的 `?` 被贪婪并入变量名（`$var?` 未定义 → 空），须 `${var}` 包裹——BETA-41 eml fixture 的 Subject 因此全空，本次修复。
**未尽事宜**：email/attachment 桶命中率随 enterprise 向量 bootstrap（Mac）出报告；搜索侧 FileType 无 Email 概念（登记候选）。

### 2026-07-04 IX+X — Claude Code (Fable 5) — 两轮收割至 97.7% + 四项口径拍板落地 + BETA-29 v2

**承接**：STATUS 下一步 ④⑤（用户选定三摊活）；第一轮后用户四项口径全按推荐拍板、当场落地。
**第一轮产出**：① 时间簇（`parse_absolute_bounds` 九形态：年月日/年月 之前之后、英文月名、混排、汉英数词月、中英区间；`parse_year` 抢跑顺序 bug 修复；这周/这个月/最近拍/新增/做的；decide_sort created 翻转收窄〔相对时间+创建触发词〕；media 标题先抽再解时间）；② keywords（EN 月份名/序数/数字词/most 停用、报告 sole-keep、又 分隔、预算表 compound、比X还大 size、图片内容子句整尾短语、"the word" 消歧）；③ 标注离群对齐 3 条（各对 5:1+ 锚点多数派）→ 952/48/0。
**第二轮产出（四项拍板）**：复数归一（`singularize_en_keyword` 装配终点做、不进 residual 抽取面〔fallback 遗漏分析复用该面，踩坑后重构〕、minutes/news/series 例外、report sole-keep）；language 降出严格匹配（`compare_json` 跳过，分语言统计不变，v0.5 +11）；clarify question 核实**既定实现**零变更（剩 8 条是 options 结构差异，另立拍板项）；ext-ft 对齐 6 条 + G15 谓词扩展（`in the <kw>`、句首「documents 里」闸门、位置义 pictures 不作 Image）+「几百KB」→<1MB 启发。
**BETA-29 v2**：`SavedSearch.intent` + `save_search` intent 参（`validate_draft_intent` 闸门）+「保存草稿…」（与重跑共用 buildDraft）+ ⚙ chip 走 `search_with_intent`；新命令 `preview_intent`（parser+Refine、无模型、零 tool call）+ ⚙/Shift+Enter 预览入口。
**结果**：**v0.9 = 977/23/0（97.7%）、v0.5 = 490/10/0**；四轮逐 case 对比全程零回归。intent-parser 230（+15）/ evals 全 gate（+1 judge 测）/ desktop 168（+3）全绿；clippy `-D warnings`/fmt/tsc/vite build 净。复盘追记：[gap-inventory §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)。
**未尽事宜**：clarify options 结构口径 8 条（Class B 唯一剩余）；BETA-29 v2 剩 BETA-30 联动项；cycle 9 手测清单已补 v2 七场景。

### 2026-07-04 XI — Claude Code (Fable 5) — 开源免费定位落库（MIT OR Apache-2.0 双许可）

**承接**：用户问"离目标多远"后拍板：不做 Class A 商业分发前置，走开源免费路线，任何人可自由使用代码与软件；许可选定 MIT OR Apache-2.0 双许可（推荐采纳）。
**产出**：① LICENSE-MIT + LICENSE-APACHE 入库、Cargo workspace `license` Proprietary→`MIT OR Apache-2.0`、desktop package.json 补 license；② PROJECT.md 核心原则加「开源免费」+ Beta 路线改开源分发 + 不做什么加「不做商业分发前置」+ IP 计划书标历史记录；③ ROADMAP：§5 五项取消（Apple Developer / 证书 / 域名 / 商标×2 / Notarization 演练）、BETA-00 re-scope 开源发布审查（律师协同取消，2w→2-3d）、BETA-10/10A re-scope GitHub Releases + Gatekeeper/SmartScreen 文档 + Homebrew/winget/Scoop 渠道、V10-08 缩量 / V10-09 dropped、§2/§4/§6.3/§6.4/§7/§8 口径同步、§11 修订记录；④ README 加 License 章节。
**边界坚持**：双平台真机 evals（质量验证）与设计伙伴 P0（护城河）不随商业前置取消；依赖授权扫描全绿（全 MIT/Apache 系、Qwen2.5 Apache-2.0）。
**续（同日 BETA-00 两项余项 done）**：① **Everything 条款核查通过**——零 voidtools 二进制入库、运行期 spawn 用户自装 `es.exe`，不构成再分发；voidtools License 本身 MIT 风格，核查记录入 [third-party-licenses.md](docs/third-party-licenses.md)。② **[PRIVACY.md](PRIVACY.md) 入库**（对照实现：无遥测、唯一联网点 = 用户触发的 HF 模型下载、落盘清单对齐 `CleanupTargets`、清除路径、daemon 形态数据边界披露）+ README 隐私节 + privacy-security.md 改指工程细则。
**续 2（同日脱敏核查 done，BETA-00 整卡收口）**：语料 66 文件全合成核实（README 明示虚构 + 抽查 + 图片无 EXIF）；密钥正则**工作区 + 924 commits 全历史双零命中**；个人用户名 148 处/37 文件等长替换清零（Roger/roger/raoli→alice，代码测试字面量自洽），受改 crate 测试全绿（harness 188 / windows-search 27 / model-runtime 5 / desktop settings 21）；报告 [beta-00-repo-sanitization-2026-07-04.md](docs/reviews/beta-00-repo-sanitization-2026-07-04.md)。
**续 3（同日转公开，用户拍板 orphan 首发）**：私有仓库改名 `LociFind-archive`（冻结保全史含 PR/Release）；新建公开 `LociFind`（canonical URL 不变）以 orphan commit `bc47473` 首发（树=脱敏 HEAD 逐字节一致；公开面 0 tag / 1 commit，gh API 验证）；本地 main 切 orphan 主线 + `full-history` 书签 + `archive` 远端；windows-setup.md 私有认证节改公开口径。
**续 4（同日开源配套 done）**：CONTRIBUTING.md（双许可贡献条款 + 验证闸门 + 范围红线 + 隐私红线）、issue 模板 ×2（bug 含日志脱敏提醒 / feature 含「不做分析层」范围自查）+ Security Advisories 引导 + PR 模板（闸门 checklist + 许可确认）、README 贡献节。
**续 5（同日 BETA-10/10A 文档层 done）**：[install.md](docs/install.md)（SmartScreen 放行 + SHA256 校验 + 升级/卸载、Gatekeeper 三路径〔含 macOS 15 新口径〕+ ad-hoc、源码构建、可选模型）；[渠道评估](docs/reviews/beta-10-distribution-channels-2026-07-04.md)（Scoop 先行→winget 待稳定期→Homebrew tap 待 DMG CI）；README 安装节 + Release body 挂链。两卡转 in_progress，剩 DMG CI + 真机安装验证。
**续 6（同日首个公开发版 done）**：bump v0.9.14（tauri.conf + Cargo.toml + lock `--locked` 验证）、tag 推公开 origin 触发 release-windows.yml（run 28708792924 **success**，NSIS 卸载 hook 首次真实构建通过、安装包 + sha256 出）；`gh release edit` 补 changelog（[草稿](docs/reviews/release-notes-v0.9.14-draft.md)，prerelease）；Scoop bucket **仓库 [scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind) 建成上线**（manifest 填真 hash `4e525b…`、installer/uninstaller 脚本按实测 `%LOCALAPPDATA%\LociFind` 路径、autoupdate；远端 manifest JSON 校验有效）；种子入 [scripts/packaging/scoop/](scripts/packaging/scoop/README.md)。
**未尽事宜**：v0.9.14 真机验证（cycle 9 + BETA-10A + Scoop 装机路径）随下次上机，归下一步 ②。

### 2026-07-06（续）— Claude Code (Opus 4.8) — clarify i18n 双语化 + macOS DMG CI

**承接**：上一 commit 后用户复问「本次会话该做什么」→ 重评纯代码只剩两项 → 用户选「A+B 都做」。
**B（i18n，本机验证）**：clarify options/question 按 language 双语——`pick`/`standard_options(reason, language)`，顶层 4 类就地 `bilingual_options`、vague 5 类走 `pick`；mixed 归中文。eval-neutral（evals 不校验 clarify 文案/options，v0.9 994、v0.5 495 不变）；intent-parser 235→238（+3 i18n 测）、clippy/fmt 净。闭合 beta-exit §6 记的既有缺口。
**A（DMG CI，仅 YAML 校验）**：[release-macos.yml](.github/workflows/release-macos.yml) 镜像 windows 版（macos-14 + aarch64 + 同款守门/features + Gatekeeper releaseBody）。可编依据=daemon workflow 已在 macos-14 编 llama。**风险**：本机无 macOS runner，下次 macOS 发版首验；windows+macos 并行同吃 v* tag、往同一 Release 幂等追加（已注释写明）。
**未尽事宜**：A 待 macOS 发版真机首验（BETA-10 剩真机放行验证）。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。
