# LociFind 会话详录 — 2026-07

> STATUS.md 只放摘要；本文件按月留改动概览、验证输出、决策细节。最新在顶部。

## 2026-07-08 — Claude Code (Opus 4.8) — BETA-54 数字/编号检索 gap 修复 + Codex↔MCP 接线

### 承接
用户带 Codex 截图（找含电话号码 150138 的文件，Codex 一路 curl `127.0.0.1:8766` → 精确搜 0 命中 → 转而直接 `sqlite3` 打桌面 index.db 的 `documents_fts` LIKE 命中）问「Codex 感觉绕过了 MCP 直连索引库，是不是有问题」。

### 诊断链（三步定性）
1. **纠错自己的第一判断**：初答误称「FTS 按整词匹配、数字前缀匹不上」；读 `packages/indexer/src/doc_db.rs:36` / `db.rs:8-10` 实锤 FTS 用 **trigram** tokenizer（≥3 字符任意子串可匹配，含数字），前判作废。
2. **audit 铁证机制**：`packages/locifind-server/src/audit.rs` → daemon 每成功 `search` 往 `<data_dir>/audit.jsonl` 追加一行（ts/subject/action:search/collections/query/results）；直连 sqlite 不留痕 → 成为「走没走 MCP」的判据。
3. **实锤 Codex 从没连上 MCP**：读 `~/.codex/config.toml` + `.codex-global-state.json`——只有 `node_repl` 一个 mcp_server，用户此前贴的 Claude 风格 `{"mcpServers":{...}}` JSON **格式不通**（Codex 只认 TOML `[mcp_servers.name]`），从没落进配置。所有「绕行」= agent 无工具时的自救，非 Codex 乱来。

### Codex↔MCP 接线修复
- `codex-cli 0.142.5` 原生支持 streamable HTTP MCP。`setx LOCIFIND_MCP_TOKEN <token>` + `codex mcp add locifind-local --url http://127.0.0.1:8766/mcp --bearer-token-env-var LOCIFIND_MCP_TOKEN`（token 走环境变量不落明文进 config）。
- 端到端验证：`/health` 200；MCP `initialize`/`tools/list`（返 `search`/`list_collections`/`read_document`）/`tools/call search 仙本那`=3 命中带 snippet + `audit.jsonl` 新增 search 行。中文 query 经 Git Bash curl 会编码乱码，改 Python urllib 发。
- **踩坑**：① 服务是 `locifind-desktop.exe` 进程内托管（非独立 locifindd），app 关即 `/health` 000；② token 轮换——旧 `1901d35a…` 401；③ 面板 token 只在启用瞬间弹一次、之后不可复看，且「重置令牌」按钮**文案有控件缺**；④ 双 settings 路径（`Roaming\ai.locifind.desktop\settings.json` 显示 enabled:false/token:null 而服务在跑）——Tauri vs dirs 老坑。最终用户 reset 取新 token `0018e452…` 打通。

### BETA-54 检索层 bug（真机复现 + 修 + 验）
- **复现**（经真 MCP，audit 留痕）：`search 150138`/`440307`/`15013866763`/`440307201312314812`→0；`准考证`→1（命中同一含号码 png）；`仙本那`→3。证明文件在索引里、号码搜不出 = 检索层 gap。
- **根因**：`packages/intent-parser/src/parsers/file_search.rs` `extract_en_residual_keywords` 的 `is_signal` 判据含 `|| tok.chars().all(|c| c.is_ascii_digit())`——纯数字 token 一律当噪声 flush 丢弃（本意剥年份/尺寸/日号），电话/案号/身份证号连带遭殃 → keywords=None → daemon `expand_intent_for_daemon` 无 group → FTS 臂无检索词 → 0。诊断佐证：`电话 15013866763`→0（电话不在文档、号码没起作用）、`准考证 150138`→1（靠中文词命中）。
- **修法**：新增 `IDENTIFIER_DIGIT_MIN=6` + `is_incidental_number(tok)`（纯数字**且 <6 位**才算噪声；≥6 位视为标识符保留为字面 keyword），替换该判据。desktop `search.rs:474` 与 daemon `search.rs` 共用 `intent_parser::parse`，**一改两受益**。
- **测试**：intent-parser `cargo test` 242 全绿；新增 4 项（`incidental_number_threshold` / `long_digit_run_kept_as_keyword`〔含 `invoice 15013866763` 合成短语，daemon 再按空格拆〕/ `short_number_still_stripped`〔2024/100 仍 None，守 date-size 零回归〕/ `parse_bare_number_keeps_number_keyword`）。
- **真机 MCP 验证**：`scripts\build-locifindd-llama.bat` 重编 locifindd（1m35s，重编 intent-parser+server+daemon）→ 挂含号码 .txt 语料到 `:8788`（token 全 1、model 复用 embeddinggemma-300m）→ 经 HTTP MCP 实搜：`150138`/`440307`/`15013866763`/`440307201312314812` **0→1 命中**、`2024` 仍 **0**（阈值有效）、`仙本那` 对照命中。A/B 对照旧 8766 desktop MCP（`150138`/`440307`=0）成立。停临时 daemon，8766 未动。

