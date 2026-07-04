# FileAction(copy/move/rename)L4 确认流 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户搜索后用「把第N个复制/移动到X」「把第N个重命名为Y」对上一轮结果执行 copy/move/rename,经 UI 确认对话框确认后才执行。

**Architecture:** 承接第 31 阶段。`handle_file_action` 对 copy/move/rename 走确认往返:首次下发解析 target_ref(只读 context)+ 单目标校验 + destination 在 wiring 展开 ~ 并 join 源文件名 → 构造自包含 pending(target_ref=Path)存入 `Arc<Mutex<Option<FileAction>>>` State + 发 `ConfirmAction` 事件;用户确认走新 `confirm_action` command(invoke 执行),取消走 `cancel_action`。harness `FileActionTool` 一行不动。

**Tech Stack:** Rust(Tauri 2 command + harness)、TypeScript(React `SearchView.tsx`)、tokio test。

**Spec:** `docs/superpowers/specs/2026-05-29-file-action-confirm-flow-design.md`

---

## 文件结构

- **Modify** `apps/desktop/src-tauri/src/search.rs` — `SearchEvent::ConfirmAction` 变体、`ActionDoneData` 结构、`resolve_destination`/`expand_tilde`/`home_dir` helper、`friendly_file_action_message` 加 PathConflict 分支、`handle_confirmable_action`、`confirm_action_impl`/`cancel_action`、`confirm_action` command、`handle_file_action` 接线、测试。
- **Modify** `apps/desktop/src-tauri/src/main.rs` — `.manage(Arc<Mutex<Option<FileAction>>>)` + 注册 `confirm_action`/`cancel_action`。
- **Modify** `apps/desktop/src/SearchView.tsx` — `confirm_action` 事件 + `confirm_pending` 状态 + 确认对话框 + confirm/cancel invoke + `describeConfirm`。

## 关键既有 API(实现者无需重新发现)

- `locifind_search_backend::{FileAction, FileActionKind, TargetRef, TargetSelector, SchemaVersion, Language}`。`FileAction` 字段:`schema_version, language: Option<Language>, action: FileActionKind, target_ref: TargetRef, destination: Option<String>, new_name: Option<String>, requires_confirmation: bool`。`TargetRef::{Path { value: String }, LastResults { selector }}`。
- `locifind_harness::context::{ContextMemory, TargetRefError}`。`ContextMemory::resolve_target_ref(&TargetRef) -> Result<Vec<PathBuf>, TargetRefError>`(`TargetRef::Path` 直接返回包装路径,不依赖上一轮)。
- `locifind_harness::file_action_tool::{FileActionTool, FileActionOutcome, FileActionError}`。`FileActionTool::invoke(&self, &FileAction, &ContextMemory) -> Result<FileActionOutcome, FileActionError>`。`FileActionOutcome::{Executed{affected: Vec<PathBuf>}, RequiresConfirmation{paths}}`。
- 第 31 阶段已有(search.rs):`SearchEvent::{Started, Result, Complete, Error, ActionDone}`、`handle_file_action`、`file_action_error_kind`、`friendly_file_action_message`、`describe_intent`、测试 helper `build_file_action_tool()`(返回 `(Arc<FileActionTool>, Arc<Mutex<Vec<String>>>)`,MockExecutor 记录 "copy:/path"/"move:/path"/"rename:/path"/"open:/path"/"locate:/path")、`context_with_results(n)`、`mk_file_action(kind, idx)`、`build_tracer_with_mock()`、`capture_channel()`、`empty_context()`、`build_test_registry`、`FakeOkBackend`、常量 `QUERY_FOR_FILE_SEARCH="find pdf"`、`QUERY_OPEN_FIRST="打开第1个"`。
- search.rs 顶部已 `use locifind_harness::Tool;`(`file_action_tool.id()` 用),`use std::sync::{Arc, Mutex}`,`use std::time::Instant`,`use tauri::ipc::Channel`,`use serde::Serialize`,`use locifind_harness::context::{ContextMemory, RefineMergeError}`,`use locifind_harness::file_action_tool::{FileActionError}`(测试模块内)+ 顶部对 `FileActionError`/`TargetRefError` 的引用(检查 use,缺则补)。

---

## Task 1: ConfirmAction 事件 + ActionDoneData + resolve_destination + PathConflict 友好文案

**Files:** Modify `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: 加 `ConfirmAction` 变体 + `ActionDoneData`**

在 `SearchEvent` enum 内、`ActionDone { ... }` 变体之后加:

```rust
    /// 写操作待用户确认(copy/move/rename)。UI 弹确认对话框。
    ConfirmAction {
        /// "copy" | "move" | "rename"。
        action_kind: String,
        /// 待操作的源路径(本阶段单个)。
        paths: Vec<String>,
        /// copy/move 的完整目标路径(已解析);rename 为 None。
        destination: Option<String>,
        /// rename 的新名;copy/move 为 None。
        new_name: Option<String>,
    },
```

在 `SearchEvent` enum 定义**之后**(非测试代码)加确认结果结构:

```rust
/// `confirm_action` command 的成功返回。UI 用它切到 action_done 态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionDoneData {
    /// "copy" | "move" | "rename"。
    pub action_kind: String,
    /// 实际涉及的绝对路径。
    pub paths: Vec<String>,
}
```

- [ ] **Step 2: 写 resolve_destination + PathConflict 友好文案的失败测试**

在 `#[cfg(test)] mod tests` 末尾加:

