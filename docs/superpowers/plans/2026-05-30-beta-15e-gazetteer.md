# BETA-15E 同义词 gazetteer 注入 keyword Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** 自然中文 query 含同义词词典内容名词短语时（如"工作汇报"），由 `SynonymExpander` 扫 query 命中并注入为 keyword group，触发同义词扩展 → BETA-15D 双查询命中（如 述职.ppt）。不动 parser / spotlight。

**Architecture:** `YamlSynonymExpander::expand` 加**兼底 gazetteer**：仅当 parser 无 keyword 时，扫 query 对 zh+en 索引 key 做子串匹配，用 `locifind_intent_parser::parse(候选)` 守护（类型/媒体词跳过），取最长（并列取首现）注入单个 group。`expand` 签名加 `query: &str`。

**Tech Stack:** Rust，`packages/harness`（新增 `locifind-intent-parser` 依赖），`apps/desktop`（调用透传 query）。

**Spec:** [docs/superpowers/specs/2026-05-30-beta-15e-gazetteer-design.md](../specs/2026-05-30-beta-15e-gazetteer-design.md)

**全程不变量（每 task 验证门）：**
- `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test`（即 `bash scripts/ci.sh` 子集；**每 task 必含 fmt**）。
- **不改 parser / spotlight / resources 词典源** → evals parser-only **472/26/2 byte-equal**（最终 task 实跑确认）。

---

## File Structure
- `packages/harness/Cargo.toml` — 加 `locifind-intent-parser` path 依赖。
- `packages/harness/src/synonym/yaml.rs` — 加 `is_pure_content_term` + `gazetteer_lookup` + `expand` 兼底分支 + 单测。
- `packages/harness/src/synonym/expander.rs` — `SynonymExpander::expand` 签名加 `query: &str`；`NoopExpander` 忽略；更新其测试。
- `apps/desktop/src-tauri/src/search.rs` — `expand(...)` 调用透传 `&query`；更新受影响测试。
- （实现者用 `git grep -n "\.expand("` 找全所有调用方一并改签名。）

---

## Task 1: gazetteer 内核（守护 + 扫描），未接线

**Files:**
- Modify: `packages/harness/Cargo.toml`
- Modify: `packages/harness/src/synonym/yaml.rs`

- [ ] **Step 1: 加依赖**

`packages/harness/Cargo.toml` 的 `[dependencies]` 加（已确认 harness↔intent-parser 互不依赖，无环）：
```toml
locifind-intent-parser = { path = "../intent-parser" }
```

- [ ] **Step 2: 写失败测试**（加到 `yaml.rs` 的 `#[cfg(test)] mod tests`；测试模块已有 `expander()` helper 返回 `YamlSynonymExpander`，见 ~line 499）

```rust
#[test]
fn is_pure_content_term_separates_content_from_type_media() {
    // 内容名词短语 → true
    assert!(is_pure_content_term("工作汇报"));
    assert!(is_pure_content_term("述职"));
    assert!(is_pure_content_term("报告"));
    assert!(is_pure_content_term("合同"));
    // 类型/媒体词（parser 消费）→ false
    assert!(!is_pure_content_term("幻灯片")); // file_type=presentation
    assert!(!is_pure_content_term("视频"));   // file_type=video
    assert!(!is_pure_content_term("文档"));   // file_type=document
    assert!(!is_pure_content_term("截图"));   // media_search/screenshot
}

#[test]
fn gazetteer_lookup_picks_longest_content_term() {
    let e = expander();
    // "工作汇报" 是词典 head，内容词 → 命中
    assert_eq!(e.gazetteer_lookup("找一份工作汇报相关的ppt").as_deref(), Some("工作汇报"));
    // 类型词 "幻灯片" 被守护跳过 → 无注入
    assert_eq!(e.gazetteer_lookup("找一份幻灯片"), None);
    // query 不含任何词典内容词 → None
    assert_eq!(e.gazetteer_lookup("随便找点东西"), None);
}
```

- [ ] **Step 3: 运行确认失败**

