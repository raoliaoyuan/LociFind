# BETA-13 同义词召回回归修复 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不回退 v0.9（726）的前提下，把 gazetteer 从「parser 无 keyword 兼底」升为「召回核心内容词提取器」，让 `expand` 进入即扫 query 多概念、命中即用合并 OR 组替代 parser 脏 keyword，使 recall 门从 5.9% 恢复 ≥70%。

**Architecture:** 仅改 `packages/harness/src/synonym/yaml.rs`（纯召回侧；v0.5/v0.9 是 parser-only evals、不经 expand，天然 byte-equal）。复用 BETA-15E 的 `gazetteer_lookup_multi`（替换单键 `gazetteer_lookup`）+ 新增 `merge_or_group`（多概念合并单 OR 组）；`expand` 改为「先扫 gazetteer，命中 ≥1 → 合并 OR 组替代；零命中 → 原 parser keyword / 裸词兜底」。

**Tech Stack:** Rust，`cargo test`，BETA-15A `synonym_recall` 评测门，`bash scripts/ci.sh`。

设计来源：[spec](../specs/2026-06-04-beta-13-recall-regression-gazetteer-core-design.md)。

---

## 关键事实（实现前必读）

- 匹配语义「**组间 AND、组内 OR**」（`packages/evals/src/recall.rs`，忠实 BETA-15D 真实后端）。多概念必须合并成**一个** OR 组才能并集召回；多组 AND 会碾压。
- `KeywordGroup`（`locifind_search_backend`）：`pub head: String` + `pub synonyms: Vec<String>`；`expand_one(&self, kw)` 命中词典返回 `{head=kw, synonyms=其余成员}`，未命中返回 `KeywordGroup::singleton(kw)`。
- `is_pure_content_term(term)`（`yaml.rs:369`）：parser 重解析守护，类型/媒体/扩展名词返回 false。
- `yaml.rs:4` 当前 `use std::collections::HashMap;`——本计划 `merge_or_group` 用 `HashSet`，需改为 `use std::collections::{HashMap, HashSet};`。
- 工作区 `unsafe_code = forbid` 且 clippy `-D warnings`；生产代码**不得** `unwrap`/`expect`/`panic`（test 模块有 `#![allow(clippy::unwrap_used)]`）。
- `expand` 现状（`yaml.rs:424-446`）：`match intent.search_keywords()`，有 keyword 走 `expand_one`+`apply_multiword_override`+`dedup`，否则走 `gazetteer_lookup → bare_content_keyword`。
- recall 门基线：当前 **5.9% FAILED**（BETA-13 引入）；目标 ≥70% 且 fp ≤5%。
- 验证基线：v0.5 `--fixtures v0.5` = **473/25/2**、v0.9 `--fixtures v0.9` = **726/225/49**（parser-only、本改动不得动）；harness 全单测；`bash scripts/ci.sh` 全绿。

---

## Task 1: 引入 `gazetteer_lookup_multi` + `merge_or_group`（兼底分支合并 OR 组）

替换单键 `gazetteer_lookup` 为多概念 `gazetteer_lookup_multi`；新增 `merge_or_group`；expand 兼底分支（仅 parser 无 keyword 时）改用合并 OR 组。此 Task 等价 BETA-15E Task1+2，recall 仍 5.9%（兜底只在无 keyword 触发）。

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`

- [ ] **Step 1: 改 HashSet import**

把 `yaml.rs:4` 的 `use std::collections::HashMap;` 改为：

```rust
use std::collections::{HashMap, HashSet};
```

- [ ] **Step 2: 把单键 `gazetteer_lookup`（342-362 行）整体替换为 `gazetteer_lookup_multi`**

```rust
    /// 召回多概念选词：返回 query 中**非重叠**的纯内容词典键，按 query 首现位置排序。
    /// 候选 = 出现在 query 中、过 [`is_pure_content_term`] 守护的索引键；贪心按「字符长降序、
    /// 首现位置升序」选取，跳过与已选键**字节跨度重叠**者（天然剔除子串包含，如「工作汇报」
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

- [ ] **Step 3: 新增 `merge_or_group` 自由函数（在 `dedup_groups_by_head` 之后，约 453 行后插入）**

