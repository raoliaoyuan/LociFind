# Tracing/Hooks 接入 Tauri search command — 设计 spec

> 阶段：M 阶段 backlog（Class B 代码层），承接 MVP-19+ Slice B 后续。
> 日期：2026-05-28
> 作者：Claude Code (Opus 4.7)

## 1. 背景与目标

MVP-08 已经把 `Tracer` / `TracingHook` / `JsonLinesHook` / `NoopHook` 落在 `packages/harness/src/tracing.rs`，单测齐全。但 MVP-19+ Slice B 把 Tauri `search` command 升级到走 `ToolRegistry + PolicyEngine + IntentRouter` 之后，**search 路径还没接 Tracer** — Tauri 后端调用 backend 的全过程对开发者不可观测，只能靠 `eprintln!` 抓个别错误。

本 wiring task 的目标：在 Tauri `search` 路径上加 `Tracer`，让开发/调试时可以用环境变量开启 JSONL 追踪文件，记录每次 tool 调用的 (id / variant / 耗时 / 结果数 / 错误类型)。

**用途定位**（已与用户对齐）：仅用于 **开发/调试观测**。不进 UI、不做用户可观测、不做产品遥测。

## 2. Scope

### In scope

- `apps/desktop/src-tauri/src/main.rs`：新增 `build_tracer()` 读环境变量 `LOCIFIND_TRACE`，构造 `Arc<Tracer>` 注入 Tauri State
- `apps/desktop/src-tauri/src/search.rs`：在 3 个 tool 层时点触发 Tracer 事件（call / result / error）
- 单元测试：`build_tracer` 默认 noop + 非法路径 fallback；search command MockHook 覆盖 success / error 路径

### Out of scope

- **不动 `Tracer` schema**（`ToolCallEvent` / `ToolResultEvent` / `ToolErrorEvent`）
- **不引 `tracing` crate**（项目当前用 `eprintln!` 写 stderr，保持一致）
- **不接前端**（trace 只写后端文件，UI 不感知）
- **不接 ContextMemory / refine 多轮**（另一项 backlog）
- **不接 pre-tool 失败**（intent 解析失败 / policy denied / router 不可路由）— 这三类沿用 `eprintln!`，开发者看 Tauri stdout 即可

## 3. 架构概览

```
┌────────────────────────────────────────────────────────────────┐
│  main()                                                        │
│    build_tracer() ──┐                                          │
│                     ├── manage(Arc<Tracer>)                    │
│    build_registry() ┤                                          │
│    PolicyEngine     │                                          │
│                     └── search command 通过 State 拿到三者     │
└────────────────────────────────────────────────────────────────┘

Tauri search command lifecycle（已有 vs 新增 trace 点）:

  ┌─ resolve_intent ─┐   ┌─ policy ─┐   ┌─ route_search ─┐
  │     [pre-tool]   │   │ [pre-]   │   │   [pre-tool]   │
  │  err→eprintln+   │   │ err→     │   │  err→eprintln+ │
  │      SearchEvent │   │ ......   │   │      Search... │
  └──────────────────┘   └──────────┘   └────────────────┘
                                              │
                              tool_start = Instant::now()
                              ┌─────────────────▼───────────────┐
                              │ TRACE A: on_tool_call           │
                              │   { tool_id, kind, variant }    │
                              └─────────────────┬───────────────┘
                                                │
                                       tool.search(...).await
                                       /                    \
                              open err                    stream Ok
                                /                              \
                  ┌─────▼────────────┐              ┌──────────▼────────────┐
                  │ TRACE C: on_error │              │  stream.next() loop  │
                  │ (duration,        │              │                       │
                  │  search_err_kind) │              │  per-item err:        │
                  └───────────────────┘              │  ┌──▼──────────────┐  │
                                                    │  │ TRACE C: on_err │  │
                                                    │  └─────────────────┘  │
                                                    │                       │
                                                    │  stream done:         │
                                                    │  ┌──▼─────────────┐   │
                                                    │  │ TRACE B:       │   │
                                                    │  │ on_tool_result │   │
                                                    │  │ (duration, n)  │   │
                                                    │  └────────────────┘   │
                                                    └───────────────────────┘
```

