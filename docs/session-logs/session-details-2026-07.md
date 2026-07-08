# LociFind 会话详录 — 2026-07

> STATUS.md 只放摘要；本文件按月留改动概览、验证输出、决策细节。最新在顶部。

## 2026-07-08 — Claude Code (Opus 4.8) — 修复本机 MCP 服务 token 持久化分叉

### 承接
2026-07-08 排查 Codex 接 LociFind MCP（memory `codex-locifind-mcp-http-wiring` / `mcp-token-ux-dual-settings-bug`）时出现矛盾态：MCP 服务在跑（`/health` 200、旧 token 401 说明持另一 token），但落盘 `%APPDATA%\ai.locifind.desktop\settings.json` 显示 `mcp_service_enabled:false`/`mcp_service_token:null`——运行态与持久态对不上；`%APPDATA%\LociFind\`（index.db/audit.jsonl/log 所在）里无 settings.json。用户手动 reset token 后两份才一致。原假设：`tauri-vs-dirs-data-dir-path-mismatch` 老坑在 token 上复发（双数据目录）。

### 根因定位（推翻双数据目录假设）
逐链核对当前代码：
- `mcp_service.rs` 的 `start`/`stop`/`reset_token`/自启全部用 `settings::settings_file_path(&app)`；UI 的 `get_settings`/`update_settings` 用 `get_settings_path(&app)`——**两者都 = `app.path().app_config_dir()/settings.json`**（Windows = `ai.locifind.desktop\settings.json`）。**同一文件，无路径分叉**。`LociFind\` 无 settings.json 属正常（它是服务 `attach_readonly` 的只读 data_dir，非配置目录）。
- 真根因是**两个写者对同一 settings.json 的覆盖竞争**：
  1. MCP 开关态/token 由后端**带外**写盘（设计如此，[McpPane.tsx:38](../../apps/desktop/src/components/preferences/McpPane.tsx) 注释「不经 AppSettings 表单」）；
  2. 偏好表单 `update_settings` 是**全量覆写**——前端 `AppSettings` 快照在弹窗挂载时经 `get_settings` 读一次，`McpPane` 之后改的 token/enabled 从不回灌该快照。用户随后保存任意设置 → 旧快照里的 `mcp_service_token:null`/`enabled:false` 把后端刚写的真 token 冲掉；
  3. 此时运行中的 axum server 仍持内存里的旧 token（`start()` bind 时注入 `config.access`）→ 磁盘 token 静默失效（401），外部 client 无感退回 grep/直连库。
- 精确复现观察态：服务在跑并持 token，磁盘却 null/false；reset 重新落盘后两份一致。

### 改动
- [settings.rs](../../apps/desktop/src-tauri/src/settings.rs)：`update_settings` 拆为 `#[tauri::command]` 薄封装 + 路径化内核 `update_settings_at(&Path, AppSettings)`；写盘前读磁盘现值、经 `merge_backend_managed_mcp_fields` 把 `mcp_service_enabled`/`mcp_service_token` 以磁盘为准合并回来（磁盘成 MCP 两字段唯一信源，表单永不动它们；其余字段仍按前端快照全量写入，语义不变）。+2 测试：`update_settings_preserves_backend_managed_mcp_fields`（前端旧快照带 null/false + 改无关字段 → 落盘后 token/enabled 保留、无关字段正常写）、`update_settings_at_writes_when_no_existing_file`（首存无文件不失败）。测试模块补 `#![allow(clippy::field_reassign_with_default)]`。
- [mcp_service.rs](../../apps/desktop/src-tauri/src/mcp_service.rs)：+1 测试 `status_reads_token_from_same_file_persist_writes`——模拟自启的 `persist()` 写 token 后，`status().token` 与磁盘 `settings.mcp_service_token` 一致（分叉守卫，task 第 3 点）。
- doc 注释按 CI pedantic clippy `doc_markdown` 核对：`reset_token`/`McpPane`/`settings.mcp_service_token`/`status.token` 加反引号。

### 为何选后端合并而非「TS interface 加字段 + 表单透传」
后者依赖前端正确 round-trip（易随 pane 重构再漂），前者是持久化边界处的硬保证——无论前端发什么，两字段都不会被冲掉，与设计「MCP 字段后端带外管理」一致。

### 验证限制
本机 host `x86_64-pc-windows-msvc` 但 PATH 无 `cl.exe`/MSVC linker/gcc，llama-cpp sys 依赖无法编译 → 桌面 crate 本地跑不了 `cargo test`/`clippy`（与 memory `ci-ubuntu-first-run-lint-gaps` 记的约束一致）。改动均自包含、类型与借用逐行核对；实际编译/测试/clippy 靠 CI（ubuntu）。未 bump 版本，随下个发版携带。

---

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