```rust
/// 把多个 keyword 组合并成单个 OR 组：首组 head 作合并 head，其余组的 head + 全部 synonyms
/// 按序去重并入 synonyms（排除与 head 重复者）。空输入返回 `None`；单元素输入返回该组本身
/// （保证单概念与历史行为一致）。
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

- [ ] **Step 4: 改 expand 兼底分支（434-438 行）用 `gazetteer_lookup_multi` + `merge_or_group`**

把 `_ =>` 分支：

```rust
            _ => self
                .gazetteer_lookup(query)
                .or_else(|| bare_content_keyword(query))
                .map(|matched| vec![self.expand_one(&matched)])
                .unwrap_or_default(),
```

替换为：

```rust
            _ => {
                let keys = self.gazetteer_lookup_multi(query);
                match merge_or_group(keys.iter().map(|k| self.expand_one(k)).collect()) {
                    Some(g) => vec![g],
                    None => bare_content_keyword(query)
                        .map(|matched| vec![self.expand_one(&matched)])
                        .unwrap_or_default(),
                }
            }
```

- [ ] **Step 5: 更新 `gazetteer_lookup` 单测为 multi 版（694-705 行附近）**

把 `gazetteer_lookup_picks_content_term_and_skips_type_media` 测试整体替换为：

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

- [ ] **Step 6: 跑 harness synonym 测试 + fmt + clippy**

Run: `cargo test -p locifind-harness synonym 2>&1 | tail -8 && cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -3`
Expected: 全 PASS（含新 `gazetteer_lookup_multi_picks_nonoverlapping_content_terms`、既有 `expand_gazetteer_injects_when_no_keyword`）；clippy 0 告警。

- [ ] **Step 7: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(harness): gazetteer_lookup_multi + merge_or_group（兼底多概念合并单 OR 组，承接 BETA-15E）"
```

---

## Task 2: `expand` 升级为召回核心提取器（进入即扫、命中即替代）

本修复核心。`expand` 改为：进入先扫 `gazetteer_lookup_multi`，命中 ≥1 → 合并 OR 组替代 parser keyword；零命中 → 原 parser keyword / 裸词兜底。recall 由此 5.9% → ≥70%。

**Files:**
- Modify: `packages/harness/src/synonym/yaml.rs`（`expand` 424-446 行 + 更新 804 测试 + 新增 3 个行为单测）

- [ ] **Step 1: 写失败/语义测试（先红/先验证新行为）**

把 `expand_does_not_gazetteer_when_parser_has_keyword`（804-812 行）整体替换为以下**反转语义**的测试（验证 parser 有 keyword 时 gazetteer 仍命中替代），并在其后新增 3 个行为单测：

```rust
    #[test]
    fn expand_gazetteer_overrides_even_when_parser_has_keyword() {
        // BETA-13 召回修复：parser 有 keyword 时，gazetteer 仍扫 query 核心内容词并替代，
        // 让结果带同义词扩展（修复前此路径不走 gazetteer、无同义词 → 召回崩）。
        let e = expander();
        let intent = locifind_intent_parser::parse("找文件名包含工作汇报的ppt");
        assert_eq!(intent.search_keywords().map(<[String]>::len), Some(1));
        let expanded = e.expand(intent, "找文件名包含工作汇报的ppt");
        assert_eq!(expanded.keyword_groups.len(), 1);
        assert_eq!(expanded.keyword_groups[0].head, "工作汇报");
        // 关键：现在走 gazetteer → 带同义词「述职」（修复前不带）
        assert!(expanded.keyword_groups[0]
            .synonyms
            .contains(&"述职".to_string()));
    }

    #[test]
    fn expand_gazetteer_drops_modifier_keyword_into_single_or_group() {
        // 崩塌模式 1（合同+乙方）：parser 多抽修饰语 → 组间 AND 碾压。gazetteer 命中核心词
        // 「工作汇报」→ 替代成单 OR 组、甩掉修饰语「张三」（不再两组 AND）。
        let e = expander();
        let intent = intent_with(vec!["工作汇报", "张三"]);
        let expanded = e.expand(intent, "工作汇报相关的，张三那份");
        assert_eq!(expanded.keyword_groups.len(), 1, "应合并为单 OR 组");
        assert_eq!(expanded.keyword_groups[0].head, "工作汇报");
        assert!(expanded.keyword_groups[0]
            .synonyms
            .contains(&"述职".to_string()));
        assert!(
            !expanded.keyword_groups[0].all().contains(&"张三"),
            "修饰语张三不应进组"
        );
    }

    #[test]
    fn expand_gazetteer_rescues_dirty_keyword() {
        // 崩塌模式 2（简历在哪）：parser 分词不净。gazetteer 从 query 原文扫到干净
        // 「工作汇报」→ 替代脏 keyword「工作汇报在哪」。
        let e = expander();
        let intent = intent_with(vec!["工作汇报在哪"]);
        let expanded = e.expand(intent, "我的工作汇报在哪");
        assert_eq!(expanded.keyword_groups.len(), 1);
        assert_eq!(expanded.keyword_groups[0].head, "工作汇报");
    }

    #[test]
    fn expand_keeps_parser_keyword_when_gazetteer_misses() {
        // gazetteer 零命中（query 无词典内容词）→ 保留 parser keyword（不恶化）。
        let e = expander();
        let intent = intent_with(vec!["项目计划"]);
        let expanded = e.expand(intent, "项目计划相关");
        assert_eq!(expanded.keyword_groups.len(), 1);
        assert_eq!(expanded.keyword_groups[0].head, "项目计划");
    }
```