Run: `cd /Users/alice/Work/LocalFind && cargo test -p locifind-harness gazetteer_lookup is_pure_content_term`
Expected: 编译失败（函数不存在）。

- [ ] **Step 4: 实现**

在 `yaml.rs` 中 `impl YamlSynonymExpander` 内（`expand_one` 附近）加方法 + 一个自由函数：

```rust
    /// 兼底 gazetteer：在 query 中对 zh+en 索引 key 做子串匹配，返回唯一应注入的内容关键词。
    /// 候选 = 索引 key 出现在 query 中者；用 `is_pure_content_term` 守护跳过类型/媒体词；
    /// 取最长（按字符数），并列取 query 中首现位置最靠前者。无合格候选 → None。
    fn gazetteer_lookup(&self, query: &str) -> Option<String> {
        // (字符长度, 首现字节位置, key)
        let mut best: Option<(usize, usize, &str)> = None;
        for key in self.zh_index.keys().chain(self.en_index.keys()) {
            let Some(pos) = query.find(key.as_str()) else {
                continue;
            };
            if !is_pure_content_term(key) {
                continue;
            }
            let len = key.chars().count();
            let better = match best {
                None => true,
                Some((blen, bpos, _)) => len > blen || (len == blen && pos < bpos),
            };
            if better {
                best = Some((len, pos, key.as_str()));
            }
        }
        best.map(|(_, _, k)| k.to_owned())
    }
```

自由函数（放 `impl` 块外，文件内）：
```rust
/// 重解析守护：候选词若被 parser 分类为类型/媒体/扩展名信号则非纯内容词。
/// 以 parser 为类型判定单一信源 —— 内容名词短语（工作汇报/报告/合同…）parse 后
/// 无 file_type/extensions 且为 FileSearch；类型词（幻灯片→presentation）/媒体词
/// （截图→media_search）会被排除。
fn is_pure_content_term(term: &str) -> bool {
    match locifind_intent_parser::parse(term) {
        locifind_search_backend::SearchIntent::FileSearch(fs) => {
            fs.file_type.is_none() && fs.extensions.is_none()
        }
        _ => false,
    }
}
```

> `gazetteer_lookup` 此刻仅被测试调用 → clippy `-D warnings` 可能报 dead_code。若报，加 `#[allow(dead_code)] // BETA-15E: Task 2 接线` 于方法上，Task 2 移除。`is_pure_content_term` 被 gazetteer_lookup 用，不会 dead。

- [ ] **Step 5: 运行确认通过**

Run: `cd /Users/alice/Work/LocalFind && cargo test -p locifind-harness gazetteer_lookup is_pure_content_term`
Expected: PASS（2 测试）。

- [ ] **Step 6: 验证门 + Commit**

Run: `cd /Users/alice/Work/LocalFind && cargo fmt --check && cargo clippy -p locifind-harness --all-targets -- -D warnings && cargo test -p locifind-harness`

```bash
git add packages/harness/Cargo.toml packages/harness/src/synonym/yaml.rs
git commit -m "feat(harness): gazetteer 内核 is_pure_content_term + gazetteer_lookup(重解析守护+最长匹配, BETA-15E Task 1)"
```

---

## Task 2: 接线 expand（签名加 query + 兼底分支 + 更新所有调用方）

**Files:**
- Modify: `packages/harness/src/synonym/expander.rs`
- Modify: `packages/harness/src/synonym/yaml.rs`
- Modify: `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: 写失败测试**（`yaml.rs` 测试模块）

```rust
#[test]
fn expand_gazetteer_injects_when_parser_gave_no_keyword() {
    let e = expander();
    // 自然 query：parser 不提 keyword（用 parse 真实结果作 intent）
    let intent = locifind_intent_parser::parse("找一份工作汇报相关的ppt");
    assert!(intent.search_keywords().map_or(true, <[String]>::is_empty)); // 前提：parser 无 keyword（MSRV 1.80 用 map_or）
    let expanded = e.expand(intent, "找一份工作汇报相关的ppt");
    assert_eq!(expanded.keyword_groups.len(), 1);
    assert_eq!(expanded.keyword_groups[0].head, "工作汇报");
    assert!(expanded.keyword_groups[0].synonyms.contains(&"述职".to_string()));
}

