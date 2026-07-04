# MVP-19+ Slice B — Tauri search command 接入 ToolRegistry 设计 spec

| 项 | 值 |
|---|---|
| ID | MVP-19+ Slice B（第 22 阶段后唯一悬而未决的产品功能） |
| 作者 | Claude Code (Opus 4.7) |
| 日期 | 2026-05-28 |
| 阶段 | M5 子阶段，承接第 26 阶段（parser screenshot extensions fix） |
| 前置 | M1 12/12 ✅（含 MVP-01 ToolRegistry / MVP-03 Policy / MVP-05 IntentRouter / MVP-07A streaming / MVP-10 FallbackChain）+ MVP-18 Tauri 骨架 |
| 后续 | MVP-26 跨平台一致性（Windows 真机后跑 v0.5 evals 对齐 macOS）|

## 1. 目标与范围

### 1.1 目标

把 `apps/desktop/src-tauri/src/search.rs` 从「直调 `SpotlightBackend::new()` + `resolve_intent`」改为「走 `main.rs` 已构建的 `ToolRegistry`，含 policy gate + intent routing」。`Channel<SearchEvent>` 流式 UX 协议保持不变。

第 13 阶段曾决定「与 MVP-26 跨平台 backend 选择一起做更顺」，但 MVP-26 卡 Windows 真机可能拖很久；独立做 wiring 让 MVP-26 只需 review backend 选择策略，不会推翻 wiring。

### 1.2 出场标准

- **必达**：
  - `search.rs` 不再直接 `use SpotlightBackend`；从 `tauri::State<Arc<ToolRegistry>>` 拿 registry
  - search 命令进入即调 `PolicyEngine::evaluate(PolicyAction::from(&intent))`，Deny / Confirm 转 `SearchEvent::Error`
  - 通过 `IntentRouter::route_search` 拿 `Arc<dyn SearchableTool>`，调 `.search().await` 拿 stream
  - `#[cfg(target_os = "macos")]` 不再出现在 search command body 中（仅出现在 main.rs 注册环节）
  - 前端 `SearchView.tsx` 与 `SearchEvent::Started` 加 `tool_id` 字段同步
  - 全 harness + desktop 测试 pass；macOS 手测：搜索框输入 query → IntentBadge 显示 `via spotlight` → 结果流式出现
- **不必达**：
  - mid-stream fallback retry（B 阶段或更高）
  - tracing/hooks 接入
  - context memory / refine 多轮

### 1.3 范围

- **改动 crate**：`packages/harness`、`apps/desktop`
- **不动**：`packages/intent-parser`（resolve_intent 接口稳定）、所有 SearchBackend 实现、前端协议大方向（仅加一字段）
- **Tool trait 不变**：保留 lib.rs:152 注释明确的「最小公约数」设计原则；新增能力走 `SearchableTool` 子 trait

## 2. 现状

`apps/desktop/src-tauri/src/search.rs` 当前实现（74-145 行）的问题：
1. `let backend = SpotlightBackend::new()` 直接 new，绕过 `main.rs::build_registry()` 构建的 `ToolRegistry`
2. 无 policy gate — `PolicyEngine` 已落地（MVP-03）但 Tauri command 未消费
3. `#[cfg(target_os = "macos")]` 把跨平台分支硬编码到 command body，Windows 直接返回 error string
4. `main.rs::build_registry()` 构建的 registry 当前**未被任何代码消费** — `.manage(registry)` 后无人 `tauri::State` 取用

Harness 侧已有的关键基础设施：
- `ToolRegistry` (lib.rs §4)：按 id BTreeMap，含 `production_tools` / `available_tools_supporting`
- `IntentRouter::route` (intent_router.rs)：返回 `&dyn Tool`
- `PolicyEngine` + `PolicyAction::from(&SearchIntent)` (policy.rs)：MVP-03 已落地
- `SearchTool::invoke` (lib.rs:242)：concrete 类型上的 search 入口，但 `dyn Tool` 不可达

**dispatch 缺口**：`Tool` trait 故意不暴露 `invoke()`（lib.rs:152 注释明确）。从 `ToolRegistry` 拿到的是 `&dyn Tool`，无法调 `backend.search()`。要让 search.rs 走 registry，必须先解决这个缺口。

