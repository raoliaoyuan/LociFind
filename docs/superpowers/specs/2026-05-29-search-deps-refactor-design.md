# SearchDeps 依赖收拢重构 — 设计

> 日期：2026-05-29
> 作者：Claude Code (Opus 4.8)
> 阶段：M（MVP）/ Class B 代码层 backlog
> 类型：纯结构重构（行为等价，唯一例外见 §4 get_backend_status）

## 1. 背景与目标

`apps/desktop/src-tauri/src/search.rs` 在第 30–32 阶段连续接入 ContextMemory、FileAction(open/locate)、FileAction(copy/move/rename) 确认流后，核心函数的依赖参数持续膨胀：

- `search`（command）：8 个参数（含 6 个 `tauri::State<Arc<…>>`）
- `search_impl`：8 个参数（6 个 `Arc<…>`）
- `handle_file_action`：6 个参数

为此用了 **3 处 `#[allow(clippy::too_many_arguments)]`**（search.rs:87 / 115 / 326）。每次新增一个共享依赖都要同时改 `search` 命令签名、`search_impl` 签名、`main.rs` 的 `.manage()` 调用、所有测试调用点——改动面随依赖数线性膨胀。

**目标**：把 6 个共享依赖收拢成一个 `SearchDeps` 结构体，作为单一 `tauri::manage` 注入。达成：

1. 消除全部 3 处 `too_many_arguments` 抑制。
2. 下次新增共享依赖只改一处（`SearchDeps` 定义 + 用到它的大函数体），不再触动命令签名与 `main.rs` 的 manage 列表。
3. 每个函数签名诚实反映"我依赖什么"（按真实依赖粒度，不强行让小函数吃整个 bundle）。

**非目标**：不动 harness/parser/evals 源；不动前端 TS；不改任何搜索/文件操作的业务逻辑；不做多目标支持等功能性扩展。

## 2. SearchDeps 结构与构造

在 `search.rs` 新增：

```rust
/// 收拢 search/confirm/cancel 命令共享的 6 个进程级依赖，
/// 作为单一 tauri::manage 状态注入。新增共享依赖只需改本结构体 + new() + 用到的大函数体。
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
        Self { registry, policy, tracer, context, file_action_tool, pending }
    }

    /// 只读访问 registry，供同 crate 的 status::get_backend_status 使用（见 §4）。
    pub(crate) fn registry(&self) -> &ToolRegistry {
        &self.registry
    }
}
```

设计要点：

- **字段私有**。`search.rs` 内部函数与 `mod tests`（子模块）可直接读 `deps.registry` 等；`main.rs` 只用 `SearchDeps::new()` 不读字段；`status.rs`（同 crate 不同模块）经 `pub(crate) fn registry()` 访问器取用。
- **持有 6 个 `Arc`**，clone 成本低（仅引用计数）。本重构内部函数一律用 `&SearchDeps` 借用，不 clone。
- **`new()` 不需要任何抑制**：clippy `too_many_arguments` 默认阈值为 7（仅在 **8+ 参**时触发）。`new()` 是 6 参，不触发；`search` / `search_impl`（8 参）才是真正触发的两处。`handle_file_action`（6 参）现有的 `allow` 是防御性的、本就不必要。本重构最终抑制数 **3 → 0**（移除 search / search_impl / handle_file_action 三处 `allow`，且不新增任何 `allow`）。

## 3. 函数签名（按真实依赖粒度）

| 函数 | 改后签名 | 参数数 | 抑制 |
|---|---|---|---|
| `search`（cmd） | `(query: String, on_event: Channel<SearchEvent>, deps: State<'_, SearchDeps>)` | 3 | ✅ 消除 |
| `search_impl` | `(query: String, on_event: Channel<SearchEvent>, deps: &SearchDeps)` | 3 | ✅ 消除 |
| `handle_file_action` | `(action: FileAction, on_event: Channel<SearchEvent>, deps: &SearchDeps)` | 3 | ✅ 消除（含原防御性 allow） |
| `confirm_action_impl` | `(deps: &SearchDeps) -> Result<ActionDoneData, String>` | 1 | — |
| `confirm_action`（cmd） | `(deps: State<'_, SearchDeps>) -> Result<ActionDoneData, String>` | 1 | — |
| `handle_confirmable_action` | `(action, on_event, pending: &Arc<Mutex<Option<FileAction>>>, context: &Arc<Mutex<ContextMemory>>)` | 4（窄） | — |
| `cancel_action_impl` | `(pending: &Arc<Mutex<Option<FileAction>>>)` | 1（窄） | — |
| `cancel_action`（cmd） | `(deps: State<'_, SearchDeps>)`，体内 `cancel_action_impl(&deps.pending)` | 1 | — |

设计要点：

- **大函数吃 `&SearchDeps`**：`search_impl` / `handle_file_action` / `confirm_action_impl`。命令薄壳解 `State<'_, SearchDeps>` 后用 `deps.inner()` 取 `&SearchDeps` 传入（`&deps` 是 `&State`，需 `deps.inner()` 或 `&*deps` 才是 `&SearchDeps`）。
- **小函数维持窄签名**：`handle_confirmable_action`（只碰 pending + context）、`cancel_action_impl`（只碰 pending）保留显式窄参数（由 owned `Arc` 改为 `&Arc`），诚实反映真实依赖。调用方从 `&SearchDeps` 取子字段传入：`handle_confirmable_action(action, on_event, &deps.pending, &deps.context)`。
- **借用安全**：全程内联 `await`，无 `tokio::spawn`/无 `'static` 要求；`&SearchDeps` 借用贯穿各 await 点成立（`State` guard 在命令栈帧持有）。
- `search_impl` 体内原先对各 `Arc` 的用法改为经 `deps.<field>`：
  - `IntentRouter::new(&deps.registry)`
  - `deps.policy.evaluate(...)`、`deps.tracer.on_tool_call(...)`、`deps.context.lock()`、`deps.pending` 等。

