# BETA-15B-2 设计：向量索引后台预热 + 解耦调度

> 类型：**生产增量**，在 BETA-15B-1 语义召回纵切之上解决两个实测体验缺口。
> 关系：BETA-15B 旗舰化（"本地语义召回层"）的第二个子项。前置 [15B-1](./2026-06-15-beta-15b-1-semantic-recall-vertical-design.md) 已 done、双平台真机验通。
> 边界：本切片只动「向量何时/如何被嵌」的调度与暖机，**不动**召回算法、融合权重、UI 徽标语义（那些是 15B-3/15B-4）。

## 1. 背景与目标

15B-1 真机暴露两个体验缺口（详 STATUS 2026-06-15 日志「实测 4 发现」①②）：

1. **首查询 16.8s 冷启动**：embedding 句柄 `ready()` 在**调用线程上阻塞 load**（Windows CPU 实测 16.8s）。稳态下（无新文档可嵌）启动后台 reindex 的嵌入 pass 因 `vector_is_current` 全命中而从不调用 `embed()` → 模型永不加载 → **首个用户查询独吞这 16.8s**。
2. **reindex 被内联嵌入拖慢**：嵌入 pass 内联在 `perform_reindex` 里、压在 `indexing` 守卫下，FTS 跑完后同步跑嵌入（Mac 5000 篇 5.2min / Windows CPU 更久）。整段 `IndexStatus.indexing=true`、期间手动 reindex 被挡、而 FTS 结果其实早就能搜了。

**唯一目标**：把模型加载（暖机）从用户查询路径上移走，并把语义嵌入 pass 从 FTS reindex 解耦成独立后台阶段、带可见进度；feature/模型双门控关闭时与今天逐字节一致。

**现状关键事实**（实现前已核实）：
- 触发源只有两个：**启动自动后台 reindex** + 设置页「立即索引」按钮（`reindex` 命令）。**无文件监听器、无定时**（`StatusIndicator` 的 30s `setInterval` 仅轮询状态显示，不触发 reindex）。→ **「防抖」在当前触发模型下是 YAGNI**（启动一次 + 手动点击，且 `indexing` 守卫已防并发），本切片不做。
- 对比 BETA-23 fallback 模型：startup 不预载、首触发时**后台异步 load**（首查询降级不阻塞）。embedding 句柄与之不同——`ready()` 是**阻塞 load**，故需显式后台暖机。

## 2. 范围护栏（YAGNI）

| 本切片做（15B-2） | 不做 / 留后续 |
|---|---|
| 启动后台暖机（消除首查询冷启动） | 模型预热前缀 KV（embedding 无生成前缀，不适用） |
| 语义嵌入 pass 与 FTS reindex 解耦成独立后台阶段 + 独立守卫 | 文件监听器 / 自动增量触发 / 防抖（无 watcher，YAGNI） |
| 轻量语义索引进度 UI（复用 `IndexStatus`） | 花哨进度条 / 取消按钮 / 优先级队列 |
| 单篇失败计数不中断（沿用现哲学） | 批处理写事务优化（已 autocommit 逐篇落库，partial 进度天然存活） |
| 召回 / 融合 / 徽标行为不变（守 15B-1） | 融合权重调优、相似度阈值、列 UX（→ 15B-3）；Windows GPU、媒体臂（→ 15B-4） |

## 3. 架构与组件

### 3.1 总览

```
启动 / 手动「立即索引」
   ↓
perform_reindex(FTS only)        ← 去掉内联嵌入，跑完即释放 indexing 守卫
   ↓ FTS 结果立即可搜
spawn_semantic_index(后台 worker，独立守卫 + 进度)
   ↓
  prewarm(): ready() 阻塞 load    ← 暖机，消除首查询 16.8s 冷启动
   ↓
  embed_pending(progress_cb)      ← 渐进填充 document_vectors
   ↓
  semantic_summary → 就绪
```

