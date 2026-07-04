# BETA-15B-3 簇 B 设计：「匹配方式」列一次性迁移 + 语义 worker panic 兜底

> 类型：**生产增量**，两个独立小项，BETA-15B 旗舰语义召回层的健壮性/可见性补强。
> 关系：BETA-15B-3 是个多项集合（融合权重调优 + held-out 评测扩量 + 相似度下限 + 列 UX + 原始 query 入 schema + M1 兜底）。按耦合性拆两簇：**簇 A（数据驱动精度核心：评测扩量→相似度下限/权重调优/原始 query）** 强耦合且语料有隐私约束，留单独一刀；**本 spec 只做簇 B 两个独立小项**——列 UX 迁移（15B-1 真机跟进 (b)）+ worker panic 兜底（15B-2 整体终审 Minor M1）。
> 边界：不碰召回算法、融合权重、相似度阈值、评测集（那些是簇 A）。

## 1. 背景与目标

两个独立来源的小缺口：

1. **「匹配方式」列对老用户默认隐藏**（15B-1 真机手测发现 (b)）：BETA-15B-1 把 match 列设 `defaultVisible: true`，但 `loadColumnPrefs()` 读老用户在 match 列**存在之前**保存的 `visible` 数组（不含 `"match"`）→ 旗舰「按意思找到」徽标列对存量用户看不到，需手动右键调出。`defaultVisible` 只对全新安装生效。
2. **语义 worker panic 会泄漏并发守卫**（15B-2 整体终审 Minor M1）：`spawn_semantic_index` 的 `spawn_blocking` 闭包无 panic 兜底。若 `semantic_index_pass` 内 `embedder.embed()` panic（FFI abort 等），panic 被 `spawn_blocking` 收进 JoinError 后丢弃，`semantic_indexing` 守卫永停 `true` → UI 永显「语义索引中」+ 后续触发被 `semantic_begin` 永久跳过（**不阻断 FTS/查询/主流程**，但 UI 状态卡死至重启）。

**唯一目标**：① 老用户也能看见旗舰语义列（一次性迁移，尊重后续主动隐藏）；② worker panic 时干净降级、守卫不泄漏。

## 2. 范围护栏（YAGNI）

| 本切片做（簇 B） | 不做 / 留簇 A 或后续 |
|---|---|
| match 列一次性 localStorage 迁移 | 列拖拽/宽度/其它列行为改动 |
| worker panic catch_unwind 兜底 + 清守卫 | 重试 / 错误上报 UI / panic 根因排查 |
| — | 相似度下限、融合权重、held-out 评测、原始 query 入 schema（簇 A） |

## 3. 架构与组件

### 3.1 「匹配方式」列一次性迁移（`apps/desktop/src/SearchView.tsx`）

- `ColumnPrefs` 接口加 `version?: number`。
- 新增 `const COLUMN_PREFS_VERSION = 2`（v2 = 引入语义 match 列的 schema）。
- `defaultColumnPrefs()` 返回的 prefs 带 `version: COLUMN_PREFS_VERSION`（新安装已含 match，无需迁移）。
- `loadColumnPrefs()` 加一次性迁移逻辑（迁移判定抽成纯函数 `migrateColumnPrefs(parsed) -> ColumnPrefs` 便于推理/测试）：
  - 解析出的 `version`（缺省视为 `1`）`< 2` **且** `visible` 不含 `"match"` → 追加 `"match"` 到 `visible`（位置无关——渲染按 `ALL_COLUMNS` 顺序，现有注释保证），标 `version = 2`，**持久化回写**（`saveColumnPrefs`）使迁移只发生一次。
  - `version >= 2` → 不注入，尊重用户选择（用户随后手动隐藏 match 不再被强加）。
  - 保留现有 `name` 列强含（`if (!visible.includes("name")) visible.unshift("name")`）与无效 key 过滤。
- **正确性论证**：v1 用户从未见过 match 列（那时它不存在），不可能「主动隐藏过」→ 对所有 v1 用户注入一次正确、不违背任何已表达意图。迁移后他们的隐藏意图才开始被尊重。

### 3.2 语义 worker panic 兜底（`apps/desktop/src-tauri/src/search/index_status.rs`）

- 新增可测外壳 `run_semantic_worker(status, db_path, prewarmed, embedder, roots)`：
  ```rust
  pub(crate) fn run_semantic_worker(
      status: &Arc<Mutex<IndexStatus>>,
      db_path: &std::path::Path,
      prewarmed: bool,
      embedder: &dyn locifind_indexer::embed::TextEmbedder,
      roots: &[std::path::PathBuf],
  ) {
      let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
          semantic_index_pass(status, db_path, prewarmed, embedder, roots);
      }));
      if r.is_err() {
          eprintln!("语义索引 worker panic，已清守卫降级 FTS-only");
          semantic_abort(status, "语义索引意外中断，已降级 FTS-only");
      }
  }
  ```
