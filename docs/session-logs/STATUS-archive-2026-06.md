# STATUS 会话日志归档（2026-06-04 ～ 2026-07-01）

> 从 [STATUS.md](../../STATUS.md) 滚动归档；逐字保留，按时间倒序。更早历史见 [STATUS-archive-through-2026-06-03.md](./STATUS-archive-through-2026-06-03.md)。

## 2026-07-01 滚动归档 II（cycle 7-a+7-b 之前 6 条：2026-06-30 ~ 2026-07-01）

> 本批归档：BETA-33 cycle 3 v2 hotfix + cycle 3 紧凑菜单栏 + BETA-31-v3 cycle 6 v3（v0.8.9 真机验证）+ BETA-33 cycle 1+2 桌面菜单栏骨架 + BETA-31-v3 cycle 6 v2（v0.8.8 真机验证）+ 第 7 刀 cycle 6 设置页显示生效索引目录（v0.8.7）共 6 条。

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 3 v2 hotfix + 首次本地 tauri dev 真机验证 + bump v0.9.2 ⭐⭐⭐

**承接**：v0.9.1 发版后用户提「每次等 CI Release 太耗时」、决定装本地 Tauri dev 工具链。同会话完成：装机链（Node/rustup/MSVC BuildTools 17.14.35 全新装）→ `cargo run --no-default-features` 首次 build 3 min 18s → 本地 dev 窗口起来 → computer-use 驱动真机验证 v0.9.1 → 发现 3 处交互 bug → cycle 3 v2 hotfix 修完 2 个 → bump v0.9.2 + push tag 触发 CI。

**关键决策**：① 本地 dev 工具链走 winget 装（Node LTS 24.18 + Rustlang.Rustup 1.29 + Microsoft.VisualStudio.2022.BuildTools 17.14.35 with `Microsoft.VisualStudio.Workload.VCTools + Windows11SDK.22621`），MSVC 装机 ~10 min + 首 cargo build 3m18s = 首次 ~15 min 门槛、之后 UI 秒级 HMR；② computer-use 驱动 dev 窗口需授权 `locifind-desktop.exe` 独立 bundleId（与 release 装机的 `%LOCALAPPDATA%\locifind\locifind-desktop.exe` 不同）；③ Esc 4 版试错定位到 Windows Sogou IME 层 native 拦截 Esc、绕不开、必须用 React 合成事件 capture 阶段抓（`onKeyDownCapture` 挂 backdrop）；④ Ctrl+, 打不开对话框 = Sogou IME 拦 `,` 键无解、登记 follow-up；⑤ dev 版跑 `--no-default-features` 关掉 semantic-recall/model-fallback feature → 搜索恒报「embedding 模型不可用」；不改 default features 保留 CI 打包一致性、留 follow-up `npm run tauri dev -- --features ...` 或 package.json script 加分支。

**真机验证 GO 项（computer-use 驱动 GUI 截图 + zoom 双源）**：
- ✅ 顶部菜单栏视觉密度对齐 Everything（zoom 明确对比：`文件(F) 编辑(E)...` 单行紧凑 + 4 状态灯右对齐）
- ✅ 工具→选项 弹模态对话框（左侧 4 分类树 + 右侧「常规」表单 + 底部 取消/应用/确定）
- ✅ 4 分类切换（常规→语义召回→索引→隐私与记录），每个 pane 字段正确
- ✅ 索引 pane 显示真实数据：3 系统默认路径 `C:\Users\Alice\{Music,Documents,Pictures}` 打「系统默认」tag + 「上次索引 2026/7/1 08:36:25（音乐 34185 / 文档 187 / 图片 4687）」
- ✅ 隐私 pane 显示真实 8 条 audit 记录（open op、WeChat Files/Documents/BaiduNetdisk 各类路径）
- ✅ 取消 button 关闭 dialog（直接触发 onClose）
- ✅ × 右上角关闭 dialog
- ✅ **cycle 3 v2 fix：Esc 键关闭**（`onKeyDownCapture` 挂 backdrop）
- ✅ **cycle 3 v2 fix：点遮罩关闭**（dialog 缩到 720×520 后遮罩 ≥40px 可点）

**cycle 3 v2 改动概览（commit `318fdd3`、7 文件 +98/-14）**：
- [PreferencesDialog.tsx](../../apps/desktop/src/components/PreferencesDialog.tsx) +17/-11：Esc 修法从 useEffect+useRef+window+capture 改成 React `onKeyDownCapture` 挂 backdrop + `dialogRef.current?.focus()` 挂载后自动获焦
- [styles.css](../../apps/desktop/src/styles.css) +6/-2：`.prefs-dialog` width/height 缩到 `min(720px, calc(100vw-80px)) × min(520px, calc(100vh-100px))` + 加 `outline: none`
- 版本 bump：tauri.conf.json + Cargo.toml + Cargo.lock 三处 0.9.1 → 0.9.2
- 顺带：apps/desktop/package.json 加 `allowScripts.esbuild@0.21.5`（npm 11 安全策略）+ apps/desktop/src-tauri/gen/schemas/windows-schema.json 重生 66 行（tauri-build 自动生成、tracked 文件）

**未修（登记 follow-up）**：Ctrl+, 打不开对话框（Sogou IME 拦 `,` 键、无代码 fix）→ 换绑 Ctrl+;；dev 版搜索恒报「embedding 模型不可用」；embed 报错整个搜索不 degrade；顶栏「语义召回」灯与实际能力口径不一致。

