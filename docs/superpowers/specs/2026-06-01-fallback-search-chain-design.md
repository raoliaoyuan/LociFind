# 搜索后端 fallback chain（mid-stream retry）设计

> 状态：spec（待 writing-plans）
> 日期：2026-06-01
> 作者：Claude Code (Opus 4.8)
> 关联：[ROADMAP §3.3 Class B](../../../ROADMAP.md) · MVP-19+ Slice B（IntentRouter / SearchableTool / Channel<SearchEvent>）· BETA-15D（Q1/Q2 并集 dedup 先例）

## 1. 背景与动机

MVP-19+ Slice B 落地的 `IntentRouter` 对一个 intent 只**选首位可用后端**（`route_search_expanded` 取单个 `Arc<dyn SearchableTool>`），该后端失败即 `SearchEvent::Error` 放弃。真实 fallback chain（主后端失败 → 切下一后端）此前留作 Class B backlog。

**多后端只在 Windows 存在**（`apps/desktop/src-tauri/src/main.rs`：macOS 仅注册 `SpotlightBackend`；Windows 注册 `WindowsSearchBackend` + `EverythingBackend`）。故本特性的真集成价值在 Windows；Mac 上链退化为单候选（零行为变化），编排核心靠 mock 单测验证。

## 2. 目标与非目标

**目标**：选中后端失败时，按有序候选列表逐个回退，跨候选按 canonical path 去重合并结果，并经新增 `BackendSwitched` 事件向 UI / trace 显式通报切换。

**非目标**：
- 不做 Windows 真双后端端到端集成验证（交接 Windows 会话）；
- 不改 parser / 不改各 backend 内部 search 实现；
- 不引入并行多后端竞速（链是**顺序**回退，非并发 union）。

**铁律**：Mac 单候选路径行为与现状等价（零回归）；evals parser-only 472/26/2 不变。

## 3. 架构与组件

可测核心放 **harness**（平台无关），desktop `search.rs` 仅做事件适配。

### 3.1 新增 `packages/harness/src/fallback_chain.rs`

```rust
pub enum SwitchReason { Unavailable, Error, Empty }   // 对应 A/B/C 三类失败

pub struct BackendSwitch {
    pub from: String,        // 失败候选 tool_id
    pub to: String,          // 下一候选 tool_id
    pub reason: SwitchReason,
}

pub struct ChainOutcome {
    pub total: usize,                 // 累积去重结果数
    pub last_error: Option<String>,   // total==0 时 desktop 用作 Error 文案
}

/// 驱动有序候选链：逐个 search_expanded，按 canonical path 去重累积，
/// 任一候选失败（A/B/C）即切下一个并经 on_switch 通报，结果经 on_result 实时投递。
pub async fn run_fallback_chain(
    candidates: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut dyn FnMut(SearchResult),   // 已 dedup
    on_switch: &mut dyn FnMut(BackendSwitch),
) -> ChainOutcome
```

### 3.2 `IntentRouter` 新增 `route_search_chain`

```rust
/// 返回**有序候选列表**（沿用 route_search_expanded 的 content-preference，
/// 但作用于排序而非只取首位）。现有 route_search / route_search_expanded 保留不动。
pub fn route_search_chain(
    &self,
    expanded: &ExpandedSearchIntent,
) -> Result<Vec<Arc<dyn SearchableTool>>, RouteError>
```

排序规则：与 `route_search_expanded` 同——若需要内容/元数据（base 需要 或 扩展产生内容关键词组），内容型后端排前；否则沿用 id 序。候选为空 → `RouteError::NoBackend`；Clarify → `RouteError::ClarifyNotRoutable`。

### 3.3 desktop `search.rs` 适配

调 `route_search_chain` 拿候选 → 调 `run_fallback_chain`，映射：
- `on_result(SearchResult)` → `SearchEvent::Result`（含原 record 累积逻辑）
- `on_switch(BackendSwitch)` → 新增 `SearchEvent::BackendSwitched { from, to, reason }` + tracer 记一条
- `ChainOutcome.total > 0` → `SearchEvent::Complete { total, elapsed_ms }`
- `ChainOutcome.total == 0` → `SearchEvent::Error { message: last_error.unwrap_or("未找到结果") }`

### 3.4 dedup

跨候选 `HashSet<PathBuf>`（`result.path`），与 BETA-15D spotlight Q1/Q2 并集同款；首个产出某 path 的候选胜出，后续候选重复 path 丢弃（`on_result` 不投递）。

