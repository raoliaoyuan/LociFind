# BETA-15E gazetteer 兼底多概念注入 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 gazetteer 兼底路径（parser 无 keyword 时）识别一条 query 里的多个独立词典内容概念，合并成单个 OR 组注入，不再静默丢弃较短概念。

**Architecture:** 仅改 `packages/harness/src/synonym/yaml.rs`。`gazetteer_lookup`（返回单个最长键）→ `gazetteer_lookup_multi`（非重叠贪心，返回多个键，按 query 首现排序）。`expand` 兼底分支把多个键各自 `expand_one` 后**并入一个 OR 组**（首现键作 head，其余 head+synonyms 去重并入）。后端/parser/词典源/recall 评测器逻辑全不动；只给 recall fixtures 加一个多概念 case 守回归。

**Tech Stack:** Rust，`cargo test`，BETA-15A `synonym_recall` 评测门，`bash scripts/ci.sh`。

设计来源：[spec](../specs/2026-06-01-beta-15e-multi-group-gazetteer-design.md)。

---

## 关键事实（实现前必读）

- 匹配语义「**组间 AND、组内 OR**」（`packages/evals/src/recall.rs:21-34`）。多概念必须合并成**一个**组才能 OR；多个组会 AND → 召回趋近 0。
- `KeywordGroup`（`packages/search-backends/common/src/expanded.rs:8`）：`pub head: String` + `pub synonyms: Vec<String>`；方法 `singleton` / `all()` / `is_singleton()`。
- `yaml.rs` 顶部已 `use locifind_search_backend::{ExpandedSearchIntent, KeywordGroup, SearchIntent};` 与 `use std::collections::HashMap;`。本计划新增 `merge_or_group` 用到 `HashSet`，需补 `use std::collections::HashSet;`（或全限定 `std::collections::HashSet`）。
- `expand_one(&self, keyword)`（`yaml.rs:275`）：命中词典返回 `{head=keyword, synonyms=其余词典成员}`，未命中返回 `KeywordGroup::singleton(keyword)`。
- `is_pure_content_term(term)`（`yaml.rs:369`）：重解析守护，类型/媒体/扩展名词返回 false。
- 工作区 `unsafe_code = forbid` 且 clippy `-D warnings`；生产代码**不得**用 `unwrap`/`expect`/`panic`（test 模块有 `#![allow(clippy::unwrap_used)]`）。
- 验证基线：harness `synonym` 单测全过；recall 门 `cargo test -p locifind-evals`（`synonym_recall_gate`）；recall 报告 `cargo run -p locifind-evals --bin synonym_recall`；parser-only evals `cargo run -p locifind-evals --bin evals` = **472/26/2**；`bash scripts/ci.sh` 全绿。

---

## Task 1: `gazetteer_lookup_multi` 非重叠贪心选词

把单键 `gazetteer_lookup` 换成返回多个非重叠内容键的 `gazetteer_lookup_multi`。

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`（替换 `gazetteer_lookup` 定义，位于 339-362 行；更新调用点 435 行；更新测试 693-705 行）

- [ ] **Step 1: 改现有单键测试为多键断言（先红）**

把 `packages/harness/src/synonym/yaml.rs` 的 `gazetteer_lookup_picks_content_term_and_skips_type_media` 测试（693-705 行）整体替换为：

```rust
    #[test]
    fn gazetteer_lookup_multi_picks_nonoverlapping_content_terms() {
        let e = expander();
        // 单内容词 → 单元素 vec
        assert_eq!(
            e.gazetteer_lookup_multi("找一份工作汇报相关的ppt"),
            vec!["工作汇报".to_string()]
        );
        // 媒体词被 is_pure_content_term 守护跳过 → 空
        assert!(e.gazetteer_lookup_multi("找一张截图").is_empty());
        // 无词典词 → 空
        assert!(e.gazetteer_lookup_multi("随便找点东西").is_empty());
    }
