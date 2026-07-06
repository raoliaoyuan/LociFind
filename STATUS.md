# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割 done**——[beta-exit.md](docs/reviews/beta-exit.md) 骨架落（parser-only 全填、真机格标 TODO）；clarify options 方案 A 拍板并落地（Class B 清零）；老账收割 9 条 → **v0.9 994/6/0（99.4%）、v0.5 495/5/0**，逐 case 零回归。前一轮：开源化整线 + v0.9.14 首个公开发版 done。
- **下一步 top-3**：① 设计伙伴/首个真实部署主动获取（护城河 P0，开源免费降低试用门槛）；② 双平台真机复跑填 [beta-exit.md](docs/reviews/beta-exit.md) 的 TODO(真机) 格（子集命中率/索引资源占用/安装可用/性能 p95）；③ BETA-33 cycle 9 + v0.9.14 装机真机验证（六场景 + BETA-10A + Scoop，随下次上机）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（clarify options 结构口径 2026-07-06 方案 A 拍板落地）。

## 当前 Task

**2026-07-06（最新）**：**BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割**。① 起草 [beta-exit.md](docs/reviews/beta-exit.md)（§9 模板，parser-only 数据全填、真机相关格标 `TODO(真机)`，B→V checklist 必交付项）。② clarify options 结构口径（Class B 唯一剩余）拍板**方案 A**并就地落地（[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）：evals 只校验 options 结构存在性（Array vs null），故按 reason 定「带不带」——非 Unknown 一律挂标准 options，parser（`clarify_with` + `standard_options`）与标注（d6/d8 共 17 条）双向对齐；v0.9 8 条 clarify partial 全清、Clarify 桶 67/0/0。③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符 / 裸 no+扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。**结果：v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 235 测 + evals/harness/server 全 gate + clippy/fmt 净。详录 [session-details-2026-07](docs/session-logs/session-details-2026-07.md)。

## 下一步

1. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
2. **BETA-33 cycle 9 真机验证**：随下次发版装机，按 [manual-test-scenarios](docs/manual-test-scenarios.md) 跑六场景；本轮验证面另含 BETA-43（出处/`read_document`/审计导出，[playbooks README](docs/playbooks/README.md) 第 8/9 条）+ **BETA-12 卸载清理**（场景 5「升级零数据损失」为发版阻断；NSIS hook 首次真实构建即本次发版 CI）+ **BETA-29 意图草稿 v1（6 场景）+ v2（7 场景）**。
3. **v0.9.14 发版 done**：CI 成功（run 28708792924，NSIS hook 首次真实构建通过）+ [Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.14) changelog 补全（prerelease）+ Scoop bucket [scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind) 上线（`scoop bucket add locifind <url> && scoop install locifind`）。装机包供下一步 ② 真机验证。
4. **BETA-10 剩余**：macOS DMG 产物 CI（下次触碰 macOS 侧时）；winget 待 BETA-14 后 / Homebrew tap 待 DMG CI。
5. **BETA-40 真实内网证据**：唯一剩余验收项，依赖 ①。
6. **剩余 6 条 partial**（不阻塞出场线，[beta-exit §3.4](docs/reviews/beta-exit.md)）：全为 v0.5 标注锁定项（markdown ft / 「上个月下载的」动词歧义 / 项目归档 location / downloads hint 双语 ×2，改标注吃 §6.5 豁免额度）+ 备份文件两难。parser 可确定性收割已见底。
7. **BETA-29 v2 余量**：修正样本入 BETA-30 失败样本箱（依赖 BETA-30 开工，唯一剩余项）。
8. **V10-16 主卡**（隐私 UI 集成 + 全量策略收口）：BETA-43 先导拆出后缩量，待 V 阶段。

**流程备忘**：Windows 发版 = bump 版本（tauri.conf.json + Cargo.toml）→ 推 `v*` tag 触发 release-windows.yml → Release 说明含 changelog（CONVENTIONS §8）。Windows 编带 llama 的 locifindd 一律用 `scripts\build-locifindd-llama.bat`。

## 阻塞 / 待用户决策

- **Class A（外部条件，阻塞出场评测，不阻塞代码）**：仅剩 BETA-09(a)/MVP-26/28 双平台 evals——需 Windows 真机 + 完整 Spotlight 索引 macOS。~~Apple Developer / 证书 / 域名 / 商标~~ **已取消（2026-07-04 开源免费拍板**，分发改 GitHub Releases 开源口径，[ROADMAP §5](./ROADMAP.md)）。
- **Class B（产品决策，不阻塞 §6 出场线）**：**已全部清零**——7-04 X 四项拍板 + 7-06 clarify options 结构口径（方案 A：按 reason 定带不带 options、非 Unknown 一律挂、parser/标注双向对齐，[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）均落地。

## 会话日志

> 摘要 ≤5 条；全文与更早历史：[STATUS-archive-2026-07.md](docs/session-logs/STATUS-archive-2026-07.md) → [STATUS-archive-2026-06.md](docs/session-logs/STATUS-archive-2026-06.md) → [STATUS-archive-through-2026-06-03.md](docs/session-logs/STATUS-archive-through-2026-06-03.md)。

### 2026-07-06 — Claude Code (Opus 4.8 / Fable 5) — BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割至 99.4%

**承接**：用户问「本次会话该做什么」→ 读三份共享文档 + 定向读 ROADMAP §2/§6.3/§8 → 判定质量线已达标、卡口全在真机/对外；用户选「先看 ROADMAP 全局再定」→ 按建议做出场报告骨架 + clarify 分析 → 拍板方案 A → 就地实现 → 续推老账收割。
**产出**：① [beta-exit.md](docs/reviews/beta-exit.md) 骨架（§9 模板，parser-only 全填、真机格标 TODO，B→V checklist 必交付项）；② clarify options **方案 A** 拍板并落地（[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）——按 reason 定带不带 options、非 Unknown 一律挂、parser（`clarify_with`+`standard_options`）与标注（d6/d8 共 17 条）双向对齐，Class B 清零；③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符保全 / 裸 no+字面扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。
**结果**：**v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 230→235 测 + evals/harness/server 全 gate（28 suite 0 failed）+ clippy `-D warnings`/fmt 净。剩 6 partial 全为 v0.5 标注锁定项 + 备份文件两难，parser 收割见底。
**未尽事宜**：真机复跑填 beta-exit TODO 格；clarify en query 返中文 options 是既有 i18n 缺口（独立小卡）。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

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
