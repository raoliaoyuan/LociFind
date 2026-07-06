# BETA-14 待拍板：clarify options 结构口径（Class B 唯一剩余项）

> 作者：Claude Code (Opus 4.8) ｜ 日期：2026-07-06
> 目的：把 v0.9 剩余 8 条 clarify partial 的根因、判定机制、口径选项讲清，供用户**一次拍板**清账。
> 关联：[gap-inventory §3.5](./beta-14-gap-inventory-2026-07-04.md#35-追记2026-07-04-ixx-会话两轮收割--四项口径拍板落地) / [beta-exit §3.4](./beta-exit.md)。
> **不阻塞 §6「>90%」出场线**（当前 977/23/0 已达标）；但清掉可让 clarify 桶口径自洽、省 §6.5 豁免额度。

## 1. 判定机制（关键：eval 只看结构，不看内容）

evals 比对 clarify 时（[`packages/evals/src/lib.rs`](../../packages/evals/src/lib.rs)）：

- `question`：**完全忽略**（`is_clarify_question_equal` 恒 `true`）——文案是本地化呈现。
- `reason`：严格比对（enum 已编码语义）。
- `options`：**只校验"结构存在性"**——`is_clarify_options_equal` 仅要求「都是 Array」或「都是 null」，**长度 / 内容 / 顺序全不校验**（parser 额外加「取消」等 UX 选项不惩罚）。

**推论**：8 条 partial 的唯一失配点 = **一边 `options` 是数组、另一边是 `null`**。内容分歧（给哪几个选项）从不进判定。所以这题本质是一个二元口径：**每类 clarify「带不带 options」，且 parser 与标注对同一 reason 必须给出相同结构判定。**

## 2. 两个失配簇（8 条）

parser 侧固定行为（[`lib.rs:82-136`](../../packages/intent-parser/src/lib.rs)）：4 类触发**带** options、`detect_vague_clarify` 一族（type/action/unknown）**不带**：

| parser 触发 | reason | options |
|---|---|---|
| `has_unsafe_delete_signal`（删除） | UnsafeAction | ✅ `["在访达/资源管理器中显示","取消"]` |
| `is_recent_only_query`（最近的） | AmbiguousTime | ✅ 时间枚举 |
| `is_ambiguous_bulk_action`（批量目标不明） | AmbiguousAction | ✅ `["确认全部","只选择部分","取消"]` |
| `is_unknown_location_only`（位置不明） | AmbiguousLocation | ✅ 位置枚举 |
| `detect_vague_clarify`（type/action/unknown/time） | 各 | ❌ 恒 `None`（[`clarify.rs`](../../packages/intent-parser/src/parsers/clarify.rs)） |

失配：

- **簇 A — d6 危险动作（4 条，parser 有 / 标注无）**：删除类 query（如「删除这些」）parser 给 UnsafeAction + 安全出口 options，**标注期望 `null`**。
- **簇 B — d8 模糊查询（4 条，标注有 / parser 无）**：如 [`d8.json`](../../packages/evals/fixtures/v0.9/_authoring/d8.json) 的 `那个东西在哪`（AmbiguousType 给类型 options `["文档","图片","视频","音乐"]`）、`处理一下这个`（AmbiguousAction 给动作 options `["打开","移动","删除"]`），但 parser 的 `detect_vague_clarify` 给 `None`。

**标注自身还内部不一致**：d8 里同为 ambiguous_type/action，仅 004/007 带 options，001/002/003/008 等不带——单靠改标注也得先统一 d8 内部口径。

## 3. 三个方案

### 方案 A（推荐）：按 reason 定"带不带 options"，parser 与标注双向对齐

确立一张**规范表**：有"可枚举的收窄维度"的 reason 一律带 options，唯 `Unknown` 无枚举项故不带。

| reason | 带 options | 呈现内容（eval 不校验，仅 UX） |
|---|---|---|
| UnsafeAction | ✅ | 安全出口：在访达/资源管理器显示、取消 |
| AmbiguousAction | ✅ | 动作枚举：打开、移动、删除、取消 |
| AmbiguousType | ✅ | 类型枚举：文档、图片、视频、音乐 |
| AmbiguousTime | ✅ | 时间枚举：今天、过去 3 天、过去一周、过去一个月 |
| AmbiguousLocation | ✅ | 位置枚举：全盘、下载、文稿、桌面、取消 |
| **Unknown** | ❌ | `null`（无可枚举收窄项，只能靠自由文本追问） |

- **落地——parser**（小改）：`detect_vague_clarify` 的 AmbiguousType / AmbiguousAction 分支补标准 options（现为 `None`）；Unknown 保持 `None`；AmbiguousTime/Location 分支若走此路径也补齐。
- **落地——标注**：d6 危险动作 4 条补 options 数组（对齐 parser UnsafeAction）；d8 把所有 AmbiguousType/Action/Time/Location 条目统一为**带** options、Unknown 条目统一为 `null`。
- **产品理由**：clarify 一旦锁定了"歧义维度"，就能给一排一键收窄选项（点一下即消歧），这正是 parser 现已对 6 类中 4 类做的事；只有全然 Unknown 无从枚举。口径自洽 + UX 最优。
- **成本**：parser ~1 处分支加 options + 标注 8 条对齐；**须逐 case 零回归验证**（重点查 v0.5 是否有 AmbiguousType/Action clarify 锚点期望 `null`——若有则该条改 parser 会引入 v0.5 回归，需同步对齐 v0.5 标注或收窄 parser 触发面）。预计 0.5d。

### 方案 B：clarify 一律不带 options

parser 4 类触发全改 `None`、标注全改 `null`。eval 上最省（全 null 对齐），但**丢掉一键收窄的 UX**（尤其危险动作的"安全出口"很有价值），与产品"可解释可控"原则相悖。不推荐。

### 方案 C：维持现状，8 条永久 partial 计入 §6.5 豁免

零改动，但留着口径不自洽，且吃 §6.5 豁免额度（累计上限 25 条、现已用 2）。仅在不想动 clarify 时的兜底。不推荐。

## 4. 推荐

**方案 A**：口径自洽 + UX 最优 + 清掉 8 条 partial（97.7%→~98.5%），成本 ~0.5d。实现时以逐 case 零回归为闸门，先验证 v0.5 clarify 锚点结构，再决定"改 parser / 改标注 / 两者"的精确分配。

> **拍板结论（2026-07-06，用户）：采纳方案 A**——按 reason 定"带不带 options"，除 `Unknown` 外均带，parser 与标注双向对齐。

## 5. 落地结果（2026-07-06，同会话就地实现）

**方案 A 已实现并通过全部零回归闸门。**

- **parser**（[`clarify.rs`](../../packages/intent-parser/src/parsers/clarify.rs)）：新增 `standard_options(reason)`，`clarify_with` 按 reason 自动挂标准 options（Unknown → `None`）。顶层 4 类直接构造的 clarify（unsafe/recent-time/bulk-action/unknown-location）已带 context-specific options，不动。
- **标注**（[`_authoring/d6.json`](../../packages/evals/fixtures/v0.9/_authoring/d6.json) / [`d8.json`](../../packages/evals/fixtures/v0.9/_authoring/d8.json)）：脚本批量给 **17 条**非 Unknown clarify 补 options（d6 危险动作 4 + d8 非 Unknown 13），Unknown 4 条保持 `null`；重跑 `assemble-coverage` + `generate-evals-v09` 重生成 fixture。
  > 注：13 条 d8 中除原 8 partial 里的少数外，多数是"当前靠 `null==null` 通过、parser 改后会翻 `Array` vs `null`"的连带项——两侧同步是零回归的必要条件，非多改。
- **验证**：
  - v0.9 **977/23/0（97.7%）→ 985/15/0（98.5%）**，Clarify 桶 **pass 67 / partial 0 / fail 0**（8 条 clarify partial 全清），fail 仍 0；剩 15 partial 全为非 clarify 老账/零星。
  - v0.5 **490/10/0 零回归**（Clarify 40/0/0）。
  - intent-parser 230 测 ✅、evals 全 gate ✅、harness 188 ✅、server 88 ✅、clippy `-D warnings` 净、fmt 净。

**本项 Class B 决策清账完毕**，STATUS「阻塞」Class B 归零。
