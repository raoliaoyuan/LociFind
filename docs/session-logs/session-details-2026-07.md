# LociFind 会话详录 — 2026-07

> STATUS.md 只放摘要；本文件按月留改动概览、验证输出、决策细节。最新在顶部。

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
