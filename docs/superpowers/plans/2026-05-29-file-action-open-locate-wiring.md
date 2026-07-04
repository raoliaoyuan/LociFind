# FileAction(open/locate)多轮接入 Tauri search command — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户在一次搜索后用「打开第 2 个」/「在访达里显示第 3 个」对上一轮结果执行 open/locate,经 Tauri `search` command 内联分支接入 `FileActionTool`。

**Architecture:** 沿用第 30 阶段「`search_impl` 内联分支」方案。query 在 Rust 端 parse 出 `effective` intent 后,若是 `FileAction(Open/Locate)` 则交给新增 `handle_file_action`(调 harness `FileActionTool::invoke`,只读 `ContextMemory`),否则走现有 search 路径。copy/move/rename/delete 在 invoke **之前**按动作类型硬拦,绝不执行。

**Tech Stack:** Rust(Tauri 2 command + harness)、TypeScript(React `SearchView.tsx`)、tokio test。

**Spec:** `docs/superpowers/specs/2026-05-29-file-action-open-locate-wiring-design.md`

---

## 文件结构

- **Modify** `apps/desktop/src-tauri/src/search.rs` — 加 `SearchEvent::ActionDone` 变体、`file_action_error_kind`、`friendly_file_action_message`、`handle_file_action`、`MockFileActionExecutor`、FileAction 分支、单元 + 集成测试。
- **Modify** `apps/desktop/src-tauri/src/main.rs` — `.manage(Arc<FileActionTool>)`。
- **Modify** `apps/desktop/src/SearchView.tsx` — `SearchEvent` / `Status` 加 `action_done` + 渲染分支。

## 关键既有 API(实现者无需重新发现)

- `locifind_harness::file_action_tool::{FileActionTool, LocalFileActionExecutor, FileActionExecutor, FileActionOutcome, FileActionError}`
- `FileActionTool::new(executor: Arc<dyn FileActionExecutor>, policy: PolicyEngine) -> Self`
- `FileActionTool::invoke(&self, action: &FileAction, context: &ContextMemory) -> Result<FileActionOutcome, FileActionError>`
- `FileActionOutcome::{Executed { affected: Vec<PathBuf> }, RequiresConfirmation { paths: Vec<PathBuf> }}`
- `FileActionError::{DeleteNotSupported, PolicyDenied{reason}, TargetRef(TargetRefError), EmptyTargets, BatchThresholdExceeded{count,threshold}, MissingDestination, MissingNewName, PathConflict{dest}, Executor(io::Error)}`
- `locifind_harness::context::TargetRefError::{NoLastResults, IndexOutOfRange{requested,available}, EmptyIndices}`
- `FileActionExecutor` trait 方法:`open(&Path)`, `locate(&Path)`, `copy(&Path,&Path)`, `move_to(&Path,&Path)`, `rename(&Path,&str)`,均返回 `io::Result<()>`。
- `locifind_search_backend::{FileAction, FileActionKind}`;`FileAction.action: FileActionKind`,`FileActionKind::{Open,Locate,Copy,Move,Rename,Delete}`。
- `locifind_harness::{ToolKind, SupportedIntent, ToolCallEvent, ToolResultEvent, ToolErrorEvent, Tracer}`(search.rs 已 import 其中部分)。

---

## Task 1: ActionDone 事件 + 纯函数 helper(error_kind + friendly_message)

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

加一个 `SearchEvent` 变体和两个纯函数(无需 search_impl 改签名,先独立 TDD)。

- [ ] **Step 1: 加 `ActionDone` 变体**

在 `search.rs` 的 `SearchEvent` enum(`Error` 变体之后)加:

```rust
    /// 文件操作(open/locate)执行完成。
    ActionDone {
        /// 动作类型:"open" | "locate"。
        action_kind: String,
        /// 实际涉及的绝对路径。
        paths: Vec<String>,
    },
```

- [ ] **Step 2: 写 helper 的失败测试**

