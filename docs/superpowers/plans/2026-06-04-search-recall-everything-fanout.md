# 搜索召回修复（Everything 并列 fan-out）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 内容查询（`keyword_groups` 非空）时让 Everything（纯文件名后端）并列参与 fan-out，恢复全盘召回；无关键词查询保持现状（避免 match-all），macOS 零行为变化。

**Architecture:** 仅改 `packages/harness/src/intent_router.rs` 的 `route_search_fanout` 内容分支：原本只返回 content-capable 后端，新增「`keyword_groups` 非空时追加非-content 后端（Everything）」。复用现有 `backend_indexes_content`/`expanded_needs_content`，合并去重与 Ranker 排序均不改。

**Tech Stack:** Rust，`cargo test`，harness 单测（mock 后端注入 backend_kind，可在 macOS 跑）。

设计来源：[spec](../specs/2026-06-04-search-recall-everything-fanout-design.md)。

---

## 关键事实（实现前必读）

- `route_search_fanout`（`intent_router.rs:173-202`）现状：`if expanded_needs_content { content = candidates.filter(content-capable); if !content.is_empty() { return Ok(content) } }` 否则返回单首位 `vec![first]`。
- `backend_indexes_content(kind)`（`intent_router.rs:260`）：`Spotlight | WindowsSearch | NativeIndex` 为 content；`Everything` 为 false（纯文件名）。
- `expanded_needs_content`（`intent_router.rs:231`）= `requires_content_or_metadata(base) || keyword_groups 非空`。**并列条件用更严的 `!expanded.keyword_groups.is_empty()`**（精确卡「有内容关键词」安全区）。
- `candidates = available_search_tools_supporting(...)` 按 id 升序（BTreeMap）：`everything < local < windows`。content filter 得 `[local, windows]`，filename filter 得 `[everything]`。
- 现有 fanout 测试用 `expanded_of(intent) = ExpandedSearchIntent::identity(intent)` → `keyword_groups` **为空** → 不触发并列 → 这些测试**不受影响**（自然验证「无 keyword_groups 不并列」）。
- 带 keyword_groups 的 expanded 构造模板（见 `intent_router.rs:607`）：
  ```rust
  ExpandedSearchIntent {
      base: file_search_extensions_only(),
      keyword_groups: vec![KeywordGroup { head: "工作汇报".to_owned(), synonyms: vec!["述职".to_owned()] }],
  }
  ```
- mock 后端：`FakeKindBackend(BackendKind::X)` + `SearchTool::new(id, name, backend, vec![SupportedIntent::FileSearch], desc)`（见 `intent_router.rs:698-705`）。`registry_three_backends()`（`:695`）= Everything(filename) + WindowsSearch(content) + NativeIndex/`search.local`(content)。
- 工作区 `unsafe_code = forbid`、clippy `-D warnings`；生产代码无 unwrap/expect/panic（test 模块允许）。

---

## Task 1: route_search_fanout 有关键词时并列 Everything

**Files:**
- Modify: `packages/harness/src/intent_router.rs`（`route_search_fanout` 189-198 行 + tests mod 新增 2 测试 + 更新 1 个现有测试注释）

- [ ] **Step 1: 写失败测试**

在 tests mod 的 fanout 测试区（`fanout_no_backend_and_clarify_errors` 之后，约 777 行 `}` 后）插入：

```rust
    #[test]
    fn fanout_keyword_query_includes_everything_for_full_recall() {
        use locifind_search_backend::{ExpandedSearchIntent, KeywordGroup};
        // 有内容关键词组 → content 后端(windows + local) 并列 Everything(filename) 做全盘召回。
        let registry = registry_three_backends();
        let router = IntentRouter::new(&registry);
        let expanded = ExpandedSearchIntent {
            base: file_search_extensions_only(),
            keyword_groups: vec![KeywordGroup {
                head: "工作汇报".to_owned(),
                synonyms: vec!["述职".to_owned()],
            }],
        };
        let fanout = router.route_search_fanout(&expanded).unwrap();
        assert_eq!(fanout.len(), 3, "content(windows+local) + Everything 并列，实际 {fanout:?}");
        assert!(
            fanout.iter().any(|t| t.id() == "search.everything"),
            "有关键词查询应并列 Everything"
        );
        assert!(
            fanout
                .iter()
                .any(|t| backend_indexes_content(t.capability().backend_kind)),
            "应仍含 content 后端"
        );
    }

    #[test]
    fn fanout_keyword_query_content_only_without_filename_backend() {
        use locifind_search_backend::{ExpandedSearchIntent, KeywordGroup};
        // macOS 形态：无纯文件名后端 → 即使有关键词，并列集 = content only（零行为变化）。
        let mut registry = ToolRegistry::new();
        registry
            .register_search(SearchTool::new(
                "search.windows_search",
                "WindowsSearch",
                FakeKindBackend(BackendKind::WindowsSearch),
                vec![SupportedIntent::FileSearch],
                "ws",
            ))
            .unwrap();
        registry
            .register_search(SearchTool::new(
                "search.local",
                "LocalIndex",
                FakeKindBackend(BackendKind::NativeIndex),
                vec![SupportedIntent::FileSearch],
                "local",
            ))
            .unwrap();
        let router = IntentRouter::new(&registry);
        let expanded = ExpandedSearchIntent {
            base: file_search_extensions_only(),
            keyword_groups: vec![KeywordGroup {
                head: "工作汇报".to_owned(),
                synonyms: vec![],
            }],
        };
        let fanout = router.route_search_fanout(&expanded).unwrap();
        assert!(
            fanout
                .iter()
                .all(|t| backend_indexes_content(t.capability().backend_kind)),
            "无 filename 后端时并列集应只含 content 后端，实际 {fanout:?}"
        );
    }
```