## 4. 接口与契约

### 4.1 `build_tracer()`（main.rs）

```rust
fn build_tracer() -> Arc<Tracer> {
    let path = std::env::var("LOCIFIND_TRACE").ok().filter(|s| !s.is_empty());
    let hooks: Vec<Box<dyn TracingHook>> = match path {
        None => vec![],
        Some(p) => match OpenOptions::new().create(true).append(true).open(&p) {
            Ok(f) => vec![Box::new(JsonLinesHook::new(f))],
            Err(err) => {
                eprintln!("LOCIFIND_TRACE 打开 {p} 失败 ({err}), tracing 禁用");
                vec![]
            }
        },
    };
    Arc::new(Tracer::with_hooks(hooks))
}
```

契约：
- 环境变量未设 / 为空字符串 → 0 hook（Tracer 实例存在，但 dispatch 是空循环）
- 环境变量设非空 path → 尝试 `OpenOptions { create=true, append=true }` 打开；成功挂 `JsonLinesHook<File>`；失败 fallback 0 hook + stderr warn
- 永远返回 `Arc<Tracer>`，不返回 `Option` / `Result` — 接入方不需要处理「Tracer 不存在」的分支

### 4.2 search command 签名

```rust
#[tauri::command]
pub async fn search(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: tauri::State<'_, Arc<ToolRegistry>>,
    policy: tauri::State<'_, Arc<PolicyEngine>>,
    tracer: tauri::State<'_, Arc<Tracer>>,  // 新增
) -> Result<(), String>
```

新增 `tracer` 参数与既有 `registry` / `policy` 同样的 `State` 模式。请求生命周期内 `Arc::clone` 出来跨 await 持有。

### 4.3 `search_error_kind` 辅助函数

```rust
fn search_error_kind(err: &SearchError) -> &'static str {
    match err {
        SearchError::BackendUnavailable { .. } => "BackendUnavailable",
        SearchError::PermissionDenied { .. } => "PermissionDenied",
        SearchError::InvalidIntent { .. } => "InvalidIntent",
        SearchError::UnsupportedIntent { .. } => "UnsupportedIntent",
        SearchError::Timeout { .. } => "Timeout",
        SearchError::Io { .. } => "Io",
    }
}
```

返回 variant 名，**不带 detail**（避免泄路径或敏感字符串）。

### 4.4 `intent_to_supported` 辅助函数

```rust
fn intent_to_supported(intent: &SearchIntent) -> SupportedIntent {
    match intent {
        SearchIntent::FileSearch(_) => SupportedIntent::FileSearch,
        SearchIntent::MediaSearch(_) => SupportedIntent::MediaSearch,
        SearchIntent::FileAction(_) => SupportedIntent::FileAction,
        SearchIntent::Refine(_) => SupportedIntent::Refine,
        SearchIntent::Clarify(_) => SupportedIntent::Clarify,
    }
}
```

若 `harness` / `locifind-search-backend` 已暴露同等映射则复用之；否则在 search.rs 内私有定义。**实施前先 grep 确认**。

## 5. trace 点 + payload schema

### Trace A — tool_call（成功 route_search 之后立刻发）

```rust
tracer.on_tool_call(&ToolCallEvent {
    tool_id: tool.id().to_owned(),
    tool_kind: ToolKind::Search,
    intent_variant: intent_to_supported(&resolved.intent),
});
let tool_start = Instant::now();
```

记 `tool_start` 在 `on_tool_call` 之后、`tool.search().await` 之前 — 让 duration 反映「从 tool 开始被调用算起」。

### Trace B — tool_result（stream 跑完发 Complete 前）

```rust
tracer.on_tool_result(&ToolResultEvent {
    tool_id: tool.id().to_owned(),
    duration: tool_start.elapsed(),
    result_count: total,
});
let _ = on_event.send(SearchEvent::Complete { total, elapsed_ms });
```

