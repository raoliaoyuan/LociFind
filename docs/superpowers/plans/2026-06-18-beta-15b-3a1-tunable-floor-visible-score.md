# BETA-15B-3 簇 A-1 续 实施计划：相似度下限可调 + 分数可见

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让语义结果显示 cosine 分数 + 相似度下限可在设置页调整（live-read settings.json，改后重搜即生效、免重启），使用户一次构建后自助把 0.30 调到合适值。

**Architecture:** 后端 `SemanticIndexBackend::new` 加 `floor_provider: Arc<dyn Fn()->f32+Send+Sync>` 闭包，`search_results` 每次查询调它取下限；desktop 侧闭包 live-read settings.json 的新字段 `semantic_similarity_floor`（clamp+默认 0.30）；前端设置页加数值输入框 + 「匹配方式」列对 semantic 结果显 cosine 分数。不碰 parser → evals byte-equal 安全。

**Tech Stack:** Rust（semantic-index backend + desktop settings）、React/TypeScript（设置控件 + 结果列）。

参考 spec：[docs/superpowers/specs/2026-06-18-beta-15b-3a1-tunable-floor-visible-score-design.md](../specs/2026-06-18-beta-15b-3a1-tunable-floor-visible-score-design.md)

---

## File Structure

| 文件 | 职责 | 改动 |
|---|---|---|
| `packages/search-backends/semantic-index/src/lib.rs` | 语义后端 | `new` 加 `floor_provider`；`search_results` 调它；移除 `SIMILARITY_FLOOR` 常量；改现有测试三参 + 加 provider 测试 |
| `apps/desktop/src-tauri/src/settings.rs` | 设置 | `AppSettings` 加 `semantic_similarity_floor`；`DEFAULT_SIMILARITY_FLOOR` + `resolve_similarity_floor` + `read_similarity_floor` + 测试 |
| `apps/desktop/src-tauri/src/main.rs` | 接线 | `build_registry` 加 `settings_path` 参 + 构造 floor_provider 闭包；setup/测试调用点更新 |
| `apps/desktop/src/pages/SettingsPage.tsx` | 设置 UI | `AppSettings` 接口加字段 + 数值输入框 |
| `apps/desktop/src/SearchView.tsx` | 结果列 | match 列对 semantic 结果显 cosine 分数 |
| `docs/manual-test-scenarios.md` | 手测登记 | 簇 A-1 续节 |

crate 名：semantic-index = `locifind-semantic-index`，desktop = `locifind-desktop`（已核实）。

---

## Task 1: 后端 `floor_provider`（semantic-index）

**Files:**
- Modify: `packages/search-backends/semantic-index/src/lib.rs`（`SemanticIndexBackend` 结构 + `new` :59；`search_results` :77-107；移除 `SIMILARITY_FLOOR` :31；测试 :214+）

- [ ] **Step 1: 改现有测试为三参 + 写 provider 失败测试**

把 `mod tests` 里三处 `SemanticIndexBackend::new(...)` 调用补第三参常量闭包；再加新测试。具体：

`semantic_query_ranks_by_cosine` 与 `semantic_floor_filters_low_relevance` 的 `SemanticIndexBackend::new(&db, Some(Arc::new(AxisEmbedder)))` → 改为 `SemanticIndexBackend::new(&db, Some(Arc::new(AxisEmbedder)), std::sync::Arc::new(|| 0.30_f32))`。

`no_embedder_is_unavailable_and_empty` 的 `SemanticIndexBackend::new(&db, None)` → `SemanticIndexBackend::new(&db, None, std::sync::Arc::new(|| 0.30_f32))`。

新增测试（加到 `mod tests` 末尾）：