```rust
    #[test]
    fn resolve_destination_expands_tilde_and_joins_basename() {
        // SAFETY: 测试进程级 env;本测试只读 HOME,不改
        let home = std::env::var("HOME").expect("HOME 应存在");
        let src = PathBuf::from("/tmp/report.pdf");
        let got = resolve_destination("~/Desktop", &src).unwrap();
        assert_eq!(got, PathBuf::from(format!("{home}/Desktop/report.pdf")));
    }

    #[test]
    fn resolve_destination_absolute_passthrough() {
        let src = PathBuf::from("/tmp/a.txt");
        let got = resolve_destination("/Users/x/Downloads", &src).unwrap();
        assert_eq!(got, PathBuf::from("/Users/x/Downloads/a.txt"));
    }

    #[test]
    fn resolve_destination_no_filename_errs() {
        let src = PathBuf::from("/");
        assert!(resolve_destination("~/Desktop", &src).is_err());
    }

    #[test]
    fn friendly_message_path_conflict() {
        let err = FileActionError::PathConflict {
            dest: PathBuf::from("/Users/x/Desktop/a.pdf"),
        };
        let msg = friendly_file_action_message(&err);
        assert!(msg.contains("已存在") && msg.contains("a.pdf"), "实得: {msg}");
    }
```

- [ ] **Step 3: 跑测试确认失败**

Run: `cargo test -p locifind-desktop resolve_destination 2>&1 | tail -5`
Expected: 编译失败 `cannot find function resolve_destination`。

- [ ] **Step 4: 实现 helper + 加 PathConflict 分支**

在 `friendly_file_action_message` 函数**之前**(非测试代码)加:

```rust
/// 把 parser 的 destination(如 "~/Desktop")展开为绝对目录,再 join 源文件名,
/// 得到 copy/move 的完整目标文件路径。
#[cfg_attr(not(test), allow(dead_code))]
fn resolve_destination(dest_hint: &str, source: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let dir = expand_tilde(dest_hint)?;
    let file_name = source
        .file_name()
        .ok_or_else(|| "无法确定目标位置".to_owned())?;
    Ok(dir.join(file_name))
}

/// 展开 `~` / `~/...` 到 home 目录;非 `~` 开头原样。
#[cfg_attr(not(test), allow(dead_code))]
fn expand_tilde(p: &str) -> Result<std::path::PathBuf, String> {
    if let Some(rest) = p.strip_prefix("~/") {
        Ok(home_dir()?.join(rest))
    } else if p == "~" {
        home_dir()
    } else {
        Ok(std::path::PathBuf::from(p))
    }
}

/// home 目录:HOME(unix)→ USERPROFILE(windows)兜底。
#[cfg_attr(not(test), allow(dead_code))]
fn home_dir() -> Result<std::path::PathBuf, String> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .ok_or_else(|| "无法确定目标位置".to_owned())
}
```

在 `friendly_file_action_message` 的 `match` 中,把现有的 `FileActionError::Executor(io) => ...` 分支**之前**插入 PathConflict 分支:

```rust
        FileActionError::PathConflict { dest } => {
            format!("目标已存在:{}", dest.display())
        }
```

> `ConfirmAction` 变体与 `ActionDoneData` 暂时未被构造/返回,但 serde derive 会视 enum 变体为已用;`ActionDoneData` 加 `#[allow(dead_code)]` 若 clippy 报未用(它有 `Serialize` derive 但无构造者,直到 Task 3)。Step 5 跑 clippy 确认;若 `ActionDoneData` 报 dead_code,在其 `#[derive(...)]` 上方加 `#[cfg_attr(not(test), allow(dead_code))]`。

- [ ] **Step 5: 跑测试确认通过 + fmt + clippy**

Run: `cargo test -p locifind-desktop resolve_destination friendly_message_path_conflict 2>&1 | tail -8`
Expected: 4 test PASS。
Run: `cargo fmt -p locifind-desktop && cargo fmt -p locifind-desktop --check`
Expected: clean。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告(dead_code 已压住)。

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: ConfirmAction 事件 + ActionDoneData + resolve_destination + PathConflict 文案(Task1)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: handle_confirmable_action(首次下发:解析+校验+存 pending+发 ConfirmAction)

**Files:** Modify `apps/desktop/src-tauri/src/search.rs`

独立实现 `handle_confirmable_action`(暂不接入 `handle_file_action`,Task 4 接线),直接单测。

- [ ] **Step 1: 写失败单测**

在 `#[cfg(test)] mod tests` 末尾加:

