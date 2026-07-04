# BETA-13-G15（C1）设计：`documents`/`pictures` 类型义 vs 位置义上下文消歧

> 作者：Claude Code (Opus 4.8)，2026-06-20
> 前置：[re-baseline 决策清单 §C1](../../reviews/beta-13-rebaseline-decisions.md)、
> STATUS「当前 Task」G15 设计要点、ROADMAP BETA-13-G15 卡片。
> 输入基线：v0.9 parser-only = **863 pass / 4 fail（§6 = 86.3%）**（G14 收口）。

## 1. 问题

parser 把英文 `documents` / `pictures` **一律当位置词**（二者在 `LOCATION_ALIASES`，
lexicon.rs:219/225）。但 v0.9 评测集里有一批 case 的 `documents`/`pictures` 是
**类型/复数名词**（句首、并列枚举、内容子句、尾置名词），期望 `location=None` +
`file_type=document/image`，parser 却误产出 `location={hint:documents}`。

这是 §6 跨 90% 出场线的**最大单块**，但因 `documents` 同一个词有两种义、且 v0.5
有把它当位置的锁定基线锚点，被列为「需从干净基线单独立项」。

### 受影响的 13 条 C1 case（v0.9）

| id | query | 期望关键字段 |
|---|---|---|
| d2-en-005 | `word or powerpoint documents` | ext=[ppt,pptx,doc,docx], ft=[document,presentation] |
| d2-en-007 | `show me documents and images` | ft=[document,image] |
| d2-en-010 | `code files and documents` | ft=[code,document] |
| d2-en-011 | `music, videos and pictures` | ft=[audio,video,image] |
| d2-en-013 | `documents, spreadsheets and presentations` | ft=[document,spreadsheet,presentation] |
| d2-en-016 | `png and jpg pictures` | ext=[png,jpg]（无 file_type）|
| d2-en-019 | `documents and images, excluding archives` | ft=[document,image], exclude=[archive] |
| d3-en-001 | `documents that mention quarterly revenue` | ft=document, kw=[quarterly revenue] |
| d3-en-008 | `documents that talk about data privacy` | ft=document, kw=[data privacy] |
| d3-en-017 | `documents whose content includes annual budget` | ft=document, kw=[annual budget] |
| d3-en-021 | `find documents that mention both onboarding and offboarding` | ft=document, kw=[onboarding, offboarding] |
| d5-en-001 | `documents modified today` | ft=document, modified_time=today |
| d5-mixed-009 | `我昨天 opened 的 documents` | ft=document, accessed_time=yesterday |

## 2. 硬约束（byte-equal）

已全量核对 v0.5 500 条锁定基线：

- 英文 `documents` 作**位置义**的锚点 **全部带显式标记**——`in documents`（如
  `find Excel modified this week in documents`）或 `Documents 里`（如
  `find Documents 里的 >100MB ppt`）。**裸 `documents` 作位置：0 条。**
- `pictures` 作位置义的锚点：**0 条**（v0.5 完全无 `pictures` 出现）。
- 中文 `文稿` / `文档目录` / `图片目录` / `图片文件夹` 作位置：**词形与英文
  `documents`/`pictures` 不同**，是 `LOCATION_ALIASES` 的独立关键词，**本设计不动**。
- `move … to documents`（9 条 file_action，hint 本就 None）：`to` 不是位置标记，
  且走 file_action parser，不受影响。

结论：只要新逻辑**仅在英文 `documents`/`pictures` 缺少 `in`/`里` 标记时改变行为**，
v0.5 全部锚点（带 `in`/`里`）逐字节不变。

## 3. 设计：单一消歧谓词 + 两处落地

### 3.1 消歧谓词（严格，仅认 v0.5 实证形态）

```text
documents_pictures_is_location(lower, k) →
    k ∈ {"documents","pictures"} 时：
        lower 含 "in <k>"  OR  <k> 后紧跟（可含空格）"里"
    其余 k（中文 alias）：恒为 true（不受门控）
```

「严格」= 只认 `in documents` / `documents里`（含 `documents 里`）两种形态，**不**扩到
`documents folder`/`documents directory`/`in the documents`（评测集 0 收益、扩大破坏面）。

