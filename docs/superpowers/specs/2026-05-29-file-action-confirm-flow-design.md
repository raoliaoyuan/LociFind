# FileAction(copy/move/rename)L4 确认流接入 Tauri search command — 设计

> 日期:2026-05-29
> 阶段:M4(MVP-19+ 后续 wiring)/ Class B 代码层 backlog
> 作者:Claude Code (Opus 4.8)
> 承接:第 31 阶段 FileAction(open/locate)多轮接入(`docs/superpowers/specs/2026-05-29-file-action-open-locate-wiring-design.md`)

## 1. 背景与目标

第 31 阶段接通了 `FileAction` 的 `open` / `locate`(Policy L1/L3 → Allow,无需确认),并在 `handle_file_action` 里用 scope gate 把 `copy` / `move` / `rename`(L4 → RequireConfirmation)挡在 `invoke` 之外(因为 parser 对这三个预设 `requires_confirmation=true`,直接进 invoke 会绕过尚未实现的确认流执行)。

本阶段把 copy / move / rename 真正接通,带 **UI 确认往返**:用户搜索后输入「把第2个复制到桌面」「移动第3个到下载」「把第1个重命名为 final」,系统先弹确认对话框,用户确认后才执行。

### 范围(明确边界)

- **接 copy / move / rename 三个 L4 动作**,带确认往返协议。
- **copy/move/rename 本阶段都限单目标**(target_ref 解析后恰好 1 个路径);多目标(`All` / 多 `Indices`)→ 友好 Error,留后续。
- **destination 解析在 wiring 层**(展开 `~` + join 源文件名 → 完整路径),**harness `FileActionTool` / `ContextMemory` 一行不动**(符合 `file_action_tool.rs` `contract_39` 注释「实际多文件 copy 的 dest 归一化由调用方负责」的原始设计意图)。
- `delete` 维持 schema + Policy 双重硬禁用,不接。
- ContextMemory 全程**只读**(沿用第 31 阶段语义,action 不 record/clear)。

### 现状缺口(第 31 阶段遗留)

`handle_file_action` 的 scope gate 对 copy/move/rename 一律转 Error「该操作暂不支持(确认流待后续阶段)」。本阶段把这条路径替换为确认往返。

### 前提(已确认)

- parser `try_parse_file_action` 对以下 query 产出对应 intent:
  - Copy:`把第2个复制到桌面` / `copy ... to desktop`;`destination` 经 `extract_destination` 映射为 `~/Desktop` / `~/Downloads` / `~/Documents` / `~/Pictures`(带 tilde 的**目录**)。
  - Move:`移动第3个到下载` / `move ... to downloads`;destination 同上。
  - Rename:`把第1个重命名为 final` / `rename the 1st to final`;`new_name` 经 `extract_new_name` 提取裸 token。
- Policy 分级:Copy/Move/Rename = L4 → RequireConfirmation;`FileActionTool::invoke` 在 `PolicyDecision::RequireConfirmation && action.requires_confirmation==true` 时**直接执行**(不返回确认)。
- harness `FileActionTool` copy/move 执行:`destination` 当作完整文件路径(`PathBuf::from(dest)` + `exists()` 冲突检查 + `std::fs::copy/rename`);rename 用 `parent.join(new_name)`。**单目标 + 完整 dest 路径时该逻辑正确**。

## 2. 架构总览

承接第 31 阶段「`search_impl` 内联分支」。`handle_file_action` 分两条路:

```
FileAction(effective)
 ├─ Open / Locate (Allow)         → 立即执行 → ActionDone               【第 31 阶段已有】
 └─ Copy / Move / Rename (L4)     → 确认往返:
      [首次下发 — search_impl 的 FileAction 分支内]
        ① 解析 target_ref(只读 context)→ 失败 = 友好 Error(NoLastResults / IndexOutOfRange)
        ② 单目标校验 → 多目标 = 友好 Error「一次只能{...}单个文件(多文件待后续)」
        ③ copy/move: resolve_destination(展开 ~ + join 源文件名 → 完整路径)→ 失败 = 友好 Error
        ④ 构造 pending FileAction { action, target_ref=Path{已解析绝对路径},
             destination=Some(完整路径)(copy/move) / new_name(rename), requires_confirmation=true }
           → 存入 Arc<Mutex<Option<FileAction>>> managed State
        ⑤ 发 SearchEvent::ConfirmAction { action_kind, paths, destination?, new_name? }
        ⑥ return(首次下发不调 invoke、不进 trace)
      [confirm_action command]
        take pending(lock + Option::take)→ None = Err「没有待确认的操作」
        invoke(&pending, &context)  // requires_confirmation=true + target_ref=Path → 直接执行,不依赖 context
        trace: tool_call(file_action.local, FileAction) → tool_result / on_error
        Executed{affected} → Ok(ActionDoneData { action_kind, paths })
        Err → Err(friendly_file_action_message)
      [cancel_action command]
        清 pending(set None)→ Ok(())
```

### 关键设计点

1. **pending 自包含**:首次下发就把 `target_ref` 解析成绝对 `TargetRef::Path`,存进 pending。确认时 invoke 不再依赖 ContextMemory 当前状态 —— 规避「首次下发到用户确认之间用户又发起新搜索」导致目标漂移。
2. **invoke 只调一次**:在 `confirm_action` 里调,L4 + `requires_confirmation=true` → 直接执行。**不需要**"先把 requires_confirmation 翻 false 拿 RequiresConfirmation"的把戏 —— copy/move/rename 是 L4 这件事 wiring 层已知,直接呈现确认。
3. **destination 在 wiring 解析**:parser 给的是 `~/Desktop`(目录);wiring 展开 ~ 并 join 源文件名得到完整文件路径,传给 invoke。invoke 现有"destination = 完整文件路径 + 单目标"逻辑正确。harness 不动。

### State 注入

`main.rs` 新增 `Arc<Mutex<Option<FileAction>>>` managed State(pending 槽,容量 1)。

### 改动文件

- `apps/desktop/src-tauri/src/search.rs`:`handle_file_action` 分支扩展 + `resolve_destination` + 单目标校验 + `SearchEvent::ConfirmAction` + `confirm_action`/`cancel_action` commands + `ActionDoneData` + 测试。
- `apps/desktop/src-tauri/src/main.rs`:`.manage(Arc<Mutex<Option<FileAction>>>)` + 注册两个新 command。
- `apps/desktop/src/SearchView.tsx`:`confirm_action` 事件 + `confirm_pending` 状态 + 确认对话框 UI + confirm/cancel 调用 + `describeConfirm`。

## 3. destination 解析 + 单目标语义

```rust
/// 把 parser 的 destination(如 "~/Desktop")展开为绝对目录,再 join 源文件名,
/// 得到 copy/move 的完整目标文件路径。
fn resolve_destination(dest_hint: &str, source: &Path) -> Result<PathBuf, String>
```

- 展开 `~`:若 `dest_hint` 以 `~/` 或 `~` 开头,用 home 目录替换。home = `std::env::var("HOME")`,失败则 `std::env::var("USERPROFILE")`(Windows),都无 → `Err("无法确定目标位置")`。非 ~ 开头的路径原样用。
- join 源文件名:`source.file_name()` 拼到展开后的目录;无 file_name → `Err("无法确定目标位置")`。
- 返回完整文件路径,作 pending action 的 `destination`。

**单目标语义**:`handle_file_action` 对 copy/move/rename 解析 target_ref 后,若路径数 ≠ 1 → 友好 Error:
- copy → 「一次只能复制单个文件(多文件待后续)」
- move → 「一次只能移动单个文件(多文件待后续)」
- rename → 「一次只能重命名单个文件(多文件待后续)」

## 4. SearchEvent::ConfirmAction + 新 command

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

`action_kind` 由 `FileActionKind` 小写化(与 §31 `ActionDone` 一致)。

确认结果用一个 serde 结构返回(不走 Channel,一次性 command):