### 生效边界与后续
- 修复在 `intent-parser`，桌面 app（含其进程内 8766 MCP）要 pick 上须**出带本改动的新版本**；当前跑的 8766 仍旧码——用户重启 Codex 后 `仙本那`/普通词能走 MCP，`150138` 类仍需新桌面版。
- 附带两 token bug 派后台会话：`task_7260d343`（token UX：常驻查看/复制 + 补「重置令牌」按钮）、`task_06f499be`（双 settings 路径分叉）——在各自 worktree 处理，本会话不碰 mcp_service/settings/UI。
- ROADMAP 登 BETA-54（done 代码层，依赖 BETA-50/53）。

### 结果
仅改 `packages/intent-parser/src/parsers/file_search.rs`（+71/-1，含 4 测试）。intent-parser 242 pass。真机 MCP A/B 验证达成。

## 2026-07-06（续 3）— Claude Code (Fable 5) — v0.9.16/17 双发版 + 真机反馈二轮修复

### 承接
BETA-45/46 落地后用户拍板「push 发 v0.9.16」→ 装机实测回报下载卡死链 + 布局问题 → 逐条修复（用户选 b：攒批再发）→ 用户拍板「发版」→ v0.9.17。

### v0.9.16 发版（macOS 首跑踩坑 + 修复）
- bump c9828b0 → tag 触发并发；**Windows success、macOS failure（E0433）**：everything crate 是 Windows target-gated 依赖，model_download.rs 模型发现代码无条件引用——加 `es_cli_available`/`es_find_files_named` 平台 shim（4160b60）。
- 修复经 `gh workflow run release-macos.yml -f tag=v0.9.16` **dispatch 重跑 success**（DMG 追加至同一 Release，无须重打 tag）；changelog 补全（含升级注意：空 roots 老装机须重新勾选系统默认）。
- 教训：跨平台代码引用 target-gated 依赖必须 cfg gate；双平台 CI 第二跑就抓住了 Windows 本机永远编不出的错误。

### 真机反馈二轮（v0.9.16 装机实测，三项修复入 v0.9.17）
1. **下载卡死链**（9c26f4c）：根因链 = HF 连接阶段长挂占满 in-flight 守卫 300s（旧配置仅整请求 timeout）→ cancel flag 只在 chunk loop 检查、连接阶段取消无效 → 前端切步重挂回 idle 与后端守卫脱节（无取消入口）→「使用此文件」/「重试」全被守卫弹回。修复：`download_model_impl` 每源 `tokio::select` 与 `wait_cancelled`（300ms 轮询）竞速（drop 下载 future + 清 partial）；`connect_timeout(15s)`；**URL 链主源→hf-mirror.com 镜像兜底**（取消不切换；PRIVACY.md 联网点同步）；新命令 `model_download_in_flight`（useModelDownload mount 时查询恢复「下载中+取消」态）；import 守卫文案引导先取消。测试 +2（wait_cancelled 置位即返 / urls 链同路径）。
2. **取消误报「下载失败」**（2db77bd）：事件路径已过滤「用户取消下载」，但 `start()` 的 invoke-catch 路径没过滤——取消后命令 Err 拒绝覆盖 idle 为 error。catch 同样过滤该 reason。
3. **目录三行卡片布局**（0121063）：上轮「换行显示完整路径」在单行 flex 下被统计列+按钮挤成逐字断行（真机截图实锤）——RootRow 改纵向三行（完整路径+标签 / 统计+上次索引 / 操作按钮），卡片下边框分隔；「上次索引」加文字前缀。

