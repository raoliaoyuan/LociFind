# BETA-06 Audit Log Implementation Plan

> Steps use checkbox (`- [ ]`). 每 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** 持久 Audit Log（append-only JSONL）记录文件操作（open/locate/copy/move/rename）执行结果，desktop 执行点记录，设置页查看 + 一键清除。无新外部依赖。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-06-audit-log-design.md) §3。harness `audit` 模块（AuditEntry/AuditLog/JsonlAuditLog/InMemoryAuditLog）；desktop record helper + 命令 + UI。

**Tech Stack:** Rust；serde_json（已在 harness）；chrono（已在 harness）。

---

## Task 1: harness `audit` 模块

**Files:** `packages/harness/src/audit.rs`（新）+ `lib.rs`（mod + re-export）+ 测试同文件。

- [ ] **Step 1:** `audit.rs`：`AuditOperation`（serde snake_case）+ `AuditResult` + `AuditEntry`（Serialize/Deserialize/Debug/Clone/PartialEq）+ `AuditLog` trait（record/read_all/clear）。
- [ ] **Step 2:** `JsonlAuditLog { path, write_lock: Mutex<()> }` + `new`：record（Mutex 串行 + OpenOptions append + create_dir_all + serde_json line + `\n`，IO 失败 eprintln）；read_all（读文件按行 from_str，坏行跳过）；clear（remove_file，忽略 NotFound）。
- [ ] **Step 3:** `InMemoryAuditLog { entries: Mutex<Vec<AuditEntry>> }` impl AuditLog（测试用）。
- [ ] **Step 4:** lib.rs `pub mod audit; pub use audit::{AuditEntry, AuditLog, AuditOperation, AuditResult, JsonlAuditLog, InMemoryAuditLog};`
- [ ] **Step 5:** 单测：① JsonlAuditLog 临时文件 record×3 → read_all 顺序 + 字段（含 CJK 路径）；② clear 清空；③ 坏行（写一行非法 JSON）跳过容错；④ InMemoryAuditLog record/read/clear；⑤ AuditEntry serde round-trip。
- [ ] **Step 6:** fmt + clippy + test。
- [ ] **Step 7:** Commit `feat(harness): audit 模块（append-only JSONL 审计日志）`。

## Task 2: desktop 记录 helper + 命令 + SearchDeps

**Files:** `apps/desktop/src-tauri/src/search.rs`（record_audit helper + 3 处调用 + SearchDeps 字段）+ `main.rs`（构造 JsonlAuditLog + SearchDeps）+ `audit_cmd.rs` 或 search.rs（get/clear 命令）+ 测试。

- [ ] **Step 1:** `SearchDeps` 加 `audit: Arc<dyn AuditLog>` 字段 + `new` 参数 + `audit()` getter；main.rs 构造 `JsonlAuditLog::new(data_dir/LociFind/audit.jsonl)` 传入。
- [ ] **Step 2:** `record_audit(audit, action, outcome)` helper（spec §3.2）：operation 映射 / Executed→affected / Err→自包含路径 + error_kind / RequiresConfirmation→不记。`action_self_contained_paths(action)` helper（TargetRef::Path/Paths）。
- [ ] **Step 3:** 在 `run_path_action`、`handle_file_action`（open/locate Executed/Err 臂）、`confirm_action_impl` 三处 invoke 后调 `record_audit(deps.audit(), &action, &outcome)`。（注意 outcome 在 match 前先持有引用。）
- [ ] **Step 4:** `AuditEntryJson`（Serialize）+ `get_audit_log_impl(deps) -> Vec<AuditEntryJson>`（read_all → newest-first → map）+ `clear_audit_log_impl(deps)`；`#[tauri::command] get_audit_log / clear_audit_log` + 注册 invoke_handler。
- [ ] **Step 5:** 测试：所有既有 SearchDeps::new 调用加 audit 参数（用 `Arc::new(InMemoryAuditLog::default())`）；新增——open 执行后 InMemoryAuditLog 有 1 条 Executed；copy 失败（mock executor err 或越界）记 Failed；record_audit 各分支单测（Executed/Err/RequiresConfirmation）；get_audit_log newest-first + clear。
- [ ] **Step 6:** fmt + clippy + test（desktop）。
- [ ] **Step 7:** Commit `feat(desktop): 文件操作执行点记审计 + get/clear_audit_log 命令`。

## Task 3: 设置页 UI + 文档 + 全套 CI

**Files:** `apps/desktop/src/pages/SettingsPage.tsx`（操作记录节）+ `packages/harness/README.md`（如有）/ `docs/privacy-security.md`（audit 条）+ `docs/manual-test-scenarios.md`（BETA-06）+ `ROADMAP.md` + `STATUS.md`。

- [ ] **Step 1:** SettingsPage.tsx 加「操作记录」节：`invoke('get_audit_log')` 加载，表格显示最近条目（时间/操作/源路径/目标/结果），「清除记录」按钮 `invoke('clear_audit_log')` + 隐私一句；tsc 通过。
- [ ] **Step 2:** privacy-security.md 补 audit log 一条（本地、全路径、可清、不上传）；BETA-12 卸载清 audit.jsonl 登记 backlog（ROADMAP/STATUS 备注）。
- [ ] **Step 3:** manual-test BETA-06 节（做几次 open/copy → 设置页见记录 → 清除 → 空）。
- [ ] **Step 4:** `bash scripts/ci.sh`（platform-macos 预存除外）+ 全 workspace test 零回归。无新外部依赖（serde_json/chrono 已在台账）。
- [ ] **Step 5:** ROADMAP BETA-06 → done + 实证；STATUS 当前阶段 + 会话日志。
- [ ] **Step 6:** 收工 commit + 向用户确认（真机手测留用户）。

---

## 验收对照（spec §5）

- audit 模块 JSONL/InMemory 单测（T1）；desktop record helper + 命令 + 零回归（T2）；UI + docs + ci（T3）。