```rust
#[derive(Serialize)]
struct ActionDoneData {
    action_kind: String,
    paths: Vec<String>,
}

#[tauri::command]
async fn confirm_action(
    pending: tauri::State<'_, Arc<Mutex<Option<FileAction>>>>,
    file_action_tool: tauri::State<'_, Arc<FileActionTool>>,
    tracer: tauri::State<'_, Arc<Tracer>>,
    context: tauri::State<'_, Arc<Mutex<ContextMemory>>>,
) -> Result<ActionDoneData, String>

#[tauri::command]
async fn cancel_action(
    pending: tauri::State<'_, Arc<Mutex<Option<FileAction>>>>,
) -> Result<(), String>
```

- `confirm_action`:`pending.lock().take()` 取出;`None` → `Err("没有待确认的操作")`。否则 invoke(只读 context guard);`Executed{affected}` → trace tool_result + `Ok(ActionDoneData{ action_kind, paths: affected as strings })`;`Err(e)` → trace on_error + `Err(friendly_file_action_message(&e))`;`RequiresConfirmation`(理论不可达,requires_confirmation=true)→ trace on_error("UnexpectedRequiresConfirmation") + `Err`。先 `on_tool_call` 再 invoke,保持 trace 配对。
- `cancel_action`:`*pending.lock() = None`;`Ok(())`。

### pending 生命周期

容量 1 槽。首次下发 copy/move/rename 时写入(覆盖任何旧 pending)。`confirm_action` take(消费)。`cancel_action` 清空。open/locate 与普通 search **不触碰** pending(它们不会进入确认路径)。若用户不点对话框直接发新搜索,UI 对话框被新状态替换,pending 滞留但无害(没有任何路径会在未显式 `confirm_action` 时执行);下一个 copy/move/rename 覆盖它。

## 5. 错误 UX

复用第 31 阶段 `friendly_file_action_message`,新增几条:

| 情况 | 文案 |
|---|---|
| 多目标 copy/move/rename | 一次只能{复制/移动/重命名}单个文件(多文件待后续)|
| destination 解析失败(无 HOME / 无文件名)| 无法确定目标位置 |
| 确认时无 pending | 没有待确认的操作 |
| 目标已存在(`PathConflict`)| 目标已存在(在 `friendly_file_action_message` 加 `PathConflict` 分支:「目标已存在:{dest}」)|
| target_ref 越界 / 无上一轮 | 复用 §31:「第 N 个结果不存在...」/「请先发起一次搜索」|

## 6. Tracing

- **首次下发**(解析 + 校验 + 存 pending + 发 ConfirmAction):pre-tool,**不进 trace**(未调 invoke)。target_ref 解析失败 / 多目标 / destination 失败均是 pre-tool → 仅友好 Error,无 trace。
- **confirm_action**:真正 invoke → `tool_call(file_action.local, FileAction)` + `tool_result(result_count=affected.len())` / `on_error(error_type=file_action_error_kind)`。
- 与 §31「trace 只记真实 tool 执行」「pre-tool failure 不进 trace」一致。注意:open/locate 的 NoLastResults 会 trace(它立即 invoke),copy/move/rename 的 NoLastResults 不 trace(首次下发自己解析 target_ref,pre-invoke)—— 这是两条不同执行路径的自然差异,可接受。

## 7. ContextMemory 语义

全程**只读**。首次下发解析 target_ref 后,pending 用 `TargetRef::Path` 自包含;`confirm_action` 的 invoke 不 record/clear。连续 action(含确认型)不污染搜索基准 —— 与第 30/31 阶段一致。

## 8. 测试计划

复用 / 扩展第 31 阶段 `MockFileActionExecutor`(已记录 copy/move/rename 调用 + 路径)。

### 单元测试(handle_file_action 首次下发 + 两个 command)

