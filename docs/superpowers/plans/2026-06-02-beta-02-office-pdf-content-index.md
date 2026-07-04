# BETA-02 Office/PDF 内容索引 Implementation Plan

> Steps use checkbox (`- [ ]`). 每个 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** 在 `packages/indexer` 新增 `DocumentIndex`：抽取 docx/xlsx/pptx/pdf/txt/md/html（+旧 xls）正文 → FTS5 全文索引 + 查询（含 snippet 片段）。每文档粒度，mtime 增量。只做索引层。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-02-office-pdf-content-index-design.md) §4。先把 BETA-01 扫描骨架重构为泛型 `IncrementalStore` + `run_incremental_index`（两索引复用），再加 `DocumentIndex` 存储 + `doc_extract` 提取。

**Tech Stack:** Rust；calamine / pdf-extract / zip / quick-xml / pulldown-cmark（均 MIT）。

---

## File Structure

- `packages/indexer/src/scan.rs` — 重构：`IncrementalStore` trait + `run_incremental_index`；`MusicIndex::index_dirs` 接入；文档扩展名 + `default_document_roots`。
- `packages/indexer/src/db.rs` — `MusicIndex` impl `IncrementalStore`（方法已存在，加 trait impl）。
- `packages/indexer/src/doc_db.rs`（新）— `DocumentIndex` 存储层。
- `packages/indexer/src/doc_extract.rs`（新）— 各格式提取。
- `packages/indexer/src/model.rs` — `DocumentEntry` / `DocumentHit` / `DocumentQuery`。
- `packages/indexer/src/lib.rs` — 模块 + 重导出。
- `packages/indexer/Cargo.toml` — 加 calamine/pdf-extract/zip/quick-xml/pulldown-cmark。
- `packages/indexer/tests/real_documents.rs`（新，`#[ignore]`）。

---

## Task 1: 泛型重构扫描骨架（BETA-01 零回归）

- [ ] **Step 1:** scan.rs 定义 `IncrementalStore` trait（`type Entry` + modified_time_of / upsert_entry / paths_under / delete_by_path）。
- [ ] **Step 2:** scan.rs 新增 `run_incremental_index<S, F>(store, roots, exts, extract)`，把现 `index_dirs_with` 的循环搬进来（用 trait 方法）。
- [ ] **Step 3:** db.rs `impl IncrementalStore for MusicIndex { type Entry = MusicEntry; ... }`（委托现有 pub(crate) 方法；这些方法可保留或改为 trait 方法实现）。
- [ ] **Step 4:** `MusicIndex::index_dirs` 改为 `run_incremental_index(self, roots, MUSIC_EXTS, crate::extract::extract_metadata)`；删除旧 `index_dirs_with`（其 stub-注入测试改为直接调 `run_incremental_index(self, roots, MUSIC_EXTS, stub)`）。
- [ ] **Step 5:** 跑 BETA-01 全部测试确认零回归：`cargo test -p locifind-indexer 2>&1 | tail`. Expected: 22 pass（scan stub 测试改用 run_incremental_index 后语义不变）。
- [ ] **Step 6:** fmt + clippy。
- [ ] **Step 7:** Commit `refactor(indexer): 抽 IncrementalStore 泛型扫描骨架（BETA-01/02 复用）`。

## Task 2: DocumentIndex 存储层

- [ ] **Step 1:** model.rs 加 `DocumentEntry` / `DocumentHit` / `DocumentQuery`。
- [ ] **Step 2:** doc_db.rs `DocumentIndex { conn }`：open/open_in_memory/from_conn（schema = documents + documents_fts，spec §4.2）/ count。
- [ ] **Step 3:** `upsert_document(&DocumentEntry, body: &str)`（事务 + FTS 同步，id 稳定 UPDATE）；`delete_by_path` / `modified_time_of` / `paths_under`（与 db.rs 同构；`path_is_under` 复用——提到 scan.rs 或 util）。
- [ ] **Step 4:** `query(&DocumentQuery)`：有 text → JOIN fts + MATCH(`fts_sanitize` 复用) + `snippet(documents_fts,2,'[',']','…',10)`；结构化 author LIKE / doc_type COLLATE NOCASE；`ORDER BY modified_time DESC LIMIT`。复用 `fts_sanitize`（提为 crate 内 `pub(crate)`）。
- [ ] **Step 5:** `impl IncrementalStore for DocumentIndex { type Entry = (DocumentEntry, String); ... }`（upsert_entry 解构 (entry, body)）。
- [ ] **Step 6:** in-memory 单测（spec §5.3）：FTS title/author/body CJK 命中 / author 子串 / doc_type / limit / 重 upsert 刷新 / 删除回收 / snippet 有无 / 转义。
- [ ] **Step 7:** fmt + clippy + test。
- [ ] **Step 8:** Commit `feat(indexer): DocumentIndex 存储层 + FTS5 全文 + snippet 查询`。