`elapsed_ms` 是从 command 入口算起，`tool_start.elapsed()` 仅 tool 调用窗口 — 二者不同：前者是 user-perceived total，后者是 backend pure cost。

### Trace C — tool_error（tool.search() open 或 stream item err）

```rust
// open err 分支：
Err(err) => {
    tracer.on_error(&ToolErrorEvent {
        tool_id: tool.id().to_owned(),
        duration: tool_start.elapsed(),
        error_type: search_error_kind(&err).to_owned(),
    });
    let _ = on_event.send(SearchEvent::Error { message: err.to_string() });
    return Ok(());
}

// stream mid 分支（per-item Err）同上
```

注意：mid-stream err 已发的部分 result 不回滚（与 SearchEvent::Error 现有契约一致）。

### 不进 Tracer 的失败位点

- `resolve_intent` 失败 → 沿用现有 `SearchEvent::Error`；额外加 `eprintln!("search: intent 解析失败: {err}")`
- `policy.evaluate()` `Deny` → 沿用 `SearchEvent::Error`；加 `eprintln!("search: policy 拒绝: {reason}")`
- `policy.evaluate()` `RequireConfirmation` → 沿用 `SearchEvent::Error`；加 `eprintln!("search: 不应触发 RequireConfirmation(intent 路由 bug)")`(RequireConfirmation 无 `reason` 字段,仅作防御性日志)
- `router.route_search()` 失败 → 沿用 `SearchEvent::Error`；加 `eprintln!("search: 无可用 tool: {err}")`

这三类没有 `tool_id`，且 ToolErrorEvent schema 要求 tool_id 必填 — 强行用占位符既破坏 trace consumer 语义又不增可观测价值（开发者直接看 stdout 即可）。

## 6. 配置与启停

| 环境变量 | 行为 |
|---|---|
| 未设 / 设为空字符串 | Tracer 持 0 hook，全程无 trace；性能等同未接入 |
| `LOCIFIND_TRACE=/tmp/locifind.jsonl` | append 模式打开文件；每条 trace 一行 JSON |
| `LOCIFIND_TRACE=/不存在/路径/...` | `eprintln!` warn + 退化为 0 hook（不 crash） |

JSONL 文件 schema（`JsonLinesHook::log` 已确定）：

```json
{"tag": "tool_call",   "data": {"tool_id": "search.spotlight", "tool_kind": "Search", "intent_variant": "FileSearch"}, "timestamp": "2026-05-28T..."}
{"tag": "tool_result", "data": {"tool_id": "search.spotlight", "duration": {"secs": 0, "nanos": 145000000}, "result_count": 23}, "timestamp": "..."}
{"tag": "tool_error",  "data": {"tool_id": "search.spotlight", "duration": {...}, "error_type": "Timeout"}, "timestamp": "..."}
```

Append 模式让多次启动累积到同一文件 — 开发者复现 bug 时不需要 rotate；如果文件过大开发者自己清。

## 7. 隐私边界

- Trace event schema 已天然不含 path / query / result body
- `error_type` 取 SearchError variant 名（一个静态字符串集合），不含 detail（detail 可能含 fs 路径）
- 路径相关 trace 字段未来若有（本 wiring 不引入），必须经 `anonymize_path` 处理（CONVENTIONS §7）
- 默认 env 未设 → 0 hook → 无任何文件落盘
- 用户不可见、不分发：JSONL 仅在开发者本地手动开启时写到开发者指定的路径

## 8. 测试策略

### 8.1 `main.rs`

| test | 期望 |
|---|---|
| `build_tracer_default_is_noop` | 不设 env → `format!("{:?}", *tracer)` 含 `"hook_count: 0"`（Tracer 现有 Debug impl 已暴露该字段）|
| `build_tracer_with_valid_env_attaches_jsonlines` | `set_var` 指向 `tempdir` 下文件 → debug 输出含 `"hook_count: 1"` + 文件存在 + 调用 `on_tool_call` 后文件有内容 |
| `build_tracer_with_invalid_path_falls_back` | `set_var` 指向 `/proc/不存在/...` 不可创建路径 → debug 输出含 `"hook_count: 0"` + 不 panic |