```rust
    fn empty_pending() -> Arc<Mutex<Option<locifind_search_backend::FileAction>>> {
        Arc::new(Mutex::new(None))
    }

    /// 构造一个指定 destination/new_name 的 FileAction(target=Index)。
    fn mk_confirmable(
        kind: locifind_search_backend::FileActionKind,
        idx: u32,
        destination: Option<&str>,
        new_name: Option<&str>,
    ) -> locifind_search_backend::FileAction {
        use locifind_search_backend::{FileAction, Language, SchemaVersion, TargetRef, TargetSelector};
        FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: kind,
            target_ref: TargetRef::LastResults { selector: TargetSelector::Index { value: idx } },
            destination: destination.map(str::to_owned),
            new_name: new_name.map(str::to_owned),
            requires_confirmation: true,
        }
    }

    #[tokio::test]
    async fn confirmable_copy_stores_pending_and_emits_confirm() {
        use locifind_search_backend::{FileActionKind, TargetRef};
        let ctx = context_with_results(2); // /tmp/f0, /tmp/f1
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        handle_confirmable_action(
            mk_confirmable(FileActionKind::Copy, 1, Some("~/Desktop"), None),
            ch,
            Arc::clone(&pending),
            Arc::clone(&ctx),
        )
        .await
        .unwrap();

        // 发了 confirm_action 事件,destination 是展开后的完整路径
        let home = std::env::var("HOME").unwrap();
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"confirm_action\"")), "实得 {events:?}");
        assert!(events.iter().any(|e| e.contains(&format!("{home}/Desktop/f0"))), "destination 应展开, 实得 {events:?}");

        // pending 槽存了自包含 action(target=Path)
        let p = pending.lock().unwrap();
        let pa = p.as_ref().expect("pending 应有值");
        assert_eq!(pa.action, FileActionKind::Copy);
        assert!(matches!(&pa.target_ref, TargetRef::Path { value } if value == "/tmp/f0"));
        assert_eq!(pa.destination.as_deref(), Some(format!("{home}/Desktop/f0").as_str()));
        assert!(pa.requires_confirmation);
    }

    #[tokio::test]
    async fn confirmable_rename_stores_pending() {
        use locifind_search_backend::FileActionKind;
        let ctx = context_with_results(2);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        handle_confirmable_action(
            mk_confirmable(FileActionKind::Rename, 1, None, Some("final")),
            ch,
            Arc::clone(&pending),
            Arc::clone(&ctx),
        )
        .await
        .unwrap();

        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"confirm_action\"") && e.contains("rename") && e.contains("final")));
        let p = pending.lock().unwrap();
        assert_eq!(p.as_ref().unwrap().new_name.as_deref(), Some("final"));
    }

    #[tokio::test]
    async fn confirmable_multi_target_errors_no_pending() {
        use locifind_search_backend::{FileAction, FileActionKind, Language, SchemaVersion, TargetRef, TargetSelector};
        let ctx = context_with_results(3);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        let action = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Copy,
            target_ref: TargetRef::LastResults { selector: TargetSelector::Indices { values: vec![1, 2] } },
            destination: Some("~/Desktop".to_owned()),
            new_name: None,
            requires_confirmation: true,
        };
        handle_confirmable_action(action, ch, Arc::clone(&pending), Arc::clone(&ctx)).await.unwrap();

        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"error\"") && e.contains("一次只能复制单个文件")), "实得 {events:?}");
        assert!(pending.lock().unwrap().is_none(), "多目标不应存 pending");
    }

    #[tokio::test]
    async fn confirmable_out_of_range_errors() {
        use locifind_search_backend::FileActionKind;
        let ctx = context_with_results(2);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        handle_confirmable_action(
            mk_confirmable(FileActionKind::Copy, 9, Some("~/Desktop"), None),
            ch,
            Arc::clone(&pending),
            Arc::clone(&ctx),
        )
        .await
        .unwrap();

        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"error\"") && e.contains('9')), "越界友好错误, 实得 {events:?}");
        assert!(pending.lock().unwrap().is_none());
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop confirmable_ 2>&1 | tail -5`
Expected: 编译失败 `cannot find function handle_confirmable_action`。

- [ ] **Step 3: 实现 handle_confirmable_action**

在 `handle_file_action` 函数**之后**(非测试代码)加:

```rust
/// 处理 `FileAction(Copy/Move/Rename)` 首次下发:只读 [`ContextMemory`] 解析 target_ref、
/// 单目标校验、copy/move 解析 destination(展开 ~ + join 源文件名),构造自包含 pending
/// (`target_ref=Path`)存入 `pending` 槽,发 [`SearchEvent::ConfirmAction`]。
///
/// 首次下发**不调 invoke、不进 trace**(pre-tool)。实际执行在 [`confirm_action_impl`]。
#[cfg_attr(not(test), allow(dead_code))]
async fn handle_confirmable_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    context: Arc<Mutex<ContextMemory>>,
) -> Result<(), String> {
    use locifind_search_backend::{FileAction, FileActionKind, TargetRef};

    // 1) 解析 target_ref(只读 context)
    let targets = {
        let guard = context.lock().unwrap_or_else(|e| e.into_inner());
        guard.resolve_target_ref(&action.target_ref)
    };
    let targets = match targets {
        Ok(t) => t,
        Err(e) => {
            let _ = on_event.send(SearchEvent::Error {
                message: friendly_file_action_message(&FileActionError::TargetRef(e)),
            });
            return Ok(());
        }
    };

    // 2) 单目标校验
    if targets.len() != 1 {
        let verb = match action.action {
            FileActionKind::Copy => "复制",
            FileActionKind::Move => "移动",
            FileActionKind::Rename => "重命名",
            _ => "处理",
        };
        let _ = on_event.send(SearchEvent::Error {
            message: format!("一次只能{verb}单个文件(多文件待后续)"),
        });
        return Ok(());
    }
    let source = &targets[0];

    // 3) copy/move 解析 destination;rename 取 new_name
    let (destination, new_name) = match action.action {
        FileActionKind::Copy | FileActionKind::Move => {
            let hint = match action.destination.as_deref() {
                Some(h) if !h.is_empty() => h,
                _ => {
                    let _ = on_event.send(SearchEvent::Error {
                        message: "无法确定目标位置".to_owned(),
                    });
                    return Ok(());
                }
            };
            match resolve_destination(hint, source) {
                Ok(p) => (Some(p.to_string_lossy().into_owned()), None),
                Err(msg) => {
                    let _ = on_event.send(SearchEvent::Error { message: msg });
                    return Ok(());
                }
            }
        }
        FileActionKind::Rename => match action.new_name.as_deref() {
            Some(n) if !n.is_empty() => (None, Some(n.to_owned())),
            _ => {
                let _ = on_event.send(SearchEvent::Error {
                    message: "未指定新文件名".to_owned(),
                });
                return Ok(());
            }
        },
        // handle_file_action 只把 Copy/Move/Rename 路由到这里
        _ => return Ok(()),
    };

    // 4) 构造自包含 pending(target=Path,确认时 invoke 不依赖 context)
    let source_str = source.to_string_lossy().into_owned();
    let pending_action = FileAction {
        schema_version: action.schema_version,
        language: action.language,
        action: action.action,
        target_ref: TargetRef::Path {
            value: source_str.clone(),
        },
        destination: destination.clone(),
        new_name: new_name.clone(),
        requires_confirmation: true,
    };
    *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(pending_action);

    // 5) 发 ConfirmAction
    let action_kind = format!("{:?}", action.action).to_lowercase();
    let _ = on_event.send(SearchEvent::ConfirmAction {
        action_kind,
        paths: vec![source_str],
        destination,
        new_name,
    });
    Ok(())
}
```

> 确认 `FileActionError` 与 `TargetRefError` 在非测试作用域可见:`friendly_file_action_message` 已 `use` 它们(顶部或函数内)。`FileActionError::TargetRef(e)` 包装需要 `FileActionError` 在 `handle_confirmable_action` 作用域;若编译报未找到,在文件顶部 use 区加 `use locifind_harness::file_action_tool::FileActionError;` 与 `use locifind_harness::context::TargetRefError;`(检查现有 use,§31 可能已在顶部引入)。

- [ ] **Step 4: 跑测试确认通过 + fmt + clippy**

Run: `cargo test -p locifind-desktop confirmable_ 2>&1 | tail -10`
Expected: 4 test PASS。
Run: `cargo fmt -p locifind-desktop && cargo fmt -p locifind-desktop --check`
Expected: clean。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5`
Expected: 无警告。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: handle_confirmable_action 首次下发(存 pending + 发 ConfirmAction)+ 4 单测(Task2)

单目标校验 + destination wiring 解析 + pending 用 Path 自包含。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: confirm_action_impl / cancel + command 包装 + 单测

**Files:** Modify `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: 写失败单测**

在 `#[cfg(test)] mod tests` 末尾加:

```rust
    /// 预置一个 pending copy action(target=Path 完整源,destination 完整目标)。
    fn pending_with_copy(src: &str, dest: &str) -> Arc<Mutex<Option<locifind_search_backend::FileAction>>> {
        use locifind_search_backend::{FileAction, FileActionKind, Language, SchemaVersion, TargetRef};
        let a = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Copy,
            target_ref: TargetRef::Path { value: src.to_owned() },
            destination: Some(dest.to_owned()),
            new_name: None,
            requires_confirmation: true,
        };
        Arc::new(Mutex::new(Some(a)))
    }

    #[test]
    fn confirm_action_no_pending_errs() {
        let pending = empty_pending();
        let (tool, _calls) = build_file_action_tool();
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let err = confirm_action_impl(&pending, &tool, &tracer, &ctx).unwrap_err();
        assert!(err.contains("没有待确认的操作"), "实得 {err}");
    }

    #[test]
    fn confirm_action_executes_and_clears_pending() {
        // dest 必须不存在,否则 invoke 返 PathConflict;用临时不存在路径
        let pending = pending_with_copy("/tmp/f0", "/tmp/locifind-confirm-test-dest-f0");
        let (tool, calls) = build_file_action_tool();
        let (tracer, trace_calls) = build_tracer_with_mock();
        let ctx = empty_context();

        let res = confirm_action_impl(&pending, &tool, &tracer, &ctx).unwrap();
        assert_eq!(res.action_kind, "copy");
        assert_eq!(res.paths, vec!["/tmp/f0".to_owned()]);

        // MockExecutor 收到 copy(src, dest)
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "copy:/tmp/f0");

        // pending 被消费
        assert!(pending.lock().unwrap().is_none(), "confirm 后 pending 应清空");

        // trace call + result
        let tc = trace_calls.lock().unwrap();
        assert!(tc.iter().any(|c| c.starts_with("call:")));
        assert!(tc.iter().any(|c| c.starts_with("result:")));
    }

    #[test]
    fn cancel_action_clears_pending() {
        let pending = pending_with_copy("/tmp/f0", "/tmp/whatever");
        {
            let p = pending.lock().unwrap();
            assert!(p.is_some());
        }
        *pending.lock().unwrap() = None; // cancel_action 的语义
        assert!(pending.lock().unwrap().is_none());
    }
```

