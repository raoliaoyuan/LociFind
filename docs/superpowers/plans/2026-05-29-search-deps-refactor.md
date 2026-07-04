# SearchDeps 依赖收拢重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `search`/`confirm_action`/`cancel_action` 命令共享的 6 个 `Arc` 依赖收拢成单一 `SearchDeps`，作为一个 `tauri::manage` 状态注入，消除全部 3 处 `#[allow(clippy::too_many_arguments)]`，并顺手修正 `get_backend_status` 的 registry 类型不匹配。

**Architecture:** 新增 `SearchDeps` 结构体（私有字段持 6 个 `Arc` + `new()` 构造 + `pub(crate) registry()` 访问器）。内部大函数（`search_impl`/`handle_file_action`/`confirm_action_impl`）改吃 `&SearchDeps`；叶子小函数（`handle_confirmable_action`/`cancel_action_impl`）维持窄 `&Arc` 引用。命令薄壳解 `State<SearchDeps>` 后用 `deps.inner()` 委托。分三步落地，每步保持编译 + 测试绿。

**Tech Stack:** Rust 2021、Tauri 2（`tauri::State` / `tauri::command`）、tokio test、locifind-harness / locifind-search-backend crate。

**这是行为等价重构（唯一例外：`get_backend_status` 从拿不到 registry 转为拿到）。安全网是既有 44 个 desktop 测试 + CI（fmt + `clippy -D warnings` + build + test）。本计划不写新行为测试，纪律是"每步后既有测试全绿 + clippy 无 `too_many_arguments`"，外加 Task 1 一个新增的 SearchDeps 单测、Task 3 一个 status 测试。**

---

## File Structure

| 文件 | 职责 | 改动 |
|---|---|---|
| `apps/desktop/src-tauri/src/search.rs` | SearchDeps 定义 + search/file-action 管道 + 命令薄壳 + 测试 | 新增结构体；改 6 个函数签名；改 ~40 测试调用点 |
| `apps/desktop/src-tauri/src/main.rs` | Tauri builder：构造依赖 + manage + 注册命令 | 6 个 `.manage()` → 1 个 `.manage(deps)` |
| `apps/desktop/src-tauri/src/status.rs` | `get_backend_status` 命令 | registry 依赖改经 `State<SearchDeps>` |

无 TS 改动；无 harness/parser/evals 源改动。

---

## Task 1: 新增 SearchDeps 结构体（additive，green）

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（在 `ActionDoneData` 定义之后、`search` 命令之前插入，约 line 84 后）
- Test: 同文件 `mod tests`

- [ ] **Step 1: 在 search.rs 插入 SearchDeps 定义**

在 `pub struct ActionDoneData { ... }` 之后插入：

```rust
/// 收拢 search/confirm/cancel 命令共享的 6 个进程级依赖，作为单一 `tauri::manage`
/// 状态注入。新增共享依赖只需改本结构体 + `new()` + 用到它的大函数体，
/// 不再触动命令签名与 `main.rs` 的 manage 列表。
pub struct SearchDeps {
    registry: Arc<ToolRegistry>,
    policy: Arc<PolicyEngine>,
    tracer: Arc<locifind_harness::Tracer>,
    context: Arc<Mutex<ContextMemory>>,
    file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
}

impl SearchDeps {
    pub fn new(
        registry: Arc<ToolRegistry>,
        policy: Arc<PolicyEngine>,
        tracer: Arc<locifind_harness::Tracer>,
        context: Arc<Mutex<ContextMemory>>,
        file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
        pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    ) -> Self {
        Self {
            registry,
            policy,
            tracer,
            context,
            file_action_tool,
            pending,
        }
    }

    /// 只读访问 registry，供同 crate 的 `status::get_backend_status` 使用（Task 3）。
    pub(crate) fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}
```

> 注：`new()` 是 6 参（clippy `too_many_arguments` 阈值 7，仅 8+ 触发），无需任何 `allow`。