### v0.9.17 发版
bump a692a10 → tag → **双平台一次 success**（并发机制三连稳：15 首验 / 16 修复重跑 / 17 一次过）；changelog 补全（[草稿](../reviews/release-notes-v0.9.17-draft.md)）。

### 验证
desktop 170 全过（540s 全量）· tsc / vite build 净 · clippy `-D warnings` / fmt 净（各修复轮均过）。

### 未尽事宜
- v0.9.17 待用户真机验证：升级零数据损失 / 下载取消即刻生效 / 切步恢复下载态 / 镜像兜底 / 三行布局 / 卸载保模型弹窗 / 零索引空态 / clarify 英文追问。
- `gh run watch` 假退出 ×3 实锤——一律用 `gh run view --json status` 轮询（流程备忘）。

## 2026-07-06（续 2）— Claude Code (Fable 5) — v0.9.15 并发发版 + cycle 9 首批真机反馈落地（BETA-45/46）

### 承接
用户拍板 push + 并发发版 → 上机测试 v0.9.15 → 回报三条真机反馈（模型重下 / 默认索引系统目录 / 选项页重构）→ 三项拍板（卸载默认保模型可勾选删 / 发现后**复制**进默认目录 / ①②当场做、③下会话）→ 当场实现。

### 发版（v0.9.15，并发首验双通过）
- bump 0.9.14→0.9.15（tauri.conf + src-tauri/Cargo.toml + Cargo.lock `--locked` 验证）→ tag 推送触发 **windows + macos 两 workflow 并发**。
- **Release macOS 首验 success**（`LociFind_0.9.15_aarch64.dmg` + app.tar.gz 产出）——上一 commit 的"未验证"风险闭合；**Release Windows success**（`LociFind_0.9.15_x64-setup.exe`）。
- 并发追加机制验证成立：先完成者建 Release、后完成者幂等追加资产到同一 tag。changelog 以 [release-notes-v0.9.15-draft](../reviews/release-notes-v0.9.15-draft.md) `gh release edit` 补全。
- 踩坑：`gh run watch` 两次假退出（exit 0/1 但 run 仍 in_progress）——改用 `gh run view --json status` 轮询才可靠。

### 真机反馈诊断
- **反馈①根因**：BETA-12 卸载 hook 非升级卸载 `RMDir /r $APPDATA\LociFind` 整目录删除、models 在内——用户"卸载旧版→装新版"即触发 ~700MB 重下（设计张力：模型是公开权重非敏感数据）。
- **反馈②根因**：`resolve_index_roots_tagged` 在 `index_roots` 为空时兜底返回系统三夹（cycle 6 v4 旧语义）= 开箱即索引用户目录。
- 代码面经 Explore 子代理全图梳理（onboarding 6 步 / PreferencesDialog 4 tab 1579 行 / 模型状态机 / Everything 集成点）。