在 `search.rs` 的 `#[cfg(test)] mod tests` 末尾(`search_error_kind_maps_all_variants` 之后)加:

```rust
    use locifind_harness::context::TargetRefError;
    use locifind_harness::file_action_tool::FileActionError;

    #[test]
    fn file_action_error_kind_maps_variants() {
        assert_eq!(
            file_action_error_kind(&FileActionError::TargetRef(TargetRefError::NoLastResults)),
            "NoLastResults"
        );
        assert_eq!(
            file_action_error_kind(&FileActionError::TargetRef(
                TargetRefError::IndexOutOfRange { requested: 9, available: 2 }
            )),
            "IndexOutOfRange"
        );
        assert_eq!(
            file_action_error_kind(&FileActionError::EmptyTargets),
            "EmptyTargets"
        );
    }

    #[test]
    fn friendly_message_for_common_errors() {
        let no_last = FileActionError::TargetRef(TargetRefError::NoLastResults);
        assert!(friendly_file_action_message(&no_last).contains("请先发起一次搜索"));

        let oob = FileActionError::TargetRef(TargetRefError::IndexOutOfRange {
            requested: 9,
            available: 2,
        });
        let msg = friendly_file_action_message(&oob);
        assert!(msg.contains('9') && msg.contains('2'), "实得: {msg}");
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p locifind-desktop file_action_error_kind 2>&1 | tail -5`
Expected: 编译失败 `cannot find function file_action_error_kind`。

- [ ] **Step 4: 实现两个 helper**

在 `search.rs` 的 `search_error_kind` 函数之后加(非测试代码):

```rust
/// 返回 FileActionError variant 名,不含 detail(避免泄路径)。供 trace 用。
#[cfg_attr(not(test), allow(dead_code))]
fn file_action_error_kind(err: &locifind_harness::file_action_tool::FileActionError) -> &'static str {
    use locifind_harness::context::TargetRefError;
    use locifind_harness::file_action_tool::FileActionError;
    match err {
        FileActionError::DeleteNotSupported => "DeleteNotSupported",
        FileActionError::PolicyDenied { .. } => "PolicyDenied",
        FileActionError::TargetRef(TargetRefError::NoLastResults) => "NoLastResults",
        FileActionError::TargetRef(TargetRefError::IndexOutOfRange { .. }) => "IndexOutOfRange",
        FileActionError::TargetRef(TargetRefError::EmptyIndices) => "EmptyIndices",
        FileActionError::EmptyTargets => "EmptyTargets",
        FileActionError::BatchThresholdExceeded { .. } => "BatchThresholdExceeded",
        FileActionError::MissingDestination => "MissingDestination",
        FileActionError::MissingNewName => "MissingNewName",
        FileActionError::PathConflict { .. } => "PathConflict",
        FileActionError::Executor(_) => "Executor",
    }
}

/// 把 FileActionError 映射成面向用户的中文友好文案。
#[cfg_attr(not(test), allow(dead_code))]
fn friendly_file_action_message(
    err: &locifind_harness::file_action_tool::FileActionError,
) -> String {
    use locifind_harness::context::TargetRefError;
    use locifind_harness::file_action_tool::FileActionError;
    match err {
        FileActionError::TargetRef(TargetRefError::NoLastResults) => {
            "没有可操作的上一轮搜索结果,请先发起一次搜索".to_owned()
        }
        FileActionError::TargetRef(TargetRefError::IndexOutOfRange {
            requested,
            available,
        }) => format!("第 {requested} 个结果不存在(上一轮共 {available} 条)"),
        FileActionError::TargetRef(TargetRefError::EmptyIndices) => {
            "未指定要操作的结果序号".to_owned()
        }
        FileActionError::EmptyTargets => "没有可操作的目标".to_owned(),
        FileActionError::Executor(io) => format!("操作失败:{io}"),
        // open/locate 路径不可达的错误,兜底用 Display
        other => other.to_string(),
    }
}
```