- [ ] **Step 2: 加一个 SearchDeps 单测**

在 `mod tests` 内（紧跟现有 helper 如 `empty_context` 之后即可）加：

```rust
#[test]
fn search_deps_new_exposes_registry() {
    let registry = build_test_registry(
        FakeOkBackend(0),
        vec![SupportedIntent::FileSearch],
    );
    let policy = Arc::new(PolicyEngine::new());
    let (tracer, _calls) = build_tracer_with_mock();
    let deps = SearchDeps::new(
        registry,
        policy,
        tracer,
        empty_context(),
        build_file_action_tool().0,
        empty_pending(),
    );
    // 访问器能取到 registry（Task 3 status 命令依赖此路径）
    let summaries = locifind_harness::CapabilityDiscovery::new(deps.registry()).backend_summary();
    assert!(
        summaries.iter().any(|s| s.id == "search.fake"),
        "registry() 应暴露已注册的 fake 后端, 实得 {summaries:?}"
    );
}
```

> 依赖的测试 helper（`build_test_registry` / `FakeOkBackend` / `build_tracer_with_mock` / `empty_context` / `build_file_action_tool` / `empty_pending` / `SupportedIntent`）均已存在于 `mod tests`。若 `CapabilityDiscovery` 未在 tests 作用域导入，在断言行用全路径 `locifind_harness::CapabilityDiscovery`（如上）即可，无需加 `use`。

- [ ] **Step 3: 编译 + 跑新测试**

Run: `cargo test -p locifind-desktop search_deps_new_exposes_registry`
Expected: PASS（1 passed）。其余测试不受影响。

> crate 名以 `apps/desktop/src-tauri/Cargo.toml` 的 `[package].name` 为准；若不是 `locifind-desktop`，用实际名。可先 `grep '^name' apps/desktop/src-tauri/Cargo.toml` 确认。

- [ ] **Step 4: clippy 确认无新增告警**

Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings`
Expected: 通过（SearchDeps 为纯新增，3 处既有 `allow` 仍在）。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "refactor(desktop): 新增 SearchDeps 结构体收拢 search 命令共享依赖(Task1)"
```

---

## Task 2: 内部管道函数改吃 &SearchDeps（green，去 2 处 allow）

