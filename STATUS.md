# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**BETA-47/48/49/50 done（代码层）**——选项页七 tab + `enable_everything` 三处 es.exe 门控（47）；`embedding_model_path` 透传（48）；音乐发现按 roots 过滤（49）；**OCR 数字校正变体**（50，真机准考证 5→S 误识沉淀：原文保留 + 校正变体追加可搜）。待随下次发版真机验证。
- **下一步 top-3**：① v0.9.17 + BETA-47/49 真机验证（用户进行中：下载取消/镜像兜底/三行布局/卸载保模型/零索引空态/Everything tab 两态/音乐发现不越界/升级零损失）；② 设计伙伴/首个真实部署主动获取（护城河 P0）；③ 双平台真机复跑填 [beta-exit.md](docs/reviews/beta-exit.md) TODO 格。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（音乐全盘发现语义 2026-07-06 方案 A〔按 roots 过滤〕拍板并落地）。

## 当前 Task

**2026-07-06 VI（最新）**：**BETA-50 OCR 数字校正（真机踩坑同轮沉淀）**。用户反馈「搜 150138 找不到准考证 PNG」→ 实机诊断 index.db 实锤根因 = Windows OCR **5→S 误识 + 空格拆组**（`15013866763` → `1 S013866763`；图已入库、trigram 子串匹配本身正常，命中的两条全是文件名命中）。索引端修法：indexer 新增 `digit_correction_variants`（易错字母 S/O/I·l/B/Z → 数字 + 跨单空格分组合并；保守规则宁漏勿误：真数字 ≥4 且易错 ≤2、纯数字分组 ≥2 且 ≥6 位、链长/条数有上限）+ `finalize_ocr_text` 统一收口（**原文一字不改**、变体以〔OCR数字校正〕行追加正文尾部，正确号码与误识形态均可 trigram 命中）；两 OCR 引擎（WinRT/Tesseract）+ 扫描 PDF 逐页管线自动共享。indexer 182（+5：真机 case 四连 / 保守反例 / doc_db FTS e2e）全过、依赖面 local-index/desktop/server 零回归、clippy/fmt 净。**生效条件**：随下次发版；存量图片 mtime skip 需清空索引重建；locifindd 下次构建须重编（indexer 变更）。

## 下一步

1. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
2. **BETA-33 cycle 9 真机验证**：随下次发版装机，按 [manual-test-scenarios](docs/manual-test-scenarios.md) 跑六场景；本轮验证面另含 BETA-43（出处/`read_document`/审计导出，[playbooks README](docs/playbooks/README.md) 第 8/9 条）+ **BETA-12 卸载清理**（场景 5「升级零数据损失」为发版阻断；NSIS hook 首次真实构建即本次发版 CI）+ **BETA-29 意图草稿 v1（6 场景）+ v2（7 场景）**+ **BETA-47 选项页**（七 tab / Everything 检测两态 / 开关关闭后音乐发现回退 + 重启后 Everything 臂消失）+ **BETA-49 音乐发现不越界**（仅生效目录内音频入库）+ **BETA-50 OCR 数字校正**（新版装机后清空索引重建、搜 `150138` 应命中准考证 PNG）。
3. **v0.9.15 发版 done（并发首版）**：windows+macos 双 workflow 同 tag 并发均 success，[Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.15) 含 exe + DMG（aarch64）+ changelog；macOS DMG CI 首验通过。用户真机测试进行中；**BETA-45/46 改动未随包**、待下次发版验证。
4. **BETA-10 剩余**：macOS DMG 产物 CI done 且 **v0.9.15 首验通过**；剩 macOS 真机放行验证（§6.3）；winget 待 BETA-14 后 / Homebrew tap 可启动（DMG CI 已跑通）。
5. **BETA-40 真实内网证据**：唯一剩余验收项，依赖 ①。
6. **剩余 6 条 partial**（不阻塞出场线，[beta-exit §3.4](docs/reviews/beta-exit.md)）：全为 v0.5 标注锁定项（markdown ft / 「上个月下载的」动词歧义 / 项目归档 location / downloads hint 双语 ×2，改标注吃 §6.5 豁免额度）+ 备份文件两难。parser 可确定性收割已见底。
7. **BETA-29 v2 余量**：修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，唯一剩余项）。
8. **V10-16 主卡**（隐私 UI 集成 + 全量策略收口）：BETA-43 先导拆出后缩量，待 V 阶段。

