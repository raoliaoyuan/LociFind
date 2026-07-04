# Parser v0.3 evals 报告

> 攻 Class B 50.5% 的实施落地。6 个 commit / 半天主会话独占。
> 计划文档：[docs/superpowers/plans/2026-05-26-parser-v0.3.md](../superpowers/plans/2026-05-26-parser-v0.3.md)。

## 数字（500 条 v0.5 fixture）

| 指标 | v0.2.1 baseline | v0.3 | Δ |
|---|---:|---:|---:|
| variant 命中率 | 69.2% | **85.4%** | **+16.2 pp** |
| 字段精确匹配率 | 16.6% | **47.4%** | **+30.8 pp** |
| pass | 83 | **237** | **+154** |
| partial | 263 | 190 | -73 |
| fail | 154 | 73 | -81 |

## 按 variant

| variant | baseline pass | v0.3 pass | partial | fail |
|---|---:|---:|---:|---:|
| Clarify | 14 | 14 | 19 | 7 |
| FileAction | 3 | **46** | 30 | 4 |
| FileSearch | 17 | **101** | 98 | 1 |
| MediaSearch | 2 | 2 | 43 | 55 |
| Refine | 47 | **74** | 0 | 6 |

## 按 language

| language | pass | partial | fail |
|---|---:|---:|---:|
| zh | 135 | 81 | 34 |
| en | 69 | 63 | 18 |
| mixed | 33 | 46 | 21 |

baseline 时三个 language 桶 pass 是 47 / 13 / 23，本轮分别 +88 / +56 / +10。

## 本轮覆盖

| Task | 主要修复 | 单次 evals 涨幅（pass） | commit |
|---|---|---:|---|
| 1 | language 检测加 ASCII 中性词白名单 | +49 (83→132) | a7d8ceb |
| 2 | file_action target_ref regex 化 + 英文 verb 松散匹配 | +43 (132→175) | e31ae23 |
| 3 | refine 加 time delta / 英文 only-in / clear / limit-to / exclude videos | +17 (175→192) | e5a2e83 |
| 4 | location hint 按输入语言保留（zh→中文 / en→英文） | +14 (192→206) | 040844c |
| 5 | file_search keywords 排除 size-shaped token 与 size 触发词 | +31 (206→237) | 6b6c1c9 |
| Task 6 | （跳过）实测数据只能修 1 case，杠杆不值 | — | — |
| 7 | 全量 ci + 本报告 + STATUS/ROADMAP 同步 | — | (本 commit) |

## 关键设计决策（与 plan 一致落地）

1. **Location hint 按输入语言保留**：`LocationAlias` 拆 `zh_hint` / `en_hint`，新增 `parse_location_with_language(lower, language)` 按 language 选择。fixture 显示英文 query 期望英文 hint，中文期望中文。
2. **file_action 英文 verb 松散匹配**：`copy`/`move`/`open` 单独 `word_present` 即触发，由 `extract_target_ref` 提取失败时 None 保护避免误路由到 file_search query。
3. **scrub_neutral_tokens**：language 检测前把 "ppt"/"Excel"/"MB"/"GB" 等 ASCII 中性词替换为空再判定 zh/en/mixed。词表严格控制；新增词须先过 evals 验证不会把真实 mixed 误判为 zh。
4. **size-shaped token 排除**：parse_size 抽走的 "100mb"/"1gb" 不再回流 keywords。同时 excluded 列表加 `larger/smaller/bigger/greater/than/over`。

## 留给后续 task 的剩余缺口

### Clarify 文案精确匹配（19 partial）

parser 用中文友好文案（"删除操作会移到回收站..."），fixture expected 是英文/简化版（"Delete is not supported in MVP. Show files instead?"）。两者均合理。

**建议**：单独 PR 把 `packages/evals/src/lib.rs` 的 `compare_json` 对 Clarify variant 加宽 —— `reason` 严格比较 + `question`/`options` 弱匹配（长度一致 + reason 等价类）。否则 parser 文案对齐 fixture 会损失中文用户友好度。

### MediaSearch 55 fail（最大未处理桶）

本 plan 只主攻 FileSearch / FileAction / Refine。MediaSearch 系统性问题（artist 识别 / media_type vs file_type 路由 / quality 等）规模较大，建议作为 parser v0.4 单独主题。

### Class D fallback 触发（v0.5 fail 中 73 条里大部分）

parser 输出结构合法但字段不全（如 fixture 凭"预算"推断 xls/spreadsheet 这类隐式语义）。**parser 不应承担这类隐式推断**；建议作为 MVP-17 模型 fallback 端到端 evals 子集量化模型实际能救多少。

### lib.rs 拆分

文件已 ~1500 行（v0.3 加了 ~150 行测试 + 50 行实现）。建议 parser v0.4 拆 `parsers/file_search.rs` / `file_action.rs` / `refine.rs` / `media_search.rs` / `clarify.rs` 子模块。

## 复跑命令

```bash
bash scripts/ci.sh
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 2>&1 | head -25
```
