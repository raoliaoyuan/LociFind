# lora-aug-keywords v1

BETA-24 keywords 补全训练数据 fixture。**v0.9 全程不进训练**（评测集污染红线）。

## 生成

```bash
cargo run --release -p locifind-evals --bin fixtures -- generate-lora-aug-keywords
```

- 种子 = 程序模板（`fixtures.rs::template_seeds`）+ `_authoring/hw-*.json` 手写分片（AugSeed：`{id, query, missing_keywords}`）。
- `expected_intent = parser draft ⊕ 补齐 keywords`（其余字段继承 draft；keywords 取并集，draft 词在前——与 `hybrid.rs::apply_patch` 并集语义同序）。
- 不触发 keywords 待填的种子在生成期**丢弃并逐条打印**（no silent caps）。
- `cases.json` = 训练份；`heldout-cases.json` = 验收量化份（id 升序每 5 取 1，**永不进训练**）。
- 词表全合成，无真实文件名/路径/搜索词/人名/歌名。

## 数据规模（v1 首轮）

| 来源 | 种子 | 存活 cases |
|---|---|---|
| 程序模板（5 形态，两臂） | 63 | 63 |
| hw-zh（中文文件） | 30 | 10 |
| hw-en（英文文件） | 30 | 8 |
| hw-mixed（中英混合文件） | 30 | 4 |
| hw-colloquial（极口语，文件+媒体） | 30 | 17 |
| hw-media-zh（中文媒体） | 30 | 28 |
| hw-media-en（英文媒体） | 30 | 22 |
| **合计** | **243** | **152**（train 122 + heldout 30） |

## 关键发现：触发分布 = 推理分布（存活率为何如此）

媒体片存活率高（媒体 50/60），文件片存活率低（文件自然片 22/90）。根因经探针定位，是 parser 的「内容词遗漏」**只在两种形态发生**：

1. **文件搜索：仅「{内容词}文件名包含{X}」逐字形态**触发。parser 对「文件名包含」做短路、干净丢前置内容名词短语；但口语变体「文件名里带 / 名称里含 / 名字里有 / 文件名带有 / 文件名包括」**不短路**——parser 把「文件名/名称/名字」粘进内容词（如 `体检报告文件名里带复查` → keywords=`["体检报告文件名","带复查"]`），「体检报告」作为子串被判已覆盖 → 不遗漏。
2. **媒体搜索：主题/情境词**（毕业旅行 / late night study）触发。parser 抽 artist/album/title/genre 后，对描述歌曲主题的词无字段可放 → 遗漏。

**这不是撰写失误，而是真实约束**：被丢弃的 query 恰恰是**推理期也不会触发模型 fallback** 的（parser 不报 keywords 遗漏 → 走 parser-only）。训练分布因此正确匹配推理分布——模型只需学会补全它实际会被要求补的两类（文件名包含短路的前置内容词 / 媒体主题词），无需学习 parser 已能正确处理的口语变体。

## 迭代

v1 首轮 152 条用于 BETA-24 重训。若 Task 9 held-out 验收 <80%，按计划 §6.4 迭代协议补样本（文件侧用「文件名包含X」逐字形态 + 更多合成内容词/口语包装；媒体侧扩主题词）再重训。
