# FileAction copy/move 多目标支持 — 设计

> 日期：2026-05-30
> 作者：Claude Code (Opus 4.8)
> 阶段：M（MVP）/ Class B 代码层 backlog（第 32 阶段新增项「多目标支持（方案 A）」）
> 类型：功能扩展（schema 新增变体 + harness destination 语义翻转 + wiring 放开单目标闸）

## 1. 背景与目标

第 31 阶段接入 FileAction(open/locate) 多目标，第 32 阶段接入 copy/move/rename **单目标**确认流。当时 `handle_confirmable_action` 用 `targets.len() != 1` 硬拦多目标转友好错误，留 backlog「copy/move/rename 多目标支持（方案 A）」。

**触发入口今天已存在**：parser `extract_target_ref`（[`packages/intent-parser/src/parsers/file_action.rs:70`](../../../packages/intent-parser/src/parsers/file_action.rs)）把 `这些 / these / all of them` 解析成 `TargetSelector::All`，已有测试 `把这些 pdf 复制到桌面 → Copy`。这条 intent 今天被 wiring 的单目标闸拦下。

**为什么 copy/move 多目标今天坏**：`FileAction` 只有一个 `destination` 字段，harness `execute_one`（[`file_action_tool.rs:304`](../../../packages/harness/src/file_action_tool.rs)）把它当**完整目标文件路径**，N 个目标共用一个 dest → 第 2 个 `dest_path.exists()` 命中 → `PathConflict`。contract_39 测试坐实此行为。

**open/locate 多目标今天已工作**：`handle_file_action`（[`search.rs:341`](../../../apps/desktop/src-tauri/src/search.rs)）不设单目标闸，直接走 `invoke`，harness 内部 `for target in &targets` 循环逐个 open/locate。open/locate 不用 destination，无此问题。

**目标**：

1. copy/move 支持多目标（`把这些 pdf 复制到桌面` 真复制 N 个文件到目标目录）。
2. rename **维持单目标**（N 文件改 1 名无意义，N>1 友好错误）。
3. 保持核心安全性质：copy/move/rename 绝不在未确认时执行。
4. evals v0.5 parser-only 维持 byte-equal **472/26/2**（parser 不产新变体）。

**非目标**：rename 多目标；多目标各自不同目标目录；delete；调整 batch 阈值（沿用 10）；fallback chain。

## 2. 核心架构决策

### 决策 1：per-target basename join 放 harness（方案 A）

`FileAction.destination` 字段语义**翻转**：copy/move 时从「完整目标文件路径」改为「目标**目录**」。harness `execute_one` 内部对每个目标 `dir.join(target.file_name())` 拼出该目标的落点。

理由：

- 与 parser 现实一致——`extract_destination`（[`file_action.rs:161`](../../../packages/intent-parser/src/parsers/file_action.rs)）只产 `~/Desktop`/`~/Downloads`/`~/Documents`/`~/Pictures` 这类**目录提示**，从不产含文件名的完整路径。「destination 是目录」比「destination 是调用方预拼的完整路径」更贴合真实输入。
- **对称性**：open/locate 多目标已在 harness `invoke` 循环；让 copy/move 也走同一条 harness 多目标循环，消除「同一功能两层循环」的不对称。`FileAction` 单 `destination` 字段配合「目录 + 内部 join」即可表达 N 目标的 N 个落点。
- 单目标 copy/move 一并统一走 harness join（wiring 不再 join），单/多目标同一路径。

代价（已接受）：动 harness + schema 新变体；翻转 contract_39 与若干旧单目标测试的 destination 语义（从完整路径改为目录）。

> 注：此决策对第 32 阶段「destination 归一化由调用方负责、harness 一行不动」原则做有意识的修订。单目标方案下「调用方归一化」是便利之选；多目标合法地需要 harness（它持有 N 目标循环与 batch 阈值）知道 destination 是目录。join 应放在迭代发生的那一层。

### 决策 2：新增 `TargetRef::Paths { values }` 自包含 pending

confirm 的 pending 需**自包含 N 个绝对路径**（沿用第 32 阶段「自包含 pending 避免确认前 context 漂移」），confirm 时单次 `invoke` 即处理 N 目标。现有 `TargetRef` 仅 `Path{value}`（单）/ `LastResults{selector}`（依赖 context，会漂移）。新增：

```rust
// search-backends/common/src/lib.rs，TargetRef（#[serde(tag = "source", rename_all = "snake_case")]）
/// 直接指定一组绝对路径（确认流自包含 pending 用；parser 不产）
Paths {
    /// 绝对路径列表
    values: Vec<String>,
},
```

serde tag = `"source": "paths"`。**pending 容器类型不变**（仍 `Arc<Mutex<Option<FileAction>>>`）——只是内部 `target_ref` 用 `Paths{N}`、`destination` 存展开后的目录，`confirm_action_impl` 一行不改（单次 invoke）。

### 决策 3：预检冲突 + 整体执行

harness `invoke` 在 `for target` 执行循环**之前**加预检：算出全部 N 个落点（`dir.join(basename)`），任一 `exists()` → 返回 `PathConflict`（零磁盘副作用）；预检通过再逐个执行。把最常见的同名冲突变成干净的 all-or-nothing。预检后中途 executor 错（罕见，权限/磁盘满）仍 fail-fast 并报告已成功部分。

### 决策 4：批量上限沿用 `batch_threshold = 10`

`invoke` 已有 `targets.len() > batch_threshold → BatchThresholdExceeded`。`这些` 解析出 >10 结果 → 友好错误「目标过多」。confirm 对话框最多列 10 项，且必须用户确认才执行，cap 作二级安全网。

## 3. 各组件改动清单

