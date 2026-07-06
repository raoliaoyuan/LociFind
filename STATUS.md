# LociFind 项目状态

> **每次会话开始**：必读本文件 + [PROJECT.md](./PROJECT.md) + [CONVENTIONS.md](./CONVENTIONS.md)；[ROADMAP.md](./ROADMAP.md) 按 [CONVENTIONS §2](./CONVENTIONS.md) 定向读取。  
> **每次"收工"**：按 [CONVENTIONS §3](./CONVENTIONS.md) 维护本文件固定骨架（速览 / 当前 Task / 下一步 / 阻塞 / 会话日志），体积守软 10-12KB / 硬 15KB。  
> 会话日志只放**摘要**（≤5 条）；详录在 [docs/session-logs/](./docs/session-logs/) 与 `docs/reviews/`。task 详情在 ROADMAP，此处只用 task ID 引用。

## 📍 速览

- **阶段**：B（Beta）进行中；P ✅ / M 代码层 ✅ / M→B 正式切换仍待 §8 长周期项；**§6「总体 evals >90%」本机 parser-only 已达 99.4%（v0.9 994/6/0、fail=0）**，出场判定余双平台真机复跑。
- **定位**：**开源免费**（2026-07-04 拍板，MIT OR Apache-2.0 双许可）本地语义检索底座——个人桌面搜索 + 企业冷归档检索（律所卷宗 / 内部审计 / 离职归档三场景）；**不做分析层**，分析经 MCP daemon + 外部 LLM 组合。以 [PROJECT.md](./PROJECT.md) 为准。
- **当前 task**：**clarify i18n 双语化 + macOS DMG CI done**——clarify options/question 按 language 出中/英文（闭合 i18n 缺口，eval-neutral、238 测）；[release-macos.yml](.github/workflows/release-macos.yml) 镜像 windows 版（BETA-10 DMG 产物 CI done，本机仅 YAML 校验、待 macOS 发版首验）。前一轮：beta-exit 骨架 + clarify 方案 A + 老账收割至 99.4%。
- **下一步 top-3**：① 设计伙伴/首个真实部署主动获取（护城河 P0，开源免费降低试用门槛）；② 双平台真机复跑填 [beta-exit.md](docs/reviews/beta-exit.md) 的 TODO(真机) 格 + macOS DMG CI 首验（下次 macOS 发版）；③ BETA-33 cycle 9 + v0.9.14 装机真机验证（六场景 + BETA-10A + Scoop，随下次上机）。
- **阻塞**：Class A 仅剩**双平台 evals 真机**（Apple Developer / 证书·域名·商标已随 2026-07-04 开源免费拍板取消）；**Class B 归零**（clarify options 结构口径 2026-07-06 方案 A 拍板落地）。

## 当前 Task