**统一语义 worker**（架构决策）：FTS 后启**一个**后台任务，先 `prewarm()`（本后台线程阻塞 load = 暖机）→ 再跑嵌入循环。不拆成「并行暖机 + 嵌入 worker」——因为 `ready()` 单飞机制下暖机占着 `Loading` 时嵌入循环调 `embed()` 会读到 `None`，那一窗口所有嵌入失败；统一成一个顺序任务无此 race。稳态（无新文档）下 worker 也先 load 再发现无事可做 → 模型照样变热。查询侧在 load 窗口内仍读到 `None` → 干净降级 FTS-only（现有正确行为，不变）。

### 3.2 indexer 层（`packages/indexer`）

- 现有 `index_dirs_with_embedder(roots, embedder)`（`doc_db.rs:428`）= FTS 增量 + 内联嵌入循环。
- **拆分**：新增 `embed_pending(roots, embedder, progress_cb)` —— **只跑补嵌循环那段**（`doc_db.rs:435-458` 的逻辑）：先遍历数出待嵌总数 `total`（`vector_is_current` 未命中者），再逐篇嵌入，每篇（或每 batch）回调 `progress_cb(done, total)`。单篇 `embed` 失败计入 `embed_failed`、跳过、不中断；`upsert_vector` DB 写失败仍向上传播中断。返回 `(embedded, failed)`。
- `index_dirs_with_embedder` 保留为薄封装（`index_dirs` + `embed_pending` 无 progress），维持现有单测语义不破坏。

### 3.3 embedding 句柄（`apps/desktop/.../search/embedding_model.rs`）

- 新增 `pub fn prewarm(&self) -> bool`：调 `self.ready().is_some()`，把 16.8s load 付在后台 worker 线程。幂等（已 `Ready` 直接 true；`Loading`/`Failed`/`Unavailable`/`NotFound` 返 false，不重试）。

### 3.4 desktop 调度层（`apps/desktop/.../search/index_status.rs` + `main.rs`）

- `perform_reindex` **删掉内联嵌入块**（现 `index_status.rs:59-84`），只做 FTS reindex，跑完即释放 `indexing` 守卫返回。
- **新增 `spawn_semantic_index(status, db, embedding)`**（在 `spawn_blocking` 内调）：
  1. 语义守卫：`semantic_indexing==true` → **跳过返回**（不排队）。否则置 `true`、写 `semantic_summary="暖机中…"`。
  2. `embedding.prewarm()` → false（无模型/失败）则清守卫、`semantic_summary` 反映句柄 `status()`、返回。
  3. `DocumentIndex::open(db)` → `embed_pending(default_document_roots, embedding, |done,total| 更新 status.semantic_progress)`。
  4. 完成：清守卫、`semantic_progress=None`、`semantic_summary="语义索引就绪 N 篇"`。
- **两处调用点**：① 启动后台任务（`main.rs:320` 那段 FTS reindex `await Ok` 后接 `spawn_semantic_index`）；② `reindex` 命令（`main.rs:233`，FTS `spawn_blocking` 完成 `Ok` 后接 `spawn_semantic_index`）。

### 3.5 状态结构（`IndexStatus` 加字段，镜像现有 `last_summary`）

```rust
pub semantic_indexing: bool,                   // 语义 pass 进行中（并发守卫）
pub semantic_progress: Option<(usize, usize)>, // (已嵌, 待嵌总数)
pub semantic_summary: Option<String>,          // "语义索引就绪 320 篇" / "暖机中…"
```

`#[derive(Default)]` 下三字段默认 `false`/`None`/`None`。feature 关 / 无模型 → worker 不 spawn → 三字段恒默认 → 前端不渲染语义行（与今天视觉一致）。序列化对老前端向后兼容（新增字段，不改旧字段）。

### 3.6 前端（`SettingsPage` / `StatusIndicator`）

复用现有 30s 轮询的 `get_index_status`。新增渲染：`semantic_indexing` 时显示「语义索引中 X/Y」（取 `semantic_progress`）；否则有 `semantic_summary` 显示就绪行；无语义字段（老构建/无模型）时不显示。

## 4. 数据流

- **索引**：触发 → `perform_reindex`（FTS only，释放守卫，结果立即可搜）→ `spawn_semantic_index`（后台：prewarm → embed_pending 渐进填 `document_vectors`，进度入 `IndexStatus`）。
- **查询**：不变（15B-1）。暖机后模型常驻 → 查询期 `ready()` 命中 `Ready` 即时 embed；暖机窗口内查询读 `Loading` → FTS-only 降级。