### 3.2 落地 (a)：抑制误判位置 —— `parse_location_with_language`（common.rs:17）

file_search 与 media_search 共享此函数。迭代 alias 命中时，对 ASCII 歧义关键词
`documents`/`pictures` 追加门控：谓词为假则**跳过该命中**（不产出 location），
继续找下一个 alias。中文关键词与其他 alias 路径不变。

media 路径自动受益：v0.5 media 锚点（`find videos … in documents`）带 `in` →
谓词真 → location 保留；评测集无 media 的 documents-类型义 case。

### 3.3 落地 (b)：补类型义 file_type —— 仅 file_search parser

- **`pictures`/`images` 无需注入**：二者已在 `EXTENSION_ALIASES`（lexicon.rs:146→Image），
  本就产出 `file_type=image`。(a) 抑制 location 后即对齐。
- **`documents` 需注入**：`documents` 不在 `EXTENSION_ALIASES`（历史移除，注释 line 183），
  且**不能简单加回**——否则 v0.5 的 `in documents` 锚点会平白多出 `file_type=document`，
  破 byte-equal。故采用**上下文门控注入**：在 `parse_file_search` 取得
  `merge_extensions` 结果后，若 `documents` 为类型义（谓词为假），把 `FileType::Document`
  **按 query 语序**插入 file_type 列表（去重），位置由 `documents` 在 query 中的偏移决定，
  复用既有「多 file_type 按出现位置排序」约定（满足 `code files and documents`→
  `[code,document]`、`documents and images`→`[document,image]`）。
- **keywords 已自动正确**：`en_is_type_or_location_word`（file_search.rs:1467）已把
  `documents` 当类型/位置词从英文关键词抽取中剥离，故 `documents that mention X`→
  `keywords=[X]`（不含 documents）无需改动。

## 4. 诚实的 pass 预估

C1 改动能**干净翻 pass 约 10-11 条**：d2-en-007/010/011/013/019、d3-en-001/008/017/021、
d5-en-001、d5-mixed-009（实际数以 TDD 逐条验证为准）。

**2 条仍 partial，且根因非 C1**：

- **d2-en-005** `word or powerpoint documents`：期望同时有 `extensions` 和 2 个
  `file_type`，但 G14 决策 B 的 `merge_extensions` 规则是「≥2 file_type → ext=None」。
  这是多类型 ext 约定冲突，属 C2/coverage 范畴，本任务不解。
- **d2-en-016** `png and jpg pictures`：期望「有 ext 无 file_type」，parser 单范畴会给
  `file_type=[image]`。同属多类型/单范畴 ext 约定，非 C1。

这两条留在 partial、登记为 C1 的近邻 follow-up（需用户对多类型 ext 约定再拍板），
**不在本任务用 coverage 凑指标**（守 G10 立的纪律）。

## 5. 测试策略

- **TDD**：每个落地点先写失败单测再实现。(a) 谓词单测覆盖 `in documents`（→location 保留）/
  `documents里`（保留）/ 裸 `documents and images`（→None）/ `pictures`（→None）。
  (b) 注入单测覆盖句首 / 并列 / 内容子句 / 尾置 / 多类型语序。
- **v0.5 byte-equal 闸门**：每刀后用规范化逐 case 比对（reporter JSON 非确定，按 id 排序 +
  剔 elapsed_ms，脚本沿用 `/tmp/v05check.py`）。**v05-* 子集 0 变化**为放行硬条件。
- **v0.9 回归**：跑全量 v0.9，确认 pass 只增不减、partial/fail 无意外回退。
- **workspace**：`cargo test --workspace` + `clippy --all-targets -D warnings` + `cargo fmt --check`。

## 6. 范围边界（不做）

- 不动中文 `文稿`/`文档目录`/`图片目录`/`图片文件夹` 位置义（无歧义，保持现状）。
- 不解 d2-en-005/016 的多类型 ext 约定冲突（C2/coverage 范畴，需用户决策）。
- 不改任何 coverage / v0.5 fixture（纯 parser 任务）。
- 不扩位置标记到 `folder`/`directory`/`the` 等非实证形态。