```rust
    #[test]
    fn floor_provider_controls_filtering() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cat.txt"), "关于猫的笔记").unwrap();
        std::fs::write(dir.path().join("dog.txt"), "关于狗的笔记").unwrap();
        let db = dir.path().join("index.db");
        let idx = DocumentIndex::open(&db).unwrap();
        idx.index_dirs(&[dir.path().to_path_buf()]).unwrap();
        let cat = dir.path().join("cat.txt").to_string_lossy().into_owned();
        let dog = dir.path().join("dog.txt").to_string_lossy().into_owned();
        // cat[1,0.2]·查询「猫」[1,0]≈0.98；dog[1,1]≈0.71。
        assert!(idx.upsert_vector(&cat, &[1.0, 0.2], "axis", "h1").unwrap());
        assert!(idx.upsert_vector(&dog, &[1.0, 1.0], "axis", "h2").unwrap());

        // 高下限 0.95：只 cat 过（0.98≥0.95，0.71<0.95）。
        let strict = SemanticIndexBackend::new(
            &db,
            Some(Arc::new(AxisEmbedder)),
            std::sync::Arc::new(|| 0.95_f32),
        );
        let r = strict.search_results(&file_search("我家的猫")).unwrap();
        assert_eq!(r.len(), 1, "高下限只留 cat");
        assert_eq!(r[0].name, "cat.txt");

        // 低下限 0.0：两条都过。
        let loose = SemanticIndexBackend::new(
            &db,
            Some(Arc::new(AxisEmbedder)),
            std::sync::Arc::new(|| 0.0_f32),
        );
        assert_eq!(loose.search_results(&file_search("我家的猫")).unwrap().len(), 2);
    }
```

Run: `cargo test -p locifind-semantic-index floor_provider` → FAIL（`new` 仍是两参，编译错误）。

- [ ] **Step 2: 加字段 + 改 `new` + `search_results` + 移除常量**

`use` 区确认有 `use std::sync::Arc;`（已有）。

`SemanticIndexBackend` 结构加字段：

```rust
pub struct SemanticIndexBackend {
    db_path: PathBuf,
    embedder: Option<Arc<dyn TextEmbedder>>,
    /// 相似度下限来源（desktop 侧 live-read settings.json；每次查询调）。
    floor_provider: Arc<dyn Fn() -> f32 + Send + Sync>,
}
```

`new` 加第三参：

```rust
    pub fn new(
        db_path: impl Into<PathBuf>,
        embedder: Option<Arc<dyn TextEmbedder>>,
        floor_provider: Arc<dyn Fn() -> f32 + Send + Sync>,
    ) -> Self {
        Self {
            db_path: db_path.into(),
            embedder,
            floor_provider,
        }
    }
```

`search_results` 里把 `filter_rank_topk(scored, SIMILARITY_FLOOR, TOP_K)` 改为：

```rust
        let floor = (self.floor_provider)();
        let scored = filter_rank_topk(scored, floor, TOP_K);
```

删除 `const SIMILARITY_FLOOR: f32 = 0.30;`（:31，不再被使用——下限统一由 provider 供给；默认值移到 desktop，Task 2）。`filter_rank_topk` 纯函数与其单测不变。

- [ ] **Step 3: 跑测试确认通过**

Run: `cargo test -p locifind-semantic-index`
Expected: PASS（`floor_provider_controls_filtering` + 改三参后的现有测试 + `filter_rank_topk_*` 全绿）

- [ ] **Step 4: fmt + clippy + commit**

```bash
cargo fmt -p locifind-semantic-index
cargo clippy -p locifind-semantic-index --all-targets -- -D warnings
git add packages/search-backends/semantic-index/src/lib.rs
git commit -m "BETA-15B-3 簇A-1续：语义后端 floor_provider 闭包（下限可注入）"
```

> 注：本 task 后 desktop crate 暂不编译（main.rs build_registry 仍两参调 `new`），Task 2 修齐。semantic-index crate 本身独立编译+测试通过。

---

## Task 2: 设置字段 + desktop 接线

**Files:**
- Modify: `apps/desktop/src-tauri/src/settings.rs`（`AppSettings` :9-18；`Default` :20-35；加常量+函数+测试）
- Modify: `apps/desktop/src-tauri/src/main.rs`（`build_registry` :56-95；setup 调用点 :284；测试调用点 :440/500）