> 注:`#[cfg_attr(not(test), allow(dead_code))]` 暂存,Task 3 接线后这两个函数被 `handle_file_action` 调用,删除该属性(见 Task 3 Step 末)。`ActionDone` 变体一旦 Task 3 用到即非 dead;但 enum 变体即便暂时未构造也不会触发 dead_code 警告(serde derive 视为已用),无需属性。

- [ ] **Step 5: 跑测试确认通过 + clippy**

Run: `cargo test -p locifind-desktop file_action_error_kind friendly_message 2>&1 | tail -8`
Expected: 2 test PASS。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告(`allow(dead_code)` 已压住 binary target 的未用告警)。

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: SearchEvent::ActionDone + file_action error_kind/friendly_message helper(Task1)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: handle_file_action + MockFileActionExecutor + 单元测试

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

实现 `handle_file_action`(暂不接入 search_impl,Task 3 接线),用 MockExecutor 直接单测。

- [ ] **Step 1: 加 MockFileActionExecutor 到 test mod**

在 `search.rs` test mod 内(`FakeCapturingBackend` 之后)加:

```rust
    use locifind_harness::file_action_tool::{FileActionExecutor, FileActionTool};
    use std::io;

    /// 记录每次文件操作调用("open:/path" / "locate:/path" 等)。
    #[derive(Debug, Default)]
    struct MockFileActionExecutor {
        calls: Arc<Mutex<Vec<String>>>,
    }
    impl MockFileActionExecutor {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            (Self { calls: Arc::clone(&calls) }, calls)
        }
    }
    impl FileActionExecutor for MockFileActionExecutor {
        fn open(&self, path: &std::path::Path) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("open:{}", path.display()));
            Ok(())
        }
        fn locate(&self, path: &std::path::Path) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("locate:{}", path.display()));
            Ok(())
        }
        fn copy(&self, src: &std::path::Path, _dest: &std::path::Path) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("copy:{}", src.display()));
            Ok(())
        }
        fn move_to(&self, src: &std::path::Path, _dest: &std::path::Path) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("move:{}", src.display()));
            Ok(())
        }
        fn rename(&self, src: &std::path::Path, _new_name: &str) -> io::Result<()> {
            self.calls.lock().unwrap().push(format!("rename:{}", src.display()));
            Ok(())
        }
    }

    /// 用 MockExecutor 建一个默认 Policy 的 FileActionTool。
    fn build_file_action_tool() -> (Arc<FileActionTool>, Arc<Mutex<Vec<String>>>) {
        let (exec, calls) = MockFileActionExecutor::new();
        let tool = FileActionTool::new(Arc::new(exec), PolicyEngine::new());
        (Arc::new(tool), calls)
    }

    /// 建一个含 N 条结果的 ContextMemory(intent 为 FileSearch{pdf})。
    fn context_with_results(n: usize) -> Arc<Mutex<ContextMemory>> {
        use locifind_search_backend::{BackendKind, MatchType, SearchResult, SearchResultMetadata};
        let results: Vec<SearchResult> = (0..n)
            .map(|i| SearchResult {
                id: format!("id-{i}"),
                path: PathBuf::from(format!("/tmp/f{i}")),
                name: format!("f{i}"),
                source: BackendKind::Spotlight,
                match_type: MatchType::Filename,
                score: None,
                metadata: SearchResultMetadata::default(),
            })
            .collect();
        let mut ctx = ContextMemory::new();
        ctx.record(mk_base_file_search_pdf(), results);
        Arc::new(Mutex::new(ctx))
    }

    /// 构造一个 FileAction intent。
    fn mk_file_action(kind: locifind_search_backend::FileActionKind, idx: u32) -> locifind_search_backend::FileAction {
        use locifind_search_backend::{FileAction, SchemaVersion, TargetRef, TargetSelector, Language};
        FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: kind,
            target_ref: TargetRef::LastResults { selector: TargetSelector::Index { value: idx } },
            destination: None,
            new_name: None,
            // parser 对 copy/move/rename 会设 true;这里统一 true 以测 scope gate 是否在
            // invoke 之前拦住(若没拦,invoke 会因 requires_confirmation=true 直接执行)。
            requires_confirmation: true,
        }
    }
```