| 测试 | 断言 |
|---|---|
| copy 首次下发存 pending 不执行 | MockExecutor **未**收到调用;事件含 `confirm_action` + destination 含解析后路径;pending 槽 Some |
| rename 首次下发 | 事件含 `confirm_action` + new_name;pending Some;executor 未调用 |
| 多目标 copy → Error 不存 pending | 事件含 error「一次只能复制单个文件」;pending None;无 trace |
| 越界 → Error | 事件含 error 越界文案;pending None |
| `resolve_destination` 展开 + join | `resolve_destination("~/Desktop", /tmp/f0)` == `${HOME}/Desktop/f0`(用 HOME env 断言)|
| `confirm_action` 无 pending → Err | `Err` 含「没有待确认的操作」|
| `confirm_action` 执行 | 预置 pending(copy, Path, 完整 dest)→ confirm_action → MockExecutor 收到 `copy(/tmp/f0, dest)`;返回 `ActionDoneData{copy, [dest]}`;trace call+result |
| `cancel_action` 清 pending | 预置 pending → cancel → pending None → `Ok(())` |

### 集成测试(search_impl 首次下发 → confirm)

| 测试 | 断言 |
|---|---|
| 搜索 → copy 第1个 → confirm | `find pdf`(FakeOkBackend 2 条)→ `把第1个复制到桌面` → ConfirmAction 事件 + pending Some → confirm_action → MockExecutor 收到 `copy(/tmp/f0, ${HOME}/Desktop/f0)` |
| 搜索 → rename 第1个 → confirm | `把第1个重命名为 final` → confirm → MockExecutor 收到 `rename(/tmp/f0, "final")` |
| cancel 后 pending 清空 | copy 首次下发 → cancel_action → pending None,executor 未调用 |

### 真机手测(用户驱动)

1. `find pdf` → `把第1个复制到桌面` → 确认对话框(「复制 X 到 .../Desktop?」)→ 确认 → 文件真出现在桌面 + UI「已复制 ...」+ trace call+result。
2. `移动第2个到下载` → 确认 → 文件真移动到下载 + UI 反馈。
3. `把第1个重命名为 testname` → 确认 → 文件真改名。
4. 任一 copy → **点取消** → 文件未动 + 对话框消失 + 无 trace。
5. 多目标 `把这些复制到桌面`(若 parser 出 All)→ 友好「一次只能复制单个文件」。

## 9. 不回归约束

- harness / parser / evals 源不动 → v0.5 parser-only **维持 472/26/2**、hybrid Q4_K_M **维持 480/18/2**(byte-equal)。
- `bash scripts/ci.sh` 全过。
- desktop crate 现有 24 测试不破;新增约 11 测试(8 单元 + 3 集成)。

## 10. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 确认前后 context 漂移导致执行错目标 | pending 用 `TargetRef::Path` 自包含,invoke 不依赖 context 当前状态 |
| 多目标 copy 把多文件写到同一路径(harness 已知缺陷)| 单目标校验在首次下发拦住,多目标转 Error 绝不进 invoke |
| `~` 未展开导致写到字面相对路径 | `resolve_destination` 展开 ~;失败转友好 Error |
| 滞留 pending 被误执行 | 只有显式 `confirm_action` 才 invoke;UI 对话框是唯一触发点;新 copy/move/rename 覆盖旧 pending |
| 改动引入 dead_code 破 clippy --all-targets(§30/§31 教训)| 新增项随 Task 接线即用;若拆分中途为 dead code 用 `#[cfg_attr(not(test), allow(dead_code))]` 暂存,接线 task 移除 |

## 11. 交付物

- `search.rs`:`handle_file_action` 确认分支 + `resolve_destination` + 单目标校验 + `SearchEvent::ConfirmAction` + `confirm_action`/`cancel_action` + `ActionDoneData` + `friendly_file_action_message` 加 `PathConflict` 分支 + 单元 + 集成测试。
- `main.rs`:`.manage(Arc<Mutex<Option<FileAction>>>)` + 注册两个新 command。
- `SearchView.tsx`:`confirm_action` 类型 + `confirm_pending` 状态 + 确认对话框(确认/取消按钮)+ confirm/cancel invoke + `describeConfirm`。
- 后续 backlog 更新:Class B 移除「FileAction copy/move/rename 确认流」;新增「FileAction copy/move/rename 多目标支持(方案 A:harness 目录语义 per-target join)」。