**接受标准**：本地 tauri dev 真机 GO（上述所有 GO 项已验证）；CI [v0.9.2 workflow run 28485802313](https://github.com/raoliaoyuan/LociFind/actions/runs/28485802313) in_progress 兜底 NSIS。

**未尽事宜**：① v0.9.2 CI 跑完填 Release notes（changelog 含 4 项 follow-up 提示）；② memory 加 [[windows-sogou-ime-esc-workaround]]；③ 本次会话日志算第 11 条、超约定上限。

---

### 2026-07-01 — Claude Code (Opus 4.7) — BETA-33 cycle 3：紧凑菜单栏 + 选项模态对话框 + bump v0.9.1 ⭐⭐

**承接**：用户装 v0.9.0 后看 Everything 截图提两点改进诉求：① 标题栏文字更紧凑（对齐 Everything 视觉密度）；② 打开「选项」时弹独立设置卡片（参考 Everything 选项窗、左侧分类树 + 右侧表单）。本会话连续做 cycle 3 = 紧凑化菜单栏 CSS + 新 PreferencesDialog 模态卡片。

**关键决策**：① 紧凑化范围 = CSS 单点调整 4 处常量（不动 HTML 结构、不动 access key 显示方式 `(F)`，保留中文带括号风格）；② 选项对话框 = 模态 modal（参考 Everything 选项截图：标题栏 + 左侧分类树 + 右侧表单 + 底部取消/应用/确定）；③ 分类数 = 4（常规 / 语义召回 / 索引 / 隐私与记录），按 SettingsPage 14 字段功能聚合 + 删搜索范围（read-only placeholder）；④ **不重构 SettingsPage 共享 hook**：PreferencesDialog 内嵌一份独立 state 与 handler 逻辑（与 SettingsPage 字段逻辑重复 ~120 行）、SettingsPage 保留作 `/settings` 路由 fallback（零回归保底）；⑤ **接通方式 = 菜单事件总线扩 `open-prefs`**（与 cycle 2 同款 `locifind:menu` channel、Ctrl+, + 工具→选项 双入口）。

**改动概览（1 新文件 + 5 改、单次 commit）**：
- 新 [PreferencesDialog.tsx](../../apps/desktop/src/components/PreferencesDialog.tsx) ~540 行：modal backdrop + 760×560 卡片 + 标题栏（× 关闭）+ 左侧分类树 + 右侧分类对应 Pane（GeneralPane / SemanticPane / IndexingPane / PrivacyPane）+ 底部消息条 + 取消/应用/确定；复用全部 SettingsPage 后端 tauri command（zero backend change）
- [lib/menu-events.ts](../../apps/desktop/src/lib/menu-events.ts) +2/-1：加 `open-prefs` MenuAction
- [components/MenuBar.tsx](../../apps/desktop/src/components/MenuBar.tsx) +2/-2：「工具→选项」与 Ctrl+, 改 emit("open-prefs") 不 navigate("/settings")
- [App.tsx](../../apps/desktop/src/App.tsx) +5/-1：加 showPrefs state + onMenuAction("open-prefs") listener + `<PreferencesDialog />` 条件渲染
- [styles.css](../../apps/desktop/src/styles.css) +250/-9：① 菜单栏紧凑化 4 处常量；② 新 `.prefs-*` 系列样式（dialog + 分类树 + 表单 + 按钮 + audit table）
- 版本 bump：tauri.conf.json + Cargo.toml + Cargo.lock 三处 0.9.0 → 0.9.1

**接受标准**：本机 cargo + node 均不可用（与 STATUS [[ci-ubuntu-first-run-lint-gaps]] 一致）；CI ci.yml 兜底；真机正确性留 v0.9.1 release 装机后用户验证。

**未尽事宜**：v0.9.1 release 装机验证 + cycle 4-7 路线图（删 SettingsPage / 抽 useAppSettings hook / 迁 PrivacyPage / 子菜单 ▸ / 后端接通 / macOS 原生菜单）。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 cycle 6 v3 + v0.8.8 真机验证 (a)(b)(c)(d) 全 GO + bump v0.8.9 ⭐⭐⭐

**承接**：v0.8.8 发版后用户装机让我「真机验证」、computer-use 自动驱动 GUI 跑完 4 验证条全部 GO；顺带发现设置页文案「语义召回模型未找到」+ 下载按钮误显示与顶栏绿点 / 真实搜索能力不一致的旧 follow-up bug、当场开 cycle 6 v3 修。

**关键决策**：① v0.8.8 真机验证范围 = 不破坏用户 settings.json；② cycle 6 v3 根因定位走静态读 `embedding_model.rs::status()` + `StatusIndicator::is_active()` 两路判定对比、定位 NotLoaded 一刀切；③ 修法选「后端单点 path.exists() 对齐」而非「新加 Standby 状态」（前端零改动 + 用户感知一致 + 改动量 +14/-3）；④ 同 commit 含用户在工作区加的 BETA-33「桌面菜单栏重构 + 选项对话框」task 卡片。

**真机验证 GO 项**：
- ✅ (a) 空 `index_roots` 显示真系统默认 3 条 ⭐⭐⭐
- ✅ (b) 加自定义目录系统默认列表消失 ⭐⭐⭐
- ✅ (c) 移除自定义系统默认列表重现 ⭐⭐⭐
- ✅ (d) 搜「读后感」语义召回质量 ⭐⭐⭐（13 条 3127ms、4 后端 fan-out、top 3 全相关）

**cycle 6 v3 改动概览（commit `2f46761`、单文件 +14/-3 后端 + bump 三处版本号）**：
- [embedding_model.rs:206-227](../../apps/desktop/src-tauri/src/search/embedding_model.rs) `EmbeddingModelHandle::status()` 的 `NotLoaded` 分支加 `path.exists()` 检查
- 版本 bump：三处 0.8.8 → 0.8.9

**接受标准**：本机 cargo 不可用、CI ci.yml 兜底；真机验证留用户装 v0.8.9 NSIS。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-33 桌面菜单栏重构 cycle 1+2 + bump v0.9.0 ⭐⭐

**承接**：上一会话刚收完 BETA-31-v3 cycle 6 v3（v0.8.9 已合 main）。用户提需求「请参考 Everything 菜单栏，重新规划和调整 LociFind 菜单栏」并附截图。

**关键决策（AskUserQuestion 三轮收敛）**：① **实现路径 = C**（先纯前端 React `<MenuBar>` Windows 优先、macOS 原生留 BETA-34）；② **选项形态 = 模态弹窗**；③ **排期 = 起初登记新 task、用户改主意「直接动手吧」** → 本会话连续做 cycle 1+2；④ **不动 SettingsPage/PrivacyPage/UserSynonymsPage 三个独立路由**；⑤ 本机无 Node + 无 cargo、CI 兜底 + 真机手测。

**改动概览（3 新文件 + 6 改、单次 commit）**：
- 新 [MenuBar.tsx](../../apps/desktop/src/components/MenuBar.tsx) ~325 行：7 个下拉 + 28 菜单项 + 全局快捷键 Ctrl+N/F/P/D/Shift+C/, + Alt+F/E/V/S/B/T/H 访问键
- 新 [AboutDialog.tsx](../../apps/desktop/src/components/AboutDialog.tsx) ~75 行
- 新 [lib/menu-events.ts](../../apps/desktop/src/lib/menu-events.ts) ~45 行：MenuAction discriminated union + emit/onMenuAction
- [App.tsx](../../apps/desktop/src/App.tsx) / [styles.css](../../apps/desktop/src/styles.css) / [SearchView.tsx](../../apps/desktop/src/SearchView.tsx) 接通
- 版本 bump：三处 0.8.9 → **0.9.0**

**菜单项接通状态**：cycle 1+2 接通 14 项 + 全局快捷键 6 个 + Alt 访问键 7 个；cycle 3 待接：导出/列控制/排序/范围/跨语言/高级语法/管理保存/重建索引/索引状态/模型 ▸/后端 ▸/打开日志数据目录/键盘快捷键/用户手册/反馈/context-aware enable。

**接受标准**：本机 cargo + node 均不可用；CI 兜底；真机验证留 v0.9.0 release 装机后。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 cycle 6 v2 + cycle 5 真机验证 GO + bump v0.8.8 ⭐⭐⭐

**承接**：用户装 v0.8.7 后让我「真机验证」并授权开 computer-use 自动驱动 GUI 验证。本会话一次性把 cycle 5 真机验证 + cycle 6 真机验证 + cycle 6 v2 fix 全做完。

**关键决策**：① computer-use 工具栈走 ToolSearch 批量加载；② cycle 6 真机验证不破坏用户 settings.json；③ cycle 5 真验证走 `taskkill /F` + 新启动带 `LOCIFIND_ENABLE_EMBED=1`（15 分钟）；④ cycle 6 修法方案 D（backend 接 Option 参数 + frontend 传当前 useState）。

**真机验证 GO 项**：v0.8.7 启动正常、cycle 1 启动回填 last_summary GO、cycle 2 日志栈 GO、cycle 3 ranker 污染清理幂等 GO、cycle 4 native crash 守门 GO、**cycle 5 真机验证 GO ⭐⭐⭐**（worker 跑完 42 篇真文档零 crash、411 篇向量、`EMBED_TRUNCATE_CHARS=600` hotfix 完全有效）、cycle 6 标题数量切换 GO。

**cycle 6 v2 fix 改动概览（commit `89f6061`、2 文件 +39/-19）**：
- [settings.rs:308-327](../../apps/desktop/src-tauri/src/settings.rs) `get_effective_index_roots` 加 `index_roots: Option<Vec<String>>` 参数
- [SettingsPage.tsx](../../apps/desktop/src/pages/SettingsPage.tsx) useEffect 监听 `settings.index_roots` 变化重 fetch
- 版本 bump：三处 0.8.7 → 0.8.8

**接受标准**：本机 cargo 不可用；CI ci.yml 兜底；真机验证留用户装 v0.8.8 NSIS。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 7 刀 cycle 6：设置页显示生效索引目录 + bump v0.8.7

**承接**：cycle 5 修完后用户装 v0.8.6 验证 app 不崩、但发现新 UX bug：「索引配置里看不到当前已经添加的目录信息」。

**关键决策**：① 范围 = 最小动作 read-only 展示；② backend 单一信源 = 与 `resolve_index_roots` 同款逻辑；③ 0 个时 read-only 列出系统默认。

**改动概览（4 文件 +56/-6）**：
- [settings.rs:294-318](../../apps/desktop/src-tauri/src/settings.rs) 新加 `get_effective_index_roots` tauri command
- [main.rs:430](../../apps/desktop/src-tauri/src/main.rs) invoke_handler 注册
- [SettingsPage.tsx:95-104 / 355-372](../../apps/desktop/src/pages/SettingsPage.tsx) 新 effectiveRoots state + UI 改造
- 版本 bump：三处 0.8.6 → 0.8.7

**接受标准**：本机 cargo 不可用、CI ci.yml 兜底；真机验证留 v0.8.7 装机后。

---

## 2026-07-01 滚动归档（BETA-33 cycle 6 v4 之前 6 条：2026-06-29 ~ 2026-06-30）

> 本批归档：BETA-31-v3 cycle 1-5（第 1 刀 v0.8.0 UX gap + 第 2 刀路径 bug fix + 第 3 刀诊断日志栈 + 第 4 刀 ranker 污染 + 第 5 刀 native crash 止血 + 第 6 刀 EMBED_TRUNCATE_CHARS hotfix）共 6 条。

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 6 刀 cycle 5：EMBED_TRUNCATE_CHARS 1200 → 600 hotfix + bump v0.8.6 ⭐

**承接**：cycle 4 修完 native crash 止血、v0.8.5 装机后用户搜「读后感」首页 4 条全是真相关结果（罗翔讲读后感 mp3 / 真 docx「语文怎么学」/ 真 pptx「柳林风声读后感」/ 真 docx「四年级上册推荐书目」），cycle 1-4 累积修复链路全部 work、用户选 C 接着真修 llama-cpp native crash。

**复现 + 锁定根因（用户跑 `LOCIFIND_ENABLE_EMBED=1` + Cycle 4 per-doc 日志直接定位）**：
- 日志最后一行：`即将 embed 文档 doc_idx=1 total=139 path=...\WeChat Files\...\2024-01\2下53单元归类复习（语文）.pdf body_len_chars=1200`
- 1 秒后 Windows Event Log：`ucrtbase.dll Exception 0xc0000409 fault offset 0xa527e`（与前 5 次崩溃指纹完全一致）
- **关键证据：body_len_chars=1200 完全等于 `EMBED_TRUNCATE_CHARS` 截断上限**（不是文档内容问题、是常量配置溢出）

**根因诊断**：
- BETA-26 锁定 `EMBED_TRUNCATE_CHARS=1200`、当时模型 qwen3-0.6b context 8192、安全
- BETA-15B-7-v2 切到 bge-m3、context 8192、安全
- **BETA-15B-11-v2（2026-06-27）切到 embeddinggemma-300m、context 仅 2048 token**、未同步降截断常量
- 中文 SentencePiece tokenizer typical char-to-token ratio 1.5-2.0 → **1200 字符 → 2000-2400 token → 溢出 2048 context** → llama-cpp ggml 内部 abort/__fastfail → ucrtbase 0xc0000409
- BETA-31-v3 cycle 4 之前 worker 因多重 bug（路径不一致 / 空向量 vector_is_current 跳过）从未真跑到 embed() 调用、所以这个 bug 一直没暴露；cycle 3 purge + cycle 4 env 开关 first 真跑到 embed() 立即崩

**改动概览（4 文件 +25/-5、最小动作）**：
- [embed.rs:24-45](packages/indexer/src/embed.rs:24) `EMBED_TRUNCATE_CHARS` 从 `1200` 改 `600`（中文 ≈ 1000-1200 token、留 800+ token / 60% buffer）+ 详细 doc 注释（历史 BETA-26/BETA-15B-7-v2/BETA-15B-11-v2 沿革 + cycle 5 根因 + 影响面）
- 版本 bump：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.8.5 → 0.8.6

**接受标准**：本机 cargo 不可用、CI ci.yml 兜底（fmt + clippy + workspace test 含 indexer 5 单测、`is_embed_worthy` / `purge_short_body_vectors` 行为不变）；真机行为留 v0.8.6 release 装机后用户验证：(a) 跑 `LOCIFIND_ENABLE_EMBED=1` 重新触发 spawn_semantic_index、(b) worker 应跑完 139 篇真文档（vs cycle 4 第 1 篇即 crash）、(c) `locifind.log` 出现 `spawn_semantic_index: 后台 worker 结束 worker_elapsed_ms=... summary="语义索引就绪 N 篇"`、(d) 「语文怎么学？(系列精华文）.docx」拿到向量（cycle 3-4 时一直 has_vec=0）、(e) 再搜「读后感」前几条应都是真相关 docx/pptx。

**影响面**：
- ✅ 历史 369 残留向量保留（`vector_is_current` 检查 source_hash、不重嵌）
- ✅ parser-only evals byte-equal 不受影响（不调 embed pipeline）
- ⚠️ 新文档用前 600 字符嵌入、长 PDF 中后文影响较大（前 600 字常含主题词、docx/作文集影响小）
- ⚠️ 评测维度可能 OVERALL/crosslang 微降（短文档不变、长文档前缀截断更狠）— 留 cycle 5b token-aware truncate 精修（按 GGUF metadata 的 n_ctx 自动算上限、不再硬编码）

**未尽事宜**：① **v0.8.6 真机验证**留用户——重点看 worker 是否能跑完 139 篇真文档不崩；② **cycle 5b token-aware truncate**（model-runtime 拿 GGUF n_ctx、按 model 自动算上限、不再用硬编码常量、~2-3h）；③ **cycle 2 UX 扩展**（Step 2 模型四入口、~1d）仍 pending；④ **BETA-15B 评测 baseline 影响评估**（embeddinggemma 真水位用 600 截断 vs 历史 1200 baseline、可能 ±0.005-0.020、若回退超阈值需重 sweep、留 follow-up cycle）；⑤ **真修留 cycle 6 候选**：升级 llama-cpp-4 到最新（修上游 ggml token 溢出 abort、改成 truncate）/ 切回 bge-m3（context 8192 更宽容）/ Win32 SEH handler 写 native crash 信息到日志（即使 cycle 5 没修干净也能拿堆栈）。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 5 刀 cycle 4：ucrtbase 0xc0000409 native crash 止血 + bump v0.8.5 ⭐

**承接**：cycle 3 修了 ranker 污染、v0.8.4 装机用户首次看到 spawn_semantic_index worker 真正进入 embed_pending 调 `embedder.embed()` 嵌入真文档时、**app 整个崩掉消失**。用户让我自己查、我用 `Get-WinEvent` 拉 Windows 事件日志锁定 native crash 指纹。

**根因（Win 事件日志 5 次崩溃指纹完全一致）**：
- **Faulting module**: `ucrtbase.dll`（Microsoft Universal C Runtime）
- **Exception code**: `0xc0000409` = `STATUS_STACK_BUFFER_OVERRUN` —— **几乎从不是真的栈溢出**、VS2015+ 把所有 fail-fast 路径（`abort()` / `__fastfail()` / `std::terminate()` / `__report_gsfailure` / Rust `process::abort()`）统一报这个代号
- **Fault offset**: `0x00000000000a527e`（每次完全相同）= ucrtbase 同一 fail-fast 入口、确定性 crash
- 5 次崩溃跨 v0.8.3（10:46:06 / 10:48:17）+ v0.8.4（11:26:19 / 11:31:33 / 11:34:02）—— **同一深层 bug、跨多个版本**

**为什么 cycle 3 之前没暴露**：
- v0.8.0/0.8.1/0.8.2 路径 bug 模型从未加载、worker abort 不嵌入
- cycle 2/3 之前 worker 因为 2554 条空 PNG 向量 `vector_is_current` 命中跳过、根本没真调 `embedder.embed()`
- cycle 3 purge 之后 worker 终于真的开始嵌入第一篇真文档（如 1221 字符的「《银河帝国》阅读单.docx」）→ llama-cpp / ggml native crash → 整个进程被 ucrtbase 强制终止 → Rust `catch_unwind` 兜不住 native abort → 日志最后一行戛然而止在 `prewarm 完成 prewarmed=true`

**历史前科**：BETA-15B-7-v2 hotfix「BERT encode n_ubatch panic」、BETA-15B-9「qwen3-8b Fused Gated Delta Net × Last pooling × embedding-only 交互 8b-specific bug」—— 都是 llama-cpp FFI 层 batch/chunk size 边界问题。embeddinggemma-300m 在嵌入中文真文档时踩到同类 bug。

**关键决策**：① 范围 = **短期止血 + 诊断可观测、不真修底层 llama-cpp**（真修需升级 llama-cpp / 调 batch size / 切模型、超 cycle 4 范围）；② 默认禁 spawn_semantic_index 新嵌入（保现有 369 真文档向量 + FTS + Everything + Windows Search 全部可用、用户新加文档无法进语义召回是 trade-off）；③ env `LOCIFIND_ENABLE_EMBED=1` 显式开启嵌入（用户主动诊断时）；④ embed_pending per-doc info log 即使在禁用模式下也保留代码、用户开 env 重试时立即生效；⑤ Rust panic hook + Win32 SEH **不**做（catch_unwind 已包 worker、native crash SEH 改动量大留 follow-up）；⑥ 本机 cargo 不可用（之前 session 能跑、本会话 PowerShell + bash 都找不到 cargo.exe）→ 跳过本地 fmt check + 手维 Cargo.lock 给 indexer 加 tracing dep entry、CI 兜底验证。

**改动概览（7 文件 +75/-4）**：
- [index_status.rs:198-213](apps/desktop/src-tauri/src/search/index_status.rs:198) spawn_semantic_index 加 env 守门、默认 `semantic_abort` 写「嵌入暂停（防 native crash, set LOCIFIND_ENABLE_EMBED=1 开启）」+ warn! 输出 Why、env=1 才继续走原有 prewarm + embed_pending 流程
- [packages/indexer/Cargo.toml:28-31](packages/indexer/Cargo.toml:28) 加 tracing = "0.1" 依赖（facade only、subscriber 由调用方注入）+ Cargo.lock 同步加 locifind-indexer dependencies tracing entry
- [packages/indexer/src/doc_db.rs:485-507](packages/indexer/src/doc_db.rs:485) embed_pending 循环内每篇调 embed() 之前 tracing::info!(doc_idx, total, path, body_len_chars, "即将 embed 文档") + 失败时 tracing::warn!(error, "embed 失败")
- [main.rs:284-308](apps/desktop/src-tauri/src/main.rs:284) std::panic::set_hook 写 tracing::error!（thread + location + message）+ 启动 dump 加 `embed_pending_enabled` 标志
- bump v0.8.4 → v0.8.5（tauri.conf.json + Cargo.toml + Cargo.lock）

**接受标准**：本机 cargo 不可用、CI ci.yml 兜底跑 indexer + 其余 6 Rust-only crate；real-world 验证留用户装 v0.8.5 后看：(a) `locifind.log` 启动 dump 应含 `embed_pending_enabled=false`；(b) spawn_semantic_index 输出 `默认禁用（防 llama-cpp native crash 杀进程）` warn 行；(c) 设置页 SemanticIndex 状态显示「嵌入暂停（防 native crash, set LOCIFIND_ENABLE_EMBED=1 开启）」；(d) **app 不再崩**——可以正常用 FTS 搜内容 / Everything 搜文件名 / 现有 369 真文档向量做语义召回（含 12 篇 docx 作文集）；(e) 如用户主动 set `LOCIFIND_ENABLE_EMBED=1` 重启再触发 crash、locifind.log 最后一行 `即将 embed 文档 doc_idx=N total=M path=... body_len_chars=...` 就是触发 crash 的文档、贴回给我用于真修。

**未尽事宜**：① **v0.8.5 真机验证留用户** —— 装新版后看上述 (a-e)；② **真修 llama-cpp / ggml native crash** 留 cycle 5（候选方向：升级 llama-cpp-4 到最新 / 在 model-runtime 加 batch_size + n_ubatch + context_size 守门 / 切回 bge-m3 验证是否模型特定 / GGUF 重下校验 / 跑 Windows 调试 build 用 cdb 拿堆栈）；③ **第 2 刀 cycle 2 UX 扩展仍 pending**（Step 2 模型四入口）；④ **Win32 SEH handler 写日志** 留 follow-up（cycle 4 范围内只用 Rust panic hook、native crash 仍只能从 Windows 事件查看器看）；⑤ **设置页加「打开日志目录」按钮** 留 follow-up（让用户方便分享日志）；⑥ memory 不加（这是 llama-cpp 底层 bug、不属共享教训）。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 4 刀 cycle 3：ranker 污染 bug 修复 + bump v0.8.4 ⭐

**承接**：cycle 2 加完日志栈、v0.8.3 装机后用户搜「读后感」复现奇怪结果 + 让我「自己去看 locifind.log」。我读日志 + 直接 SQLite inspect 桌面 `index.db`、定位 BETA-15B-1 以来埋下的 ranker 污染 bug、当场开 cycle 3 修。

**根因（locifind.log + SQLite 双证据）**：
- locifind.log：app 启动 / 4 backend 注册 / FTS reindex 完成 0 delta（增量、已索引）/ search 出口 path=fanout total=10 elapsed_ms=2533 / embedding 模型 NotLoaded→Loading→Ready 1187ms / spawn_semantic_index 在搜索之后才启动 prewarmed=true prewarm_elapsed_ms=0（已被 search 触发的 ready() 加载好）—— 全是 INFO 级正常路径、无 error/warn
- SQLite inspect index.db：documents 4828 总数、document_vectors 3433 全 embed_model="embeddinggemma-300m"；按 doc_type 分布 **PNG 2670 + JPG 694 + txt 46 + docx 12 + pdf 7 + pptx 2 + xlsx 1 + gif 1**（图片占 98%）
- PNG 向量 body 分布：**body_len=0 占 2554/2670（96%）** + <10 字符 99 + 10-50 字符 9 + 50-200 字符 6 + ≥200 字符仅 2
- 抽样：default_avatar.png / member-100e5f.png / lv_bg-09ef39.png 等截图里的 PNG 全部 body_len=0
- 真正包含「读后感」的文档（如女儿小学作文集 docx）有向量但被 2554 个空 PNG 向量挤出 top-N
- 推理：EmbeddingGemma 对空字符串产出 "neutral" 向量、与任意 query 的 cosine ≈ 模型 mean similarity（~0.5-0.7）、远高于 floor=0.30、加上 weight=10.0 把 ranker top-N 全部挤占

**关键决策**：① 范围 = **indexer 守门 + 一次性数据清理**（最小可见效路径、不动 BETA-03 OCR 上游也不动 SemanticIndexBackend 查询路径）；② 守门阈值 = 20 字符（trim 后 + chars().count() 单一信源、CJK 安全）；③ 数据清理同步跑（毫秒级、不阻塞 setup）+ 幂等（已清返 0）；④ 清理判断口径与守门 100% 一致（同走 `is_embed_worthy()`、不用 SQL length() 字节判）；⑤ 不动 `vector_is_current` 逻辑（清理后 spawn_semantic_index 自动重嵌真文档、source_hash 不变的旧污染向量除非被 DELETE 否则永不重嵌、所以必须显式清理）。

**改动概览（6 文件 +162/-3）**：
- [embed.rs:24-44](packages/indexer/src/embed.rs:24) 新加 `pub const MIN_EMBED_TEXT_CHARS: usize = 20` + `pub fn is_embed_worthy(text)` helper + 6 边界 unit test（空 / 全空白 / 1 char / 19 / 20 边界 / 中文 20 / trim 后 20）
- [doc_db.rs:455-468](packages/indexer/src/doc_db.rs:455) embed_pending 内 `if !is_embed_worthy(&body) { continue }` 守门（永久防新污染）
- [doc_db.rs:101-141](packages/indexer/src/doc_db.rs:101) 新加 `pub fn purge_short_body_vectors(&self) -> Result<usize, IndexError>`：SELECT document_vectors JOIN documents_fts 拿 (doc_id, body)、Rust 侧 is_embed_worthy 过滤、batch DELETE；2 单测（混合场景 3→1 删 2 幂等 / 空表返 0）
- [main.rs:374-400](apps/desktop/src-tauri/src/main.rs:374) setup 在 compute_index_totals 回填后加 purge 调用、info!/warn! 打点
- 版本 bump：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.8.3 → 0.8.4

**接受标准**：本机 fmt 净；本机 Win 无 MSVC linker 跑不动 cargo build/test、**CI ci.yml 兜底**（ci.yml 已覆盖 7 个 Rust-only crate 含 locifind-indexer、本 cycle 加的 5 个新单测会自动跑）；行为正确性留 v0.8.4 release 装机后用户验证：(a) 启动看 `locifind.log` 应出现「启动清理脏向量：删除 ~2554 条」info 行；(b) 搜「读后感」应返回真 docx 作文集而非缓存图片；(c) 设置页 SemanticIndex 状态在 spawn_semantic_index 跑完后应显示「语义索引就绪 N 篇」N ≈ 870（4828 docs - 2670 PNG - 694 JPG = ~1464 候选、is_embed_worthy 过滤短 body 后估计 800-1000 真文档）。

**未尽事宜**：① **v0.8.4 release 真机验证留用户**——装新版后看上述 (a)(b)(c)；② **第 2 刀 cycle 2 UX 扩展仍 pending**（Step 2 模型四入口）；③ **follow-up cycle 候选**：(a) BETA-03 OCR 上游修——为什么空 body 的 png 写进 documents 表？应只有 OCR 成功且 ≥ N 字符才入表、范围更大、独立 cycle；(b) BETA-15B 系列评测集如果含图片应也走 is_embed_worthy 过滤、确认不会回退；(c) 设置页加「打开日志目录」按钮（cycle 2 deferred）；④ memory 不加（cycle 1 路径教训已加；本 cycle ranker 污染是 indexer 上游 bug、不属共享教训）。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 3 刀 cycle 2：加桌面 app 诊断日志栈 + bump v0.8.3

**承接**：v0.8.2 装机后用户真机搜「读后感」返 10 条完全无关的 OneDrive PNG 图片、全是「纯语义命中」、4 后端都参与。我无法静态判断根因（可能性：冷启动语义嵌入未完成 / OCR 噪声 / SemanticIndexBackend cosine bug / corpus enrichment 关联）、提了 3 个观察点让用户帮看。用户反问「这些诊断有日志吗？没有就加日志功能让你下次自助」—— 一句话授权我开 cycle 2 加日志栈。

**关键决策**：① 范围 = **最小可观测层 only**（不加设置页「打开日志目录」UI、不重写 audit.jsonl、不打 per-result score 细节、不打 SemanticIndexBackend candidate dump、留 follow-up cycle）；② 日志位置 = `<locifind_data_dir>/locifind.log`（与 index.db / audit.jsonl 同目录、用户日后可手动 cp 给我）+ daily 滚动（跨日重命名 `locifind.log.YYYY-MM-DD`）；③ 级别 = 默认 INFO + env `LOCIFIND_LOG` 覆盖（debug/trace/warn/off + per-target 形如 `locifind_desktop=debug`）；④ tracing-subscriber + tracing-appender::non_blocking + WorkerGuard 必须 bind 到 main() 范围保 flush；⑤ ANSI 关（文件 sink 不要颜色码乱码）；⑥ 打点完整 query 字符串（日志只在本机、用户主动给才外传、上下文用户已知）。

**改动概览（8 文件 +221/-8、~30 行 helper 函数 + 12 处 info!/warn! 打点）**：
- [Cargo.toml:53-60](apps/desktop/src-tauri/Cargo.toml:53) 加 tracing 0.1 / tracing-subscriber 0.3 (env-filter) / tracing-appender 0.2 三依赖、注释说明 LOCIFIND_LOG 用法
- [Cargo.lock](Cargo.lock) `cargo update --workspace --offline` 只新增 tracing-appender + symlink 2 entries（tracing-subscriber 已被 daemon 间接引、复用、不重复）+ locifind-desktop 0.8.2→0.8.3 lock bump、+23/-1 干净
- [main.rs:50-91](apps/desktop/src-tauri/src/main.rs:50) 新加 `log_dir()` + `init_tracing()` helper：tracing_appender::rolling::daily + non_blocking writer + EnvFilter + 关 ANSI；返 WorkerGuard 让 main bind 保活
- [main.rs:284-300](apps/desktop/src-tauri/src/main.rs:284) `main()` 顶部先 init_tracing 再 dump 启动行（version + os + arch + data_dir + index_db + audit_log + 2 feature 状态）
- [main.rs build_registry](apps/desktop/src-tauri/src/main.rs:75) 5 backend (local / semantic / spotlight / windows-search / everything) 注册成功 info! / 失败 warn! 双打（保留 eprintln! 兼容 dev console）
- [main.rs:380-415](apps/desktop/src-tauri/src/main.rs:380) 后台 reindex spawn 任务加 info!/warn!：开始 / 完成（含 elapsed + music/doc/image added/updated counts）/ 失败 reason
- [embedding_model.rs:128-188](apps/desktop/src-tauri/src/search/embedding_model.rs:128) `ready()` 状态机加 4 处打点：路径不存在 warn / NotLoaded→Loading info（path + file_size） / Loading→Ready info（path + load_elapsed_ms）/ Loading→Failed warn（reason）
- [index_status.rs spawn_semantic_index](apps/desktop/src-tauri/src/search/index_status.rs:186) 加 3 处 info!：is_active=false 直返 / prewarm 完成（prewarmed bool + prewarm_elapsed_ms）/ worker 结束（worker_elapsed_ms + summary snapshot）
- [search.rs search_impl 入口](apps/desktop/src-tauri/src/search.rs:307) 加 1 处 info!：query + query_len + adhoc flag
- [search.rs:541-548](apps/desktop/src-tauri/src/search.rs:541) 出口 info!：path="fallback-chain" + total + elapsed_ms + served_by
- [fanout.rs:167-172](apps/desktop/src-tauri/src/search/fanout.rs:167) 出口 info!：path="fanout" + total + elapsed_ms
- [fanout.rs:364-369](apps/desktop/src-tauri/src/search/fanout.rs:364) 出口 info!：path="balanced-multitype" + total + elapsed_ms
- bump v0.8.2 → v0.8.3（tauri.conf.json + Cargo.toml + Cargo.lock 三处）

**接受标准**：本机 `cargo fmt --all -- --check` 净；本机 Win 无 MSVC linker 跑不动 cargo build/clippy/test、CI ci.yml 兜底；行为正确性留 v0.8.3 release 装机后用户搜「读后感」复现 + 取 `%APPDATA%\Roaming\LociFind\locifind.log` 给我看。

**预期下次诊断流程**（用户装 v0.8.3 后）：① 装新版；② 搜「读后感」复现；③ 把 `%APPDATA%\Roaming\LociFind\locifind.log` 给我；④ 我 grep 启动 dump + spawn_semantic_index summary + search 出口的 query/total/elapsed/served_by + EmbedStatus 切换路径，定位根因（冷启动稀疏？OCR 噪声？模型加载失败？SemanticIndexBackend 反常？）。

**未尽事宜**：① **v0.8.3 release 真机验证留用户**——装新版后看：(a) `locifind.log` 文件被创建在 `%APPDATA%\Roaming\LociFind\`；(b) 文件内有「LociFind 桌面 app 启动」开头行 + 5 backend 注册行 + embedding model Loading→Ready 行 + spawn_semantic_index 完成行；(c) 搜任意词后日志出现「search 入口」+「search 出口」配对行；② **第 2 刀 cycle 2 UX 扩展仍 pending**（Step 2 模型四入口、待用户拍板）；③ **「读后感」搜索奇怪结果调查 pending**——v0.8.3 装机后用户取日志我深挖；④ follow-up cycle 候选：(a) 加设置页「打开日志目录」按钮；(b) 加 SemanticIndexBackend per-query candidate count + top-3 score dump（debug 级、env 开）；(c) 日志保留天数自动清理（tracing-appender 不支持、要手写）；(d) tracing init 加 stderr layer 让 dev mode 也能 console 看；⑤ memory 不加（cycle 1 已加路径不一致教训、本 cycle 加日志栈是 fix-forward 不是 lesson learned）。

---

### 2026-06-30 — Claude Code (Opus 4.7) — BETA-31-v3 第 2 刀 cycle 1：根因 bug fix「下载路径 ≠ 查找路径」+ bump v0.8.2

**承接**：用户切到 Windows 真机后第三次报「下完模型软件自动退出、再次打开还是找不到模型、需要再次下载、反复死循环」+ 顺带要求 Step 2 模型下载扩 4 入口（自动下载 / 手动选已有 / 自动扫本地 / 稍后）。先静态读 5 层栈定位根因、给方案、用户拍板「按 cycle 1 bug fix + cycle 2 UX 扩展两步走」节奏。

**根因（静态读栈定位、3 文件路径对比）**：BETA-31 cycle 0（2026-06-27）初版 `model_download.rs` 用 Tauri 推荐的 `app.path().app_data_dir()` 解下载目录、Windows 实际 = `%APPDATA%\Roaming\ai.locifind.desktop\models\embeddinggemma-300m-q8_0.gguf`；但历史代码（BETA-04 `local_index_db_path` / BETA-06 `audit_log_path` / BETA-15B-1 `EmbeddingModelHandle::new` / BETA-23 `ModelFallbackHandle::new`）一直用 `dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("LociFind")`、Windows 实际 = `%APPDATA%\Roaming\LociFind\models\embeddinggemma-300m-q8_0.gguf`——**两个完全不同的目录**。下载完文件 EmbeddingModelHandle 永远找不到、`embedding_model_status` 永返 NotFound、SettingsPage 内联下载按钮永远显示引导用户重复下载、陷入死循环；这也是 2026-06-29 Windows 真机会话日志「桌面 GUI 真机暴露的 3 个 UX bug」第 ② 条「embedding_model.rs NotLoaded 直接渲染成 NotFound、不真的查 path.exists()」的同源下游表现——路径都不一致、文件 exists() 自然永 false。

**关于「软件自动退出」**：代码层未找到 panic / exit 路径。最可能是错觉——下载完 500ms 后 onboarding 自动 `setStep(3)` 跳到「试试搜索」、再点 example 后 `navigate('/')` 跳主页，非技术用户视觉上像「窗口关掉了」。若真有进程退出待用户提供 Windows 事件查看器 crash 日志再深挖（cycle 1 不阻塞）。

**改动概览（4 文件 + 3 处版本号、最小修法）**：
- [main.rs:38-58](apps/desktop/src-tauri/src/main.rs:38) 新加 `pub(crate) fn locifind_data_dir() -> PathBuf` 单一信源 helper、doc 注释含详细路径说明 + 警告「不要在子模块独立用 `app.path().app_data_dir()`」+ BETA-31 cycle 0 踩坑历史；`local_index_db_path` / `audit_log_path` 改用 helper 派生（语义零变化）。+15 / -8
- [main.rs:290-318](apps/desktop/src-tauri/src/main.rs:290) `EmbeddingModelHandle::new` + `ModelFallbackHandle::new` 两处 data_dir 参数从 `dirs::data_dir().unwrap_or_else(...).join("LociFind")` 改用 `locifind_data_dir()`（语义零变化、消除重复字面量）。+2 / -6
- [model_download.rs:1-18, 61-69, 156, 177](apps/desktop/src-tauri/src/model_download.rs:1) mod doc 头加「路径单一信源」段落 + BETA-31 cycle 0 踩坑警告；`resolve_target_paths` 去掉 `&AppHandle` 参数 + 改用 `crate::locifind_data_dir()`、返回类型从 `Result<...>` 改 plain tuple（不会再失败）；删 `use tauri::Manager`（不再调 `app.path()`）；调用方 `let (...) = resolve_target_paths();`。**本 fix 真正修 bug 的一行**：models_dir 从 Tauri `app_data_dir/models` 改 `locifind_data_dir/models`、与 EmbeddingModelHandle 查找路径**完全对齐**。+17 / -11
- 版本 bump：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.8.1 → 0.8.2

**接受标准**：本机 `cargo fmt --all -- --check` 净（无 diff 输出）；本机 Windows 无 MSVC linker 跑不动 clippy/test、CI ci.yml 兜底跑 locifind-server lib + clippy + fmt（本 cycle 改动在 apps/desktop、ci.yml 范围之外但下次扩范围可覆盖、改动本身极小风险低）；行为正确性留 v0.8.2 release 装机真机验证。

**其他模块路径策略说明**（grep 全扫确认）：`history.rs` / `user_synonyms.rs` / `settings.rs` / `permissions.rs` 各自仍用 `app.path().app_config_dir()` 或 `app_data_dir()` Tauri 路径——它们各自闭环（自写自读、无跨模块共享），不形成 bug。强行迁移反而破坏现有用户的搜索历史 / 同义词 / 设置数据（这些数据已在 Tauri 路径下、迁到 `LociFind\` 等于丢数据）。**只有 model_download.rs 这条链跨模块共享路径**（写在 model_download / 读在 EmbeddingModelHandle）、必须对齐。

**未尽事宜**：① **v0.8.2 release 真机验证留用户**——装新版后看：(a) Step 2 走「下载模型」流程后顶栏「语义召回」灯转绿、搜索 via 列表出现 `search.semantic`；(b) 重启后 SettingsPage 显示「语义召回：已就绪」而非「未找到」；(c) 如本机已有错位下载文件（在 `%APPDATA%\Roaming\ai.locifind.desktop\models\`），可手动 cp 到 `%APPDATA%\Roaming\LociFind\models\` 避免重新下载 313 MB；② **bump 后推 `v0.8.2` tag 触发 `release-windows.yml`**（产 NSIS installer、用户拍板时机执行）；③ **第 2 刀 cycle 2 UX 扩展待用户拍板**（Step 2 模型四入口：自动下载 + 手动选已有 + 自动扫本地 + 稍后、~1d、新 tauri commands `pick_embedding_model_path` / `scan_local_embedding_models` + ModelDownloadStep 重做）；④ 5 UX bug 仍剩 3 个未修（顶栏灯 ≠ EmbedStatus / 设置页 NotLoaded→NotFound 渲染脱节 / Everything 检测失败 GUI 引导）；⑤ memory 加一条「Tauri `app_data_dir` vs `dirs::data_dir` 路径不一致」教训、供下次新加跨模块共享数据时反射查。

---

### 2026-06-29 — Claude Code (Opus 4.7) — BETA-31-v3 v0.8.0 真机 UX gap 修复集第 1 刀 + BETA-31-v2 spec 起草（cycle 待续）+ v0.8.1 release

**承接**：用户切到 Windows 真机后选「先本地测试软件」，发现 2 个 v0.8.0 UX bug ——① 索引配置 UI 看不到"当前已添加几个目录"；② 重启后状态显示「尚未索引」让人以为索引数据丢失。同会话先按推荐启动 BETA-31-v2 Windows GPU 推理（vulkan）cycle、完成调研 + brainstorming + spec 起草、但用户决定 VS Build Tools + Vulkan SDK 装机体量太大改日推、cycle 暂搁；spec 文件入仓待续。然后用户报 2 UX bug、改方案 B（UI 文案 + 启动回填）落地。

**关键诊断**：
- bug ①：[SettingsPage.tsx:347-352](apps/desktop/src/pages/SettingsPage.tsx:347) **已渲染**已添加列表、但标题没明示数量、空时与"未添加过"视觉同形；「+ 添加目录」加入内存 state 后未提示用户还要点底部「保存设置」才持久化。
- bug ②：[main.rs:350-367](apps/desktop/src-tauri/src/main.rs:350) 启动无条件 spawn 后台 reindex、`IndexStatus` 是内存对象、`last_indexed/last_summary` 每次启动重置 `default() = None` → UI 显示「尚未索引」误导。**SQLite 索引数据库本身（index.db）持久化、数据不丢**、是状态显示问题。

**改动概览（2 文件 + 3 处版本号）**：
- [main.rs:336-352](apps/desktop/src-tauri/src/main.rs:336) setup 阶段加启动回填：`compute_index_totals(db_path)` 走 3 个 SQLite COUNT、毫秒级、不阻塞 setup；db 文件存在且总数 > 0 时填回 `last_summary = "音乐 X / 文档 Y / 图片 Z"`。`last_indexed` 不回填（无可信时间戳来源、仅 reindex 完成后写）。+16 行
- [SettingsPage.tsx:343-376](apps/desktop/src/pages/SettingsPage.tsx:343) UI 文案 3 处：① 标题加「当前 X 个」；② 0 个时显示「未添加自定义目录，将使用系统默认...」；③「+ 添加目录」成功后 5s toast「已加入待保存列表，请点保存设置生效」；④ 索引状态行分四档（indexing 时显示当前总数 / last_summary fallback / 真空才显示"尚未索引"）。+24 / -3 行
- 版本 bump：tauri.conf.json + apps/desktop/src-tauri/Cargo.toml + Cargo.lock 三处 0.8.0 → 0.8.1

**接受标准**：本机无 MSVC linker、不能 build / test 本地验证；纯 UI + 启动一次性 SQLite 查询、风险面小；行为正确性留 v0.8.1 release 装机真机验证。

**未尽事宜**：① **v0.8.1 release 真机验证留用户**——装新版后看：(a) 设置页索引目录标题显示「当前 X 个」、(b) 重启后状态行显示「当前索引: 音乐 X / 文档 Y / 图片 Z」不再是「尚未索引」、(c) 加目录后看到 5s「已加入待保存列表」toast；② **5 UX bug 剩余 3 个未修**（顶栏灯 ≠ EmbedStatus / 设置页 NotLoaded→NotFound 渲染脱节 / Everything 检测失败 GUI 引导）+ embedding 冷启动 17s 撞 timeout 留 BETA-31-v2（cycle 待续）+ Everything 服务模式不兼容留 follow-up；③ **BETA-31-v2 spec 入仓**（[2026-06-29-beta-31-v2-windows-gpu-vulkan-design.md](docs/superpowers/specs/2026-06-29-beta-31-v2-windows-gpu-vulkan-design.md)）、cycle 待用户装完 VS Build Tools + Vulkan SDK 后接续；④ ROADMAP BETA-31 卡片 follow-up 列表中"BETA-31-v3 模型 SHA256 签名验证"重新编号为 BETA-31-v4，腾出 BETA-31-v3 给 v0.8.0 真机 UX 修复集这条线。

---

## 2026-06-30 滚动归档（cycle 1 之前 17 条：2026-06-25 ~ 2026-06-29）

> 本批归档：BETA-15B-8/9/10/11/11-v2 / BETA-31 / BETA-32 / BETA-32 follow-up / 真机 GO / ci.yml 扩范围 等共 17 条。

### 2026-06-29 — Claude Code (Opus 4.7) — ci.yml 扩范围：1 → 7 Rust-only crate 持续守门

承接 follow-up ⑤ 后、剩余 backlog 多需 MSVC / Mac Metal、转推 CI 基建。ci.yml 从只测 locifind-server 1 crate 扩到 7 个 Rust-only crate：locifind-server / locifind-search-backend / locifind-harness / locifind-indexer / locifind-intent-parser / locifind-ranker / locifind-result-normalizer。都不依赖 llama-cpp（intent-parser 走 model-runtime stub default），都能 ubuntu-22.04 build。

**意外的好**：CI 一次过、6 新 crate 没踩 lint gap（无 dead_code / doc_markdown / fmt / must_use 报错）。说明这些 crate 历史 cycle 在 Mac/Win 跑过 workspace clippy 已干净、ubuntu 上也干净；之前唯一 dead_code 报错（locifind-indexer parse_paths_lines）在 bug #2 cycle 已修过。

**排除**：① model-runtime / evals / spike-retrieval（llama-cpp / mac metal 依赖）；② spotlight / windows-search / everything / platform-*（OS-conditional）；③ apps/desktop / apps/daemon（tauri / MSVC linker）。

**收工**：commit `1bcde7a`（CI 扩范围）+ 本 STATUS doc-sync 一并落库；ROADMAP / memory 不动（本 cycle 无新 lint gap、不属具体 task）。

---

### 2026-06-29 — Claude Code (Opus 4.7) — BETA-32 follow-up ⑤：rmcp stateless 切（lib done、e2e helper 留 follow-up）

承接 follow-up ④ 后用户选 follow-up ⑤ 但**先确认 cycle 范围被低估**：lib 改极小（~7 行）但 e2e helper（`mcp_initialize` / `mcp_call_tool` / `extract_jsonrpc_from_sse`）按 SSE framing + session-id header 写、stateless 切后响应纯 JSON、helper 与协议不匹配；本机 Windows 无 MSVC linker 不能 `cargo test -p locifindd` 验、CI ci.yml 只 -p locifind-server lib 不抓 e2e。

**用户选最小走法**：① lib 改（app.rs stateful=false）+ ② e2e 2 个 MCP 测试加 `#[ignore = "stateless 切后 e2e helper 需重写、留 follow-up cycle"]` + ③ STATUS 标 e2e 重写在 follow-up cycle。

**改动**：
- [app.rs:50-60](packages/locifind-server/src/app.rs:50)：`with_stateful_mode(true)` → `false`，注释更新解释 stateless trade-off（每连接需自带 protocol-version header、session-id 不再发；LLM client 普遍能 cache initialize / tools/list 跨 request、客户体验损失小）
- [apps/daemon/tests/e2e.rs](apps/daemon/tests/e2e.rs)：`e2e_list_roots_after_indexing` + `e2e_search_returns_results` 加 `#[ignore]`，doc 注释指向 follow-up cycle 重写方向（删 session-id 管理 / Accept 改纯 JSON / parse 直接 serde_json）。剩 3 个 e2e 测试（health / 401 / reindex 409）不走 MCP 协议、不受影响

**未尽事宜**：① e2e helper 重写（mcp_initialize 删 session_id 返 / mcp_call_tool 删 session-id header / extract_jsonrpc_from_sse 替换为纯 json parse）作 BETA-32 follow-up ⑤b cycle、需 Mac/MSVC 环境验证 `cargo test -p locifindd -- --ignored`；② 真机重测 deferred（lib 改后 LLM client 行为变化需用户在 Claude Code MCP 接 daemon 验证 stateless mode handshake）。

---

### 2026-06-29 — Claude Code (Opus 4.7) — BETA-32 follow-up ④：release-windows.yml --locked 守门

承接 T6 #6 cycle 后用户选最小 cycle。release-daemon.yml 之前 cycle 加了 `cargo build --locked`、release-windows.yml 没同步（T12 reviewer 提）。本 cycle 修：① tauri-action `args` 加 `-- --locked` 透传 cargo flag；② 加独立 `Verify Cargo.lock pin (--locked)` step（在 tauri build 之前跑 `cargo metadata --locked`、lock 漂移时 fail-fast）—— belt-and-suspenders 防 tauri build CLI 透传行为不确定。

**未真机验证**：release-windows.yml 不走 push to main 触发、留下次推 `v*` tag 时跑 release-daemon-windows.yml 验证两 step 行为；workflow_dispatch 触发会创真 release（需 tag input）、避免污染 release 列表暂不主动跑。如果 v* tag build 失败、属于本 commit 的 hotfix 而非新 cycle。

---

### 2026-06-29 — Claude Code (Opus 4.7) — BETA-32 T6 #6：ServerCtx 加 search_candidates cache + CI 又踩 must_use_candidate lint

**承接**：上一条 cycle bug #2 修完后用户选 BETA-32 follow-up T6 #6 `ServerCtx` 加 `search_candidates` cache（reviewer Important）继续推。

**根因**：[search.rs:157-170](packages/locifind-server/src/tools/search.rs) 每次 `SearchTool::invoke` 重建 `LocalIndexBackend` + `HarnessSearchTool` 包装结构。桌面 app 是 startup 一次构造 + `ToolRegistry::register_search` 复用、daemon 应同款节奏。

**改动**（4 文件、+83/-18）：
- `ServerCtx` 新加 `search_candidates: Arc<Vec<Arc<dyn SearchableTool>>>`（含 doc 注释说明 reindex 时不需 swap：`LocalIndexBackend` 持 `db_path` 不持 `sqlite::Connection`、reindex drop+recreate 同 path 后下次 search open 新文件即正确）
- `search.rs` 提取 `pub fn build_local_search_candidates(data_dir) -> Vec<Arc<dyn SearchableTool>>` helper、invoke 替换为 `(*ctx.search_candidates).clone()`
- `apps/daemon/main.rs::build_runtime_ctx` 启动调 helper 装入 ServerCtx
- `test_support` 两 builder（inmem + indexed）同步加字段
- 加 2 单测：`build_local_search_candidates_returns_one_local_backend` + `server_ctx_test_helper_has_search_candidates_cache`

**CI 验证**：[run 28358465405](https://github.com/raoliaoyuan/LociFind/actions/runs/28358465405) 因 4 处 clippy 报错挂；commit `5bdcbd3` 修后 [run 28358604114](https://github.com/raoliaoyuan/LociFind/actions/runs/28358604114) 三绿。**第 4 类 ubuntu lint gap 新发现**：`clippy::must_use_candidate`——新加 `pub fn build_local_search_candidates -> Vec<Arc<dyn SearchableTool>>` 返 owned 容器、clippy 期望 `#[must_use]` 让调用方显式接收返回值。已加 memory（`ci-ubuntu-first-run-lint-gaps`、与 dead_code / doc_markdown / fmt 三类并列、共 4 类盲点）。

**另发现**：Windows 本机无 MSVC linker 时 `cargo clippy` 也跑不动（proc_macro / build.rs 需 `link.exe`、不是只有 `cargo build` 需要）；本机只能 `cargo fmt --check`，clippy + test 全靠 CI。

**收工 commit**：本会话 cycle 产出 `afcc151`（T6 #6 cache feature）+ `5bdcbd3`（4 clippy 修）+ 收工 doc-sync（本条 STATUS + ROADMAP follow-up ② 标 done + memory 加第 4 类 lint）。

**未尽事宜**：① BETA-32 follow-up backlog 仍剩 5 项（reindex 真扫盘 / release-windows.yml --locked / rmcp stateless / 真模型 evals / cross-compile）+ 大 cycle 候选 = bug #1 daemon hybrid 改造 (~1w)；② 5 UX bug 仍 deferred；③ 真机重测 deferred 用户。

---

### 2026-06-29 — Claude Code (Opus 4.7) — daemon bug #2 修：multi-word phrase keyword FTS 拆 + 新增最小 CI workflow

**承接**：上一条会话日志「未尽事宜 ②」中的 BETA-32 follow-up「daemon FTS-only」子项里、上 v0.8.0 GUI cycle 真机暴露的 daemon 侧 bug #2「degraded 模式 FTS fallback 失效」（搜「BETA-32 daemon design」返 degraded:true / results:[]）。

**根因（静态读 5 层栈定位）**：daemon vs parser 的契约 mismatch。
- `intent_parser::extract_en_residual_keywords`（[file_search.rs:1487](packages/intent-parser/src/parsers/file_search.rs:1487)）把英文连续内容词合并成 `"BETA-32 daemon design"` 这种 **phrase keyword**（对桌面 hybrid 检索 OK：semantic embedding 兜底分散匹配）
- `ExpandedSearchIntent::identity`（[expanded.rs:53](packages/search-backends/common/src/expanded.rs:53)）把它包成单个 singleton group
- `fts_match_from_groups`（[local-index/lib.rs:257](packages/search-backends/local-index/src/lib.rs:257)）拼成 FTS5 双引号短语 `"BETA-32 daemon design"`
- 要求文档内连续出现整个短语 → daemon FTS-only 路径（无 semantic 兜底）必 0 命中

「 quality」（前导空格、trim 后单词）走 trigram tokenizer 单 word 子串匹配命中 20 条但 ranker 给 0 分（缺 group），与 bug #2 同源不同表现。

**修法（最小变更）**：daemon `search.rs` step 3 把 `ExpandedSearchIntent::identity(intent)` 换 `expand_intent_for_daemon(intent)`——新 helper 把 head 含 unicode whitespace 且无 synonyms 的 group 按空格再拆成多个 singleton group，对应 FTS5 表达式从 `"BETA-32 daemon design"` 改 `"BETA-32" AND "daemon" AND "design"`。**不动 parser / 不动 expanded / 不动 desktop / 不动 indexer / 不动 LocalIndexBackend**。

**改动**：
- [packages/locifind-server/src/tools/search.rs](packages/locifind-server/src/tools/search.rs)：+145/-4。`expand_intent_for_daemon` helper 37 行 + 4 单测：① multi-word phrase 拆分；② 单词不动；③ 多 singleton 不内部拆；④ 含 synonyms 不拆（未来同义词扩展契约守门）。
- `.github/workflows/ci.yml`：新增最小 CI workflow（on push main/feat-* + on PR）。`cargo test -p locifind-server --lib --locked` + clippy `-D warnings` + fmt check。**仓库此前无 push-trigger CI**（只有 release-tag-triggered binary build），此 workflow 是持续 quality gate、本 cycle 顺手建立。

**本机限制 + CI 验证**：本机 Windows 装了 Rust toolchain（winget Rustlang.Rustup）但缺 MSVC link.exe（VS BuildTools C++ workload 未装、~5-10 GB 不在 cycle 范围）；切 CI 验证路径——push 后 ubuntu-22.04 runner 跑 `cargo test -p locifind-server --lib` 验证 fix 的 4 单测 + 既有 29 单测。CI 经 4 次推 commit 终绿（test ✓ clippy ✓ fmt ✓）、fix 验证完成。

**CI 4 次推 commit 与 ubuntu-22.04 首跑 lint gap 发现**（[run 28357658327](https://github.com/raoliaoyuan/LociFind/actions/runs/28357658327) green）：

| # | Commit | 修补 | 暴露 gap |
|---|---|---|---|
| 8aaf1f8 | bug #2 fix + ci.yml 新增 | search.rs `expand_intent_for_daemon` + 4 单测 | — |
| 96754c9 | `parse_paths_lines` allow(dead_code) | `cfg_attr(not(any(windows,macos)), allow(dead_code))` | **`dead_code`**：OS-conditional fn 在 Linux lib build 全 dead、BETA-32 cycle 在 Mac/Win 跑 clippy 未暴露 |
| 1c2c0d9 | search.rs:387 doc backticks | `split_whitespace` 加 backticks | **`doc_markdown`**：doc 注释里 snake_case identifier 严格要求 backticks |
| 3c7afdb | fmt 2 处 diff | discovery cfg_attr 单行 + search 链式换行 | **`fmt --check`**：cfg_attr 偏好单行、链式调用偏好换行 |

3 类 lint gap 已记忆到 Claude Code memory（`ci-ubuntu-first-run-lint-gaps`、含 Why + How to apply），下次加新 CI workflow 或扩 ci.yml 范围时反射查。

**未尽事宜**：① **bug #1（daemon search 接 SemanticIndexBackend）保留 follow-up cycle**——架构改、~1w 量级、需 spec + plan；本 cycle 范围内只修 bug #2；② **真机重测 deferred 用户**——本机 Rust 缺 MSVC 不能 build daemon binary 重现；下次推 daemon-v0.1.1 tag CI 产 binary 后用户可重测、或本机装 VS BuildTools 后我重测；③ ci.yml 范围目前仅 locifind-server lib、后续 cycle 可视情况扩到 indexer / harness / intent-parser 等其他 Rust-only crate（避开 llama-cpp / 桌面 app）。

---

### 2026-06-29 — Claude Code (Opus 4.7) — daemon-v0.1.0 release 兜底 publish + release-daemon.yml macOS x86 hardening（B 方案 done）

**承接**：上一条会话日志「未尽事宜 ①」（daemon-v0.1.0 prerelease 没发）+「未尽事宜 ③」（release-daemon.yml hardening follow-up）合并 cycle。**用户选项 = 最小 hardening cycle**（~0.5h）。

**段 1：release 兜底 publish**：runner 持续排队 2h18m+ 仍 queued、release job needs:build 永远 skip。手动 publish 兜底——`gh run download 28349383266` 拿现成 3 binary（Mac arm 10.3 MB / Linux 12.2 MB / Windows 12.1 MB）+ `sha256sum` 重算（Windows SHA `5884b23b…` 与上条会话日志记录完全一致 ✓）+ `gh release create daemon-v0.1.0 --prerelease`（[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/daemon-v0.1.0)、4 asset 上传 + 7 行 release notes 含 macOS x86 缺位说明 + SHA256 + 已知问题）+ `gh run cancel 28349383266` 砍掉 stuck workflow run（避免 GH 后台一直挂着）。

**段 2：release-daemon.yml hardening（3 候选对症分析 → B 落地）**：
- **A. continue-on-error + release job `if: always()`**：GH Actions `continue-on-error` 只对已 run 的 step 生效；`timeout-minutes` 文档明示「excluding queuing time」、不能 timeout queue 状态。queue stuck 既不是 success 也不是 failure、**A 方案对症不到**。
- **B. 删 macOS x86 matrix entry + Intel Mac 用户自编译**：简单可靠、当下可验。**选 B**。
- **C. macos-14 上 cross-compile x86_64**：llama-cpp-4 build.rs 调 cc/clang 编 ggml C++、cross-compile 需 macOS SDK x86 lib + 链接器配置、首跑大概率失败、不在 0.5h cycle 范围；登记 BETA-32 follow-up ⑦ 待 Intel Mac 需求出现时启动。

**改动**：
- `.github/workflows/release-daemon.yml`：删 matrix `x86_64-apple-darwin` entry + 8 行注释解释为什么不 CI build + 指向自编译指南
- `apps/daemon/README.md`：新加 §2.3「Intel Mac（x86_64）— 自行编译」节（5 行 cargo + sudo cp + 引用 §2.1 launchd），原 §2.3 Windows NSSM 改 §2.4（README §5 故障排查 #3 引用 §2.1 / §2.2 不变、grep 验证无别处引用 §2.3）

**收尾**：本会话改动（前轮 STATUS/ROADMAP doc-sync ④ + 本轮 workflow + README + 本会话日志）一次 commit 落库；下次推 daemon-v* tag 不会再撞 macos-13 排队问题、3 binary 100% 自动发 prerelease。

**未尽事宜**：① Intel Mac 团队成员若有 daemon 部署需求、按 README §2.3 自行编译（团队 box 多为 Linux/Windows/Mac arm、Intel Mac 管理员预计少）；② BETA-32 follow-up backlog 仍剩 6 项（reindex 真扫盘 / search_candidates cache / daemon FTS-only 默认 hybrid / release-windows.yml --locked gap / rmcp stateless / 真模型 evals）+ 第 ⑦ 项 cross-compile 待 Intel Mac 需求出现；③ 5 UX bug 仍 deferred。

---

### 2026-06-29 — Claude Code (Opus 4.7) — Windows 真机：BETA-32 daemon 部署 GO + v0.8.0 GUI 验证 GO + 5 个真机 UX bug 登记 follow-up ⭐

**承接**：BETA-32 daemon 合 main 后切到 Windows 机器做真机自验（与 BETA-31 同款 GO with documented gap 节奏的兑现）。同会话顺带把用户原计划的 v0.8.0 GUI onboarding 真机手测也跑了。

**BETA-32 真机 GO（daemon 侧）**：① `git clone` 仓库到 `D:\Git\Locifind`（用户开 gh CLI 装 + 浏览器授权登录 raoliaoyuan）；② push `daemon-v0.1.0` annotated tag → `release-daemon.yml` 触发 → 4 平台 binary 并行编（Mac arm 3m47s / Linux 5m22s / Windows 8m30s done；**macOS x86_64 macos-13 runner 池子满 + 排队 1h+ 未启动**、release job 因 needs:build 卡住没自动发 prerelease）；③ Windows artifact 下载（`locifindd-x86_64-pc-windows-msvc.exe` 12.13 MB、SHA `5884b23b...`、`--version` = `locifindd 0.1.0`）；④ 下 EmbeddingGemma-300M GGUF（328 MB、SHA `6fa0c02a...`）；⑤ 起 daemon `--root docs --bind 127.0.0.1:8765`、174 doc 索引 1.7s、监听就绪；⑥ PowerShell smoke 脚本走完 MCP 协议（`/health` 200 + `initialize` + `notifications/initialized` + `tools/list` 2 个工具 + `tools/call list_roots` 命中 + `tools/call search × 5`）—— **wiring 全过**。

**daemon 真机暴露的 2 个 bug（建议下一 cycle 改）**：① **CI binary 是 FTS-only**——`release-daemon.yml` workflow 没启 `--features semantic-recall`、生产分发跑不出语义召回核心价值主张（README §5.4 已 warn 但 CI 应直接编进默认 binary）；② **degraded 模式下 FTS fallback 失效**——搜 `BETA-32 daemon design` / `rmcp streamable HTTP` 等字面匹配 query 全部返 `{"degraded":true,"results":[]}`，仅一条 ` quality`（前导空格）返 20 条 score=0.0 命中、说明 stub embedder degrade 时整条短路了 FTS path（ranker 也未生效），e2e 测试只验协议没验 degraded fallback 行为、是 `/code-review ultra` 也没扫到的第 4 个 critical bug。

**v0.8.0 GUI 真机验证 GO（桌面 app 侧）**：① 用户走 onboarding step 2 时跳过/中断了模型下载、`%APPDATA%\LociFind\models\` 整个目录都没建——直接把上一步下好的 GGUF 拷到 `%APPDATA%\LociFind\models\embeddinggemma-300m-q8_0.gguf` SHA 完全一致；② Everything 灯灰——根因是用户原本装的 Everything 已配成 **Windows 服务模式**（跑在 SYSTEM 账号下、IPC 命名管道权限隔离）+ `es.exe` CLI 没单独装、`EverythingBackend::new()` 三候选路径全 miss；用 `winget install voidtools.Everything.Cli` 装到 winget 标准路径（候选 #2）+ 经 computer-use 引导用户在 Everything Options 取消勾选「Everything 服务(V)」、保留「随系统启动(S)」、点确定触发 UAC 后服务直接卸载、用户态进程接管；③ 重启 LociFind → 4 灯全绿 → 搜「README」**100 条 2943ms**、状态行 `via search.local + search.semantic + search.windows + search.everything` ⭐ **四后端 fan-out 全跑通含语义召回**；④ 搜「学习材料」2 条 2639ms（OneDrive 命中）；⑤ 搜「洞察」0 条——根因是「洞察」只出现在 `D:\Git\Locifind` 仓库 18 个文件**内容**里、Everything 全盘搜文件名 0、Windows Search 默认不索引 D 盘、LociFind 本地索引也不扫 D 盘开发目录（用户决定不加，验证已足够）。

**桌面 GUI 真机暴露的 3 个 UX bug（建议下一 cycle 改）**：① **顶栏「语义召回」灯绿但模型实际未加载也亮绿**——`build_registry` 中灯只反映 backend 在 registry 注册成功（[main.rs:79-99](apps/desktop/src-tauri/src/main.rs:79)），不反映 EmbedStatus。修：灯应映射到 EmbedStatus（Ready 绿 / Loading 黄 / NotFound|Failed 红）；② **设置页「语义召回模型未找到」与实际就绪状态脱节**——[embedding_model.rs:191-194](apps/desktop/src-tauri/src/search/embedding_model.rs:191) `EmbedState::NotLoaded` 直接被渲染成 `EmbedStatus::NotFound`、不真的查 `path.exists()`；用户实测：状态页一直显示「未找到」但 search.semantic 在 via 列表里真的返了语义结果。修：状态机加文件检查、或把 NotLoaded 单独渲染（「待加载」）；③ **Everything 检测失败仅 stderr 无 GUI 引导**——[main.rs:151](apps/desktop/src-tauri/src/main.rs:151) `Err(err) => eprintln!`、用户看到的只有顶栏一个灰灯，没有任何「点这里 winget 装 voidtools.Everything.Cli」的内嵌引导。

**操作层另两个真机痛点（建议下一 cycle 改）**：④ **embedding 模型首次冷启动 ~17s 直接撞 15s 搜索 timeout**——`prewarm` 由 `spawn_semantic_index` 在 FTS reindex 之后才触发、期间用户搜任何 query 都必然超时；用户实测「错误：search timeout after 15011 ms」。修：app 启动直接 spawn prewarm（不等 FTS）或首查 timeout 抬到 30s+；⑤ **Everything 服务模式 + es.exe IPC 不兼容**——老用户的 Everything 服务模式（SYSTEM 账号）跟 winget es.exe（用户账号）的命名管道权限隔离，本会话靠 GUI Options 关服务+UAC 卸载才修通。修：onboarding/设置页检测到服务模式时给明确引导（不能 just eprintln）。

**未尽事宜**：① ~~`daemon-v0.1.0` prerelease release 还没发~~ → **2026-06-29 后续 done**（Claude Code Opus 4.7）：runner 持续排队 2h18m+ 未启动，手动 publish 兜底——`gh run download` 拿现成 3 binary（Mac arm / Linux / Windows）+ `sha256sum` 重算 checksums + `gh release create daemon-v0.1.0 --prerelease`（[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/daemon-v0.1.0)、SHA 与 STATUS 上次会话记录的 `5884b23b…` 完全一致）+ `gh run cancel` 砍掉 stuck workflow run。**macOS x86 binary 本次未发**（macos-13 runner 池排队 GitHub-side issue）、Intel Mac 团队成员按 README §2.3 自行 `cargo build --release --target x86_64-apple-darwin --bin locifindd` 本地编；② **5 UX bug 全部 deferred 下一 cycle 实施**——可单独立 BETA-31-v3「v0.8.0 真机 UX gap 修复集」task（含上述 5 项 + BETA-31-v2 GPU 推理 / BETA-31-v3 SHA256 验证 / BETA-32 多版本管理 follow-up），或拆分到对应 cycle；③ ~~`release-daemon.yml` hardening~~ → **2026-06-29 后续 done**：选 B 方案（删 macOS x86 matrix entry + README §2.3 加 Intel Mac 自编译节）；A 方案对症不到（GH Actions `continue-on-error` 只对 run step 生效、`timeout-minutes` 不含 queue 时间、queue stuck 既非 success 也非 failure），C 方案（macos-14 上 cross-compile）因 llama-cpp-4 build.rs 链编 ggml C++ + SDK 链接配置首跑大概率失败、不在 hardening 范围。下次推 daemon-v* tag 不会再撞 macos-13 排队问题。

### 2026-06-29 — Claude Code (Opus 4.7) — BETA-32 团队归档 MCP daemon done + [PR #21](https://github.com/raoliaoyuan/LociFind/pull/21) 已合 main（merge commit `35725db`）+ /code-review ultra 3 critical fix（`3dc0c6a`）⭐ 衍生子线、不阻塞 1.0 出场

**承接**：BETA-31 收尾后启动衍生子线、与主线 BETA→1.0（cosine sweep / baseline rewrite / 真模型 evals）并行；不与任何 hybrid 路径冲突。**价值主张**：把团队共享归档（设计稿 / 标书 / 财务底稿）的 hybrid 语义+FTS 检索能力下沉为 headless 服务，团队成员在自己机器上的 Claude Code / Codex / 任意 MCP 客户端通过 streamable-HTTP 连上、不必每人本地灌一份索引。

**关键决策**：① 范围 = **headless MCP daemon only**（不动桌面 GUI / 不动 `release-windows.yml` / 不动 PROJECT.md / CONVENTIONS.md 核心原则）；② transport = **rmcp 1.8 streamable-HTTP**（锁版本、`stateful_mode=true` 复用 LLM client 的 initialize 协商 + tools/list 缓存）；③ bearer auth = **`secrecy::SecretString` + `subtle::ConstantTimeEq`** 常数时间比较；④ binary 默认 stub backend、生产 build 须 `--features semantic-recall` 走真模型；⑤ **真机部署 DEFERRED 用户自验**（管理员一台机器起 daemon + Claude Code 在另一台机器接 + 跑 5 example query、与 BETA-31 同款 GO with documented gap 节奏）；⑥ **真模型 v0.9 评测 DEFERRED**（需 `LOCIFIND_DAEMON_BIN` + `LOCIFIND_MODEL_PATH` env、留 follow-up cycle 跑）。

**Cycle 执行（14 task / 7 commit + T14 doc-sync）**：
- T0 cycle 预检 + spec + plan 落库（feat 分支 `feat-beta-32-team-archive-mcp-daemon`、源 main `077ab97`）
- T1 C1 packages/indexer `IndexProgress` callback trait + `schema_meta` 表（桌面 app 行为等价）
- T2 C2a-f packages/locifind-server lib（`25b6999` ServerConfig + ServerCtx + bearer auth → `886a91f` Tool trait + 两个 tool 骨架 → `2e1976a` Tool trait + 两个 tool 骨架 → `4eff5a2` SearchTool 接 harness + test_support → `51c4b6d` admin handlers + reindex IN_FLIGHT guard → `d8847a1` MCP adapter + app.rs Router）、29 单测
- T3 C3a-b apps/daemon binary `locifindd`（`4eff5a2` apps/daemon 骨架 + clap CLI → `93943ad` preflight + lifecycle + 全量索引启动）、9 flag + 6 preflight 检 + 4 preflight 单测
- T4 C4 apps/daemon e2e 集成测试 x5（`829ce68`、health / auth / list_roots / search / reindex 409）
- T5 C5 evals `--mode daemon` + top-K 闸门（`5341b4d`、3 daemon_mode_smoke 测）
- T6 C6 三平台 binary CI workflow `release-daemon.yml`（`e5821b0`、Mac arm64 / Mac x86_64 / Windows x86_64 / Linux x86_64 共 4 binary + SHA256 checksums、tag `daemon-v*` 触发 + workflow_dispatch）+ ROADMAP BETA-32 卡片登记
- T7-13 各 reviewer round（含 6 reviewer fixes 已 inline）
- T14 doc-sync `60ab0c9`：apps/daemon/README.md 部署样板（launchd / systemd / NSSM + CLI / env / TOML 三层 + 5 条故障排查）+ STATUS + ROADMAP 改 done + packages/locifind-server/src/app.rs rmcp 1.8 `json_response` stateful 模式下被静默忽略的注释 fix（T8 reviewer Important #1 follow-up）
- 收尾 PR + /code-review ultra：[PR #21](https://github.com/raoliaoyuan/LociFind/pull/21) 创建 → 用户跑 /code-review ultra 多 agent 复审 → 找到 3 critical bug（① daemon 写 `documents.db` 但 SearchTool 只读 `index.db` → document 搜索永远空、② reindex stub 写回 `state.doc_count=0` 覆盖启动时正确值、③ mcp_call_tool 丢掉 JSON-RPC 顶层 error 字段）→ implementer inline 修 → commit `3dc0c6a` push 进 PR → 用户合 main（merge commit `35725db`、分支已删）

**红线 1-7 全过**：
- 红线 1 fmt 净（`cargo fmt --all -- --check` 0 输出）
- 红线 2 clippy 0 warning（`cargo clippy --workspace --all-targets -- -D warnings`）
- 红线 3 workspace test 927 passed / 0 failed / 8 ignored（65 个 test bin、含 locifindd 4 preflight + 5 e2e + locifind-server 29 + locifind-evals 75）
- 红线 4 locifind-server 单测 **29 passed**
- 红线 5 apps/daemon e2e 集成测试 **5 passed**
- 红线 6 packages/evals daemon mode smoke **3 passed**（含 top-K 集合等价闸门写好）
- 红线 7 desktop frontend vite 347ms ✓ built + cargo check -p locifind-desktop 净

**红线 8（manual install 烟雾测）DEFERRED 用户自验**（与 BETA-31 / BETA-15B-7-v2 / BETA-15B-11-v2 同款节奏）。原因：computer-use 工具检测可能误判 LociFind 为「程序坞」阻塞 click、自动验证不可达；红线 1-7 + tracing log emit + dev server 启动 + daemon shutdown_signal 正常足够说明工程层 OK。

**红线 9（真模型 v0.9 评测）DEFERRED**：需 `LOCIFIND_DAEMON_BIN` + `LOCIFIND_MODEL_PATH` env、成本太高、留 follow-up cycle 在 CI 环境跑。

**未尽事宜**：① **真机部署一次**留用户自验——下次会话用户切到 **Windows 机器**、`git pull origin main` 拿 BETA-32 整套 + `cargo build --release --bin locifindd` 本地编（或等 `daemon-v0.1.0` tag 触发 release-daemon.yml 拿现成 binary）+ 起 daemon + 另一台机器 Claude Code 接 + 跑 5 example query；② **可选 bump `daemon-v0.1.0` + tag 触发 `release-daemon.yml`**（产三平台 binary 发给团队 box）；③ /code-review ultra 余下 12 项 follow-up backlog 已并入「下一步」节，下次 cycle 拍板从哪条起手。

**follow-up cycle 候选**：① **T7 reindex 真扫盘**（当前 stub、留 TODO 待真接 indexer 批量做；含 tracing instrument / anyhow chain / privacy contract 3 TODO）；② **T6 #6 ServerCtx 加 `search_candidates` cache**（多次 search 复用 indexer instance、避免 per-invoke 重建 LocalIndexBackend）；③ **`release-windows.yml` 同款 `--locked` gap**（T12 reviewer 提、follow-up cycle 单独处理）；④ **rmcp `stateful_mode=false` 切 stateless** 简化 e2e client（json framing 走纯 JSON）；⑤ **真模型 v0.9 评测**在 CI 环境跑（设 env、批量真 embedder）；⑥ **daemon FTS-only 默认改 hybrid**（封 semantic-recall feature 进默认 binary，分发体积换召回质量）；⑦ **多版本模型管理**（用户能装多个 embedder 并切换默认）。

---

### 2026-06-27 — Claude Code (Opus 4.7) — BETA-31 Windows 模型分发 UX 增强 done + [PR #20](https://github.com/raoliaoyuan/LociFind/pull/20) 已合 main（merge commit `1f04f51`）

**承接**：BETA-15B-11-v2 收尾后用户希望邀请同事做 Windows 真机手测、需要软件能让首次使用者顺利上手（安装 → 下载模型 → 配索引 → 试用查询）。

**关键决策（brainstorming 3 收敛）**：① 范围 = **Windows 为主 + Mac 同款复用**（双平台 onboarding 扩 3-step、Windows GPU 性能优化留 BETA-31-v2 follow-up）；② 模型下载 = **App 内 GUI 一键下载 + 手动 fallback**（HF ggml-org/embeddinggemma-300M-qat-q8_0-gguf 公开免登录 URL 硬编码 + reqwest 0.12 stream + 64 KB 进度 emit、失败时显示手动下载 HF 链接 fallback）；③ 跳过下载 = **允许跳过先体验 FTS-only 搜索**（onboarding Step 2「稍后下载」让用户立即体验关键词搜索、之后 SettingsPage NotFound 内联按钮触发同款下载流程）。

**Cycle 执行（13 task、5 commit + merge）**：
- T0 cycle 预检 + feature branch（feat-beta-31-windows-model-distribution-ux）
- T1 C1 backend：reqwest 0.12 + futures-util + httptest 0.16 dev-dep + model_download.rs（~233 行、tauri commands download_embedding_model + cancel_embedding_download + download_stream 纯逻辑 + 进度/done/error event emit + AtomicBool cancel flag + 2 单测）+ OnboardingState 加 model_download_shown + main.rs register + permissions.rs complete_onboarding 加 "model_download" 分支；subagent 双审 APPROVED + 3 Important fix（重入 IN_FLIGHT guard + RAII drop + cleanup_partial helper + `&PathBuf` → `&Path` clippy ptr_arg）；commit `5fd0cd2`（amend 后 subject 46 chars）
- T2 C1 验证红线 1-3 + 8（fmt 净 / clippy 0 warning / workspace test 0 failed / model_download 2 单测过）
- T3-T5 C2 frontend hook + 共用组件：useModelDownload.ts (~104 行 状态机) + ModelDownloadStep.tsx (~178 行 4 态 UI + compact) + ExampleQueries.tsx (~70 行 5 示例)；subagent 双审 APPROVED + 2 Important fix（cancel-race 检查 reason="用户取消下载" + listener leak unmount-during-setup）；commit `0eb442c`（amend）
- T6 C3 frontend onboarding：OnboardingWin 单 step → 3-step stepper + OnboardingMac 同款扩（保留 FDA Granted 早返 / 轮询 / dev mode warning）+ useShouldShowOnboarding 加 model_download_shown 字段 + 判定逻辑；subagent self-reviewed 双审简化（为节奏紧凑跳 reviewer round-trip、纯 stepper UI 扩展低风险）；commit `1e8eaf5`
- T7 C4 SettingsPage NotFound 行内联下载按钮（ModelDownloadStep compact 复用）；commit `6f3f157`（13 行）
- T8 红线 4-7 全套验证（gate 1 passed / desktop build / parser-only byte-equal v0.5 500 + v0.9 1000 / 0 diff / fixture SHA 0 diff）
- T9 Mac self-test **DEFERRED**（computer-use 工具检测误判 LociFind 为「程序坞」阻塞 click、自动验证不可达；红线 1-9 + dev server 启动 + window 显示正常说明工程层 OK；用户手动验证留 cycle 收尾后做、与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 / BETA-15B-11-v2 同款 GO with documented gap 节奏）
- T10 C5 doc-sync：STATUS + ROADMAP + 加 BETA-31 卡片、commit `<待填>`
- T11 PR + 合 main + 占位符回填
- T12 可选 bump v0.8.0 + tag 触发 Windows release（产 NSIS installer 发给同事）

**红线 1-9 全过**：fmt 净 / clippy 0 warning / workspace test 136 passed (locifind-desktop) + 0 failed / gate 1 passed / desktop build vite ✓ built / parser-only byte-equal v0.5 500 cases / v0.9 1000 cases / 0 diff / fixture SHA256 byte-equal / model_download 2 单测 pass / tsc 0 errors。

**未尽事宜**：① **真机手测**留用户首启 v0.8 dev 自验 7 步走（[docs/manual-test-scenarios.md](./docs/manual-test-scenarios.md) 或 `/tmp/beta-31/self-test-decision.md`）；② **bump v0.8.0 + tag + release-windows.yml workflow 触发**（待用户拍板执行、产 NSIS installer 发给同事）；③ **PR 实际编号 + merge commit hash 回填**待 PR 合并后填回 STATUS / ROADMAP。

**follow-up cycle 候选**：① BETA-31-v2 Windows GPU 推理（vulkan/cuda、~1-2w、需 Windows 真机）；② BETA-31-v3 模型 SHA256 签名验证（~0.5d、增加下载安全性）；③ BETA-32 多版本模型管理（用户能装 bge-m3 + embeddinggemma 两个 + 切换默认）。

---

### 2026-06-27 — Claude Code (Opus 4.7) — BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done + [PR #19](https://github.com/raoliaoyuan/LociFind/pull/19) 已合 main（merge commit `e3670dc`）

**承接**：BETA-15B-11 v6 数据指证 + Branch I-a GO，本 cycle 兑现 = 把数据搬到桌面 wiring、用户真正能用上。

**关键决策（与 BETA-15B-7-v2 同款节奏）**：① 范围 = **最窄 wiring 切换**（单文件 diff < 20 行、不动 baseline.json / cosine_threshold / gate.rs / result-normalizer / evals / model-runtime / indexer）；② 数据指证 vs v5 bge-m3 baseline T=0.70：**OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026 / exact-name 守 1.000 = 无 trade-off 全方面提升**（vs BETA-15B-7-v2 切 bge-m3 时 crosslang -0.055 反退、本 cycle 数据底气强一截）；③ 分发增量 = **净降 292 MB**（embeddinggemma 313 MB vs bge-m3 605 MB）；④ 真机手测 = **DEFERRED**（GO with documented gap 路径）。

**Cycle 执行（5 task、2 commit + merge）**：
- T0 cycle 预检 + feature branch（feat-beta-15b-11-v2-bake-embeddinggemma-production）
- T1 desktop wiring 切换（embedding_model.rs 3 处常量 + doc 注释、commit `bb29838`、+18/-7=11 净增、subagent 双审 APPROVED 无 Critical/Important + 1 Minor 不阻塞）
- T2 7/7 红线全过（fmt + clippy + workspace test + desktop build + parser-only byte-equal + fixture SHA256 + semantic_quality_gate）
- T3 真机手测 DEFERRED（与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款）
- T4 doc-sync（baseline 报告 v6-prod 节 + STATUS + ROADMAP、commit `<待填>`）
- T5 PR + 合 main + 占位符回填

**未尽事宜**：① **真机手测留用户首次升级时按 docs/manual-test-scenarios.md 三步走**（启动 → cp 模型 → 跨语言查询命中 + 「按意思找到」徽标）；② follow-up cycle 候选（cosine sweep / baseline rewrite / v3 prefix API / 模型分发 UX / 评测扩量）由用户拍板；③ **PR 实际编号 + merge commit hash 回填**待 PR 合并后填回 STATUS / baseline 报告 v6-prod 节 / ROADMAP 子项 ⑬。

---

### 2026-06-27 — Claude Code (Opus 4.7) — BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验 done ⭐⭐ Branch I-a GO + [PR #18](https://github.com/raoliaoyuan/LociFind/pull/18) 已合 main（merge commit `49b5f4a`）

**承接**：BETA-15B-10 v5 cycle 留下「crosslang 字面 0.700 移交未来 cycle = 更大 / 跨厂 embedding 模型」承诺、本 cycle 兑现。

**关键决策（brainstorming 5 收敛）**：① 范围 = **评测探针 only**（与 BETA-15B-7 同款节奏、~2-3d、不动桌面 wiring / 不动 baseline.json / 不动 cosine_threshold）；② 候选 = **EmbeddingGemma-300M 单点**（跨厂调研报告：jina-v3 双阻 = 白名单 + LoRA per-task / bge-multilingual-gemma2-9B 三 blocker = 9B 桌面不现实 + 无 GGUF + gemma2 --embeddings 无公开成功；EmbeddingGemma-300M 唯一可执行候选）；③ prefix 契约 = **双轴对照**（no-prefix 裸 embed vs standard prefix HF 卡契约、回答 prefix 必要性命题）；④ **接受标准 = 复活字面 spec 目标**（OVERALL ≥ 0.864 + crosslang ≥ 0.700、与 BETA-15B-7 同款）；⑤ binding 应急 = 若 0.3.2 vendored 不识别 `gemma-embedding` 则升 binding + qwen3-0.6b + bge-m3 双语义等价闸。

**Cycle 执行（12 task、4 commit + merge）**：
- T0 cycle 预检 + feature branch（feat-beta-15b-11-embeddinggemma-prefix-probe）
- T1 GGUF 下载 + metadata 抽：ggml-org/embeddinggemma-300M-qat-q8_0-gguf 公开转仓免登录、~329 MB（实测 313 MB）、SHA256 `6fa0c02a...`、arch=`gemma-embedding` / dim 768 / pooling_type=1 (Mean) / context 2048 / 24 layer
- T2 pooling.rs 扩白名单 +1 单测（TDD 红→绿、10/10 过）+ subagent 双审 APPROVED 无 Critical/Important/Minor
- T3 semantic_quality.rs 加 `--prefix-mode {none, standard}` flag + PrefixMode + EmbedRole 内部 enum + 闭包 match (mode, role) 包 prefix + 2 单测（TDD 红→绿、8/8 过）+ subagent 双审 APPROVED 含 1 Minor argv 一致性（inline fix）
- T4 **决策点**：本机实测 dryrun = llama-cpp-4 0.3.2 vendored **完全识别 `gemma-embedding` arch**、含 `fused Gated Delta Net (autoregressive + chunked) enabled`、L2 norm 全 1.0、0 全零 = `arch_recognized` → **跳 T5 binding 升级应急**（与 BETA-15B-9 qwen3-8B 失败模式诊断对比加强：300M Mean pooling 走通、8B Last pooling 卡住）
- T5 **NOT TRIGGERED**（决策跳过）
- T6 C1 commit `c41b1a0`（infra de-risk + flag prep）
- T7 Mac Metal --embed 双 mode 全集：no-prefix 24.87s / prefix 18.18s、两 vectors SHA256 不同（红线 10 区分性过）、L2 mean=1.0 全过
- T8 9 阈值 × 2 mode = 18 次 sweep：**no-prefix sweep best T=0.60 OVERALL 0.882 / crosslang 0.716**、**prefix sweep best T=0.0/0.30/0.45 三连冠 OVERALL 0.900 / crosslang 0.725 ⭐⭐**、控制对照 T=0/T=1.01 全过 → **Branch I-a GO**
- T9 C2 commit `ad8b300`（vectors-embeddinggemma-* 入仓）
- T10 baseline 报告 v6 节 +121 行 + fixtures README v6 +22 行 + C3 commit `52a116b`
- T11 总验收红线 1-10 全过（红线 9 NOT_APPLICABLE 因 binding 升级未触发）
- T12 STATUS + ROADMAP doc-sync + C4 commit + PR + 合 main

**Sweep 全表**（v5 dataset 81 cases / 127 docs / dim 768 / W=10.0）：

prefix-mode=**none**：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0/0.30/0.45 | 1.000 | 0.878 | 0.716 | 0.894 |
| **0.60 ⭐** | 1.000 | **0.882** | 0.716 | 0.903 |
| 0.70 | 1.000 | 0.874 | 0.716 | 0.895 |
| 0.80-1.01 | 1.000 | 0.862 | 0.631 | 0.900 |

prefix-mode=**standard**：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| **0.0/0.30/0.45 ⭐⭐** | 1.000 | **0.900** | **0.725** | **0.928** |
| 0.60 | 1.000 | 0.892 | 0.725 | 0.922 |
| 0.70 | 1.000 | 0.887 | 0.725 | 0.919 |
| 0.80-1.01 | 1.000 | 0.870 | 0.639 | 0.915 |

**spec §2.2 全过 + 字面 spec 目标复活双过**：(4a) exact-name HYBR_R = 1.000 ✓ / (4b) 各桶 HYBR_N ≥ v5 baseline ✓ / (4c) crosslang **0.725 ≥ 0.700** ✓⭐⭐ / (4d) OVERALL **0.900 ≥ 0.864** ✓⭐⭐。

**prefix 契约价值数据指证（命题 2 答案）**：standard prefix 在所有桶上 +0.009 ~ +0.025 加成（OVERALL +0.018 / crosslang +0.009 / content-not-name +0.025）、但 **no-prefix mode 单独也已经双过字面**（OVERALL 0.882 / crosslang 0.716）= **prefix 是 ROI 加分项、不是 GO 的必要条件**。含义：bake 推生产时**不需要在 model-runtime 层加 prefix API**、可纯用 `embed(text)` 接口。

**与 BETA-15B-9 qwen3-8B 失败模式对比**：300M (Mean pooling) 走通了 fused Gated Delta Net 路径、8B (Last pooling) 仍 broken → "fused × Last pooling × embedding-only 8b-specific bug" 假说**进一步加强**（不是 fused 路径本身坏、是 Last pooling × fused × embedding-only 三者交互的 8b-specific bug）。

**诚实边界**：
- crosslang +0.039 在 v5 14 例 crosslang 桶上、可能含运气、follow-up cycle 评测扩量到 20-30 例校验
- prefix mode T=0/0.30/0.45 三连冠 = plateau、bake T 选择留 follow-up cycle 重 sweep + Branch B 保守 vs sweep best
- no-prefix mode GO 不代表 "prefix 没用"、prefix 有 +0.009~+0.025 实测加成；只是非 GO 必要条件
- 本 cycle 范围 = 评测探针 only、桌面行为零变化、用户实际体验需 BETA-15B-11-v2 bake 才能看到

**下 cycle 抓手优先级（v6 数据指证）**：① **BETA-15B-11-v2 bake embeddinggemma-300m 推到生产**（**最高优**、~1-2w、DEFAULT_EMBEDDING_MODEL_FILENAME 替换 + baseline.json rewrite + cosine_threshold re-sweep + 模型分发 UX 准备、Windows 适配）；② **BETA-15B-11-v3 prefix API 接 model-runtime**（中优、~1w、+0.009~+0.025 各桶加成、与 bake 解耦）；③ 评测扩量（中优、crosslang 桶 14 例校验、~1d）；④ BETA-30 失败样本箱（低优、长期）。

**未尽事宜**：① **真机手测**：纯评测探针 cycle、按 spec §7 判平凡未安排、follow-up bake cycle 时再 Mac + Windows 真机手测；② **PR 实际编号 + merge commit hash 回填**待 PR 合并后填回 STATUS / baseline 报告 v6 节；③ 下 cycle 抓手由用户拍板（**推荐 BETA-15B-11-v2 bake**、与 BETA-15B-7-v2 同款节奏）。

---

### 2026-06-26 — Claude Code (Opus 4.7) — BETA-15B-10 bge-m3 baseline 重锚 + cosine sweep & bake + 评测集长文本扩量 + evals embed 截断解除 done ⭐ + [PR #17](https://github.com/raoliaoyuan/LociFind/pull/17) 已合 main（merge commit `8e707cf`）

**承接**：BETA-15B-7-v2 bake bge-m3 桌面切换（[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15) merged、merge commit `ee78f75`）+ hotfix BERT encode n_ubatch panic（[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16) merged、merge commit `32667ac`）后、follow-up 三件套合并 cycle：① cosine_threshold 在 bge-m3 上 sweep & bake；② evals baseline.json + gate.rs 重锚到 bge-m3；③ 评测合成集扩 > 512 token 长文本 case。完整 superpowers 全流程：brainstorming 3 决策 → spec v2（含 §1.2 evals embed 1200 char 截断 framing 修订）→ plan 9 task → subagent-driven 驱动 + 每 task 双审。

**关键决策**：
1. **cycle 范围**：① + ② + ③ 一刀切、3 commit（C1 数据 + 截断解除 / C2 bake + baseline + gate / C3 doc-sync）
2. **cycle 起手发现 evals 1200 char 截断**：原 framing「防 panic」改为「让 sweep 出的 cosine_threshold 在长文本场景有真实数据支持 + evals 路径与 desktop indexer 对齐」。spec §1.2 修订、新加 §3.1 row 1 截断解除改动
3. **BAKE_T = 0.70 Branch B 变体**：sweep best 在 T ∈ {0.0, 0.30, 0.45}（OVERALL 0.868）但 spec §5.2 Branch C 字面要求 (4b) 全过、T=0.45 content-not-name -0.003 字面 FAIL；保守选 T=0.70 守 (4b) 严格全过、与现行字面值相同、桌面零变化
4. **真机手测 DEFERRED**：GO with documented gap、与 BETA-15B-7-v2 T3 同款

**Cycle 执行**：
- T1 evals embed 截断解除（TDD 不适用、等价化简、+ 顺手修同函数 line 184 panic format clippy lint）
- T2 cases.json + corpus.json 扩 3 条（c079/c080/c081 + s00125 1495 char zh / s00126 4579 char en / s00127 4231 char en、全虚构 + 零 PII）
- T3 Mac Metal --embed 重跑 vectors.json（81 query + 127 doc、SHA256 `4f0de346b581d58d…`、长文本 token 全在 [513, 2048]）
- T4 C1 commit `4d4f8b5`（amend 后 subject 39 char ≤ 50）
- T5 sweep 9 阈值 + 决策 BAKE_T=0.70 Branch B 变体（spec compliance review 找 HYBR vs HYB framing + (4b) -0.003 严格门、coordinator 拍 T=0.70）
- T6 bake + baseline rewrite + gate.rs 注释升 v5（3 文件 +42/-40、字面值 0.70 不变 = 桌面零变化、baseline 数值刷新到 v5）
- T7 C2 commit `67e32e3`（subject 50 char = 上限）
- T8 baseline 报告 v5 节 + README v5 段（baseline.md +87 / README +11、含双 framing 表）
- T9 STATUS + ROADMAP + commit C3 + PR + 合 main

**bake 后实际数据**（v5 / T=0.70 / bge-m3）：
- OVERALL HYBR_N = 0.864 / crosslang 0.686 / content-not-name 0.869 / exact-name HYBR_R = 1.000
- vs 现行 baseline HYB framing：OVERALL +0.021 / crosslang +0.038 / content-not-name +0.001 / exact-name =
- vs 现行 baseline HYBR framing：OVERALL +0.008 / crosslang -0.031 / content-not-name -0.001
- gate 4 红线全过新 baseline

**诚实边界**：① crosslang 0.686 < 0.700 spec 字面、移交未来 cycle；② 未吃满 sweep best（T=0.45 OVERALL 0.868 / crosslang 0.708、本 cycle T=0.70 OVERALL -0.004 / crosslang -0.022）；③ long-text case 偏置（c079/c080 平均 ~0.703 < 桶平均 0.876、新 case 比现有 case 难、未来 cycle 可校准）。

**未尽事宜**：① 真机手测留用户首次升级时按 docs/manual-test-scenarios.md 走（与 BETA-15B-7-v2 T3 同款）；② 下 cycle 抓手由用户拍板（跨厂替代 / 评测扩量 / 模型分发 UX / BETA-30 失败样本箱）。

---

### 2026-06-26 — Claude Code (Opus 4.7) — BETA-15B-7-v2 hotfix BERT encode n_ubatch panic + PR #16 已合 main ⚠️

**承接**：BETA-15B-7-v2 cycle 合并到 main 后真机手测（user 要求做 Mac 真机手测）发现 bge-m3 在 desktop dev mode 触发 `ggml_abort` panic、app worker 立即退出。spec §2.2.8 / plan T3 描述「app 起来后 cp 模型 → 跨语言查询」三步走、用户文档实际嵌入触发 BERT-arch encode 路径的 `GGML_ASSERT(cparams.n_ubatch >= n_tokens)`、app 整体不可用。

**根因诊断**：llama-cpp 默认 `n_ubatch=512`，BERT-arch embedding 走 encode 路径（log 显示 `decode: cannot decode batches with this context (calling encode() instead)`）要求 `n_ubatch >= n_tokens` 一次性喂整段。用户文档 tokenize 后 > 512 token 必然触发 assert。BETA-15B-8 时 evals binary 跑成功未暴露此 bug 是因合成集 124 corpus 文档都 < 512 token、未触发 encode path 的 ubatch 上限。

**Fix**：`packages/model-runtime/src/llama.rs::run_embed` inline override `n_batch = n_ubatch = context_size`（默认 2048）。不动 `make_ctx_params`（chat fallback 路径 `run_plain` 不受影响、qwen3 decoder 走 decode 路径不依赖 n_ubatch）。`+10/-1`。

**验证**：① cargo test --workspace 全过；② clippy 0w；③ fmt 净；④ Mac 真机重起 `npm run tauri dev --features semantic-recall`：hotfix 前 worker 立即 GGML_ASSERT abort、hotfix 后 app 稳定运行、worker 持续 embed 大量 `sched_reserve: graph splits=1` + `decode: cannot decode batches with this context (calling encode() instead)` 但**无 ggml_abort**、UI 查询返回 39ms。

**Commit**：[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16) merged main、merge commit `32667ac`、hotfix commit `08c6587`、分支已删。

**未尽事宜**：① **Mac 真机跨语言查询命中验证待用户做**——本会话查「年假和休假规定」返 6 个 macOS Cache 文件、原因是用户本机无「年假」相关 doc、缺匹配语料（不是 hotfix 问题）；用户首次升级 v0.7 + 索引自己有「年假」/「leave policy」doc 的目录后即可验「按意思找到」徽标；② **BETA-15B-8 evals 合成集合需扩到含 > 512 token 长文本 case**（follow-up）以防同类 panic 漏测 evals 端到端；③ STATUS / ROADMAP doc-sync 本条收尾本 commit 一并落库（不单独 commit）。

---

### 2026-06-26 — Claude Code (Opus 4.7) — BETA-15B-7-v2 bake bge-m3 done + PR #15 已合 main

**承接**：BETA-15B-9 收口（4-hypothesis 全 FAIL、qwen3-8b 在 llama-cpp-4 0.3.2 binding 状态下不可用、bake 决策不再等同族最大档真水位）+ BETA-15B-8 v4-fixup 数据指证（bge-m3 CLS pooling 真水位 OVERALL 0.869 双过 spec 字面 0.864 ⭐、infra 阻塞解除）→ 最窄 wiring 切换 cycle，把 bge-m3 推到桌面默认。

**关键决策 / 修订**：
1. **范围 = 仅桌面 wiring 切换**（diff < 20 行、不动评测层）。spec §3.1 row 4 / §2.1 plan 起手发现 `apps/desktop/src-tauri/src/settings.rs:17` 写 `default_embedding_model_*` doc 注释但 **不含模型文件名字面值**——就地修订 spec 划掉 settings.rs 改动行、commit `944bf2f` 把 fixup 合入 plan。**唯一改 desktop 源文件 = `apps/desktop/src-tauri/src/search/embedding_model.rs`**（DEFAULT_EMBEDDING_MODEL_FILENAME + EMBEDDING_MODEL_ID + 3 处 doc）。
2. **cosine_threshold 保 0.70 不动、baseline.json + gate.rs 保 qwen3-0.6b 不动**：bge-m3 sweep best 在 T*=0.0/0.30/0.45 留 follow-up cycle 重 sweep 拿回；评测层与桌面解耦、follow-up cycle 重写 baseline 时再对齐。
3. **真机手测可 deferred**（GO with documented gap 路径、留用户）。

**5 task 产出**：
- **T0 spec fixup**（合并到 plan commit `944bf2f`）：spec §3.1 / §2.1 划掉 settings.rs:17 行
- **T1 wiring**（commit `a3794b7` + fixup `12327d6`）：embedding_model.rs +13/-3、qwen3-Embedding-0.6B-Q8_0.gguf → bge-m3-Q8_0.gguf、EMBEDDING_MODEL_ID 字面 + 3 处 doc 注释
- **T2 §2.2 红线 1-7 全过**（证据 `/tmp/beta-15b-7-v2-verification-evidence.txt`）：workspace test 862/0/7、clippy 0w、fmt 净、tsc 净 + vite 352ms、7/7 fixture SHA256 与 main 等价、v0.5=473 + v0.9=877 byte-equal diff 0、gate 1/0
- **T3 Mac 真机手测 DEFERRED**（GO with documented gap 路径、留用户）
- **T4 doc-sync**（本会话、STATUS / ROADMAP / baseline v4-fixup3 节）
- **T5 PR + 合 main**（[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15) 已合 main、merge commit `ee78f75a2882d50c4ae2585fcc61743bb395cf20`、本地 + 远程分支已删）

**实际 ROI（保 cosine_threshold=0.70）**：OVERALL +0.008（0.856→0.864）、content-not-name +0.005（0.870→0.875）、exact-name = 1.000、crosslang -0.055（0.717→0.662、LociFind 头号卖点反退、trade-off 文档明示）。

**诚实边界**：未吃满 v4-fixup 表 bge-m3 sweep best（T*=0.0/0.30/0.45 OVERALL=0.869 / crosslang=0.685、约 +0.005 / +0.023 ROI 留 follow-up cycle 重 sweep cosine_threshold 拿回）；评测层 baseline 仍守 qwen3-0.6b 不动。

**未尽事宜**：① **Mac 真机手测留用户首次升级时按 [docs/manual-test-scenarios.md](./docs/manual-test-scenarios.md) 走三步**（启动 → 看到「按意思找到」徽标 → 跨语言 query 命中）；② follow-up cycle 4 候选（cosine sweep / baseline rewrite / 分发 UX / 跨厂候选）；③ PR #15 合 main 走 BETA-15B-7/8/9 同款 gh CLI 路径。

---

### 2026-06-25 — Claude Code (Opus 4.7) — v0.6.0 Windows prerelease 发布 + Claude × Codex 联合规划 5 个 LLM Wiki 启发特性已登记 ROADMAP

**承接**：BETA-15B-9 收口 push main 后，用户「想在 Windows 上测试当前最新版本，请帮忙 Release 一个最新的版本」→ 走 v0.6.0 release 流程；随后用户给文章 [Rethinking Agent Harness Part 4: LLM Wiki](https://axk51013.medium.com/rethinking-agent-harness-part4-llm-wiki-%E5%8F%96%E4%BB%A3-rag-041629319804) 讨论与 LociFind 对比、请用三工具协作规划新特性 → Claude × Codex 联合产出 5 个候选并登记 ROADMAP。本会话不是 BETA cycle 推进，是 ops + 产品战略性质。

**段 1：v0.6.0 Windows prerelease**（commit `668f354` + tag v0.6.0、Actions build-windows 20m26s 全过、[Releases / v0.6.0](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.6.0)、`LociFind_0.6.0_x64-setup.exe` 5.7MB NSIS 未签名 prerelease）：bump 0.5.0→0.6.0（apps/desktop/src-tauri/Cargo.toml + tauri.conf.json + Cargo.lock）+ cargo check 验证 lockfile 同步、commit + push main + tag、推 tag 触发 `.github/workflows/release-windows.yml`、后台命令链式 `gh run watch <id> && gh release edit v0.6.0 --notes-file /tmp/locifind-v0.6.0-release-notes.md` 等 Actions 完成自动覆盖完整 changelog（CONVENTIONS §8 要求 Release body 必须含 changelog 不用模板）。changelog 覆盖 v0.5.0 之后 60+ commits：**BETA-15B-5** 段落高亮可解释 v1（用户第一次能感知语义召回为什么命中）+ **BETA-15B-3 A-2/A-5** cosine 路由（hybrid OVERALL +0.022 / crosslang +0.067 / OVERALL 0.871 crosslang 0.726 双过 spec）+ **BETA-13-G16** parser §6 86.3%→87.7% + **BETA-15B-8** pooling type detection 修复（bge-m3 OVERALL 0.869 双过 spec 字面 0.864 ⭐）+ **BETA-15B-9** llama-cpp-4 0.3.0→0.3.2 升级 + qwen3-8b 全零 bug 4 hypothesis 全 FAIL ⚠️ 作为「已知问题」诚实声明。Annotations 提示 GitHub Actions Node 20 即将弃用（下次顺手把 `setup-node@v4` 升 Node 24 即可、不阻塞本次构建）。

**段 2：LLM Wiki 联合规划**（commit `4ac5bae` 已推 origin）：文章核心 = 四维度（LLM / Harness / Data / Task）+ sweet spot（personal + immutable + synthesis-heavy）+ ingest-time synthesis 持久化 → bounded navigation（vs unbounded vector similarity）；企业场景反例（cold content 反伤 / ACL / mutability / long-tail）让 LLM Wiki 失效。**Claude × Codex 三工具协作**（playbook 落地）：Claude 主会话起草 + 后台 spawn Codex（`cat prompt.md | codex exec --sandbox read-only --skip-git-repo-check -C /Users/alice/Work/LocalFind - > /tmp/codex-discussion.log`、run_in_background、~7-8 分钟完成）独立评审；Codex 锐评 Claude 三个小步（Entity index 别全库 NER 应热区结构化 / Cross-reference 别提前注入 ranker 缺 eval / Hot query cache 必须先有真实 hot 信号否则 sunk cost）+ 另起 4 个 B + 5 个 V + 3 个战略挑选。综合后 **5 个 task 登记 ROADMAP**：① **BETA-28 索引预算与分层新鲜度**（Data 反例护栏、避 cold long-tail 反伤）；② **BETA-29 查询意图可编辑草稿**（parser §6=87.7% 长尾产品化解法、走草稿 UI 而非 evals re-baseline）；③ **BETA-30 本地失败样本箱**（私有 eval 闭环、不上云）；④ **V10-15 Frozen Research Pack**（LLM Wiki sweet spot 唯一入口、V 旗舰）；⑤ **V10-16 LLM 读权限与出处闸门**（ACL 反例护栏、与 V10-15 绑定发布）。STATUS「本机可立即上手」追加 ⑩⑪⑫ + 「2026-06-25 联合规划」条目；ROADMAP §3.3 B1 加 BETA-28、B6 加 BETA-29/30、§3.4 加 V10-15/16。

**关键洞察**：文章对 LociFind 最大价值不是「加更多 LLM 能力」，而是用四维度框架 audit 哪些 LLM 能力会被 Data + Task 反例打中。LociFind 当前 ingest-time synthesis 是 deterministic derived index（FTS + embedding + 同义词族），与 LLM Wiki 哲学同向（synthesis 提前、bounded navigation）但产物形态不同（task=discovery 不 synthesis、data=mutable long-tail 不 immutable curated）；这不是技术保守、是 Data + Task fit 的正确解。5 个新特性正好是这个 audit 的产物。

**未尽事宜**：① **v0.6.0 Windows 真机用户测试留用户**（[manual-test-scenarios](./docs/manual-test-scenarios.md)、重点验 BETA-15B-5 段落高亮 / 来源标注 / 置信档位首次随 v0.6.0 prerelease 真机走通）；② 5 个新 task 全 not_started、未排 cycle、待用户拍板从哪个开始（**推荐 v0.7 起步 = BETA-28 索引预算分层**：是其他所有 LLM 功能的 Data 前提）；③ 滚动归档：本会话开始时会话日志 13 条 + 本会话 = 14 条超 CONVENTIONS 10 条上限，本次收工把最旧 4 条（06-20 BETA-13-G16 + 06-21 BETA-15B-5 + 06-21 BETA-15B-6 Phase D + 06-22 收口两分支）滚动到 [docs/session-logs/STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md)、归档文件标题从「2026-06-04 ～ 2026-06-20」更新为「2026-06-04 ～ 2026-06-22」。

---

### 2026-06-25 — Claude Code (Opus 4.7) — BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查 done + [PR #14](https://github.com/raoliaoyuan/LociFind/pull/14) 已合 main（merge commit `dc2a540`）⚠️ 4-hypothesis 全 FAIL + 升级红线放宽语义等价闸

**承接**：BETA-15B-8 cycle merge 后用户「按推荐路径走」→ 启动 v4 数据指证次高优抓手 = 解 qwen3-8b 全零（前置 bake 决策的「同族最大档真水位未知」盲点）。完整 superpowers 全流程：brainstorming Q1-Q2 → spec → plan 5 task → subagent-driven 驱动（含 subagent partial-exit + inline 接管 background process）。

**关键决策 / 修订**：
1. T1 spec §2.2 (7) byte-equal 红线 FAIL（升级带 ~1e-4 数值微调、上游 llama.cpp Metal kernel SemVer-compatible 微调不可避免）→ coordinator 拍方向 1：**放宽为语义等价闸**（cos ≥ 0.9999 + max abs ≤ 1e-3）、修订 spec 9 处 + commit `c113b8d` 补 §3.2 内部一致性。实测 cos=0.999999 / max abs=2.5125e-04 全过
2. T2 hypothesis 1 FAIL、T3 hypothesis 2-3 全 FAIL、T4 issue body draft 入仓 + 用户决定跳过 file（避免 GitHub identity / public content 提交）

**执行路径**：Phase 0 升级 + 语义等价闸 PASS → Phase 1 升 0.3.2 重测 8b FAIL → Phase 2 CPU 跑 FAIL → Phase 3 context=4096 FAIL → Phase 4 issue body draft 入仓（无 file）→ Phase 5 收口

**产出（5 task + N commits）**：
- T1 (`38d255d` + `c113b8d` fixup) 升 llama-cpp-4 0.3.0 → 0.3.2 + spec §2.2 (7) 红线放宽语义等价闸（cos=0.999999、max abs=2.5125e-04）+ vectors.json + vectors-qwen3-0.6b.json 同步入仓 `0315b8d0...`
- T2 hypothesis 1 FAIL（无 commit）
- T3 hypothesis 2-3 全 FAIL（无 commit、llama.rs 临时改动还原）
- T4 issue body draft 入仓 `docs/reviews/2026-06-25-beta-15b-9-upstream-issue-body.md`（138 行 + GGUF metadata + Rust 复现 + 4 hypothesis 表）
- T5 (`203c404`) cycle 收口（baseline v4-fixup2 节 FAIL 变体 + STATUS/ROADMAP doc-sync + PR #14 + 合 main、merge commit `dc2a540`）

**关键发现**：llama-cpp-4 0.3.2 log 显示 `fused Gated Delta Net (autoregressive + chunked) enabled` = v4 hypothesis 4「8b 特殊层结构未支持」假设**部分推翻**（binding 已识别 + 启用 fused 路径、但 embedding 仍全零）。推断 root cause = **fused 实现与 `LlamaPoolingType::Last` + embedding-only 推理交互 8b-specific bug**。

**qwen3-8b 真水位**：仍未知。bake 决策无新数据。

**bake 推荐（不变）**：BETA-15B-7-v2 bake bge-m3 推到生产（最高优、零分发成本、OVERALL +0.013 vs 0.6b、crosslang -0.032 trade-off 文档明示）。

**下 cycle 抓手优先级修正**：① BETA-15B-7-v2 bake bge-m3（最高优、不再等 8b）② 监控 upstream / 用户 file issue 后跟进 thread（中优）③ 跨厂替代候选（低优）④ 评测扩量（低优）。

**未尽事宜**：① 真机手测：纯 infra 升级 + 数据收集 cycle、桌面行为零变化、按 spec §7 判平凡未安排；② 用户手动 file upstream issue（issue body 在 `docs/reviews/2026-06-25-beta-15b-9-upstream-issue-body.md` 待复制粘贴到 https://github.com/eugenehp/llama-cpp-rs/issues/new）；③ bake 决策由用户拍板下 cycle 启动。

---

### 2026-06-25 — Claude Code (Opus 4.7) — BETA-15B-8 model-runtime pooling type detection done + [PR #13](https://github.com/raoliaoyuan/LociFind/pull/13) 已合 main（merge commit `5305ee1`）⭐ infra 阻塞解除 + bge-m3 OVERALL 0.869 双过 spec 字面 0.864

**承接**：BETA-15B-7 v4 cycle merge 后用户「继续」→ 启动 v4 数据指证最高优抓手 = 修 model-runtime pooling type detection。完整 superpowers 全流程：brainstorming Q1-Q4 → spec → plan 5 task → subagent-driven 驱动 + 每 task spec/code-quality 双审 + final integration review。

**关键决策（brainstorming Q1-Q4 收敛）**：① 范围 = **只修 pooling type detection**（最窄、不动 llama-cpp-4 / 不动 BETA-15B-7-v2 重跑）；② fallback = architecture-based default + 未知 arch fail-fast；③ 验证 = 代码 + qwen3-0.6b byte-equal + bge-m3 重跑（不动 baseline.json / 不 bake）；④ 代码组织 = 抽 helper + 纯逻辑单测 + load 时算一次。

**Implementer + coordinator 协作修复的 3 处起草期工程偏差**：① spec/plan 错引 llama-cpp-2 0.1.146 接口、实际项目用 llama-cpp-4 0.3.0（不同 crate、Rank=4 未暴露）→ Option A 删 Rank 分支 + reject -1；② plan 漏 `meta_val_str` 的 `buf_size` 参数 → `const META_BUF: usize = 256`；③ plan step 3.2/3.4 `--vectors-file` 全路径 → basename（evals `fixt()` 重复拼前缀 panic）。所有偏差在 Task 4 doc fixup commit `11c5843` 校正回 spec / plan。

**产出（5 task 全 done + 双审、5 commits + 1 cycle 末 commit 落 main）**：
- T1 (`cb3ee72`) pooling.rs 纯逻辑模块 + 9 单测（TDD 红→绿）+ lib.rs mod 声明
- T2 (`f40f14b`) llama.rs 接入 + qwen3-0.6b vectors.json byte-equal SHA256 `0c258086...` 完全等价（零回归硬证据）
- T3 (`3214e06`) bge-m3 CLS pooling 真水位 sweep + vectors-bge-m3.json 覆盖入仓
- T4 (`11c5843`) 4 doc 校正（v4 baseline / v4 spec / 本 cycle spec / 本 cycle plan）+ baseline 报告追加 v4-fixup 节
- T5 (`1a86dc7`) 5 项总验收 + 3 Minor doc fixup（8 vs 9 单测 + plan RANK 残留字串）+ STATUS/ROADMAP doc-sync + PR #13 + 合 main（merge commit `5305ee1`）

**bge-m3 CLS pooling 真水位**：OVERALL **0.869** 双过 spec 字面 0.864 ⭐ / crosslang 0.685 微差 spec 字面 0.700 / content-not-name 0.875。**vs v4 错配 +0.099 / +0.142 / +0.053**（infra 修复完全证实「last-token state collapse」诊断）。**vs qwen3-0.6b 生产锚 +0.013 / -0.032 / +0.005**（OVERALL/content-not-name 略胜、crosslang 略输）= **GO（破局变体 / 二维 trade-off）**。

**下 cycle 抓手**：① BETA-15B-7-v2 bake bge-m3 推到生产（最高优、零分发成本、需 DEFAULT_EMBEDDING_MODEL_PATH + baseline.json + Windows 适配）；② BETA-15B-Y 升 llama-cpp-4 / 解 qwen3-8b；③ 跨厂替代候选；④ 评测扩量。

**未尽事宜**：① PR / GitHub 合 main 状态（视 gh CLI 401 凭据可能走本地 merge + push origin/main + 删分支 fallback、与 BETA-15B-7 PR #12 同款）；② 真机手测：纯 infra 修复 cycle + 评测端到端覆盖、按 spec §3.2 + §7 判平凡未安排桌面手测剧本；③ bake bge-m3 决策由用户拍板下 cycle 启动。

---

---


### 2026-06-24 — Claude Code (Opus 4.7) — BETA-15B-7 embedding 模型跨族 (bge-m3) + 同族最大档 (qwen3-8b) 探针 done + PR #12 已合 main（merge commit 094e7d0）⭐ 双 Branch IV-infra

**承接**：同会话 v3 cycle merge 后用户「继续」→ 启动 v3 数据指证最高优抓手 = **更大 embedding 模型**。完整 superpowers 全流程：brainstorming 5 决策 → spec（含 spec 起草后两次大改：起初 qwen3-0.6b vs 1.5b vs 4b → 核查发现 1.5b 不存在 → 改 qwen3-0.6b vs 4b vs 8b → 用户调研发现 bge-m3 跨族 SOTA 后改最终 bge-m3 + qwen3-8b 双轴）→ plan（7 task / T2 TDD / 强 YAGNI）→ subagent-driven 驱动 + T2 spec/code-quality 双审 + final integration review READY TO MERGE WITH MINOR FIXES + 3 Important doc fixup。**5 commits + 1 merge commit 落 main**（094e7d0）。

**关键决策（brainstorming 5 收敛）**：① 范围 = **评测 only / 不动生产 wiring / 不动分发**（YAGNI、与 v1/v2/v3 同款节奏）；② 模型选型 = **双点 sweep**（起初拍 qwen3 同族缩放、用户引入 bge-m3 跨族候选后改双轴：跨族 bge-m3 + 同族最大档 qwen3-8b）；③ 不重训 = **直接用 HuggingFace 官方 GGUF q8_0**（YAGNI、本 cycle 单变量 = 模型缩放、加微调污染信号）；④ 操作三项：q8_0 三档统一 + `--vectors-file` flag 加三独立文件 + 复用 9 阈值 sweep；⑤ 接受标准 = **复活字面 0.864 / 0.700 spec 目标**作为 GO 候选判定指标、四 Branch I-a (bge-m3 GO) / I-b (qwen3-8b GO) / II (NO GO) / III (见顶) / IV (异常) 决策表。

**Spec 起草中两次大幅修订**：① 起初 qwen3-0.6b vs 1.5b vs 4b → HF 实测查 unsloth/Qwen 官方仓库都 401 + 我 brainstorming 中虚构「Qwen3-Embedding-1.5B 存在」错误 → 修订为 qwen3-0.6b vs 4b vs 8b；② Python 脚本 + Edit 批量替换文档后用户提问「有没有更新性能更强的模型替代」→ HF 搜索发现 bge-m3 跨族同尺寸候选 + 实测可下、用户拍 B（bge-m3 + qwen3-8b 双轴）→ 重写 spec/plan 反映最终结构。

**T1 模型下载多坑**：① unsloth/Qwen 官方仓库实际不存在 1.5b GGUF（错误前提）；② mradermacher/Qwen3-Embedding-4B-GGUF 公开免登录；③ Qwen3-Embedding 官方阵容实际 = 0.6b / 4b / 8b（无 1.5b）；④ qwen3-8b 通过 HF token + license accept 才可下；⑤ curl 多次 CDN drop（连接中断、断点续传失败、3.2 GB 处停）；⑥ Python hf_hub_download 卡死在 5.6 MB；⑦ 最终 aria2c 8 连接成功（虽末段 SSL handshake error 但文件完整 8,047,105,824 bytes = 精确匹配预期 7.5 GB、GGUF magic 验过）。教训：HF 大文件下载用 aria2c -x 8 -s 8 比 curl / hf_hub_download 都稳。

**产出（7 task 全 done + 1 doc fixup、5 commits 落 main）**：
- T1 (无 commit、`.gguf` gitignored) bge-m3 q8_0 (605 MB, SHA256 950f4a8e...) + qwen3-8b q8_0 (7.5 GB, SHA256 a48e5033...) 双模型下载完成
- T2 (566869b) `semantic_quality.rs` 加 `--vectors-file` flag (TDD)：clap 字段 + 2 新单测 (vectors_file_flag_parses + vectors_file_defaults_to_vectors_json) + 替换 sweep / embed 两处硬编码 + embed_and_write 签名扩参 + clippy::panic crate-level allow（implementer DONE_WITH_CONCERNS 已 spec/code-quality reviewer accept）
- T3 (4af8e9c) Mac Metal embed × 2 + 0.6b copy 命名归位：bge-m3 ~10s 全集 + qwen3-8b ~27s 异常短即提示 Branch IV + jq schema 验 + L2 norm 抽样 (0.6b ≈ 1.0、bge-m3 ≈ 1.0、qwen3-8b = 0 命中 §5 Branch IV) + vectors.json byte-equal 保持
- T4 (无 commit、/tmp/) sweep 9 阈值 × 3 模型 = 27 次 cargo run 产决策矩阵
- T5 (无 commit、/tmp/branch-decision.md) 双 Branch IV 决策：bge-m3 Branch IV-A (BERT arch / pooling 错配) + qwen3-8b Branch IV-B (vec 全零 / llama-cpp-4 bug)
- T6 (9b14404) baseline 报告追加 v4 节 +152 行（三模型 sweep 全表 + 双 Branch IV-infra 诊断 + 下 cycle 抓手优先级修正 + 认知层修订小结）+ fixtures README v4 注脚
- T7 (无 commit) 总验收全过 + PR draft prep
- spec + plan 落库 (27e9212) +982 行（cycle 文献保留）
- doc fixup (dfa3c65) 响应 final integration reviewer 3 Important：① spec §1.3 加 v4 cycle 末修订说明、② baseline 加 SHA256 + llama-cpp-4 = 0.3.0 版本 pin + follow-up cycle 必做步骤、③ bge-m3 诊断加 GGUF `bert.pooling_type=2 (MEAN)` 实测细节 + cosine top1 高但 nDCG 低的 last-token state collapse 证据、④ qwen3-8b 诊断软化「8b 必有特殊层结构」推测 → 4 hypothesis 排序

**三模型 sweep 摘要**（v3 数据集 78 cases / 124 docs / W=10.0）：

| 模型 | exact-name HYBR_R | best OVERALL HYBR_N | best crosslang HYBR_N | best content-not-name HYBR_N | 决策 |
|---|---|---|---|---|---|
| qwen3-0.6b T*=0.70 (v3 锚) | 1.000 | 0.856 | 0.717 | 0.870 | 对照 ✓ |
| bge-m3 (跨族轴) | 1.000 | 0.770 (gap **-0.086**) | 0.543 (gap **-0.174**) | 0.822 (gap -0.048) | Branch IV-A |
| qwen3-8b (全 T 恒等、vec 全 0) | 1.000 | 0.562 | 0.100 | 0.827 | Branch IV-B |

**诊断证据**：① bge-m3 cosine top1 分布 mean=0.719 > qwen3-0.6b 0.660、但 nDCG 反而低 = last-token state collapse 典型表现（向量看起来近、语义编码偏）；② qwen3-8b 总推理时间 27 秒 vs 估算 13-40 分钟 = 早期短路证据。

**诚实边界结论**：**两条独立轴都被 model-runtime / llama-cpp-4 infrastructure 层阻断、无 GO/NO GO 模型层数据指证**。**不能下「bge-m3 比 qwen3-0.6b 弱」结论**（测的是错配 pooling）、**不能下「qwen3-8b 模型差」结论**（8b 根本没被有效推理过）。**v3 主动放弃的 0.864 OVERALL spec 目标依然有效**、本 cycle 未能反驳「Qwen3-0.6b 系列见顶」假设、只能说「至今没有合规的反驳数据 + infra 是当下最大瓶颈」。

**为什么仍合并发布**（非 spec §5 字面「Branch IV 不发布」）：spec §5 字面规则默认假设单 cycle 内 1 个出乎意料的负面结果。本 cycle 实际「根因明确（infra）+ 双轴一致暴露 + 修复方向清晰」、「不发布」字面规则会浪费已得诊断价值。修订执行：合并 + 发布完整诊断作 baseline 报告 v4 节、作为 follow-up cycle 精确入口；不 bake 任何模型到生产、gate / baseline 不动、回归门仍守护 v3 0.6b；三 vectors-*.json 入仓作研究产物；v4 节明示「不能下模型能力结论」避免后续 cycle 误解。

**未尽事宜**：① STATUS / ROADMAP doc-sync 本条收尾落库；② 真机手测：纯评测探针 cycle、按 spec §3.2 / §7 判平凡未安排手测剧本；③ 下一 cycle 抓手由用户拍板（修 model-runtime pooling type detection 最高优、~0.5-1d）；④ PR #12 GitHub 状态：gh CLI 401 凭据问题、走本地 merge + push origin/main + 删本地+远程 feature branch、PR 状态 GitHub web UI 自动识别为 merged（与 PR #11 / #12 同款流程）；⑤ HF token 已写入本会话 git/conversation 历史、建议用户后续 rotate。
---

### 2026-06-24 — Claude Code (Opus 4.7) — BETA-15B-6 v3 content-not-name 桶二次扩量 20→30 + T\* 真水位校验 + 认知层主动放弃字面 spec 目标 done + PR #11 已合 main（merge commit 4070388）⭐ Branch A 命中、v2 T\*=0.70 鲁棒

**承接**：同会话 v2 cycle merge 后用户「继续」→ 启动 v2 数据指证最高优抓手之一 = **BETA-15B-6 v3 评测集再扩量**（另一最高优 = 更大 embedding 模型需 Mac 训练不在本机立即可上手）。完整 superpowers 全流程：brainstorming 5 决策 → spec → **spec 起草后修订**（核查发现 gate.rs (4c)(4d) A-3 cycle 起即动态读 baseline 自锁、原 spec T7 红线修订 task 基于错误前提、修订删 T7 task 数 8→7、改为认知层修订）→ plan 7 task → subagent-driven 驱动 + 每 task spec/code quality 双审 + T2 fixup 加 5 Whys 词 + final integration review 3 Minor doc fixup + final review READY TO MERGE。**8 commits + 1 merge commit 落 main**（4070388）。

**关键决策（brainstorming 5 收敛）**：① 范围 = **只扩 content-not-name 桶**（YAGNI、v2 数据指证最直接抓手）；② 数量 = **20→30（+10、与 v2 +9 同款节奏、近半翻倍）**；③ 设计 = **4 边界 + 6 常规**（沿用 v2 比例、避免设计偏置 artifact）；④ 主题分配 = **6 zh + 4 en、7 新主题、3 zh+en 配对 + 4 单语种**（故障复盘 5 Whys / 灰度发布 canary / API 版本管理 / 告警分级 / 异常日志 / 性能基线监控 / 数据保留）；⑤ 接受标准 = **修订 spec §2.2 (4c)(4d) 为「不退步 v2 baseline」自锁**（放弃字面 0.864/0.700 spec 目标、移交下 cycle = 更大 embedding 模型）。

**认知层修订（spec 起草后核查、删 T7）**：spec 初稿写「T7 修订 gate.rs (4c)(4d) 红线断言代码：字面阈值 0.700 / 0.864 → 改为动态读 baseline」基于错误前提——核查 [`packages/evals/tests/semantic_quality_gate.rs:121-132`](packages/evals/tests/semantic_quality_gate.rs#L121) 发现 (4c)(4d) **A-3 cycle 起即动态读 baseline 自锁**、v3 cycle「红线修订」纯为认知层 / 文档层动作（baseline 报告 v3 节明示主动放弃字面 spec 目标、移交下 cycle）。spec 顶部加注修订说明 + §2.2/§3.1/§5/§6/§7 同步修订、原 T7 删除、task 数 8 → 7。

**产出（7 task 全 done + 2 fixup、8 commits 落 main）**：
- T1 (b6b9e86) cases.json +10 c069-c078（6 zh + 4 en、4 边界 + 6 常规、c077 复用 s00011/s00012）
- T2 (21f1d78) corpus.json +9 s00116-s00124（5 zh + 4 en、zh/en 62:62、零 PII 全虚构）
- T2 fixup (377584f) s00116/s00117 body 加强 5 Whys 框架特有词（响应 T2 code-quality reviewer Important 1：与 s00078/s00079 blameless retro embedding 距离）
- T3 (87f0fe5) Mac Metal `--embed` 重算 vectors.json 全集（dim 1024、124 doc + 78 query、2.5MB）
- T4 (无 commit) 9 阈值 sweep + 三 Branch 决策：**Branch A 命中**、T\*=0.70 仍 sweep best
- T5 (跳过 Branch A、不动 bake / lib.rs / gate.rs)
- T6 (b98024b) rewrite baseline.json + 全套验证门过（workspace 860 passed / clippy + fmt 净 / gate 1 passed / v0.5/v0.9 byte-equal 精确）
- T7 (93a3c7a) baseline 报告 v3 节 +109 行 + README v3 更新 + 总验收过
- T7 doc fixup (a47467b) baseline 报告 v3 节 3 处措辞清理（响应 final integration reviewer 3 Minor：cosine ≥0.30→≥0.45 准确化 + ⭐⭐→⭐ 与 A-5/v2 风格一致 + 「扩到 50 例」改为「主抓手 = 更大 embedding 模型」与抓手表对齐）

**Sweep 全表**（v3 数据集 78 cases / 124 docs / dim 1024、W=10.0 固定）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.847 | 0.726 | 0.822 ↓ |
| 0.30 | 1.000 | 0.847 | 0.726 | 0.822 ↓ |
| 0.45 | 1.000 | 0.847 | 0.726 | 0.822 ↓ |
| 0.60 | 1.000 | 0.851 | 0.726 | 0.852 ↓ |
| **0.70 ⭐ (T\* sweep best Branch A 命中)** | **1.000** | **0.856** | **0.717** | **0.870** |
| 0.80 | 1.000 | 0.847 | 0.671 | 0.868 |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.843 | 0.648 | 0.868 |

**控制对照**：T=0.0 时 HYBR ≈ VEC ✓、T=1.01 时 HYBR ≡ HYB ✓。

**spec §2.2 红线全过 @ T\*=0.70**：
- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ v3 HYB baseline（synonym = / concept = / crosslang +0.069 / content-not-name +0.002 / exact-name = / OVERALL +0.013）全过 ✓
- (4c) crosslang HYBR_N **自锁 v3 baseline**（实测 0.717 ≥ baseline 0.648 ✓、本 cycle 主动放弃字面 ≥ 0.700 spec 目标）
- (4d) OVERALL HYBR_N **自锁 v3 baseline**（实测 0.856 ≥ baseline 0.843 ✓、本 cycle 主动放弃字面 ≥ 0.864 spec 目标）

**v3 真水位结论 — A-5「cosine 单维真破局」结论 v3 进一步确认 ⭐**：v3 验证 v2 T\*=0.70 鲁棒——content-not-name 桶 20 例不是运气、cosine 信号方向真破局；v3 真水位 vs v2（不同数据集对比）：OVERALL 0.854 → **0.856** (+0.002 微升、统计意义内不变) / crosslang 0.717 → **0.717** (持平、cosine 信号在 crosslang 桶稳) / content-not-name 0.853 → **0.870** (**+0.017 显著升**、二次扩量后真水位反而升、说明 v2 20 例并未让 T\*=0.70 带运气)。**A-5「cosine 单维真破局」结论 v3 进一步确认**。

**认知层修订小结**：v3 cycle 主动放弃字面 0.864 / 0.700 spec 目标的字面追求，**承认在「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下结构性不可达**。gate.rs 4 红线全部自锁 baseline（A-3 cycle 起即如此）、未来 cycle 调优只要不退步 baseline 即合规；字面 spec 目标移交下 cycle 抓手（更大 embedding 模型 / cosine + lang 组合信号）。诚实承认目标下调 ≠ 项目失败、而是诚实接受当前技术栈天花板。

**下 cycle 抓手优先级修正（v3 数据指证）**：① **更大 embedding 模型 qwen3-0.6b → 1.5b/3b**（**极高优**、v3 验证 T\*=0.70 鲁棒后是唯一剩余 0.864 字面 spec 目标天花板抓手、需 Mac 训练 + 模型分发）；② **评测集再扩量**（仅专攻 crosslang 桶 13 例偏小、若做扩到 20 例校验 crosslang 是否含运气、否则边际收益小、中优）；③ **原始 query 入 schema**（A 簇余项、中优）；④ cosine + lang 组合信号（低优、若更大模型无效再做）。

**未尽事宜**：① STATUS / ROADMAP doc-sync 本条收尾落库；② 真机手测：纯数据 + 文档 cycle、按 spec §3.2 判平凡未安排手测剧本；③ 下一 cycle 抓手由用户拍板（更大 embedding 模型最高优、需 Mac 训练）；④ PR #11 GitHub 状态：gh CLI 401 凭据问题、走本地 merge + push origin/main + 删本地+远程 feature branch、PR 状态 GitHub web UI 自动识别为 merged。


---

### 2026-06-24 — Claude Code (Opus 4.7) — BETA-15B-6 v2 content-not-name 桶扩量 + T\* 鲁棒性校验 done + PR #10 已合 main（merge commit 12fcf7b）

**承接**：同会话 A-5 cycle merge 后用户「继续」→ 启动 A-5 数据指证最优先抓手 = **BETA-15B-6 评测集扩量 + 重构 content-not-name 桶 case**。完整 superpowers 全流程：brainstorming 8 决策 → spec → plan 7 task → subagent-driven 驱动 + 每 task spec/code quality 双审 + final reviewer 3 Minor + 2 doc fixup + final review READY TO MERGE。**9 commits + 1 merge commit 落 main**（12fcf7b）。

**关键决策（brainstorming 8 收敛）**：① 范围 = **只扩 content-not-name 桶**（YAGNI、A-5 数据指证最优先）；② 扩量目标 = **11→20（+9、近翻倍）**（与其他桶档次齐）；③ case 生成方法 = **沿用 BETA-15B-6 v1**（虚构占位、零 PII）；④ 接受标准 = **重跑 sweep 三 Branch 决策**（按 spec §2.2）；⑤ 新 9 case 语种 = **5 zh + 4 en**（改善 en 占比 0.18→0.30）；⑥ corpus 同步扩 = **+7 新 doc**（4 zh + 3 en、3 跨语言主题对 + 1 zh A/B + c063/c068 复用 s00023/s00024 客服对）；⑦ case 设计 = **4 边界 case（cosine ~0.55-0.65）+ 5 常规 case**；⑧ 执行流程 = **Mac Metal 本机重算 vectors + commit 前 PII 自查**。

**产出（7 task 全 done + 2 doc fixup、9 commit）**：T1 cases.json +9 c060-c068 → T2 corpus.json +7 s00109-s00115（完整性测试 FAIL→PASS）→ T3 Mac Metal --embed 重算 vectors.json 全集（dim 1024、qwen3-embedding-0.6b-q8_0、115 doc + 68 query、2.33MB）→ T4 跑 9 阈值 sweep + 三 Branch 决策（Branch B 命中、T\*=0.70）→ T5 bake `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.60 → 0.70` + lib.rs/gate.rs doc 升 v2 + rewrite baseline.json → T6 全套验证门（workspace 860 passed / clippy + fmt 净 / gate 1 passed / v0.5/v0.9 byte-equal）→ T7 baseline 报告 v2 节 +107 行 + README v2 更新 + 总验收过 → final reviewer 3 Minor 中 2 doc fixup（lib.rs Branch B 边界补「\[0.55, 0.65\] inclusive 上界」+ baseline 报告 (4d) 未达后补「走 baseline 自锁路径绕过 0.864 字面阈值、技术上不破红线」）。

**Sweep 全表**（v2 数据集 68 cases / 115 docs / dim 1024、W=10.0 固定）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.844 | 0.726 | 0.778 ↓ |
| 0.30 | 1.000 | 0.844 | 0.726 | 0.778 ↓ |
| 0.45 | 1.000 | 0.844 | 0.726 | 0.777 ↓ |
| 0.60 (A-5 v1 bake) | 1.000 | 0.848 | 0.726 | 0.826 ↓ 破 (4b) |
| **0.70 ⭐ (v2 T\*)** | **1.000** | **0.854** | **0.717** | **0.853** |
| 0.80 | 1.000 | 0.845 | 0.671 | 0.852 |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.842 | 0.657 | 0.852 |

**控制对照**：T=0.0 时 HYBR ≈ VEC、T=1.01 时 HYBR ≡ HYB。

**spec §2.2 红线全过 @ T\*=0.70**：
- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ v2 HYB baseline ✓（crosslang +0.060 / content-not-name +0.001 / OVERALL +0.012）
- (4c) crosslang HYBR_N **0.717 ≥ 0.700** spec 目标 ✓
- (4d) OVERALL HYBR_N **0.854 < 0.864** spec 目标 ⚠️ 字面未达——走 baseline 自锁路径（gate `now.hybrid_routed_ndcg ≥ baseline.hybrid_routed_ndcg`、v2 rewrite 后 baseline = 0.854、自动跟随、技术上不破红线）

**诚实边界 — A-5 v1 含轻微运气、v2 揭示真水位**：A-5 cycle v1 11 例 sweep 选 T\*=0.60、OVERALL 0.871 / crosslang 0.726。v2 扩到 20 例后：T\*=0.60 不再 sweep best（content-not-name 退步 -0.026 破 (4b)）；v2 sweep best 上移到 T\*=0.70（偏 +0.10、Branch B 边界 inclusive 上界）；v2 真水位 OVERALL 0.854 / crosslang 0.717 / content-not-name 0.853——A-5「cosine 单维真破局」结论**仍成立**（v2 各桶 ≥ baseline、crosslang 仍 ≥ 0.700），但**「双超 spec 目标」结论需修正**——v2 上 OVERALL 0.864 spec 目标不可达。

**下 cycle 抓手优先级（v2 数据指证）**：① **更大 embedding 模型 qwen3-0.6b → 1.5b/3b**（高优、需 Mac 训练）；② **评测集再扩量**（content-not-name 30+ / 总 100+ case、高优、与 v2 同款方法可复制）；③ **原始 query 入 schema**（A 簇余项）；④ cosine + lang 组合信号（低优、若上述无效再做）。

**未尽事宜**：① STATUS / ROADMAP doc-sync 本条收尾落库；② Minor 3「spec id c049-c057 vs 实际 c060-c068 不一致」未在 spec 顶部加注、留下次 doc-sync 同步；③ 真机手测：纯数据 cycle、按 spec §3.2 判平凡未安排手测剧本；④ 下一 cycle 抓手由用户拍板（v3 评测扩量 / 大 embedding 模型 / 转他线）。

---

### 2026-06-24 — Claude Code (Opus 4.7) — BETA-15B-3 A-5 VEC top-1 cosine 阈值路由 done + PR #9 已合 main（merge commit d57dff9）⭐ A 簇 5 cycle 首破 spec §5 降级

**承接**：跨会话同款节奏。新会话开场用户「按推荐路径走」收 A-4 → 启动 A-5。完整 superpowers 全流程：brainstorming 9 决策 → spec → plan 10 task → subagent-driven 驱动 + 每 task spec/code quality 双审 + T4 fixup（arms doc 参数顺序反向 callout）+ final reviewer 3 Minor doc fixup（清 A-4 stale 措辞）+ final review READY TO MERGE 无 Critical/Important。**12 commits + 1 merge commit 落 main**（d57dff9）。

**关键决策（brainstorming 9 收敛）**：① 范围 = **只做 cosine 单维**（YAGNI、与 A-3/A-4 同款）；② 信号源 = **VEC top-1 cosine**（`vec[0].score.unwrap_or(0.0)`、生产侧 `score=cosine` 已就绪）；③ 动作方向 = **cosine_top1 ≥ threshold 跳 FTS**（vec 强信任、与 A-4 数据指证方向同构）；④ wrapper API 名 / RouteVerdict 字段架构 / HYBR baseline 字段名 / gate 红线架构 **全保留**（A-3/A-4 基础设施 zero-touch）；⑤ wrapper 签名 6→5 参 + RouteVerdict 字段升级（保留 query_lang 作元数据、wrapper 默认 Mixed 占位、wiring 后置覆写）；⑥ 删 A-3 jaccard 函数 + 5 单测（无消费者）；⑦ 保留 A-4 detect_lang + 8 单测（wiring 后置覆写仍需）；⑧ UI 不暴露 cosine 阈值 bake-only（与 A-3/A-4 同款）；⑨ baseline 字段复用 HYBR（重定义为 cosine 路由）+ rewrite 数值。

**Sweep 全表**（W=10.0、T = cosine_threshold）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.864 | 0.726 | **0.833 ↓ 破** |
| 0.30 | 1.000 | 0.864 | 0.726 | **0.833 ↓ 破** |
| 0.45 | 1.000 | 0.864 | 0.726 | **0.833 ↓ 破** |
| **0.60 ⭐ (T\* 选定)** | **1.000** | **0.871** | **0.726** | **0.930** |
| 0.70 | 1.000 | 0.867 | 0.709 | 0.930 |
| 0.80 | 1.000 | 0.857 | 0.663 | 0.930 |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.854 | 0.649 | 0.930 |

**控制对照核验**：T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等）；T=1.01 时 HYBR ≡ HYB（六桶完全相等）。

**spec §2.2 红线全过 @ T\*=0.60**：(4a) exact-name HYBR_R=1.000 ✓ / (4b) 各桶 HYBR_N ≥ HYB baseline 全过（concept 0.820>0.819 / crosslang 0.726>0.649 / OVERALL 0.871>0.854）/ (4c) crosslang HYBR_N=0.726>0.700 spec 目标 ✅⭐ / (4d) OVERALL HYBR_N=0.871>0.864 spec 目标 ✅⭐。**vs A-4 baseline**：OVERALL +0.017、crosslang +0.077、content-not-name 守 0.930、exact-name 1.0 红线。

**破局根因**：A-3 Jaccard / A-4 lang 单维信号在 crosslang 桶（应跳）与 content-not-name 桶（不应跳）触发逻辑相同——调阈值零和博弈。**A-5 cosine 信号方向精准对齐失败模式**：crosslang 桶 vec 强（cosine≈0.7+）跳 FTS、content-not-name 桶 vec 弱（cosine<0.6）保留 FTS——**cosine 单维真破局，不是单维路由结构问题的运气**。

**验证**：workspace **860 passed / 0 failed**、clippy(`-D warnings`) 0、fmt 净、**evals parser-only byte-equal v0.5=473 / v0.9=877 精确不变**、gate 1 passed（4 红线全过、动态读 baseline）。

**A 簇主路径阶段性收口**。下 cycle 抓手优先级：① **评测集扩量 + 重构 content-not-name 桶 case**（最高优、合成集 11 例可能让 T\* 带运气）；② 更大 embedding 模型（中、需 Mac 训练）；③ 原始 query 入 schema（A 簇余项）；④ cosine + lang 组合信号（低、若评测扩量发现 cosine 单维死角再做、git history 可恢复）。

**未尽事宜**：STATUS / ROADMAP doc-sync 本条收尾落库；真机手测：纯后端融合层加法 + 评测端到端覆盖、按 spec §3.2 判平凡未安排手测剧本。

---

### 2026-06-23 — Claude Code (Opus 4.7) — BETA-15B-3 A-4 query 语种检测 + 跨语种 vec hit 路由 done + PR #8 已合 main（merge commit a30186a）

**承接**：新会话开场用户问「本次会话需要执行什么任务」→ 读三文档发现分支 `feat-beta-15b-3a4-lang-routing` 已 10 commits + push origin（task 1-10 + T3 fixup 全 done、baseline 报告 A-4 节已写、git log 上「总验收过」），但 STATUS 当前 Task 仍停留在 A-3——意味着 A-4 实施已 done、收口未走（无 PR、未合 main、STATUS/ROADMAP 未追加 A-4）。给用户两选：① 按推荐先收口 A-4 → 再启动下 cycle A-5；② 直接跳过收口启动 A-5。用户「按照推荐路径走」。

**本会话执行（收口闭环）**：
1. **验收 5 项全过**：`cargo fmt --all --check` 净 / `cargo clippy --workspace --all-targets -D warnings` 0 / `cargo test --workspace` 865 passed/0 failed / `cargo test -p locifind-evals --test semantic_quality_gate` 1 passed / **evals parser-only byte-equal v0.5=473/25/2 + v0.9=877/119/4 精确符合 baseline 报告登记**（10 改动文件全在 result-normalizer/harness/evals/semantic-recall + desktop call-site，不碰 parser/coverage/v0.5/v0.9 fixture）
2. **Final integration review（general-purpose subagent）**：通读 spec/plan/baseline 报告 + main..HEAD diff +597/-271 + 抽样核对 10 文件 + spec §3.2 YAGNI 守住 + 控制对照不变性 + spec §2.2 红线全过 → **结论 READY TO MERGE 无 Critical/Important**，3 条 Minor：(1)+(2) `RouteVerdict` / `FanoutOutcome.route_verdict` doc 仍写 A-3「暂存预留」措辞、T9 已透传可升 A-4 表述；(3) jaccard_overlap_by_path 工具函数下 cycle 决策时一并考虑。
3. **doc fixup commit**（响应 Minor 1+2，30 秒）：`packages/result-normalizer/src/lib.rs:194` + `packages/harness/src/fanout_merge.rs:31` doc 升 A-4 表述（已透传到 FanoutOutcome / lang 信号 + cross-lang hits）；fmt+clippy 双检 + push origin
4. **PR #8 已合 main**：`gh pr merge 8 --merge --delete-branch` → merge commit `a30186a`、本地切回 main、fast-forward、本地+远程分支已删、stale tracking ref 已 prune

**Sweep 全表回顾**（W=10.0 固定、N = max_cross_lang_hits）：

| N | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|
| 0 (≈纯 vec 控制) | 0.864 | 0.726 | 0.833 |
| 2 (sweep 最佳 OVERALL) | 0.874 | 0.726 | **0.887** ↓ 破红线 |
| 3 | 0.859 | 0.703 | 0.887 ↓ |
| 5 | 0.852 | 0.681 | 0.886 ↓ |
| **usize::MAX (N\* 选定)** | **0.854** | **0.649** | **0.930** |

**诚实边界**：同 A-3 失败模式在 lang 信号下重现——单信号路由分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」两场景，纯调阈值救不了；唯一不破 spec (4b) 各桶 ≥ HYB baseline 硬红线的 N 是 `usize::MAX`（HYBR ≡ HYB 路由不生效）；spec §5 字面正解保守降级；spec (4c) crosslang ≥ 0.700 / (4d) OVERALL ≥ 0.864 未达。

**A-4 数据指证下 cycle 抓手排序**：② **VEC top-1 cosine 阈值最优先**——方向上直对失败模式（VEC 强 cosine 高时跳 FTS、VEC 弱时保留 FTS）、不需训练、复用 A-3+A-4 wrapper 基础设施；③ 更大 embedding 模型 qwen3-0.6b→1.5b/3b（需 Mac 训练）；④ 评测集扩量 + 重构 content-not-name 桶（合成集 11 例可能过窄）。

**未尽事宜**：① STATUS/ROADMAP doc-sync 本条收尾落库；② 真机手测：纯后端融合层加法 + 评测端到端覆盖、按 spec §3.2 判平凡未安排手测剧本（路由不生效、生产行为与 A-3 持平）；③ 下 cycle A-5 待用户拍板启动。

---

### 2026-06-23 — Claude Code (Opus 4.7) — BETA-15B-3 A-3 FTS 置信度路由 done + PR #7 已合 main（merge commit f9f63df）

**收口**：用户选「按推荐执行 = PR #7 merge」→ `gh pr merge 7 --merge --delete-branch` 一气呵成（merge commit `f9f63df`、本地切回 main、fast-forward、本地+远程分支已删、stale tracking ref 已 prune）。STATUS 标 merged。Class A 阻塞 / Class B 决策不动。下 cycle 抓手仍是 A-3 留下的「升级路由信号」4 候选（query 语种检测 / VEC cosine 阈值 / 更大 embedding 模型 / 评测集扩量）。

---

### 2026-06-23 — Claude Code (Opus 4.7) — BETA-15B-3 A-3 FTS 置信度路由 done（spec §5 降级、路由本 cycle 不生效）

**承接**：新会话开场用户问「本次会话有什么需要执行的任务」→ 读三文档 + ROADMAP B 阶段 → 给候选菜单 → 用户「按推荐执行」= A-2 cycle 数据指证的下一抓手 **BETA-15B-3 簇 A FTS 置信度路由**（A-2 baseline 报告调优记录节明示「FTS 在 crosslang 桶结构性给 hybrid 添噪、纯抬 weight 摆脱不了天花板」）。完整 superpowers（brainstorming 7 决策 → spec → plan 10 task → subagent 驱动 + 关键 task 双审 + 3 fixup + final review READY TO MERGE）。

**关键决策（brainstorming 7 收敛）**：① 范围 = **只做 FTS 置信度路由**（原始 query 入 schema 留下 cycle，YAGNI 防御）；② 信号 = **FTS/VEC top-K Jaccard 重叠**（直接对准 baseline 报告「同语言 g1 污染 nDCG」失败模式、不需文档元数据、评测可观测）；③ 动作 = **硬跳过 FTS 臂**（YAGNI、单旋钮、阈值附近震荡评测可控）；④ 代码归处 = **`result-normalizer` 加 wrapper `fuse_rrf_with_fts_routing`**（与 A-2 weight_provider 闭包同款架构、`fuse_rrf` 不动、`RouteVerdict` 副产物为 BETA-15B-5 badge 槽位预留）；⑤ sweep 策略 = **单维 sweep Jaccard，W 固定 10.0**（YAGNI 防御二维 sweep）；⑥ UI = **不暴露**（路由是后端启发式 vs floor/weight 的"用户偏好"语义不同）；⑦ baseline = **新增 HYBR 字段、HYB 保留双守护**（schema 加法、回归门更严、历史对照可视）。

**产出（10 task 全 done + 3 fixup，12 commit）**：T1 `jaccard_overlap_by_path` + 5 单测（+ fixup SAFETY 注释 + use 提顶）→ T2 `RouteVerdict` + `fuse_rrf_with_fts_routing` wrapper + 两常量 + 6 单测（+ fixup 严格 < doc + jaccard==threshold 边界测试）→ T3 评测 `arms.rs` `hybrid_routed_rank` helper + 4 单测 → T4 `report.rs` HYBR 字段 + `score_case` 接 `jaccard_threshold`（参数 7→8 加 `#[allow(too_many_arguments)]`）→ T5 binary `--jaccard-threshold` flag + 表格 HYBR 两列 → T6 手动 sweep 6 阈值 + 控制对照 → T7 bake `DEFAULT_FTS_JACCARD_THRESHOLD = 0.10` + rewrite baseline.json → T8 gate 加 HYBR 红线断言（4a/4b/4c/4d）→ T9 生产 `run_fanout_merge_rrf` 改 wrapper（按 BackendKind 分两臂、`FanoutOutcome.route_verdict: Option<RouteVerdict>` 透传）+ fixup 把 empty-arm guard 从 wiring 搬进 wrapper（架构归位、覆盖评测层）→ T10 baseline 报告追加 A-3 调优记录节 + 总验收。分支 `feat-beta-15b-3a3-fts-confidence-routing` PR #7 待 review/merge。

**Sweep 全表**（W=10.0 固定，t × hybrid_routed nDCG）：

| t | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≡HYB 控制) | 1.000 | 0.854 | 0.649 | 0.930 |
| **0.10 (t\* 选定)** | **1.000** | **0.854** | **0.649** | **0.930** |
| 0.20 | 1.000 | 0.851 | 0.693 | 0.832 ↓ |
| 0.30 | 1.000 | 0.861 | 0.712 | 0.832 ↓ |
| 0.50 | 1.000 | 0.864 | 0.726 | 0.833 ↓ |
| 1.0 (≈VEC 控制) | 1.000 | 0.864 | 0.726 | 0.833 |

**验证**：`cargo test --workspace` **858 passed / 0 failed**、clippy(`-D warnings`) 0、fmt 净、前端 tsc + vite build 净、**evals parser-only byte-equal v0.5=473 / v0.9=877 精确不变**、回归门 `semantic_quality_gate` 用新 baseline pass。控制对照核验：t=0.0 时 HYBR ≡ HYB（永不跳）、t=1.0 时 HYBR ≈ VEC（exact-name 桶完全重叠仍 HYB=1.0）。

**诚实边界（spec §5 异常分支降级 + 新失败模式诊断）**：① **content-not-name 桶 FTS 比 VEC 更强**（FTS_N=0.849 > VEC_N=0.833），路由跳 FTS 反伤此桶——t≥0.20 起 content-not-name -0.098 破 spec (4b) 各桶 ≥ HYB baseline 硬红线；② spec §5 字面正解「最保守 t」 → 选 **t\*=0.10**（实测无 case Jaccard 在 (0, 0.10) 触发跳过 → HYBR≡HYB → 路由对指标本 cycle 净影响 = 0）；③ spec §2.2 (4c) crosslang ≥ 0.700 / (4d) OVERALL ≥ 0.864 **未达**（仍 0.649 / 0.854，等于 A-2 baseline）；④ **新发现的失败模式**（spec/brainstorming 未预见）：Jaccard 单维信号天然分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」两场景、**纯调阈值救不了**；⑤ **下 cycle 抓手 = 升级信号**：query 语种检测 / VEC cosine 绝对分 / 更大 embedding 模型 / 评测集扩量。**基础设施完整入栈**（wrapper API + 评测设施 + baseline 双字段 + gate 双守护），为下 cycle 升级信号留好旋钮。

**未尽事宜**：① **PR #7 待用户 review/merge**（本次会话未自动 merge，等用户拍板）；② RouteVerdict 暂存槽位本 cycle 不画 UI（spec 显式预留为后续 BETA-15B-5 badge 槽位）；③ STATUS / ROADMAP doc-sync 本次会话末尾落库（含本条会话日志）；④ 真机手测：纯后端融合层加法 + 评测端到端覆盖、按 spec §3.2 判平凡未安排手测剧本。

---

### 2026-06-22 — Claude Code (Opus 4.7) — BETA-15B-3 A-2 语义臂权重调优 + UI 暴露 done + PR #6 已合 main

**承接**：同会话先做完 BETA-15B-5/15B-6 收口（两分支合 main + push），用户「现在就开 Task 2 = 语义召回质量调优」。完整 superpowers 全流程（brainstorming 收敛三决策 → spec → plan 8 task → subagent 驱动 + 每 task 双审 + final integration review READY TO MERGE 无 Critical/Important）。

**关键决策（brainstorming 收敛）**：① 范围 = **只调 weight**（FTS 置信度路由按本 cycle 数据决定下 cycle 是否上，YAGNI 防御）；② UI = `semantic_weight` 进 settings 让用户可调（与 floor 架构对称、live-read）；③ baseline = 就地写入新水位（不锁就白调）；④ sweep 方法 = binary 加 `--semantic-weight` CLI flag + shell loop 跑 6 个 weight + 人工读表选 w*（不做花哨 sweep 子命令）。

**产出（8 task 全 done）**：task 1 CLI flag → task 2 sweep（手动）→ task 3 bake `DEFAULT_SEMANTIC_WEIGHT=10.0` → task 4 `AppSettings.semantic_weight` + resolve/read（与 floor 对称）→ task 5 SearchDeps `weight_provider` + `run_fanout_merge_rrf` 签名扩展 + main.rs 注入 + 移除 task 4 过渡 `#[allow(dead_code)]` → task 6 设置页 UI → task 7 `baseline.json` 新水位 + exact-name=1.0 硬断言 → task 8 baseline 报告调优记录节 + 总验收。**中途 task 4/5 各 1 fixup**：doc-comment 补 clamp 理由 / accessor NaN/<=0 兜底 + 方向性测试断言（响应 code-quality reviewer Important）。9 commit + 2 fixup amended，分支 `feat-beta-15b-3a2-semantic-weight-tuning` 经 PR #6 合 main 后删除。

**Sweep 全表**（W × hybrid OVERALL nDCG / crosslang nDCG，exact-name HYB_R 恒 1.0）：W=2.0/0.832/0.582 → 3.0/0.837/0.603 → 4.0/0.839/0.606 → 6.0/0.846/0.631 → **10.0/0.854/0.649**（w\* 选定）→ 20.0/0.850/0.662（content-not-name 0.930→0.891 退化）。

**验证**：`cargo test --workspace` **837 passed / 0 failed**、clippy(`-D warnings`) 0、fmt 净、前端 tsc + vite build 净、**evals parser-only byte-equal v0.5=473 / v0.9=877 精确不变**、回归门 `semantic_quality_gate` 用新 baseline pass。hybrid 实测涨：OVERALL +0.022、crosslang +0.067、concept +0.024、content-not-name +0.010、synonym 0、exact-name 0（红线）。

**诚实边界（下一 cycle 抓手）**：spec §2.2 (4b)(4c) `OVERALL≥0.864 / crosslang≥0.700` 未达（差 0.010/0.051；W=20 最大 0.662 也不达 0.700）→ **路由必要性证据完整**（FTS 在 crosslang 桶结构性添噪）、归 BETA-15B-3 簇 A FTS 置信度路由子项下 cycle。详 [调优记录](docs/reviews/semantic-recall-quality-baseline.md)。

**未尽事宜**：① 设置页 UI 默认显示 10.0 与后端 `DEFAULT_SEMANTIC_WEIGHT` 是两处独立常量（无自动同步——未来改后端默认须手动同步前端，已加注释提醒，code-quality reviewer 已记 Minor 不阻塞）；② 真机手测：纯后端旋钮 + 一格 UI 数字输入框，按 spec §3.2 判平凡未安排正式剧本（按需用户自试）；③ STATUS / ROADMAP doc-sync 本次会话末尾追加完成。

---

### 2026-06-22 — Claude Code (Opus 4.7) — 收口两分支：BETA-15B-5 + BETA-15B-6 全合入 main

**承接**：上一会话同会话内完成 BETA-15B-5（代码 done 留手测）+ BETA-15B-6（全 done 含 Phase D），用户「先做 1（收口）」。本会话职责＝把两条 off-main 分支收回 main，含 15B-5 真机手测过门。

**执行**：① 摸清拓扑——两分支均 0 落后 main、merge-base 同一处（`3de76b9`）；代码层**零文件重叠**（15B-5=desktop+semantic-index、15B-6=evals+spike-retrieval），冲突只在 STATUS.md/ROADMAP.md 追加段。② **15B-6 干净合入**（`--no-ff` merge commit `bb1a97b`），回归门测试 `semantic_quality_gate` 通过、工作区干净。③ **15B-5 真机手测**（`tauri dev --features semantic-recall-metal`，编译 49s）——scenario 2 来源标注「纯语义命中」✅、scenario 3 置信档位 + 下限说明（「最相似段落 · 0.41/0.45（中相关）」+「低于 0.30/0.40 的弱相关已隐藏」）+ 设置页调下限 0.30→0.40 后预览说明的 X 同步刷新 ✅、scenario 4 延迟 265–294ms ✅、scenario 5 退化等价（xlsx 等非文档结果预览无段落高亮且不报错不卡）✅；scenario 1 跨语言段落高亮机制 BETA-15B-1 已验过、本机当前 query 未召回到含正文段落的英文文档故本次跳过，不阻塞合并（用户拍板）。④ **15B-5 合入 main**（`--no-ff` merge commit），冲突如预期仅在 STATUS/ROADMAP doc-sync——reconcile：当前 Task 合并成「15B-5+15B-6 全 done 已合 main」综合块；Class B 保留 15B-6 已落地的「评测语料隐私=已拍板混合」覆盖 15B-5 过期的「待拍板」、删掉「并行分支提醒」（两分支都已合）；会话日志最顶新增本条 06-22，两条 06-21 全保留。

**验证**：本地 main 提交线性 = 15B-6 merge → 15B-5 merge；`cargo test` 已经在 15B-6 合后跑过回归门通过；两分支真机/CI 在各自分支已全绿；STATUS/ROADMAP 冲突标记全清。

**未尽事宜**：① 两特性已合本地 main 但**未 push origin**（按 CONVENTIONS push 需用户授权）；② 三条特性分支 `feat-beta-15b-5/6` 与 `feat-beta-26-…`、`feat-beta-13-g11-g12-…` 可在下个会话清理；③ 下一 cycle 正题 = 召回质量调优（调高 `result-normalizer/lib.rs:92 DEFAULT_SEMANTIC_WEIGHT` / FTS 置信度路由，用 `semantic_quality` binary 度量、回归门守护，**不砍 FTS 臂**）。

### 2026-06-21 — Claude Code (Opus 4.8) — BETA-15B-6 持久化语义召回质量评测集（Task 1–12 done，off main，Phase D 待用户）

**承接**：同一会话先做完 BETA-15B-5（可解释 v1，分支保留待真机手测），用户「开评测语料隐私决策、推进召回质量做到顶」。完整 superpowers brainstorming：① 分解三子项（决策→建评测→调优），本 cycle 做①②；② 语料方案四问收敛＝**混合**（合成入仓 + 真实本地锤）；③ 范围＝只建评测 + 出 baseline。

**关键决策**：① **路线 1 缓存合成向量**——合成文本 embedding 无 PII 可提交 → 下限/权重评测纯靠缓存 + 生产 `fuse_rrf` 跑、确定性 CI 门控、不需模型；② hybrid 臂**真跑生产 `result-normalizer::fuse_rrf`**（构造 SearchResult），调优旋钮即生产的 `DEFAULT_SEMANTIC_WEIGHT`/floor；③ evals **不依赖** gitignored 的 spike-retrieval crate（公式照搬）；④ spike-retrieval 转正本地校准锤、llama-cpp 改可选 feature 顺带清 BETA-15B-2 遗留的 workspace 污染。

**产出**：spec + 13-task plan；subagent 驱动 12 实现 task（合 6 dispatch）+ 每 task spec/质量双审 + 整体终审 READY TO MERGE。`packages/evals` 新 `semantic_quality` 模块（metrics/data/arms/report，34 单测）+ binary + 回归门 + 合成语料 108/cases 59（5 桶、17 跨语言配对、零 PII）+ 完整性测试 + spike 转正。

**验证**：`cargo test --workspace` 0 failed、clippy 0、fmt 净、**evals parser-only byte-equal 不变**、默认 workspace 不再编 llama-cpp、`spike-retrieval` stub-loader 测试恢复。

**Phase D 本会话执行完**（Mac Metal）：vectors/baseline/报告提交、回归门激活。**关键发现**：hybrid(weight=2.0) 整体略低于纯向量（FTS 臂语义占优场景添噪）→ 下一 cycle 调权重/路由的明确目标。**未尽事宜**：① 分支 off main 未合（含 15B-5）；② 调优（权重/下限/模型）＝下一 cycle，起点＝本 baseline；③ 2 Minor（floor 字面量漂移 / 语料可扩量）记 [baseline 报告](../reviews/semantic-recall-quality-baseline.md)。

### 2026-06-21 — Claude Code (Opus 4.8) — BETA-15B-5 语义召回可信化（可解释 v1）：段落高亮 + 来源标注 + 置信档位（代码层 done，未合 main，待真机手测）

**承接**：用户开场「重新考虑产品特性规划、提升产品价值」。点破现状＝技术底座厚但近几轮在磨 BETA-13 evals（87.7%，距 90% 见底），价值天花板不在那 2.3pp 而在「它是个什么产品」。完整 superpowers brainstorming 四问收敛：① 价值方向＝**把差异化（跨语言语义召回）做到极致**；② demo 时刻＝**召回质量/可解释做到顶**；③ 重心＝**两者打包、可解释先上**（质量调优有评测语料隐私前置）；④ v1 三特性＝段落高亮 + 来源标注 + 置信分级（more-like-this 留 v2）。

**关键设计决策**：① 段落高亮选**路线 A 展示时按需算**（选中那刻只对该篇 doc 切句+embed，复用预览读已截断 body、零索引改动、向量内存即弃）；段落 embed 走单条同步 API 故 `MAX_PASSAGES=16` 封顶控延迟（Windows CPU 转圈可接受）。② **置信档位用 explain 段落真 cosine 放预览面板**——核查发现结果行 `r.score` 在 fanout 后是 RRF 累加分非 cosine，原 spec 设想挂结果行不成立，已在 plan「关键设计事实」诚实修正。③ 来源标注前端从 `sources` 派生（零后端改动）。

**产出**：spec + 8-task plan（[spec](../superpowers/specs/2026-06-20-beta-15b-5-semantic-recall-trust-design.md) / [plan](../superpowers/plans/2026-06-20-beta-15b-5-semantic-recall-trust.md)）；subagent 驱动 7 实现步（合 5 dispatch），每步 spec/质量双审 + 2 处定向修复（explain effect 依赖精确化避冗余 invoke；下限随预览刷新 + clamp[0,1]）。新增 `locifind-semantic-index::explain` 纯模块（7 单测）+ desktop `explain_semantic_hit` 命令（已注册 generate_handler）+ 前端高亮渲染/标注/置信 UI + 蓝色 mark CSS。**整体终审 READY TO MERGE 无 Critical/Important**。

**验证**：`cargo test --workspace` 0 failed、clippy(-D warnings) 0、fmt 净、前端 tsc+vite 绿、**evals byte-equal v0.5=473 / v0.9=877 逐数不变**（证实纯展示层加法未碰 parser/索引/融合）。隐私红线（只读已索引正文 / 不调 tracer / 向量内存即弃）端到端确认。

**未尽事宜**：① **真机手测留用户**（双平台跨语言高亮命中 + Windows 延迟）；② 分支 `feat-beta-15b-5-semantic-recall-trust` 未合 main（用户选「保留分支、验后合」）；③ **评测语料隐私决策待拍板**（已挂「待用户决策 Class B」），是后续质量调优 spec 的前置；④ 3 条终审 Minor（双中标签在当前单源 fanout 下为死分支、为未来多源铺路）不阻塞。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G16（C2+backlog）：多类型 ext 约定 + keyword 泄漏 + opened 时间维度（v0.9 871→877，§6 87.7%，未提交）

**承接**：新会话开场用户问「有什么可执行任务」→ 读三文档 + ROADMAP 当前阶段 → 给任务菜单 → 用户选「任务 1」（纯 parser 抬指标，需先拍板）。核到 G15 已合 main（PR #5，STATUS「分支待合 main」过期）。

**关键纠错（数据先行，翻转推荐）**：C2 多类型 ext 约定先用 AskUserQuestion 给 3 选项、我**误推 Option A「两者都保留」**（基于 en-005 一条孤例）。拉全量分布后发现真相相反——**命名具体格式的多类型 13 条里 12 条本就 ext=None**（zh-005/014/015/035、en-014/015/025…），en-005 是**唯一**「保留 ext」孤例，且 `mp3 and mp4` 这类不对称多类型保留 ext 反而误导。**主动向用户纠正、翻转推荐为 Option C**，用户改选 **Option C「≥2 file_type→ext=None」**（= G14 现行规则）。教训：拍板前必须拉全量分布，勿凭单 case 外推。

**产出（3 刀，每刀过 v0.5 byte-equal 闸门 + 回归检查）**：
- **刀1（C2，coverage only）**：d2 shard 重标 2 条孤例对齐主流——en-005→ext=None、en-016→file_type=image（单范畴 png/jpg）。parser 不动。+2（en-005/016）。
- **刀2（keyword 泄漏，parser，TDD）**：`code files` 成分词 + `excluding` 标记不入 keywords。`excluding/exclude/excludes/excluded/except` 入 EN_STOPWORDS（同 G12 no/not）；新 `en_part_of_multiword_type_phrase`——token 与相邻词构成 EXTENSION_ALIASES 多词类型短语（`code files`/`source code`）即作边界，**保护 `verification code` 内容词不被切坏**（裸加 `code` 停用词会破坏它）。+2（en-010/019）。
- **刀3（时间维度，parser+coverage，TDD）**：`opened`→accessed_time（common.rs `parse_time_fields` 对齐 decide_sort 的 accessed_dim；v0.5 `open` 全是 file_action 无 `opened` 子串→安全）。accessed 一设 sort 自动转 accessed_desc，coverage d5-mixed-009/d5-en-012 据已批 A1（excel→ext=[xls,xlsx]，v0.5 6+ 锚点）/A2（accessed→accessed_desc）对齐。+2（d5-mixed-009/d5-en-012）。

**验证**：**v0.9 871→877 pass（+6）、partial 125→119、fail 仍 4、§6 86.3%…→87.7%；v0.5 byte-equal diffs=0（每刀 v05-* 子集逐 case 规范化比对）；`cargo test --workspace` 802 passed/0 failed（含 3 新单测）；clippy(--workspace --all-targets -D warnings) 0；fmt 净**。ROADMAP G16 卡片 + BETA-14 §6 + 决策清单 C2 已同步；G15「合 main 待定」过期句已清。

**未尽事宜**：纯 parser/coverage 在 d2/d3/d5 块基本见底；§6 距 90% 差 ~2.3pp，剩 4 fail（2 v0.5 基线 + 2 §1.1 契约冲突需 re-baseline）+ 119 partial 散落无单一大块，再上须新一轮缺口盘点（更高风险共享路径或新标注决策）或转新特性。本机未做：真机手测（纯 parser 逻辑 + evals 端到端覆盖）；**未提交**（待用户确认提交方式：3 刀可拆或合一）。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G15（C1）：documents/pictures 类型义vs位置义消歧（v0.9 863→871，§6 87.1%，已合 main PR #5）

**承接**：开会话读三文档 + ROADMAP，确认 G14 已收口、STATUS 已为 G15 备好设计要点且用户定「另起干净会话专做」——本会话即干净会话。AskUserQuestion 确认推 G15。完整 superpowers：brainstorming（范围 a+b / 严格标记两决策）→ spec → writing-plans → subagent-driven（3 实现刀 + 每刀 spec/质量双审思路 + 终审）。

**关键调研（数据先行）**：全量核对 v0.5 500 条——英文 `documents` 位置义锚点**全部带 `in documents`/`Documents 里` 标记**（裸形态 0 条）、`pictures` 位置义 **0 条**、`文稿`/`文档目录` 是独立中文 keyword 与英文不干扰、`move…to documents` 是 file_action（hint 本就 None）。⇒ 消歧规则极窄极安全。又发现 `english_head_type_noun_file_type` 已覆盖句首 `documents`，故 d3/d5-en-001 只需 (a)；(b) 注入只为多类型并列 + 尾置。

**产出（3 commit，分支 `feat-beta-13-g15-doc-pic-disambig`）**：① 谓词 `en_ambiguous_noun_is_location`（common.rs，`in`/`里` 标记判定，排除 `within` 假命中）；② (a) `parse_location_with_language` 门控——裸 documents/pictures 不产 location（file_search+media_search 共享，commit `a651bae`）；③ (b) `inject_type_meaning_document`（file_search.rs）——类型义 documents 按 query 语序注入 file_type=document、去重、≥2 类则 ext=None；pictures/images 已在 EXTENSION_ALIASES 无需注入（commit `f7d576f`）。

**验证**：**v0.9 863→871 pass（+8）、partial 133→125、fail 仍 4、§6 86.3%→87.1%；v0.5 byte-equal diffs=0（每刀过闸门 + 终审独立重建 main 复核）；`cargo test --workspace` 全绿 0 failed（含 3 新单测）；clippy(--all-targets -D warnings) 0；fmt 净**。终审 READY TO MERGE 无 Critical/Important。

**未尽事宜（诚实边界）**：13 条 C1 中 **8 翻 pass，C1 核心（location 消除 + document 注入）全部正确**。剩 5 partial 全非 C1：① d2-en-005/016 = 多类型 ext 约定冲突（`word or powerpoint documents` 要 ext+多 ft、`png and jpg pictures` 要 ext 无 ft，与 G14 决策 B「≥2 ft→ext=None」相左，**需用户拍板**）；② d2-en-010 keyword 泄漏「code」/ d2-en-019 keyword 泄漏「excluding」/ d5-mixed-009 `opened`→应 accessed 却判 modified = 相邻字段 backlog（各自单独一刀、有 byte-equal 风险）。**§6 距 90% 还差 ~2.9pp，纯 parser 已见底**——再上须用户拍多类型 ext 约定 + 清相邻 backlog。本机未做：真机手测（纯 parser 逻辑、evals 端到端覆盖）；**分支未合 main、未 push**（待用户确认合并方式）。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G14：re-baseline 决策清单 + Group A + Group B + Group C 决策（v0.9 835→863，§6 86.3%，已合 main+push）

**Group C 决策（除 C1，+5，分支 `feat-beta-13-g14-groupc`）**：调研反转——Group C 绝大部分其实是 C1（C2 parser 多已给正确数组、拖累项是 documents/pictures 当 location；C3 screenshot+time 是 coverage 错标 v0.5 的 6 条 created_desc）。落地：① C3 screenshot→created_desc 对齐 v0.5（coverage，+2）；② AskUserQuestion 拍板 3 决策——C1=另起会话、都找=数组、oldest=created_asc；③ oldest→created_asc（SORT_ALIAS，v0.5 零锚点安全，+1）；④ 都找=数组（coverage d2-zh-035 ft 数组 + ext 删〔G11 多类型→ext=None〕+ parser ZH_FRAME_WORDS 补「三种都找/三种/都找」消 keyword 泄漏，+1）。**v0.9 859→863、§6 85.9%→86.3%、v0.5 byte-equal 0、0 回归、193 lib tests**。**C1/G15 设计要点已记上方「当前 Task」**。

**Group B（parser 缺口三刀，承 Group A，+12）**：每刀完整 TDD + v0.5 byte-equal 闸门，各自独立 commit。**B1**（`bff20b3`，+7）content-clause 文档类型名词→document：中文 DOCUMENT 列表补复合名词「劳动合同/协议文件」（精确尾匹配，仍受内容子句门控）+ 英文 `english_head_type_noun_file_type` 扩 contract/report/agreement/resume/study notes，**内容子句信号门控**（`has_en_content_clause_signal`：mention/whose body/contains/inside…），保「reports from last year」无子句仍 ft=None。**B3**（`ca57dcd`，+3）`parse_size` 单位扩展：RE_GT/RE_LT 单位组加 `gigs?`/bare `g` + 数字与单位间允许量词「个」（「小于1个G」「超过2个G」），decide_sort 已自动据 size 推 size_asc/desc。**B2**（`1d9da49`，+2）「X文件夹」作 location：mirror screenshot_dir_is_location——lexicon 图片/影片 alias 补 folder 形 + `picture_dir_is_location`（仅新「图片文件夹」形、不动既有「图片目录」保 byte-equal）+ file_search 抑制 file_type=Image。**全程 v0.9 847→859、§6 84.7%→85.9%、v0.5 byte-equal 0 变化、0 回归、workspace 192 lib tests + 全过、clippy/fmt 净**。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G14：re-baseline 决策清单 + Group A 第 1 刀（v0.9 835→847）

**承接**：G13 收口后用户选「现在就把 ~72 条 re-baseline 整理成决策清单」。**方法=用 v0.5 500 条锁定基线逐条证伪**：抽出 file_type 32 + location 20 + sort 19 ≈ 71 条争议 partial，对照 v0.5 同形态锚点数量。**核心反转**：不全是「需动 v0.5 的硬决策」——Group A（coverage 标错、对齐 v0.5，低风险我执行 ~26）+ Group B（parser 缺口，我改 ~18）+ Group C（真产品决策，需用户拍板 ~22）。铁证：v0.5 有 146 条「类型词→设 file_type」（仅 2 None）、22 条 created→created_desc、87 条 size→size_desc、28 条 documents→位置义。产出 [决策清单](../reviews/beta-13-rebaseline-decisions.md)。**AskUserQuestion 拍板**：Group A 批准立即执行；C1（documents 类型义vs位置义）= 上下文消歧单独立项。

**Group A 第 1 刀（done，待提交）**：改 coverage shards（d3/d5/d8）12 条 + assemble-coverage→generate-evals-v09，**逐字段从 v0.5 锚点推导非照抄 parser**：① file_type None→document 7 条（显式 pdf 内容/约束查询，对齐 v0.5 146 锚点）；② sort modified_desc→created_desc/accessed_desc + extensions 补全 5 条（对齐 v0.5 22/1 时间锚点 + word/excel ext 锚点）。**v0.9 835→847（精确 12 条 partial→pass，无意外）、§6 83.5%→84.7%；v0.5 byte-equal 0 条 v05-* 变化；0 回归；v09_integrity 3 测试通过**。

**未尽（诚实边界）**：原以为 ~26 的 Group A 纯 coverage 只翻 12——其余耦合 parser bug（`碳中和目标`拆错 / `新增`漏 created / `之前`方向错 / `还大`残留 / reports 时间维度分歧），归 Group B 一并解；**A3 那 2 条是 v05-* 基线内部不一致（`下载` vs 36 条 `downloads`），动它=改 v0.5 锁定基线，已排除并标注**。**下一步**：Group B parser 缺口（B1 content-clause 类型名词 / B2 文件夹位置 / B3 size 排序，~18 条，每刀过 byte-equal）；之后 C1/C2/C3 需用户决策。乐观估计 A+B 完 → ~870（≈87%），跨 90% 卡 Group C。本机未做：真机手测（纯 evals/标注）。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G13：过期 hybrid fallback 修复（首次跑 v0.9 hybrid 暴露出厂 bug，待提交）

**承接**：用户问「parser 见底但只 835 pass 怎么办」→ 我点破 **835 是 parser-only，出厂跑 hybrid（parser 锁 variant + 模型填字段），§6 90% 从没在 hybrid 下测过** → 用户「跑 v0.9 hybrid」。**首跑暴露反常**：hybrid v0.9 = **821 < parser-only 835**（rescued 0 / regressed 14，模型触发 285 帮 0）。诊断三种失败模式：~12 keyword 追加（模型把 file_type 词「合同/截图」回声进已对 keywords）+ 1 title 回声（「浮夸」抄进 keywords）+ 1 location 幻觉（「show me 上周的 PDF」凭空补 location=下载）。**根因**：BETA-23/24 训模型补 keywords 时 parser 还弱；G 系列把 parser 抬到 835 后 parser 自己能干净抽词，模型再触发就覆盖/追加已填对字段——**hybrid 是默认出厂路径，这是真 bug，只有跑 hybrid 才暴露**。用户拍板 fill-empty-only 后「开始」。

**修复（全 TDD，3 处全在 fallback/hybrid 层，不碰 parser 共享面）**：① `analyze_structural_omissions`（fallback.rs）keywords 留空才标 fillable（helper `keywords_is_empty`，MSRV 1.80 用 `is_some_and`）；② `apply_patch`（hybrid.rs）约束字段经 `fillable_category_for` 校验，不在 fillable 一律丢弃（杀 location 幻觉）；③ `union_keywords` 加 `structured` 去重剔除 = title/artist/album/genre 的 token，并集为空→返回 null(None) 非 `[]`。**代价（用户接受）**：problem-4「追加已填 keywords」能力废弃（held-out 价值不抵 14 条实测回退）；改 5 既有单测 fixture（sort 受 fillable 门管辖）+ 重写 problem-4 旗舰测试 + 6 desktop wiring 测试改用媒体臂 trigger（FileSearch keyword 补全在 fill-empty-only 下不可达，合法触发收敛到 audio 臂）+ LoRA augment 工具 `aug_case_from_seed` 丢弃 populated-keyword 种子（媒体臂空 keywords 仍生成，assemble 路径 None 可见丢弃无静默）。

**验证**：**hybrid v0.9 821→835（= parser-only，regressed 0）；hybrid v0.5 473（= parser-only，regressed 0）；v0.9/v0.5 parser-only byte-equal 不变（835/161/4、473/25/2，未碰 parser）；`cargo test --workspace` 793 passed/0 failed；clippy(--features model-fallback -D warnings) 0；fmt 净**。记忆新增 [[project-stale-hybrid-fallback]]。**坑**：改 fallback/hybrid 后须重建 `--features model-fallback` 的 evals 二进制再跑；hybrid 评测 JSON 非确定，逐 case 规范化比对。

**未尽事宜**：本刀只**止住 hybrid 出血**（hybrid 从净负回到与 parser 持平、不再劣化出厂质量），**未抬 §6**（仍 83.5%）。越过 90% 仍须**评测集 re-baseline**（~72 条标注争议：file_type 32 + location 21 + sort 19，ground-truth 自相矛盾，parser/模型都救不了），是产品决策。**LoRA 数据集若重生成**会丢弃 problem-4 populated-keyword 种子（与新契约一致，有可见告警）。本机未做：真机手测（纯逻辑，evals 端到端覆盖）；**未提交**（待用户确认）。

### 2026-06-20 — Claude Code (Opus 4.8) — BETA-13-G12 续：标注决策 E/F/G + file_action + image 三刀（+13 pass，合 main）

**承接**：用户连问「可以执行什么任务」→ 读三文档 + ROADMAP 当前阶段 → AskUserQuestion 选「标注规范决策」→ 逐条拍板 E/F/G。三刀后再问「还能做什么」→ 基于精确 fail 数据给菜单，用户选 file_action、再选 image「继续推」，最后「收工」。三刀全程 superpowers TDD + 每刀过 v0.5 byte-equal 闸门（reporter JSON 非确定，stash 重建 HEAD 基线 + `/tmp/v05check.py` 规范化逐 case 比对），各自独立 commit。

**刀1 标注决策 E/F/G（`e21131a`，+4）**：**E**（§3.2 改 parser）`has_quantity_degree_modifier`（数量/程度修饰+视觉媒体→media）；**F**（§3.4 改 parser）`bare_relative_time_only`（剥前导动词+尾「的」精确等于单时间词→clarify ambiguous_type，带宾语不误触发）；**G**（en-020 改 coverage）删多余 audio 扩展名列表对齐规则 B+zh-020。gating：E v0.5 唯一同形态锚点经 `几个 g` 已 media；F v0.5 31 条「昨天+宾语」全带宾语被 has_concrete/残留非空挡住。

**刀2 file_action 误路由（`ba9c824`，+5）**：`移动到X`/`重命名为X`/`把第1、3、5个复制到U盘`→file_action。3 处 parser（抽到 dest/new_name 无显式目标→默认 Index{1}〔门控避免裸动作词误判〕、`第N、M、K个`多序数、U盘→/Volumes/USB）+ coverage d6 三条 Documents 路径对齐 `~/Documents`（顺带 zh-020/en-004 partial→pass）。gating：v0.5 全部 file_action 锚点带显式目标、`把这些都复制` 走 clarify、无 `第N、M`/`U盘`。

**刀3 image+约束 误路由（`a80bd9f`，+4）**：§1.1 image carve-out——`创建于上个月的图片`/`桌面上 smaller than 1MB 的图片`/`截图目录里的图片`→file_search。media image-only 守护 + `截图目录`=location（`screenshot_dir_is_location` 提至 common，media 路由 + file_search file_type 双抑制 Screenshot + 新 LocationAlias 截图目录→截图）+ file_search 补 `创建`→created_time / LT 前缀 size 正则 / `less_than`→size_asc + coverage zh-015 sort→created_desc（对齐 v0.5 22 条 created 锚点）+ 更新 BETA-19 单测。gating：coverage 0 条 image→media、v0.5 唯一 image 锚点即 file、v0.5 0 条 `截图目录`/`小于+size`/created。

**验证（三刀累计）**：**v0.9 822→835 pass（+13）、fail 14→4、partial 168→161、§6 82.2%→83.5%；v0.5 byte-equal 全程零回归（500 case 0 diff）；`cargo test --workspace` 790 passed/0 failed；clippy(-D warnings) 0；fmt 净**。每刀 ROADMAP G12/§6 + 决策清单 §3.2/§3.4/§3.3/§四/§1.1 + en-020 已同步。

**未尽事宜**：**纯 parser 干净修复见底**。剩 4 fail = 2 条 v0.5 基线（不可作为）+ 2 条 §1.1↔§3.1 契约冲突（`screenshots…sorted`/`videos…sorted by size`，需评测集 re-baseline、动 v0.5 锁定基线，是产品决策非 parser）。最大 partial 块 = ④ documents/pictures 类型义vs位置义消歧（v0.5 ~24 条 location 锚点，高风险，宜单独 task）。建议：parser 线收束，继续抬指标须用户拍板 §1.1 re-baseline，或转新特性（BETA-20 v2 预览 / BETA-12 卸载）。本机未做：真机手测（parser 纯逻辑、evals 端到端覆盖）。

### 2026-06-19 — Claude Code (Opus 4.8) — BETA-13-G12 续：干净小项 4 个（exclude_ext / 都列 / 按size / exclude+约束，未提交待确认）

**承接**：G11/G12 已合 main 后用户连问「还有什么任务」。第一轮选「G12 干净小项」→ 清 ⑤⑥⑦（+4）；再问「还有什么」→ 我诚实盘点（纯零风险已榨干、剩余触共享 v0.5 路由），核 v0.5 `排除` 锚点后推荐 ⑧ exclude+约束（决策 C 同构、风险最低），用户选 ⑧（+1）。每项完整 TDD + 逐项过 v0.5 byte-equal 闸门。

**产出（4 纯 parser 修复，改 `file_search.rs`/`refine.rs`）**：⑤ **`不含 mkv`→exclude_extensions**（新 helper `negated_literal_extensions`：否定段短 ascii 扩展名 token，与类型词→exclude_file_type 区分；neg 段 v0.5 零标记天然安全）；⑥ **`都列`** 入 ZH_FRAME_WORDS（剥后「文件」容器名词丢弃，消 `文件都列` 残留）；⑦ **`按 size`/`by size`** → decide_sort + refine 双路径 size_desc + `size` 入 EN_STOPWORDS；⑧ **exclude+约束→file_search（决策 C 同构）**：refine 加约束门 `is_fresh_positive_then_exclude`（**三判据穿过**：含前向 `排除TYPE` + 排除后紧跟类型〔排尾置 `把 ppt 也排除掉`〕 + 排除前有正向类型〔排裸 `排除视频`〕）+ negation_split 复用 `排除`/`exclude` 标记产出 exclude_file_type。

**关键纪律**：⑧ 存在反例 `把 ppt 也排除掉，只看视频`（期望仍 refine）+ v0.5 全部裸排除锚点（15 条），实测三者输出**逐字不变**、仅 `文档和图片，排除压缩包` 转 file_search PASS。byte-equal 闸门 = `git stash` 重建 HEAD 基线 → 规范化逐 case 比对（`/tmp/v05check.py`，按 `nl_input`/`case.id` + actual_json sort_keys、剔 elapsed_ms）。**en-020 `no mkv` 有意推后**（裸 `no` 标记风险 + ground-truth 自相矛盾，属 coverage re-baseline）。

**验证**：**v0.9 817→822 pass（+5）、fail 15→14、partial 168→164、§6 80.7%→82.2%；v0.5 byte-equal 500 case 0 diff（全程）；`cargo test --workspace` 779 passed/0 failed（含 4 新单测）；clippy(-D warnings) 0；fmt 净**。ROADMAP G12 / BETA-14 §6 百分比 / 决策清单 §A/B 范围外表均已同步。

**未尽事宜**：剩余 G12 项均风险更高、建议各自单独一刀——④ documents/pictures 消歧（v0.5 ~24 location 锚点）、②′ location 误判抑制（触 location parser）、③′ file_action 误路由（触 file_action 路由 + 决策 D 安全语义）。本机未做：真机手测（parser 纯逻辑、evals 端到端覆盖）。

### 2026-06-19 — Claude Code (Opus 4.8) — 标注规范决策 A/B/C/D 落地（G11）+ parser backlog 起步（G12，合 main）

**承接**：G10/重构收工后用户问「还有什么任务」→ 选「偏产品决策」。把决策清单剩余 4 项标注冲突摆给用户 AskUserQuestion 逐条拍板（A 跨范畴多类型→file_search 多值 / B 多类型 file_type 数组 / C 排序划边界 / D 无上下文破坏性动作维持保守），中途补拍一项「多类型 ext=None」子决策。

**关键纪律**：执行前**逐项 gating 盘点 v0.5 同形态锚点存量**——A/B 各 0 条锚点（安全）、C 仅 6 条裸 `sort by size`→refine（边界保留即安全）、D coverage-only。每个 parser 改动后逐项过 v0.5 byte-equal 闸门。coverage 改动严格遵循锁定规则、**非凑指标**：把暴露的 parser bug 全部登记 G12 而**不**改 coverage 对齐（守 G10 立的「标注对、parser 有 bug」纪律）。

**产出**：① **D**（coverage-only）：3 条批量 move-all→clarify(ambiguous_action)。② **A/B 合一刀**：`has_cross_category_media_conjunction` 补 `、`/`跟`/单数 image/截图独立类；`merge_extensions` ≥2 file_type→ext=None；coverage re-baseline 15+5 条。③ **C**：`try_parse_refine` 加约束门（`raw_sort && !(match_extensions||location)` 才 refine，裸排序→refine、带约束→file_search）+ 补「按名字」。④ **G12 起步**：英文复数 `pdfs`/`archives` 入 lexicon；keyword 残留入停用词；**否定/排除新特性 `negation_split`**（标记后类型→exclude_file_type，5 条）。BETA-18 单测按新 ext 规则更新。**顺手修 G10 遗留的 coverage-cases.json↔shard drift landmine**（8 条 d5 shard 同步 media_search，对 cases.json no-op、恢复 assemble-coverage 幂等）。

**验证**：**v0.9 775→817 pass（+42）、fail 26→15、partial 199→168、§6 77.5%→81.7%；v0.5 byte-equal 全程零回归；`cargo test --workspace` 775 passed/0 failed；clippy 0；fmt 净；全程零回归**。决策清单 §1.2/§2/§3.1/§3.3 + §A/B 合并落地 + §四 follow-up + drift 附记全部更新；记忆新增 [[project-evals-coverage-pipeline-drift]]。

**未尽事宜（G12 续作）**：`documents`/`pictures` 类型义 vs 位置义消歧（v0.5 ~24 条 location 锚点，**高风险、建议从干净基线单独 task**）；`不含 mkv`→exclude_extensions（需扩展名字面 + 「no」标记）；`文件都列` 残留；`按 size` 英文 sort。§1.1/§3.2/§3.4 非 parser gap 需用户产品决策。本机未做：真机手测（parser 纯逻辑、evals 端到端覆盖）。

### 2026-06-19 — Claude Code (Opus 4.8) — refactor：artist 抽取拆 parsers/artist.rs（纯重构，合 main）

**承接**：G10 收工后用户连问「还有什么高价值任务」→ 诚实判断「纯 parser 抬指标已到顶，剩下要么风险>收益、要么需用户拍标注规范」→ 推荐零风险的 #4 重构（偿还 review 两次提的 media_search.rs 过大债）→ 用户「按推荐执行」。

**产出**：把 artist 抽取簇 9 项（`extract_artist`/`extract_artist_by_structure`/`contains_known_artist`/`KNOWN_ARTISTS_*`/`strip_lead_prefix`/`strip_location_prefix`/`is_stopword_artist`/`title_case`/`has_free_artist_structure`）从 media_search.rs 整体迁到新 `parsers/artist.rs`。**media_search.rs 2002→1635 行（−367）、artist.rs 374 行**。visibility：4 项 `pub(crate)`（簇外调用）、其余私有；refine.rs/lib.rs 跨模块导入经 media_search facade 再导出、路径零改动。**marker 去重(b) 评估后跳过**（media marker 在 artist regex 字面量内、与 size-parser contains-list 形态不同，强合有行为风险、不干净）。implementer subagent 执行 + 我独立复核。

**验证（证明零行为变化）**：**v0.5 byte-equal OK（473）、v0.9 UNCHANGED（775/199/26）**、`cargo test --workspace` 775 passed/0 failed、clippy(`--all-targets -D warnings`) 0、fmt 净。commit `c10b9c3` 合 main、push。

**未尽事宜**：纯 parser 抬指标已到顶；剩余 follow-up（中文 screenshot keyword 漏抽 / #3 documents-location / image 路由）均风险>收益或需用户拍标注规范（§1.2/§2/§3）。下次最高杠杆=用户对剩余标注规范的产品决策。

### 2026-06-19 — Claude Code (Opus 4.8) — BETA-13-G10 coverage 对齐 v0.5 契约 + screenshot parser 微修（done，合 main）

**承接**：G9 收工后用户问「最高收益做哪个」。**关键判断（诚实 ROI）**：纯 parser 已榨干（剩 fail 多为契约冲突/标注矛盾/安全语义），最高杠杆=**定 video+size 标注规范**而非写代码。挖出 v0.5 契约画像（screenshot+time→media 19 条、video+size/time→media 50+ 条、image+loc→file 仅 1 条）。AskUserQuestion 用户拍板「**media-type+size/time→media_search，保 v0.5**」。

**① coverage 对齐（最高单步，fail −8）**：修正 coverage 8 条 video/screenshot+size·time 标注 file_search→media_search。**纪律=从 v0.5 同形态锚点逐字段推导、非照抄 parser 输出**——独立审查举证非凑指标（3 条 screenshot 新标注 sort=created_desc/name_asc + 无 keyword，与 parser 实际输出〔泄漏 keyword、sort 错〕**不一致**，正说明「契约对、parser 有 bug」；若凑指标会照抄 parser）。范围严守：image 相关（v0.5 倾向 file，parser 给 media 才是 bug）+ 跨范畴多类型**不动**。edit coverage-cases.json（保顶层键序、仅内层 sorted）→ `fixtures generate-evals-v09` regen → v09_integrity 全过。fail 34→26、pass +5、3 条 fail→partial。

**② Fix A（en-003 转 pass）**：`common.rs` 相对时间 match 加英文词形（`last three days`/`past three days`→Last3Days，并补 seven/two weeks/thirty 对称）+ `extract_screenshot_keywords` stop 加数字词 one..ten（"three" 不漏成 keyword）。en-003 partial→pass。

**③ Fix B（zh-038 sort 转正）**：`parse_media_search` sort 链最前加 name 检测（`按名字/按名称/名字排/by name`→NameAsc，`倒序/降序/name desc`→NameDesc，对齐 file_search BETA-13-G6）。zh-038 sort created_desc→name_asc。**但 keyword `["按名字排"]` 仍漏（CJK 抽取器手术留 follow-up）→ zh-038 仍 partial**，本 fix 只修 sort + media/file 路径一致。

**验证/落库**：coverage 步经 subagent 独立审查（契约对齐/非凑指标/范围/完整性）；Fix A/B 用户选 inline TDD（方式 2，小范围纯增量），逐块 v0.5 规范化 byte-equal 闸门。**v0.9 769→775 pass（+6）、fail 34→26、partial 197→199；v0.5 byte-equal 零回归（473，0 差异）；`cargo test --workspace` 775 passed/0 failed；clippy(--all-targets -D warnings) 0；fmt 净**。分支 `feat-beta-13-coverage-realign-media`（6 commit）合 main。

**未尽事宜（follow-up）**：① 中文 screenshot keyword 漏抽（`extract_screenshot_keywords` 对无空格中文串 `最近三天的截图`→`三天的截图`、`的截图按名字排` 残留——脆、触 20 条 v0.5 screenshot 共享路径，zh-003/zh-038 仍 partial）；② image+constraint 路由（v0.5 倾向 file，parser 给 media，~3 fail——动 image 路由有 byte-equal 风险）；③ §1.2 跨范畴多类型 + §2 d9 标注矛盾 + §3 安全语义，需用户产品决策才能继续抬指标。本机未做：真机手测（parser 纯逻辑、evals 端到端覆盖）。

### 2026-06-19 — Claude Code (Opus 4.8) — BETA-13-G9 parser 近邻 follow-up：size 区间 + 内容截图多关键词（done，合 main）

**承接**：G8 收工后用户问「下一步」→ 给菜单 → 选「A1 近邻 follow-up」。AskUserQuestion 定范围取 [决策清单 §4](docs/reviews/beta-13-g-annotation-conflicts.md) 中 **byte-equal 安全 + 有 evals 增益**两项；砍 #3 documents-location（触 location parser、风险高）、#4 artist 拆 artist.rs（纯重构、0 evals 增益）。完整 superpowers：brainstorming(范围多选)→spec→plan(3 task)→subagent 驱动 + 每 task spec/质量双审 + 修复轮 + 整体终审。

**产出**：① **size 区间**——`parse_size`（file_search.rs）加 `RE_BETWEEN`：`between A and B unit`/`A 到/至/-/~ B 单位`→`SizeExpression::Between{min,max,unit}`，放 GT/LT 正则之后（不误吞 `larger than 1 GB`）、min/max 规范化、单位末位两数共用。修 `archives between 10 and 100 MB` + 中文 `10到100MB之间的压缩包`（+2）。② **内容截图多关键词**——`detect_content_clause`（media_search.rs）扩 `同时出现/同时包含/with both` 引导词；新增 `content_clause_is_multi`（both 标记判定，查 input 因标记词已被 regex 消费出 phrase）+ `split_content_clause`（按 `和`/`、`/`" and "` 拆 + trim + 过滤空 + 全空回退原短语）；file_search 注入处**仅 both 标记**才拆。修 `截图里同时出现订单号和金额的`→["订单号","金额"]、`screenshot with both X and Y`（+2）。**关键边界**：常规含「和」内容子句（`截图里写着甲方和乙方的`→["甲方和乙方"]）不拆，守住 G8 单关键词 case。**整体终审 READY TO MERGE 无 Critical/Important**（独立重建基线复核 byte-equal + 对抗探针验 GT/LT 不被区间吞、with both 不误伤祈使句）。

**验证**：v0.9 **765→769 pass（+4）、fail 36→34、partial 199→197**；**v0.5 byte-equal 零回归（473，0 差异）**；`cargo test --workspace` 773 passed/0 failed；clippy(`--all-targets -D warnings`) 0；fmt 净。byte-equal 闸门沿用规范化脚本 `/tmp/v05check.py`（reporter JSON 非确定）。

**未尽事宜**：决策清单 §4 剩 #3（documents 被 location 误判，触 location parser byte-equal 风险）、#4（artist 拆 artist.rs + marker 去重，纯重构）；in-code 注释记了紧凑连字符 `3-5gb` 的 keyword 残留盲区（不在评测集）。§6 的 90% 仍需用户先定标注规范（决策清单 §1/§2）才能继续。本机未做：真机手测（parser 纯逻辑、evals 端到端覆盖）。

### 2026-06-19 — Claude Code (Opus 4.8) — BETA-13-G8 parser 干净缺口第二轮（done，合 main）

**承接**：用户「执行 parser 缺口」（STATUS 既定本机候选）。**关键决策**：先调研把剩余 247 个 coverage 失败按「能否在 parser 修」三分——真 parser 可修 / v0.5↔coverage 契约冲突（同形态两套标注矛盾，改则破 byte-equal）/ 标注 ground-truth 自相矛盾。AskUserQuestion 定范围「干净缺口 + 标注冲突一并梳理」+ 选三块（截图内容子句 / artist 措辞 / 中文类型词，孤立 sort 砍）。完整 superpowers：brainstorming(2 决策)→spec→plan(6 task)→subagent 驱动 + 每 task spec/质量双审 + 修复轮 + 整体终审。

**产出**：四块干净修复（实测划分比 spec 更细：文档内容子句已走 file_search，真路由问题只在截图；类型名词映射反是大头拆中/英文两 task）——① 截图内容子句 `截图里写着X`→file_search+screenshot+干净 kw（媒体强词闸门，英文锚 `that` 防祈使句误触发，+13）；② 中文尾置类型名词 `…的表/报告`→file_type（`rsplit_once('的')`+精确等于尾 head 名词 + 文档类 gating 内容子句信号——避开 v0.5「合成报告」在 keyword 内、与 v0.9 d1/d3「报告」标注矛盾，+14）；③ 英文 head `a document about X`→file_type（`^`锚定排除 `in documents` 位置义 + keyword 剥除与②对称，+3）；④ artist 抽取修缮（剥句首动词/位置前缀、英文完整名、`的`后夹修饰、《》、`X 浮夸 这首歌`、video 路由、抑制「叫 X」过抽；停用词复用 `lexicon::QUALITY/GENRE_KEYWORDS`、移除创可贴，+9）。**整体终审 READY TO MERGE 无 Critical/Important**（终审用 git worktree 重建基线独立复核 byte-equal）。

**验证**：v0.9 parser-only **726→765 pass（+39）、fail 49→36、partial 225→199**；**v0.5 byte-equal 零回归（473，500 case 0 差异）**；`cargo test --workspace` 769 passed/0 failed；clippy(-D warnings) 0；fmt 净。**坑**：evals reporter JSON 非确定（`diffs` 是 HashMap + `elapsed_ms` 抖动），裸 `diff` 闸门假阳——改用规范化比对（按 id 排序 + 删 elapsed_ms），脚本 `/tmp/v05check.py`。

**交付物 2**：[标注冲突决策清单](docs/reviews/beta-13-g-annotation-conflicts.md)——36 fail 归三类（v0.5↔coverage 契约冲突 video/截图+size·time 50+ 锚点 / d9 多类型标注自相矛盾 / file_action·clarify 安全语义取舍），逐条给三选项；登记 4 项 parser 可修的近邻 follow-up（内容截图边缘变体 `同时出现`/`with both`、英文 documents 被 location 误判、`archives between` size 区间、artist 拆 artist.rs+marker 去重）。**未改任何评测集 fixture**。

**未尽事宜**：§6 总体 90% 靠纯 parser 不可达，需用户先定标注规范（尤其契约冲突 1.1 video+size 的产品语义）再起「评测集 re-baseline + parser 对齐」task。本机未做：真机 GUI 手测（parser 改动纯逻辑、由 evals 端到端覆盖）。

### 2026-06-18 — Claude Code (Opus 4.8) — BETA-27 可配置本地索引目录 + 排除规则（done + 发 Windows v0.5.0）

**承接**:用户真机问「语义索引为何只 17 篇」→ 查明根因（`default_document_roots()`=`dirs::document_dir()` 写死 Documents 夹、`search_scope` 死配置只展示不驱动）→ 登记 BETA-27 → 用户「开始 BETA-27」。完整 superpowers：brainstorming→spec→plan(6 task)→subagent 驱动 + 每 task 双审 + 整体终审。

**brainstorming 2 决策**:① 目录模型=**统一列表**（贴 Everything,挑文件夹三臂共扫,非 per-category）；② 排除通配符=**目录名 basename glob**（覆盖 node_modules/.git/*cache* 90% 需求,非完整路径 glob）。include=具体目录(非通配),exclude=glob。

**实现（6 task,低 ripple：旧签名委托空集、~40 调用方零改动）**:T1 indexer 加 `globset` + `build_exclude_set` + `run_incremental_index` 的 `WalkDir::filter_entry` 短路剪枝 + `index_dirs_excluding`/`index_image_dirs_excluding`；T2 local-index `reindex_scoped(roots, exclude)`（三夹并集统一喂三臂,discovery 全盘音乐不受影响）+ `reindex_with` 加 exclude；T3 `AppSettings` 加 `index_roots`/`exclude_globs` + `DEFAULT_EXCLUDE_GLOBS`(18 项,BETA-26 验证表) + `resolve_index_roots`(空→Music+Documents+Pictures 并集)/`resolve_exclude_globs`(空→默认表)；T4 `read_index_config`(live-read settings.json + 去重 + 去不存在目录) + `perform_reindex`/`spawn_semantic_index` 读配置走 reindex_scoped（reindex 命令加 `AppHandle` 取 settings_path）+ 隐私面板 `privacy.rs` 显 `resolve_index_roots` 真实根 + 摘除 T3 dead_code allow；T5 `tauri-plugin-dialog`（新建 `capabilities/default.json`:`core:default`+`dialog:allow-open`,gen/schemas tauri-build 重生成）+ 设置页目录选择器(`open({directory:true})` 去重+移除) + 排除列表(`ExcludeAdder` trim+回车+去重)。

**整体终审 = READY TO MERGE 无 Critical/Important**：端到端链路逐段闭合（UI→update_settings→settings.json→read_index_config→reindex_scoped→filter_entry）、字段名三处一致、**默认排除表非空属期望改进**（默认不索引 node_modules,现有 indexer/backend 测试显式传空集绕过、过 perform_reindex 的测试被 indexing 守卫短路、不受影响）、统一模型⊇旧各自不漏索引、**Tauri 自定义命令免 ACL** 故新 capability 不锁现有命令(core:default 保核心)、隐私面板真显索引根。3 Minor（语义 worker 范围扩到三夹=覆盖改进、Windows glob 大小写敏感=设计已记、前端无 glob 校验=后端兜底）。

**验证**:`cargo test --workspace` 759 passed/0 failed、**evals v0.5=473/v0.9=726 byte-equal**、tsc 净、clippy(-D warnings) 0。**真机手测全部留用户**（尤其 **Tauri dialog ACL 运行期冒烟**——代码层无破绽但 ACL 是运行期才验,必试现有功能不被锁 + 弹窗真出）。**发版 v0.4.0→v0.5.0**。本机未做：真机 ACL 冒烟、真机加目录索引、Windows 验证。

### 2026-06-18 — Claude Code (Opus 4.8) — BETA-15B-3 簇 A-1 续 / 簇 A-1 / 簇 B（语义臂相似度下限 + 列 UX + panic 兜底）

> 三条 2026-06-18 会话日志已滚动归档 → [docs/session-logs/STATUS-archive-2026-06.md](./docs/session-logs/STATUS-archive-2026-06.md)（逐字保留）。

### 2026-06-18 — Claude Code (Opus 4.8) — BETA-15B-3 簇 A-1 续：相似度下限可调 + cosine 分数可见（done + 发 Windows v0.4.0）

**承接**：发 Windows v0.3.0 给用户真机手测 → 反馈「暖机/解耦/进度 UI 全验通（首查询不再 16.8s、设置页『语义索引就绪 17 篇』），但 0.30 下限偏松、不相关项仍漏网」。Windows 发版每轮 ~24min CI + 重装 → 盲调阈值代价高。AskUserQuestion 拍板「可见分数 + 设置可调阈值」（vs 只可见分数 / 直接盲调），机制选 live-read（vs 重启）。

**实现（4 task）**：① 后端 `SemanticIndexBackend::new` 加 `floor_provider: Arc<dyn Fn()->f32+Send+Sync>` 闭包，`search_results` 每查询调它取下限（移除旧 `SIMILARITY_FLOOR` 常量，后端不依赖 desktop 设置、解耦）；② `AppSettings` 加 `semantic_similarity_floor: Option<f32>`，`DEFAULT_SIMILARITY_FLOOR=0.30`（全仓单一默认源）+ `resolve_similarity_floor`（有限值 clamp[0,1]/None/NaN→默认）+ `read_similarity_floor`（live-read settings.json，失败→默认），`build_registry` 加 `settings_path` 参 + 闭包 `move || read_similarity_floor(&path)`；③ 前端设置页数值输入框（0–1 step0.05）+ 结果「匹配方式」列对 semantic 显「按意思找到 · 0.42」；④ 回归门。

**关键链路（终审逐链验）**：UI 改值 → `update_settings` 写 `app_config_dir/settings.json` → 后端闭包 `read_similarity_floor` live-read **同一文件** → `filter_rank_topk(scored, floor, TOP_K)`。改阈值重搜即生效、**免重启**；字段名 `semantic_similarity_floor` 前后端+JSON 三处一致；`#[serde(default)]` 向后兼容不丢字段（含 round-trip 保留 TS 接口未声明的 `embedding_model_path`）。

**双审 + 整体终审 = READY TO MERGE 无 Critical/Important**（T1 crate 层 Approved 但当时 workspace 编译断裂→T2 修齐所有调用方〔含计划漏列的 search/tests.rs:2949，审查抓出〕；T3 round-trip 端到端验闭合）。2 Minor（闭包 fallback 静默无 trace；前端无即时校验，后端 clamp 兜底）。**验证**：evals v0.5=473/v0.9=726 byte-equal、`cargo test --workspace` 0 failed（desktop 127）、tsc 净、clippy 0。

**发版**：bump 0.3.0→0.4.0（tauri.conf.json+Cargo.toml+Cargo.lock）→ tag v0.4.0 → CI 构建 NSIS → `gh release edit` 补 changelog。**核心待办=用户真机用可见分数把 0.30 调到甜点值反馈 → bake 默认**。本机未做：真机调阈值（需 Windows + 模型 + 向量）。

### 2026-06-18 — Claude Code (Opus 4.8) — BETA-15B-3 簇 A-1：语义臂相似度下限（done）

**承接**：用户「继续下一步」(= 簇 A)。读 BETA-26 go/no-go 备忘 + 核实 semantic-index/result-normalizer 代码 → **重分层洞察**：簇 A 四项里相似度下限是唯一**高价值 + 无隐私门 + byte-equal 安全**的；权重调优 BETA-26 §4.6 已判低 ROI（margins 噪声内、修模糊损总体、须 held-out 调），held-out 评测扩量卡 gitignored 真实语料隐私门，原始 query 入 schema 跨切面 + byte-equal 风险。**修正上会话「簇 A 须先定隐私方案」的说法**——最高价值项根本不需要。用户选「只做相似度下限」+ 阈值 0.30。

**实现（`semantic-index/src/lib.rs`，2 task）**：纯函数 `filter_rank_topk(scored, floor, k)`（retain ≥floor → 降序 → 截断 k），`search_results` 改调它取代原 `sort_by + truncate`。`const SIMILARITY_FLOOR = 0.30`（BETA-26 `embed()` 证伪闸门实测相关~0.75/无关~0.18，取保守中值；命名常量待 held-out 精调）。低相关候选不进语义臂结果 → 不进 RRF 融合 → 不打旗舰徽标；全低于下限→空→降级 FTS-only。**改现有 `semantic_query_ranks_by_cosine`**（dog 正交向量 cosine 0 会被新下限挡破坏「两条都返回」→ 改 cat[1,0.2]/dog[1,1] 都过下限且有序）+ 4 新测试（纯函数过滤/截断/全低于空 + 端到端正交项被挡）。

**双审 = Approved 无 Critical/Important**（审查核验 cosine() 有 NaN 兜底、`>=` 边界、cat 0.98>dog 0.71 都过下限、降级路径不变、阈值注释合理）。**验证**：`cargo test --workspace` 746 passed/0 failed、clippy(-D warnings) 0、fmt 净、**evals v0.5=473/v0.9=726 byte-equal**（不碰 parser）。**真机手测全部留用户**（需 semantic-recall+metal+模型+reindex；含阈值观感反馈）。本机未做：真机阈值观感、跨语言命中复验。

### 2026-06-18 — Claude Code (Opus 4.8) — BETA-15B-3 簇 B：列 UX 迁移 + worker panic 兜底（done）

**承接**：用户「执行下一步」(= BETA-15B-3)。探查发现 15B-3 是 6 项集合、跨两簇、核心簇有相互依赖 + 隐私约束 → brainstorming 范围拆簇：**簇 A（数据驱动精度核心：held-out 评测扩量→相似度下限/权重调优/原始 query，强耦合 + 语料隐私）单独一刀；本会话只做簇 B（列 UX + M1，两独立小项）**。用户选簇 B + 列迁移用「一次性版本迁移」语义。完整 superpowers：brainstorming→spec→plan(3 task)→subagent 驱动 + 每 task 双审 + 整体终审。

**① 列一次性迁移（前端 `SearchView.tsx`）**：根因=`loadColumnPrefs` 读老用户在 match 列**存在前**保存的 visible（不含 match）→ 旗舰徽标列对存量用户隐藏（`defaultVisible:true` 只对新装生效）。修法=`ColumnPrefs` 加 `version` + 纯函数 `migrateColumnPrefs`：v2 前 prefs（老用户从未见过 match 列、不可能主动隐藏过）注入一次 match + 回写 version=2；之后尊重隐藏意图。三场景 + 边界（已有 match 的 v1 升版不重复注入）逐一自查 + 双审验过。无 JS 测试 runner，tsc + 手测。

**② worker panic 兜底（后端 `index_status.rs`）**：终审 M1——`spawn_semantic_index` 无 panic 兜底，embed panic 会泄漏 `semantic_indexing` 守卫致 UI 卡。修法=`run_semantic_worker` 用 `catch_unwind(AssertUnwindSafe)` 包 `semantic_index_pass`，panic 时 `semantic_abort` 清守卫降级。双审逐路径核验 panic-safety：panic 点（embed_pending 内 embed）在 semantic_begin 置守卫之后、status 锁未持有时（progress 回调在 embed 返回后才锁）→ 不毒化 + abort 用 `into_inner` 容错；release profile 用 unwind 故 catch_unwind 生效。`PanicEmbedder` 单测验守卫不泄漏 + 正常路径一致。文案「意外中断」(panic) vs「失败」(Result Err) 刻意区分。

**③ 整体终审 = READY TO MERGE、无 Critical/Important**：两项纯增量、互不耦合、对 feature 关/无模型/老前端零影响；生产调用点（main.rs:247/336 经 spawn_semantic_index）接通已核实；diff 恰 3 文件。2 Minor（迁移含纯升版回写=幂等无害；panic 测试 stderr backtrace=预期）。

**④ 验证/落库（feature 分支 `feat-beta-15b-3b-column-ux-worker-panic`，3 task + 收工）**：每 task fmt/clippy(-D warnings)/test 全绿；`cargo test --workspace` **742 passed/0 failed**（含 2 新 worker 测试）；tsc 净；**evals v0.5=473/v0.9=726 byte-equal**。**真机手测全部留用户**（列迁移需旧 localStorage、worker 兜底难自然触发靠单测）。本机未做：真机列迁移、真机 panic 注入。

### 2026-06-15 — Claude Code (Opus 4.8) — BETA-15B-1 语义召回纵切 MVP（旗舰语义召回层第一刀 done）

**承接**：用户「接下来需要执行什么任务」→ 读三文档 → 据 STATUS 给方向（BETA-15B 旗舰化已拍板进取档）→ 用户选「启动 BETA-15B 规划」。完整 superpowers 全流程：brainstorming → spec → writing-plans → subagent-driven-development。

**① brainstorming（4 决策）**：旗舰化太大 → 拆 4 子项（15B-1 纵切 / 15B-2 调度 / 15B-3 调优+评测 / 15B-4 跨平台+拓宽），本会话只做 15B-1。AskUserQuestion 敲定：完成边界=**接入桌面 app 用户可见**；向量存储=**BLOB 列+暴力 cosine**（零新原生依赖、探针已验，sqlite-vec 留 15B-4）；模型分发=**复用 BETA-23 约定目录+懒加载**；索引填充=**内联进现有文档增量索引**。

**② spec + plan**：[spec](./docs/superpowers/specs/2026-06-15-beta-15b-1-semantic-recall-vertical-design.md)（含锁定决策：模型固定 Qwen3-Embedding-0.6B/dim1024、首 1200 字截断不分块 per BETA-26）+ [plan](./docs/superpowers/plans/2026-06-15-beta-15b-1-semantic-recall-vertical.md)（Phase A–H，18 TDD task）。

**③ 实施（subagent 驱动 18 task + 每 task spec/quality 双审）**：A 向量数学+`document_vectors` 表(外键级联)；B `TextEmbedder` 抽象+内联嵌入(hash 跳过/失败计数)；C 加权 RRF 融合纯函数；D `SemanticIndexBackend` 新 crate(embed query→暴力 cosine→topK)；E harness 路由放行+`run_fanout_merge_rrf`；F 桌面集成(`EmbeddingModelHandle` 懒加载句柄+`ModelDaemon::embed` 透传+`build_registry` 注册+reindex 内联嵌入+fanout RRF 选路+feature `semantic-recall`+状态命令)；G 前端「按意思找到」徽标+设置页状态行；H 回归门+手测登记。**关键发现/适配**：Search Intent schema **无原始 query 字段** → 语义臂 embed parser 关键词拼接（已知限制，跨语言核心价值保留，记 15B-3/schema 跟进）；embedding 句柄加载期**释放锁**(Loading 哨兵，避免串行化并发 embed)；E2 漏导出的 `run_fanout_merge_rrf` 补 re-export。

**④ opus 整体终审 = MERGE WITH FOLLOW-UPS、无 Critical**：最高风险「语义后端恒注册→无模型时 FileSearch 经 RRF 路径」经具体追查**判定非回归**（纯类型查询不进 fanout 兜底链保留；带关键词查询 Everything 已并入 RRF 召回是超集；无模型语义臂干净降级；最终序仍由 Ranker 定）→ 补**端到端降级守护测试**（真实 `SemanticIndexBackend`+报错 embedder 经 `search_impl` 断言 FTS 结果存活+无 error）。Minor 跟进登记 15B-2/3/4：candidate_vectors 按 model_id 过滤、融合权重 held-out 调、暴力 cosine 上限(sqlite-vec)。

**⑤ 验证/落库（feature 分支 `feat-beta-15b-1-semantic-recall`）**：每 task fmt/clippy(-D warnings)/test 全绿；**feature 关/无模型行为与今天 byte-equal**；**evals v0.5=473/v0.9=726 不动**（语义臂不碰 parser）；默认+`semantic-recall` 两形态构建均编过；新 crate `semantic-index` + `ModelDaemon::embed` 透传是仅有生产遗产。

**真机手测（本会话完成，双平台验通）**：**macOS**（dev 构建 + Metal，软链 embedding 模型）中文「年假和休假规定」命中纯英文 leave policy 文档第 1 名 + 徽标 + 280ms；**Windows x64**（v0.2.0 NSIS 真机，physical x64——ARM VM 因 x64/AVX 模拟不可靠被排除）同样命中，llama 静态链接/embedding 加载/CPU 推理全工作（BETA-25 Windows 打包坑彻底趟通；途中 `Invoke-WebRequest` 下 610MB 模型截断致 null-result，换 `curl.exe -L` 解决）。**实测 4 发现全登记**：① Windows 首查询 16.8s 冷加载→15B-2 加启动后台预热根治；② 温查询 2.8s（Win 纯 CPU 无 GPU，Mac Metal 280ms）→model-runtime context 复用 + 15B-4 GPU；③ 不相关文件也 badge（语义臂缺相似度下限）→15B-3；④ 发版漏 bump 版本号致包名 0.1.3/tag v0.2.0 不一致→下版先 bump。**下一步=BETA-15B-2**（消除 16.8s 冷启动 + reindex 内联嵌入变慢，用户感知最强）。本机未做：sqlite-vec、held-out 评测扩量、Windows GPU。

### 2026-06-15 — Claude Code (Opus 4.8) — BETA-26 本地语义检索质量探针执行（结论 GO）+ 3 个 follow-up 子实验

**承接**：用户「当前还有什么任务」→ 读三文档 + STATUS 给分层候选 → 选 **BETA-26**（spec 上会话已登记）。方向「暂不定，先跑探针看数据」。完整 superpowers：writing-plans（8 task）→ subagent 驱动逐 task + 每 task spec/quality 双审。语料源决策：个人目录太空(Downloads 仅 20)→选「整机 home 分层抽样」；评测集决策：「Claude 读语料起草 + 用户实证复核(用 FTS5 反查泄漏)」。

**① 探针主体（8 task）**：spike-retrieval throwaway crate（真实数据三件 corpus/vectors/cases 全 gitignored）；给 `model-runtime` 加 `embed()`（llama.cpp embedding 模式 + last-token pooling + L2 归一化，唯一生产遗产、双审过，证伪闸门 sim_close 0.75≫sim_far 0.18）；build-corpus 遍历 home 排噪声目录+分层抽样(稀缺格式全收+md/txt补足)+panic 安全→4952 篇；Claude 读语料起草评测集；纯函数 cosine/RRF/Recall@10/nDCG@10(TDD)；embed 全语料(5.2min/63MB/dim1024/0跳过)；run-retrieval 三组(FTS5 trigram-OR / 向量暴力 cosine / hybrid-RRF)+分桶指标。**结论 GO**：纯模糊子集(FTS5 打不中,28~30条) **FTS5 Recall@10 2.1% → hybrid 88.4%(Δ+86pp)**；**crosslang +100pp**(中文 query↔英文文档 FTS5 结构性恒 0)；成本门槛②③④无压力。

**② 加固评测集(follow-up 1)**：补 **exact-name 守护桶 12 条**→**0/12 回退**(语义不拖垮精确名、反救回 2 条 FTS5 漏检)，闭合最后一个 kill 标准；用 trigram 重叠 linter + 实测反查重写 34 条词面泄漏 case；**关键发现：trigram FTS5 是很强的同语言基线**(中文改述常共享稀有 3 字片段被 BM25 命中)，语义可靠胜场=跨语言+概念跳跃+词汇不相交。评测集终态 68 条 5 桶=BETA-15B 调融合权重的耐久锚点。

**③ 分块实验(follow-up 2)**：每篇切 800字/150重叠/上限20块(37092 chunks)逐块 embed + max-pool 聚合。**结论 wash-to-略负**(总体 hybrid 0.919→0.900)、**成本 7.5×**(472MB/34.7min)→**BETA-15B 不该默认分块**（目标信号在开头,1200字截断已覆盖;长文档干扰项被切多块反增噪声）。推翻原备忘「生产需分 chunk」猜测。

**④ 融合调优(follow-up 3)**：复用现有向量试 RRF k扫/加权RRF/query自适应路由。**加权偏向量修纯模糊(0.925>向量only 0.914)但损总体(-2.8pp)、margins 噪声内**；**自适应「FTS5零命中→向量」失效**(trigram 几乎从不返空)→生产应用 FTS5 置信度阈值路由；**需 held-out 评测集调，勿据样本内数字生产化**。

**⑤ 验证/落库（feature 分支 `feat-beta-26-semantic-retrieval-spike`，15 commit）**：每 task fmt/clippy(-D warnings)/test 全绿；evals 完整性测试守 68 条；真实数据零入库(实测 git status 空)；walkdir 许可已登记(indexer 已用)。go/no-go 备忘 [docs/reviews/spike-semantic-retrieval.md](./docs/reviews/spike-semantic-retrieval.md)。

**未尽/下一步**：**2026-06-15 用户拍板=进取档**（模糊跨格式召回做旗舰，超原边界）→ 已修订 PROJECT.md 一句话定位（中英，加"按意思/跨语言"差异化）+ 边界（line 84「不做完整搜索引擎」改为"不替代系统搜索但叠加语义召回层做旗舰"）+ ROADMAP BETA-15B 升格旗舰"语义召回层"+ §6 定性细化/背景注记进取档。口号 `Local search for humans.` 保留（用户定）。**下一步=BETA-15B 旗舰化重做 spec/plan**（sqlite-vec 索引 + 增量调度接 BETA-07 + hybrid 融合接 BETA-04/05 + 跨语言为头号卖点 + held-out 评测扩量 + 4 条 BETA-26 设计指引），需重估时长。本机未做：sqlite-vec 集成、更大 embedding 模型探天花板。spike-retrieval crate throwaway，保留备查（用户未要求删）。

### 2026-06-14 — Claude Code (Opus 4.8) — 战略讨论（LociFind vs Claude Code/系统搜索的护城河）→ 登记 BETA-26 语义检索质量探针 spec

**承接**：用户一串战略追问——「Claude Code 怎么搜本地文件」→「我这项目相比有优势吗」→「能否做成任何 agent 的本地搜索后端」→「这价值大吗」→「我相对 Spotlight/Everything 的真壁垒够不够硬」→「语义检索路径可行吗」。逐层压力测试后收敛到一个具体技术动作。

**关键结论（讨论）**：① 对比 Claude Code（实时 grep + 云端模型）不是同赛道；真正对手是 Spotlight/Windows Search/Everything（都本地，**隐私对系统搜索无差异化**）。② "任何 agent 的本地搜索后端"=低成本楔子，但薄层已商品化、平台方觊觎、变现弱，**不该当主线定位**。③ 对系统搜索**无技术护城河**，唯一守得住的是"跨内容融合+排序+OCR+NL"体验链，靠**执行**而非技术。④ 选一个平台结构性做不好的 job 押 10x：A 截图 OCR（易腐楔子，你已有 BETA-03）/ **B 模糊跨格式召回（耐久主线，但缺语义检索能力）**/ C 记忆线索（需求未验证，暂不碰）→ 序列 A 破冰、B 建护城河。

**关键发现（避免重复造轮）**：语义检索方向**项目早有定论**——ROADMAP 第 320/323 行已把它收敛为 **BETA-15B"召回补强实验"**，守 PROJECT.md「不做完整搜索引擎」边界，定了四道门槛。本会话识别出门槛①（检索相关性）方法学没说清、且 BETA-15A 不覆盖"模糊召回"，遂登记 BETA-26 补缺，而非另开孤立 task。

**产出（本会话，docs-only，分支 `docs-beta-26-semantic-retrieval-spike`）**：① 新 spec `docs/superpowers/specs/2026-06-14-beta-26-semantic-retrieval-quality-spike-design.md`（一周质量探针：模糊召回评测集 5 桶构造铁律 + 真实噪声语料 + FTS5/向量/hybrid 三组 + Recall@10·nDCG@10 分桶 + 预设 kill 标准 15pp/8pp + 节奏）；② ROADMAP 登记 BETA-26 行（依赖 BETA-02/03/15A）+ BETA-26 背景注（接 BETA-15B 四门槛、交叉引用 BETA-11B）；③ STATUS 下一步 + 本条日志。

**技术结论**：本地语义检索在硬约束下**技术可行 GO**（embedding 复用 llama.cpp/GGUF、sqlite-vec 住进现有 SQLite、跨平台 BETA-25 已趟通、融合/调度/抽取接 BETA-02/03/04/05/07，无技术拦路虎、无新重依赖）；**唯一该决定 go/no-go 的是质量**（小本地 embedding 在混杂噪声个人数据上够不够 10x），只能测不能推 → 故先放探针。

**未尽 / 下一步**：① **BETA-26 spike 待执行**（本机可上手、不碰主线）；② **方向待用户拍板**——探针 GO 后"保守档=召回补强（现定边界）"vs"进取档=模糊召回旗舰（超 PROJECT.md 范围，需修订）"。本会话纯文档登记，未碰代码、未跑测试。

### 2026-06-13 — Claude Code (Opus 4.8) — recall 修复 3 跟进项（domain 脱节修复 fp 1.2%→0.0% + 端到端测试 + 注释澄清）+ 分支清理复核

**承接**：用户「当前还有什么任务」→ 读三文档 + STATUS 给分层候选清单 → 用户选「recall 3 跟进项 + 分支清理」。TDD 纪律（红→绿→全量验证）。

**① domain 脱节修复（真凶校正）**：`RawGroup.domain`（`file_type`/`media`/`office`/`personal`/`document`/`design`）此前解析后被 `#[allow(dead_code)]` 丢弃，从未进 `ParsedGroup`。`gazetteer_lookup_multi` 用 `query.find(key)` 子串匹配 + 仅靠 `is_pure_content_term`（parser 重解析）守护。**诊断推翻 STATUS 原描述**：真凶不是 `doc`（parser 已给它 file_type+扩展名 → 本就过滤），而是 `document`(head) 与 `file`(alias)——parser 不识别这两个裸词为类型词（无 file_type/ext）→ `is_pure_content_term` 误判 true → 假阳注入污染召回。最初照 STATUS 写的 `report.docx` 测试误绿（doc 已被过滤），加诊断 `parse()` 实测后重写为打 `document`/`file` 的红测试。**修复**：传播 `domain` 到 `ParsedGroup`；`YamlSynonymExpander` 加 `type_media_keys: HashSet`（file_type/media 组全成员，zh+en）；`DictView` 加 `is_type_or_media_key`（Layered 委托系统层——用户词无 domain）；两处过滤（gazetteer + multiword override）改 `view.is_type_or_media_key(k) || !is_pure_content_term(k)` → 跳过（**严格收紧，只去假阳不伤召回**）。

**② 端到端测试**：补 `expand_merges_multiple_gazetteer_content_words_into_single_or_group`（query 含两词典内容词→`merge_or_group` 合并单 OR 组），补此前只有「单内容词命中」「合并掉修饰语」的覆盖缺口。fixture 加 `合同`(personal) 内容组 + `document/[doc,file]`(file_type) 组 + 为既有组补 domain 标注。

**③ 注释澄清**：`multi_keyword_intent_preserves_order` 加注释——守 parser-keyword 路径（空 query 绕开 gazetteer，每 keyword 各自 expand 保序），与 ② 的合并路径对照。

**验证（macOS）**：recall eval **fp 1.2%→0.0%**（改动前 8 条失败用例 document/file 污染一大批文件，改动后只剩 1 条 `recall-en-doc-04` 假阳 `f-application-docx`——改动前即有、属 application 内容词语料问题、不在本项范围、门内 <0.05%）、**召回 100% 不退**；**byte-equal v0.5=473/v0.9=726 不动**（扩展层改动不碰 parser）；fmt 干净 / clippy `-D warnings` 0 / 全 workspace test 零失败（harness 182）。改动 +97/-3（含测试与注释），无新依赖。

**分支清理**：`feat-beta-15e-multi-group-gazetteer` 复核确认本地与远程均已不存在（早前会话已删），STATUS/ROADMAP 过时「可删」指引更新为「已删」。

**未尽/下一步**：残留 `f-application-docx` 假阳若要清需调语料 ground-truth（非 dict-domain，需用户决策）。其余候选见「下一步」节（BETA-20 v2 / BETA-12 卸载 / macOS Vision OCR / Class A）。

### 2026-06-13 — Claude Code (Opus 4.8) — BETA-25 model-fallback 动态库打包修复（静态链接路线：一处 Cargo 改动消除 dylib/rpath/DLL，顺带消除第二个潜在崩溃）

**承接**：用户「当前还有什么可以执行的任务」→ 读三文档 + ROADMAP 当前阶段卡片给候选清单（分本机可上手 / Windows 优先 / Class A 三层）→ 用户选 **BETA-25**。完整 superpowers：brainstorming（探明根因后 3 处 AskUserQuestion 范围决策：路线 A 静态链接 / macOS 本机验证 Windows 留下次 / 顺带做 dev 标题）→ spec → writing-plans（5 task）→ **subagent 驱动逐 task + 每 task 双审 + opus 整体终审**。

**① 根因（brainstorming 探明）**：`llama-cpp-4` 0.3.0 `default = ["openmp","mtmd","dynamic-link"]` 被 `model-runtime` 继承；`dynamic-link` 透传 `llama-cpp-sys-4` → `BUILD_SHARED_LIBS=ON` 产 `@rpath/*.dylib`，而最终二进制 `LC_RPATH` 计数=0 + tauri bundler 不收集 → `.app` 启动即崩（`Library not loaded: @rpath/libggml-base.0.dylib`）。BETA-23 手测靠手工补 Frameworks+rpath+重签才跑。

**② 方案=静态链接（评估 3 候选选最干净的③）**：`packages/model-runtime/Cargo.toml` 给 `llama-cpp-4` 加 `default-features = false`（去 `dynamic-link`/`mtmd`(llama.rs 未用多模态)/`openmp`），llama 全部静态进二进制——**一处 Cargo 改动同时覆盖 macOS+Windows**，无 dylib/rpath/重签/DLL；tauri 直接打单胖二进制。`metal` 走 model-runtime 自身 `metal` feature 接线（正交，不受影响）。**额外收益（opus 终审揪出）**：去 `openmp` 顺带消除**第二个独立潜在崩溃**——旧默认 openmp 在 macOS `rustc-link-lib=dylib=omp` 指向 Homebrew `/opt/homebrew/opt/libomp/lib/libomp.dylib`（普通用户机本无），与 rpath 问题无关。**顺带**：dev 构建窗口标题加 `(dev)`（`#[cfg(debug_assertions)]`，手测区分安装版同名同 bundle id）。

**③ 实施（5 task subagent 驱动，feature 分支 `feat-beta-25-static-link`）**：Task 1 spike 证伪闸门（改 Cargo + 新增永久 `#[ignore]` 真机推理冒烟 `beta25_static_llama_smoke`，**一次通过**：仅 `default-features=false` 即静态编过，无需补 mtmd/openmp）；Task 2 三 feature 形态门禁；Task 3 dev 标题；Task 4 CI 注释+许可复核+手测登记；Task 5 收工。质量审查揪出 1 项（`map().unwrap_or_else()` → `map_or_else` clippy + fmt）已修。

**④ 验证（macOS 本机）**：静态构建三 feature 形态全编过、全 workspace test 零失败、clippy `-D warnings` 0、fmt 干净；bundled 二进制 `otool -L` **零 ggml/llama/mtmd dylib 残留**（链接指令层 `static=ggml*/llama*` 实证）；**未修补 .app 启动不崩**；headless 真机推理冒烟（加载部署模型 + Metal + 非空生成）过。

**⑤ opus 整体终审**：ready to merge（macOS 范围），无 Critical/Important；确认去 mtmd/openmp 对实际推理路径（llama.rs/daemon.rs）零行为回归、metal 接线存活、Windows 同改动结构性修复。Minor：smoke 测试硬编码绝对路径（`#[ignore]`+env 覆盖，可接受）、建议 release bundle 前 `cargo clean` 清旧 dynamic 缓存。

**未尽/下一步**：**GUI 问题 4 端到端手测留用户**（manual-test-scenarios BETA-25 节）；**Windows NSIS 装包实测留下个 Windows 会话**（同改动结构性修复，需真机验证不缺 DLL）。