## 4. main.rs 与 get_backend_status

### main.rs

`main()` 把 6 个 `.manage()` 替换为一个：

```rust
let deps = search::SearchDeps::new(registry, policy, tracer, context, file_action_tool, pending_action);
// ...
.manage(deps)
```

`build_registry()` / `build_tracer()` 不变；`build_registry_exposes_real_spotlight_on_macos` 测试直接调 `build_registry()`，不受影响。

### get_backend_status（顺手修正）

发现：`status::get_backend_status` 当前签名是 `get_backend_status(registry: State<'_, ToolRegistry>)`，取**裸 `ToolRegistry`**；而 `main.rs` 管理的是 `Arc<ToolRegistry>`。Tauri State 按精确 `TypeId` 查找，`ToolRegistry` ≠ `Arc<ToolRegistry>`，故该命令今天实际拿不到被管理的 registry（invoke 应失败，被前端 `StatusIndicator.tsx` 的 catch 吞掉，状态栏静默退化）。这是先于本重构存在的潜在 bug。

由于本重构正好移除 `.manage(registry)`（registry 收进 SearchDeps），顺手把 `get_backend_status` 改为经 SearchDeps 取 registry：

```rust
#[tauri::command]
pub fn get_backend_status(deps: tauri::State<'_, crate::search::SearchDeps>) -> Vec<BackendSummary> {
    let discovery = CapabilityDiscovery::new(&deps.registry); // &Arc<ToolRegistry> deref-coerce 到 &ToolRegistry
    discovery.backend_summary()
}
```

效果：把"类型不匹配、拿不到 registry"转为"经 SearchDeps 确定拿到 registry"，状态栏真正可用。这是在本重构变动路径上的最小修正，不扩大 scope。

> `deps.registry` 字段私有，但 `status.rs` 与 `search.rs` 同 crate 不同模块——需让 `get_backend_status` 不直接读私有字段。两种落地：
> (a) 在 `SearchDeps` 上加 `pub(crate) fn registry(&self) -> &ToolRegistry`（或 `&Arc<ToolRegistry>`）只读访问器；
> (b) 把 `registry` 字段改 `pub(crate)`。
> 选 **(a) 访问器**：保持其余字段封装，只暴露 status 真正需要的只读视图。实现阶段据此加 `SearchDeps::registry(&self)`。

## 5. 测试与验证

### search.rs tests（约 40 调用点）

- 每个 test 仍构造原本的 mock `Arc`（`registry` / `policy` / `tracer` / `ctx` / `tool` / `pending` 等），改为 `SearchDeps::new(...)` 后向 `search_impl` / `handle_file_action` / `confirm_action_impl` 传 `&deps`。
- 断言仍用各 test 自己保留的 `Arc` 句柄（如 `ctx`、`pending`），不读 `deps` 字段。
- `handle_confirmable_action` / `cancel_action_impl` 调用点维持窄签名，仅把 owned `Arc` 改 `&`（`&pending` / `&ctx`）。
- 这些是等价改写，不增减测试数量。

### status.rs test

- 加（或调整）1 个测试点验证 `get_backend_status` 经 `SearchDeps` 能拿到 registry（构造含真实/stub registry 的 `SearchDeps`，断言 backend_summary 非空或含预期 id）。desktop crate 测试数 44 → 约 45。

### main.rs test

- 不受影响（`build_registry_*` 直接调 `build_registry()`）。

### 验证门

- `bash scripts/ci.sh` 全过：fmt + `clippy -D warnings`（**关键：search_impl/handle_file_action/search 在无 `allow` 下不得报 `too_many_arguments`**）+ build + test。
- evals 不依赖 desktop crate；顺带跑 v0.5 parser-only 确认维持 **472 / 26 / 2** byte-equal。

## 6. 影响面与风险

**净改动文件**：`apps/desktop/src-tauri/src/{search.rs, main.rs, status.rs}`。无 TS 改动；无 harness/parser/evals 源改动。

**行为等价性**：除 `get_backend_status` 从"拿不到 registry"转为"拿到 registry"外，搜索 / 文件操作 / 确认流 / 多轮 context 行为完全不变。第 30–32 阶段真机验收过的 5 路径不受影响（无需重新真机验收，但 CI 全测覆盖）。

**风险**：

| 风险 | 缓解 |
|---|---|
| `&SearchDeps` 借用跨 await 生命周期不成立 | 全程内联 await、无 spawn；编译期由 borrow checker 兜底，CI build 即验证 |
| `get_backend_status` 改签名后前端 invoke 协议变化 | 命令名/返回类型不变（仍 `Vec<BackendSummary>`），仅参数注入方式变，对前端透明 |
| 测试调用点遗漏改造导致编译失败 | 编译期全暴露；逐文件 `cargo test -p` 验证 |
| `confirm_action_impl` 改吃 `&SearchDeps` 后，confirm/cancel 测试需多构造 registry/policy | 用 `ToolRegistry::new()` + `PolicyEngine::new()` 空默认填充，构造成本低；符合 §3 表"big 函数吃 &SearchDeps"的既定取舍 |

## 7. 后续 backlog（不在本次 scope）

- copy/move/rename 多目标支持（方案 A 改 harness）。