**目标**：`search_impl` / `handle_file_action` 改 `&SearchDeps`；`handle_confirmable_action` 改窄 `&Arc` 引用。`search` 命令暂时在体内用 6 个既有 `State` 构造 `SearchDeps` 再委托（**保留其 `allow` 到 Task 3**）。`confirm_action_impl` / `cancel_action_impl` / `confirm_action` / `cancel_action` / `main.rs` / `status.rs` 本步不动。

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`

- [ ] **Step 1: 改 `search_impl` 签名为 `&SearchDeps`**

把（约 line 115–125）：

```rust
#[allow(clippy::too_many_arguments)]
pub(crate) async fn search_impl(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: Arc<ToolRegistry>,
    policy: Arc<PolicyEngine>,
    tracer: Arc<locifind_harness::Tracer>,
    context: Arc<Mutex<ContextMemory>>,
    file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
) -> Result<(), String> {
```

改为（**删除 `#[allow]`**）：

```rust
pub(crate) async fn search_impl(
    query: String,
    on_event: Channel<SearchEvent>,
    deps: &SearchDeps,
) -> Result<(), String> {
```

- [ ] **Step 2: 改 `search_impl` 体内对各依赖的引用**

在 `search_impl` 体内，把裸标识符改为 `deps.<field>`：

- `apply_refine_if_needed` 处的 `context.lock()` → `deps.context.lock()`
- FileAction 分支（约 line 168-172）：
  ```rust
  if let SearchIntent::FileAction(ref fa) = effective {
      let action = fa.clone();
      return handle_file_action(action, on_event, deps).await;
  }
  ```
- Policy gate：`policy.evaluate(&action)` → `deps.policy.evaluate(&action)`
- Router：`IntentRouter::new(&registry)` → `IntentRouter::new(&deps.registry)`
- Trace 三处：`tracer.on_tool_call(...)` / `tracer.on_error(...)` / `tracer.on_tool_result(...)` → `deps.tracer.<...>`
- record（约 line 285-288）：`context.lock()` → `deps.context.lock()`

> `&deps.registry` 是 `&Arc<ToolRegistry>`，传给要 `&ToolRegistry` 的 `IntentRouter::new` 经 deref coercion 自动成立。

- [ ] **Step 3: 改 `handle_file_action` 签名为 `&SearchDeps`**

把（约 line 326–334）：

```rust
#[allow(clippy::too_many_arguments)]
async fn handle_file_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    file_action_tool: Arc<locifind_harness::file_action_tool::FileActionTool>,
    tracer: Arc<locifind_harness::Tracer>,
    context: Arc<Mutex<ContextMemory>>,
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
) -> Result<(), String> {
```

改为（**删除 `#[allow]`**）：

```rust
async fn handle_file_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    deps: &SearchDeps,
) -> Result<(), String> {
```

- [ ] **Step 4: 改 `handle_file_action` 体内引用**

- copy/move/rename 分支：`return handle_confirmable_action(action, on_event, pending, context).await;` → `return handle_confirmable_action(action, on_event, &deps.pending, &deps.context).await;`
- `let tool_id = file_action_tool.id()...` → `deps.file_action_tool.id()`
- `tracer.on_tool_call(...)` → `deps.tracer.on_tool_call(...)`
- invoke：`file_action_tool.invoke(&action, &guard)` → `deps.file_action_tool.invoke(&action, &guard)`，其中 `context.lock()` → `deps.context.lock()`
- 其余 `tracer.<...>` → `deps.tracer.<...>`

- [ ] **Step 5: 改 `handle_confirmable_action` 签名为窄 `&Arc`**

把（约 line 416–421）：

```rust
async fn handle_confirmable_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    pending: Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    context: Arc<Mutex<ContextMemory>>,
) -> Result<(), String> {
```

改为：

```rust
async fn handle_confirmable_action(
    action: locifind_search_backend::FileAction,
    on_event: Channel<SearchEvent>,
    pending: &Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    context: &Arc<Mutex<ContextMemory>>,
) -> Result<(), String> {
```

> 体内对 `pending` / `context` 的用法（`context.lock()`、`*pending.lock()... = Some(...)`）对 `&Arc` 透明，**无需改体内**（`Arc` 的方法经 autoref/deref 在 `&Arc` 上同样可调）。

- [ ] **Step 6: 改 `search` 命令体内：构造 SearchDeps 委托（保留 allow）**

`search` 命令签名**不动**（仍 6 个 `State` + `#[allow]`）。把体内（约 line 98–108）：

```rust
    search_impl(
        query,
        on_event,
        Arc::clone(&*registry),
        Arc::clone(&*policy),
        Arc::clone(&*tracer),
        Arc::clone(&*context),
        Arc::clone(&*file_action_tool),
        Arc::clone(&*pending),
    )
    .await
```

改为：

```rust
    let deps = SearchDeps::new(
        Arc::clone(&*registry),
        Arc::clone(&*policy),
        Arc::clone(&*tracer),
        Arc::clone(&*context),
        Arc::clone(&*file_action_tool),
        Arc::clone(&*pending),
    );
    search_impl(query, on_event, &deps).await
```

> 这是过渡形态：`search` 命令此刻仍带 6 个 `State` 和 `allow`，Task 3 才收掉。

- [ ] **Step 7: 更新 `run_search` 测试 helper（集中改造点）**

把（约 line 2168–2189）`run_search` 体内的 `search_impl(query.into(), ch, Arc::clone(registry), ...8 args...)` 改为：

```rust
        let (ch, events) = capture_channel();
        let deps = SearchDeps::new(
            Arc::clone(registry),
            Arc::clone(policy),
            Arc::clone(tracer),
            Arc::clone(ctx),
            Arc::clone(tool),
            Arc::clone(pending),
        );
        search_impl(query.into(), ch, &deps).await.unwrap();
```

`run_search` 的参数签名（取 `&Arc` 引用）保持不变。

- [ ] **Step 8: 更新所有直接 `search_impl(...)` 测试调用点**

模式：把 8 参位置调用替换为"先 `SearchDeps::new(...)` 再传 `&deps`"。**若该测试在调用后还要断言某依赖（如 `ctx` / `pending`），用 `Arc::clone` 喂给 `new()` 并保留原句柄。**

示例（`search_impl_success_emits_call_then_result`，约 line 1104）：

```rust
        let deps = SearchDeps::new(
            registry,
            policy,
            tracer,
            empty_context(),
            build_file_action_tool().0,
            empty_pending(),
        );
        search_impl(QUERY_FOR_FILE_SEARCH.into(), ch, &deps)
            .await
            .unwrap();
```

需改的直接 `search_impl` 调用点行号（Task 1/2 改动会使行号漂移，用 grep 重新定位）：

Run: `grep -n 'search_impl(' apps/desktop/src-tauri/src/search.rs`

逐个改为 `SearchDeps::new(...)` + `&deps` 形态。`run_search` 内那处已在 Step 7 改过，跳过。

- [ ] **Step 9: 更新所有 `handle_file_action(...)` 测试调用点**

Run: `grep -n 'handle_file_action(' apps/desktop/src-tauri/src/search.rs`

把每处 6 参调用（`handle_file_action(action, ch, Arc::clone(&tool), Arc::clone(&tracer), Arc::clone(&ctx), Arc::clone(&pending))` 之类）改为：

```rust
        let deps = SearchDeps::new(
            empty_registry_arc(),       // 见下方 Step 9b
            Arc::new(PolicyEngine::new()),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        );
        handle_file_action(action, ch, &deps).await.unwrap();
```

> `handle_file_action` 不读 registry/policy，但 `SearchDeps::new` 需要全 6 个。用空 registry + 默认 policy 填充。各测试若已持 `ctx`/`pending`/`tool`/`tracer` 句柄做后续断言，照旧用 `Arc::clone` 喂入并保留句柄。

- [ ] **Step 9b: 加 `empty_registry_arc` 测试 helper（DRY）**

在 `mod tests` helper 区（紧邻 `empty_context` / `empty_pending`）加：

```rust
    /// 空 registry，供不依赖搜索后端的 file-action 测试构造 SearchDeps 用。
    fn empty_registry_arc() -> Arc<ToolRegistry> {
        Arc::new(ToolRegistry::new())
    }
```

> 若 `ToolRegistry` 未在 tests 导入，用全路径 `locifind_harness::ToolRegistry::new()`。`handle_confirmable_action` 测试调用点（Step 10）也复用本 helper 思路，但它们不经 SearchDeps，无需改 registry。

- [ ] **Step 10: 更新 `handle_confirmable_action(...)` 测试调用点**

Run: `grep -n 'handle_confirmable_action(' apps/desktop/src-tauri/src/search.rs`

签名从 owned `Arc` 改 `&Arc`，故调用点把 `Arc::clone(&pending), Arc::clone(&ctx)`（或 `Arc::clone(&pending)`、`pending` 值传）改为传引用 `&pending, &ctx`。示例（约 line 1956）：

```rust
        handle_confirmable_action(action, ch, &pending, &ctx).await
```

逐处确保传 `&pending` / `&ctx`（引用），而非 owned clone。

- [ ] **Step 11: 编译 + 全测试**

Run: `cargo test -p locifind-desktop`
Expected: 全 PASS（既有 44 + Task1 新增 1 = 45）。若编译报"expected `&SearchDeps`, found `Arc<...>`"或参数数量不符，说明有调用点遗漏，按报错行补改。

- [ ] **Step 12: clippy 确认去掉了 2 处 allow 后无 too_many_arguments**

Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings`
Expected: 通过。此刻 search.rs 仅 `search` 命令一处仍带 `#[allow(clippy::too_many_arguments)]`（Task 3 收）。

- [ ] **Step 13: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "refactor(desktop): search_impl/handle_file_action 改吃 &SearchDeps + 测试调用点迁移(Task2)"
```

---

## Task 3: 收拢命令层 + main.rs + status.rs（green，去最后 1 处 allow）

**目标**：`main.rs` 改为单一 `.manage(deps)`；`search`/`confirm_action`/`cancel_action` 命令改 `State<SearchDeps>`；`confirm_action_impl` 改 `&SearchDeps`；`status::get_backend_status` 经 `State<SearchDeps>` + `registry()`。

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`
- Modify: `apps/desktop/src-tauri/src/status.rs`

- [ ] **Step 1: 改 `search` 命令为 State<SearchDeps>（去最后一处 allow）**

把（约 line 86–109）整个 `search` 命令替换为：

```rust
/// 主搜索 command:thin wrapper,解 State 后委托 [`search_impl`]。
#[tauri::command]
pub async fn search(
    query: String,
    on_event: Channel<SearchEvent>,
    deps: tauri::State<'_, SearchDeps>,
) -> Result<(), String> {
    search_impl(query, on_event, deps.inner()).await
}
```

- [ ] **Step 2: 改 `confirm_action_impl` 为 `&SearchDeps`**

把（约 line 522–527）：

```rust
fn confirm_action_impl(
    pending: &Arc<Mutex<Option<locifind_search_backend::FileAction>>>,
    file_action_tool: &Arc<locifind_harness::file_action_tool::FileActionTool>,
    tracer: &Arc<locifind_harness::Tracer>,
    context: &Arc<Mutex<ContextMemory>>,
) -> Result<ActionDoneData, String> {
```

改为：

```rust
fn confirm_action_impl(deps: &SearchDeps) -> Result<ActionDoneData, String> {
```

体内引用改：`pending` → `deps.pending`，`file_action_tool` → `deps.file_action_tool`，`tracer` → `deps.tracer`，`context` → `deps.context`。（即 `pending.lock()` → `deps.pending.lock()`、`file_action_tool.id()` → `deps.file_action_tool.id()`、`file_action_tool.invoke(...)` → `deps.file_action_tool.invoke(...)`、`tracer.on_*` → `deps.tracer.on_*`、`context.lock()` → `deps.context.lock()`。）

- [ ] **Step 3: 改 `confirm_action` 命令为 State<SearchDeps>**

把（约 line 584–592）：

```rust
#[tauri::command]
pub async fn confirm_action(
    pending: tauri::State<'_, Arc<Mutex<Option<locifind_search_backend::FileAction>>>>,
    file_action_tool: tauri::State<'_, Arc<locifind_harness::file_action_tool::FileActionTool>>,
    tracer: tauri::State<'_, Arc<locifind_harness::Tracer>>,
    context: tauri::State<'_, Arc<Mutex<ContextMemory>>>,
) -> Result<ActionDoneData, String> {
    confirm_action_impl(&pending, &file_action_tool, &tracer, &context)
}
```

改为：

```rust
#[tauri::command]
pub async fn confirm_action(
    deps: tauri::State<'_, SearchDeps>,
) -> Result<ActionDoneData, String> {
    confirm_action_impl(deps.inner())
}
```

- [ ] **Step 4: 改 `cancel_action` 命令为 State<SearchDeps>**

`cancel_action_impl(pending: &Arc<...>)` **签名不动**。把（约 line 600–606）`cancel_action` 命令改为：

```rust
#[tauri::command]
pub async fn cancel_action(deps: tauri::State<'_, SearchDeps>) -> Result<(), String> {
    cancel_action_impl(&deps.pending);
    Ok(())
}
```

> `deps.pending` 字段私有，但 `cancel_action` 与 `SearchDeps` 同在 search.rs 模块，可直接访问。

- [ ] **Step 5: 更新 `confirm_action_impl(...)` 测试调用点**

Run: `grep -n 'confirm_action_impl(' apps/desktop/src-tauri/src/search.rs`

每处 `confirm_action_impl(&pending, &tool, &tracer, &ctx)` 改为先建 SearchDeps 再传 `&deps`：

```rust
        let deps = SearchDeps::new(
            empty_registry_arc(),
            Arc::new(PolicyEngine::new()),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        );
        let res = confirm_action_impl(&deps).unwrap();
```

> confirm_action_impl 不读 registry/policy，用 `empty_registry_arc()`（Task2 Step9b 已加）+ 默认 policy 填。`pending` / `tool` / `calls` 等句柄继续 `Arc::clone` 喂入并保留，供调用后断言（如 `pending.lock().unwrap().is_none()`）。`confirm_action_no_pending_errs`、`confirm_action_executes_and_clears_pending`、`confirm_action_invoke_error_maps_to_err_and_traces` 三处均按此改。

- [ ] **Step 6: 改 main.rs — 单一 manage**

把 `main()`（约 line 125–132）的 6 个 `.manage()` 收为构造 + 单个 manage。在 `tauri::Builder::default()` 之前插入：

```rust
    let deps = search::SearchDeps::new(
        registry,
        policy,
        tracer,
        context,
        file_action_tool,
        pending_action,
    );
```

并把：

```rust
        .manage(registry)
        .manage(policy)
        .manage(tracer)
        .manage(context)
        .manage(file_action_tool)
        .manage(pending_action)
```

替换为单行：

```rust
        .manage(deps)
```

> `build_registry()` / `PolicyEngine::new()` / `build_tracer()` / `ContextMemory` / `FileActionTool` / `pending_action` 的构造（line 113–123）保持不变，只是它们的 `Arc` 现在被 `SearchDeps::new` 接管。`SearchDeps` 需在 main.rs 可见：`search` 模块已 `mod search;`，用 `search::SearchDeps`（结构体与 `new` 为 `pub`）。

- [ ] **Step 7: 改 status.rs — get_backend_status 经 SearchDeps**

把 `status.rs` 整体改为：

```rust
use locifind_harness::{BackendSummary, CapabilityDiscovery};
use tauri::State;

/**
 * 获取所有后端工具的状态摘要。
 *
 * 供前端状态栏显示各搜索后端的可用性、实现状态(Real/Stub)等。
 * registry 经 SearchDeps 取用(单一 managed 状态)。
 */
#[tauri::command]
pub fn get_backend_status(deps: State<'_, crate::search::SearchDeps>) -> Vec<BackendSummary> {
    let discovery = CapabilityDiscovery::new(deps.registry());
    discovery.backend_summary()
}
```

> 移除原 `ToolRegistry` 的 import（不再直接用）。`deps.registry()` 返回 `&ToolRegistry`，正合 `CapabilityDiscovery::new` 的入参。

- [ ] **Step 8: 加 status 测试验证经 SearchDeps 能拿到 registry**

在 `status.rs` 末尾加：

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use locifind_harness::{PolicyEngine, ToolRegistry, Tracer};
    use locifind_harness::file_action_tool::{FileActionTool, LocalFileActionExecutor};
    use locifind_harness::context::ContextMemory;
    use std::sync::{Arc, Mutex};

    #[test]
    fn get_backend_status_reads_registry_via_searchdeps() {
        // 空 registry → 摘要为空但不 panic，证明 SearchDeps::registry() 路径打通
        let deps = crate::search::SearchDeps::new(
            Arc::new(ToolRegistry::new()),
            Arc::new(PolicyEngine::new()),
            Arc::new(Tracer::with_hooks(vec![])),
            Arc::new(Mutex::new(ContextMemory::new())),
            Arc::new(FileActionTool::new(Arc::new(LocalFileActionExecutor), PolicyEngine::new())),
            Arc::new(Mutex::new(None)),
        );
        let summaries = CapabilityDiscovery::new(deps.registry()).backend_summary();
        assert!(summaries.is_empty(), "空 registry 摘要应为空, 实得 {summaries:?}");
    }
}
```

> 本测试直接调 `confirm_action_impl` 之外的纯逻辑（`CapabilityDiscovery` + `registry()`），不构造 Tauri `State`（`State` 无公开构造），等价覆盖命令体内的取值路径。

- [ ] **Step 9: 编译 + 全测试**

Run: `cargo test -p locifind-desktop`
Expected: 全 PASS（45 + status 新增 1 = 46）。若 `main.rs` 报 `deps` move/borrow 问题或命令注册类型不符，按报错调整。

- [ ] **Step 10: clippy 确认零 too_many_arguments**

Run: `cargo clippy -p locifind-desktop --all-targets -- -D warnings`
Expected: 通过。

Run: `grep -n 'too_many_arguments' apps/desktop/src-tauri/src/*.rs`
Expected: **无输出**（3 处 `allow` 全部移除，未新增）。

- [ ] **Step 11: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/status.rs
git commit -m "refactor(desktop): 命令层收口 SearchDeps + main 单一 manage + 修 get_backend_status registry 依赖(Task3)"
```

---

## Task 4: 全量验证 + evals byte-equal

**Files:** 无源改动（纯验证）

- [ ] **Step 1: 全套 CI**

Run: `bash scripts/ci.sh`
Expected: fmt + clippy(-D warnings) + build + test 全过。

- [ ] **Step 2: 确认零 too_many_arguments 抑制**

Run: `grep -rn 'too_many_arguments' apps/desktop/src-tauri/src/`
Expected: 无输出。

- [ ] **Step 3: evals parser-only byte-equal（evals 不依赖 desktop crate，应维持）**

Run: `cargo run -p locifind-evals --bin evals 2>/dev/null | tail -20`
Expected: pass / partial / fail 维持 **472 / 26 / 2**（与第 32 阶段 parser-only baseline 一致）。

> 若命令名/参数与本仓不符，参照 `ROADMAP` PROTO-08 验收里的 evals 调用方式（`cargo run -p locifind-evals --bin evals`）。本步只读对照，不改任何东西。

- [ ] **Step 4: 收工前不提交**（STATUS/ROADMAP 同步与最终 commit 走"收工"流程，由主会话在用户说"收工"时执行）

---

## Self-Review 结论（计划作者自查）

- **Spec 覆盖**：§2 结构体 → Task1；§3 函数签名表 → Task2（search_impl/handle_file_action/handle_confirmable_action）+ Task3（命令层 + confirm_action_impl + cancel_action）；§4 main.rs + get_backend_status → Task3 Step6/7/8；§5 测试与验证 → Task2/3 各 Step + Task4；§6 行为等价 → Task4 CI + evals 对照。无遗漏。
- **Placeholder**：无 TBD/TODO；所有改动给出确切前后代码或 grep 定位 + 统一模式 + 编译器兜底（~40 机械等价调用点）。
- **类型一致**：`SearchDeps` / `SearchDeps::new` / `registry()` / `deps.inner()` 在 Task1→3 一致；`empty_registry_arc` 在 Task2 Step9b 定义、Task3 Step5 复用。
- **抑制账**：Task2 去 search_impl + handle_file_action 两处；Task3 去 search 命令一处；`new()` 6 参不触发、不新增 → 终态 0 处（Task3 Step10 + Task4 Step2 grep 验证）。
