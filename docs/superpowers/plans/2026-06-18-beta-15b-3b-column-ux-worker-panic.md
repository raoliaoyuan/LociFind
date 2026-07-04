# BETA-15B-3 簇 B 实施计划：「匹配方式」列一次性迁移 + 语义 worker panic 兜底

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让旗舰「匹配方式」语义列对老用户浮现（一次性 localStorage 迁移），并消除语义 worker panic 泄漏并发守卫（catch_unwind 兜底清守卫）。

**Architecture:** 两个独立小项。① 前端 `SearchView.tsx`：`ColumnPrefs` 加 `version`，`loadColumnPrefs` 经纯函数 `migrateColumnPrefs` 对 v2 前的 prefs 注入一次 `match` 列并回写。② 后端 `index_status.rs`：新增 `run_semantic_worker` 用 `catch_unwind` 包 `semantic_index_pass`，panic 时 `semantic_abort` 清守卫降级；`spawn_semantic_index` 改调它。

**Tech Stack:** React/TypeScript（Tauri，无 JS 测试 runner，靠 tsc + 手测）、Rust（std::panic::catch_unwind）。

参考 spec：[docs/superpowers/specs/2026-06-18-beta-15b-3b-column-ux-worker-panic-design.md](../specs/2026-06-18-beta-15b-3b-column-ux-worker-panic-design.md)

---

## File Structure

| 文件 | 职责 | 改动 |
|---|---|---|
| `apps/desktop/src/SearchView.tsx` | 搜索结果列表 + 列偏好 | `ColumnPrefs` 加 `version`；新增 `COLUMN_PREFS_VERSION` + 纯函数 `migrateColumnPrefs`；`loadColumnPrefs`/`defaultColumnPrefs` 接入 |
| `apps/desktop/src-tauri/src/search/index_status.rs` | 索引状态 + 语义 worker | 新增 `run_semantic_worker`（catch_unwind 兜底）；`spawn_semantic_index` 改调它；`decouple_tests` 加 `PanicEmbedder` + 2 测试 |
| `docs/manual-test-scenarios.md` | 真机手测登记 | 加簇 B 节 |

---

## Task 1: 「匹配方式」列一次性迁移（前端）

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx:263-301`（`COLS_STORAGE_KEY` / `ColumnPrefs` / `defaultColumnPrefs` / `loadColumnPrefs`）

> 无 JS 测试 runner（已核实 package.json 无 vitest/jest）。本 task 验证 = `tsc --noEmit` 类型门 + 迁移逻辑抽纯函数便于推理 + 手测登记（Task 3）。无红/绿测试步骤，改为「实现 → 类型门 → 自查迁移逻辑」。

- [ ] **Step 1: 改 `ColumnPrefs` 接口 + 加版本常量 + `defaultColumnPrefs`**

把现有 `COLS_STORAGE_KEY`（263 行）下方的 `ColumnPrefs` 接口（265-270）与 `defaultColumnPrefs`（272-277）改为：

```tsx
const COLS_STORAGE_KEY = "locifind.columns.v1";
// BETA-15B-3：列偏好 schema 版本。v2 = 引入「匹配方式」语义列；驱动一次性列迁移。
const COLUMN_PREFS_VERSION = 2;

interface ColumnPrefs {
  /** 可见列 key（保持 ALL_COLUMNS 顺序渲染） */
  visible: ColKey[];
  /** 每列宽度覆盖（缺省用 defaultWidth） */
  widths: Partial<Record<ColKey, number>>;
  /** BETA-15B-3：prefs schema 版本，缺省（旧数据）视为 1。 */
  version: number;
}

