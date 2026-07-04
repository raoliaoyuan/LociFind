# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 97.7%**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**开源化整线落地 + 首个公开发版 done**——双许可 + PRIVACY + 脱敏（BETA-00 done）→ 仓库转公开（orphan 首发，archive 冻结保全史）→ CONTRIBUTING/模板 + install.md/渠道评估（BETA-10/10A 文档层）→ **v0.9.14 CI 成功（NSIS hook 首次真实构建通过）+ changelog 补全 + Scoop bucket `scoop-locifind` 上线**。前一轮：evals 97.7% + BETA-29 v2 代码层完。
- **下一步 top-3**：① 设计伙伴/首个真实部署主动获取（护城河 P0，开源免费降低试用门槛）；② BETA-33 cycle 9 真机验证（随下次发版，验证面含 BETA-43 出处 + BETA-12 卸载/升级 + BETA-29 草稿 v1+v2）；③ v0.9.14 装机真机验证（cycle 9 六场景 + BETA-10A「下载→放行→装→可用」+ Scoop 装机路径，随下次上机）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；Class B 仅剩 1 项：clarify options 结构口径（危险动作给不给「在访达显示/取消」、模糊查询给不给类型/动作 options，8 条，不阻塞出场线）。

## 当前 Task

**2026-07-04 XI（最新）**：**开源免费定位落库**。用户拍板：不做 Class A 商业分发前置（Apple Developer / 证书 / 域名 / 商标），LociFind 走开源免费路线，**MIT OR Apache-2.0 双许可**。落地：LICENSE-MIT + LICENSE-APACHE 入库、Cargo workspace `license = "MIT OR Apache-2.0"`、desktop package.json 补 license 字段；PROJECT.md（核心原则 + 路线图 + 不做什么）、ROADMAP（§2/§4/§5/§6.3/§6.4/§7/§8/§11 + BETA-00/10/10A re-scope + V10-08 缩量 / V10-09 dropped）、README（简介 + License 章节）同步。**双平台真机 evals 与设计伙伴 P0 不受影响**（质量验证与商业分发无关）。开源前置新增两项检查归 BETA-00：Everything SDK 再分发条款、公开仓库脱敏。

**前一轮 2026-07-04 IX+X**：两轮 partial 收割 + 四项口径拍板落地 + BETA-29 v2。**第一轮**（IX）：① 时间表达簇——before/after 绝对日期（年月无日 / 英文月名 / 混排 / 汉英数词月）+ 日期区间 + 措辞（这周/这个月/新增/做的/最近拍）+ created→created_desc 翻转收窄 + 标题词不作时间；② keywords 小刀——月份名/序数/数字词停用、「报告」sole-keep、「又」分隔、「预算表」compound、「比X还大」size、图片内容子句整尾短语、"the word" 消歧；标注离群对齐 3 条 → **952/48/0（95.2%）**。**第二轮**（X，用户四项拍板全按推荐落地）：复数归一（装配终点 + minutes/news 例外 + report sole-keep）、language 降出严格匹配（judge）、clarify question 核实为既定实现（剩 8 条是 options 结构差异另拍板）、ext-ft 标注对齐 6 条 + G15 谓词扩展（in the <kw> / 句首 documents 里 / 位置义 pictures 抑制）+「几百KB」启发 → **v0.9 = 977/23/0（97.7%）、v0.5 = 490/10/0**，全程逐 case 零回归（[复盘 §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)）。**BETA-29 v2**：`SavedSearch.intent`（向后兼容）+「保存草稿…」+ 带 ⚙ chip 重跑走 `search_with_intent`；新命令 `preview_intent`（只解析零执行）+ 搜索框 ⚙ / Shift+Enter 搜索前预览。

## 下一步

1. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
2. **BETA-33 cycle 9 真机验证**：随下次发版装机，按 [manual-test-scenarios](docs/manual-test-scenarios.md) 跑六场景；本轮验证面另含 BETA-43（出处/`read_document`/审计导出，[playbooks README](docs/playbooks/README.md) 第 8/9 条）+ **BETA-12 卸载清理**（场景 5「升级零数据损失」为发版阻断；NSIS hook 首次真实构建即本次发版 CI）+ **BETA-29 意图草稿 v1（6 场景）+ v2（7 场景）**。
3. **v0.9.14 发版 done**：CI 成功（run 28708792924，NSIS hook 首次真实构建通过）+ [Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.14) changelog 补全（prerelease）+ Scoop bucket [scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind) 上线（`scoop bucket add locifind <url> && scoop install locifind`）。装机包供下一步 ② 真机验证。
4. **BETA-10 剩余**：macOS DMG 产物 CI（下次触碰 macOS 侧时）；winget 待 BETA-14 后 / Homebrew tap 待 DMG CI。
5. **BETA-40 真实内网证据**：唯一剩余验收项，依赖 ①。
6. **剩余 23 条 partial**（不阻塞出场线，明细见[复盘 §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)）：clarify options 结构口径 8 条（待拍板，见「阻塞」）；v0.5 老账 10 条（hint 双语形态 / synthetic-artist ×4 / markdown ft 等，卡 §6.5 豁免额度、攒批处理）；零星 5 条（碳中和分词 / 保密协议 ft（d3 标注自身不一致）/ 备份文件 两难 / 裸 no / music 目录 hint 形态）。
7. **BETA-29 v2 余量**：修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，唯一剩余项）。
8. **V10-16 主卡**（隐私 UI 集成 + 全量策略收口）：BETA-43 先导拆出后缩量，待 V 阶段。