- [ ] **Step 2: 运行新测试，确认失败**

Run: `cargo test -p locifind-harness fanout_keyword_query_includes_everything_for_full_recall 2>&1 | tail -8`
Expected: FAIL —— 当前 fanout 内容分支只返回 content 后端（2 个），断言 `len == 3` 失败。

- [ ] **Step 3: 改 route_search_fanout 内容分支**

把 `intent_router.rs:189-198` 的内容分支：

```rust
        if expanded_needs_content(expanded) {
            let content: Vec<Arc<dyn SearchableTool>> = candidates
                .iter()
                .filter(|tool| backend_indexes_content(tool.capability().backend_kind))
                .map(Arc::clone)
                .collect();
            if !content.is_empty() {
                return Ok(content);
            }
        }
```

替换为：

```rust
        if expanded_needs_content(expanded) {
            let mut selected: Vec<Arc<dyn SearchableTool>> = candidates
                .iter()
                .filter(|tool| backend_indexes_content(tool.capability().backend_kind))
                .map(Arc::clone)
                .collect();
            // 有内容关键词时，并列纯文件名后端（Everything）做全盘召回——有关键词时其文件名
            // 匹配是合理召回、不会 match-all（无关键词查询才会，见 search.rs 注释），故无关键词
            // 的纯类型查询不并列、维持现状。无纯文件名后端（如 macOS）时此追加为空 → 零变化。
            if !expanded.keyword_groups.is_empty() {
                selected.extend(
                    candidates
                        .iter()
                        .filter(|tool| !backend_indexes_content(tool.capability().backend_kind))
                        .map(Arc::clone),
                );
            }
            if !selected.is_empty() {
                return Ok(selected);
            }
        }
```

- [ ] **Step 4: 运行新测试 + 全 harness 路由测试，确认通过**

Run: `cargo test -p locifind-harness intent_router 2>&1 | tail -15`
Expected: 全 PASS（含新 2 个；现有 `fanout_content_query_returns_all_content_backends`/`fanout_attribute_only_returns_single_primary` 等仍过——它们用 `expanded_of`，keyword_groups 为空、不触发并列）。

- [ ] **Step 5: 更新现有测试注释（明确修复边界）**

把 `fanout_content_query_returns_all_content_backends`（约 710 行）的注释行：

```rust
        // 内容查询 → 全部 content-capable（WindowsSearch + NativeIndex），排除 Everything。
```

更新为：

```rust
        // 无 keyword_groups 的内容查询（base 有 keyword 但未扩词）→ 仅 content-capable，
        // 不并列 Everything（并列只在 keyword_groups 非空时发生）。
```

- [ ] **Step 6: fmt + clippy + 全 workspace 测试（macOS 形态零回归）**

Run: `cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3 && cargo test --workspace 2>&1 | grep -E "test result: FAILED|^test .* FAILED" | sort -u`
Expected: fmt 无输出、clippy 0 告警、最后 grep 为空（全 workspace 零失败）。

- [ ] **Step 7: Commit**

```bash
git add packages/harness/src/intent_router.rs
git commit -m "feat(harness): 有关键词时 Everything 并列 fan-out，修搜索召回遮蔽"
```

---

## Self-Review 记录

- **Spec 覆盖**：决策 1（并列条件 `keyword_groups 非空`）→ Step 3 的 `if !expanded.keyword_groups.is_empty()`；决策 2（不限流）→ 不改 Ranker/limit，无对应代码；决策 3（保留兜底）→ 不动 `route_filename_fallback`，无对应改动；§4 跨平台 macOS 零变化 → `fanout_keyword_query_content_only_without_filename_backend` 测试 + Step 6 全 workspace；§6 护栏（无关键词不变）→ 现有 `fanout_*` 测试用 expanded_of 自然覆盖 + Step 5 注释明确。
- **占位符**：无 TBD/TODO；每个代码 step 含完整代码与确切命令。
- **类型一致**：`route_search_fanout(&ExpandedSearchIntent) -> Result<Vec<Arc<dyn SearchableTool>>, RouteError>`、`backend_indexes_content`、`expanded.keyword_groups`、`SearchTool::new`/`FakeKindBackend`/`registry_three_backends` 均与现码一致。变量改名 `content` → `selected`（语义更准，已含 filename 后端）。
- **真机验证**：macOS 单测覆盖路由逻辑；端到端召回完整性须 Windows + Everything 真机（spec §8）。