- [ ] **Step 2: 写 handle_file_action 的失败单测**

在 test mod 末尾加 5 个单测:

```rust
    #[tokio::test]
    async fn handle_file_action_open_executes() {
        use locifind_search_backend::FileActionKind;
        let (tool, calls) = build_file_action_tool();
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = context_with_results(2);
        let (ch, events) = capture_channel();

        handle_file_action(mk_file_action(FileActionKind::Open, 1), ch, tool, tracer, ctx)
            .await
            .unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1, "应 1 次 open, 实得 {calls:?}");
        assert_eq!(calls[0], "open:/tmp/f0", "应打开第 1 个(0-based f0)");
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"action_done\"")), "实得 {events:?}");
        assert!(events.iter().any(|e| e.contains("open")), "action_kind 应含 open");
    }

    #[tokio::test]
    async fn handle_file_action_locate_executes() {
        use locifind_search_backend::FileActionKind;
        let (tool, calls) = build_file_action_tool();
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = context_with_results(3);
        let (ch, events) = capture_channel();

        handle_file_action(mk_file_action(FileActionKind::Locate, 2), ch, tool, tracer, ctx)
            .await
            .unwrap();

        let calls = calls.lock().unwrap();
        assert_eq!(calls[0], "locate:/tmp/f1", "应 locate 第 2 个(f1)");
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"action_done\"") && e.contains("locate")));
    }

    #[tokio::test]
    async fn handle_file_action_copy_blocked_not_executed() {
        use locifind_search_backend::FileActionKind;
        let (tool, calls) = build_file_action_tool();
        let (tracer, trace_calls) = build_tracer_with_mock();
        let ctx = context_with_results(2);
        let (ch, events) = capture_channel();

        handle_file_action(mk_file_action(FileActionKind::Copy, 1), ch, tool, tracer, ctx)
            .await
            .unwrap();

        assert!(calls.lock().unwrap().is_empty(), "copy 不应进 executor(scope gate)");
        assert!(trace_calls.lock().unwrap().is_empty(), "scope gate 是 pre-tool, 不进 trace");
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"error\"") && e.contains("暂不支持")));
    }

    #[tokio::test]
    async fn handle_file_action_index_out_of_range_errors() {
        use locifind_search_backend::FileActionKind;
        let (tool, _calls) = build_file_action_tool();
        let (tracer, trace_calls) = build_tracer_with_mock();
        let ctx = context_with_results(2);
        let (ch, events) = capture_channel();

        handle_file_action(mk_file_action(FileActionKind::Open, 9), ch, tool, tracer, ctx)
            .await
            .unwrap();

        let events = events.lock().unwrap();
        assert!(
            events.iter().any(|e| e.contains("\"error\"") && e.contains('9') && e.contains('2')),
            "应越界友好错误, 实得 {events:?}"
        );
        // 可路由的 open → 进 trace(call + error)
        let tc = trace_calls.lock().unwrap();
        assert!(tc.iter().any(|c| c.starts_with("call:")), "实得 {tc:?}");
        assert!(tc.iter().any(|c| c.starts_with("error:")), "实得 {tc:?}");
    }

    #[tokio::test]
    async fn handle_file_action_no_context_errors() {
        use locifind_search_backend::FileActionKind;
        let (tool, _calls) = build_file_action_tool();
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let (ch, events) = capture_channel();

        handle_file_action(mk_file_action(FileActionKind::Open, 1), ch, tool, tracer, ctx)
            .await
            .unwrap();

        let events = events.lock().unwrap();
        assert!(
            events.iter().any(|e| e.contains("\"error\"") && e.contains("请先发起一次搜索")),
            "实得 {events:?}"
        );
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p locifind-desktop handle_file_action 2>&1 | tail -5`
Expected: 编译失败 `cannot find function handle_file_action`。