**流程备忘**：Windows 发版 = bump 版本（tauri.conf.json + Cargo.toml）→ 推 `v*` tag 触发 release-windows.yml → Release 说明含 changelog（CONVENTIONS §8）。Windows 编带 llama 的 locifindd 一律用 `scripts\build-locifindd-llama.bat`。

## 阻塞 / 待用户决策

- **Class A（外部条件，阻塞出场评测，不阻塞代码）**：仅剩 BETA-09(a)/MVP-26/28 双平台 evals——需 Windows 真机 + 完整 Spotlight 索引 macOS。~~Apple Developer / 证书 / 域名 / 商标~~ **已取消（2026-07-04 开源免费拍板**，分发改 GitHub Releases 开源口径，[ROADMAP §5](./ROADMAP.md)）。
- **Class B（产品决策，不阻塞 §6 出场线）**：7-04 X 四项拍板已全部落地（复数归一 / language 降出 / clarify question 核实既定 / ext-ft 对齐，[复盘 §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)）。**仅剩 1 项**：**clarify options 结构口径**（8 条）——d6 危险动作 4 条：标注期望无 options、parser 给「在访达/资源管理器中显示 / 取消」；d8 模糊查询 4 条：标注期望类型/动作 options（如「文档/图片/视频/音乐」）、parser 不给。方向 = 统一「danger 类给安全出口 options、vague 类给消歧 options」并对齐标注，或维持现状。

## 会话日志

> 摘要 ≤5 条；全文与更早历史：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md) → [STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) → [STATUS-archive-through-2026-06-03.md](docs/session-logs/STATUS-archive-through-2026-06-03.md)。

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

### 2026-07-04 IX+X — Claude Code (Fable 5) — 两轮收割至 97.7% + 四项口径拍板落地 + BETA-29 v2

**承接**：STATUS 下一步 ④⑤（用户选定三摊活）；第一轮后用户四项口径全按推荐拍板、当场落地。
**第一轮产出**：① 时间簇（`parse_absolute_bounds` 九形态：年月日/年月 之前之后、英文月名、混排、汉英数词月、中英区间；`parse_year` 抢跑顺序 bug 修复；这周/这个月/最近拍/新增/做的；decide_sort created 翻转收窄〔相对时间+创建触发词〕；media 标题先抽再解时间）；② keywords（EN 月份名/序数/数字词/most 停用、报告 sole-keep、又 分隔、预算表 compound、比X还大 size、图片内容子句整尾短语、"the word" 消歧）；③ 标注离群对齐 3 条（各对 5:1+ 锚点多数派）→ 952/48/0。
**第二轮产出（四项拍板）**：复数归一（`singularize_en_keyword` 装配终点做、不进 residual 抽取面〔fallback 遗漏分析复用该面，踩坑后重构〕、minutes/news/series 例外、report sole-keep）；language 降出严格匹配（`compare_json` 跳过，分语言统计不变，v0.5 +11）；clarify question 核实**既定实现**零变更（剩 8 条是 options 结构差异，另立拍板项）；ext-ft 对齐 6 条 + G15 谓词扩展（`in the <kw>`、句首「documents 里」闸门、位置义 pictures 不作 Image）+「几百KB」→<1MB 启发。
**BETA-29 v2**：`SavedSearch.intent` + `save_search` intent 参（`validate_draft_intent` 闸门）+「保存草稿…」（与重跑共用 buildDraft）+ ⚙ chip 走 `search_with_intent`；新命令 `preview_intent`（parser+Refine、无模型、零 tool call）+ ⚙/Shift+Enter 预览入口。
**结果**：**v0.9 = 977/23/0（97.7%）、v0.5 = 490/10/0**；四轮逐 case 对比全程零回归。intent-parser 230（+15）/ evals 全 gate（+1 judge 测）/ desktop 168（+3）全绿；clippy `-D warnings`/fmt/tsc/vite build 净。复盘追记：[gap-inventory §3.5](docs/reviews/beta-14-gap-inventory-2026-07-04.md)。
**未尽事宜**：clarify options 结构口径 8 条（Class B 唯一剩余）；BETA-29 v2 剩 BETA-30 联动项；cycle 9 手测清单已补 v2 七场景。