### BETA-45 落地（模型本地发现 + 卸载保模型）
- **NSIS hook**（[uninstall-hooks.nsh](../../apps/desktop/src-tauri/nsis/uninstall-hooks.nsh)）：MessageBox 询问删模型否、默认/静默 `/SD IDNO` = 保留；保留经 models 同卷 Rename 暂存→整删→移回（敏感派生数据零遗漏、新增子项自动纳入删除面）；$UpdateMode 守卫不变；[uninstall.rs](../../apps/desktop/src-tauri/src/uninstall.rs) 闸门测试 +2 断言（Rename models / `/SD IDNO`）。应用内清理仍全删（§6.3 指标不动）。
- **everything crate**（[lib.rs](../../packages/search-backends/everything/src/lib.rs)）：新公开 `find_files_named(filename, limit)`（`wfn:` 精确整名 + 复用 es 两段式定位与 `-export-txt -utf8-bom` 解码）+ `es_cli_available()`；非 Windows 恒空/false。
- **desktop 命令**（[model_download.rs](../../apps/desktop/src-tauri/src/model_download.rs)）：`discover_local_model`（默认路径 ≥100MB → present；否则按 kind 白名单文件名发现候选，embedding 双名〔canonical + HF `-qat` 原名〕、generation 单名）+ `import_local_model`（白名单校验 + ≥100MB + copy→`.partial`→rename 原子落盘 + 复用下载 in-flight 守卫与 done event）。main.rs 注册。
- **前端**（[ModelDownloadStep.tsx](../../apps/desktop/src/components/ModelDownloadStep.tsx)）：mount 时 discover——present 直接就绪进下一步；候选列表「使用此文件（复制）」；Everything 未装提示手动放置。import 走既有 done event、状态机零改动。

### BETA-46 落地（默认零索引 + 目录 UX）
- [settings.rs](../../apps/desktop/src-tauri/src/settings.rs)：`resolve_index_roots_tagged` 新语义——三夹**仅当 include_system_defaults=true** 纳入、与 raw 空否解耦（空+false = 零索引）；测试改四分支（空+false 断言空）。
- [PreferencesDialog.tsx](../../apps/desktop/src/components/PreferencesDialog.tsx)：checkbox 常显、覆盖语义 banner 退役（CSS 同删）、空态改「默认不索引、请添加或勾选」、路径 title hover。
- [styles.css](../../apps/desktop/src/styles.css)：`.prefs-root-path` 退役 ellipsis 单行截断 → `overflow-wrap: anywhere` 完整显示。
- [IndexRootsStep.tsx](../../apps/desktop/src/components/onboarding/IndexRootsStep.tsx)：文案改新语义（顺带修错写的「桌面、文档、下载」→ 实际三夹是音乐/文档/图片）；`usingDefaults` 与自定义解耦。
- **行为变化**：旧装机 index_roots 空者升级后停止索引三夹（须重新勾选）——beta 接受、卡内注记。

### 验证
desktop 168 全过（全量 365s）· everything 15 · settings 四分支/uninstall 闸门/model_download 全绿 · tsc / vite build 净 · clippy `-D warnings` / fmt 净。

### 未尽事宜
- BETA-45/46 待随下次发版真机验证（NSIS 弹窗须真装真卸）；BETA-47 选项页重构下会话（含 `enable_everything` 开关等）。
- v0.9.15 其余 cycle 9 场景测试继续中（用户侧）。

## 2026-07-06（续）— Claude Code (Opus 4.8) — clarify i18n 双语化 + macOS DMG CI

### 承接
上一 commit（439b120，已 push）后用户复问「本次会话该做什么」→ 我重评：纯代码可推的只剩两项，用户选「A+B 都做」。

### B：clarify options/question i18n 双语化（本机完整验证）
- **动机**：上轮方案 A 让所有非 Unknown clarify 挂了中文 options，英文/mixed query 用户看到中文 options+question（beta-exit §6 记为既有 i18n 缺口）。
- **落地**：
  - [clarify.rs](../../packages/intent-parser/src/parsers/clarify.rs)：新增 `pick(language, zh, en)` + `pick_opts`；`standard_options` 加 `language` 参，5 类 reason 各配中/英 options（En→英文、zh/mixed/unknown→中文）。
  - [lib.rs](../../packages/intent-parser/src/lib.rs)：顶层 4 类高优先级 clarify（unsafe/recent-time/bulk-action/unknown-location）问题+options 就地双语构造（`bilingual_options`，因其 options 语义异于通用 `standard_options`，如 bulk 给「确认全部/只选择部分」）；`detect_vague_clarify` 5 分支问题走 `pick`。
  - mixed 归中文（CJK 主导）；`bare_relative_time_only` 仅中文输入触发、不涉 En。
