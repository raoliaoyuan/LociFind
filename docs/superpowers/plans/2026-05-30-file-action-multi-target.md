# FileAction copy/move 多目标支持 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `把这些 pdf 复制到桌面` 这类多目标 copy/move intent 真复制/移动 N 个文件到目标目录（rename 维持单目标），保持「未确认绝不执行」安全性质。

**Architecture:** 方案 A——`FileAction.destination` 语义翻转为「目标目录」，harness `execute_one` 内部 `dir.join(basename)` 逐目标拼落点；`invoke` 加预检冲突 pass（任一落点已存在 → 整体中止零副作用）。新增 `TargetRef::Paths { values }` 让 confirm 的 pending 自包含 N 个绝对路径（避 context 漂移），`confirm_action_impl` 单次 invoke 即处理 N 目标。wiring 放开 copy/move 单目标闸（rename 保留），destination 不再 join basename（下放 harness）。

**Tech Stack:** Rust（workspace：`search-backends/common` schema、`harness`、`intent-parser`、`apps/desktop` Tauri）、TypeScript（`SearchView.tsx`）。验证：`bash scripts/ci.sh`（fmt + clippy + test）、evals v0.5 parser-only。

**设计参考：** [docs/superpowers/specs/2026-05-30-file-action-multi-target-design.md](../specs/2026-05-30-file-action-multi-target-design.md)

**全局约束（每个 task 都适用）：**
- 每个 task 的验证门**必须含 `cargo fmt --check`**（不能只 clippy+test）——见 [STATUS 第 33 阶段经验教训](../../../STATUS.md)。
- 改动面：schema 1 文件 + harness 2 文件 + jsonschema 测试 + wiring `search.rs` + `SearchView.tsx`。**parser 源不改**（`lib.rs:242` 的 `fa.target_ref` match 已有 `_ => None` catch-all，新变体不破坏它）。
- evals v0.5 parser-only 必须维持 byte-equal **472/26/2**（parser 不产 `Paths`）。

---

## 文件结构

| 文件 | 责任 | 本计划改动 |
|---|---|---|
| `packages/search-backends/common/src/lib.rs` | Search Intent schema 类型定义 | 加 `TargetRef::Paths { values: Vec<String> }` 变体 |
| `packages/search-backends/common/tests/jsonschema.rs` | schema 运行时校验测试 | `validate_target_ref` 加 `Paths` 臂 |
| `packages/harness/src/context.rs` | ContextMemory + target_ref 解析 | `resolve_target_ref` 加 `Paths` 臂 |
| `packages/harness/src/file_action_tool.rs` | FileActionTool 执行 + Policy | `execute_one` copy/move 目录 join；`invoke` 预检冲突；`dest_path_for` helper；测试更新 |
| `apps/desktop/src-tauri/src/search.rs` | Tauri 命令层 wiring | `handle_confirmable_action` 放开 copy/move 多目标；`resolve_destination` → 只展开目录；batch 预检；测试更新 |
| `apps/desktop/src/SearchView.tsx` | 前端搜索视图 | `describeConfirm` 支持 N 文件 |

---

## Task 1: schema 新增 `TargetRef::Paths` 变体

**Files:**
- Modify: `packages/search-backends/common/src/lib.rs:627-638`（`TargetRef` enum）
- Test: `packages/search-backends/common/tests/jsonschema.rs:94-108`（`validate_target_ref`）+ 同文件新增 serde 测试

- [ ] **Step 1: 写失败测试（serde roundtrip + jsonschema 校验）**

在 `packages/search-backends/common/tests/jsonschema.rs` 文件末尾的测试模块内（与现有 `#[test]` 同级）新增：

```rust
#[test]
fn target_ref_paths_serde_roundtrip() {
    use locifind_search_backend::TargetRef;
    let tr = TargetRef::Paths {
        values: vec!["/tmp/a.pdf".to_owned(), "/tmp/b.pdf".to_owned()],
    };
    let json = serde_json::to_string(&tr).unwrap();
    assert!(json.contains("\"source\":\"paths\""), "实得 {json}");
    let back: TargetRef = serde_json::from_str(&json).unwrap();
    assert_eq!(back, tr);
}

#[test]
fn validate_target_ref_paths_non_empty_ok() {
    use locifind_search_backend::TargetRef;
    let tr = TargetRef::Paths {
        values: vec!["/tmp/a.pdf".to_owned()],
    };
    assert!(validate_target_ref(&tr).is_ok());
}

#[test]
fn validate_target_ref_paths_empty_errs() {
    use locifind_search_backend::TargetRef;
    let tr = TargetRef::Paths { values: vec![] };
    assert!(validate_target_ref(&tr).is_err());
}
```