## 3. 架构

### 3.1 端到端数据流（修改后）

```text
SearchView.tsx
  ↓ invoke("search", { query, onEvent: Channel<SearchEvent> })
search.rs (Tauri command)
  ├─ tauri::State<Arc<ToolRegistry>>           ← 从 main.rs 注册
  ├─ tauri::State<Arc<PolicyEngine>>           ← 从 main.rs 注册
  ├─ resolve_intent(query)                      ← intent-parser 不变
  ├─ PolicyEngine::evaluate(SearchIntent(...))  ← Deny / Confirm → SearchEvent::Error
  ├─ IntentRouter::route_search(&intent)        ← 新 method,返回 Arc<dyn SearchableTool>
  └─ tool.search(&intent, cancel).await         ← 通过 SearchableTool dispatch
       ↓ Stream<Result<SearchResult, SearchError>>
     for each item → channel.send(SearchEvent::Result)
```

### 3.2 dispatch 设计：SearchableTool 子 trait + 并行 Arc 表

**方案 A**（采纳）：在 harness 内新增 `SearchableTool: Tool` 子 trait；`SearchTool<B>` 实现它；`ToolRegistry` 内部并行维护一份 `Arc<dyn SearchableTool>` 表。

理由（vs 替代）：
- vs B「给 `Tool` 加 `as_search() -> Option<&dyn SearchableTool>`」：会破坏 Tool 「最小公约数」设计原则
- vs C「Any + downcast」：`SearchTool<B>` 泛型参数 B 在 dispatch 点不可知，无法 downcast
- A 的代价是 ToolRegistry 内部多一个 BTreeMap（Arc 共享，无重复存储），换取 dyn 调用的类型安全

### 3.3 模块分工

| 模块 | 改动 | 说明 |
|---|---|---|
| `packages/harness/src/searchable_tool.rs` | 新建 | `SearchableTool: Tool` trait + `impl SearchableTool for SearchTool<B>` |
| `packages/harness/src/lib.rs` | 改 | `ToolRegistry` 内部加 `searchable: BTreeMap<String, Arc<dyn SearchableTool>>`；新方法 `register_search` / `find_search_tool` / `available_search_tools_supporting`；新 newtype `SearchableToolHandle` 让两表共享同一 Arc |
| `packages/harness/src/intent_router.rs` | 改 | 新方法 `route_search(intent) -> Result<Arc<dyn SearchableTool>, RouteError>` |
| `apps/desktop/src-tauri/src/search.rs` | 重写 | ~80 行净，去掉 cfg 分支与 SpotlightBackend 直调；加 policy gate + IntentRouter + Channel 流式仍走 backend.search() |
| `apps/desktop/src-tauri/src/main.rs` | 改 | `register` → `register_search`；Arc 化 registry；新增 PolicyEngine state；Windows cfg 分支注册 WindowsSearchBackend + EverythingBackend |
| `apps/desktop/src/SearchView.tsx` | 改 | `SearchEvent.started` 加 `tool_id: string`；`IntentSummary` 加 `tool_id`；`IntentBadge` 显示 `via <tool_id>` |

## 4. 关键接口

### 4.1 `SearchableTool` trait

```rust
// packages/harness/src/searchable_tool.rs
use locifind_search_backend::{BackendSearchFuture, CancellationToken, SearchIntent};
use crate::Tool;

pub trait SearchableTool: Tool {
    fn search<'a>(
        &'a self,
        intent: &'a SearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a>;
}

impl<B: locifind_search_backend::SearchBackend + 'static> SearchableTool for crate::SearchTool<B> {
    fn search<'a>(
        &'a self,
        intent: &'a SearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a> {
        self.backend().search(intent, cancel)
    }
}
```

### 4.2 `ToolRegistry` 新接口