function defaultColumnPrefs(): ColumnPrefs {
  return {
    visible: ALL_COLUMNS.filter((c) => c.defaultVisible).map((c) => c.key),
    widths: {},
    version: COLUMN_PREFS_VERSION,
  };
}
```

- [ ] **Step 2: 新增纯函数 `migrateColumnPrefs` + 改 `loadColumnPrefs`**

把现有 `loadColumnPrefs`（279-293）替换为「纯迁移函数 + 薄 load」：

```tsx
/**
 * 纯函数：把解析出的（可能旧版）prefs 迁移到当前 schema。
 * v2 前的 prefs 早于「匹配方式」列 → 注入一次 match（老用户从未见过该列、不可能主动隐藏过），
 * 标 version=2；此后尊重用户选择（手动隐藏 match 不再被强加）。
 * 返回 { prefs, migrated }；migrated=true 时调用方应回写持久化，使迁移只发生一次。
 */
function migrateColumnPrefs(parsed: Partial<ColumnPrefs>): {
  prefs: ColumnPrefs;
  migrated: boolean;
} {
  const validKeys = new Set<ColKey>(ALL_COLUMNS.map((c) => c.key));
  let visible = Array.isArray(parsed.visible)
    ? parsed.visible.filter((k): k is ColKey => validKeys.has(k as ColKey))
    : defaultColumnPrefs().visible;
  if (!visible.includes("name")) visible = ["name", ...visible]; // 名称列始终可见
  const version = typeof parsed.version === "number" ? parsed.version : 1;
  let injected = false;
  if (version < 2 && !visible.includes("match")) {
    // 一次性补显旗舰语义列（渲染按 ALL_COLUMNS 顺序，visible 内位置无关）。
    visible = [...visible, "match"];
    injected = true;
  }
  const prefs: ColumnPrefs = {
    visible,
    widths: parsed.widths ?? {},
    version: COLUMN_PREFS_VERSION,
  };
  // injected 或仅版本落后（含已有 match 的 v1）都回写一次，把 version 升到当前。
  return { prefs, migrated: injected || version < COLUMN_PREFS_VERSION };
}

function loadColumnPrefs(): ColumnPrefs {
  try {
    const raw = localStorage.getItem(COLS_STORAGE_KEY);
    if (!raw) return defaultColumnPrefs();
    const parsed = JSON.parse(raw) as Partial<ColumnPrefs>;
    const { prefs, migrated } = migrateColumnPrefs(parsed);
    if (migrated) saveColumnPrefs(prefs); // 迁移结果回写，迁移只发生一次
    return prefs;
  } catch {
    return defaultColumnPrefs();
  }
}
```

`saveColumnPrefs`（295-301）不变——它 `JSON.stringify(prefs)` 现在会带上 `version`，向后兼容（旧版读到多余 `version` 字段无害）。

- [ ] **Step 3: 排查其它 `ColumnPrefs` 字面量构造点**

`ColumnPrefs` 现在 `version` 为必填。grep 确认无其它地方手写 `{ visible, widths }` 字面量（漏 `version` 会 tsc 报错）：

Run: `grep -n "ColumnPrefs\|visible:.*widths:\|setColumnPrefs\|useState<ColumnPrefs" apps/desktop/src/SearchView.tsx`
预期：构造点只有 `defaultColumnPrefs` / `migrateColumnPrefs`（均已带 version）；state 用 `useState<ColumnPrefs>(loadColumnPrefs)` 或类似（值来自上述函数，类型自洽）。若发现裸字面量构造，补 `version: COLUMN_PREFS_VERSION`。

- [ ] **Step 4: 类型门**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: PASS（严格 tsconfig 零类型错误）。

- [ ] **Step 5: 自查迁移逻辑（对照三场景）**

人工核对 `migrateColumnPrefs`：
- 新安装（`!raw`）→ `loadColumnPrefs` 直接 `defaultColumnPrefs()`（含 match + version=2），不进迁移。✓
- 老用户 v1（无 version、visible 无 match）→ `version=1<2` 且无 match → 注入 match、`migrated=true` → 回写。✓
- 已迁移 v2 用户手动隐藏 match（version=2、visible 无 match）→ `version=2` 不 `<2` → 不注入、`migrated=false` → 尊重隐藏。✓

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "BETA-15B-3 簇B-1：匹配方式列一次性迁移（老用户补显旗舰语义列）"
```

