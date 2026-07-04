# BETA-13-G parser 干净缺口修复 + 标注冲突决策清单（设计）

> 日期：2026-06-19 · 工具：Claude Code (Opus 4.8) · 阶段：B（Beta）
> 关联：[STATUS.md](../../../STATUS.md) §下一步「BETA-13-G* parser 缺口」、[ROADMAP.md](../../../ROADMAP.md) BETA-13/BETA-14
> 前序：BETA-13-G1~G7（2026-06-04 done，§6 总体 evals 51.4%→72.6%）

## 1. 背景与问题

当前 v0.9 评测集 parser-only 通过率 **726/1000（72.6%）**，分布：

| 子集 | pass | partial | fail |
|---|---|---|---|
| v0.5（500，回归锚点） | 473 | 25 | 2 |
| coverage（500，手标 ground-truth） | 253 | 200 | 47 |

机会全在 coverage 子集的 247 个失败里。本会话调研把这 247 个按「能否在 parser 修」分三类：

- **真能在 parser 修（干净、无 byte-equal 风险）**：内容子句路由（~17 fail + 一批 keywords partial）、artist 自然措辞（~20 partial）、中文类型词→file_type（file_type 73 partial 的安全子集）。
- **parser 无解（v0.5↔coverage 标注契约冲突）**：video/截图 + size/time——v0.5 有 50+ 条 `大于100MB的视频`→media_search 锚点，coverage 却要 file_search。同一形态两套标注矛盾，任何路由规则都不能两全，改则破 byte-equal。
- **标注 ground-truth 本身问题**：d9 多类型组标注自相矛盾（`pdf 和 doc`→null vs `pdf 和图片`→array）、location vs keyword 边界、`备份文件` 切分等。

§6 的「总体 evals >90%」靠纯 parser **达不到**——一大块是标注契约冲突。本设计的目标不是凑 90%，而是：① 吃掉真正干净的 parser 缺口；② 把无法用 parser 解决的冲突整理成决策清单，交用户后续拍板。

## 2. 目标与非目标

**目标**
1. 修三块干净 parser 缺口（下文 Fix 1/2/3），全程 byte-equal 守护（v0.5 恒 473 pass）。
2. 产出一份标注冲突决策清单文档（仅产出，不改评测集 ground-truth）。

**非目标（明确不做）**
- 不碰 v0.5↔coverage 契约冲突的路由（会破 byte-equal，须用户先定标注规范）。
- 不改任何评测集 ground-truth（coverage-cases.json / v0.5 cases.json 一字不动）。
- 不做孤立 sort/refine 边界（与 refine 语义纠缠，YAGNI 排除）。
- 不追求 90% 总体指标。

## 3. 约束

- **byte-equal 铁律**：v0.5 = 473 pass、v0.9 = 726→更高，但 v0.5 段逐字节不变。每个 fix 完成即重跑 v0.5 验证。机制纪律：**每条规则只对 v0.5 不存在的新形态生效**。
- 不碰评测集 fixture 文件（受 `tests/v09_integrity.rs` 三条不变量保护）。
- Rust：`cargo fmt --check` + `cargo clippy --deny warnings` + `cargo test --workspace` 全绿。
- 改动集中在 `packages/intent-parser/src`，不外溢。

## 4. 设计

### 4.1 路由现状（落点）

`lib.rs::parse()` 分派顺序：clarify → vague-clarify → file_action → refine → **`is_media_query(lower)` → parse_media_search** → 否则 file_search。

`is_media_query`（[media_search.rs:13](../../../packages/intent-parser/src/parsers/media_search.rs)）当前逻辑：
1. 跨范畴媒体连词（`音乐和视频`）且无 artist → false（交 file_search）。
2. `has_strong_media_signal`（含 `截图/screenshot` 等强词）或 `contains_known_artist` → true。
3. 音频 metadata 信号 → true。
4. 视觉媒体 + 抽象修饰 → true。

截图含强词 `截图`，故任何截图 query（含内容截图）当前命中第 2 步 → media。

### 4.2 Fix 1 — 内容子句路由 + 干净关键词抽取

**问题**：`截图里写着已支付的` 当前 → MediaSearch(media_type=screenshot, keywords=粗抽)；coverage 期望 FileSearch(file_type=screenshot, keywords=[已支付])。更广地，「内容子句」是一个跨截图/文档的「按内容搜」信号：
- `截图里写着已支付的` → file_search, file_type=screenshot, keywords=[已支付]
- `内容里提到季度营收的文档` → file_search, file_type=document, keywords=[季度营收]

**机制**：
1. 新增内容子句检测纯函数 `detect_content_clause(input) -> Option<String>`，识别形态并抽取**干净内容短语**：
   - 中文：`里写着X / 里写了X / 写着X / 写了X / 里提到X / 提到X / 内容…X / 正文…X / 里面有X / 里面提到X`（X 为子句核心名词短语，剥掉尾部 `的`/`的那张`/`的报错` 等容器/指示尾巴）。
   - 英文：`says X / that says X / mention(s) X / that mention(s) X / shows X / with X`。
