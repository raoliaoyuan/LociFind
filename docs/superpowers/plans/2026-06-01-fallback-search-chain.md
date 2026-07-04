# 搜索后端 fallback chain（mid-stream retry）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 选中搜索后端失败（起不来/中途崩/零结果）时，按有序候选列表逐个回退，跨候选按 canonical path 去重合并结果，并经新增 `BackendSwitched` 事件向 UI/trace 显式通报。

**Architecture:** 可测编排核心 `run_fallback_chain` + 有序候选 `IntentRouter::route_search_chain` 放 harness（平台无关，mock 单测）；desktop `search.rs` 仅做事件适配（chain 回调 → `SearchEvent`）。Mac 单候选退化为现状（零回归），真双后端集成留 Windows。

**Tech Stack:** Rust async（`futures_util` Stream/StreamExt）、`Arc<dyn SearchableTool>`、`HashSet<PathBuf>` dedup、Tauri `Channel<SearchEvent>`、React/TS 前端最小处理。

**前置类型（已核实，所有任务通用）：**
- `BackendSearchFuture<'a> = Pin<Box<dyn Future<Output = Result<BackendStream, SearchError>> + Send + 'a>>`
- `BackendStream = Pin<Box<dyn Stream<Item = Result<SearchResult, SearchError>> + Send>>`
- `SearchError::BackendUnavailable { reason }`（→ `SwitchReason::Unavailable`），其余变体 → `SwitchReason::Error`
- helper（`locifind_search_backend`）：`backend_stream_from_results(Vec<SearchResult>, CancellationToken) -> BackendStream`、`backend_stream_from_error(SearchError) -> BackendStream`
- mock 后端模式：实现 `SearchBackend` → `SearchTool::new(id, name, backend, supported, desc)` → `register_search` → `Arc<dyn SearchableTool>`
- `SearchableTool::search_expanded(&self, &ExpandedSearchIntent, CancellationToken) -> BackendSearchFuture`
- `SearchResult.path: PathBuf`（dedup 键）

---

### Task 1: `IntentRouter::route_search_chain`（有序候选列表）

**Files:**
- Modify: `packages/harness/src/intent_router.rs`（在 `route_search_expanded` 后新增方法 + 测试模块加 3 个测试）

- [ ] **Step 1: 写失败测试**

在 `intent_router.rs` 的 `#[cfg(test)] mod tests` 内追加（复用已有的 `registry_everything_and_windows()` / `file_search_intent()` / content-query helper；若无 content-query helper 用 `expanded_of(intent)` 包装）：

```rust
#[test]
fn route_search_chain_returns_all_candidates_ordered() {
    let registry = registry_everything_and_windows();
    let router = IntentRouter::new(&registry);
    // 纯文件名查询（不需内容）→ 沿用 id 序，两个候选都在
    let expanded = expanded_of(file_search_intent());
    let chain = router.route_search_chain(&expanded).unwrap();
    assert_eq!(chain.len(), 2, "应返回全部可用候选");
}

#[test]
fn route_search_chain_content_query_puts_rich_backend_first() {
    let registry = registry_everything_and_windows();
    let router = IntentRouter::new(&registry);
    // 内容查询 → 内容型后端（WindowsSearch）排首位
    let expanded = expanded_of(content_query_intent());
    let chain = router.route_search_chain(&expanded).unwrap();
    assert!(
        backend_indexes_content(chain[0].capability().backend_kind),
        "内容查询内容型后端应排首位"
    );
}

#[test]
fn route_search_chain_no_backend_when_empty() {
    let registry = ToolRegistry::new();
    let router = IntentRouter::new(&registry);
    let expanded = expanded_of(file_search_intent());
    assert_eq!(
        router.route_search_chain(&expanded).unwrap_err(),
        RouteError::NoBackend
    );
}
```