| 组件 | 文件 | 改动 |
|---|---|---|
| schema | `packages/search-backends/common/src/lib.rs` | 加 `TargetRef::Paths { values: Vec<String> }` 变体（tag `paths`） |
| harness 解析 | `packages/harness/src/context.rs` | `resolve_target_ref` 加 `Paths` 臂（映射 PathBuf；空列表兜底 `EmptyIndices`） |
| harness 执行 | `packages/harness/src/file_action_tool.rs` | `execute_one` copy/move 把 destination 当目录 `dir.join(basename)`；抽 `dest_for(target, dir)` helper；`invoke` 加预检冲突 pass；改写 contract_39 + 旧单目标 destination 测试为目录语义 |
| parser match | `packages/intent-parser/src/lib.rs:242` | `fa.target_ref` match 加 `Paths` 臂 / catch-all（仅为编译完整，parser 不产 Paths） |
| jsonschema 测试 | `packages/search-backends/common/tests/jsonschema.rs:95` | match 加 `Paths` 臂 |
| wiring | `apps/desktop/src-tauri/src/search.rs` | `handle_confirmable_action`：rename 保留单目标闸、copy/move 放开（pending 用 `Paths{N 绝对路径}` + `destination = expand_tilde(dir)`）；`resolve_destination` 简化为只 `expand_tilde`（不再 join basename）；`ConfirmAction` paths=N 源、destination=目录 |
| UI | `apps/desktop/src/SearchView.tsx` | 确认对话框渲染 N 个文件（`describeConfirm` 支持多源 → 「复制 N 个文件到 ~/Desktop？」+ 列表） |

## 4. 数据流（copy/move 多目标）

```
"把这些 pdf 复制到桌面"
  → parser: Copy + target_ref=All + destination="~/Desktop"
  → wiring handle_confirmable_action:
      读 context 解析 target_ref → Vec<PathBuf>（N 个绝对路径）
      copy/move 分支（非 rename）：
        N ≤ batch_threshold 校验（否则友好错误）
        pending = FileAction {
          target_ref: Paths { values: [abs1..absN] },   // 自包含
          destination: Some(expand_tilde("~/Desktop")),  // 目录，不 join
          ...
        }
      发 ConfirmAction { paths: [src1..srcN], destination: dir, new_name: None }
  → UI 弹「复制 N 个文件到 ~/Desktop？」+ 文件列表
  → 用户确认 → confirm_action（command 一行不改）
  → confirm_action_impl: 单次 invoke(pending, context)
      harness invoke:
        resolve_target_ref(Paths) → N 路径
        N ≤ batch_threshold
        Policy（Allow|RequireConfirmation，requires_confirmation=true 直接执行）
        预检：∀ target，dir.join(basename) 不得 exists（任一冲突 → PathConflict，零副作用）
        ∀ target：copy(src, dir.join(basename))  // move 同理
      → Executed { affected: N 路径 }
  → ActionDone { paths: N }
```

rename 多目标：`handle_confirmable_action` 在 rename 分支保留 `targets.len() != 1` → 友好错误「一次只能重命名单个文件」。

## 5. 错误处理

| 场景 | 行为 |
|---|---|
| 预检任一落点已存在 | `PathConflict`，零磁盘副作用，友好文案点名冲突 |
| N > 10 | `BatchThresholdExceeded` 友好错误「目标过多」 |
| rename N>1 | wiring 友好错误「一次只能重命名单个文件」 |
| 预检后中途 executor 错（权限/磁盘） | fail-fast，报告已成功部分（罕见） |
| copy/move 缺 destination | 现有 `MissingDestination` |
| 无上一轮 / 越界 | 现有 `NoLastResults` / `IndexOutOfRange` 友好文案 |

**安全性质（不变）**：copy/move/rename 唯一 `invoke` 仍只在 `confirm_action_impl`；`handle_confirmable_action` 任何路径都不碰 invoke。多目标不引入新的 invoke 调用点。

## 6. 测试

**harness**：
- `execute_one` 目录 join：单目标 + 多目标（不同 basename 进同一目录均成功）。
- 预检冲突原子中止：N 目标中一个落点已存在 → `PathConflict` 且**其余文件未被创建**（断言磁盘零副作用）。
- `resolve_target_ref` `Paths` 臂：N 路径映射；空列表兜底。
- 改写 contract_39：dir 语义下，多目标不同名 → 成功；同名 → 冲突。
- 更新所有把 destination 当完整文件路径的旧单目标测试 → 目录语义。

**wiring（`search.rs`）**：
- copy/move 多目标：pending 为 `Paths{N}` + `destination=dir`（断言不含 basename）。
- rename N>1：友好错误，不存 pending。
- `resolve_destination` 简化后测试（只展开 ~ 得目录）。
- confirm 集成：单次 invoke 处理 N 目标，`MockFileActionExecutor` 断言 N 次 copy 调用 + 各落点 = `dir/basename`。
- 取消不执行：多目标 pending 下 cancel → 无 invoke。

**jsonschema 测试**：加 `Paths` 臂（require_non_empty values）。

**evals**：v0.5 parser-only 维持 byte-equal **472/26/2**（parser 不产 Paths，fixtures 不含新变体）。

## 7. 验证门

- `bash scripts/ci.sh` 全过（fmt + clippy + test，**per-task 验证含 `cargo fmt --check`**）。
- intent-parser / harness / desktop crate 测试全过。
- evals v0.5 parser-only `472/26/2` byte-equal。
- desktop src `grep` 无新增 `too_many_arguments` / `allow(dead_code)` 残留。
- macOS 真机手测：copy 多目标确认后真落地 N 文件 / move 多目标 / 取消不执行 / 同名冲突预检中止 / rename N>1 友好错误 / 超 10 个友好错误。