> 注:`MockFileActionExecutor.copy` 只记录 `"copy:{src}"`(忽略 dest,见第 31 阶段实现),`Ok(())` 不真正写盘 → 不依赖 dest 是否存在。但 `FileActionTool::invoke` 的 copy 分支会先 `dest_path.exists()` 检查;`/tmp/locifind-confirm-test-dest-f0` 预期不存在 → 不冲突。若 CI 上偶发存在,测试用唯一名规避(已含 test 标识)。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop confirm_action 2>&1 | tail -5`
Expected: 编译失败 `cannot find function confirm_action_impl`。

- [ ] **Step 3: 实现 confirm_action_impl + 两个 command**

在 `handle_confirmable_action` 之后(非测试代码)加:

```rust
/// `confirm_action` command 主体:take pending → invoke 执行 → 返回 [`ActionDoneData`]。
/// 不依赖 [`tauri::State`],可单测。pending 为 None 返 Err。
#[cfg_attr(not(test), allow(dead_code))]
fn confirm_action_impl(
    pending: &Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    file_action_tool: &Arc<locifind_harness::file_action_tool::FileActionTool>,
    tracer: &Arc<locifind_harness::Tracer>,
    context: &Arc<Mutex<ContextMemory>>,
) -> Result<ActionDoneData, String> {
    let action = {
        let mut guard = pending.lock().unwrap_or_else(|e| e.into_inner());
        guard.take()
    };
    let action = action.ok_or_else(|| "没有待确认的操作".to_owned())?;

    let tool_id = file_action_tool.id().to_owned();
    let tool_start = Instant::now();
    tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: tool_id.clone(),
        tool_kind: locifind_harness::ToolKind::FileAction,
        intent_variant: locifind_harness::SupportedIntent::FileAction,
    });

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
            Ok(ActionDoneData {
                action_kind: format!("{:?}", action.action).to_lowercase(),
                paths: affected
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            })
        }
        Ok(locifind_harness::file_action_tool::FileActionOutcome::RequiresConfirmation { .. }) => {
            tracer.on_error(&locifind_harness::ToolErrorEvent {
                tool_id,
                duration: tool_start.elapsed(),
                error_type: "UnexpectedRequiresConfirmation".to_owned(),
            });
            Err("操作未能确认执行".to_owned())
        }
        Err(err) => {
            tracer.on_error(&locifind_harness::ToolErrorEvent {
                tool_id,
                duration: tool_start.elapsed(),
                error_type: file_action_error_kind(&err).to_owned(),
            });
            Err(friendly_file_action_message(&err))
        }
    }
}

/// 确认并执行待确认的 file action。
#[tauri::command]
pub async fn confirm_action(
    pending: tauri::State<'_, Arc<Mutex<Option<locifind_search_backend::FileAction>>>>,
    file_action_tool: tauri::State<'_, Arc<locifind_harness::file_action_tool::FileActionTool>>,
    tracer: tauri::State<'_, Arc<locifind_harness::Tracer>>,
    context: tauri::State<'_, Arc<Mutex<ContextMemory>>>,
) -> Result<ActionDoneData, String> {
    confirm_action_impl(&pending, &file_action_tool, &tracer, &context)
}

/// 取消待确认的 file action(清空 pending 槽)。
#[tauri::command]
pub async fn cancel_action(
    pending: tauri::State<'_, Arc<Mutex<Option<locifind_search_backend::FileAction>>>>,
) -> Result<(), String> {
    *pending.lock().unwrap_or_else(|e| e.into_inner()) = None;
    Ok(())
}
```

> `confirm_action_impl(&pending, ...)`:`pending` 是 `tauri::State<Arc<...>>`,通过 Deref 强转为 `&Arc<...>` 传入(deref coercion)。若编译器不自动强转,改为 `&*pending` / `&file_action_tool` → `&*file_action_tool` 等。

- [ ] **Step 4: 跑测试通过 + fmt + clippy**

Run: `cargo test -p locifind-desktop confirm_action cancel_action 2>&1 | tail -10`
Expected: 3 test PASS。
Run: `cargo fmt -p locifind-desktop && cargo fmt -p locifind-desktop --check` → clean。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5` → 无警告。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: confirm_action_impl + confirm_action/cancel_action command + 3 单测(Task3)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: 接线 handle_file_action + thread pending + 集成测试

