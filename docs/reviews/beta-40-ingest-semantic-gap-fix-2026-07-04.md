# PDF/JPG/PNG/OCR 落库专项排查与修复 + daemon 语义臂补齐

> 日期：2026-07-04
> 执行者：Claude Code
> 承接：[enterprise-semantic-daemon-test-report-2026-07-03.md](./enterprise-semantic-daemon-test-report-2026-07-03.md) §5.1「PDF/JPG/PNG/OCR 仍需专项验收」与 STATUS 下一步 ①。

## 1. 排查方法

直接检查 2026-07-03 smoke 遗留的 7 个 collection SQLite（`E:\Locifind-smoke\semantic-daemon\data`），
逐库对比 `documents` 表与磁盘源文件，再顺代码链路定位根因。

## 2. 根因（4 个，全部实锤复现）

### 2.1 中文文本层 PDF 因 pdf-extract panic 静默丢失

`blueharbor-judgment-text-layer.pdf`（smoke 中 scanned=15 / added=14 差的那 1 个）复现：

```text
pdf-extract-0.10.0\src\lib.rs:983: unsupported encoding UniGB-UCS2-H → panic
```

`UniGB-UCS2-H` 是中文 CID 字体极常用的 CMap（大量国产办公软件产出的 PDF 都用），
panic 被 `scan.rs::catch_extract` 兜住计 `stats.failed`——**文件静默不落库、无任何留痕**。
对律所卷宗场景是结构性缺口。

### 2.2 daemon 完全没有图片索引轮

`apps/daemon/main.rs`（首次索引）与 `locifind-server/reindex.rs` 只跑 music + document
两轮；桌面端的 `index_image_dirs_*`（BETA-03 图片 OCR）从未接入 daemon → JPG/PNG
**在企业 collection 上从不进索引**（smoke 材料中 5 张图片全部缺失）。

### 2.3 daemon 语义臂名不副实（重要）

- 7 个 collection 的 `document_vectors` 全部为 **0**——`embed_pending`（写向量的
  嵌入 pass）只有桌面端调用，daemon 从不跑；
- daemon 检索候选链 `build_local_search_candidates` **只有** `LocalIndexBackend`
  （FTS 臂），`SemanticIndexBackend` 根本不在链上；
- 加载的真实 GGUF 模型在 daemon 里只用于启动 ping probe，之后再无消费点。

**结论：2026-07-03 报告中三场景的手工 MCP 命中实际全部是 FTS 字面命中，语义召回
从未生效**；`handover` / `项目交接` degraded 的直接原因就是无语义兜底。

### 2.4 WinRT OCR 拒绝正斜杠路径（接入图片轮后暴露）

daemon TOML 配置 roots 惯用 `/`，walkdir 拼出 `D:/.../offboarding\xxx.png` 混合分隔符；
WinRT `GetFileFromPathAsync` 报「指定的路径无效」→ 图片 OCR 全数失败。桌面端设置
存的是反斜杠路径，从未踩到。

另有横切问题：**文件级提取失败只累计 `IndexStats.failed` 计数**，哪个文件、什么原因
均不落库（`document_failed_pages` 只覆盖"成功处理的扫描 PDF 的失败页"），企业取证
场景无法复核。

## 3. 修复

| # | 修复 | 位置 |
|---|---|---|
| 1 | `extract_pdf` 内层 `catch_unwind`：pdf-extract panic / Err → 按「无可用文本层」降级 rasterize + OCR 管线（不再整份丢） | `packages/indexer/src/doc_extract.rs` |
| 2 | 文件级提取失败留痕：新表 `index_failures(path, reason, failed_time)`；失败落表、成功重扫清除、磁盘删除随回收清除；`IncrementalStore` trait 加默认 no-op 方法（音乐库暂不留痕）；公开 `extraction_failures()` / `extraction_failure_count()` | `packages/indexer/src/{doc_db,scan,model}.rs` |
| 3 | daemon 补图片 OCR 轮：首次索引与 `/admin/reindex` 都跑 `index_image_dirs_excluding_with_progress`；OCR 引擎 reindex 时现场重探测（装好依赖无需重启） | `apps/daemon/src/main.rs`、`packages/locifind-server/src/reindex.rs` |
| 4 | daemon 语义臂补齐：① 索引后跑 `embed_pending`（`embed_images=false` 与桌面默认一致）写 `document_vectors`；② 候选链按 embedder probe 结果追加 `SemanticIndexBackend`（floor 0.30 镜像桌面默认）；③ `SearchTool::invoke` 含语义臂时改走桌面同款 `run_fanout_merge_rrf` 加权融合，否则维持原 fallback chain 零变化 | `packages/locifind-server/src/tools/search.rs`、同上两处 |
| 5 | WinRT OCR 路径归一：`WindowsOcrEngine::recognize` 把 `/` 归一为 `\` 再传脚本 | `packages/indexer/src/ocr.rs` |
| 6 | 启动期依赖探测留日志：OCR 引擎 / pdftoppm 不可用时 daemon 启动 warn 指明后果 | `apps/daemon/src/main.rs` |

## 4. 验证

自动化（本机全通过）：

- `cargo test -p locifind-indexer`：全绿，新增 ① `extract_failure_recorded_cleared_and_recycled_for_documents`（留痕全周期）② `cjk_cmap_text_layer_pdf_falls_back_to_ocr_not_panic`（UniGB PDF 回归，CI 安全二分断言）
- `cargo test -p locifind-server`（66）/ `locifindd`（含 e2e 9/9）/ `local-index` / `semantic-index` / `harness`：全绿
- `cargo clippy -p locifind-indexer -p locifind-server -p locifindd --all-targets`：净

真实材料端到端（真实模型 + 企业三场景材料，全新 data dir）：

| 验证点 | 结果 |
|---|---|
| UniGB 文本层 PDF | 降级日志出现 → OCR 落库（body 88 chars）→ 入语义索引 |
| `document_vectors` | 7 collection 全部写入（例：blueharbor 14、audit 15），embed_failed=0 |
| 图片 JPG/PNG | 修复 5 后二轮全部入库（image_added 1/1/2、failed=0），`对账文件` 命中 kunpeng-dashboard-screenshot.png、`现场交付照片` 顶位命中 evidence-delivery-site.jpg |
| 失败留痕 | 首轮图片 OCR 失败按 (path, reason) 落 `index_failures`；成功重扫后自动清除 |
| 语义臂生效 | 昨日 degraded 的 `项目交接` 现返回 handover-checklist / handover-arrangement / exit-confirmation 等相关命中，degraded=false |

## 5. 遗留

1. **2 字 CJK 词仍无 FTS 召回**（`鲲鹏`/`值班`/`看板`）：trigram 结构性限制（BETA-42
   已知），图片这类**默认不入语义索引**的文档在纯 2 字词查询下仍不可达；后续可评估
   图片语义 opt-in（BETA-39）在企业场景默认开启，或 2 字词走 LIKE 兜底。
2. `DEFAULT_SEMANTIC_WEIGHT=10` 下语义臂强势，FTS 字面命中（如图片 OCR 精确匹配）
   排位靠后（score 0.016 vs 0.16）；权重是否要 daemon 侧独立调低待评测数据说话。
3. 音乐库暂不做文件级失败留痕（trait 默认 no-op），需要时补 `MusicIndex` 实现即可。
4. 桌面端复用同一 indexer，自动获得修复 1/2/5；桌面 UI 消费 `extraction_failures()`
   展示"未能索引的文件"清单是后续增强项。