```

- [ ] **Step 2: 跑测试确认编译失败**

Run: `cargo test -p locifind-harness gazetteer_lookup_multi 2>&1 | tail -5`
Expected: 编译错误 `no method named gazetteer_lookup_multi`。

- [ ] **Step 3: 替换 `gazetteer_lookup` 为 `gazetteer_lookup_multi`**

把 339-362 行的整个 `gazetteer_lookup` 方法替换为：

```rust
    /// 兼底 gazetteer 多概念选词：返回 query 中**非重叠**的纯内容词典键，按 query 首现位置排序。
    /// 候选 = 出现在 query 中、过 [`is_pure_content_term`] 守护的索引键；贪心按「字符长降序、
    /// 首现字节位置升序」选取，跳过与已选键**字节跨度重叠**者（天然剔除子串包含，如「工作汇报」
    /// 已选时跳过其内的「汇报」）；最终按首现位置排序。无合格候选则返回空 `Vec`。
    fn gazetteer_lookup_multi(&self, query: &str) -> Vec<String> {
        // 候选: (字符长, 起始字节, 结束字节, key)
        let mut cands: Vec<(usize, usize, usize, &str)> = Vec::new();
        for key in self.zh_index.keys().chain(self.en_index.keys()) {
            let Some(pos) = query.find(key.as_str()) else {
                continue;
            };
            if !is_pure_content_term(key) {
                continue;
            }
            cands.push((key.chars().count(), pos, pos + key.len(), key.as_str()));
        }
        // 贪心优先级：长度降序，同长则首现位置升序
        cands.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut chosen: Vec<(usize, usize, &str)> = Vec::new(); // (start, end, key)
        for (_, start, end, key) in cands {
            let overlaps = chosen.iter().any(|(s, e, _)| start < *e && *s < end);
            if !overlaps {
                chosen.push((start, end, key));
            }
        }
        // 按 query 首现位置排序输出
        chosen.sort_by_key(|(s, _, _)| *s);
        chosen.into_iter().map(|(_, _, k)| k.to_owned()).collect()
    }
```

- [ ] **Step 4: 更新 `expand` 调用点（临时单组，保持编译）**

把 435 行 `.gazetteer_lookup(query)` 改为取首元素以暂时维持单组行为（Task 2 会改成合并）：

```rust
            _ => self
                .gazetteer_lookup_multi(query)
                .into_iter()
                .next()
                .or_else(|| bare_content_keyword(query))
                .map(|matched| vec![self.expand_one(&matched)])
                .unwrap_or_default(),
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p locifind-harness synonym 2>&1 | tail -15`
Expected: 全部 PASS（含新 `gazetteer_lookup_multi_picks_nonoverlapping_content_terms`、既有 `expand_gazetteer_injects_when_parser_gave_no_keyword`）。

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无 warning。

- [ ] **Step 7: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(beta-15e): gazetteer_lookup_multi 非重叠贪心选词（替换单键 lookup）"
```

---

## Task 2: 多概念合并成一个 OR 组

新增 `merge_or_group`，把 `expand` 兼底分支改成合并多键为单 OR 组。

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`（新增 `merge_or_group` 自由函数 + 改 `expand` 兼底分支 434-438 行 + 顶部补 `HashSet` import + 新增单测）

- [ ] **Step 1: 写多概念合并失败测试（先红）**

在 `mod expand_tests` 内（`fn zh_yaml` 之后任意位置）先扩充测试词典，把 `zh_yaml()` 函数体（623-633 行附近）替换为含两个内容概念 + 一个子串概念的版本：

```rust
    fn zh_yaml() -> &'static str {
        r"
version: 1
language: zh
groups:
  - head: 工作汇报
    aliases: [述职, 年度总结]
  - head: 截图
    aliases: [截屏, 屏幕截图]
  - head: 简历
    aliases: [个人简历, 求职简历]
  - head: 会议纪要
    aliases: [会议记录, 会议备忘]
"
    }