- [ ] **Step 2: 跑新测试确认失败（除被动兼容的外）**

Run: `cargo test -p locifind-harness expand_gazetteer_drops_modifier_keyword_into_single_or_group 2>&1 | tail -8`
Expected: FAIL —— 当前 expand 对有 keyword 的 intent 走 `expand_one` 两组（工作汇报 + 张三），断言 `len==1` 失败。

- [ ] **Step 3: 改写 `expand`（424-446 行）为「先扫 gazetteer、命中即替代」**

把 `fn expand` 的 `let groups = match ... ;` 整段替换为：

```rust
    fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent {
        // BETA-13 召回回归修复：gazetteer 升为召回核心内容词提取器——无论 parser 是否抽出
        // keyword，进入 expand 先扫 query 原文的词典核心内容词；命中 ≥1 → 合并单 OR 组替代
        // （甩掉 parser 的噪声/修饰语 keyword、用组内 OR 避免组间 AND 碾压）。零命中再走原
        // 路径（parser keyword + 多词覆盖 / 裸内容词兜底）。
        let gaz = self.gazetteer_lookup_multi(query);
        let groups = if let Some(merged) =
            merge_or_group(gaz.iter().map(|k| self.expand_one(k)).collect())
        {
            vec![merged]
        } else {
            match intent.search_keywords() {
                Some(kws) if !kws.is_empty() => {
                    let mut gs = kws.iter().map(|kw| self.expand_one(kw)).collect::<Vec<_>>();
                    self.apply_multiword_override(query, &mut gs);
                    dedup_groups_by_head(&mut gs);
                    gs
                }
                _ => bare_content_keyword(query)
                    .map(|matched| vec![self.expand_one(&matched)])
                    .unwrap_or_default(),
            }
        };
        let (groups, _warn_truncated) = cap_keyword_groups(groups, RUNTIME_KEYWORD_CAP);
        ExpandedSearchIntent {
            base: intent,
            keyword_groups: groups,
        }
    }
```

> 注：Task 1 Step 4 改的 `_ =>` 兼底分支现被并入此 else 块；`gazetteer_lookup_multi`/`merge_or_group` 现由本函数顶部调用，Task 1 的兼底分支调用点已被本次重写覆盖（不再有重复 gazetteer 调用）。

- [ ] **Step 4: 跑 harness 全 synonym 测试**

Run: `cargo test -p locifind-harness synonym 2>&1 | tail -15`
Expected: 全 PASS（新增 4 个测试 + 既有全部；`expand_gazetteer_injects_when_no_keyword`、`expand_fallback_no_dict_hit_yields_identity`、`expand_bare_content_word_*` 仍通过——它们的 query 要么 gazetteer 命中且 head 不变、要么零命中走原兜底）。

- [ ] **Step 5: 跑 harness 全 crate 测试 + fmt + clippy**

