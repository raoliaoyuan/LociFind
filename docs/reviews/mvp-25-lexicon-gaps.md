# MVP-25 v0.5 evals lexicon 缺口分析

本轮只扩展评测集并归因失败，不修改 `packages/intent-parser/`。

## 评测概览

命令：

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
```

核心结果：

| 指标 | 结果 |
|---|---:|
| 用例总数 | 500 |
| variant 命中率 | 346 / 500 = 69.2% |
| 字段级精确匹配率 | 82 / 500 = 16.4% |
| pass / partial / fail | 82 / 264 / 154 |

按 variant：

| variant | pass | partial | fail |
|---|---:|---:|---:|
| FileSearch | 16 | 183 | 1 |
| MediaSearch | 2 | 43 | 55 |
| FileAction | 3 | 1 | 76 |
| Refine | 47 | 18 | 15 |
| Clarify | 14 | 19 | 7 |

按语言：

| language | pass | partial | fail |
|---|---:|---:|---:|
| zh | 47 | 138 | 65 |
| en | 13 | 81 | 56 |
| mixed | 22 | 45 | 33 |

## 失败分类

统计对象是 `partial + fail = 418` 条未完全命中的 case。归因规则：

- Class A：用户 Class 1 同义词扩展命中，但 parser 词典未覆盖或未形成目标字段。
- Class B：parser 识别了相关信号，但字段归一化、字段归属或 variant 路由错误。
- Class C：clarify 语义边界错误，包含该触发未触发、问题文案/options 不一致、或不该进入 clarify 的边界。
- Class D：parser 产出结构化 intent 但关键字段为空，属于 MVP-17 signals/model fallback 应接管的结构性遗漏。

| Class | 数量 | 占未完全命中 | 主要现象 |
|---|---:|---:|---|
| A lexicon 缺口 | 28 | 6.7% | `最大的` / `一周内` / `几个 G` 等词未稳定映射 |
| B 字段映射错误 | 211 | 50.5% | zh 误判 mixed、location hint 规范化不一致、file_action/refine 路由弱 |
| C clarify 边界 | 26 | 6.2% | `find recent` 未 clarify；英文 clarify 文案/options 未本地化到 expected |
| D fallback 应触发 | 153 | 36.6% | signals 可见但 time/size/location/media 字段为空 |

## Class A 同义词清单

时间：

- `一周内` → `modified_time.relative = last_7_days`
- `近一周` → `modified_time.relative = last_7_days`
- `本周` / `this week` → `modified_time.relative = this_week`
- `past 7 days` → `modified_time.relative = last_7_days`

大小：

- `几个 G` → `size.greater_than >= 1 GB`
- `X MB 以上` → `size.greater_than = X MB`
- `>100MB` → `size.greater_than = 100 MB`
- `大文件` → 默认 `size.greater_than = 100 MB`

排序：

- `最大的` / `最大` / `最重` / `体积最大` → `sort = size_desc`
- `biggest` / `largest` → `sort = size_desc`

位置：

- `downloads` 在 expected 中有时保持英文 hint，有时 parser 归一到 `下载`；parser v0.2 需要统一 location hint 规范。
- `synthetic-place` / `项目归档` 这类未知位置应进入明确的 `ambiguous_location` 边界，而不是混入普通 file search。

媒体：

- `视频` / `video` 在“最大的视频”“本周修改的视频”等组合中应优先路由到 `media_search.media_type = video`。
- `screenshots from yesterday` 应保留 `media_type = screenshot`，并识别 `created_time`，避免把 `from/last/month/JPG/PNG` 当关键词。

## Class B 映射规则修正建议

- 语言检测：中文查询中含 `ppt` / `Excel` / `word` / `100MB` 不应直接判为 `mixed`；建议要求中英两侧都有非文件类型/单位的实义词后才输出 `mixed`。
- location hint：统一 `downloads/desktop/documents` 与 `下载/桌面/文稿` 的 expected 规范。建议 SearchIntent 层使用稳定枚举或 canonical hint，UI 再本地化。
- size 解析：`larger than 1 GB`、`X MB 以上`、`>100MB` 应统一走 `SizeExpression::GreaterThan`，不要把 `larger/100mb/1gb` 抽成 keyword。
- sort 解析：size 最高级词应映射到 `sort = size_desc`，和 `size.greater_than` 分开处理。
- media 路由：包含 `video/screenshot/song/audio/歌/视频/截图` 且不是普通扩展名筛选时，优先进入 `media_search`。
- file_action：`open/copy/move/rename/show ... result`、中英混合“第 N 个”需要稳定抽取 `target_ref`，不要落回 file search。
- refine：`clear location`、`只看 last week 的`、`排除 video` 等应保持 `refine.delta/clear`，不要按普通搜索重解。
- clarify 文案：评测对 `question/options` 是精确匹配；parser v0.2 要么固定标准文案，要么 evals 后续把 clarify 文案改成语义级匹配。

## 后续建议

- parser v0.2 第一批先补 Class A 词典和 Class B 中的语言/location/size/sort 映射，预计能显著提升字段精确匹配率。
- MVP-17 fallback 已有 signals 机制，Class D 不建议在 evals 本轮修；下一批可用 `resolve_intent` 路径增加一组独立评测，验证模型 fallback 是否覆盖结构性遗漏。
- Clarify 建议拆成两层评测：reason 精确匹配、question/options 文案弱匹配，避免文案差异掩盖触发边界。