**Files:** Modify `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: handle_file_action 加 pending 参数 + 路由 Copy/Move/Rename**

修改 `handle_file_action` 签名,在 `context: Arc<Mutex<ContextMemory>>` 之后加:
```rust
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
```
把现有的 scope gate(`match action.action { Open | Locate => {} other => { ... 暂不支持 ... } }`)替换为:
```rust
    // scope gate:open/locate 立即执行;copy/move/rename 走确认往返;delete 拒绝。
    match action.action {
        FileActionKind::Open | FileActionKind::Locate => {}
        FileActionKind::Copy | FileActionKind::Move | FileActionKind::Rename => {
            return handle_confirmable_action(action, on_event, pending, context).await;
        }
        FileActionKind::Delete => {
            eprintln!("search: delete 不支持");
            let _ = on_event.send(SearchEvent::Error {
                message: "删除操作不支持".to_owned(),
            });
            return Ok(());
        }
    }
```
(其余 open/locate 逻辑不变;`pending` 在 open/locate 路径不使用 —— 加 `let _ = &pending;` 不必要,因为 Copy/Move/Rename 分支已用到它,编译器不会报未用参数。)

- [ ] **Step 2: thread pending 过 search_impl + search command**

在 `pub(crate) async fn search_impl` 签名末尾(`file_action_tool` 之后)加:
```rust
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
```
在 search_impl 内调用 `handle_file_action(...)` 的那处(FileAction 分支),追加 `pending` 作为最后实参。

在 `#[tauri::command] pub async fn search` 签名末尾加:
```rust
    pending: tauri::State<'_, Arc<Mutex<Option<locifind_search_backend::FileAction>>>>,
```
并在其对 `search_impl(...)` 的调用追加 `Arc::clone(&*pending)` 作为最后实参。

- [ ] **Step 3: 移除 dead_code 暂存属性**

移除以下项上的 `#[cfg_attr(not(test), allow(dead_code))]`(现在均被接线为可达):`handle_confirmable_action`、`confirm_action_impl`、`resolve_destination`、`expand_tilde`、`home_dir`、`ActionDoneData`(若 Task1 加了)。`confirm_action`/`cancel_action` command 仍未注册(Task 5),但 `#[tauri::command]` 生成的 wrapper 默认不会触发 dead_code(被 `generate_handler!` 引用前,函数本体仍可能报未用 → 若 clippy 报,保留它们的 dead_code attr 到 Task 5;但 `confirm_action` 调用了 `confirm_action_impl` 使其可达)。**Step 6 跑 clippy 校验**;按结果决定 confirm_action/cancel_action 是否暂留 attr。

- [ ] **Step 4: 更新现有 handle_file_action / search_impl 测试调用点**

所有调用 `handle_file_action(...)` 的测试加最后实参 `empty_pending()`;所有调用 `search_impl(...)` 的测试加最后实参 `empty_pending()`。

`handle_file_action(...)` 调用点(5 处):`handle_file_action_open_executes`、`handle_file_action_locate_executes`、`handle_file_action_copy_blocked_not_executed`(将改写,见 Step 5)、`handle_file_action_index_out_of_range_errors`、`handle_file_action_no_context_errors`。

`search_impl(...)` 调用点(逐个加 `empty_pending()`):`search_impl_success_emits_call_then_result`、`_open_err_emits_call_then_error`、`_mid_stream_err_emits_call_then_error`、`_pre_tool_failure_emits_no_trace`、`search_impl_record_then_refine_merges_base`(2 处)、`search_impl_refine_without_context_errors`、`search_impl_chained_refine_accumulates`(循环内 1 处)、`search_then_open_first_executes_on_last_results`(2 处)、`action_does_not_clobber_context`(循环内 1 处)。

> 每处在原最后一个实参后追加 `empty_pending()`。`empty_pending()` 已在 Task 2 定义。

- [ ] **Step 5: 改写 copy_blocked 测试为新行为 + 加集成测试**

把 `handle_file_action_copy_blocked_not_executed` 整个测试**替换**为(copy 现在不再是 Error,而是走确认 → 存 pending + ConfirmAction,不执行):

```rust
    #[tokio::test]
    async fn handle_file_action_copy_routes_to_confirm() {
        use locifind_search_backend::FileActionKind;
        let (tool, calls) = build_file_action_tool();
        let (tracer, trace_calls) = build_tracer_with_mock();
        let ctx = context_with_results(2);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        handle_file_action(
            mk_confirmable(FileActionKind::Copy, 1, Some("~/Desktop"), None),
            ch,
            tool,
            tracer,
            ctx,
            Arc::clone(&pending),
        )
        .await
        .unwrap();

        // 不执行、不 trace(首次下发是 pre-tool)
        assert!(calls.lock().unwrap().is_empty(), "copy 首次下发不应执行");
        assert!(trace_calls.lock().unwrap().is_empty(), "首次下发不进 trace");
        // 存了 pending + 发了 confirm_action
        assert!(pending.lock().unwrap().is_some(), "应存 pending");
        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"confirm_action\"")), "实得 {events:?}");
    }
```

在 test mod 末尾加集成测试(端到端:search → copy 首次下发 → confirm):