> 注：若该测试文件无 `#[test]` 模块包裹（顶层函数即测试），将上述三个函数直接置于文件末尾顶层即可。先确认文件结构（`validate_target_ref` 是否在 `mod` 内）再放置。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p locifind-search-backend --test jsonschema 2>&1 | tail -20`
Expected: 编译失败——`no variant named Paths`（变体未定义）。

- [ ] **Step 3: 加 `Paths` 变体到 schema**

`packages/search-backends/common/src/lib.rs`，把 `TargetRef`（行 627-638）改为：

```rust
/// 文件操作的目标指代（schema §4.6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum TargetRef {
    /// 上一轮搜索结果（由 Context Memory 保存）
    LastResults {
        /// 选择器
        selector: TargetSelector,
    },
    /// 直接指定绝对路径
    Path {
        /// 绝对路径
        value: String,
    },
    /// 直接指定一组绝对路径。确认流的自包含 pending 用（多目标 copy/move），
    /// 由 wiring 层在首次下发时把已解析的 N 个绝对路径写入，规避确认前 context 漂移。
    /// parser 不产此变体。
    Paths {
        /// 绝对路径列表
        values: Vec<String>,
    },
}
```

- [ ] **Step 4: jsonschema 校验加 `Paths` 臂**

`packages/search-backends/common/tests/jsonschema.rs`，`validate_target_ref`（行 94-108）改为：

```rust
fn validate_target_ref(target_ref: &TargetRef) -> Result<(), String> {
    match target_ref {
        TargetRef::LastResults { selector } => match selector {
            TargetSelector::Index { value } if *value == 0 => {
                Err("target index must be 1-based".to_owned())
            }
            TargetSelector::Indices { values } if values.is_empty() || values.contains(&0) => {
                Err("target indices must be non-empty and 1-based".to_owned())
            }
            TargetSelector::Index { .. } | TargetSelector::Indices { .. } | TargetSelector::All => {
                Ok(())
            }
        },
        TargetRef::Path { value } => require_non_empty(Some(value.as_str()), "target path"),
        TargetRef::Paths { values } => {
            if values.is_empty() {
                return Err("target paths must be non-empty".to_owned());
            }
            for v in values {
                require_non_empty(Some(v.as_str()), "target path")?;
            }
            Ok(())
        }
    }
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p locifind-search-backend --test jsonschema 2>&1 | tail -20`
Expected: 全部 PASS（含新增 3 个）。

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt -p locifind-search-backend --check && cargo clippy -p locifind-search-backend --all-targets -- -D warnings 2>&1 | tail -10`
Expected: 无 diff、无 warning。

- [ ] **Step 7: Commit**

```bash
git add packages/search-backends/common/src/lib.rs packages/search-backends/common/tests/jsonschema.rs
git commit -m "feat(schema): TargetRef 加 Paths 变体(多目标确认流自包含 pending 用)"
```

---

## Task 2: harness `resolve_target_ref` 支持 `Paths`

**Files:**
- Modify: `packages/harness/src/context.rs:99-107`（`resolve_target_ref`）
- Test: `packages/harness/src/context.rs`（测试模块，约行 777+）

- [ ] **Step 1: 写失败测试**

在 `packages/harness/src/context.rs` 测试模块内（与 `resolve_target_ref_path_returns_value` 同级，约行 777 附近）新增：

```rust
#[test]
fn resolve_target_ref_paths_returns_all() {
    use locifind_search_backend::TargetRef;
    let mem = ContextMemory::new();
    let target = TargetRef::Paths {
        values: vec!["/tmp/a.pdf".to_owned(), "/tmp/b.pdf".to_owned()],
    };
    let got = mem.resolve_target_ref(&target).unwrap();
    assert_eq!(
        got,
        vec![PathBuf::from("/tmp/a.pdf"), PathBuf::from("/tmp/b.pdf")]
    );
}

#[test]
fn resolve_target_ref_paths_empty_errs() {
    use locifind_search_backend::TargetRef;
    let mem = ContextMemory::new();
    let target = TargetRef::Paths { values: vec![] };
    assert!(matches!(
        mem.resolve_target_ref(&target),
        Err(TargetRefError::EmptyIndices)
    ));
}
```

> 注：`Paths` 是直接路径，不依赖 `last`，故空 context 也能解析（与 `Path` 一致）。空列表复用既有 `EmptyIndices` 错误兜底（schema 校验层已在 Task 1 阻止空列表，此为防御）。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p locifind-harness resolve_target_ref_paths 2>&1 | tail -20`
Expected: 编译失败——`match` 非穷尽 / `no variant Paths` 已有，故缺臂导致 `resolve_target_ref` 不覆盖（编译错 non-exhaustive）。

- [ ] **Step 3: 加 `Paths` 臂**

`packages/harness/src/context.rs`，`resolve_target_ref`（行 99-107）改为：

```rust
    pub fn resolve_target_ref(
        &self,
        target_ref: &TargetRef,
    ) -> Result<Vec<PathBuf>, TargetRefError> {
        match target_ref {
            TargetRef::Path { value } => Ok(vec![PathBuf::from(value)]),
            TargetRef::Paths { values } => {
                if values.is_empty() {
                    return Err(TargetRefError::EmptyIndices);
                }
                Ok(values.iter().map(PathBuf::from).collect())
            }
            TargetRef::LastResults { selector } => self.resolve_last_results(selector),
        }
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p locifind-harness resolve_target_ref 2>&1 | tail -20`
Expected: 全部 PASS。

- [ ] **Step 5: fmt + clippy + 全 crate test**

Run: `cargo fmt -p locifind-harness --check && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -10 && cargo test -p locifind-harness 2>&1 | tail -15`
Expected: 无 diff、无 warning；测试除 `file_action_tool.rs` 待 Task 3 更新的若干外应基本通过（若 `file_action_tool` 已有 `Paths` 相关 non-exhaustive 错，留 Task 3 修——本 task 不动 file_action_tool.rs）。

> 注：`file_action_tool.rs` 不 match `TargetRef`（它走 `context.resolve_target_ref`），故 Task 2 不会因 file_action_tool 编译失败。若 clippy 报 file_action_tool 其它无关问题，停下排查。

- [ ] **Step 6: Commit**

```bash
git add packages/harness/src/context.rs
git commit -m "feat(harness): resolve_target_ref 支持 TargetRef::Paths"
```

---

## Task 3: harness `execute_one` 目录 join + `invoke` 预检冲突

**Files:**
- Modify: `packages/harness/src/file_action_tool.rs`（`invoke` 行 230-292、`execute_one` 行 294-332）
- Test: 同文件测试模块（contract_39 行 628-665 改写；新增多目标成功 + 预检冲突原子中止测试）

- [ ] **Step 1: 写失败测试（多目标 copy 成功 + 预检冲突原子中止）**

在 `packages/harness/src/file_action_tool.rs` 测试模块内（contract_39 附近）新增：

```rust
/// 方案 A：多目标 copy，destination 当目录，逐目标 join basename。
#[test]
fn copy_multi_target_joins_basename_per_target() {
    let mock = Arc::new(MockExecutor::default());
    let tool = mk_tool(mock.clone());
    let context = mk_context_with(3);

    let action = FileAction {
        schema_version: SchemaVersion::V1,
        language: Some(Language::Zh),
        action: FileActionKind::Copy,
        target_ref: TargetRef::LastResults {
            selector: TargetSelector::Indices {
                values: vec![1, 2, 3],
            },
        },
        // destination 现在是目录（测试环境下不存在 → 预检通过）
        destination: Some("/tmp/locifind-multi-dest-nonexist".to_owned()),
        new_name: None,
        requires_confirmation: true,
    };

    let outcome = tool.invoke(&action, &context).unwrap();
    let FileActionOutcome::Executed { affected } = outcome else {
        panic!("expected Executed")
    };
    assert_eq!(affected.len(), 3);
    // 每个目标的 dest = dir.join(basename)
    assert_eq!(
        *mock.calls.lock().unwrap(),
        vec![
            MockCall::Copy(
                PathBuf::from("/tmp/test-1.txt"),
                PathBuf::from("/tmp/locifind-multi-dest-nonexist/test-1.txt")
            ),
            MockCall::Copy(
                PathBuf::from("/tmp/test-2.txt"),
                PathBuf::from("/tmp/locifind-multi-dest-nonexist/test-2.txt")
            ),
            MockCall::Copy(
                PathBuf::from("/tmp/test-3.txt"),
                PathBuf::from("/tmp/locifind-multi-dest-nonexist/test-3.txt")
            ),
        ]
    );
}

/// 预检：任一落点已存在 → 整体 PathConflict，零 executor 调用（原子中止）。
#[test]
fn copy_multi_target_preflight_conflict_atomic() {
    // 建一个真实临时目录，预先放一个与第 2 个目标同名的文件
    let dir = std::env::temp_dir().join(format!(
        "locifind-preflight-{}-{}",
        std::process::id(),
        "atomic"
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // mk_context_with 的源是 /tmp/test-N.txt，basename = test-N.txt
    std::fs::write(dir.join("test-2.txt"), b"x").unwrap();

    let mock = Arc::new(MockExecutor::default());
    let tool = mk_tool(mock.clone());
    let context = mk_context_with(3);

    let action = FileAction {
        schema_version: SchemaVersion::V1,
        language: Some(Language::Zh),
        action: FileActionKind::Copy,
        target_ref: TargetRef::LastResults {
            selector: TargetSelector::Indices {
                values: vec![1, 2, 3],
            },
        },
        destination: Some(dir.to_string_lossy().into_owned()),
        new_name: None,
        requires_confirmation: true,
    };

    let err = tool.invoke(&action, &context).unwrap_err();
    assert!(
        matches!(err, FileActionError::PathConflict { .. }),
        "实得 {err}"
    );
    // 原子性：预检失败 → 没有任何 executor 调用（连第 1 个都没执行）
    assert!(
        mock.calls.lock().unwrap().is_empty(),
        "预检冲突应零副作用, 实得 {:?}",
        mock.calls.lock().unwrap()
    );

    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p locifind-harness copy_multi_target 2>&1 | tail -25`
Expected: `copy_multi_target_joins_basename_per_target` FAIL——当前 `execute_one` 把 destination 当完整路径，3 次 copy 的 dest 都是 `/tmp/locifind-multi-dest-nonexist`（未 join basename），断言不匹配；且第 2 个起 `dest.exists()` 行为依赖磁盘。`copy_multi_target_preflight_conflict_atomic` FAIL——当前无预检，第 1 个会先执行（calls 非空）。

- [ ] **Step 3: 加 `dest_path_for` helper + 改 `execute_one` + `invoke` 预检**

`packages/harness/src/file_action_tool.rs`，在 `execute_one` 之后（行 332 后）新增 helper：

```rust
    /// copy/move 的落点：把 destination 当**目录**，join 源文件名。
    /// 返回 `dir.join(target.file_name())`。
    fn dest_path_for(action: &FileAction, target: &Path) -> Result<PathBuf, FileActionError> {
        let dir = action.destination.as_deref().unwrap_or_default();
        let file_name = target
            .file_name()
            .ok_or(FileActionError::MissingDestination)?;
        Ok(Path::new(dir).join(file_name))
    }
```

把 `execute_one` 的 Copy / Move 臂（行 304-323）改为（去掉原来的 `dest_path.exists()` 检查——预检已统一负责）：

```rust
            FileActionKind::Copy => {
                let dest_path = Self::dest_path_for(action, target)?;
                self.executor
                    .copy(target, &dest_path)
                    .map_err(FileActionError::Executor)
            }
            FileActionKind::Move => {
                let dest_path = Self::dest_path_for(action, target)?;
                self.executor
                    .move_to(target, &dest_path)
                    .map_err(FileActionError::Executor)
            }
```

在 `invoke` 的执行循环（行 286-289 `for target in &targets`）**之前**插入预检（紧接 §5 参数校验 `match action.action {...}` 之后、`// 6. 执行` 注释之前）：

```rust
        // 5.5 预检冲突（copy/move）：算出全部落点，任一已存在 → 整体中止，零副作用。
        if matches!(action.action, FileActionKind::Copy | FileActionKind::Move) {
            for target in &targets {
                let dest = Self::dest_path_for(action, target)?;
                if dest.exists() {
                    return Err(FileActionError::PathConflict { dest });
                }
            }
        }
```

- [ ] **Step 4: 改写 contract_39（dir 语义下多目标成功）**

`packages/harness/src/file_action_tool.rs`，把 `contract_39_copy_all_with_confirmation`（行 628-665）整体替换为：

```rust
    /// §7.6 #39 第二阶段：把 refine 后的全部 pdf 复制到桌面。
    /// 方案 A：destination 是目录，多目标各 join basename → 3 次 copy，落点互不冲突。
    #[test]
    fn contract_39_copy_all_with_confirmation() {
        let mock = Arc::new(MockExecutor::default());
        let tool = mk_tool(mock.clone());
        let context = mk_context_with(3);

        let action = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Copy,
            target_ref: TargetRef::LastResults {
                selector: TargetSelector::Indices {
                    values: vec![1, 2, 3],
                },
            },
            // 目录（测试环境不存在 → 预检通过）
            destination: Some("/tmp/locifind-contract39-dest".to_owned()),
            new_name: None,
            requires_confirmation: true,
        };

        let outcome = tool.invoke(&action, &context).unwrap();
        let FileActionOutcome::Executed { affected } = outcome else {
            panic!("expected Executed")
        };
        assert_eq!(affected.len(), 3);
        // requires_confirmation=true → 直接执行；3 个不同 basename 进同一目录无冲突。
        assert_eq!(mock.calls.lock().unwrap().len(), 3);
    }
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p locifind-harness 2>&1 | tail -25`
Expected: 全部 PASS（含新增 2 个 + 改写的 contract_39；`write_action_without_confirmation_returns_requires_confirmation` 在 policy 阶段提前返回 RequiresConfirmation 不受影响）。

> 若有其它把 destination 当完整文件路径的旧测试失败，按 dir 语义更新（dest 改为目录，断言 `dir.join(basename)`）。

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt -p locifind-harness --check && cargo clippy -p locifind-harness --all-targets -- -D warnings 2>&1 | tail -10`
Expected: 无 diff、无 warning。

- [ ] **Step 7: Commit**

```bash
git add packages/harness/src/file_action_tool.rs
git commit -m "feat(harness): copy/move destination 当目录逐目标 join + invoke 预检冲突原子中止"
```

---

## Task 4: wiring `handle_confirmable_action` 放开 copy/move 多目标

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（`handle_confirmable_action` 行 428-530、`resolve_destination` 行 677-688）
- Test: 同文件测试模块（行 1892/1956/2016 相关测试更新 + 新增 rename 多目标错误测试 + resolve_destination 测试更新）

- [ ] **Step 1: 写/改失败测试**

在 `apps/desktop/src-tauri/src/search.rs` 测试模块：

(a) 把 `confirmable_multi_target_errors_no_pending`（行 1955-1987）**改写**为「copy 多目标存 Paths pending」：

```rust
    #[tokio::test]
    async fn confirmable_copy_multi_target_stores_paths_pending() {
        use locifind_search_backend::{
            FileAction, FileActionKind, Language, SchemaVersion, TargetRef, TargetSelector,
        };
        let ctx = context_with_results(3);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        let action = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Copy,
            target_ref: TargetRef::LastResults {
                selector: TargetSelector::Indices { values: vec![1, 2] },
            },
            destination: Some("~/Desktop".to_owned()),
            new_name: None,
            requires_confirmation: true,
        };
        handle_confirmable_action(action, ch, &pending, &ctx)
            .await
            .unwrap();

        // 发 confirm_action 事件，paths 含 2 个源
        let evs = events.lock().unwrap();
        assert!(
            evs.iter()
                .any(|e| e.contains("\"confirm_action\"") && e.contains("copy")),
            "实得 {evs:?}"
        );
        drop(evs);

        // pending 自包含 Paths{2}，destination 为目录（不 join basename）
        let home = home_dir().unwrap();
        let dir_str = home.join("Desktop").to_string_lossy().into_owned();
        let p = pending.lock().unwrap();
        let pa = p.as_ref().unwrap();
        assert_eq!(pa.action, FileActionKind::Copy);
        assert_eq!(pa.destination.as_deref(), Some(dir_str.as_str()));
        match &pa.target_ref {
            TargetRef::Paths { values } => {
                assert_eq!(values, &vec!["/tmp/f0".to_owned(), "/tmp/f1".to_owned()]);
            }
            other => panic!("expected Paths, got {other:?}"),
        }
    }
```

(b) **新增** rename 多目标仍报错：

```rust
    #[tokio::test]
    async fn confirmable_rename_multi_target_errors_no_pending() {
        use locifind_search_backend::{
            FileAction, FileActionKind, Language, SchemaVersion, TargetRef, TargetSelector,
        };
        let ctx = context_with_results(3);
        let pending = empty_pending();
        let (ch, events) = capture_channel();

        let action = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Rename,
            target_ref: TargetRef::LastResults {
                selector: TargetSelector::Indices { values: vec![1, 2] },
            },
            destination: None,
            new_name: Some("final".to_owned()),
            requires_confirmation: true,
        };
        handle_confirmable_action(action, ch, &pending, &ctx)
            .await
            .unwrap();

        let evs = events.lock().unwrap();
        assert!(
            evs.iter()
                .any(|e| e.contains("\"error\"") && e.contains("一次只能重命名单个文件")),
            "实得 {evs:?}"
        );
        assert!(pending.lock().unwrap().is_none(), "rename 多目标不应存 pending");
    }
```

(c) 把 `confirmable_copy_stores_pending_and_emits_confirm`（行 1892）中对 `dest_str` 的断言改为目录（不 join basename）。当前它构造 `home.join("Desktop").join("f0")`；改为 `home.join("Desktop")`，并把 pending `target_ref` 断言从 `Path` 改为 `Paths{["/tmp/f0"]}`。读取该测试体后按下列模式更新：

```rust
        let home = home_dir().unwrap();
        let dir_str = home.join("Desktop").to_string_lossy().into_owned();
        // confirm 事件 destination 为目录
        let events = events.lock().unwrap();
        assert!(
            events
                .iter()
                .any(|e| e.contains("\"confirm_action\"") && e.contains(&dir_str)),
            "实得 {events:?}"
        );
        drop(events);
        let p = pending.lock().unwrap();
        let pa = p.as_ref().unwrap();
        assert_eq!(pa.destination.as_deref(), Some(dir_str.as_str()));
        assert!(matches!(&pa.target_ref, TargetRef::Paths { values } if values == &vec!["/tmp/f0".to_owned()]));
```

(d) 把 `confirmable_move_stores_pending`（行 2016-2042）的 pending 断言从 `TargetRef::Path { value } if value == "/tmp/f0"` 改为 `TargetRef::Paths { values } if values == &vec!["/tmp/f0".to_owned()]`。

(e) `resolve_destination` 简化后，删除依赖「join basename」的旧测试 `resolve_destination_expands_tilde_and_joins_basename`（行 1793）、`resolve_destination_absolute_passthrough`（行 1806）、`resolve_destination_no_filename_errs`（行 1813），新增：

```rust
    #[test]
    fn resolve_destination_dir_expands_tilde() {
        let home = home_dir().unwrap();
        assert_eq!(
            resolve_destination_dir("~/Desktop").unwrap(),
            home.join("Desktop")
        );
    }

    #[test]
    fn resolve_destination_dir_absolute_passthrough() {
        assert_eq!(
            resolve_destination_dir("/Users/x/Downloads").unwrap(),
            std::path::PathBuf::from("/Users/x/Downloads")
        );
    }
```

（`expand_tilde_bare_returns_home` 行 1801 保留不动。）

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p locifind-desktop confirmable_ 2>&1 | tail -30`
Expected: 编译/断言失败——`resolve_destination_dir` 未定义、copy/move 仍走单目标闸（多目标报错而非存 Paths pending）。

- [ ] **Step 3: 改 `handle_confirmable_action`（rename 单目标 / copy/move 多目标）**

`apps/desktop/src-tauri/src/search.rs`，把 `handle_confirmable_action`（行 428-530）的「2) 单目标校验」+「3) destination 解析」+「4) 构造 pending」+「5) 发 ConfirmAction」整段重构为：