**流程备忘**：Windows 发版 = bump 版本（tauri.conf.json + Cargo.toml）→ 推 `v*` tag 触发 release-windows.yml → Release 说明含 changelog（CONVENTIONS §8）。Windows 编带 llama 的 locifindd 一律用 `scripts\build-locifindd-llama.bat`。

## 阻塞 / 待用户决策

- **Class A（外部条件，阻塞出场评测，不阻塞代码）**：仅剩 BETA-09(a)/MVP-26/28 双平台 evals——需 Windows 真机 + 完整 Spotlight 索引 macOS。~~Apple Developer / 证书 / 域名 / 商标~~ **已取消（2026-07-04 开源免费拍板**，分发改 GitHub Releases 开源口径，[ROADMAP §5](./ROADMAP.md)）。
- **Class B（产品决策，不阻塞 §6 出场线）**：**已全部清零**——最新一项「Everything 音乐全盘发现 vs 零索引语义」2026-07-06 拍板**方案 A（发现结果按 roots 过滤）**并当场落地（ROADMAP BETA-49）；此前 clarify options 等各项均已落地。

## 会话日志

> 摘要 ≤5 条；全文与更早历史：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md) → [STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) → [STATUS-archive-through-2026-06-03.md](docs/session-logs/STATUS-archive-through-2026-06-03.md)。

### 2026-07-06 VI — Claude Code (Fable 5) — BETA-50 OCR 数字校正（真机准考证误识诊断 + 沉淀）

**承接**：用户问「为什么搜 150138 找不到准考证 PNG 内容」→ 实机诊断 index.db：图已入库、trigram 子串匹配正常，根因 = Windows OCR 把 5 识成 S（`15013866763` → `1 S013866763`）+ 空格拆组 → 用户拍板「现在就做」索引端校正。
**产出**：indexer `digit_correction_variants`（易错字母 S/O/I·l/B/Z → 数字 + 跨单空格分组合并；保守规则：真数字 ≥4 且易错 ≤2、纯数字分组 ≥2 且 ≥6 位）+ `finalize_ocr_text` 收口（**原文保留**、变体以〔OCR数字校正〕行追加，trigram 子串两态可搜）；两 OCR 引擎 + 扫描 PDF 逐页管线共享。
**结果**：indexer 182（+5：真机 case 四连 / 保守反例 / doc_db FTS e2e）、local-index 26、desktop + server 全量 exit 0；clippy/fmt 净。
**未尽事宜**：随下次发版生效；存量图片 mtime skip、需清空索引重建才带变体；locifindd 下次构建须重编（indexer 变更）。

### 2026-07-06 V — Claude Code (Fable 5) — BETA-48 修复 + BETA-49 音乐发现按 roots 过滤

**承接**：BETA-47 收工后用户指示继续处理两条顺带发现 → BETA-48 直接修、发现语义经 AskUserQuestion 拍板方案 A 后当场落地。
**关键决策**：音乐全盘发现改**按生效 roots 过滤入库**（发现器纯做加速、越界不入库；空 roots 连 es.exe 都不 spawn）——BETA-46 零索引语义对齐，BETA-01A「全盘入库」废弃；旧库越界记录不主动清（沿用「生效目录之外」提示 + purge 口径）。
**产出**：local-index 三处发现分支统一过滤 + `filter_discovered_to_roots` 纯函数；BETA-48 `embedding_model_path` 前端透传 + 语义 tab 路径覆盖 UI；文案「全盘发现」→「快速发现（仅限所选目录）」。
**结果**：local-index 26（+2，含改写的行为变更测试）/ desktop 全量 exit 0；clippy（清 2 条 doc 缩进）/fmt/tsc/vite 净。
**未尽事宜**：音乐发现不越界随 BETA-47 真机一并验证。

### 2026-07-06 IV — Claude Code (Fable 5) — BETA-47 选项页重构（七 tab + Everything 开关 + 拆文件）