- [ ] **Step 4: 实现 handle_file_action**

在 `search.rs` 的 `apply_refine_if_needed` 之后加(非测试代码):

```rust
/// 处理 `FileAction(Open/Locate)`:只读 [`ContextMemory`] 解析 target_ref,经
/// [`FileActionTool`] 执行,结果走 [`SearchEvent::ActionDone`]。
///
/// **安全 gate**:仅 `Open`/`Locate` 放行;`Copy`/`Move`/`Rename`/`Delete` 一律
/// 转 `Error`,**绝不进 invoke**(parser 对前者预设 `requires_confirmation=true`,
/// 否则会绕过尚未实现的确认流直接执行)。
///
/// 不 `record` / `clear` context —— action 无搜索结果,只读保证连续 action
/// 引用同一搜索基准。
async fn handle_file_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
    tracer: Arc<locifind_harness::Tracer>,
    context: Arc<Mutex<ContextMemory>>,
) -> Result<(), String> {
    use locifind_search_backend::FileActionKind;

    // 1) scope gate:仅 open/locate(pre-tool,不进 trace)
    match action.action {
        FileActionKind::Open | FileActionKind::Locate => {}
        other => {
            eprintln!("search: file_action 暂不支持: {other:?}");
            let _ = on_event.send(SearchEvent::Error {
                message: "该操作暂不支持(确认流待后续阶段)".to_owned(),
            });
            return Ok(());
        }
    }

    let tool_id = file_action_tool.id().to_owned();
    let tool_start = Instant::now();

    // 2) tool_call trace
    tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: tool_id.clone(),
        tool_kind: locifind_harness::ToolKind::FileAction,
        intent_variant: locifind_harness::SupportedIntent::FileAction,
    });

    // 3) invoke(只读 context guard)
    let outcome = {
        let guard = context.lock().unwrap_or_else(|e| e.into_inner());
        file_action_tool.invoke(&action, &guard)
    };

    match outcome {
        Ok(locifind_harness::file_action_tool::FileActionOutcome::Executed { affected }) => {
            tracer.on_tool_result(&locifind_harness::ToolResultEvent {
                tool_id,
                duration: tool_start.elapsed(),
                result_count: affected.len(),
            });
            let action_kind = format!("{:?}", action.action).to_lowercase();
            let paths: Vec<String> = affected
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect();
            let _ = on_event.send(SearchEvent::ActionDone { action_kind, paths });
            Ok(())
        }
        Ok(locifind_harness::file_action_tool::FileActionOutcome::RequiresConfirmation { .. }) => {
            // open/locate 为 Allow,理论不可达;防御性转 Error,不静默执行。
            eprintln!("search: open/locate 不应触发 RequiresConfirmation");
            let _ = on_event.send(SearchEvent::Error {
                message: "该操作暂不支持(确认流待后续阶段)".to_owned(),
            });
            Ok(())
        }
        Err(err) => {
            tracer.on_error(&locifind_harness::ToolErrorEvent {
                tool_id,
                duration: tool_start.elapsed(),
                error_type: file_action_error_kind(&err).to_owned(),
            });
            let _ = on_event.send(SearchEvent::Error {
                message: friendly_file_action_message(&err),
            });
            Ok(())
        }
    }
}
```

> `Tool::id` 在 `FileActionTool` 上实现(返回 `"file_action.local"`);需 `use locifind_harness::Tool;` 若 search.rs 尚未引入。检查文件顶部 use,若缺则在 Step 4 一并加 `use locifind_harness::Tool;`(或用全限定 `locifind_harness::Tool::id(&*file_action_tool)`)。`handle_file_action` 当前仅被测试调用 → 加 `#[cfg_attr(not(test), allow(dead_code))]` 于函数上,Task 3 接线后移除。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p locifind-desktop handle_file_action 2>&1 | tail -10`
Expected: 5 test PASS。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告。

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: handle_file_action(open/locate)+ MockFileActionExecutor + 5 单测(Task2)

scope gate 在 invoke 之前按动作类型拦 copy/move/rename;只读 context 不 record/clear。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: 接线 search_impl + 集成测试

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

给 `search` / `search_impl` 加 `file_action_tool` 参数,在 `effective` 计算后分支到 `handle_file_action`;更新现有测试调用点;移除 dead_code 属性;加集成测试。

- [ ] **Step 1: 给 search command 加 State 参数**

修改 `#[tauri::command] pub async fn search`,在 `context` 参数后加:

```rust
    file_action_tool: tauri::State<'_, Arc<locifind_harness::file_action_tool::FileActionTool>>,
```

并在 `search_impl(...)` 调用里追加 `Arc::clone(&*file_action_tool)` 作为最后一个实参。

- [ ] **Step 2: 给 search_impl 加参数 + 分支**

修改 `pub(crate) async fn search_impl` 签名,在 `context: Arc<Mutex<ContextMemory>>` 后加:

```rust
    file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
```

在 `effective` 解析完成后(`// 3) Policy gate` 注释**之前**)插入分支:

```rust
    // 2.5) FileAction(open/locate)分支:交给 handle_file_action(自带 Policy + 只读 context)。
    //      其余 intent 落到下方现有 search 路径。
    if let SearchIntent::FileAction(action) = &effective {
        return handle_file_action(
            action.clone(),
            on_event,
            file_action_tool,
            tracer,
            context,
        )
        .await;
    }
```

> `on_event` / `tracer` / `context` 在该 diverging 分支被 move;因分支 `return`,其后的 search 路径仍可用这些变量(Rust 允许 move-in-diverging-branch)。

- [ ] **Step 3: 移除两个 helper + handle_file_action 上的 dead_code 属性**

删除 `file_action_error_kind`、`friendly_file_action_message`、`handle_file_action` 三处的 `#[cfg_attr(not(test), allow(dead_code))]`(现在它们被 search_impl 间接调用,非 dead)。

- [ ] **Step 4: 更新所有现有 search_impl 测试调用点**

现有 6 处 `search_impl(...)` 调用(`search_impl_success_emits_call_then_result` 等)缺新参数。在每处调用的最后一个实参后追加 `build_file_action_tool().0`。

例(改 `search_impl_success_emits_call_then_result`):

```rust
        search_impl(
            QUERY_FOR_FILE_SEARCH.into(),
            ch,
            registry,
            policy,
            tracer,
            empty_context(),
            build_file_action_tool().0,
        )
        .await
        .unwrap();
```

对以下测试同样追加最后一个参数 `build_file_action_tool().0`:
`search_impl_success_emits_call_then_result`、`search_impl_open_err_emits_call_then_error`、`search_impl_mid_stream_err_emits_call_then_error`、`search_impl_pre_tool_failure_emits_no_trace`、`search_impl_record_then_refine_merges_base`(2 处调用)、`search_impl_refine_without_context_errors`、`search_impl_chained_refine_accumulates`(循环内 1 处)。

> `build_file_action_tool()` 返回 `(Arc<FileActionTool>, Arc<Mutex<Vec<String>>>)`;`.0` 取 tool。链式 refine 测试在 `for` 循环里调用,直接在循环体内 `build_file_action_tool().0`(每轮新建,互不干扰)。

- [ ] **Step 5: 加集成测试(record-then-action + context 不被动)**

在 test mod 末尾加:

```rust
    /// "打开第1个" 稳定解析为 FileAction(Open, LastResults{Index:1})。
    const QUERY_OPEN_FIRST: &str = "打开第1个";
    /// "打开第2个" → Index:2。
    const QUERY_OPEN_SECOND: &str = "打开第2个";

    #[tokio::test]
    async fn search_then_open_first_executes_on_last_results() {
        // 用同一个 executor 句柄跨两轮 search_impl,验证 open 命中上一轮 results[0]。
        let registry = build_test_registry(
            FakeOkBackend(2),
            vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
        );
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let (tool, calls) = build_file_action_tool();

        // 第一轮:find pdf → 记录 2 条(f0, f1)
        let (ch1, _c1) = capture_channel();
        search_impl(
            QUERY_FOR_FILE_SEARCH.into(),
            ch1,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
        )
        .await
        .unwrap();

        // 第二轮:打开第1个 → executor.open(f0)
        let (ch2, _c2) = capture_channel();
        search_impl(
            QUERY_OPEN_FIRST.into(),
            ch2,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
        )
        .await
        .unwrap();

        let calls_snapshot = calls.lock().unwrap().clone();
        assert_eq!(calls_snapshot, vec!["open:/tmp/f0".to_owned()], "应打开上一轮第 1 个");
    }

    #[tokio::test]
    async fn action_does_not_clobber_context() {
        // 搜索 → 打开第1个 → 打开第2个;两次 action 都应命中同一搜索基准。
        let registry = build_test_registry(
            FakeOkBackend(2),
            vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
        );
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let (tool, calls) = build_file_action_tool();

        for q in [QUERY_FOR_FILE_SEARCH, QUERY_OPEN_FIRST, QUERY_OPEN_SECOND] {
            let (ch, _c) = capture_channel();
            search_impl(
                q.into(),
                ch,
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&tracer),
                Arc::clone(&ctx),
                Arc::clone(&tool),
            )
            .await
            .unwrap();
        }

        let calls_snapshot = calls.lock().unwrap().clone();
        assert_eq!(
            calls_snapshot,
            vec!["open:/tmp/f0".to_owned(), "open:/tmp/f1".to_owned()],
            "第二次 action 仍应命中同一搜索(context 未被 record/clear)"
        );
    }
```

- [ ] **Step 6: 跑全量 desktop 测试 + clippy**

Run: `cargo test -p locifind-desktop 2>&1 | tail -15`
Expected: 全部 PASS(原 15 + Task1 的 2 + Task2 的 5 + 本 Task 2 = 24 test)。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告。

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: search_impl 接 FileAction(open/locate)分支 + 2 集成测试(Task3)

effective 计算后分支到 handle_file_action;移除 dead_code 暂存属性。
集成测试验证 record-then-open 命中上一轮 + 连续 action 不 clobber context。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: main.rs 注入 FileActionTool State

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: 加 import + 构造 + manage**

在 `main.rs` 顶部 use 区加:

```rust
use locifind_harness::file_action_tool::{FileActionTool, LocalFileActionExecutor};
```

在 `fn main()` 内,`let context = ...` 之后加:

```rust
    let file_action_tool = Arc::new(FileActionTool::new(
        Arc::new(LocalFileActionExecutor),
        PolicyEngine::new(),
    ));
```

在 builder 链 `.manage(context)` 之后加:

```rust
        .manage(file_action_tool)
```

- [ ] **Step 2: build + clippy**

Run: `cargo build -p locifind-desktop 2>&1 | tail -8`
Expected: 编译通过(`search` command 的 `file_action_tool: State<...>` 现有对应 managed state)。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告。

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/main.rs
git commit -m "desktop: main.rs 注入 Arc<FileActionTool> State(Task4)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: SearchView.tsx action_done 渲染

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`

- [ ] **Step 1: 扩 SearchEvent 类型**

在 `type SearchEvent =` 联合(`| { event: "error"; message: string };` 之前)加:

```typescript
  | { event: "action_done"; action_kind: string; paths: string[] }
```

- [ ] **Step 2: 扩 Status 类型**

在 `type Status =` 联合(`| { kind: "error"; message: string };` 之前)加:

```typescript
  | { kind: "action_done"; action_kind: string; paths: string[] }
```

- [ ] **Step 3: switch 加 action_done 分支**

在 `onEvent.onmessage` 的 `switch (msg.event)` 内,`case "error":` 之前加:

```typescript
        case "action_done": {
          setStatus({
            kind: "action_done",
            action_kind: msg.action_kind,
            paths: msg.paths,
          });
          break;
        }
