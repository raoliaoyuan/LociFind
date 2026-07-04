# BETA-07 后台索引调度 — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B2 BETA-07；承接 BETA-01A/02/04（索引 + reindex）+ 真机验证（reindex 目前仅手动）
> ID：BETA-07

## 1. 背景与目标

真机验证暴露：reindex **只能手动点「立即索引」**——首次启动索引为空，搜索本地源无结果直到用户手动触发。
且 `index_paths`（音乐全盘发现）**不回收已删文件**（BETA-01A known limitation）。

BETA-07：**启动后台自动索引（非阻塞）+ stale 回收 + 索引状态可见**，让本地索引「开箱即有、自动保鲜」。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | 触发时机 | **启动时后台自动索引**（非阻塞，UI 立即可用）+ 保留手动「立即索引」；定时 / 文件监听留后续 |
| ② | 低优先级 | **best-effort 后台线程**（不阻 UI + rayon 已并行），跨平台安全无 unsafe；OS 线程降优先级留后续 |
| ③（spec 定） | stale 回收 | `MusicIndex::prune_deleted`（扫记录、磁盘不存在则删）——比依赖发现集更稳（OneDrive 占位符路径存在不误删） |
| ④（spec 定） | 状态 | `IndexStatus`（indexing / last_indexed / summary）+ `get_index_status` 命令 + 设置页显示；并发守卫防重索引 |

## 3. 架构

### 3.1 stale 回收（`packages/indexer`）

```rust
impl MusicIndex {
    /// 回收：删除磁盘上已不存在的记录（含 FTS）。返回删除数。
    /// 用 path 存在性判定（非发现集）——OneDrive 占位符路径存在不误删；发现遗漏也不误删。
    pub fn prune_deleted(&self) -> Result<u64, IndexError>;
}
```

实现：`SELECT path FROM music` → 对每个 `Path::new(p).exists()` 为 false 的 `delete_by_path` → 计数。
文档：`DocumentIndex::index_dirs` 经 `run_incremental_index` 已自带回收（paths_under + delete），无需额外 prune。

`LocalIndexBackend::reindex_with`：音乐走发现 `index_paths` 后调 `music.prune_deleted()`（回退目录扫描走
`index_dirs` 已回收，不重复 prune）。

### 3.2 索引状态 + 并发守卫（desktop）

```rust
#[derive(Clone, Serialize, Default)]
pub struct IndexStatus {
    pub indexing: bool,
    pub last_indexed: Option<String>,   // rfc3339
    pub last_summary: Option<String>,   // "音乐 947 / 文档 320"
}
```

`SearchDeps` 加 `index_status: Arc<Mutex<IndexStatus>>`。统一编排 helper：

```rust
/// 执行一次 reindex 并更新状态。并发守卫：已在索引中则跳过返回 None。
/// 手动命令 + 后台启动共用，保证不并发重索引。
fn perform_reindex(status: &Arc<Mutex<IndexStatus>>, db: PathBuf) -> Option<ReindexStats>;
```

- 进入：`status.indexing` 已 true → 返 None（跳过）；否则置 true。
- 跑 `LocalIndexBackend::reindex(default_music_roots, default_document_roots)`（含 prune）。
- 退出：置 `indexing=false`、`last_indexed=now`、`last_summary`。

### 3.3 后台启动自动索引（desktop `main.rs`）

`setup()` 内 spawn 后台任务（`tauri::async_runtime::spawn` + `spawn_blocking` 跑阻塞索引），
不阻塞 UI 启动：

```rust
let status = deps.index_status_arc();   // clone Arc
tauri::async_runtime::spawn(async move {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        perform_reindex(&status, db_path);   // 后台跑，更新状态
    }).await;
});
```

incremental（mtime skip）→ 后续启动多数文件跳过，秒级；首次较久但后台不阻 UI。

### 3.4 命令 + 设置页

- `reindex` 命令改用 `perform_reindex`（更新状态 + 并发守卫；已在索引则提示"正在索引"）。
- 新 `get_index_status` 命令 → `IndexStatus`。
- 设置页「本地索引」节显示：`正在索引…` / `上次索引: <time>（音乐 N / 文档 M）`；轮询或刷新 `get_index_status`。

## 4. 验收 / 验证门

1. **prune_deleted 单测**（in-memory + temp 真文件）：插入存在 + 不存在的 path → prune 只删不存在的、FTS 同步、返回删除数；占位符路径存在不删（用真文件模拟）。
2. **reindex_with 回收**：发现路径含已删文件 → reindex 后该记录被 prune（mock discovery + temp）。
3. **perform_reindex 单测**（desktop，impl 级）：并发守卫（indexing=true 时返 None 跳过）；成功后 status.indexing=false + last_indexed/summary 填充。注入 InMemory/temp。
4. **get_index_status / reindex 命令**：impl 级单测。
5. **零回归**：既有 indexer / local-index / desktop / 全 workspace test（除 platform-macos 预存）；fmt + clippy `-D warnings`。无新外部依赖。
6. **真机手测**（用户）：启动 app → 不点任何按钮，稍等后搜本地音乐/文档**已有结果**（后台自动索引）；设置页见「上次索引」时间；删一个已索引文件再启动 → 该文件从结果消失（回收）。
7. **文档**：indexer README prune 一句 + ROADMAP done + STATUS + manual-test BETA-07。

## 5. 非目标（YAGNI）

- 不做定时 / 文件系统监听（startup-only；留后续）。
- 不做 OS 线程降优先级（best-effort 后台线程；平台 API/unsafe 留后续）。
- 不做索引进度条 / 取消（v1 仅 indexing bool + 完成态）。
- 不做后台调度的可配置开关（默认启用；后续设置项）。
- 文档不加 prune（index_dirs 已回收）。
- 不抑制 catch_unwind 的 panic 日志（cosmetic，release 无 console）。

## 6. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 启动后台索引拖慢启动 | spawn 独立任务，不阻 UI 线程；incremental mtime skip 后续秒级 |
| 后台 + 手动并发重索引 | `IndexStatus.indexing` 守卫，已在索引则跳过 |
| prune 误删存在的文件 | 用 `Path::exists()` 判定（非发现集）；占位符路径存在不删 |
| 首次启动重 IO | best-effort 后台 + rayon 并行（BETA-01A）；可接受 |
| reindex panic（已修） | BETA-01A 真机修复的 catch_unwind 兜住，prune/索引不崩 |