- **eval-neutral**：evals 不校验 clarify 文案/options 内容（`is_clarify_question_equal` 恒真、`is_clarify_options_equal` 只查结构），故 v0.9 994/6/0、v0.5 495/5/0 完全不变（已验证）。
- 单测 +3（[`tests_clarify_i18n`](../../packages/intent-parser/src/lib.rs)：en 出英文 options+ascii question / zh 保中文 / Unknown 双语均 None）。intent-parser 235→238；28 suite 全 0 failed；clippy `-D warnings`/fmt 净。

### A：macOS DMG CI（本机仅 YAML 校验）
- 新建 [release-macos.yml](../../.github/workflows/release-macos.yml)，镜像 [release-windows.yml](../../.github/workflows/release-windows.yml)：
  - `macos-14`（Apple Silicon）+ `targets: aarch64-apple-darwin`；同款 `cargo metadata --locked` 守门 + tauri-action + `--features model-fallback,semantic-recall -- --locked`。
  - 平台差异：Gatekeeper 放行 releaseBody（右键打开 / `xattr -dr com.apple.quarantine`，替 SmartScreen）；模型路径 `~/Library/Application Support/LociFind/models/`；Intel Mac 走源码构建（mirror daemon 抉择，避 macos-13 排队）。
- **可编依据**：release-daemon.yml 已在 macos-14 成功编 locifindd（走 llama-cpp-4 同款 path dep），故桌面 app 带同款 features 可编。
- **两点风险（如实登记）**：① 本机无 macOS runner，只有推 v* tag 才实跑 → **下次 macOS 发版首验**（Metal 编译/DMG 打包/tauri-action 细节可能暴露问题）；② windows+macos 两 workflow 现并行同吃 v* tag、往同一 Release 幂等追加各自安装包（tauri-action 对已存在 Release 幂等），最终 changelog 仍靠 `gh release edit` 统一覆盖。已在 workflow 注释写明。
- YAML 解析有效、结构与 windows 对齐（7 步、同 v* 触发）。

### 未尽事宜
- A 待下次 macOS 发版真机首验；BETA-10 剩「真机放行验证」（§6.3 指标）。
- clarify i18n 已闭合缺口；`bare_relative_time_only` 的中文 label 仅中文路径触发、无需 en 化。

## 2026-07-06 — Claude Code (Opus 4.8 / Fable 5) — 出场报告骨架 + clarify options 方案 A + 老账收割

### 承接
用户问「本次会话该做什么」→ 先读三份共享文档 + 定向读 ROADMAP §2/§6.3/§8 全局 → 判定：代码/质量线已达标，挡在出场前的全是真机/对外条件。用户选「先看 ROADMAP 全局再定」→ 按建议执行「① 出场报告骨架 ② clarify options 分析」→ 拍板方案 A → 就地实现 → 继续推进老账收割。