> 若测试模块没有 `expanded_of` / `content_query_intent` helper，本步同时添加：
> ```rust
> fn expanded_of(intent: SearchIntent) -> ExpandedSearchIntent {
>     ExpandedSearchIntent::identity(intent)   // 若构造名不同，按 common crate 实际 API 调整
> }
> ```
> `content_query_intent()` = 一个 `keywords` 非空的 FileSearch（参考已有能力路由测试里构造内容查询的 helper，直接复用其名）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p locifind-harness route_search_chain 2>&1 | grep -E "test result|error\["`
Expected: 编译失败（`route_search_chain` 不存在）或测试 FAIL。

- [ ] **Step 3: 实现 `route_search_chain`**

在 `intent_router.rs` 的 `impl<'a> IntentRouter<'a>` 内、`route_search_expanded` 之后插入：

```rust
/// 返回**有序候选列表**，供 fallback chain 逐个回退使用。
///
/// 排序与 [`Self::route_search_expanded`] 一致：base 需要内容/元数据，或扩展产生
/// 任何内容关键词组 → 内容型后端排首位；否则沿用 id 序。与单选版区别仅在于返回
/// 全部候选而非首位。
pub fn route_search_chain(
    &self,
    expanded: &ExpandedSearchIntent,
) -> Result<Vec<Arc<dyn SearchableTool>>, RouteError> {
    let supported_intent = SupportedIntent::from_intent(&expanded.base);
    if supported_intent == SupportedIntent::Clarify {
        return Err(RouteError::ClarifyNotRoutable);
    }

    let mut candidates = self
        .registry
        .available_search_tools_supporting(supported_intent);
    if candidates.is_empty() {
        return Err(RouteError::NoBackend);
    }

    let needs_content = requires_content_or_metadata(&expanded.base)
        || expanded
            .keyword_groups
            .iter()
            .any(|group| !group.head.trim().is_empty());
    if needs_content {
        // 把首个内容型后端稳定前移到 index 0，其余相对顺序不变。
        if let Some(pos) = candidates
            .iter()
            .position(|tool| backend_indexes_content(tool.capability().backend_kind))
        {
            let rich = candidates.remove(pos);
            candidates.insert(0, rich);
        }
    }
    Ok(candidates)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p locifind-harness route_search_chain 2>&1 | grep "test result"`
Expected: `ok. 3 passed`

- [ ] **Step 5: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-harness --check
cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | grep -E "^error|^warning" || echo clean
git add packages/harness/src/intent_router.rs
git commit -m "feat(fallback-chain): IntentRouter::route_search_chain 返回有序候选列表"
```

---

### Task 2: `fallback_chain.rs` 编排核心 + 8 个 mock 单测

**Files:**
- Create: `packages/harness/src/fallback_chain.rs`
- Modify: `packages/harness/src/lib.rs`（`mod fallback_chain;` + `pub use`）

- [ ] **Step 1: 建模块骨架 + 类型 + 在 lib.rs 挂载**

创建 `packages/harness/src/fallback_chain.rs`：

```rust
//! 搜索后端 fallback chain：选中后端失败（起不来/中途崩/零结果）时按有序候选
//! 逐个回退，跨候选按 canonical path 去重合并。详见
//! `docs/superpowers/specs/2026-06-01-fallback-search-chain-design.md`。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;
use locifind_search_backend::{
    CancellationToken, ExpandedSearchIntent, SearchError, SearchResult,
};

use crate::SearchableTool;

/// 切换原因，对应三类失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchReason {
    /// 后端整体不可用（pre-stream `BackendUnavailable`）。
    Unavailable,
    /// 其它错误（pre-stream 非 unavailable，或流中途 Err）。
    Error,
    /// 干净跑完但本候选贡献 0 条新结果。
    Empty,
}

impl SwitchReason {
    /// 前端/trace 用的稳定字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Error => "error",
            Self::Empty => "empty",
        }
    }
}

/// 一次后端切换的描述。
#[derive(Debug, Clone)]
pub struct BackendSwitch {
    pub from: String,
    pub to: String,
    pub reason: SwitchReason,
}

/// 链执行结果。
#[derive(Debug, Clone, Default)]
pub struct ChainOutcome {
    /// 累积去重结果数。
    pub total: usize,
    /// `total == 0` 时供调用方生成 Error 文案。
    pub last_error: Option<String>,
}
```

在 `packages/harness/src/lib.rs` 合适位置（其它 `mod` 声明附近）加：

```rust
mod fallback_chain;
pub use fallback_chain::{run_fallback_chain, BackendSwitch, ChainOutcome, SwitchReason};
```

- [ ] **Step 2: 写失败测试（mock + 8 用例）**

在 `fallback_chain.rs` 末尾追加测试模块：

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use locifind_search_backend::{
        backend_stream_from_results, BackendKind, BackendSearchFuture, BackendStream,
        ImplementationStatus, SearchBackend, SearchIntent,
    };
    use crate::{SearchTool, SupportedIntent};

    // 脚本化 mock：决定本后端的流行为。
    #[derive(Clone)]
    enum Script {
        /// 干净产出这些路径。
        Results(Vec<&'static str>),
        /// pre-stream 直接 BackendUnavailable。
        Unavailable,
        /// 先产出这些路径，再以 Io 错误中途崩。
        ResultsThenError(Vec<&'static str>),
    }

    struct MockBackend {
        script: Script,
        called: Arc<AtomicBool>,
    }

    fn result_at(path: &str) -> SearchResult {
        // 用 common crate 的最小构造；若 SearchResult 无 Default，按实际字段补全。
        SearchResult::for_test(PathBuf::from(path))
    }

    impl SearchBackend for MockBackend {
        fn kind(&self) -> BackendKind { BackendKind::Spotlight }
        fn implementation_status(&self) -> ImplementationStatus { ImplementationStatus::Real }
        fn is_available(&self) -> bool { true }
        fn search<'a>(
            &'a self,
            _intent: &'a SearchIntent,
            cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            self.called.store(true, Ordering::SeqCst);
            let script = self.script.clone();
            async move {
                let stream: BackendStream = match script {
                    Script::Results(paths) => backend_stream_from_results(
                        paths.iter().map(|p| result_at(p)).collect(),
                        cancel,
                    ),
                    Script::Unavailable => {
                        return Err(SearchError::BackendUnavailable { reason: "mock".into() })
                    }
                    Script::ResultsThenError(paths) => {
                        let mut items: Vec<Result<SearchResult, SearchError>> =
                            paths.iter().map(|p| Ok(result_at(p))).collect();
                        items.push(Err(SearchError::Io { detail: "mid-stream".into() }));
                        Box::pin(futures_util::stream::iter(items))
                    }
                };
                Ok(stream)
            }
            .boxed()
        }
    }

    fn tool(id: &'static str, script: Script) -> (Arc<dyn SearchableTool>, Arc<AtomicBool>) {
        let called = Arc::new(AtomicBool::new(false));
        let backend = MockBackend { script, called: Arc::clone(&called) };
        let st = SearchTool::new(id, id, backend, vec![SupportedIntent::FileSearch], id);
        (Arc::new(st) as Arc<dyn SearchableTool>, called)
    }

    fn run(
        cands: &[Arc<dyn SearchableTool>],
        cancel: CancellationToken,
    ) -> (Vec<String>, Vec<BackendSwitch>, ChainOutcome) {
        let expanded = ExpandedSearchIntent::identity(SearchIntent::for_test_file_search());
        let mut results = Vec::new();
        let mut switches = Vec::new();
        let outcome = futures_util::executor::block_on(run_fallback_chain(
            cands,
            &expanded,
            cancel,
            &mut |r| results.push(r.path.to_string_lossy().into_owned()),
            &mut |s| switches.push(s),
        ));
        (results, switches, outcome)
    }

    #[test]
    fn single_candidate_success_no_switch() {
        let (a, _) = tool("a", Script::Results(vec!["/x", "/y"]));
        let (results, switches, outcome) = run(&[a], CancellationToken::new());
        assert_eq!(results, vec!["/x", "/y"]);
        assert!(switches.is_empty());
        assert_eq!(outcome.total, 2);
    }

    #[test]
    fn unavailable_switches_to_next() {
        let (a, _) = tool("a", Script::Unavailable);
        let (b, _) = tool("b", Script::Results(vec!["/x"]));
        let (results, switches, outcome) = run(&[a, b], CancellationToken::new());
        assert_eq!(results, vec!["/x"]);
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].reason, SwitchReason::Unavailable);
        assert_eq!(switches[0].from, "a");
        assert_eq!(switches[0].to, "b");
        assert_eq!(outcome.total, 1);
    }

    #[test]
    fn empty_switches_to_next() {
        let (a, _) = tool("a", Script::Results(vec![]));
        let (b, _) = tool("b", Script::Results(vec!["/x"]));
        let (results, switches, outcome) = run(&[a, b], CancellationToken::new());
        assert_eq!(results, vec!["/x"]);
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].reason, SwitchReason::Empty);
        assert_eq!(outcome.total, 1);
    }

    #[test]
    fn midstream_error_keeps_partials_and_switches() {
        let (a, _) = tool("a", Script::ResultsThenError(vec!["/x1", "/x2", "/x3"]));
        let (b, _) = tool("b", Script::Results(vec!["/y1", "/y2"]));
        let (results, switches, outcome) = run(&[a, b], CancellationToken::new());
        assert_eq!(results, vec!["/x1", "/x2", "/x3", "/y1", "/y2"]);
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].reason, SwitchReason::Error);
        assert_eq!(outcome.total, 5);
    }

    #[test]
    fn dedup_across_candidates() {
        // A 出 X 后中途崩（故切 B）；B 出 X+Y → 最终 X(归 A) + Y，B 的 X 去重。
        let (a, _) = tool("a", Script::ResultsThenError(vec!["/x"]));
        let (b, _) = tool("b", Script::Results(vec!["/x", "/y"]));
        let (results, _switches, outcome) = run(&[a, b], CancellationToken::new());
        assert_eq!(results, vec!["/x", "/y"], "B 的重复 /x 不应再次投递");
        assert_eq!(outcome.total, 2);
    }

    #[test]
    fn all_fail_yields_error_outcome() {
        let (a, _) = tool("a", Script::Unavailable);
        let (b, _) = tool("b", Script::Results(vec![]));
        let (results, _switches, outcome) = run(&[a, b], CancellationToken::new());
        assert!(results.is_empty());
        assert_eq!(outcome.total, 0);
        assert!(outcome.last_error.is_some(), "全失败应带 last_error");
    }

    #[test]
    fn success_stops_chain() {
        let (a, a_called) = tool("a", Script::Results(vec!["/x"]));
        let (b, b_called) = tool("b", Script::Results(vec!["/y"]));
        let (results, switches, _outcome) = run(&[a, b], CancellationToken::new());
        assert_eq!(results, vec!["/x"]);
        assert!(switches.is_empty());
        assert!(a_called.load(Ordering::SeqCst));
        assert!(!b_called.load(Ordering::SeqCst), "首个成功后次个不应被调用");
    }

    #[test]
    fn cancel_stops_chain() {
        let (a, _) = tool("a", Script::Unavailable);
        let (b, b_called) = tool("b", Script::Results(vec!["/y"]));
        let cancel = CancellationToken::new();
        cancel.cancel(); // 预取消
        let (results, _switches, outcome) = run(&[a, b], cancel);
        assert!(results.is_empty());
        assert!(!b_called.load(Ordering::SeqCst), "已取消则不再尝试候选");
        assert_eq!(outcome.total, 0);
    }
}
```

> 注：测试用到的 `SearchResult::for_test(path)` / `SearchIntent::for_test_file_search()` / `ExpandedSearchIntent::identity(..)` 若 common crate 无现成构造，本步在 common crate 加 `#[cfg(any(test, feature = "test-util"))]` 的最小构造，或改用 crate 内已有的测试构造 helper（先 `grep -rn "for_test\|fn.*SearchResult.*{" packages/search-backends/common/src` 找现成的，优先复用）。

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test -p locifind-harness fallback_chain 2>&1 | grep -E "test result|error\[" | head`
Expected: 编译失败（`run_fallback_chain` 未定义）。

- [ ] **Step 4: 实现 `run_fallback_chain`**

在 `fallback_chain.rs` 的类型定义之后、测试模块之前插入：

```rust
/// 驱动有序候选链。逐个 `search_expanded`，按 canonical path 去重累积；任一候选失败
/// （pre-stream Err / 流中途 Err / 干净零新结果）即经 `on_switch` 通报并切下一候选；
/// 一旦某候选**干净跑完且贡献 ≥1 条新结果**即成功停链。`on_result` 仅对去重后的新结果调用。
pub async fn run_fallback_chain(
    candidates: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut dyn FnMut(SearchResult),
    on_switch: &mut dyn FnMut(BackendSwitch),
) -> ChainOutcome {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut total = 0usize;
    let mut last_error: Option<String> = None;

    let mut iter = candidates.iter().peekable();
    while let Some(tool) = iter.next() {
        if cancel.is_cancelled() {
            break;
        }
        let from_id = tool.id().to_owned();

        let reason: SwitchReason = match tool.search_expanded(expanded, cancel.clone()).await {
            Err(err) => {
                let reason = match err {
                    SearchError::BackendUnavailable { .. } => SwitchReason::Unavailable,
                    _ => SwitchReason::Error,
                };
                last_error = Some(err.to_string());
                reason
            }
            Ok(mut stream) => {
                let mut this_yield = 0usize;
                let mut stream_err = false;
                while let Some(item) = stream.next().await {
                    if cancel.is_cancelled() {
                        break;
                    }
                    match item {
                        Ok(result) => {
                            if seen.insert(result.path.clone()) {
                                total += 1;
                                this_yield += 1;
                                on_result(result);
                            }
                        }
                        Err(err) => {
                            last_error = Some(err.to_string());
                            stream_err = true;
                            break;
                        }
                    }
                }
                if stream_err {
                    SwitchReason::Error
                } else if this_yield == 0 {
                    SwitchReason::Empty
                } else {
                    // 干净 + ≥1 新结果 → 成功，停链
                    return ChainOutcome { total, last_error: None };
                }
            }
        };

        // 需要切换：若还有下一候选，通报。
        if let Some(next) = iter.peek() {
            on_switch(BackendSwitch {
                from: from_id,
                to: next.id().to_owned(),
                reason,
            });
        }
    }

    ChainOutcome { total, last_error }
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p locifind-harness fallback_chain 2>&1 | grep "test result"`
Expected: `ok. 8 passed`

- [ ] **Step 6: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-harness --check
cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | grep -E "^error|^warning" || echo clean
git add packages/harness/src/fallback_chain.rs packages/harness/src/lib.rs packages/search-backends/common/src/lib.rs
git commit -m "feat(fallback-chain): run_fallback_chain 编排核心 + 8 mock 单测"
```

---

### Task 3: desktop 接入 + `BackendSwitched` 事件 + 前端最小处理

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（加 SearchEvent 变体 + 替换 dispatch 段）
- Modify: `apps/desktop/src/SearchView.tsx`（TS 类型 + 最小处理）

- [ ] **Step 1: 加 `BackendSwitched` SearchEvent 变体**

在 `search.rs` 的 `pub enum SearchEvent { ... }` 内（`Error { message }` 之后）加：

```rust
    /// 主后端失败，已切到下一候选后端。
    BackendSwitched { from: String, to: String, reason: String },
```

- [ ] **Step 2: 替换 dispatch 段为 chain 驱动**

把 `search.rs` 现有 `route_search_expanded` → 单 tool → `tool.search_expanded` → stream 循环这一整段（约 226–333 行，自 `let router = IntentRouter::new(...)` 起到 `on_tool_result` 止）替换为：

```rust
    let router = IntentRouter::new(&deps.registry);
    let candidates = match router.route_search_chain(&expanded) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("search: 无可用 tool: {err}");
            let _ = on_event.send(SearchEvent::Error { message: err.to_string() });
            return Ok(());
        }
    };

    let first_id = candidates[0].id().to_owned();
    let tool_start = Instant::now();

    deps.tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: first_id.clone(),
        tool_kind: locifind_harness::ToolKind::Search,
        intent_variant: locifind_harness::SupportedIntent::from_intent(&effective),
    });
    let _ = on_event.send(SearchEvent::Started {
        intent_summary: describe_intent(&effective),
        fallback_used: matches!(source, IntentSource::Model),
        signals: signals_to_labels(&signals),
        tool_id: first_id.clone(),
    });

    // SynonymExpandEvent 段保持原样（沿用现有 6c 之前那段循环，不改）。

    let cancel = CancellationToken::new();
    let mut recorded: Vec<locifind_search_backend::SearchResult> = Vec::new();
    let mut send_failed = false;
    let outcome = {
        let on_event_ref = &on_event;
        let recorded_ref = &mut recorded;
        let send_failed_ref = &mut send_failed;
        let tracer = &deps.tracer;
        locifind_harness::run_fallback_chain(
            &candidates,
            &expanded,
            cancel,
            &mut |result| {
                let json = SearchResultJson {
                    id: result.id.clone(),
                    path: result.path.to_string_lossy().into_owned(),
                    name: result.name.clone(),
                    source: format!("{:?}", result.source).to_lowercase(),
                    match_type: format!("{:?}", result.match_type).to_lowercase(),
                    score: result.score,
                    modified_time: result.metadata.modified_time.map(|t| t.to_rfc3339()),
                    size_bytes: result.metadata.size_bytes,
                };
                recorded_ref.push(result);
                if on_event_ref.send(SearchEvent::Result { item: json }).is_err() {
                    *send_failed_ref = true;
                }
            },
            &mut |sw| {
                tracer.on_error(&locifind_harness::ToolErrorEvent {
                    tool_id: sw.from.clone(),
                    duration: tool_start.elapsed(),
                    error_type: format!("fallback_switch:{}", sw.reason.as_str()),
                });
                let _ = on_event_ref.send(SearchEvent::BackendSwitched {
                    from: sw.from,
                    to: sw.to,
                    reason: sw.reason.as_str().to_owned(),
                });
            },
        )
        .await
    };

    deps.tracer.on_tool_result(&locifind_harness::ToolResultEvent {
        tool_id: first_id.clone(),
        duration: tool_start.elapsed(),
        result_count: outcome.total,
    });

    if outcome.total == 0 {
        let _ = on_event.send(SearchEvent::Error {
            message: outcome.last_error.unwrap_or_else(|| "未找到结果".to_owned()),
        });
        return Ok(());
    }
    let _ = send_failed; // UI 通道断开仅停止后续投递，不改终止语义
```

> `SwitchReason` 需在 search.rs 引入：把顶部 `use locifind_harness::{...}` 加上 `SwitchReason`（`as_str` 是其方法，引入类型即可）。`run_fallback_chain` 同理（已 `pub use`）。
>
> 替换后，原先 `record` 本轮结果 + `Complete` 事件那段（337–344 行）保持在此段之后不变（`guard.record(expanded.base, recorded)` + `Complete { total: outcome.total, elapsed_ms }`）——注意把 `Complete` 的 `total` 改用 `outcome.total`。

- [ ] **Step 3: 前端 TS 类型 + 最小处理**

在 `SearchView.tsx` 的 SearchEvent 联合类型加分支，并在事件处理 switch 里加 `BackendSwitched` 分支（最小：console 或一行提示，不崩即可）：

```tsx
// 类型联合中加：
  | { event: "backendSwitched"; data: { from: string; to: string; reason: string } }

// 事件处理 switch 中加：
  case "backendSwitched":
    // 最小处理：UI 暂仅记录，避免未知变体导致渲染分支缺失
    console.debug(`backend switched ${ev.data.from} → ${ev.data.to} (${ev.data.reason})`);
    break;
```

> 实际 tag 名以 Rust `SearchEvent` serde 重命名规则为准（现有变体在 TS 里的 tag 形如 `started`/`result`/`complete`/`error` → 对应 `backendSwitched` 或 `backend_switched`；按现有变体命名风格对齐，grep `SearchView.tsx` 看现有 case 拼写）。

- [ ] **Step 4: 编译 + 既有测试**

Run:
```bash
cargo build -p locifind-desktop 2>&1 | tail -3
cargo test -p locifind-desktop 2>&1 | grep "test result"
cd apps/desktop && npx tsc --noEmit 2>&1 | tail -5 ; cd ../..
```
Expected: desktop 编译通过；既有 Rust 测试全过；tsc 无新错误。

- [ ] **Step 5: fmt + clippy + 提交**

```bash
cargo fmt -p locifind-desktop --check
cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | grep -E "^error|^warning" || echo clean
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src/SearchView.tsx
git commit -m "feat(fallback-chain): desktop 接入 run_fallback_chain + BackendSwitched 事件"
```

---

### Task 4: 回归门 + 收工同步

**Files:**
- Modify: `STATUS.md`、`ROADMAP.md`

- [ ] **Step 1: 全回归门**

Run:
```bash
cargo build --release -p locifind-evals --bin evals
./target/release/evals --fixtures v0.5 2>&1 | grep -E "pass:|partial:|fail:"
cargo test -p locifind-harness 2>&1 | grep "test result" | tail -3
cargo test -p locifind-desktop 2>&1 | grep "test result" | tail -2
bash scripts/ci.sh 2>&1 | tail -5
```
Expected: evals parser-only **472/26/2**；harness + desktop 测试全过；ci.sh 全绿。

- [ ] **Step 2: STATUS / ROADMAP 同步**

- ROADMAP Class B「真 fallback chain mid-stream retry」从 backlog 改为 **partial — Mac 编排核心 + mock 单测 done，真双后端集成留 Windows**，附报告/spec 链接。
- STATUS 顶部加 blockquote + 当前 Task 更新 + 「下一步」加「fallback chain Windows 真双后端集成验证」+ 会话日志顶部追加本会话条目（署名 `Claude Code (Opus 4.8)`）。

- [ ] **Step 3: 收工 commit**

```bash
git add STATUS.md ROADMAP.md
git commit -m "收工: fallback chain mid-stream retry（Mac 编排核心 + mock 单测 done，真集成留 Windows）"
```

---

## 自审记录（writing-plans self-review）

- **Spec 覆盖**：§3.1 类型+run_fallback_chain→Task2；§3.2 route_search_chain→Task1；§3.3 desktop 适配→Task3；§3.4 dedup→Task2 实现 + test `dedup_across_candidates`；§4 失败语义表→Task2 实现分支 + tests 2/3/4/7；§5 BackendSwitched→Task3 Step1/2 + 前端 Step3；§6.1 八单测→Task2 Step2（test 5 dedup 已修正为「A 出 X 后崩→B」以绕开成功即停）；§6.2 回归门→Task4 Step1 + 各 task fmt/clippy；§6.3 Mac/Windows→Task4 Step2 文档。全覆盖。
- **占位符**：测试用的 `SearchResult::for_test` / `SearchIntent::for_test_file_search` / `ExpandedSearchIntent::identity` 标注「先 grep 复用现成构造，无则加最小 test-util」——非占位，是明确的复用优先指令。前端 tag 拼写标注「以现有变体 serde 命名为准」——明确对齐指令。
- **类型一致**：`SwitchReason`/`BackendSwitch`/`ChainOutcome`/`run_fallback_chain` 在 Task2 定义，Task3 消费签名一致（`as_str()` / `.total` / `.last_error` / `.from/.to/.reason`）；`route_search_chain` 返回 `Vec<Arc<dyn SearchableTool>>`，Task3 按 `candidates[0]` + 传 `&candidates` 消费一致。
- **执行顺序**：Task1（route_search_chain）独立；Task2（chain 核心）独立；Task3 依赖 Task1+Task2；Task4 收尾。
</content>