**2026-07-06（最新）**：**BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割**。① 起草 [beta-exit.md](docs/reviews/beta-exit.md)（§9 模板，parser-only 数据全填、真机相关格标 `TODO(真机)`，B→V checklist 必交付项）。② clarify options 结构口径（Class B 唯一剩余）拍板**方案 A**并就地落地（[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）：evals 只校验 options 结构存在性（Array vs null），故按 reason 定「带不带」——非 Unknown 一律挂标准 options，parser（`clarify_with` + `standard_options`）与标注（d6/d8 共 17 条）双向对齐；v0.9 8 条 clarify partial 全清、Clarify 桶 67/0/0。③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符 / 裸 no+扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。**结果：v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 235 测 + evals/harness/server 全 gate + clippy/fmt 净。详录 [session-details-2026-07](docs/session-logs/session-details-2026-07.md)。

## 下一步

1. **设计伙伴 / 首个真实部署获取**（护城河 P0，ROADMAP §5）：BETA-40 真实内网证据、BETA-44 真实语料扩充、场景词表积累均以此为前提——主动获取（律所/审计/离职归档任一场景即可）。
2. **BETA-33 cycle 9 真机验证**：随下次发版装机，按 [manual-test-scenarios](docs/manual-test-scenarios.md) 跑六场景；本轮验证面另含 BETA-43（出处/`read_document`/审计导出，[playbooks README](docs/playbooks/README.md) 第 8/9 条）+ **BETA-12 卸载清理**（场景 5「升级零数据损失」为发版阻断；NSIS hook 首次真实构建即本次发版 CI）+ **BETA-29 意图草稿 v1（6 场景）+ v2（7 场景）**。
3. **v0.9.14 发版 done**：CI 成功（run 28708792924，NSIS hook 首次真实构建通过）+ [Release](https://github.com/raoliaoyuan/LociFind/releases/tag/v0.9.14) changelog 补全（prerelease）+ Scoop bucket [scoop-locifind](https://github.com/raoliaoyuan/scoop-locifind) 上线（`scoop bucket add locifind <url> && scoop install locifind`）。装机包供下一步 ② 真机验证。
4. **BETA-10 剩余**：macOS DMG 产物 CI **done**（[release-macos.yml](.github/workflows/release-macos.yml)，待下次 macOS 发版首验）；剩真机放行验证（§6.3）；winget 待 BETA-14 后 / Homebrew tap 待 DMG CI 真机跑通。
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

### 2026-07-06（续）— Claude Code (Opus 4.8) — clarify i18n 双语化 + macOS DMG CI

**承接**：上一 commit 后用户复问「本次会话该做什么」→ 重评纯代码只剩两项 → 用户选「A+B 都做」。
**B（i18n，本机验证）**：clarify options/question 按 language 双语——`pick`/`standard_options(reason, language)`，顶层 4 类就地 `bilingual_options`、vague 5 类走 `pick`；mixed 归中文。eval-neutral（evals 不校验 clarify 文案/options，v0.9 994、v0.5 495 不变）；intent-parser 235→238（+3 i18n 测）、clippy/fmt 净。闭合 beta-exit §6 记的既有缺口。
**A（DMG CI，仅 YAML 校验）**：[release-macos.yml](.github/workflows/release-macos.yml) 镜像 windows 版（macos-14 + aarch64 + 同款守门/features + Gatekeeper releaseBody）。可编依据=daemon workflow 已在 macos-14 编 llama。**风险**：本机无 macOS runner，下次 macOS 发版首验；windows+macos 并行同吃 v* tag、往同一 Release 幂等追加（已注释写明）。
**未尽事宜**：A 待 macOS 发版真机首验（BETA-10 剩真机放行验证）。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。

### 2026-07-06 — Claude Code (Opus 4.8 / Fable 5) — BETA-14 出场报告骨架 + clarify options 方案 A + 老账收割至 99.4%

**承接**：用户问「本次会话该做什么」→ 读三份共享文档 + 定向读 ROADMAP §2/§6.3/§8 → 判定质量线已达标、卡口全在真机/对外；用户选「先看 ROADMAP 全局再定」→ 按建议做出场报告骨架 + clarify 分析 → 拍板方案 A → 就地实现 → 续推老账收割。
**产出**：① [beta-exit.md](docs/reviews/beta-exit.md) 骨架（§9 模板，parser-only 全填、真机格标 TODO，B→V checklist 必交付项）；② clarify options **方案 A** 拍板并落地（[决策备忘](docs/reviews/beta-14-clarify-options-decision-2026-07-06.md)）——按 reason 定带不带 options、非 Unknown 一律挂、parser（`clarify_with`+`standard_options`）与标注（d6/d8 共 17 条）双向对齐，Class B 清零；③ 老账收割 9 条（songs by 小写连字符 artist ×4 / 碳中和 compound 占位符保全 / 裸 no+字面扩展名窄路径 / music 目录 mixed hint / 几个G→size_desc / d3 ft 对齐 ×2）。
**结果**：**v0.9 977/23/0→994/6/0（99.4%）、v0.5 490/10/0→495/5/0**，逐 case 零回归；intent-parser 230→235 测 + evals/harness/server 全 gate（28 suite 0 failed）+ clippy `-D warnings`/fmt 净。剩 6 partial 全为 v0.5 标注锁定项 + 备份文件两难，parser 收割见底。
**未尽事宜**：真机复跑填 beta-exit TODO 格；clarify en query 返中文 options 是既有 i18n 缺口（独立小卡）。详录 → [session-details-2026-07.md](docs/session-logs/session-details-2026-07.md)。
