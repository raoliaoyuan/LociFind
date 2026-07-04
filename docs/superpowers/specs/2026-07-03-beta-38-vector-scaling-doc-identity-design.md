# BETA-38 设计：向量检索规模化 + 文档身份/去重策略

> 状态：spec（2026-07-03）｜依赖 BETA-15B（语义索引已落地）｜B7 能力卡最后一张
> 关键决策 2026-07-03 用户拍板（AskUserQuestion 四问全采推荐），见 §7。

## 1. 背景与目标

语义召回后端 [`SemanticIndexBackend`](../../../packages/search-backends/semantic-index/src/lib.rs) 现状是**暴力 cosine**：每次查询 `candidate_vectors()` 重开 DB、把 `document_vectors` 全部向量 BLOB 从 sqlite **全量重载进内存**，再逐条算 cosine。冷归档三场景（律所卷宗 / 审计取证 / 离职归档）的语料水位是**十万级文档**，且**常有重复副本**（判决书存多盘、迁移盘、压缩包展开副本）。现状两处不达标：

1. **规模化**：每查询全量重载 BLOB 是真瓶颈（10 万×1024 维×4B ≈ 400MB / 查询），而非 cosine 数学本身。
2. **文档身份**：`documents` 表以 `path` 为唯一键，同内容多副本各存一份向量、各占 topK 名额，且审计留痕无法表达"这是同一份材料的 N 个副本"。

**目标**（对齐 ROADMAP BETA-38 验收）：
- ① 十万级文档向量检索水位基准（p95 延迟 + 内存）对比现暴力扫描，达标可用。
- ② doc identity 策略定义并落库——重复副本不造成索引与审计留痕失真。
- ③ 现有语义召回质量 evals 不回归。

## 2. 核心决策与路线（本次拍板）

### 2.1 规模化 = 进程级内存缓存 + 优化暴力（不引入 ANN 依赖）

`SemanticIndexBackend` 是**进程级长生命周期单例**（main.rs 构造一次注册进 registry），具备驻留缓存条件。方案：

- **进程级向量缓存**：首次查询（或 reindex 后失效）把 `document_vectors` 加载进内存驻留结构（identity → 向量 + 关联 paths），后续查询直接算 cosine，不再重开 DB / 重载 BLOB。
- **缓存失效**：以 db 文件 mtime + `vector_count()` 作廉价失效信号；reindex 写入向量后下次查询自动重载。
- **可选 int8 量化**（YAGNI 门内、基准不达标才做）：把 f32 缓存量化到 int8（内存 ÷4、SIMD 友好），查询时反量化或用 int8 点积近似。默认**不做**，留基准数据驱动。
- **不引入 sqlite-vec / 纯 Rust ANN**：守"轻量可用"（16GB 机）+ 许可洁癖 + 零构建复杂度上升。基准达标即停；若十万级 p95 仍不达标，再在后续卡评估 ANN（本卡不做，登记 backlog）。

> 理由：真瓶颈是 I/O（每查询重载 400MB），不是 O(N) cosine。去掉重载后，10 万×1024 的单次暴力 cosine 在缓存命中下是数十 ms 量级，足够个人/内网检索交互。ANN 的亚线性收益要到更高水位才压过其构建/依赖/近似召回代价。

### 2.2 doc identity = 文件原始字节 hash（提取前全字节）

- 新增文档身份 = **提取正文前的文件原始全字节内容 hash**（非现有 `source_hash`——那是截断后正文的 FNV-1a 指纹，会因格式/提取差异漂移）。
- hash 算法沿用 FNV-1a 64bit（`content_hash` 同族，零依赖、确定性；抗碰撞要求不高——身份判等辅以 size 预判，非密码学场景）。对大文件按全字节流式喂入。
- `size` + `mtime` 作**快速预判**：identity 计算只在 (size 相同) 的候选间才需要，且身份 hash 落库后增量索引可先比 size/mtime 再决定是否重算。