```rust
    /// "把第1个复制到桌面" 稳定解析为 FileAction(Copy, Index:1, dest=~/Desktop)。
    const QUERY_COPY_FIRST_TO_DESKTOP: &str = "把第1个复制到桌面";

    #[tokio::test]
    async fn search_copy_then_confirm_executes() {
        let registry = build_test_registry(
            FakeOkBackend(2),
            vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
        );
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let (tool, calls) = build_file_action_tool();
        let pending = empty_pending();

        // 第一轮:find pdf → 记录 f0, f1
        let (ch1, _c1) = capture_channel();
        search_impl(
            QUERY_FOR_FILE_SEARCH.into(),
            ch1,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        )
        .await
        .unwrap();

        // 第二轮:把第1个复制到桌面 → 存 pending + ConfirmAction,不执行
        let (ch2, events2) = capture_channel();
        search_impl(
            QUERY_COPY_FIRST_TO_DESKTOP.into(),
            ch2,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        )
        .await
        .unwrap();

        assert!(events2.lock().unwrap().iter().any(|e| e.contains("\"confirm_action\"")), "应发 confirm_action");
        assert!(calls.lock().unwrap().is_empty(), "确认前不应执行");
        assert!(pending.lock().unwrap().is_some(), "应存 pending");

        // 确认 → 执行
        let res = confirm_action_impl(&pending, &tool, &tracer, &ctx).unwrap();
        assert_eq!(res.action_kind, "copy");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], "copy:/tmp/f0", "应复制上一轮第 1 个");
    }
```

> 若 `把第1个复制到桌面` 经 `resolve_intent` 未稳定解析为 Copy(集成测试 routing 不对),STOP 并报告;按 parser `try_parse_file_action`(`复制到` + 第N个 + `extract_destination("桌面")=~/Desktop`)它应产出 `FileAction(Copy, Index:1, dest="~/Desktop")`。

- [ ] **Step 6: 全量 desktop 测试 + fmt + clippy**

Run: `cargo test -p locifind-desktop 2>&1 | tail -15`
Expected: 全部 PASS(原 24 + Task1 的 4 + Task2 的 4 + Task3 的 3 + 本 Task 集成 1 + copy_routes 改写后净 +1 ≈ 36 左右)。
Run: `cargo fmt -p locifind-desktop --check` → clean。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5` → 无警告。

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: handle_file_action 接 copy/move/rename 确认分支 + thread pending + 集成测试(Task4)

移除 dead_code 暂存属性;copy_blocked 测试改写为 routes_to_confirm。

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: main.rs 注入 pending State + 注册 command

**Files:** Modify `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: 构造 + manage + 注册**

在 `fn main()` 内,`let file_action_tool = ...;` 之后加:
```rust
    let pending_action: Arc<Mutex<Option<locifind_search_backend::FileAction>>> =
        Arc::new(Mutex::new(None));
```
在 builder 链 `.manage(file_action_tool)` 之后加:
```rust
        .manage(pending_action)
```
在 `invoke_handler(tauri::generate_handler![...])` 列表中,`search::search,` 之后加:
```rust
            search::confirm_action,
            search::cancel_action,
```
顶部确认 `locifind_search_backend` 可用:`main.rs` 已通过其他 crate 间接引入;若 `locifind_search_backend::FileAction` 未解析,在顶部加 `use locifind_search_backend::FileAction;` 并把上面类型写成 `Arc<Mutex<Option<FileAction>>>`。

- [ ] **Step 2: build + fmt + clippy + test**

Run: `cargo build -p locifind-desktop 2>&1 | tail -8` → 编译通过(confirm_action/cancel_action 的 State 现有对应 managed value)。
Run: `cargo fmt -p locifind-desktop --check` → clean。
Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -5` → 无警告(若 Task4 Step3 给 confirm_action/cancel_action 暂留了 dead_code attr,现已注册,移除该 attr 并重跑 clippy 确认)。
Run: `cargo test -p locifind-desktop 2>&1 | tail -6` → 全过。

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: main.rs 注入 pending State + 注册 confirm_action/cancel_action(Task5)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: SearchView.tsx 确认对话框

**Files:** Modify `apps/desktop/src/SearchView.tsx`

- [ ] **Step 1: 扩 SearchEvent 类型**

在 `type SearchEvent =` 联合,`| { event: "error"; message: string };` 之前加:
```typescript
  | {
      event: "confirm_action";
      action_kind: string;
      paths: string[];
      destination: string | null;
      new_name: string | null;
    }
```

- [ ] **Step 2: 扩 Status 类型**

在 `type Status =` 联合,`| { kind: "error"; message: string };` 之前加:
```typescript
  | {
      kind: "confirm_pending";
      action_kind: string;
      paths: string[];
      destination: string | null;
      new_name: string | null;
    }
```

- [ ] **Step 3: switch 加 confirm_action 分支**

在 `switch (msg.event)` 内,`case "error":` 之前加:
```typescript
        case "confirm_action": {
          setStatus({
            kind: "confirm_pending",
            action_kind: msg.action_kind,
            paths: msg.paths,
            destination: msg.destination,
            new_name: msg.new_name,
          });
          break;
        }
```

- [ ] **Step 4: 确认/取消处理函数**

在 `handleSearch` 之后(组件函数体内)加:
```typescript
  const handleConfirm = useCallback(async () => {
    try {
      const res = await invoke<{ action_kind: string; paths: string[] }>(
        "confirm_action",
      );
      setStatus({
        kind: "action_done",
        action_kind: res.action_kind,
        paths: res.paths,
      });
    } catch (err) {
      setStatus({ kind: "error", message: String(err) });
    }
  }, []);

  const handleCancel = useCallback(async () => {
    try {
      await invoke("cancel_action");
    } catch {
      // 取消失败无关紧要,直接回 idle
    }
    setStatus({ kind: "idle" });
  }, []);