### 产出 1：BETA-14 出场报告骨架
- 新建 [docs/reviews/beta-exit.md](../reviews/beta-exit.md)，按 [ROADMAP §9](../../ROADMAP.md#9-出场报告模板) 模板：已知的 parser-only 数据（准确率/回归/分桶/文档层就绪）全部落死，真机相关格（子集命中率、索引资源占用、安装可用性、性能 p95）统一标 `TODO(真机)`。下次上机照此清单批量填格即可定稿。

### 产出 2：clarify options 结构口径（Class B 唯一剩余项 → 清零）
- 决策备忘 [docs/reviews/beta-14-clarify-options-decision-2026-07-06.md](../reviews/beta-14-clarify-options-decision-2026-07-06.md)。
- **关键机制**：evals `is_clarify_options_equal` 只校验结构存在性（都是 Array 或都是 null），内容/长度/顺序全不看。故 8 条 partial 纯粹是「一边有数组、一边 null」的结构错配；d8 标注自身还内部不一致（同为 ambiguous_type/action，仅 004/007 带 options）。
- **拍板方案 A**：按 reason 定「带不带 options」——凡有可枚举收窄维度的 reason 一律带标准 options（一键收窄 UX），唯 `Unknown` 不带。
- **落地**：
  - parser（[clarify.rs](../../packages/intent-parser/src/parsers/clarify.rs)）：新增 `standard_options(reason)`，`clarify_with` 按 reason 自动挂（Unknown→None）；顶层 4 类直接构造的 clarify 已带 context-specific options 不动。
  - 标注（[d6.json](../../packages/evals/fixtures/v0.9/_authoring/d6.json)/[d8.json](../../packages/evals/fixtures/v0.9/_authoring/d8.json)）：脚本批量给 17 条非 Unknown clarify 补 options（d6 危险动作 4 + d8 非 Unknown 13），Unknown 4 条保持 null；重跑 assemble-coverage + generate-evals-v09。
  - 零回归确认：v0.5 全 40 条 clarify 锚点都带 options 数组、reason∈{time,unsafe,location,action}、无 ambiguous_type/unknown，由顶层触发器服务，不受影响。
- **结果**：v0.9 977/23/0 → 985/15/0（97.7%→98.5%），Clarify 桶 67/0/0；v0.5 490/10/0 零回归。

### 产出 3：老账收割（9 条转正，6 组修复）
| 修复 | 文件 | 说明 |
|---|---|---|
| `songs by` 小写连字符 artist | artist.rs | RE_EN_BY 加 `[a-z0-9_]+(-[a-z0-9_]+)+` 分支（须含连字符，裸小写词 size/name 不命中）；synthetic-artist ×4 |
| 碳中和 compound 保全 | file_search.rs | `ZH_HE_COMPOUNDS`+私用区占位符：切段前把词内「和」换占位符、切后还原；真并列「找合同和报告」不受影响 |
| d3 ft 标注对齐 ×2 | d3.json | zh-030 补 document（对齐 pdf 5 锚点）、zh-040 删 document（对齐裸「的文件」34 锚点） |
| 裸 no + 字面扩展名 | file_search.rs | `bare_no_literal_extensions` 窄路径（no 不入通用否定标记，只认 no+紧邻单 token 且 token 是字面扩展名）；v0.5 零 `no <word>` 形态 |
| music 目录 mixed hint | lexicon.rs + common.rs | keywords 加「music 目录」；`alias_name_part_is_ascii`（剥中文容器尾词后名字部分纯 ascii→en_hint）；纯 ascii/纯中文行为不变 |
| 几个G 抽象 size | media_search.rs | `has_size_desc_sort_word` 加「几个 g/m、几 g/m」（镜像 has_size_sort_signal，26 锚点全 size_desc） |

### 验证（全绿）
- intent-parser 230→235 测（+5 新测，每修复带正反守护）；evals/harness/server 全 gate；28 suite 全 0 failed。
- v0.9：**994/6/0 = 99.4%**（en/mixed/zh 各 99.4%）；v0.5：**495/5/0 = 99.0%**；双集 fail=0，逐 case 零回归。
- clippy `-D warnings` 净、fmt 净（fmt 修了 2 处 let-else 换行）。
- 剩 6 partial：5 条 v0.5 标注锁定（markdown ft / 「上个月下载的」动词歧义 / 项目归档 location / downloads hint 双语 ×2，改标注吃 §6.5 豁免额度）+ 1 条备份文件两难（「备份文件」整词 vs「备份」，与「临时文件」惯例互斥）。

### 环境备注
- 本机（Roger）cargo 1.96.1 + msvc 工具链可正常 build/link/test/clippy——[memory 里「Windows 无 MSVC linker」](../../../../Users/Roger/.claude/projects/D--Git-Locifind/memory/ci-ubuntu-first-run-lint-gaps.md) 那条是 Alice 机器的，不适用本机。
- Python 文本模式写 JSON 会引入 CRLF（仓库 .gitattributes 是 LF），改标注后须 `open(...,newline='\n')` 或二进制 replace `\r\n`→`\n`。

### 未尽事宜
- clarify options 方案 A 的 en query 返回中文 options 是既有 i18n 缺口（独立小卡）。
- 剩 6 partial 的 v0.5 标注锁定项攒批处理（§6.5 豁免额度，累计仍 0 用）。