### 2.3 去重 = 索引期嵌一次 + 多 path 映射（结果合并 + 留痕全部副本）

- **索引期**：`embed_pending` 嵌入某文档前，若已有**相同 identity** 的文档持有当前模型的向量，则**复制该向量**而非重新 embed（省算力）。
- **结果期**：语义召回按 identity 去重，同一 identity 的多个 path **合并为一条结果** + `metadata` 带"其余副本位置列表"（留痕不丢，审计可见全部位置）。
- **审计留痕**：identity 落 `documents.content_hash` 列；副本关系可由 `SELECT path FROM documents WHERE content_hash=?` 还原，取证可查"这份材料在哪些位置有副本"。

### 2.4 十万级基准 = evals 侧合成语料生成器

- 在 `packages/evals` 加**合成十万文档生成器**（模板 + 随机正文，含**已知副本组**——复用 BETA-41 `dup_group` 靶设计），可确定性重跑、不入仓大文件（生成到临时目录 / 按 seed 复现）。
- 基准指标：向量检索 **p95 延迟** + **常驻内存**，对比"暴力 baseline（现状全量重载）"vs"缓存后"，出报告。
- 去重正确性：生成语料里 K 组已知副本，断言检索结果里同组合并为一条 + 副本位置齐全。

## 3. 范围护栏（YAGNI）

- **不引入 ANN**（sqlite-vec / hnsw）——见 §2.1；不达标才在后续卡评估。
- **不做跨机/分布式索引**——单机十万级水位。
- **不改嵌入模型 / 维度 / 截断常量**——规模化与身份是检索/存储层，不动 embedding 语义。
- **不做密码学级去重**（无需抗恶意碰撞）——FNV-1a + size 预判足够冷归档副本识别。
- **不改 FTS 臂 / trigram / 排序融合**——仅语义臂 + indexer 存储层。
- **daemon 侧对齐**：per-collection 独立 db 已是 BETA-36 物理信息墙；本卡缓存/去重在 backend 层，daemon 复用同后端天然获益，不新增跨 collection 逻辑。

## 4. 数据流改动

### 4.1 indexer（packages/indexer）

- **schema**：`documents` 加 `content_hash TEXT`（可空——老库 / 未回填时 NULL）；`CREATE INDEX idx_documents_content_hash ON documents(content_hash)`。`CREATE IF NOT EXISTS` + 老库打开自动 `ALTER TABLE ADD COLUMN`（同既有无 schema-bump 演进套路；列可空避免迁移）。
- **hash 计算**：`scan.rs` 增量提取时对文件原始字节算身份 hash（`embed::file_identity_hash(bytes)` 或流式）；写入 `documents.content_hash`。图片/邮件等已读字节的提取路径复用同一入口。
- **索引期去重**：`embed_pending` 收集 pending 后，对每篇先查"是否已有同 `content_hash` 且当前 model_id 的向量"，命中则 `upsert_vector` 复制该向量（不调 `embedder.embed`）；未命中才真嵌。计数区分"嵌入 / 复用 / 失败"。
- **查询候选**：`candidate_vectors()` 附带 `content_hash`（供 backend 按 identity 去重 + 收集副本 paths）；或新增 `candidate_vectors_with_identity()` 保旧接口。

### 4.2 semantic-index（packages/search-backends/semantic-index）

- **进程级缓存**：`SemanticIndexBackend` 内持 `Mutex<Option<VectorCache>>`（或 `RwLock`）；`VectorCache { loaded_signal, entries: Vec<{identity, vector, paths}> }`。查询时校验 db mtime + vector_count 未变则复用缓存，变则重载。
- **按 identity 去重**：暴力 cosine 在 identity 粒度算（同 identity 只算一次），结果映射时取该 identity 的代表 path + 其余副本进 `metadata`。
- `filter_rank_topk` 逻辑不变（纯函数）；`vector_hit_to_result` 扩展带副本列表。
- **降级不变**：无嵌入器 / db 不存在 → 空，FTS 兜底。