```rust
    // 2) batch 上限（copy/move 多目标）：超过 harness 默认阈值直接友好错误
    use locifind_harness::file_action_tool::DEFAULT_BATCH_THRESHOLD;
    if targets.is_empty() {
        let _ = on_event.send(SearchEvent::Error {
            message: "没有可操作的目标".to_owned(),
        });
        return Ok(());
    }

    // rename：维持单目标（N 文件改 1 名无意义）
    if matches!(action.action, FileActionKind::Rename) && targets.len() != 1 {
        let _ = on_event.send(SearchEvent::Error {
            message: "一次只能重命名单个文件(多文件待后续)".to_owned(),
        });
        return Ok(());
    }
    // copy/move：放开多目标，但受 batch 上限保护
    if matches!(action.action, FileActionKind::Copy | FileActionKind::Move)
        && targets.len() > DEFAULT_BATCH_THRESHOLD
    {
        let _ = on_event.send(SearchEvent::Error {
            message: format!("目标过多(最多 {DEFAULT_BATCH_THRESHOLD} 个),请缩小范围"),
        });
        return Ok(());
    }

    // 3) copy/move 解析 destination 目录(展开 ~,不 join basename);rename 取 new_name
    let (destination, new_name) = match action.action {
        FileActionKind::Copy | FileActionKind::Move => {
            let hint = match action.destination.as_deref() {
                Some(h) if !h.is_empty() => h,
                _ => {
                    let _ = on_event.send(SearchEvent::Error {
                        message: "无法确定目标位置".to_owned(),
                    });
                    return Ok(());
                }
            };
            match resolve_destination_dir(hint) {
                Ok(p) => (Some(p.to_string_lossy().into_owned()), None),
                Err(msg) => {
                    let _ = on_event.send(SearchEvent::Error { message: msg });
                    return Ok(());
                }
            }
        }
        FileActionKind::Rename => match action.new_name.as_deref() {
            Some(n) if !n.is_empty() => (None, Some(n.to_owned())),
            _ => {
                let _ = on_event.send(SearchEvent::Error {
                    message: "未指定新文件名".to_owned(),
                });
                return Ok(());
            }
        },
        other => {
            unreachable!("handle_confirmable_action 只应处理 copy/move/rename, 实得 {other:?}")
        }
    };

    // 4) 构造自包含 pending(target_ref=Paths,确认时 invoke 不依赖 context)
    let path_strs: Vec<String> = targets
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let pending_action = FileAction {
        schema_version: action.schema_version,
        language: action.language,
        action: action.action,
        target_ref: TargetRef::Paths {
            values: path_strs.clone(),
        },
        destination: destination.clone(),
        new_name: new_name.clone(),
        requires_confirmation: true,
    };
    *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(pending_action);

    // 5) 发 ConfirmAction(paths=全部源,destination=目录)
    let action_kind = format!("{:?}", action.action).to_lowercase();
    let _ = on_event.send(SearchEvent::ConfirmAction {
        action_kind,
        paths: path_strs,
        destination,
        new_name,
    });
    Ok(())
```