- [ ] **Step 1: 写 settings 失败测试**

在 `settings.rs` 的 `mod tests` 加：

```rust
    #[test]
    fn resolve_similarity_floor_clamps_and_defaults() {
        assert_eq!(resolve_similarity_floor(None), DEFAULT_SIMILARITY_FLOOR);
        assert_eq!(resolve_similarity_floor(Some(0.5)), 0.5);
        assert_eq!(resolve_similarity_floor(Some(-1.0)), 0.0);
        assert_eq!(resolve_similarity_floor(Some(2.0)), 1.0);
        assert_eq!(resolve_similarity_floor(Some(f32::NAN)), DEFAULT_SIMILARITY_FLOOR);
    }

    /// 旧 settings.json 无 semantic_similarity_floor 字段 → 解析 ok、字段 None。
    #[test]
    fn old_settings_without_similarity_floor_parses_ok() {
        let json = r#"{"global_shortcut":"Ctrl+Space","search_scope":["~"],"enable_model_fallback":true,"enable_tracing":false}"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert!(s.semantic_similarity_floor.is_none());
    }

    #[test]
    fn read_similarity_floor_reads_or_defaults() {
        // None 路径 → 默认。
        assert_eq!(read_similarity_floor(&None), DEFAULT_SIMILARITY_FLOOR);
        // 写一个含字段的 settings.json → 读到该值。
        let dir = std::env::temp_dir().join(format!("locifind-floor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("settings.json");
        std::fs::write(&f, r#"{"semantic_similarity_floor":0.55}"#).unwrap();
        assert_eq!(read_similarity_floor(&Some(f)), 0.55);
        std::fs::remove_dir_all(&dir).ok();
    }
```

Run: `cargo test -p locifind-desktop resolve_similarity_floor` → FAIL（未定义）。

- [ ] **Step 2: 加字段 + 常量 + 函数**

`AppSettings` 结构加字段（保留现有字段 + `#[serde(default)]` 结构级已在）：

```rust
    /// BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。
    pub embedding_model_path: Option<String>,
    /// BETA-15B-3 簇A-1：语义相似度下限覆盖（None = 默认 DEFAULT_SIMILARITY_FLOOR）。
    pub semantic_similarity_floor: Option<f32>,
```

`Default` impl 加 `semantic_similarity_floor: None,`。

在 `settings.rs`（结构下方、`settings_file_path` 附近）加：

```rust
/// 语义相似度下限默认值（全仓单一默认源）。
pub(crate) const DEFAULT_SIMILARITY_FLOOR: f32 = 0.30;

/// 把设置里的原始下限值规整：有限值 clamp 到 [0,1]；None / 非有限 → 默认。
pub(crate) fn resolve_similarity_floor(raw: Option<f32>) -> f32 {
    match raw {
        Some(v) if v.is_finite() => v.clamp(0.0, 1.0),
        _ => DEFAULT_SIMILARITY_FLOOR,
    }
}

/// 从 settings.json live-read 语义相似度下限（每次查询调）。读/解析失败 → 默认。
pub(crate) fn read_similarity_floor(settings_path: &Option<std::path::PathBuf>) -> f32 {
    let raw = settings_path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
        .and_then(|v| v.semantic_similarity_floor);
    resolve_similarity_floor(raw)
}
```

Run: `cargo test -p locifind-desktop resolve_similarity_floor && cargo test -p locifind-desktop read_similarity_floor && cargo test -p locifind-desktop old_settings_without_similarity_floor` → PASS（settings 单测先绿；main.rs 仍需 Step 3 修齐才能整 crate 编译）。

- [ ] **Step 3: `build_registry` 加 settings_path + floor_provider 闭包**

`main.rs` 的 `build_registry` 签名加参 + 构造闭包传入 `SemanticIndexBackend::new`：

```rust
fn build_registry(
    embedding: Arc<search::embedding_model::EmbeddingModelHandle>,
    settings_path: Option<PathBuf>,
) -> ToolRegistry {
```