---

## Task 2: 语义 worker panic 兜底（后端）

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/index_status.rs`（新增 `run_semantic_worker`；`spawn_semantic_index` 改调；`decouple_tests` 模块加测试）

- [ ] **Step 1: 写失败测试**

在 `index_status.rs` 的 `decouple_tests` 模块内（15B-2 已建，含 `StubEmbedder` + `temp_fts_db` 辅助）追加。先把模块顶部的 `#![allow(clippy::unwrap_used)]` 扩为 `#![allow(clippy::unwrap_used, clippy::panic)]`（PanicEmbedder 用 `panic!`，本 crate 启用了 `clippy::panic` restriction 需 allow）。然后加：

```rust
    /// 故意在 embed 时 panic，验证 worker 兜底清守卫。
    struct PanicEmbedder;
    impl locifind_indexer::embed::TextEmbedder for PanicEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, locifind_indexer::IndexError> {
            panic!("boom: simulated embed panic");
        }
        fn model_id(&self) -> &str {
            "panic"
        }
    }

    #[test]
    fn run_semantic_worker_clears_guard_on_panic() {
        let dir = tempfile::tempdir().unwrap();
        let (db, roots) = temp_fts_db(dir.path());
        let status = Arc::new(Mutex::new(IndexStatus::default()));

        // embed_pending 触达 PanicEmbedder（temp_fts_db 有 2 篇待嵌）→ panic 经 catch_unwind 兜底。
        // 注：catch_unwind 仍会经默认 panic hook 往 stderr 打一行 backtrace，属预期、无害。
        run_semantic_worker(&status, &db, true, &PanicEmbedder, &roots);

        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing, "panic 后守卫已清，不泄漏");
        assert_eq!(s.semantic_progress, None);
        assert_eq!(
            s.semantic_summary.as_deref(),
            Some("语义索引意外中断，已降级 FTS-only")
        );
    }

    #[test]
    fn run_semantic_worker_normal_path_finalizes() {
        let dir = tempfile::tempdir().unwrap();
        let (db, roots) = temp_fts_db(dir.path());
        let status = Arc::new(Mutex::new(IndexStatus::default()));

        // 正常 stub embedder 经兜底外壳与直接 semantic_index_pass 行为一致（就绪摘要）。
        run_semantic_worker(&status, &db, true, &StubEmbedder, &roots);

        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing);
        assert_eq!(s.semantic_summary.as_deref(), Some("语义索引就绪 2 篇"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop run_semantic_worker -- --nocapture`
Expected: FAIL（`run_semantic_worker` 未定义）

- [ ] **Step 3: 实现 `run_semantic_worker` + 改 `spawn_semantic_index`**

在 `index_status.rs` 的 `semantic_index_pass` 之后、`spawn_semantic_index` 之前插入：

```rust
/// 语义 worker 的 panic 兜底外壳（可单测）：`catch_unwind` 包 `semantic_index_pass`，
/// panic 时清守卫（`semantic_abort`）降级 FTS-only——守卫不泄漏，UI 不卡「语义索引中」。
/// （`embedder.embed()` 走 FFI 等理论上可能 panic；正常 `Result` 错误仍由 `semantic_index_pass` 内部处理。）
pub(crate) fn run_semantic_worker(
    status: &Arc<Mutex<IndexStatus>>,
    db_path: &std::path::Path,
    prewarmed: bool,
    embedder: &dyn locifind_indexer::embed::TextEmbedder,
    roots: &[std::path::PathBuf],
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        semantic_index_pass(status, db_path, prewarmed, embedder, roots);
    }));
    if result.is_err() {
        eprintln!("语义索引 worker panic，已清守卫降级 FTS-only");
        semantic_abort(status, "语义索引意外中断，已降级 FTS-only");
    }
}
```