> 注意：函数顶部已有 `use locifind_search_backend::{FileAction, FileActionKind, TargetRef};`（行 434）——`TargetSelector` 不需要。删除原「单目标校验」中 `let source = &targets[0];` 及其后续依赖 `source` 的逻辑（已被 `path_strs` 取代）。

- [ ] **Step 4: 改 `resolve_destination` → `resolve_destination_dir`（只展开目录）**

`apps/desktop/src-tauri/src/search.rs`，把 `resolve_destination`（行 677-688）替换为：

```rust
/// 把 parser 的 destination(如 "~/Desktop")展开为绝对**目录**。
/// basename join 下放 harness `execute_one`(方案 A,逐目标 join)。
fn resolve_destination_dir(dest_hint: &str) -> Result<std::path::PathBuf, String> {
    expand_tilde(dest_hint)
}
```

> `expand_tilde`（行 691）/ `home_dir`（行 702）保留不动。

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test -p locifind-desktop confirmable_ 2>&1 | tail -30 && cargo test -p locifind-desktop resolve_destination 2>&1 | tail -15`
Expected: 全部 PASS。

- [ ] **Step 6: fmt + clippy**

Run: `cargo fmt -p locifind-desktop --check && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -10`
Expected: 无 diff、无 warning。

- [ ] **Step 7: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "feat(desktop): handle_confirmable_action 放开 copy/move 多目标(Paths pending + 目录 destination),rename 维持单目标"
```