```

然后在 `mod expand_tests` 末尾（`expand_gazetteer_injects_when_parser_gave_no_keyword` 测试之后，798 行附近）加入：

```rust
    #[test]
    fn expand_gazetteer_merges_multiple_concepts_into_one_or_group() {
        let e = expander();
        // parser 对此自然 query 无 keyword → 走 gazetteer 兼底
        let intent = locifind_intent_parser::parse("找简历和会议纪要");
        assert!(intent.search_keywords().map_or(true, <[String]>::is_empty));
        let out = e.expand(intent, "找简历和会议纪要");
        // 两个概念合并成单个 OR 组（非两个 AND 组）
        assert_eq!(out.keyword_groups.len(), 1);
        let g = &out.keyword_groups[0];
        // head = query 首现概念（简历 在 会议纪要 之前）
        assert_eq!(g.head, "简历");
        // OR 成员含两个概念及各自同义词
        for term in ["简历", "个人简历", "求职简历", "会议纪要", "会议记录", "会议备忘"] {
            assert!(
                g.all().contains(&term),
                "merged group 应含 {term}，实际 {:?}",
                g.all()
            );
        }
    }

    #[test]
    fn expand_gazetteer_single_concept_stays_byte_equal() {
        let e = expander();
        // 单概念 query：合并退化为原单组单概念，与多概念前行为 byte-equal
        let intent = locifind_intent_parser::parse("找一份工作汇报相关的ppt");
        let out = e.expand(intent, "找一份工作汇报相关的ppt");
        assert_eq!(out.keyword_groups.len(), 1);
        assert_eq!(out.keyword_groups[0].head, "工作汇报");
        assert_eq!(out.keyword_groups[0].synonyms, vec!["述职", "年度总结"]);
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-harness expand_gazetteer_merges 2>&1 | tail -15`
Expected: FAIL —— 当前兼底只注入最长单键（会议纪要），断言 `head == "简历"` 与「含简历成员」失败。

- [ ] **Step 3: 顶部补 HashSet import**

把 `yaml.rs:4` 的 `use std::collections::HashMap;` 改为：

```rust
use std::collections::{HashMap, HashSet};
```

- [ ] **Step 4: 新增 `merge_or_group` 自由函数**

在 `dedup_groups_by_head`（449 行附近）之后插入：

```rust
/// 把多个 keyword 组合并成单个 OR 组：首组 head 作合并 head，其余组的 head + 全部 synonyms
/// 按序去重并入 synonyms（排除与 head 重复者）。空输入返回 `None`。
/// 单元素输入返回该组本身（保证单概念兼底与历史行为 byte-equal）。
fn merge_or_group(groups: Vec<KeywordGroup>) -> Option<KeywordGroup> {
    let mut iter = groups.into_iter();
    let first = iter.next()?;
    let head = first.head;
    let mut synonyms = first.synonyms;
    for g in iter {
        synonyms.push(g.head);
        synonyms.extend(g.synonyms);
    }
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(head.clone());
    synonyms.retain(|s| seen.insert(s.clone()));
    Some(KeywordGroup { head, synonyms })
}
```

- [ ] **Step 5: 改 `expand` 兼底分支用合并**

把 Task 1 Step 4 改出的兼底分支（434-440 行附近，`_ => self.gazetteer_lookup_multi(...)...`）替换为：

```rust
            // 兼底：parser 无 keyword → gazetteer 多概念（BETA-15E 后续，合并单 OR 组）；
            // 零命中再退裸内容词兜底（让「英语」这类非词典裸词走内容搜索而非 match-all）。
            _ => {
                let keys = self.gazetteer_lookup_multi(query);
                let merged =
                    merge_or_group(keys.iter().map(|k| self.expand_one(k)).collect());
                match merged {
                    Some(g) => vec![g],
                    None => bare_content_keyword(query)
                        .map(|matched| vec![self.expand_one(&matched)])
                        .unwrap_or_default(),
                }
            }
```

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test -p locifind-harness synonym 2>&1 | tail -20`
Expected: 全 PASS（新 2 个 + 既有全部，含 `expand_gazetteer_single_concept_stays_byte_equal`）。

- [ ] **Step 7: fmt + clippy + 全 harness 测试**

Run: `cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -5 && cargo test -p locifind-harness 2>&1 | tail -5`
Expected: 无 warning，全测试 PASS。

- [ ] **Step 8: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(beta-15e): gazetteer 多概念合并成单 OR 组（merge_or_group，组内 OR）"
```

---

## Task 3: recall fixture 多概念 case + 评测门验证

加一个多概念召回 case 守回归，跑全套 CI + evals 零回归确认。

**Files:**
- Modify: `packages/evals/fixtures/synonym-recall/cases.json`（新增一条多概念 case）

- [ ] **Step 1: 新增多概念 recall case**

在 `packages/evals/fixtures/synonym-recall/cases.json` 的 zh office 段末尾（最后一条 zh 用例之后，保持 JSON 数组合法）插入一条：

```json
  {
    "id": "recall-zh-office-multi-01",
    "query": "找简历和会议纪要，都要",
    "language": "zh",
    "bucket": "office",
    "expected_hits": ["f-geren-jl-pdf", "f-qiuzhi-jl-pdf", "f-huiyi-jl-docx", "f-huiyi-bw-txt"]
  },
```

> 说明：`简历`（命中 f-geren-jl-pdf「个人简历」/ f-qiuzhi-jl-pdf「求职简历」）+ `会议纪要`（别名 会议记录/会议备忘命中 f-huiyi-jl-docx「会议记录」/ f-huiyi-bw-txt「会议备忘」）。修复前 gazetteer 只注入最长键「会议纪要」→ 只召回 2/4；修复后两概念并入单 OR 组 → 召回 4/4。

- [ ] **Step 2: 确认 parser 对该 query 无 keyword（验证 case 走的是兼底路径）**

Run: `cargo run -p locifind-evals --bin synonym_recall 2>&1 | grep -A2 "multi-01" || cargo run -p locifind-evals --bin synonym_recall 2>&1 | tail -25`
Expected: 报告打印总召回 / 假阳 / 分桶，门通过（退出码 0）；multi-01 不在 failures 列表（若 `--only-failures` 默认不开则看总召回未跌）。

> 若该 case 召回 < 100%（说明 parser 意外抽了 keyword 或概念未进词典），停下诊断：用 `cargo run -p locifind-evals --bin synonym_recall -- --only-failures` 看 missing；确认 `简历`/`会议纪要` 在 `resources/synonyms/zh.yaml` 且 `parse("找简历和会议纪要，都要")` 无 keyword。

- [ ] **Step 3: 跑 recall 评测门（随 workspace test）**

Run: `cargo test -p locifind-evals synonym_recall_gate 2>&1 | tail -10`
Expected: PASS（总召回 ≥70% 且假阳 ≤5%；新增 case 不破门）。

- [ ] **Step 4: parser-only evals 零回归**

Run: `cargo run -p locifind-evals --bin evals 2>&1 | tail -8`
Expected: pass **472** / partial **26** / fail **2**（本改动不沾 parser，必须 byte-equal）。

- [ ] **Step 5: 全套 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -20`
Expected: fmt / clippy / build / test 全绿。

- [ ] **Step 6: Commit**

```bash
git add packages/evals/fixtures/synonym-recall/cases.json
git commit -m "test(beta-15e): recall fixture 加多概念 case（简历+会议纪要，守多 group 合并回归）"
```

---

## Self-Review 记录

- **Spec 覆盖**：决策 1（合并单 OR 组）→ Task 2 `merge_or_group`；决策 2（非重叠贪心）→ Task 1；决策 3（首现 head）→ Task 2 测试断言 + `merge_or_group` 取 first.head；不变量「单概念 byte-equal」→ Task 2 Step 1 专测；假阳门 → Task 3 干扰由 recall 全 corpus（100 文件含 20 干扰）天然提供 + 门校验。验证节全部映射到 Task 1/2/3 的 Step。
- **类型一致**：`gazetteer_lookup_multi`（Task 1 定义、Task 2 Step 5 调用）、`merge_or_group`（Task 2 定义+调用）、`KeywordGroup{head,synonyms}` / `expand_one` / `is_pure_content_term` 均与现码签名一致。
- **占位符**：无 TBD/TODO；每个代码 step 含完整代码与确切命令。