## 5. 错误处理

- **prewarm 失败 / 无模型**：worker 不跑嵌入循环，`semantic_summary` 反映句柄 `status()`（NotFound/Failed），查询降级 FTS-only。
- **单篇 embed 失败**：计数、跳过、不中断；文档仍 FTS 可搜。
- **语义 worker 在跑时又触发**：FTS 照跑，`spawn_semantic_index` 见守卫 `true` → 跳过。**已知限制**：本轮 FTS 新增文档要等下次触发才补嵌（触发源只有启动+手动，可接受）→ STATUS / 文档登记。
- **进程嵌入中途退出**：每篇 `upsert_vector` 即时落库（autocommit），已嵌部分存活；下次启动 worker 续嵌（`vector_is_current` 跳过已嵌）。
- **feature 关 / 无模型**：`is_active()` false → worker 不 spawn → 逐字节一致。

## 6. 测试

**单元**
- `embed_pending`（用 scan.rs 现有 `StubEmbedder` 3 维确定性桩）：待嵌总数正确；`progress_cb` 回调单调至 `(total,total)`；二次调用全 `vector_is_current` 命中 → 待嵌 0、回调不触发；失败桩计入 failed 不中断。
- `prewarm`：feature 关返 false 不 panic；幂等（连调两次）。
- `IndexStatus` 新字段进度写回的成功分支单测（不跑真 reindex）。

**集成**
- 小 fixture：`spawn_semantic_index` 等价逻辑端到端 —— FTS 先就绪可搜 → 语义 worker 后台补向量 → 进度从 `(0,N)` 走到就绪 summary。
- 解耦验证：`perform_reindex` 现在只做 FTS —— 断言返回时 `indexing` 已释放、`document_vectors` 未被本次填充（由 worker 负责）。

**回归门（硬）**
1. feature **关** / 无模型：现有 evals **byte-equal**（v0.5=473 / v0.9=726 parser-only 不动）；`IndexStatus` 序列化向后兼容。
2. feature **开** + 模型在场：15B-1 跨语言召回 case 不退、exact-name 守护不回退。
3. 全 workspace `fmt --check` / `clippy -D warnings` 0 / `cargo test --workspace`（含 `semantic-recall` 形态）零失败。

**真机手测（登记，留用户）** → `docs/manual-test-scenarios.md` 加 15B-2 节：
- **冷启动消除**：放模型 → 启动 → 等状态条语义就绪 → 首个跨语言查询**不再 16.8s**（对比 15B-1 实测）。
- **解耦**：删大量缓存触发全量 → FTS 结果秒级可搜的同时状态条显示「语义索引中 X/Y」渐进 → 期间可正常搜（FTS）。
- macOS + Windows 双平台（Windows 是冷启动证据来源，必验）。

## 7. 平台

macOS + Windows 同一份调度代码（暖机/解耦与平台无关）。Windows GPU 加速、embedding context 复用属 15B-4。

## 8. 验收标准

1. feature 关 / 无模型：搜索 + 索引行为与今天逐字节一致（evals byte-equal + 现有 backend 测试零回归）。
2. 首查询不再付模型冷加载（暖机已在后台完成）。
3. FTS 结果在语义嵌入完成前即可搜；语义进度在 UI 可见。
4. 全 workspace fmt/clippy/test 三门全绿（含新 feature 形态）。
5. 真机双平台手测通过（登记，留用户执行）。
6. 实测并记录：暖机耗时、解耦后 FTS 可搜时延、语义 worker 全量耗时、常驻内存（供门槛③ / 15B-4 参考）。

## 9. 未尽 / 后续子项交接

- **15B-3**：held-out 评测扩量 + 加权 RRF 权重调优 + FTS 置信度阈值路由 + 语义臂相似度下限 + 「匹配方式」列 UX。
- **15B-4**：Windows GPU（vulkan/cuda）、`model-runtime` embed context 复用、音乐/媒体/OCR 臂、sqlite-vec + int8、探更大模型天花板。
- 本切片已知限制（语义 worker 在跑时新增文档延到下次触发）若需消除 → 引入轻量「dirty 重跑」标志，非当前触发模型必需。