---

## Task 5: wiring confirm 集成测试（多目标端到端 + 修冲突测试语义）

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（测试 `confirm_action_invoke_error_maps_to_err_and_traces` 行 2161；新增多目标 confirm 集成测试；`pending_with_copy` 注释/用法）

- [ ] **Step 1: 修 `confirm_action_invoke_error_maps_to_err_and_traces`（dir 语义）**

当前该测试（行 2161-2202）把 dest 设为一个真实存在的**文件**来触发 PathConflict。方案 A 下 destination 是**目录**、harness join basename 后检查 `dir/f0`。改为：建临时目录 + 目录内放同名文件 `f0`：

```rust
    #[test]
    fn confirm_action_invoke_error_maps_to_err_and_traces() {
        // 方案 A：destination 是目录，harness 预检 dir.join(basename)。
        // 建临时目录 + 放与源 basename("f0")同名的文件 → 预检 PathConflict。
        let dir = std::env::temp_dir().join(format!("locifind-confirm-err-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f0"), b"x").unwrap();
        let pending = pending_with_copy("/tmp/f0", dir.to_str().unwrap());
        let (tool, calls) = build_file_action_tool();
        let (tracer, trace_calls) = build_tracer_with_mock();
        let ctx = empty_context();
        let deps = SearchDeps::new(
            empty_registry_arc(),
            Arc::new(PolicyEngine::new()),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        );

        let err = confirm_action_impl(&deps).unwrap_err();
        assert!(
            err.contains("已存在"),
            "应返回 PathConflict 友好文案, 实得 {err}"
        );
        assert!(
            calls.lock().unwrap().is_empty(),
            "PathConflict 不应到达 executor"
        );
        let tc = trace_calls.lock().unwrap();
        assert!(tc.iter().any(|c| c.starts_with("call:")));
        assert!(tc.iter().any(|c| c.starts_with("error:")));
        assert!(
            !tc.iter().any(|c| c.starts_with("result:")),
            "失败不应有 result"
        );
        drop(tc);
        assert!(pending.lock().unwrap().is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
```