- `spawn_semantic_index` 内 `spawn_blocking` 闭包改调 `run_semantic_worker` 而非直接 `semantic_index_pass`（暖机 `prewarm()` 仍在外、roots 取 `default_document_roots()` 不变）。
- **正确性**：`semantic_index_pass` 的助手（`semantic_begin/set_progress/done/abort`）都短暂持锁即释放、不跨 `embed_pending` 循环持锁，故 panic 发生在 `embedder.embed()` 时状态锁未被持有 → 不毒化；即便毒化，助手 `unwrap_or_else(|e| e.into_inner())` 容忍。`AssertUnwindSafe` 因闭包捕获 `&Arc<Mutex>` 等非 `UnwindSafe` 类型所需。panic 后无论 `semantic_begin` 是否已置守卫，`semantic_abort` 都把 `semantic_indexing` 清回 `false`（begin 未跑时仅多写一次摘要，无害）。

## 4. 数据流

- **列迁移**：app 启动/首次渲染 → `loadColumnPrefs()` → 老用户一次性注入 match + 回写 → 结果列表渲染含「匹配方式」列 → 语义命中显「按意思找到」徽标。
- **worker 兜底**：触发 reindex → FTS → `spawn_semantic_index` → `spawn_blocking{ prewarm; run_semantic_worker }` → 正常时 `semantic_index_pass` 走 done/abort；panic 时 catch_unwind 捕获 → `semantic_abort` 清守卫降级。

## 5. 错误处理

- `loadColumnPrefs` 解析异常 → 现有 `catch` 退回 `defaultColumnPrefs()`（含 match + version）；`saveColumnPrefs` 失败已 try/catch 忽略，迁移回写失败仅意味下次再迁一次（幂等，无害）。
- worker panic → catch_unwind + `semantic_abort` 干净降级 FTS-only。
- 其余路径（无模型 `is_active()` 早退、单篇 embed 失败计数、open 失败）由 15B-2 既有逻辑处理，不变。

## 6. 测试

- **前端**：仓库**无 JS 测试 runner**（已核实 package.json 无 vitest/jest），不为本切片引入测试框架。迁移逻辑抽成纯函数 `migrateColumnPrefs(parsed) -> ColumnPrefs`（按构造可推理：v1 prefs 无 version、visible 无 match → 注入 match + version=2；v2 prefs 用户已隐藏 match → 不注入；新安装走 `defaultColumnPrefs`），靠 `tsc --noEmit` 类型门 + 登记手测（模拟旧 localStorage）验证。
- **后端**：`run_semantic_worker` 用 `PanicEmbedder`（`embed()` 内 `panic!`）+ 临时 FTS 库（有文档使 `embed_pending` 触达 embedder）→ 断言 panic 后 `semantic_indexing == false`、`semantic_progress == None`、`semantic_summary` 为降级原因。另验正常 stub embedder 经 `run_semantic_worker` 与直接 `semantic_index_pass` 行为一致（就绪摘要）。
- **回归（硬门）**：
  - feature 关 / 无模型：`is_active()` 早退、搜索/索引行为与今天逐字节一致；evals v0.5=473/v0.9=726 不动（不碰 parser）。
  - `cargo test --workspace` 零失败、`cargo clippy -D warnings` 0、`cargo fmt --check` 净、前端 `tsc` 净。
- **真机手测（登记，留用户）** → `docs/manual-test-scenarios.md` 加簇 B 节：
  1. **列迁移**：用旧版（match 列隐藏的 localStorage）→ 升级后启动 → 「匹配方式」列自动出现、语义命中显「按意思找到」→ 手动隐藏该列 → 重启仍隐藏（迁移只一次、尊重意图）。
  2. **worker 兜底**：（难自然触发，主要靠单测）如能注入失败模型则验 UI 不卡「语义索引中」。

## 7. 平台

前端迁移、后端兜底均与平台无关，macOS + Windows 同一份代码。

## 8. 验收标准

1. 老用户（match 隐藏的旧 localStorage）升级后「匹配方式」列自动浮现；迁移后手动隐藏被尊重（不再强加）。
2. 语义 worker panic 时守卫清回、UI 不卡「语义索引中」、降级 FTS-only。
3. feature 关 / 无模型逐字节一致；evals byte-equal；`cargo test --workspace` + clippy + fmt + tsc 全绿。
4. 真机手测场景登记。

## 9. 未尽 / 后续交接

- **簇 A（数据驱动精度核心，单独一刀）**：held-out 评测扩量（先决 BETA-26 真实语料的隐私方案：合成 vs gitignored）→ 语义臂 cosine 相似度下限 + 加权 RRF 权重调优 + FTS 置信度路由 + 原始 query 入 schema（修 keywords 拼接近似）。
- worker panic 的**根因排查**（embed FFI 何时会 panic）不在本切片——本切片只做兜底不泄漏。