## 4. 链控制流与失败语义

逐候选处理。**一个后端只有"干净跑完且贡献 ≥1 条新结果"才算成功并停链**；否则切下一个：

| 候选状态 | 处理 | reason |
|---|---|---|
| pre-stream Err = `BackendUnavailable` | 切下一候选 | `Unavailable` |
| pre-stream Err = 其它错误 | 切下一候选 | `Error` |
| 流中途 Err（已吐 N 条）| 保留这 N 条（已进 dedup 集），切下一候选 | `Error` |
| 干净跑完，本候选贡献 0 条新结果 | 切下一候选 | `Empty` |
| **干净跑完，贡献 ≥1 条新结果** | **成功，停链**（后续候选不再调用其 search） | — |

> 「成功即停」：A 正常出结果时不多余跑 B。「全触发」：A 吐 3 条后中途崩仍切 B 追加去重结果（保留 3 + B 新结果）。

**终止行为**（候选耗尽）：
- `total > 0` → `Complete{total}`（哪怕过程每个后端都失败过，只要最终收集到结果即成功）
- `total == 0` → `Error{last_error 或「未找到结果」}`

**cancel**：CancellationToken 贯穿全链，取消即停（视作终止，按当前 total 决定 Complete/Error）。

## 5. 事件协议变更

`SearchEvent` 新增一个变体（desktop `search.rs`，同步到前端 TS 类型）：

```rust
BackendSwitched { from: String, to: String, reason: String },  // reason: "unavailable"|"error"|"empty"
```

在**启动下一候选前**发出。`Started`/`Result`/`Complete`/`Error` 语义不变。前端 `SearchView.tsx` 加最小处理（显示「{from} 无结果，改用 {to}…」提示或忽略——UI 细节不在本 spec 强制，至少不崩）。

## 6. 测试

### 6.1 harness 单元测试（Mac 可验，mock SearchableTool）

新增可脚本化 mock `SearchableTool`，可产出：正常 N 条流 / pre-stream Err（含 BackendUnavailable 与其它）/ 吐 M 条后 Err / 干净空流 / 记录 search 是否被调用。覆盖：

1. 单候选成功 → 无切换，全部结果投递
2. 首个 `BackendUnavailable` → 切换(unavailable) → 次个成功
3. 首个干净零结果 → 切换(empty) → 次个成功
4. 首个吐 3 条后中途 Err → 切换(error)，保留 3 条 + 次个去重追加
5. dedup：A 出 path X、B 出 X+Y → 最终 X(归 A)+Y，无重复（on_result 调用次数与路径断言）
6. 全部失败/空 → total=0 → ChainOutcome 触发 Error 路径（last_error 非空）
7. 成功即停：首个成功 → 次个 mock 的 search **未被调用**（断言标志位）
8. cancel 中途取消 → 停链，不再调用后续候选

### 6.2 回归门

- evals parser-only **472/26/2** byte-equal（只动 router/dispatch，不碰 parser）
- harness 既有测试全过 + desktop src-tauri 测试全过
- `cargo fmt --check` + `cargo clippy --all-targets -D warnings` 干净

### 6.3 Mac 集成 / Windows 延后

- **Mac 集成**：Spotlight 单候选路径 = 现状（链退化，零行为变化），desktop 编译 + 既有测试守护。
- **Windows 延后**：真双后端（WindowsSearch 失败 → Everything）端到端 + `BackendSwitched` 在真实 UI 呈现，交接 Windows 会话。

## 7. 风险与已知限制

- **R1 Mac 不可真集成**：唯一后端 Spotlight，多后端链路只能 mock 验证，Windows 真机可能暴露需调整处（用户已知并接受）。
- **R2 dedup 仅按 path**：同文件不同路径（符号链接 / 大小写不敏感卷）可能漏 dedup —— 沿用 spotlight 既有 path-based dedup 限制，不在本 spec 扩展。
- **R3 事件协议扩展**：新增 `BackendSwitched` 需前端 TS 类型同步，否则 Tauri 反序列化未知变体可能告警 —— 前端加最小处理。

## 8. 成功标准

- `run_fallback_chain` + `route_search_chain` 实现 + 8 个单测全过；
- desktop 接入 + `BackendSwitched` 事件 + 前端最小处理，编译通过、既有测试不破；
- evals parser-only 472/26/2 + fmt/clippy 干净；
- Windows 真集成作为明确交接项记入 STATUS/ROADMAP。
</content>