> `pending_with_copy("/tmp/f0", dir)`：现在 `dest` 参数语义是「目录」。更新其文档注释（行 2075）为「destination 为目录（方案 A）」。

- [ ] **Step 2: 新增多目标 confirm 集成测试（Paths pending → N 次 copy）**

在 confirm 测试区新增。先加一个多目标 pending helper（紧邻 `pending_with_copy`）：

```rust
    /// 预置一个多目标 pending copy（target_ref=Paths，destination 为目录）。
    fn pending_with_copy_paths(
        srcs: &[&str],
        dest_dir: &str,
    ) -> Arc<Mutex<Option<locifind_search_backend::FileAction>>> {
        use locifind_search_backend::{
            FileAction, FileActionKind, Language, SchemaVersion, TargetRef,
        };
        let a = FileAction {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            action: FileActionKind::Copy,
            target_ref: TargetRef::Paths {
                values: srcs.iter().map(|s| (*s).to_owned()).collect(),
            },
            destination: Some(dest_dir.to_owned()),
            new_name: None,
            requires_confirmation: true,
        };
        Arc::new(Mutex::new(Some(a)))
    }

    #[test]
    fn confirm_action_multi_target_executes_n_copies() {
        // 目录不存在 → 预检通过 → 逐个 copy
        let dir = std::env::temp_dir().join(format!("locifind-multi-confirm-{}", std::process::id()));
        // 不创建 dir：MockExecutor 不写盘；预检只查 dir.join(basename).exists()=false
        let pending = pending_with_copy_paths(&["/tmp/f0", "/tmp/f1"], dir.to_str().unwrap());
        let (tool, calls) = build_file_action_tool();
        let (tracer, _t) = build_tracer_with_mock();
        let ctx = empty_context();
        let deps = SearchDeps::new(
            empty_registry_arc(),
            Arc::new(PolicyEngine::new()),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
            Arc::clone(&tool),
            Arc::clone(&pending),
        );

        let res = confirm_action_impl(&deps).unwrap();
        assert_eq!(res.action_kind, "copy");
        assert_eq!(res.paths, vec!["/tmp/f0".to_owned(), "/tmp/f1".to_owned()]);

        let calls = calls.lock().unwrap();
        assert_eq!(*calls, vec!["copy:/tmp/f0".to_owned(), "copy:/tmp/f1".to_owned()]);

        assert!(pending.lock().unwrap().is_none(), "confirm 后 pending 应清空");
    }
```

