# ContextMemory 多轮接入 Tauri search command —— 设计

> 阶段：M（MVP），MVP-19+ 续作（Class B 代码层）
> 作者：Claude Code (Opus 4.8)
> 日期：2026-05-29
> 关联：[MVP-06 ContextMemory](../../../packages/harness/src/context.rs)、[MVP-19+ Slice B](./2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md)、[第 28 阶段 Tracing](./2026-05-28-tracing-hooks-search-wiring-design.md)

## 1. 背景与问题

MVP-06 已在 `packages/harness/src/context.rs` 落地完整的 `ContextMemory`：

- `record(intent, results)` / `last_turn()` / `clear()` / `is_empty()`
- `apply_refine(&Refine) -> Result<RefineOutcome, RefineMergeError>` —— 按 schema §3.4 合并语义把 `delta` / `clear` 应用到上一轮 intent，产出新的 `FileSearch` / `MediaSearch` 基准
- `resolve_target_ref(&TargetRef)` —— 把 `LastResults { selector }` 解析为路径列表

这套合并逻辑带完整单测，是 refine 合并的**单一信源**。

但 Tauri 的 `apps/desktop/src-tauri/src/search.rs::search_impl` **完全没有使用它**：每次调用 `resolve_intent(&query, None)` 独立解析，无任何跨调用状态。后果：

- parser 看到 “只看 X / 限定到 / 排除 / 按大小” 等信号时会产出 `SearchIntent::Refine { base_ref: LastIntent, delta, clear }`，
- 但没有任何一层把 `delta` 合并到上一轮 intent；`IntentRouter::route_search` 也无法直接路由 `Refine` variant（它不是 search-typed tool 能消费的具体查询）。
- 因此多轮渐进细化（`find pdf` → `只看下载目录里的` → `上周修改的`）在 UI 端完全失效。

本设计把 `ContextMemory` 接进 Tauri search command，使 `Refine` 能合并上一轮并执行。

## 2. 范围（已与用户对齐）

| 维度 | 决策 |
|---|---|
| 接入范围 | **仅 `Refine` 合并**。`FileAction` 的 `target_ref` 解析是另一条 command 路径（带 policy 确认流），留后续 |
| 链式语义 | **渐进收窄**：每次成功搜索（含合并后的 refine）都 `record` 为新的 last turn，与 schema `base_ref: LastIntent` 一致 |
| 会话边界 | **隐式覆盖**：context 活在整个 app 进程生命周期；每次新的非-refine 搜索自然覆盖上一轮。不做显式 clear command / 窗口事件清空 |
| 错误 UX | refine 无上一轮 / 合并出错 → **复用 `SearchEvent::Error` + 友好文案**，不加新事件变体 |

### 不做（YAGNI）

显式 clear command、窗口隐藏/重开清空、`FileAction` target_ref 解析、向 UI 暴露 refine 冲突、CLI 多轮、ContextMemory TTL/过期。

## 3. 架构

新增一个 Tauri managed State `Arc<Mutex<ContextMemory>>`（`std::sync::Mutex`）。`search_impl` 仿照现有的 `tracer` 参数，再加一个 `context` 参数；`search()` 薄包装解 State 后 `Arc::clone` 传入。**harness 的 `ContextMemory` 一行不动**。

```text
main.rs:  .manage(Arc::new(Mutex::new(ContextMemory::new())))
search(): tauri::State<'_, Arc<Mutex<ContextMemory>>>
            → Arc::clone → search_impl(.., context)
```

选 `std::sync::Mutex` 而非 tokio async Mutex 的理由：两段临界区都是同步短操作（读 last turn / 写 record），**不跨 await**，无需异步锁。

## 4. 组件改动

### 4.1 `main.rs`

- 增 `use std::sync::Mutex;` 与 `use locifind_harness::ContextMemory;`
- `tauri::Builder` 链上加 `.manage(Arc::new(Mutex::new(ContextMemory::new())))`
- 既有 `build_registry` / tracer 注入测试不受影响；可加 1 个最小测试断言 State 类型可构造（可选）

### 4.2 `search.rs`

新增自由函数：

```rust
/// 若 intent 是 Refine，按 ContextMemory 合并上一轮基准；否则原样返回。
/// conflicts（clear↔delta 同名）走 eprintln 记录，不阻断。
fn apply_refine_if_needed(
    intent: SearchIntent,
    ctx: &ContextMemory,
) -> Result<SearchIntent, RefineMergeError> {
    match intent {
        SearchIntent::Refine(ref r) => {
            let outcome = ctx.apply_refine(r)?;
            if !outcome.conflicts.is_empty() {
                eprintln!("search: refine 字段冲突(以 clear 为准): {:?}", outcome.conflicts);
            }
            Ok(outcome.intent)
        }
        other => Ok(other),
    }
}
```

`search_impl` 签名增 `context: Arc<Mutex<ContextMemory>>` 参数；流程见第 5 节。

### 4.3 前端 `SearchView.tsx`

**无需改动**。refine 无上下文走现有 `SearchEvent::Error.message`，前端已能渲染错误态；友好文案由 Rust 侧给出。

## 5. 数据流

```text
query
 → resolve_intent(query, None)                       // 现状不变
 → effective = apply_refine_if_needed(intent, &ctx)  // 新：锁 ctx 短读
     · 非 Refine        → 原样
     · Refine + 有上一轮 → 合并出 FileSearch/MediaSearch
     · Refine + 无上一轮 → Err → SearchEvent::Error，return（不 record / 不 trace）
 → policy gate（跑在 effective 上）
 → route_search(effective) → tool
 → tracer.on_tool_call + SearchEvent::Started
 → 流式：边发 Result 事件，边累积原始 SearchResult
 → tracer.on_tool_result + SearchEvent::Complete
 → record(effective, results)                        // 新：仅成功路径，锁 ctx 短写
```