```

- [ ] **Step 5: 渲染 confirm_pending 态 + describeConfirm**

在 `{status.kind === "action_done" && (...)}` 块之前加:
```tsx
      {status.kind === "confirm_pending" && (
        <div className="search-status confirm-pending">
          <p>{describeConfirm(status.action_kind, status.paths, status.destination, status.new_name)}</p>
          <div className="confirm-actions">
            <button type="button" className="confirm-yes" onClick={handleConfirm}>
              确认
            </button>
            <button type="button" className="confirm-no" onClick={handleCancel}>
              取消
            </button>
          </div>
        </div>
      )}
```
在文件末尾(组件外)加(`basename` 已在第 31 阶段存在,勿重复定义):
```tsx
function describeConfirm(
  kind: string,
  paths: string[],
  destination: string | null,
  newName: string | null,
): string {
  const name = paths.length > 0 ? basename(paths[0]) : "";
  if (kind === "copy") {
    return `复制 ${name} 到 ${destination ?? ""}?`;
  }
  if (kind === "move") {
    return `移动 ${name} 到 ${destination ?? ""}?`;
  }
  if (kind === "rename") {
    return `重命名 ${name} 为 ${newName ?? ""}?`;
  }
  return `确认对 ${name} 执行 ${kind}?`;
}
```

- [ ] **Step 6: 类型检查 + build**

Run: `cd apps/desktop && npm run build 2>&1 | tail -12`
Expected: tsc + vite build 通过,无类型错误。(确认 `basename` 未重复声明 —— 第 31 阶段已加;若报 redeclare,删本 Task 的重复,只加 `describeConfirm`。)

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "desktop UI: confirm_action 确认对话框 + confirm/cancel 调用(Task6)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: 全量 CI + evals 不回归

**Files:** 无改动,纯验证。

- [ ] **Step 1: 全量 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -20`
Expected: fmt + clippy(-D warnings)+ build + test 全过。

- [ ] **Step 2: evals byte-equal 确认**

Run: `cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 2>&1 | grep -iE "pass:|partial:|fail:" | head`
Expected: **pass 472 / partial 26 / fail 2**(harness/parser 不动,evals 不依赖 desktop)。

- [ ] **Step 3: 无需 commit(验证 task)**

若 Step 1/2 出现非预期变化,回到对应 Task 修复。

---

## 真机手测(用户驱动,收尾会话执行)

用户运行 `cd apps/desktop && LOCIFIND_TRACE=/tmp/locifind-trace-confirm.jsonl npm run tauri dev`,agent 盯 trace + 核对 UI:

1. `find pdf` → `把第1个复制到桌面` → 确认对话框「复制 X 到 .../Desktop?」+ 确认/取消按钮;**首次下发无 trace**。
2. 点**确认** → 文件真出现在桌面 + UI「已复制 ...」(action_done)+ trace `call(file_action.local)` + `result`。
3. `移动第2个到下载` → 确认 → 文件真移动到下载。
4. `把第1个重命名为 testname` → 确认 → 文件真改名。
5. 任一 copy → 点**取消** → 文件未动 + 回 idle + 无 trace。
6. (可选)多目标 `把这些复制到桌面`(若 parser 出 All)→ 友好「一次只能复制单个文件」。

> 真机手测会真改你的文件系统(复制/移动/改名),请用 fixture 合成文件或不重要的文件验证。

---

## Self-Review(写计划后自检)

- **Spec 覆盖**:§2 架构 → Task2(首次下发)+ Task3(confirm/cancel)+ Task4(接线);§3 destination + 单目标 → Task1(resolve_destination)+ Task2(校验);§4 ConfirmAction + command → Task1 + Task3;§5 错误 UX → Task1(PathConflict)+ Task2(多目标/destination);§6 Tracing → Task3(confirm 才 trace)+ Task2(首次下发不 trace,测试断言);§7 ContextMemory 只读 → Task2/Task3 用只读 guard,无 record/clear;§8 测试 → Task2/3/4;§9 不回归 → Task7。全覆盖。
- **Placeholder 扫描**:无 TBD/TODO;每步含完整代码或精确命令。
- **类型一致**:`SearchEvent::ConfirmAction { action_kind, paths, destination, new_name }` 字段名 Rust(Task1)与 TS(Task6)一致;`ActionDoneData { action_kind, paths }` Rust(Task1)与 TS confirm_action 返回类型(Task6)一致;`handle_confirmable_action`/`confirm_action_impl`/`cancel_action`/`resolve_destination`/`expand_tilde`/`home_dir`/`mk_confirmable`/`empty_pending`/`pending_with_copy` 命名跨 Task 一致;pending 类型 `Arc<Mutex<Option<FileAction>>>` 贯穿 Task2-5。
- **修订**:Task4 Step3 对 confirm_action/cancel_action 的 dead_code 处理留了"按 clippy 结果决定"的明确指引(Tauri command wrapper 在 generate_handler! 注册前可能报未用),Task5 注册后移除 —— 避免 clippy 卡住。