#[test]
fn expand_does_not_gazetteer_when_parser_has_keyword() {
    let e = expander();
    // parser 已给 keyword 的显式 phrasing → 走原 expand_one，不被 gazetteer 覆盖
    let intent = locifind_intent_parser::parse("找文件名包含购房的pdf");
    assert_eq!(intent.search_keywords().map(<[String]>::len), Some(1));
    let expanded = e.expand(intent.clone(), "找文件名包含购房的pdf");
    assert_eq!(expanded.keyword_groups.len(), 1);
    assert_eq!(expanded.keyword_groups[0].head, "购房"); // 来自 parser，非 gazetteer
}

#[test]
fn expand_type_word_query_yields_identity() {
    let e = expander();
    let intent = locifind_intent_parser::parse("找一份幻灯片");
    let expanded = e.expand(intent, "找一份幻灯片");
    assert!(expanded.keyword_groups.is_empty()); // 幻灯片被守护跳过 → 无注入
}
```

> 若 `is_none_or` 不可用（MSRV 1.80 < 1.82）：改用 `intent.search_keywords().map_or(true, <[String]>::is_empty)`。（注：harness MSRV 与 workspace 一致 1.80，**用 `map_or`**。）

- [ ] **Step 2: 运行确认失败**

Run: `cd /Users/alice/Work/LocalFind && cargo test -p locifind-harness expand_gazetteer expand_does_not_gazetteer expand_type_word`
Expected: 编译失败（expand 签名不符 / arity）。

- [ ] **Step 3: 改 trait 签名**（`expander.rs`）

```rust
pub trait SynonymExpander: Send + Sync + std::fmt::Debug {
    /// `query` 为原始自然语言查询串，供兼底 gazetteer 使用（parser 无 keyword 时）。
    fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent;
}

impl SynonymExpander for NoopExpander {
    fn expand(&self, intent: SearchIntent, _query: &str) -> ExpandedSearchIntent {
        ExpandedSearchIntent::identity(intent)
    }
}
```
并更新 `expander.rs` 测试里两处 `NoopExpander.expand(intent)` → `NoopExpander.expand(intent, "")`。

- [ ] **Step 4: 改 `YamlSynonymExpander::expand`**（`yaml.rs`）

```rust
impl SynonymExpander for YamlSynonymExpander {
    fn expand(&self, intent: SearchIntent, query: &str) -> ExpandedSearchIntent {
        let groups = match intent.search_keywords() {
            Some(kws) if !kws.is_empty() => {
                kws.iter().map(|kw| self.expand_one(kw)).collect::<Vec<_>>()
            }
            // 兼底：parser 无 keyword → gazetteer 扫 query 注入单个内容词 group
            _ => match self.gazetteer_lookup(query) {
                Some(matched) => vec![self.expand_one(&matched)],
                None => Vec::new(),
            },
        };
        let (groups, _warn_truncated) = cap_keyword_groups(groups, RUNTIME_KEYWORD_CAP);
        ExpandedSearchIntent {
            base: intent,
            keyword_groups: groups,
        }
    }
}
```
移除 Task 1 在 `gazetteer_lookup` 上加的 `#[allow(dead_code)]`（现已被 expand 调用）。

- [ ] **Step 5: 更新所有 expand 调用方**

Run: `cd /Users/alice/Work/LocalFind && git grep -n "\.expand(" -- '*.rs'`
逐一改签名传 query：
- `apps/desktop/src-tauri/src/search.rs:251`：`deps.synonym_expander().expand(effective.clone(), &query)`（`query: String` 在 `search_impl` 入参，line 147，作用域内）。
- 任何 harness/desktop/evals 测试中的 `.expand(intent)` → 补 query 实参（测试一般传原 query 串或 `""`）。

- [ ] **Step 6: 运行确认通过**