把 `spawn_semantic_index` 内 `spawn_blocking` 闭包里对 `semantic_index_pass(...)` 的调用改为 `run_semantic_worker(...)`：

```rust
    tauri::async_runtime::spawn_blocking(move || {
        let prewarmed = embedding.prewarm();
        let roots = locifind_indexer::default_document_roots();
        run_semantic_worker(&status, &db_path, prewarmed, embedding.as_ref(), &roots);
    });
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop run_semantic_worker`
Expected: PASS（2 个测试；panic 测试的 stderr backtrace 属预期）

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt -p locifind-desktop
cargo clippy -p locifind-desktop --all-targets -- -D warnings
git add apps/desktop/src-tauri/src/search/index_status.rs
git commit -m "BETA-15B-3 簇B-2：语义 worker panic 兜底（catch_unwind 清守卫降级）"
```

---

## Task 3: 回归门 + 手测登记 + 收尾

**Files:**
- Modify: `docs/manual-test-scenarios.md`

- [ ] **Step 1: 全量回归门**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop && npx tsc --noEmit
```
Expected: fmt 净；clippy 仅一条无害 non-root profile 提示；`cargo test --workspace` 全绿（含两个新 `run_semantic_worker` 测试）；tsc 零错误。

- [ ] **Step 2: evals byte-equal 硬门**

本切片不碰 parser/expand，evals 应不动（基线 v0.5=473 / v0.9=726）。
Run: `cargo run -p locifind-evals --bin evals -- --fixtures v0.5`（再 `--fixtures v0.9`）
Expected: v0.5 pass 473、v0.9 pass 726（与基线逐字相符）。

- [ ] **Step 3: 登记真机手测**

在 `docs/manual-test-scenarios.md` 加「BETA-15B-3 簇 B」节：

```markdown
## BETA-15B-3 簇 B（列 UX 迁移 + worker panic 兜底）

1. **列迁移（老用户补显）**：用一份「匹配方式」列隐藏的旧 localStorage（`locifind.columns.v1` 的 visible 不含 `match`、无 version）→ 升级后启动 app → 结果列表「匹配方式」列**自动出现**，语义命中显「按意思找到」徽标。
2. **迁移只一次 + 尊重意图**：步骤 1 后手动隐藏「匹配方式」列 → 重启 app → 该列**仍隐藏**（迁移已标 version=2，不再强加）。
3. **新安装**：清空 localStorage → 启动 → 「匹配方式」列默认可见（`defaultVisible:true`）。
4. **worker 兜底**（难自然触发，主要靠单测 `run_semantic_worker_clears_guard_on_panic`）：若能注入会 panic 的 embedding 模型，验证设置页不卡「语义索引中」、降级 FTS-only。
```

- [ ] **Step 4: Commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-15B-3 簇B-3：回归门通过 + 登记真机手测场景"
```

---

## Self-Review 覆盖核对

- **spec §3.1 列一次性迁移（version + migrateColumnPrefs + 回写 + 尊重隐藏）** → Task 1 ✅
- **spec §3.2 worker panic 兜底（run_semantic_worker + catch_unwind + spawn 改调）** → Task 2 ✅
- **spec §6 测试（前端 tsc + 纯函数自查；后端 PanicEmbedder 单测 + 正常路径；回归门；evals byte-equal；手测登记）** → Task 1 Step 4-5 + Task 2 + Task 3 ✅
- **spec §5 错误处理（load 解析异常退默认、save 失败幂等、panic 降级）** → Task 1 Step 2（保留 catch）+ Task 2 实现 ✅
- **类型一致性**：`COLUMN_PREFS_VERSION`、`ColumnPrefs.version`、`migrateColumnPrefs(parsed)->{prefs,migrated}`、`run_semantic_worker(status,db_path,prewarmed,embedder,roots)`、`PanicEmbedder` 全计划内一致 ✅
- **降级摘要文案一致**：`run_semantic_worker` 写 `"语义索引意外中断，已降级 FTS-only"` 与测试断言一致 ✅
