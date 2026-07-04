# BETA-27 可配置本地索引目录 + 排除规则 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户在设置页选择/增删索引目录（统一列表，三臂共用）+ 排除目录名通配符；reindex/语义索引读配置；无配置时与今天逐字节一致。

**Architecture:** indexer 加 `globset` 在 `WalkDir::filter_entry` 短路排除（**附加 `_excluding` 方法，旧 `index_dirs` 委托空排除集**，零现有调用方改动）；`AppSettings` 加 `index_roots`/`exclude_globs` + 解析助手；`perform_reindex`/`spawn_semantic_index` 读配置走 `reindex_scoped`/`index_dirs_excluding`；前端加 `tauri-plugin-dialog` + 目录/排除编辑器；隐私面板显真实根。

**Tech Stack:** Rust（globset / walkdir）、Tauri（dialog plugin）、React/TS。

参考 spec：[docs/superpowers/specs/2026-06-18-beta-27-configurable-index-dirs-design.md](../specs/2026-06-18-beta-27-configurable-index-dirs-design.md)

---

## File Structure

| 文件 | 改动 |
|---|---|
| `packages/indexer/Cargo.toml` | 加 `globset` dep |
| `packages/indexer/src/scan.rs` | `build_exclude_set` + `run_incremental_index` 加 `exclude` 参 + `filter_entry`；`index_dirs`(music) 委托 + 新 `index_dirs_excluding`；测试 |
| `packages/indexer/src/doc_db.rs` 或 scan.rs | `DocumentIndex::index_dirs`/`index_image_dirs` 委托 + 新 `*_excluding` 变体 |
| `packages/indexer/src/lib.rs` | re-export `build_exclude_set` / `GlobSet`（供 desktop 用） |
| `packages/search-backends/local-index/src/lib.rs` | `reindex_with` 加 `exclude` 参；新 `reindex_scoped(roots, exclude)`；旧 `reindex` 委托空集 |
| `apps/desktop/src-tauri/src/settings.rs` | `index_roots`/`exclude_globs` 字段 + `DEFAULT_EXCLUDE_GLOBS` + `resolve_index_roots`/`resolve_exclude_globs` + 测试 |
| `apps/desktop/src-tauri/src/search/index_status.rs` | `perform_reindex`/`spawn_semantic_index` 读配置走 scoped/excluding |
| `apps/desktop/src-tauri/src/main.rs` | 注册 dialog 插件 |
| `apps/desktop/src-tauri/src/privacy.rs` | 隐私面板源改 `resolve_index_roots` |
| `apps/desktop/src-tauri/Cargo.toml` + `apps/desktop/package.json` | dialog 插件依赖 |
| `apps/desktop/src/pages/SettingsPage.tsx` | 目录选择器 + 排除列表编辑 |
| `docs/manual-test-scenarios.md` | BETA-27 节 |

crate 名：indexer=`locifind-indexer`、local-index=`locifind-local-index-backend`、desktop=`locifind-desktop`。

---

## Task 1: indexer 排除 glob（filter_entry 短路）

**Files:**
- Modify: `packages/indexer/Cargo.toml`；`packages/indexer/src/scan.rs`（`run_incremental_index` :43-115；music `index_dirs` :135）；`packages/indexer/src/lib.rs`（re-export）

- [ ] **Step 1: 加 globset dep**

`packages/indexer/Cargo.toml` 的 `[dependencies]` 加（版本对齐 workspace 既有风格；若 workspace 用 `*` 或具体版本，照其风格）：

```toml
globset = "0.4"
```

- [ ] **Step 2: 写失败测试（scan.rs 测试模块）**