**承接**：用户问「本次会话该做什么」→ 判定 BETA-47 为唯一标注「下会话」的主卡 → 用户拍板直接开工。
**产出**：① `enable_everything` 设置 + 三处 es.exe 门控（搜索后端条件注册〔重启生效〕/ 音乐全盘发现〔live、关闭回退目录扫描〕/ 模型本地发现〔live〕）+ `check_everything_available` 检测命令（与开关独立、非 Windows shim 恒 false）；② 七 tab（常规/索引/Everything/语义召回/Windows/隐私与记录/杂项，平台 tab 仅 Windows 显示；模型管理从常规迁入语义召回）；③ PreferencesDialog 1579→513 行、面板拆 `preferences/` 九文件。
**结果**：desktop 171（+1）/ local-index 24（+1，phase 级回归测试）全过；tsc/vite/clippy/fmt 净。
**未尽事宜**：BETA-47 真机验证随下次发版；顺带发现两条——**BETA-48**（前端 AppSettings 缺 `embedding_model_path`，UI 保存冲掉手工值，已登 ROADMAP B8）+ Everything 全盘发现 vs 零索引语义张力（进 Class B 待拍板）。

### 2026-07-06 III — Claude Code (Fable 5) — v0.9.16/17 双发版 + 真机反馈二轮修复

**承接**：用户拍板发 v0.9.16 → 装机实测回报下载卡死链等 → 逐条修复攒批 → 拍板发 v0.9.17。
**发版**：v0.9.16 macOS 首跑 E0433（target-gated 依赖坑）→ shim 修复 + dispatch 重跑 success；v0.9.17 双平台一次 success，并发机制三连稳。changelog 均补全。
**产出**：下载卡死链修复四刀（select 取消竞速〔连接阶段即刻生效〕+ connect_timeout 15s + hf-mirror 镜像兜底〔PRIVACY 同步〕+ model_download_in_flight 前端恢复下载态）；取消误报失败修复（invoke-catch 补过滤）；目录三行卡片布局（路径/统计/按钮分行）。BETA-45 真机首验：发现 UI 工作（Everything 命中 artifacts 模型）。
**结果**：desktop 170 全过、tsc/vite/clippy/fmt 净；[Release v0.9.17](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.17) exe+DMG 齐。
**未尽事宜**：v0.9.17 待用户验证（取消即刻生效/镜像/三行布局/卸载保模型弹窗/零索引空态/升级零损失）；`gh run watch` 假退出 ×3 → 一律 --json 轮询。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-06 II — Claude Code (Fable 5) — v0.9.15 并发发版 + cycle 9 真机反馈落地（BETA-45/46）

**承接**：用户拍板 push + 并发发版 → 真机测 v0.9.15 → 回报三条反馈 → 三项拍板后当场实现 ①②、③登记下会话。
**发版**：v0.9.15 双平台并发**双 success**——macOS DMG CI 首验通过（aarch64 DMG 产出）、并发同 Release 幂等追加成立；changelog 补全。踩坑：`gh run watch` 假退出 ×2，改 `--json status` 轮询。
**产出**：**BETA-45** 模型本地发现 + 卸载默认保模型（NSIS `/SD IDNO` + 同卷 Rename 暂存；everything `find_files_named`〔wfn: 精确名 + UTF-8 导出〕；discover/import 命令 + 白名单 + 原子落盘 + 复用下载 done event；ModelDownloadStep 发现 UI）；**BETA-46** 默认零索引（`resolve_index_roots_tagged` 三夹仅勾选纳入、空+false=零索引）+ checkbox 常显 + banner 退役 + 路径完整显示；**BETA-47** 选项页重构登记（ROADMAP 新 B8 小节）。
**结果**：desktop 168 / everything 15 / settings 四分支 / uninstall 闸门（+2 断言）全绿；tsc/vite/clippy `-D warnings`/fmt 净。根因诊断：反馈① = BETA-12 整目录删含模型；反馈② = 空 roots 兜底三夹旧语义。
**未尽事宜**：BETA-45/46 随下次发版真机验证（NSIS 弹窗须真装真卸）；升级行为变化（空 roots 老装机停索三夹）随 cycle 9 复测确认；BETA-47 下会话。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。