2. 在 `is_media_query` 的 strong-media 判定**之前**加闸门：若 `detect_content_clause` 命中 → 返回 false（交 file_search）。
3. file_search 侧：截图词→`file_type=screenshot`、文档词（文档/PDF/报告/合同/协议/邮件…）→对应 file_type 或 null；内容短语→`keywords`。复用 file_search 已有的类型词映射（[file_search.rs](../../../packages/intent-parser/src/parsers/file_search.rs) 已含 screenshot 映射），新增内容子句关键词注入。
4. 退役/绕过 `extract_screenshot_keywords` 的粗暴 stop-word 删除路径——内容截图改走 file_search 后由内容子句抽取器产干净 keywords。仅「截图+time/size」无内容子句的 query 仍走 media（与 v0.5 一致）。

**byte-equal 安全**：v0.5 含 0 条「内容子句」锚点（已用 `里写着|写着|写了|says|mention|提到|出现|shows` 扫描确认）；触发词不出现在任何 v0.5 通过 case，故闸门对 v0.5 完全惰性。

**作用域决策（用户已确认）**：内容子句做成跨截图/文档的统一抽取器，截图分支修 fail，文档分支顺带修一批 keywords partial。

### 4.3 Fix 2 — artist 自然措辞抽取

**问题**：`周华健的歌`、`找邓紫棋的歌曲` 当前已路由进 media（`has_free_artist_structure` 命中）但 artist 值抽不出（partial）。

**机制**：扩展 artist 值抽取，覆盖：
- 中文：`X的歌 / X的歌曲 / X唱的 / X的音乐视频`（X = 2–4 字 CJK）。
- 英文：`songs by X / tracks by X / music videos by X`（X = 名字 token）。
- `周杰伦的音乐视频` → artist=周杰伦 **且 media_type=video**（artist 抽取兼容 video，不只 audio）。

**纪律**：只在现有 artist 抽取为 None 时填值；逐条对 v0.5 重跑，确认不改任何 v0.5 通过 case（v0.5 已有 artist 锚点如 `张学友` 系列须保持原值）。

### 4.4 Fix 3 — 中文类型词→file_type 干净子集

**问题**：file_type 73 条 partial 里，一部分是 parser 漏映射的非字面中文类型词（如 `表`→spreadsheet 单字形态）；另一部分是 d9 多类型 null 标注冲突（**不属本 fix**）。

**机制**：
1. 先用一次性脚本从 73 条 file_type partial 里筛出「intent=file_search、单 file_type、zh、parser 当前为 None」的干净集。
2. 逐条确认不与 d9 多类型 null 标注冲突。
3. 对干净集补 BETA-13-G3 遗漏的类型词 alias（纯增量映射）。

**byte-equal 安全**：新增 alias 是增量映射，对 v0.5 重跑确认无回归。

### 4.5 交付物 2 — 标注冲突决策清单文档

产出 `docs/reviews/beta-13-g-annotation-conflicts.md`，把 parser 无解的缺口整理成逐条决策清单：
1. **v0.5↔coverage 契约冲突**（video/截图 + size/time）：列每条冲突形态 + v0.5 现判 + coverage 现判，给三选项（保留 v0.5 / 改判 coverage / 分流）。
2. **d9 多类型标注自相矛盾**：列出矛盾对，建议统一规则。
3. **其余标注边界**：location-vs-keyword、`备份文件` 切分等。

**本会话只产清单，不动任何 ground-truth。** 应用须用户逐条拍板后另起任务（涉及 byte-equal re-baseline）。

## 5. 验证策略

每个 fix 独立完成后立即：
1. `cargo run -p locifind-evals --bin evals -- --fixtures v0.5` → 确认 **473 pass byte-equal**（核心闸门）。
2. `cargo run -p locifind-evals --bin evals -- --fixtures v0.9` → 确认 coverage pass 数上升、无新增 fail。
3. `cargo fmt --check` + `cargo clippy --deny warnings` + `cargo test --workspace` 全绿。

预计三块合计 +4~7pp（v0.9 726 → ~760-790）。最终数字以实测为准，如实报告。

## 6. 风险

- **artist/type-word 规则与 v0.5 共享**：抽取/映射规则可能误伤 v0.5 通过 case。缓解=每条规则改动后立即重跑 v0.5（473 闸门），发现回归即收紧触发条件。
- **内容子句抽取边界**：剥离容器尾巴（`的那张`/`的报错`）可能过/欠剥。缓解=对 coverage 内容截图全集逐条核对 keywords 精确匹配。
- **作用域蔓延**：内容子句统一抽取器可能改动文档路径的既有行为。缓解=v0.5 闸门 + 只在「无内容子句时行为不变」。

## 7. 实现单元（供 writing-plans 拆 task）

1. 内容子句检测纯函数 + 单测（TDD）。
2. `is_media_query` 内容子句闸门 + file_search 内容关键词注入。
3. artist 自然措辞值抽取扩展 + 单测。
4. 中文类型词干净子集筛选脚本 + alias 补全。
5. 标注冲突决策清单文档。
6. 回归门（v0.5 byte-equal + v0.9 + fmt/clippy/test）。