把语义后端构造（:80-83）改为：

```rust
        let floor_settings_path = settings_path.clone();
        let semantic = locifind_search_backend_semantic::SemanticIndexBackend::new(
            local_index_db_path(),
            Some(embedding.clone() as Arc<dyn locifind_indexer::embed::TextEmbedder>),
            std::sync::Arc::new(move || settings::read_similarity_floor(&floor_settings_path)),
        );
```

setup 调用点（:284）`build_registry(embedding.clone())` → `build_registry(embedding.clone(), settings::settings_file_path(&app.handle().clone()))`。

两处测试调用点（:440、:500）`build_registry(embedding)` → `build_registry(embedding, None)`。

- [ ] **Step 4: 跑整 desktop crate**

Run: `cargo test -p locifind-desktop`
Expected: PASS（settings 测试 + main.rs 编译通过 + 既有测试全绿）

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt -p locifind-desktop
cargo clippy -p locifind-desktop --all-targets -- -D warnings
git add apps/desktop/src-tauri/src/settings.rs apps/desktop/src-tauri/src/main.rs
git commit -m "BETA-15B-3 簇A-1续：相似度下限进 AppSettings + build_registry live-read 闭包"
```

---

## Task 3: 前端（设置控件 + 结果列分数）

**Files:**
- Modify: `apps/desktop/src/pages/SettingsPage.tsx`（`AppSettings` 接口 :4-11；加输入框）
- Modify: `apps/desktop/src/SearchView.tsx`（match 列 render :244-252）

- [ ] **Step 1: 设置接口 + 输入框**

`SettingsPage.tsx` 的 `AppSettings` 接口（:4-11）加字段：

```tsx
interface AppSettings {
  global_shortcut: string;
  search_scope: string[];
  enable_model_fallback: boolean;
  enable_tracing: boolean;
  // BETA-23：模型文件路径覆盖（null = 默认数据目录 models/）。
  model_path: string | null;
  // BETA-15B-3 簇A-1：语义相似度下限覆盖（null = 默认 0.30，越高越严）。
  semantic_similarity_floor: number | null;
}
```

在设置页表单中（model_path 输入框附近，参照 :203-206 的现有 text input 模式）加数值输入框。`settings` 为 null 时不渲染（沿用现有 guard）。示例块（插到合适的设置项位置）：

```tsx
        <label style={{ display: 'block', marginBottom: '12px' }}>
          <span style={{ fontSize: '14px' }}>语义相似度下限（0–1，越高越严，默认 0.30）</span>
          <input
            type="number"
            min={0}
            max={1}
            step={0.05}
            value={settings.semantic_similarity_floor ?? 0.30}
            onChange={e =>
              setSettings({
                ...settings,
                semantic_similarity_floor:
                  e.target.value === '' ? null : parseFloat(e.target.value),
              })
            }
            style={{ marginLeft: '8px', width: '80px' }}
          />
          <span style={{ fontSize: '12px', color: '#999', marginLeft: '8px' }}>
            语义结果低于此 cosine 分数将被过滤；改后重新搜索即生效。
          </span>
        </label>
```

保存沿用现有「保存设置」按钮的 `update_settings`（:130）—— `settings` 整对象回传，新字段随之持久化。

- [ ] **Step 2: 结果列显 cosine 分数**

`SearchView.tsx` 的 match 列 render（:244-252）改为 semantic 结果在徽标后附分数：

```tsx
    render: (r) =>
      r.match_type === "semantic" ? (
        <span className="badge-semantic" title="按语义/跨语言召回">
          按意思找到
          {typeof r.score === "number" && (
            <span style={{ color: "#999", marginLeft: "6px", fontWeight: 400 }}>
              · {r.score.toFixed(2)}
            </span>
          )}
        </span>
      ) : (
        matchTypeLabel(r.match_type)
      ),
