# BETA-06 Audit Log — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B2 BETA-06；承接 MVP-10A FileActionTool；服务 PROJECT「可解释可控」
> ID：BETA-06

## 1. 背景与目标

PROJECT 原则「可解释可控：Agent 每一步工具调用、权限判断、错误状态可追踪」。现有 `Tracer`
是**开发调试观测**（默认 noop、`LOCIFIND_TRACE` env 开关、临时文件、路径脱敏）。缺一个**面向用户的、
持久的、可查看可清除的敏感操作记录**——文件操作（open/locate/copy/move/rename）做了什么、对哪些文件、
结果如何。

BETA-06 提供持久 Audit Log：**每次文件操作执行后记一条**，用户可在设置页查看 / 一键清除。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | 存储 | **append-only JSONL**（`data_dir/LociFind/audit.jsonl`）；轻量，serde_json 已在 harness，不拉 rusqlite 进 harness；追加/读全/清空都简单，契合审计日志（追加为主、整读展示、一键清） |
| ② | 记录点 | **desktop 执行点**：3 个 `invoke` 调用点（open/locate、handle_file_action、confirm copy/move/rename）执行后经 helper 记一条。**保持 FileActionTool 单一职责**；desktop 是唯一文件操作调用方，覆盖全部真实使用 |

## 3. 架构

### 3.1 harness `audit` 模块（`packages/harness/src/audit.rs`，新；无新依赖）

```rust
#[derive(Serialize, Deserialize)] // serde
pub enum AuditOperation { Open, Locate, Copy, Move, Rename }

pub enum AuditResult { Executed, Failed }

pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub operation: AuditOperation,
    pub source_paths: Vec<String>,    // 操作的源路径（本地全路径——用户的自有记录、可清除、不上传）
    pub destination: Option<String>,  // copy/move 目标目录
    pub new_name: Option<String>,     // rename 新名
    pub result: AuditResult,
    pub error: Option<String>,        // 失败时的错误分类（如 "PathConflict"）
}

/// 持久审计日志：追加、整读、清空。失败内部 eprintln（审计绝不让 app 崩）。
pub trait AuditLog: Send + Sync {
    fn record(&self, entry: &AuditEntry);
    fn read_all(&self) -> Vec<AuditEntry>;
    fn clear(&self);
}

/// append-only JSONL 文件实现（每行一条 JSON）。
pub struct JsonlAuditLog { path: PathBuf, write_lock: Mutex<()> }
impl JsonlAuditLog { pub fn new(path: PathBuf) -> Self; }

/// 内存实现（测试用）。
pub struct InMemoryAuditLog { entries: Mutex<Vec<AuditEntry>> }
```

- `JsonlAuditLog::record`：`Mutex` 串行化下 `OpenOptions::append` 写一行 `serde_json` + `\n`；
  父目录不存在则 create_dir_all；IO 失败 eprintln 不 panic。
- `read_all`：读文件按行 `serde_json::from_str`，解析失败的行跳过（容错），newest-last（调用方可 reverse）。
- `clear`：删除文件（或截断）。

### 3.2 desktop 记录 helper（`search.rs`）

```rust
/// 文件操作执行后记一条审计（仅 Executed/Failed 记；RequiresConfirmation 未执行不记）。
fn record_audit(audit: &dyn AuditLog, action: &FileAction, outcome: &Result<FileActionOutcome, FileActionError>);
```

- operation 由 `action.action` 映射（Delete 永不到此）。
- Executed → `source_paths = affected`（权威），result=Executed，error=None。
- Err → `source_paths = action_self_contained_paths(action)`（TargetRef::Path/Paths 自包含；LastResults 解析失败则空），result=Failed，error=`file_action_error_kind(e)`。
- destination=`action.destination`，new_name=`action.new_name`。
- RequiresConfirmation → 不记（未执行）。

在 `run_path_action`（open/locate）、`handle_file_action`（open/locate）、`confirm_action_impl`
（copy/move/rename）三处 invoke 后调用。

### 3.3 SearchDeps + 命令

- `SearchDeps` 加 `audit: Arc<dyn AuditLog>` 字段；main.rs 构造 `JsonlAuditLog::new(data_dir/LociFind/audit.jsonl)`。
- 新命令：
  ```rust
  #[tauri::command] async fn get_audit_log(deps) -> Result<Vec<AuditEntryJson>, String>; // newest-first
  #[tauri::command] async fn clear_audit_log(deps) -> Result<(), String>;
  ```
  `AuditEntryJson`：timestamp(rfc3339) / operation / source_paths / destination / new_name / result / error。

### 3.4 设置页「操作记录」

SettingsPage.tsx 加一节：加载 `get_audit_log` 显示最近 N 条（时间 / 操作 / 源路径 / 目标 / 结果），
+「清除记录」按钮（`clear_audit_log`）。隐私一句：本地记录、不上传、可一键清除。

## 4. 隐私

- **本地优先**：audit.jsonl 在用户 data_dir，**永不上传**、不进任何 telemetry。
- **全路径**：审计要透明（用户需知道操作了哪些文件），记全路径——这是用户自有的本地记录、可一键清除，
  与「dev tracing 默认脱敏」不冲突（那是开发观测，会外发给开发者）。
- **可清除**：设置页一键清；BETA-12 卸载流程清 audit.jsonl（登记 backlog）。
- **不记**：文件内容、搜索查询词（那是 dev tracing 范畴）。

## 5. 验收 / 验证门

1. **audit 模块单测**（in-memory + 临时文件 JSONL）：record→read_all 往返；多条顺序；clear 清空；
   JSONL 损坏行跳过容错；并发 record 不丢/不串行（Mutex）；serde round-trip（含 CJK 路径）。
2. **desktop record helper 单测**：Executed→记 affected + Executed；Err→记 Failed + error 分类；
   RequiresConfirmation→不记；各 operation 映射正确。注入 `InMemoryAuditLog` 断言。
3. **desktop 集成**：现有 file-action 测试零回归（加 audit 字段后）；至少 1 测断言「open 执行后 audit 有 1 条」。
4. **命令**：get_audit_log newest-first + clear 清空（单测 impl 函数）。
5. **零回归**：全 workspace test（除 platform-macos 预存）+ fmt + clippy `-D warnings`。无新外部依赖。
6. **UI 手测**（用户）：做几次 open/copy → 设置页「操作记录」见条目 → 清除 → 空。落 manual-test。
7. **文档**：harness README/audit 段 + privacy-security 文档补 audit 一条 + ROADMAP done + STATUS。

## 6. 非目标（YAGNI）

- 不审计搜索查询 / 权限 Deny 单独事件（v1 仅文件操作执行；Deny 体现为 Failed 或不到执行）。
- 不做筛选 / 导出 CSV / 保留期自动清理（v1 整读 + 一键清；后续按需）。
- 不审计 cancel（用户取消 = 未执行，无操作可记）。
- 不在 FileActionTool 内记录（保持单一职责）。
- 不做路径脱敏开关（audit 本地透明记全路径；脱敏是 dev tracing 的事）。
- 不做日志轮转（append-only；超大留后续，audit 量级小）。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| audit 写失败影响主流程 | record 内部 eprintln 不 panic、不返 Result 进主路径 |
| 并发 record 串行/损坏行 | Mutex 串行化写；read_all 容错跳过坏行 |
| 全路径隐私 | 本地 data_dir、不上传、可一键清；文档说明；与 dev tracing 脱敏分离 |
| Err 时源路径不全（LastResults 未解析） | best-effort（自包含 Path/Paths 记，否则空）；Executed 用权威 affected |