```

- [ ] **Step 4: 渲染 action_done 态**

在 `{status.kind === "error" && (...)}` 块之前加:

```tsx
      {status.kind === "action_done" && (
        <div className="search-status action-done">
          {describeAction(status.action_kind, status.paths)}
        </div>
      )}
```

并在文件末尾(组件函数外)加 helper:

```tsx
function basename(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

function describeAction(kind: string, paths: string[]): string {
  const verb = kind === "locate" ? "已在访达中显示" : "已打开";
  if (paths.length === 1) {
    return `${verb} ${basename(paths[0])}`;
  }
  return `${verb} ${paths.length} 个文件`;
}
```

- [ ] **Step 5: TS 类型检查 + build**

Run: `cd apps/desktop && npm run build 2>&1 | tail -10`
Expected: tsc + vite build 通过,无类型错误。

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "desktop UI: SearchEvent.action_done 渲染 open/locate 反馈(Task5)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: 全量 CI + evals 不回归确认

**Files:** 无改动,纯验证。

- [ ] **Step 1: 全量 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -20`
Expected: fmt + clippy(-D warnings)+ build + test 全过。

- [ ] **Step 2: evals byte-equal 确认(parser/harness 源未动)**

Run: `cargo run -q -p locifind-evals --bin evals 2>&1 | tail -6`
Expected: parser-only **pass 472 / partial 26 / fail 2**(与第 30 阶段一致;本阶段只动 desktop crate,evals 不依赖 desktop)。

- [ ] **Step 3: 无需 commit(验证 task)**

若 Step 1/2 出现非预期变化,回到对应 Task 修复。

---

## 真机手测(用户驱动,收尾会话执行)

agent 无法点 Tauri 窗口。用户运行 `cd apps/desktop && LOCIFIND_TRACE=/tmp/locifind-trace-fileaction.jsonl npm run tauri dev`,agent 盯 trace + 核对 UI:

1. `find pdf` → 有结果 → `打开第1个` → 真打开应用 + UI「已打开 ...」+ trace `call:file_action.local` / `result:...:1`。
2. `在访达里显示第2个` → 真跳访达 + UI「已在访达中显示 ...」。
3. `打开第99个` → UI 越界友好错误(「第 99 个结果不存在...」)+ trace call+error。
4. 重启后首查 `打开第1个`(无上下文)→ UI「请先发起一次搜索」+ trace **0 行**(scope gate 之后才 trace,但 NoLastResults 是可路由 open → 实际有 call+error;注意:无上下文≠scope 拦截,trace 会有 call+error。**预期 trace 非 0**)。

> 修订 case 4 预期:`打开第1个` 是 Open(可路由)→ 进 trace call,invoke 返回 NoLastResults → on_error。故 trace 有 1 call + 1 error,**非 0 行**。0 行只出现在 scope gate 拦截(copy/move/rename)的情况。

---

## Self-Review(写计划后自检)

- **Spec 覆盖**:§2 架构→Task3 分支;§3 安全 gate→Task2 Step4 + `copy_blocked` 测试;§4 handle_file_action→Task2;§5 ActionDone+UI→Task1 + Task5;§6 Tracing→Task2(call/result/error)+ `index_out_of_range` 测试验证 call+error;§7 测试→Task2/3;§8 不回归→Task6。全覆盖。
- **Placeholder 扫描**:无 TBD/TODO;每步含完整代码或精确命令。
- **类型一致**:`handle_file_action`/`file_action_error_kind`/`friendly_file_action_message`/`build_file_action_tool`/`mk_file_action`/`context_with_results`/`MockFileActionExecutor` 命名跨 Task 一致;`SearchEvent::ActionDone { action_kind, paths }` 字段名 Rust(Task1)与 TS(Task5)一致;`describeAction`/`basename` 仅 Task5 内部。
- **修订**:真机 case 4 trace 预期已在计划内修正(无上下文的 open 是可路由→有 call+error,非 0 行;0 行仅 scope gate 拦截)。