env 变量类测试需要 serial 跑（Rust 默认并行测试，env 是进程级状态）— 用 `serial_test` crate 或手动 mutex。先看项目是否已用 serial_test。

### 8.2 `search.rs`

加 `MockHook { calls: Arc<Mutex<Vec<String>>> }`，测：

| test | 期望 calls |
|---|---|
| search 成功路径（用 fake registry + fake backend → 流出 N 条） | `["call:search.fake", "result:search.fake"]` |
| search backend open err | `["call:search.fake", "error:search.fake"]` |
| search mid-stream err | `["call:search.fake", "error:search.fake"]`（已发的 result 不影响 trace） |
| search pre-tool 失败（policy denied） | `[]`（pre-tool 不进 trace） |

这些测试需要 search.rs 的逻辑可在没有真 Tauri Channel 时跑 — 当前 search.rs 强依赖 `tauri::ipc::Channel`。**实施前评估**：要么抽出 inner async fn 不依赖 Channel，要么用 mock channel；plan 阶段决定。

### 8.3 集成检查（手测，不入自动化）

- 不设 env → `npm run tauri dev` 跑 C1 (`find pdf`) → 无文件创建
- `LOCIFIND_TRACE=/tmp/locifind-trace.jsonl npm run tauri dev` → 跑 C1 → `/tmp/locifind-trace.jsonl` 有 2 行（call + result）
- 同上跑 C3 (`搜下`) → trace 文件**无新增**（pre-tool 失败不进 trace）

## 9. 风险与未决

| # | 风险 | 缓解 |
|---|---|---|
| R1 | search.rs 与 `tauri::ipc::Channel` 强耦合，单测难写 | plan 阶段评估抽 inner fn vs mock channel；若 mock 太重则单测仅覆盖 `build_tracer` + `search_error_kind` + `intent_to_supported`，trace 触发交手测 |
| R2 | env 测试在并行下 flake | 用 serial_test 或单测内 mutex；先 grep 项目是否已用 |
| R3 | append 模式让文件无限增长 | 文档明示开发者自己 rotate；不在 wiring task 加 rotation 逻辑 |
| R4 | `SearchEvent::Error` 已发后 `tool.search()` 流可能仍有 in-flight item — 是否要等 stream 完整收尾再 trace？ | 当前实现 mid-stream err 即 return，stream Drop 是后续问题；trace 时点对齐现有 error return 即可 |
| R5 | Tracer dispatch 是同步 — JsonLinesHook 持 Mutex<File> + sync `writeln!`，可能阻塞 await | 文件 IO 在本地 SSD < 1ms；不动；如未来出现明显延迟再换 async writer |

未决：JSONL timestamp 当前是 UTC（JsonLinesHook 已用 `Utc::now()`），本 spec 不动。

## 10. 不可回归约束

- v0.5 evals byte-equal（pass 472 parser-only / pass 480 hybrid Q4_K_M / partial / fail / variant 命中 / 字段精确匹配率全部 0 Δ）— 本 wiring 只动 Tauri 层 + harness 不动
- `cargo test --workspace` + `bash scripts/ci.sh` 全过
- main.rs `build_registry_exposes_real_spotlight_on_macos` 现有 test 不破

## 11. 实施摘要（细化留 plan）

净增 LOC ≈ 100（main +20 / search +30 / tests +50）。预计 task 数 4-6：
1. `build_tracer` + main.rs 注入 + 3 个 test
2. `search_error_kind` + `intent_to_supported` helpers
3. search.rs 注入 State + 3 个 trace 点
4. search.rs MockHook test（或决定退化为手测）
5. 手测验证 + STATUS / ROADMAP 同步

详细 task / TDD self-check / 顺序 / 验收 → 下一步 writing-plans。