Run: `cargo test -p locifind-harness 2>&1 | tail -5 && cargo fmt -p locifind-harness && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -3`
Expected: 全 PASS；clippy 0 告警。若有其它 harness 测试因 query 含词典词、走了 gazetteer 替代而断言失败，按新语义（命中即替代成单 OR 组）更新其断言。

- [ ] **Step 6: Commit**

```bash
git add packages/harness/src/synonym/yaml.rs
git commit -m "feat(harness): expand 升为召回核心提取器（gazetteer 进入即扫、命中即替代），修 BETA-13 召回回归"
```

---

## Task 3: recall 门恢复 + 全套护栏验证

确认 recall 门由 5.9% 恢复 ≥70%（fp ≤5%），且 v0.5/v0.9 parser-only 零回归、ci.sh 全绿。

**Files:**
- 无源码改动（纯验证；若 recall 未达门则回 Task 2 诊断）

- [ ] **Step 1: recall 门**

Run: `cargo test -p locifind-evals synonym_recall_meets_gate 2>&1 | tail -5`
Expected: PASS（不再 `召回门槛未过`）。

- [ ] **Step 2: recall 报告核对召回率与假阳**

Run: `cargo run -q -p locifind-evals --bin synonym_recall 2>&1 | tail -5`
Expected: 总召回 ≥70%、fp ≤5%。若召回未达标，记录仍漏的 case（其核心词可能不在 ship 词典），回 Task 2 评估；**不得改 fixtures/词典凑指标**。

- [ ] **Step 3: v0.5 / v0.9 parser-only 零回归（byte-equal 锚点）**

Run: `cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 2>&1 | grep -A4 总览 && cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 2>&1 | grep -A4 总览`
Expected: v0.5 pass **473** / partial 25 / fail 2；v0.9 pass **726** / partial 225 / fail 49（本改动不沾 parser，必须不变）。

- [ ] **Step 4: 全套 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -20`
Expected: fmt / clippy / build / test 全绿（含 `synonym_recall_meets_gate`）。

- [ ] **Step 5: Commit（若 Step 1-4 全过、无源码改动则跳过；否则提交修正）**

```bash
git add -A
git commit -m "test(harness): 确认 BETA-13 召回回归修复——recall 门恢复 ≥70%、v0.5/v0.9 零回归"
```

---

## Self-Review 记录

- **Spec 覆盖**：决策 1（gazetteer 升核心提取器）→ Task 2 Step 3 `expand` 重写；决策 2（命中即替代）→ Task 2 Step 3 的 `if let Some(merged)…else`；决策 3（多核心词合并单 OR 组）→ Task 1 `merge_or_group` + Task 2 单测 `drops_modifier_keyword`；§4 复用 BETA-15E → Task 1（`gazetteer_lookup_multi` 替换单键、新增 `merge_or_group`）；§6 不变量（v0.5/v0.9 byte-equal、recall ≥70%、fp ≤5%、harness 单测更新）→ Task 3 + Task 2 Step 1/5；§7 验证全部映射到 Task 1 Step 6 / Task 2 Step 4-5 / Task 3 Step 1-4。
- **崩塌模式覆盖**：模式 1（多抽修饰语 AND 碾压）→ `expand_gazetteer_drops_modifier_keyword_into_single_or_group`；模式 2（分词不净）→ `expand_gazetteer_rescues_dirty_keyword`；零命中回退 → `expand_keeps_parser_keyword_when_gazetteer_misses`。
- **类型一致**：`gazetteer_lookup_multi(&self, &str) -> Vec<String>`（Task 1 定义、Task 2 Step 3 调用）、`merge_or_group(Vec<KeywordGroup>) -> Option<KeywordGroup>`（Task 1 定义、Task 2 调用）、`KeywordGroup{head,synonyms}`/`expand_one`/`is_pure_content_term`/`bare_content_keyword`/`apply_multiword_override`/`dedup_groups_by_head` 均与 main 现码签名一致。
- **占位符**：无 TBD/TODO；每个代码 step 含完整代码与确切命令。
- **已知风险（spec §8）**：替代降精确性（有意权衡）、词典覆盖依赖（Task 3 Step 2 兜底诊断、不凑指标）、真实搜索行为变化（recall fp 门守）。