### 4.3 evals（packages/evals）

- 合成十万语料生成器（seed 可复现 + K 组已知副本）+ 基准 harness（p95 延迟 + 内存）+ 去重正确性断言。
- 基准报告落 `docs/reviews/` 或 evals 报告目录。

### 4.4 desktop（apps/desktop）

- 复用同后端，无需改命令；reindex 后缓存自动失效。
- （可选）状态摘要展示"去重节省 N 篇嵌入"——YAGNI，基准阶段不做。

## 5. 已知边界与 trade-off

- **FNV-1a 碰撞**：非密码学 hash，理论碰撞概率极低但非零；辅以 `size` 预判降低误判。冷归档去重误合并的代价是"两份不同材料被当副本合并"——用 size 预判 + 可选二次比对缓解；本卡不做全字节二次比对（YAGNI，登记边界）。
- **缓存内存**：10 万×1024×4B ≈ 400MB 常驻。16GB 机可接受；更高水位触发 int8 量化（§2.1 可选项）或 ANN（后续卡）。
- **首查询冷启动**：缓存未命中的首查询仍需一次全量加载（与现状每查询等价），之后摊薄。
- **identity 只覆盖已嵌文档**：未过嵌入门槛（body 极短 / 图片默认关）的文档不参与语义去重（本就不进语义召回）。

## 6. 验收对照

| ROADMAP 验收 | 本 spec 对应 |
|---|---|
| ① 十万级 p95 延迟 + 内存基准对比暴力 | §2.4 + §4.3 合成语料生成器 + 基准 harness；缓存后 vs 暴力 baseline 出报告 |
| ② doc identity 定义并落库 | §2.2 文件原始字节 hash → `documents.content_hash` + 索引；§2.3 副本关系可 SQL 还原（审计留痕） |
| ③ 语义召回质量 evals 不回归 | 缓存/去重不改 cosine 语义；现有 semantic_quality evals + 单测守护；去重仅合并真副本（identity 相同） |

## 7. 关键决策（2026-07-03 用户确认，全采推荐）

1. **规模化路线** → 先做内存缓存 + 优化暴力（不引入 ANN）；基准达标即停，不达标后续卡再评估 ANN。
2. **doc identity 主键** → 文件原始字节 hash（+ size/mtime 预判），非现有正文 `source_hash`。
3. **去重层** → 索引期嵌一次 + 多 path 映射；结果合并为一条 + 留痕全部副本位置。
4. **基准验收** → evals 侧合成十万语料生成器（复用 BETA-41 dup_group 靶），跑 p95 延迟 + 内存。

## 8. Cycle 计划

- **Cycle 1｜doc identity 落库**：`documents.content_hash` 列 + 老库 ALTER 迁移 + `file_identity_hash` + scan.rs 回填 + 单测（新库/老库迁移/hash 稳定性/副本同 hash）。
- **Cycle 2｜索引期去重**：`embed_pending` 同 identity 复用向量（不重嵌）+ 计数区分嵌入/复用/失败 + 单测（两副本只嵌一次、向量一致、第三副本复用）。
- **Cycle 3｜进程级向量缓存**：`SemanticIndexBackend` 缓存 + mtime/count 失效 + identity 粒度 cosine + 结果带副本列表 + 单测（缓存命中/失效重载/去重合并/副本留痕）。
- **Cycle 4｜十万级基准**：evals 合成语料生成器 + 基准 harness（p95 + 内存）+ 去重正确性断言 + 报告；对比暴力 baseline 记录数据。
- **收尾**：ROADMAP BETA-38 标 done + 验收对照回填；README（indexer + semantic-index）补 identity/缓存说明；licenses 无新依赖（如引入 criterion 仅 dev-dep 需登记）。

每 cycle 收口：`cargo fmt` + `clippy -D warnings` + 相关 crate 全测 + 无回归。
