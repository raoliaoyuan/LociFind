# FileAction(open/locate)多轮接入 Tauri search command — 设计

> 日期:2026-05-29
> 阶段:M4(MVP-19+ 后续 wiring)/ Class B 代码层 backlog
> 作者:Claude Code (Opus 4.8)
> 承接:第 30 阶段 ContextMemory Refine 合并接入(`docs/superpowers/specs/2026-05-29-context-memory-refine-wiring-design.md`)

## 1. 背景与目标

第 30 阶段把 `ContextMemory::apply_refine` 接进了 Tauri `search` command,打通了「渐进收窄」的 Refine 多轮链。但 `ContextMemory` 的另一条能力 —— `resolve_target_ref`(把 `TargetRef::LastResults { selector }` 解析成上一轮结果的路径)—— 尚未接线。

本阶段接入 **`SearchIntent::FileAction` 中 `open` / `locate` 两个只读型动作**,让用户在一次搜索之后,用自然语言「打开第 2 个」/「在访达里显示第 3 个」直接对上一轮结果执行操作。

### 范围(明确边界)

- **只接 `open` / `locate`**(Policy L3 / L1 → Allow,无需确认流)。
- **不接 `copy` / `move` / `rename`**(L4 → RequireConfirmation,需确认对话框 + 重新下发协议,留后续阶段)。
- **`delete` 维持 schema + Policy 双重硬禁用**,不接。
- harness `FileActionTool` / `ContextMemory` 源**一行不动**:`FileActionTool::invoke` 已完整实现 target_ref 解析 + Policy 校验 + 执行。本阶段是纯 Tauri 桥接层 wiring。

### 现状缺口

当前 `search_impl`(`apps/desktop/src-tauri/src/search.rs`)处理 `FileSearch` / `MediaSearch` / `Refine`。当 query 解析为 `FileAction` 时,会落到 `IntentRouter::route_search`,因 FileAction 不是 search-typed intent 而失败,以 `SearchEvent::Error` 死路结束。`FileActionTool` 也从未在 `build_registry` 注册。

### 前提验证(已确认)

- parser `try_parse_file_action`(`packages/intent-parser/src/parsers/file_action.rs`)对以下 query 稳定产出 `FileAction(Open/Locate, target_ref: LastResults{...})`:
  - Open:`打开第2个` / `打开第二个` / `open the 2nd one` / `open the second`
  - Locate:`在访达里显示第3个` / `show in finder ...` / `reveal ...` / `in finder`
  - All selector:`打开这些` / `open all of them`
- Policy 分级(`packages/harness/src/policy.rs`):Locate=L1→Allow、Open=L3→Allow、Copy/Move/Rename=L4→RequireConfirmation、Delete=L5→Deny。

## 2. 架构总览

沿用第 30 阶段「`search_impl` 内联分支」方案。理由:UI 只有一个搜索框,query 在 Rust 端 parse,所有意图必然经唯一的 `search` command —— 无法在 UI 端提前分流到独立 command。

```
query
  → resolve_intent → intent
  → apply_refine_if_needed(intent, ctx) → effective        // 第 30 阶段已有
  → match effective:
        FileAction(Open | Locate)            → handle_file_action  ── ActionDone 事件
        FileAction(Copy/Move/Rename/Delete)  → SearchEvent::Error「暂不支持」(pre-tool,无 trace)
        FileSearch / MediaSearch             → 现有 search 路径(policy → route_search → stream → record)
        其余(Clarify 等)                     → 现有路径(route_search 失败 → Error)
```

分支点在 `effective` 计算完成之后(refine 合并之后)、现有 search policy gate **之前** —— 因为 `FileActionTool::invoke` 内部自带 Policy 校验,不能让 search 路径的 policy gate 再跑一遍。

### State 注入

`main.rs` 新增 `Arc<FileActionTool>` managed State:

```rust
let file_action_tool = Arc::new(FileActionTool::new(
    Arc::new(LocalFileActionExecutor),
    PolicyEngine::new(),
));
// .manage(file_action_tool)
```

`FileActionTool::new` 默认即 open/locate/copy/move/rename(不含 delete),自带 batch_threshold=10。本阶段虽只放行 open/locate,但 tool 能力声明不变(scope gate 在 `handle_file_action`,见 §3)。

### 改动文件