Run: `cd /Users/alice/Work/LocalFind && cargo test -p locifind-harness && cargo test -p locifind-desktop`（desktop crate 名以实际为准，用 `cargo test --workspace` 兜底）
Expected: 新 3 测试 + 既有全过。

- [ ] **Step 7: 验证门 + Commit**

Run: `cd /Users/alice/Work/LocalFind && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

```bash
git add packages/harness/src/synonym/expander.rs packages/harness/src/synonym/yaml.rs apps/desktop/src-tauri/src/search.rs
git commit -m "feat(harness): expand 加 query 参数 + 兼底 gazetteer 注入 keyword + 透传调用方(BETA-15E Task 2)"
```

---

## Task 3: 全量验证 + 真机确认 + 文档同步

**Files:**
- Modify: `STATUS.md`, `ROADMAP.md`

- [ ] **Step 1: 全 workspace CI**

Run: `cd /Users/alice/Work/LocalFind && bash scripts/ci.sh`
Expected: 全过。

- [ ] **Step 2: evals byte-equal 回归**

Run: `cd /Users/alice/Work/LocalFind && cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 2>&1 | grep -E "pass:|partial:|fail:|variant"`
Expected: **pass 472 / partial 26 / fail 2 / variant 99.6%**（不动 parser，必 byte-equal）。

- [ ] **Step 3: 真机端到端确认（agent 可跑）**

构造并实测 gazetteer 产出的扩展 Q1 谓词命中（locifind-cli 不走 expand，故用 harness 集成验证 or 直接复核 expand 产物）。最低限度：
Run: `cd /Users/alice/Work/LocalFind && cargo test -p locifind-harness expand_gazetteer_injects_when_parser_gave_no_keyword -- --nocapture`
并人工确认 spec §4 数据流：`找一份工作汇报相关的ppt` → group head=工作汇报 + synonyms 含述职（扩展谓词命中 述职.ppt 已由 BETA-15D 阶段 mdfind 实测确认）。

- [ ] **Step 4: 同步 STATUS + ROADMAP**

- `ROADMAP.md` BETA-15E 行：`diagnosed` → `done`，补完成摘要（兼底 gazetteer + 重解析守护 + evals byte-equal）。
- `STATUS.md` 顶部加 BETA-15E done 段（架构、改动文件、验证、真机结论：自然 query `找一份工作汇报相关的ppt` 现经 expand 产出工作汇报组 → 解锁同义词 demo）。修订 BETA-15D 段中"手测 scenario 退化"的说明为"BETA-15E 已修，自然 query 可验证"。
- `docs/manual-test-scenarios.md` BETA-15D 节的 ⚠️ caveat：更新为"BETA-15E 已修复，scenario 1/4 自然 query 现可真正验证"。

- [ ] **Step 5: Commit**

```bash
git add STATUS.md ROADMAP.md docs/manual-test-scenarios.md
git commit -m "docs(beta-15e): done 收尾同步 STATUS/ROADMAP + 手测 caveat 解除(BETA-15E)"
```

---

## Self-Review（写完计划自查）

**Spec coverage：** §2 决策 1 兼底触发 → Task 2 Step 4；决策 2 最长匹配单 group → Task 1 gazetteer_lookup；决策 3 重解析守护 → Task 1 is_pure_content_term；决策 4 expand 加 query → Task 2 Step 3-5。§3 改动清单 → Task 1/2。§6 测试 → Task 1/2 各步。§7 验证门 → Task 3。

**Type/命名一致：** `is_pure_content_term(&str)->bool`、`gazetteer_lookup(&self,&str)->Option<String>`、`SynonymExpander::expand(&self, SearchIntent, &str)`、`NoopExpander`（非 IdentityExpander）跨 task 一致。MSRV 1.80 → 用 `map_or` 非 `is_none_or`（测试若用到已注明）。

**Placeholder：** 无 TBD；每 code step 含完整代码。

**风险：** desktop crate 名 / evals 是否有 expand 调用方 → Step 5 用 `git grep` 兜全；workspace clippy 守 dead_code。