```rust
    #[test]
    fn build_exclude_set_matches_basenames() {
        let set = build_exclude_set(&["node_modules".to_string(), "*cache*".to_string()]);
        assert!(set.is_match("node_modules"));
        assert!(set.is_match("mycache"));
        assert!(!set.is_match("src"));
    }

    #[test]
    fn build_exclude_set_empty_never_matches() {
        let set = build_exclude_set(&[]);
        assert!(!set.is_match("node_modules"));
        assert!(!set.is_match("anything"));
    }

    #[test]
    fn build_exclude_set_skips_invalid_glob() {
        // 非法 glob（未闭合 `[`）跳过、不 panic；合法的仍生效。
        let set = build_exclude_set(&["[".to_string(), "node_modules".to_string()]);
        assert!(set.is_match("node_modules"));
    }

    #[test]
    fn index_dirs_excluding_prunes_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        let nm = docs.join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(docs.join("keep.txt"), "hello").unwrap();
        std::fs::write(nm.join("junk.txt"), "junk").unwrap();
        let db = dir.path().join("idx.db");
        let idx = DocumentIndex::open(&db).unwrap();

        let exclude = build_exclude_set(&["node_modules".to_string()]);
        idx.index_dirs_excluding(&[docs.clone()], &exclude).unwrap();

        // keep.txt 入库、node_modules/junk.txt 不入库。
        assert_eq!(idx.count().unwrap(), 1, "仅 keep.txt 入库，node_modules 被剪枝");
    }

    #[test]
    fn index_dirs_empty_exclude_equals_old_behavior() {
        // 旧 index_dirs（无排除）应索引全部（含 node_modules 内）。
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("docs");
        let nm = docs.join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(docs.join("keep.txt"), "hello").unwrap();
        std::fs::write(nm.join("junk.txt"), "junk").unwrap();
        let db = dir.path().join("idx.db");
        let idx = DocumentIndex::open(&db).unwrap();
        idx.index_dirs(&[docs.clone()]).unwrap();
        assert_eq!(idx.count().unwrap(), 2, "无排除时全索引（含 node_modules）");
    }
```

Run: `cargo test -p locifind-indexer build_exclude_set` → FAIL（未定义）。

- [ ] **Step 3: 实现 `build_exclude_set` + `run_incremental_index` 加 exclude + 委托/excluding 变体**

scan.rs 顶部 `use globset::{Glob, GlobSet, GlobSetBuilder};`。加：

```rust
/// 把目录名 glob 编译成 basename 匹配的 GlobSet。非法 glob 跳过 + 记日志，不中断。
/// 空输入 → 空 GlobSet（`is_match` 恒 false → 无排除）。
#[must_use]
pub fn build_exclude_set(globs: &[String]) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for g in globs {
        match Glob::new(g) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(e) => eprintln!("排除 glob 非法，已跳过 `{g}`: {e}"),
        }
    }
    builder.build().unwrap_or_else(|_| GlobSet::empty())
}

/// 目录名命中排除集（basename 匹配）→ 剪掉整棵子树。
fn is_excluded_dir(entry: &walkdir::DirEntry, exclude: &GlobSet) -> bool {
    entry.file_type().is_dir() && exclude.is_match(entry.file_name())
}
```

`run_incremental_index` 加 `exclude: &GlobSet` 参（在 `exts` 后、`extract` 前），并把 WalkDir 链改为：

```rust
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_excluded_dir(e, exclude))
            .filter_map(Result::ok)
        {
```

music `index_dirs`（:135）改为委托 + 新 excluding：

```rust
    pub fn index_dirs(&self, roots: &[PathBuf]) -> Result<IndexStats, IndexError> {
        self.index_dirs_excluding(roots, &GlobSet::empty())
    }

    /// 增量索引 + 目录名排除（剪掉匹配子树）。
    pub fn index_dirs_excluding(
        &self,
        roots: &[PathBuf],
        exclude: &GlobSet,
    ) -> Result<IndexStats, IndexError> {
        run_incremental_index(self, roots, MUSIC_EXTS, exclude, crate::extract::extract_metadata)
    }
```

`packages/indexer/src/lib.rs` re-export 供 desktop：`pub use scan::build_exclude_set;` + `pub use globset::GlobSet;`（或 `pub use globset;`）。

- [ ] **Step 4: DocumentIndex 的 index_dirs / index_image_dirs 委托 + excluding 变体**

`packages/indexer/src/scan.rs`（DocumentIndex impl :283-310 附近）。`DocumentIndex::index_dirs` 改委托 + 新 `index_dirs_excluding`；`index_image_dirs` 同理：

```rust
    pub fn index_dirs(&self, roots: &[PathBuf]) -> Result<IndexStats, IndexError> {
        self.index_dirs_excluding(roots, &GlobSet::empty())
    }

    pub fn index_dirs_excluding(
        &self,
        roots: &[PathBuf],
        exclude: &GlobSet,
    ) -> Result<IndexStats, IndexError> {
        run_incremental_index(self, roots, DOC_EXTS, exclude, crate::doc_extract::extract_document)
    }

    pub fn index_image_dirs(
        &self,
        roots: &[PathBuf],
        ocr: &dyn crate::ocr::OcrEngine,
    ) -> Result<IndexStats, IndexError> {
        self.index_image_dirs_excluding(roots, ocr, &GlobSet::empty())
    }

    pub fn index_image_dirs_excluding(
        &self,
        roots: &[PathBuf],
        ocr: &dyn crate::ocr::OcrEngine,
        exclude: &GlobSet,
    ) -> Result<IndexStats, IndexError> {
        run_incremental_index(self, roots, IMAGE_EXTS, exclude, |path, mtime| {
            Ok((image_entry(path, mtime), ocr.recognize(path)?))
        })
    }
```

（注：照搬现有 `index_image_dirs` 的闭包体——若现有写法不同，保持其逻辑，仅加 exclude 透传。）

- [ ] **Step 5: 跑测试 + 确认现有不退**

Run: `cargo test -p locifind-indexer`
Expected: PASS（5 新测试 + 现有 index_dirs/image 测试不退——它们走委托的空排除，行为不变）

- [ ] **Step 6: fmt + clippy + commit**

```bash
cargo fmt -p locifind-indexer
cargo clippy -p locifind-indexer --all-targets -- -D warnings
git add packages/indexer/
git commit -m "BETA-27：indexer 加 globset 目录排除（filter_entry 短路 + excluding 变体）"
```

---

## Task 2: local-index reindex_scoped

**Files:**
- Modify: `packages/search-backends/local-index/src/lib.rs`（`reindex`/`reindex_with` :58-112）

- [ ] **Step 1: 写失败测试**

在 local-index `mod tests` 加（验统一 roots + 排除）：

```rust
    #[test]
    fn reindex_scoped_indexes_unified_roots_with_exclude() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data");
        let nm = root.join("node_modules");
        std::fs::create_dir_all(&nm).unwrap();
        std::fs::write(root.join("doc.txt"), "hello world").unwrap();
        std::fs::write(nm.join("junk.txt"), "junk").unwrap();
        let db = dir.path().join("idx.db");
        let backend = LocalIndexBackend::new(&db);

        let exclude = locifind_indexer::build_exclude_set(&["node_modules".to_string()]);
        backend.reindex_scoped(&[root.clone()], &exclude).unwrap();

        // 文档臂索引到 doc.txt（统一 root），node_modules/junk.txt 被排除。
        let docs = locifind_indexer::DocumentIndex::open(&db).unwrap();
        assert_eq!(docs.count().unwrap(), 1, "统一 root 索引 doc.txt、排除 node_modules");
    }
```

Run: `cargo test -p locifind-local-index-backend reindex_scoped` → FAIL（未定义）。

- [ ] **Step 2: 实现 `reindex_scoped` + `reindex_with` 加 exclude**

`reindex_with` 加 `exclude: &GlobSet` 参（末位），内部三臂改调 `_excluding` 变体：

```rust
    pub(crate) fn reindex_with(
        &self,
        discovery: Option<&dyn locifind_indexer::AudioDiscovery>,
        ocr: Option<&dyn OcrEngine>,
        music_roots: &[PathBuf],
        doc_roots: &[PathBuf],
        image_roots: &[PathBuf],
        exclude: &locifind_indexer::GlobSet,
    ) -> Result<(IndexStats, IndexStats, IndexStats), SearchError> {
        // ...（父目录创建、discovery 分支不变）...
        // 三处目录扫描改 _excluding：
        //   music fallback: music.index_dirs_excluding(music_roots, exclude)
        //   docs:           docs.index_dirs_excluding(doc_roots, exclude)
        //   image:          docs.index_image_dirs_excluding(image_roots, engine, exclude)
    }
```

旧 `reindex(music, doc, image)` 改为委托空排除集（保持现有 12 调用方不变）：

```rust
    pub fn reindex(
        &self,
        music_roots: &[PathBuf],
        doc_roots: &[PathBuf],
        image_roots: &[PathBuf],
    ) -> Result<(IndexStats, IndexStats, IndexStats), SearchError> {
        self.reindex_with(
            locifind_indexer::default_audio_discovery().as_deref(),
            locifind_indexer::default_ocr_engine().as_deref(),
            music_roots, doc_roots, image_roots,
            &locifind_indexer::GlobSet::empty(),
        )
    }
```

新增统一入口：

```rust
    /// BETA-27：统一 roots（三臂共用）+ 排除。生产 reindex 路径。
    pub fn reindex_scoped(
        &self,
        roots: &[PathBuf],
        exclude: &locifind_indexer::GlobSet,
    ) -> Result<(IndexStats, IndexStats, IndexStats), SearchError> {
        self.reindex_with(
            locifind_indexer::default_audio_discovery().as_deref(),
            locifind_indexer::default_ocr_engine().as_deref(),
            roots, roots, roots,
            exclude,
        )
    }
```

确认 `reindex_with` 的其它调用方（grep `reindex_with(`，应只有 reindex 测试若干）补 `, &GlobSet::empty()`。

- [ ] **Step 3: 跑测试 + 现有不退**

Run: `cargo test -p locifind-local-index-backend`
Expected: PASS（新测试 + 现有 reindex 测试不退）

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt -p locifind-local-index-backend
cargo clippy -p locifind-local-index-backend --all-targets -- -D warnings
git add packages/search-backends/local-index/
git commit -m "BETA-27：local-index reindex_scoped（统一 roots + 排除）"
```

---

## Task 3: settings 字段 + 解析助手

**Files:**
- Modify: `apps/desktop/src-tauri/src/settings.rs`

- [ ] **Step 1: 写失败测试**

```rust
    #[test]
    fn resolve_index_roots_empty_uses_system_dirs() {
        let roots = resolve_index_roots(&[]);
        // 空配置 → 系统三夹并集（数量取决于系统，至少不 panic；断言「非配置驱动」）。
        // 用一个非空配置验「配置优先」更稳：
        let custom = resolve_index_roots(&["/tmp/x".to_string(), "/tmp/y".to_string()]);
        assert_eq!(custom, vec![PathBuf::from("/tmp/x"), PathBuf::from("/tmp/y")]);
        let _ = roots; // 空路径分支只验不 panic
    }

    #[test]
    fn resolve_exclude_globs_empty_uses_defaults() {
        let d = resolve_exclude_globs(&[]);
        assert!(d.iter().any(|g| g == "node_modules"), "空 → 默认表含 node_modules");
        let custom = resolve_exclude_globs(&["foo".to_string()]);
        assert_eq!(custom, vec!["foo".to_string()]);
    }

    #[test]
    fn old_settings_without_index_fields_parses_ok() {
        let json = r#"{"global_shortcut":"Ctrl+Space","search_scope":["~"],"enable_model_fallback":true,"enable_tracing":false}"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert!(s.index_roots.is_empty());
        assert!(s.exclude_globs.is_empty());
    }
```

Run: `cargo test -p locifind-desktop resolve_index_roots` → FAIL（未定义）。

- [ ] **Step 2: 实现字段 + 默认表 + 解析助手**

`AppSettings` 在 `semantic_similarity_floor` 后加：

```rust
    /// BETA-27：索引的具体文件夹列表（统一，三臂共用）。空 = 系统默认（Music+Documents+Pictures）。
    pub index_roots: Vec<String>,
    /// BETA-27：排除的目录名 glob（basename，树中任何同名子目录被剪枝）。空 = 默认噪声表。
    pub exclude_globs: Vec<String>,
```

`Default` impl 加 `index_roots: Vec::new(), exclude_globs: Vec::new(),`。

结构下方加（`use std::path::PathBuf;` 顶部确认/补）：

```rust
/// BETA-27 默认目录名排除表（BETA-26 build_corpus 验证）。
pub(crate) const DEFAULT_EXCLUDE_GLOBS: &[&str] = &[
    "node_modules", ".git", "target", ".cargo", ".rustup", ".venv", "venv",
    "__pycache__", "dist", "build", ".next", "Pods", ".gradle", ".Trash",
    "vendor", ".cache", "DerivedData", "Library",
];

/// 解析索引根：配置非空用配置，空回退系统 Music+Documents+Pictures 三夹并集。
pub(crate) fn resolve_index_roots(raw: &[String]) -> Vec<std::path::PathBuf> {
    if raw.is_empty() {
        [dirs::audio_dir(), dirs::document_dir(), dirs::picture_dir()]
            .into_iter()
            .flatten()
            .collect()
    } else {
        raw.iter().map(std::path::PathBuf::from).collect()
    }
}

/// 解析排除 glob：配置非空用配置，空回退默认噪声表。
pub(crate) fn resolve_exclude_globs(raw: &[String]) -> Vec<String> {
    if raw.is_empty() {
        DEFAULT_EXCLUDE_GLOBS.iter().map(|s| (*s).to_owned()).collect()
    } else {
        raw.to_vec()
    }
}
```

确认 `dirs` crate 在 desktop 已是依赖（`dirs::data_dir()` 已用，是）。

- [ ] **Step 3: 跑测试 + commit**

Run: `cargo test -p locifind-desktop resolve_index_roots && cargo test -p locifind-desktop resolve_exclude_globs && cargo test -p locifind-desktop old_settings_without_index_fields`
Expected: PASS

```bash
cargo fmt -p locifind-desktop
git add apps/desktop/src-tauri/src/settings.rs
git commit -m "BETA-27：AppSettings 加 index_roots/exclude_globs + 解析助手"
```

---

## Task 4: desktop reindex 接线 + 隐私面板

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/index_status.rs`（`perform_reindex` :77-105、`spawn_semantic_index` :145-158）；`apps/desktop/src-tauri/src/privacy.rs`

- [ ] **Step 1: perform_reindex 读配置走 reindex_scoped**

`perform_reindex` 需 settings_path。**沿用 BETA-15B-3 的 live-read 模式**：在 index_status.rs 加 helper 读 settings.json 得 roots + exclude_globs（mirror `read_similarity_floor`），或给 perform_reindex 传 settings_path。最小改动：加

```rust
/// 从 settings.json 读索引配置（roots + 排除 GlobSet）。读/解析失败 → 默认。
pub(crate) fn read_index_config(
    settings_path: &Option<std::path::PathBuf>,
) -> (Vec<std::path::PathBuf>, locifind_indexer::GlobSet) {
    let settings = settings_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<crate::settings::AppSettings>(&s).ok())
        .unwrap_or_default();
    let roots = crate::settings::resolve_index_roots(&settings.index_roots);
    let globs = crate::settings::resolve_exclude_globs(&settings.exclude_globs);
    (roots, locifind_indexer::build_exclude_set(&globs))
}
```

`perform_reindex` 签名加 `settings_path: Option<PathBuf>`，内部：

```rust
    let (roots, exclude) = read_index_config(&settings_path);
    let result = {
        let backend = locifind_local_index_backend::LocalIndexBackend::new(&db_path);
        backend.reindex_scoped(&roots, &exclude)
    };
```

（替换原 `backend.reindex(&default_music_roots(), &default_document_roots(), &default_image_roots())`。）

- [ ] **Step 2: spawn_semantic_index 用配置 roots + exclude**

`spawn_semantic_index` 的 `spawn_blocking` 闭包里，把 `let roots = locifind_indexer::default_document_roots();` 改为读配置：

```rust
        let (roots, exclude) = read_index_config(&settings_path);
```

并把 `embed_pending` 那条路径改用 `index_dirs_excluding`（在 `semantic_index_pass` 里——`semantic_index_pass` 当前调 `embed_pending(roots, ...)`，FTS 部分已由 perform_reindex 的 reindex_scoped 处理；语义 worker 只补向量，`embed_pending` 本就不建 FTS、只遍历 `paths_under` DB 内文档，**不走 walkdir**，故排除已在 FTS 索引阶段生效、语义 worker 无需再排除）。`spawn_semantic_index` 加 `settings_path` 参，roots 用 `read_index_config` 的 roots。

> 注：`embed_pending` 遍历的是**已在 documents 表的文档**（`paths_under`），不重新 walkdir，所以 node_modules 内文件本就不会进 documents 表（Step 1 的 reindex_scoped 已排除）→ 语义 worker 自然不嵌它们。`spawn_semantic_index` 只需把 roots 从写死改成配置 roots（让 `paths_under(roots)` 范围对齐）。

- [ ] **Step 3: 接线 main.rs 调用点传 settings_path**

`main.rs` 的 `perform_reindex` 两处调用（reindex 命令 + 启动后台任务）、`spawn_semantic_index` 调用，传 `settings::settings_file_path(...)`（生产）/ 测试传 `None`。grep `perform_reindex(` / `spawn_semantic_index(` 全部更新。

- [ ] **Step 4: 隐私面板源改 index_roots**

`privacy.rs`：`privacy_overview_impl` 注入的 `search_scope` 展示改为 `resolve_index_roots(&settings.index_roots)` 的字符串化（字段名可保留 `search_scope` 或改 `index_roots`——前端对应同步；最小改动保留字段名、值换成真实根）。更新对应测试断言。

- [ ] **Step 5: 跑 desktop 测试 + commit**

Run: `cargo test -p locifind-desktop`
Expected: PASS（编译通过 + 既有测试更新后全绿）

```bash
cargo fmt -p locifind-desktop
cargo clippy -p locifind-desktop --all-targets -- -D warnings
git add apps/desktop/src-tauri/src/search/index_status.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/privacy.rs
git commit -m "BETA-27：reindex/语义索引读配置 roots + 排除；隐私面板显真实根"
```

---

## Task 5: 前端目录选择器 + 排除编辑 + dialog 插件

**Files:**
- Modify: `apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/src/main.rs`、`apps/desktop/src-tauri/capabilities/*.json`（若有权限文件）、`apps/desktop/src/pages/SettingsPage.tsx`

- [ ] **Step 1: 加 dialog 插件依赖 + 注册**

- `apps/desktop/package.json` deps 加 `"@tauri-apps/plugin-dialog": "^2"`，跑 `cd apps/desktop && npm install`。
- `apps/desktop/src-tauri/Cargo.toml` deps 加 `tauri-plugin-dialog = "2"`。
- `main.rs` 的 `tauri::Builder` 链加 `.plugin(tauri_plugin_dialog::init())`（与现有 `.plugin(tauri_plugin_global_shortcut::...)` 并列）。
- 若有 `apps/desktop/src-tauri/capabilities/default.json`（Tauri 2 权限），加 `"dialog:allow-open"` 权限项（查该文件结构对齐；无则 Tauri 2 默认可能需建）。

- [ ] **Step 2: 设置页接口 + 目录/排除编辑器**

`SettingsPage.tsx` 的 `AppSettings` 接口加：

```tsx
  index_roots: string[];
  exclude_globs: string[];
```

在设置表单（`settings` 非 null 区）加两块。索引目录：

```tsx
        <div style={{ marginBottom: '16px' }}>
          <div style={{ fontSize: '14px', marginBottom: '6px' }}>索引目录（留空 = 系统音乐/文档/图片）</div>
          {settings.index_roots.map((d, i) => (
            <div key={i} style={{ display: 'flex', gap: '8px', marginBottom: '4px' }}>
              <span style={{ flex: 1, fontSize: '13px', color: '#444' }}>{d}</span>
              <button onClick={() => setSettings({ ...settings, index_roots: settings.index_roots.filter((_, j) => j !== i) })}>移除</button>
            </div>
          ))}
          <button onClick={async () => {
            const { open } = await import('@tauri-apps/plugin-dialog');
            const picked = await open({ directory: true, multiple: false });
            if (typeof picked === 'string') {
              setSettings({ ...settings, index_roots: [...settings.index_roots, picked] });
            }
          }}>+ 添加目录</button>
        </div>
```

排除规则（文本输入 + 列表）：

```tsx
        <div style={{ marginBottom: '16px' }}>
          <div style={{ fontSize: '14px', marginBottom: '6px' }}>排除目录名（通配符，留空 = 默认排除 node_modules/.git 等）</div>
          {settings.exclude_globs.map((g, i) => (
            <div key={i} style={{ display: 'flex', gap: '8px', marginBottom: '4px' }}>
              <span style={{ flex: 1, fontSize: '13px', color: '#444' }}>{g}</span>
              <button onClick={() => setSettings({ ...settings, exclude_globs: settings.exclude_globs.filter((_, j) => j !== i) })}>移除</button>
            </div>
          ))}
          <ExcludeAdder onAdd={(g) => setSettings({ ...settings, exclude_globs: [...settings.exclude_globs, g] })} />
        </div>
```

`ExcludeAdder` 小组件（文件内定义，受控输入 + 添加按钮，回车/点击加非空 trim 值）：

```tsx
function ExcludeAdder({ onAdd }: { onAdd: (g: string) => void }) {
  const [v, setV] = useState('');
  const add = () => { const t = v.trim(); if (t) { onAdd(t); setV(''); } };
  return (
    <div style={{ display: 'flex', gap: '8px' }}>
      <input value={v} onChange={e => setV(e.target.value)} onKeyDown={e => e.key === 'Enter' && add()}
        placeholder="如 node_modules 或 *cache*" style={{ flex: 1 }} />
      <button onClick={add}>添加</button>
    </div>
  );
}
```

保存沿用现有 `update_settings`（整 `settings` 回传）。

- [ ] **Step 3: 类型门 + 前端构建**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: PASS（`@tauri-apps/plugin-dialog` 类型解析、动态 import 类型正确）

- [ ] **Step 4: commit**

```bash
git add apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/capabilities apps/desktop/src/pages/SettingsPage.tsx
git commit -m "BETA-27：设置页目录选择器 + 排除规则编辑 + dialog 插件"
```

---

## Task 6: 回归门 + 手测登记

**Files:**
- Modify: `docs/manual-test-scenarios.md`

- [ ] **Step 1: 全量回归门**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop && npx tsc --noEmit && cd ../..
```
Expected: 全绿；clippy 仅一条无害 non-root profile 提示。

- [ ] **Step 2: evals byte-equal**

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9
```
Expected: v0.5=473、v0.9=726（不碰 parser）。

- [ ] **Step 3: 登记手测**

`docs/manual-test-scenarios.md` 加「BETA-27」节：

```markdown
## BETA-27 可配置索引目录 + 排除规则

1. **加目录**：设置页 → 索引目录「+ 添加目录」选一个含文档的文件夹（如桌面 / D:\工作）→ 保存 → 立即索引 → 设置页语义索引篇数增加、该目录文档可「按意思找到」。
2. **排除规则**：在含 `node_modules` 的项目夹上加索引目录 → 排除规则含 `node_modules`（默认即有）→ 立即索引 → `node_modules` 内文件不被索引（搜不到）。
3. **空配置 = 默认**：清空索引目录列表 → 立即索引 → 仍索引系统音乐/文档/图片（与之前一致）。
4. **隐私面板**：隐私面板「索引范围」显示真实索引根（你配的目录），而非旧的 `~`。
5. **通配符**：加排除 `*cache*` → 名字含 cache 的子目录被剪掉。
```

- [ ] **Step 4: commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-27：回归门通过 + 登记真机手测场景"
```

---

## Self-Review 覆盖核对

- **spec §3.1 数据模型 + 默认** → Task 3 ✅
- **spec §3.2 索引层排除 glob（filter_entry 短路 + 空集不变）** → Task 1 ✅
- **spec §3.3 接线 reindex_scoped + 语义 worker 读配置** → Task 2 + Task 4 ✅
- **spec §3.4 前端 dialog + 编辑器 + 隐私面板** → Task 5 + Task 4 Step 4 ✅
- **spec §5 错误处理（非法 glob 跳过 / 空配置默认 / 向后兼容 / 不存在目录跳过）** → Task 1 Step 2（invalid glob 测试）+ Task 3 测试 + walkdir best-effort ✅
- **spec §6 测试 + 回归门 + evals byte-equal** → 各 Task + Task 6 ✅
- **类型一致性**：`build_exclude_set(&[String])->GlobSet`、`index_dirs_excluding(roots,&GlobSet)`、`index_image_dirs_excluding(roots,ocr,&GlobSet)`、`reindex_scoped(roots,&GlobSet)`、`reindex_with(...,exclude)`、`resolve_index_roots(&[String])->Vec<PathBuf>`、`resolve_exclude_globs(&[String])->Vec<String>`、`read_index_config(&Option<PathBuf>)->(Vec<PathBuf>,GlobSet)`、`index_roots`/`exclude_globs`(Rust+TS) 全计划内一致 ✅
- **低 ripple**：现有 `index_dirs`/`index_image_dirs`/`reindex` 签名不变（委托空集），现有 ~40 调用方零改动 ✅
- **已知**：语义 worker `embed_pending` 不走 walkdir（遍历 DB 内文档），排除在 FTS 阶段已生效（Task 4 Step 2 注）✅