- `apps/desktop/src-tauri/src/search.rs`:新增 `handle_file_action` + `SearchEvent::ActionDone` 变体 + 分支 + 测试。
- `apps/desktop/src-tauri/src/main.rs`:`.manage(Arc<FileActionTool>)`。
- `apps/desktop/src/SearchView.tsx`:`SearchEvent` 类型加 `action_done` + 一个渲染分支(唯一 TS 改动)。

## 3. 关键安全 gate:按动作类型拦截

**必须显式处理的安全点**:parser 对 copy/move/rename 预设 `requires_confirmation = true`(file_action.rs:44/47/50)。而 `FileActionTool::invoke` 的逻辑是:

```
PolicyDecision::RequireConfirmation if !action.requires_confirmation => 返回 RequiresConfirmation
PolicyDecision::Allow | PolicyDecision::RequireConfirmation => {}  // 继续执行
```

即:`requires_confirmation == true` 时,L4 动作会**跳过返回确认、直接执行**。若把 copy/move/rename 无脑丢给 `invoke`,会**绕过尚未实现的 UI 确认流直接执行文件操作**。

因此 `handle_file_action` 第一步即按 `action.action` 类型 gate:

```rust
match action.action {
    FileActionKind::Open | FileActionKind::Locate => { /* 放行,见下 */ }
    _ => {
        // copy/move/rename/delete:本阶段一律拒绝,绝不进 invoke
        eprintln!("search: file_action 暂不支持: {:?}", action.action);
        let _ = on_event.send(SearchEvent::Error {
            message: "该操作暂不支持(确认流待后续阶段)".to_owned(),
        });
        return Ok(());
    }
}
```

这是本阶段的安全底线与 scope 边界硬约束。

## 4. handle_file_action 主体

签名(可被单测注入 mock):

```rust
async fn handle_file_action(
    action: FileAction,
    on_event: &Channel<SearchEvent>,
    file_action_tool: Arc<FileActionTool>,
    tracer: Arc<Tracer>,
    context: Arc<Mutex<ContextMemory>>,
) -> Result<(), String>
```

流程:

1. **scope gate**(§3):非 open/locate → Error,return(pre-tool,不进 trace)。
2. **tool_call trace**:`tracer.on_tool_call(ToolCallEvent { tool_id, ToolKind::FileAction, SupportedIntent::FileAction })`。
3. **invoke**(持 context **不可变** guard):
   ```rust
   let guard = context.lock().unwrap_or_else(|e| e.into_inner());
   file_action_tool.invoke(&action, &guard)
   ```
4. 结果分发:
   - `Ok(FileActionOutcome::Executed { affected })` → `tracer.on_tool_result` + `SearchEvent::ActionDone { action_kind, paths }`。
   - `Ok(FileActionOutcome::RequiresConfirmation { .. })` → open/locate 永不触发(Allow),理论不可达;防御性转 Error「该操作暂不支持」+ `eprintln`。
   - `Err(e)` → `tracer.on_error` + `SearchEvent::Error { friendly_message(e) }`。

`friendly_message` 把 `FileActionError` 映射成中文友好文案:

| `FileActionError` | 文案 |
|---|---|
| `TargetRef(NoLastResults)` | 没有可操作的上一轮搜索结果,请先发起一次搜索 |
| `TargetRef(IndexOutOfRange{requested,available})` | 第 {requested} 个结果不存在(上一轮共 {available} 条) |
| `TargetRef(EmptyIndices)` | 未指定要操作的结果序号 |
| `EmptyTargets` | 没有可操作的目标 |
| `Executor(io)` | 操作失败:{io} |
| 其余(DeleteNotSupported / PolicyDenied / Batch / MissingDestination / MissingNewName / PathConflict) | open/locate 不可达;兜底用 `e.to_string()` |

**ContextMemory 语义**:`handle_file_action` 只读 context(不可变 guard),**不 record、不 clear**。action 无搜索结果,record 会误清「上一轮」链;只读保证「打开第2个」后再「打开第3个」引用同一次搜索基准。

## 5. SearchEvent::ActionDone

```rust
/// 文件操作(open/locate)执行完成。
ActionDone {
    /// 动作类型:"open" | "locate"。
    action_kind: String,
    /// 实际涉及的绝对路径。
    paths: Vec<String>,
},
```

`action_kind` 由 `FileActionKind` 小写化(`format!("{:?}").to_lowercase()`,与现有 source/match_type 序列化风格一致)。

UI 渲染(`SearchView.tsx`):新增 `action_done` 分支,显示一行反馈:

- open + 1 path:`已打开 {basename}`
- open + N path:`已打开 {N} 个文件`
- locate + 1 path:`已在访达中显示 {basename}`
- locate + N path:`已在访达中显示 {N} 个文件`

(平台无关文案;Windows 上「访达」措辞后续 i18n 处理,本阶段沿用既有中文 UI。)

## 6. Tracing(与 search 路径对齐)

- **可路由的 open/locate**:`tool_call`(ToolKind::FileAction)→ invoke → 成功 `tool_result`(result_count = affected.len())/ 失败 `on_error`(error_type = FileActionError variant 名)。
- **pre-tool 失败**(非 open/locate 动作 → §3 scope gate):不进 trace,仅 `eprintln` + Error —— 与第 28/30 阶段「pre-tool failure 不进 Tracer」语义一致。
- 需要一个 `file_action_error_kind(&FileActionError) -> &'static str` helper(类比现有 `search_error_kind`),返回不含路径 detail 的 variant 名,供 trace 用。

## 7. 测试计划

新增 `MockFileActionExecutor`(desktop crate test mod),记录 open/locate/copy/move/rename 调用与路径。

### 单元测试(handle_file_action)

| 测试 | 断言 |
|---|---|
| open 执行 | MockExecutor 收到 open(results[0].path);事件含 `action_done` + action_kind=open |
| locate 执行 | MockExecutor 收到 locate;事件含 `action_done` + action_kind=locate |
| copy 转 Error 不执行 | MockExecutor **未**收到任何调用;事件含 error「暂不支持」;无 trace |
| target_ref 越界 | 事件含 error 友好文案;trace 有 call+error |
| 无上一轮结果 | 事件含 error「请先发起一次搜索」;trace 有 call+error |

### 集成测试(record-then-action,复用 `FakeOkBackend` + capturing 思路)

| 测试 | 断言 |
|---|---|
| 搜索后 open 第 1 个 | `find pdf` 记录 2 条 → `打开第1个` → executor.open 收到 results[0].path |
| action 不动 context | 上述之后再 `打开第2个` → 仍成功(executor.open 收到 results[1].path),证 context 未被 record/clear |

### 真机手测(用户驱动,agent 无法点 Tauri 窗口)

1. `find pdf` → 有结果 → `打开第1个` → 真打开应用 + UI 显示「已打开 ...」。
2. `在访达里显示第2个` → 真跳访达高亮 + UI 显示「已在访达中显示 ...」。
3. `打开第99个` → 友好越界错误。
4. 重启后首查 `打开第1个`(无上下文)→ 友好「请先发起一次搜索」+ trace 0 行。

## 8. 不回归约束

- evals 不依赖 desktop crate,parser/harness 源不动 → v0.5 parser-only **维持 472 / 26 / 2**、hybrid Q4_K_M **维持 480 / 18 / 2**(byte-equal)。
- `bash scripts/ci.sh` 全过(fmt + clippy -D warnings + build + test)。
- desktop crate 现有 15 测试不破;新增约 7 测试(5 单元 + 2 集成)。

## 9. 风险与缓解

| 风险 | 缓解 |
|---|---|
| copy/move/rename 被误执行(parser 预设 requires_confirmation=true) | §3 scope gate 在 invoke **之前**按动作类型硬拦,copy/move/rename/delete 绝不进 invoke |
| action 误 record 清空 refine 链 | §4 只读 context,不 record/clear;集成测试验证连续 action 引用同一基准 |
| RequiresConfirmation 理论不可达分支处理不当 | §4 防御性转 Error,不静默执行 |
| clippy --all-targets 对未用代码报错(第 30 阶段教训) | Task 拆分时确保 `handle_file_action` 在拆分中途若为 dead code,用 `#[cfg_attr(not(test), allow(dead_code))]` 暂存,接线 task 移除 |

## 10. 交付物

- `search.rs`:`handle_file_action` + `SearchEvent::ActionDone` + `friendly_message` + `file_action_error_kind` + 分支 + 5 单元 + 2 集成测试 + `MockFileActionExecutor`。
- `main.rs`:`.manage(Arc<FileActionTool>)` + 测试(可选验证 state 构造)。
- `SearchView.tsx`:`action_done` 类型 + 渲染分支。
- 后续 backlog 更新:Class B 移除「FileAction target_ref 多轮接入」,新增「FileAction copy/move/rename 确认流(L4 往返协议 + 确认对话框)」。