## Task 3: 文档提取 doc_extract.rs

- [ ] **Step 1:** Cargo.toml 加依赖（calamine="0.35" / pdf-extract="0.10" / zip="2" / quick-xml="0.40" / pulldown-cmark="0.13"）。`cargo build` 解析版本回填 Cargo.lock。
- [ ] **Step 2:** `extract_document(path, mtime) -> Result<(DocumentEntry, String)>` 按扩展名 dispatch（spec §4.5）。各格式 helper：
  - `extract_ooxml_docx` / `extract_ooxml_pptx`（zip + quick-xml 收集 `<w:t>`/`<a:t>` + core.xml meta + slide 计数）；
  - `extract_spreadsheet`（calamine 遍历 sheet/cell + sheet 计数）；
  - `extract_pdf`（pdf_extract::extract_text）；
  - `extract_html`（quick-xml Text 事件 + 跳 script/style + `<title>`）；
  - `extract_md`（pulldown-cmark Text/Code 事件）；
  - `extract_txt`（读 lossy）。
  - body cap 1 MiB 字符（`truncate_chars` helper）。
- [ ] **Step 3:** 单测（测试内生成样本）：
  - docx/pptx/xlsx：用 zip/calamine 在 tempfile 构造最小样本 → 断言正文含已知词 + doc_type + page_count；
  - md/html/txt：内联字符串 → 断言剥语法、含正文词；
  - 损坏/非文档 → Err。
  > 若 OOXML 手工构造成本高，docx/pptx 用最小 zip 部件（content_types + document.xml/slide1.xml）；可行性实现期确认，不行则降级为 `#[ignore]` 真机样本 + 文档说明。
- [ ] **Step 4:** fmt + clippy + test。
- [ ] **Step 5:** Commit `feat(indexer): 文档正文提取（docx/pptx/xlsx/pdf/md/html/txt）`。

## Task 4: 文档增量 + 默认根目录 + 真机测试

- [ ] **Step 1:** scan.rs 加 `DOC_EXTS`（docx/xlsx/pptx/pdf/txt/md/html/xls/ods，小写）+ `default_document_roots`（dirs::document_dir）。
- [ ] **Step 2:** `DocumentIndex::index_dirs` = `run_incremental_index(self, roots, DOC_EXTS, extract_document)`。
- [ ] **Step 3:** 增量单测（stub 提取器 + run_incremental_index 已测；补 doc 扩展名命中 / 非文档不计 / removed 等）。
- [ ] **Step 4:** `tests/real_documents.rs`（`#[ignore]`）：index_dirs(default_document_roots) + count + 抽样 query + snippet 打印。
- [ ] **Step 5:** fmt + clippy + test。
- [ ] **Step 6:** Commit `feat(indexer): 文档增量索引 + default_document_roots + 真机测试`。

## Task 5: 台账 + README + 全套 CI + 文档收尾

- [ ] **Step 1:** 三方台账登记 calamine/pdf-extract/zip/quick-xml/pulldown-cmark + 关键间接依赖（`cargo tree`，版本以 Cargo.lock 为准）。
- [ ] **Step 2:** README 加 BETA-02 节（格式覆盖 / API / schema / 提取语义 / known limitation：旧二进制 doc/ppt / pdf page_count / body cap / 每文档粒度）。
- [ ] **Step 3:** `bash scripts/ci.sh`（platform-macos Windows 预存失败除外）+ 全 workspace test 零回归确认。
- [ ] **Step 4:** ROADMAP BETA-02 → done + 实证一行。
- [ ] **Step 5:** STATUS 当前阶段 + 会话日志。
- [ ] **Step 6:** 收工 commit + 向用户确认。

---

## 验收对照（spec §5）

- 泛型重构零回归（Task 1）；DocumentIndex 存储/查询/snippet（Task 2）；多格式提取（Task 3）；增量 + 真机（Task 4）；台账/README/ROADMAP/STATUS + ci 绿（Task 5）。