非-refine 查询：`effective == parsed`，正常 record → 成为下一次 refine 的基准（渐进收窄链）。

**锁顺序与时机**：

1. 合并锁（读）：`let effective = { let g = context.lock(); apply_refine_if_needed(intent, &g) };` —— clone 出 effective 后立即 drop guard。
2. record 锁（写）：流结束、发 `Complete` 之前 `context.lock().record(effective.clone(), results)`。

两段临界区之间是 `tool.search(...).await` 与流消费，**guard 不跨这些 await 点**。

## 6. 错误处理

所有失败路径都**不 record**（保持上一轮不变）：

| 失败点 | 行为 | 是否 trace |
|---|---|---|
| `resolve_intent` 失败 | `SearchEvent::Error` | 否（pre-tool） |
| refine 合并失败（NoLastIntent / InvalidBase / FieldNotApplicable） | `SearchEvent::Error` + 友好文案 | 否（pre-tool，与第 28 阶段一致） |
| policy `Deny` / `RequireConfirmation` | `SearchEvent::Error` | 否 |
| `route_search` 失败 | `SearchEvent::Error` | 否 |
| backend open err / mid-stream err | `SearchEvent::Error` + `tracer.on_error` | 是 |
| refine 的 clear↔delta 冲突 | `eprintln` 记录，**不阻断**，按 clear 为准继续合并 | 否 |

`NoLastIntent` 友好文案：`"没有可细化的上一轮搜索，请先发起一次搜索"`。其余合并错误用 `RefineMergeError::Display` 文案。

合并失败属于 pre-tool 失败：尚未进入 backend，沿用第 28 阶段「pre-tool 不进 Tracer」约定，仅经 `SearchEvent::Error` + eprintln。

## 7. 并发与锁纪律

`std::sync::Mutex`，两段同步短临界区（开头合并读、结尾 record 写），**都不跨 await**。并发搜索在 mutex 上串行化，record 为 last-writer-wins —— 单搜索框场景下查询天然串行，可接受。

**clippy 约束**：workspace 级 `unwrap_used` / `expect_used` 均为 `warn` 且 CI 跑 `-D warnings`，生产代码**不得用 `.lock().unwrap()` / `.expect()`**。lock 一律用 `.lock().unwrap_or_else(|e| e.into_inner())` 恢复 poison（临界区内无 panic 风险，into_inner 安全）。测试模块已 `#![allow(clippy::unwrap_used, clippy::expect_used)]`，测试内可继续用 `.unwrap()`。

## 8. 结果回收

`record` 需要原始 `SearchResult`（非序列化后的 `SearchResultJson`）。当前 `search_impl` 在构造 `SearchResultJson` 时 move 走了 `result` 的字段。改为：构造 JSON 时 clone 所需字段（`SearchResult: Clone`），随后把 `result` move 进累积 `Vec<SearchResult>`。容量 1 轮，内存可忽略。仅在成功完成（发 `Complete`）时把累积结果交给 `record`。

## 9. 测试

### 9.1 单元（search.rs 自由函数 `apply_refine_if_needed`）

- 非 Refine intent（FileSearch）→ 原样透传
- Refine + 已填充 context → 合并后 intent 字段正确（含 base + delta）
- Refine + 空 context → `Err(RefineMergeError::NoLastIntent)`

### 9.2 集成（`search_impl`，`#[tokio::test]`，注入 context + 新增「捕获 intent」fake backend）

新增 `FakeCapturingBackend`：内部 `Arc<Mutex<Vec<SearchIntent>>>`，`search()` 把收到的 intent clone 入表，再返回若干 fake 结果。用于断言路由到 backend 的 **effective intent**。

- **T1 record-then-refine**：先跑基准 `find pdf`（记录 FileSearch），再跑确认会解析为 `Refine` 的查询（如 `只看 png`）→ 捕获后端断言末次 effective intent 是 `FileSearch` 且含 `png` 扩展名 → 第二次查询走 Started/Complete 而非 Error。
- **T2 refine-without-context**：空 context 下首查即 refine 查询 → `SearchEvent::Error`（文案含「上一轮」）+ trace 无 `call`/`result`。
- **T3 链式**：基准 → refine1（如 `只看下载目录`）→ refine2（如 `只看 png`）全部成功；捕获后端断言末轮 effective intent 同时体现 refine1 + refine2 的 delta（location + extensions）。

> **实现期 de-risk**：用于触发 Refine 的查询串先用 CLI 或快速单测确认其确实解析为 `SearchIntent::Refine`（验证 parser dispatch 顺序，避免被其他 parser 抢先）。`find pdf` 已知稳定解析为 FileSearch（见 search.rs 既有 `QUERY_FOR_FILE_SEARCH`）。

## 10. 影响面与不可回归

- 改动文件：`apps/desktop/src-tauri/src/main.rs`、`apps/desktop/src-tauri/src/search.rs`。无 TS 改动。
- **不动 parser / harness / evals / 模型**：wiring 层改动，v0.5 evals（hybrid Q4_K_M：pass 480 / partial 18 / fail 2 / 字段精确匹配 96.0% / variant 命中 99.6%）预期 byte-equal 维持。
- harness ContextMemory 既有测试不动。desktop crate 测试数增加（单元 3 + 集成 3 左右）。
- `bash scripts/ci.sh` 全过为收工前置。
- UI 端真机手测（record-then-refine 链路）需用户驱动，列入收工「未尽事宜」（agent 无法点击 Tauri 窗口）。