```rust
pub struct ToolRegistry {
    tools: BTreeMap<String, Box<dyn Tool>>,
    searchable: BTreeMap<String, Arc<dyn SearchableTool>>,
}

impl ToolRegistry {
    /// Search 类工具的专用注册：同时入 tools 表（通过 newtype）和 searchable 表。
    /// FileAction 类工具继续走通用 `register`。
    pub fn register_search<B: SearchBackend + 'static>(
        &mut self,
        tool: SearchTool<B>,
    ) -> Result<(), DuplicateToolId>;

    pub fn find_search_tool(&self, id: &str) -> Option<Arc<dyn SearchableTool>>;

    /// production + available + supports intent，三层过滤复用现有逻辑。
    pub fn available_search_tools_supporting(
        &self,
        intent: SupportedIntent,
    ) -> Vec<Arc<dyn SearchableTool>>;
}
```

`SearchableToolHandle(Arc<dyn SearchableTool>)` 是内部 newtype，实现 `Tool` trait by delegation（让 `tools` 表持有 Arc 的一份引用 — 不重复堆分配 backend），用于保持 `register_search` 后 `find_by_id` / `production_tools` 等通用查询接口仍能返回该工具。

**`register` vs `register_search` 误用守护**：通用 `register<T: Tool>` 在编译期无法静态拒绝 `SearchTool<B>`（T 满足 Tool 约束）。若 Search 类工具被错调 `register`，`tools` 表会有它但 `searchable` 表没有，`find_search_tool` / `available_search_tools_supporting` 返回 None。MVP 期采用文档约束：`register` 的 doc comment 明确"Search 类工具必须用 `register_search`"。未来若实际出现误用，再用 sealed marker trait 提升到编译期。

### 4.3 `IntentRouter::route_search`

```rust
impl<'a> IntentRouter<'a> {
    pub fn route_search(
        &self,
        intent: &SearchIntent,
    ) -> Result<Arc<dyn SearchableTool>, RouteError> {
        let supported = SupportedIntent::from_intent(intent);
        if supported == SupportedIntent::Clarify {
            return Err(RouteError::ClarifyNotRoutable);
        }
        self.registry
            .available_search_tools_supporting(supported)
            .into_iter()
            .next()
            .ok_or(RouteError::NoBackend)
    }
}
```

`route` 方法（返回 `&dyn Tool`）保留兼容，未来 FileAction 路由会用。

### 4.4 `SearchEvent::Started` 加 `tool_id`

```rust
pub enum SearchEvent {
    Started {
        intent_summary: String,
        fallback_used: bool,
        signals: Vec<&'static str>,
        tool_id: String,  // 新增
    },
    ...
}
```

前端 `SearchView.tsx` 同步：`IntentSummary` 加 `tool_id`，`IntentBadge` 末尾追加 `<span className="intent-tool">via {tool_id}</span>`。

## 5. 测试策略

### 5.1 harness 单测（新加 7 个）

- `searchable_tool.rs`：
  - `searchable_tool_via_dyn_dispatch_returns_backend_stream` — 用 FakeBackend，验证 Arc<dyn SearchableTool> 调 search() 拿到 stream
- `lib.rs` ToolRegistry：
  - `register_search_indexes_both_tables` — 注册后 `find_by_id` + `find_search_tool` 都能拿到
  - `available_search_tools_supporting_filters_stub` — Stub backend 不出现
  - `available_search_tools_supporting_filters_unavailable` — is_available=false 不出现
- `intent_router.rs`：
  - `route_search_returns_real_backend_arc` — Real 优先 Stub
  - `route_search_no_backend_when_none_available`
  - `route_search_clarify_returns_clarify_not_routable`

### 5.2 desktop 测试

- 扩展现有 `build_registry_exposes_real_spotlight_on_macos`：改用 `find_search_tool("search.spotlight")` 验证 search-typed view 也可达

### 5.3 手测（不能自动化的部分）

macOS dev 启动后：
1. 搜索框输入 `find pdf` → 验证 `IntentBadge` 出现 + 显示 `via search.spotlight`
2. 结果流式出现（不是一次性闪出）
3. 输入触发 Clarify 的 query（如 `搜下`）→ 收到 `SearchEvent::Error: clarify intent is not routable`
4. macOS 上没有 Spotlight 索引的极端情况（如禁用 Spotlight）→ 收到 backend error

### 5.4 不可回归