```

前端结果类型为 `SearchResultJson`（`SearchView.tsx:5`），已声明 `score: number | null`（:13）与 `match_type: string`（:12）——**无需改类型**，`r.score` 直接可用。

- [ ] **Step 3: 类型门**

Run: `cd apps/desktop && npx tsc --noEmit`
Expected: PASS（零类型错误）

- [ ] **Step 4: commit**

```bash
git add apps/desktop/src/pages/SettingsPage.tsx apps/desktop/src/SearchView.tsx
git commit -m "BETA-15B-3 簇A-1续：设置页相似度下限输入框 + 结果列显 cosine 分数"
```

---

## Task 4: 回归门 + 手测登记

**Files:**
- Modify: `docs/manual-test-scenarios.md`

- [ ] **Step 1: 全量回归门**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop && npx tsc --noEmit && cd ../..
```
Expected: fmt 净；clippy 仅一条无害 non-root profile 提示；`cargo test --workspace` 全绿；tsc 零错误。

- [ ] **Step 2: evals byte-equal 硬门**

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9
```
Expected: v0.5 pass=473、v0.9 pass=726（不碰 parser，逐字相符）。

- [ ] **Step 3: 登记手测**

在 `docs/manual-test-scenarios.md` 加「BETA-15B-3 簇 A-1 续（可调下限 + 可见分数）」节：

```markdown
## BETA-15B-3 簇 A-1 续（相似度下限可调 + cosine 分数可见）

前提：feature `semantic-recall`（+ metal）+ 放 embedding 模型 + 已 reindex 出向量。

1. **分数可见**：做语义查询 → 结果「匹配方式」列显「按意思找到 · 0.XX」（cosine 2 位小数）。看不相关项 vs 真实命中各多少分。
2. **调下限即时生效**：设置页把「语义相似度下限」从 0.30 调高（如 0.45）→ 保存 → **回搜索框重新搜同一查询（无需重启）** → 低分项消失、只留高分命中。
3. **找到合适值**：据步骤 1 看到的分数分布，把下限调到「不相关项被挡、真实命中保留」的甜点值，记录之（反馈给开发 bake 为新默认）。
4. **越界/默认**：留空或填非法值 → 回落 0.30；填 >1 或 <0 → clamp。未碰设置的用户行为与之前一致。
```

- [ ] **Step 4: commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-15B-3 簇A-1续：回归门通过 + 登记真机手测场景"
```

---

## Self-Review 覆盖核对

- **spec §3.1 分数可见** → Task 3 Step 2 ✅
- **spec §3.2 后端 floor_provider + 移除常量** → Task 1 ✅
- **spec §3.3 AppSettings 字段** → Task 2 Step 2 ✅
- **spec §3.4 DEFAULT_SIMILARITY_FLOOR + resolve_floor + build_registry 闭包 + settings_path 透传** → Task 2 Step 2/3 ✅
- **spec §3.5 前端控件** → Task 3 Step 1 ✅
- **spec §5 错误处理（读失败/越界/NaN→默认+clamp；score null→只显徽标）** → `resolve_similarity_floor` 测试 + Task 3 Step 2 的 `typeof r.score` guard ✅
- **spec §6 测试（provider 控制过滤 / 向后兼容 / clamp / 前端 tsc / 回归门 / evals）** → Task 1/2/3/4 ✅
- **类型一致性**：`floor_provider: Arc<dyn Fn()->f32+Send+Sync>`、`SemanticIndexBackend::new(db, embedder, floor_provider)`、`semantic_similarity_floor: Option<f32>`(Rust)/`number|null`(TS)、`DEFAULT_SIMILARITY_FLOOR`、`resolve_similarity_floor`、`read_similarity_floor`、`build_registry(embedding, settings_path)` 全计划内一致 ✅
- **不破坏现有**：三处 `SemanticIndexBackend::new` 调用点改三参（Task 1 Step 1）；`build_registry` 两处测试调用点 + setup 调用点改两参（Task 2 Step 3）；未碰 parser → evals byte-equal ✅