> 注：desktop `MockFileActionExecutor.copy` 只记录 `copy:{src}`（忽略 dest）——dest 落点的精确性已在 Task 3 的 harness 测试（`MockExecutor` 记录 src+dest）覆盖；本集成测试只验证 wiring→harness 的多目标 fan-out（N 次 copy）。

- [ ] **Step 3: 运行测试确认通过**

Run: `cargo test -p locifind-desktop confirm_action 2>&1 | tail -25`
Expected: 全部 PASS（含修改的冲突测试 + 新增多目标测试）。

- [ ] **Step 4: fmt + clippy + 全 desktop test**

Run: `cargo fmt -p locifind-desktop --check && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -10 && cargo test -p locifind-desktop 2>&1 | tail -15`
Expected: 无 diff、无 warning、全过。

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "test(desktop): confirm 多目标端到端集成 + 修 PathConflict 测试为目录语义"
```

---

## Task 6: UI `describeConfirm` 支持 N 文件

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx:403-420`（`describeConfirm`）

- [ ] **Step 1: 改 `describeConfirm`**

`apps/desktop/src/SearchView.tsx`，把 `describeConfirm`（行 403-420）改为：

```tsx
function describeConfirm(
  kind: string,
  paths: string[],
  destination: string | null,
  newName: string | null,
): string {
  const count = paths.length;
  const subject =
    count === 1 ? basename(paths[0]) : `${count} 个文件`;
  if (kind === "copy") {
    return `复制 ${subject} 到 ${destination ?? ""}?`;
  }
  if (kind === "move") {
    return `移动 ${subject} 到 ${destination ?? ""}?`;
  }
  if (kind === "rename") {
    return `重命名 ${basename(paths[0] ?? "")} 为 ${newName ?? ""}?`;
  }
  return `确认对 ${subject} 执行 ${kind}?`;
}
```