- 全量 `bash scripts/ci.sh` 通过
- `cargo run -p locifind-evals --bin evals -- --with-fallback --hybrid` pass 480 / partial 18 / fail 2 不变（parser 路径与 hybrid 完全没动）

## 6. 风险

| ID | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | `SearchableTool` Arc 拷贝在热路径有开销 | 低 | 低 | 每次 search 只 clone 一个 Arc，<10ns；可忽略 |
| R2 | `SearchableToolHandle` newtype 把 Tool 转包后丢失 `Debug` 等元信息 | 低 | 低 | 显式 `impl Debug` 转发到底层 |
| R3 | Windows backend 注册路径 untested in this session | 中 | 中 | cfg 守护编译；具体可用性测试归 MVP-26 跨平台真机 |
| R4 | `Arc<ToolRegistry>` 全局共享后未来若需要动态注册/卸载工具会困难 | 低 | 低 | MVP 期不支持动态注册；Beta 阶段若需要再换 RwLock |
| R5 | 前端加 `tool_id` 字段若漏改 SearchView TS 会编译失败 | 低 | 低 | 一并改 |

## 7. 与第 17/24-26 阶段 parser fix 路线的关系

本 task 100% 在 wiring 层；不改 parser 任何 lexicon / extraction 逻辑；不动 LoRA adapter；不动 evals fixture。因此 v0.5 evals `--with-fallback --hybrid` 指标（pass 480 / partial 18 / fail 2 / 字段精确匹配 96.0%）**完全不应变化**。本 task 完成后跑一次 evals 应 byte-equal。

## 8. 本会话 vs 后续边界

| 项 | 本会话 | 后续 |
|---|---|---|
| harness SearchableTool + Registry 扩展 | ✅ | — |
| IntentRouter::route_search | ✅ | — |
| search.rs 重写走 Registry | ✅ | — |
| Policy gate | ✅（evaluate + Deny/Confirm 转 error）| 真正的 confirm UI 走 BETA |
| Streaming via Channel | ✅（保留 v0.2 协议 + tool_id 字段）| — |
| Tracing/Hooks 接入 | ❌ | backlog |
| FallbackChain 真正多 backend retry | ❌ | B 阶段 |
| Windows 真机验证 | ❌ | MVP-26 |
| ContextMemory 多轮 | ❌ | backlog |

## 9. 出场验收清单

- [ ] `cargo build -p locifind-harness` ✓
- [ ] `cargo test -p locifind-harness` 全 pass（含新加 6-8 test）
- [ ] `cargo build -p locifind-desktop` ✓（含 main.rs 改动）
- [ ] `cargo test -p locifind-desktop` 全 pass
- [ ] `bash scripts/ci.sh` 整体 ✓
- [ ] `apps/desktop/src/SearchView.tsx` 类型对齐 + IntentBadge 显示 `tool_id`
- [ ] 手测 macOS dev：query → 流式结果 + IntentBadge 显示 `via search.spotlight`
- [ ] v0.5 evals 重跑 pass/partial/fail 与本会话前 byte-equal（评测路径未改动）
- [ ] STATUS + ROADMAP MVP-19+ Slice B 标 done
- [ ] 出场报告（inline 会话日志 + 关键改动 + 评测对比）落 STATUS

## 10. 实施估时

| 步骤 | LOC | 时间 |
|---|---|---|
| 1. searchable_tool.rs 新建 + 单测 | ~140 | 20 min |
| 2. ToolRegistry 扩展 + 单测 | ~70 | 25 min |
| 3. IntentRouter::route_search + 单测 | ~50 | 15 min |
| 4. main.rs 改 register_search + Arc + PolicyEngine state | ~20 | 10 min |
| 5. search.rs 重写 | ~80 net | 30 min |
| 6. SearchView.tsx tool_id 同步 | ~10 | 10 min |
| 7. `bash scripts/ci.sh` + 修编译 | — | 15 min |
| 8. macOS 手测 + STATUS/ROADMAP 收尾 | — | 25 min |
| **总计** | **~370** | **~150 min = 2.5 h** |

落在 STATUS 给的 1-3 h 估时范围内。