> rename 始终单目标，仍取 `paths[0]`。

- [ ] **Step 2: 类型检查 / 构建前端**

Run: `cd apps/desktop && npm run build 2>&1 | tail -15` （或项目既有的 TS 检查命令，如 `npx tsc --noEmit`；先看 `apps/desktop/package.json` scripts）
Expected: 无 TS 错误。

> 若 `npm run build` 触发完整 tauri 打包过慢，改用 `npx tsc --noEmit -p apps/desktop/tsconfig.json` 只做类型检查。

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "feat(desktop-ui): 确认对话框支持多文件 copy/move 文案"
```

---

## Task 7: 全量验证 + evals byte-equal + 残留扫描

**Files:** 无源改动（验证 + 准备收工材料）

- [ ] **Step 1: 全量 CI**

Run: `bash scripts/ci.sh 2>&1 | tail -30`
Expected: fmt + clippy + 全 workspace test 全过。

- [ ] **Step 2: evals v0.5 parser-only byte-equal**

Run: `cargo run -p locifind-evals --release --bin evals 2>&1 | tail -20`（parser-only baseline，不带 `--with-fallback`）。若该 bin 需子命令/参数，先看 `packages/evals/src/bin/evals.rs` 头部 usage。
Expected: **pass 472 / partial 26 / fail 2 / variant 99.6%**——与第 33 阶段 baseline byte-equal（parser 不产 `Paths`，fixtures 不含新变体）。

> 若数字偏离，停下排查：本计划不应改动任何 parser 路径或 fixture。

- [ ] **Step 3: 残留扫描**

Run: `grep -rn "too_many_arguments\|allow(dead_code)" apps/desktop/src-tauri/src/ packages/harness/src/file_action_tool.rs packages/harness/src/context.rs`
Expected: 无**新增**抑制（既有的若有，与本计划无关——确认未引入新的）。

- [ ] **Step 4: 真机手测清单（用户驱动）**

提示用户启动 `LOCIFIND_TRACE=/tmp/locifind-trace-multi.jsonl npm run tauri dev`，agent 盯 trace + dev stderr，跑以下路径：

1. 先搜出多个结果（如 `find pdf`）→ `把这些复制到桌面` → 弹「复制 N 个文件到 ~/Desktop?」→ 确认 → trace 1 call + 1 result(result_count=N) → 桌面真出现 N 个文件。
2. `把这些移动到下载` → 确认 → N 文件真移动。
3. 多目标 + 取消 → 无 file_action 执行（trace 不增 result）。
4. 同名冲突：目标目录已有同名文件 → 确认 → 友好「已存在」错误 + trace error（非 result）+ 零文件改动。
5. rename 多目标 `把这些重命名为 final` → 友好错误「一次只能重命名单个文件」。
6. 超过 10 个结果 `把这些复制到桌面` → 友好错误「目标过多」。

- [ ] **Step 5: 收工（更新 STATUS / ROADMAP）**

按 [CONVENTIONS.md §3 收工流程](../../../CONVENTIONS.md)：STATUS.md 顶部追加第 34 阶段段 + 会话日志；ROADMAP 同步 Class B「多目标支持」状态；本计划勾选完成。一次中文 commit 落库。

---

## 自审（Self-Review）

**Spec 覆盖：**
- §2 决策 1（harness join）→ Task 3 ✓
- §2 决策 2（TargetRef::Paths）→ Task 1（schema）+ Task 2（resolve）✓
- §2 决策 3（预检冲突）→ Task 3 Step 3 + 测试 ✓
- §2 决策 4（batch 上限）→ Task 4 Step 3（wiring 用 `DEFAULT_BATCH_THRESHOLD` 预检 + harness invoke 兜底）✓
- §3 组件清单：schema(T1) / context(T2) / file_action_tool(T3) / wiring(T4,T5) / UI(T6)；parser 无需改（catch-all）已在全局约束说明 ✓
- §4 数据流：T4（首次下发 Paths pending）+ T5（confirm 单次 invoke 处理 N）✓
- §5 错误处理：预检冲突(T3)、N>10(T4)、rename N>1(T4)、缺 destination(T4 沿用)✓
- §6 测试：harness(T3) / wiring(T4,T5) / jsonschema(T1) / evals(T7) ✓
- §7 验证门：T7 ✓

**占位符扫描：** 无 TBD/TODO；每个代码 step 含完整代码。

**类型一致性：** `TargetRef::Paths { values: Vec<String> }`（T1）在 T2/T4/T5 一致使用 `values`；`dest_path_for`(T3) / `resolve_destination_dir`(T4) / `describeConfirm`(T6) 签名前后一致；`DEFAULT_BATCH_THRESHOLD` 为 harness 既有 pub const（已核实 file_action_tool.rs:36）。
