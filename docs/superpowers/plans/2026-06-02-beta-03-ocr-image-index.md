# BETA-03 图片 OCR 内容索引 Implementation Plan

> Steps use checkbox (`- [ ]`). 每 task 末尾过 fmt + clippy(-D warnings) + test（含 fmt，勿只 clippy+test）。

**Goal:** 对图片做 OCR、文字进现有 FTS、「找含某词的截图/图片」端到端命中。Windows 先行（WinRT + Tesseract 兜底），引擎 trait 留 macOS。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-03-ocr-image-index-design.md) §3。indexer `OcrEngine` trait + WinRT/Tesseract 实现 + `index_image_dirs`（复用 `run_incremental_index`）+ 回收按扩展名收窄；doc_db `DocumentQuery.doc_types` 过滤；local-index `MediaSearch(Image)` 路由 + reindex 图片轮；desktop 接 image_roots + 状态计数。

**Tech Stack:** Rust；**无新 cargo 依赖**（PowerShell / tesseract 为运行期外部进程）。spike 已验证 WinRT 路径（spec §1）。

---

## Task 1: OCR 引擎层（`packages/indexer/src/ocr.rs` + `ocr/win_ocr.ps1`，全新）

**Files:** 新建 `packages/indexer/src/ocr.rs`、`packages/indexer/src/ocr/win_ocr.ps1`；`lib.rs` 挂模块 + 导出。

- [ ] **Step 1:** `OcrEngine` trait（`recognize(&self, &Path) -> Result<String, IndexError>` + `name(&self) -> &str`，`Send+Sync+Debug`）。
- [ ] **Step 2:** `normalize_ocr_text(&str) -> String` 纯函数：相邻 CJK 表意字符间空白丢弃，拉丁词间保留。+ 单测（`会 议 纪 要`→`会议纪要`；`Hello World` 不变；`图 片 abc 文 字`→`图片 abc 文字`）。CJK 判定用字符范围 helper（`is_cjk` 覆盖 CJK 统一表意 + 扩展 A + 兼容）。
- [ ] **Step 3:** `win_ocr.ps1`：spike 验证过的脚本（`AsTask` await 辅助 + `BitmapDecoder.CreateAsync→GetSoftwareBitmapAsync` + `OcrEngine.TryCreateFromUserProfileLanguages.RecognizeAsync`），图片路径读自 `$env:LOCIFIND_OCR_IMAGE`，stdout 打印 `$result.Text`，错误写 stderr + exit 非 0。`[Console]::OutputEncoding=UTF8`。
- [ ] **Step 4:** `WindowsOcrEngine`（`#[cfg(windows)]`）：`detect() -> bool`（轻量 PS 查 `AvailableRecognizerLanguages` 非空）；`recognize` = 临时落地内嵌 .ps1（`include_str!`）→ spawn `powershell -NoProfile -NonInteractive -File <tmp>` + env `LOCIFIND_OCR_IMAGE` → 超时 kill → stdout UTF-8 → `normalize_ocr_text`。non-zero exit / 空输出按需返 Err。
- [ ] **Step 5:** `TesseractOcrEngine`（跨平台）：`detect()`（`tesseract --version` 在 PATH）；`recognize` = spawn `tesseract <image> stdout -l chi_sim+eng`（结构化参数 + 超时 kill）→ stdout → `normalize_ocr_text`。
- [ ] **Step 6:** `default_ocr_engine() -> Option<Box<dyn OcrEngine>>`：Windows detect→WinRT；否则 tesseract detect→Tesseract；都无→None。探测分支逻辑抽 `pick_engine(win_ok: bool, tess_ok: bool)` 纯函数便于单测（不真调系统）。
- [ ] **Step 7:** lib.rs 挂 `mod ocr;` + `pub use ocr::{OcrEngine, default_ocr_engine, normalize_ocr_text};`。
- [ ] **Step 8:** fmt + clippy + test（indexer）。spawn 执行器对 `print_stderr`/`unwrap` 按现有 crate 习惯处理。
- [ ] **Step 9:** Commit `feat(indexer): OCR 引擎层（Windows.Media.Ocr + Tesseract 兜底 + CJK 归一）`。

## Task 2: 图片增量索引 + 回收收窄 + doc_types 过滤（`packages/indexer`）

**Files:** `src/scan.rs`（IMAGE_EXTS、index_image_dirs、回收收窄、default_image_roots、image_entry）、`src/doc_db.rs` + `src/model.rs`（DocumentQuery.doc_types）。

- [ ] **Step 1:** scan.rs 回收循环加 `&& has_ext(Path::new(&p), exts)` 收窄（spec §3.2）。+ 单测：同目录 1 png + 1 txt，文档轮（DOC_EXTS）不回收 png；图片轮（IMAGE_EXTS）不回收 txt。既有 `deleted_file_is_removed` 不回归。
- [ ] **Step 2:** model.rs `DocumentQuery` 加 `pub doc_types: Option<Vec<String>>`（Default None）。
- [ ] **Step 3:** doc_db.rs `query` 接 doc_types：filters 动态加 `d.doc_type IN (...)`（参数绑定，非插值）当 `Some` 且非空。+ 单测：插 png + docx，`doc_types=Some(["png"])` 只返 png；None 返全部。
- [ ] **Step 4:** scan.rs `IMAGE_EXTS` const + `default_image_roots()`（`dirs::picture_dir()`）+ `image_entry(path, mtime) -> DocumentEntry`（doc_type=小写扩展名，title/author=None）。
- [ ] **Step 5:** scan.rs `impl DocumentIndex { pub fn index_image_dirs(&self, roots, ocr: &dyn OcrEngine) -> Result<IndexStats> }` = `run_incremental_index(self, roots, IMAGE_EXTS, |p, mt| Ok((image_entry(p, mt), normalize_ocr_text(&ocr.recognize(p)?))))`。
- [ ] **Step 6:** 测试（stub `OcrEngine` 隔离真 OCR，返回固定文字）：`index_image_dirs` 扫描只数图片扩展名；OCR 文字进 FTS 可 `query` 命中；mtime skip；删图回收；stub 返 Err → failed 不中断。
- [ ] **Step 7:** fmt + clippy + test（indexer）。
- [ ] **Step 8:** Commit `feat(indexer): 图片 OCR 增量索引 + 回收按扩展名收窄 + doc_types 过滤`。

## Task 3: 检索路由 + reindex 图片轮（`packages/search-backends/local-index`）

**Files:** `src/lib.rs`。

- [ ] **Step 1:** `IMAGE_DOC_TYPES` const（与 indexer IMAGE_EXTS 对齐）+ `build_image_query(m: &MediaSearch) -> Option<DocumentQuery>`（keyword 非空 → `Some(DocumentQuery{ text, doc_types: Some(IMAGE_DOC_TYPES), limit })`，空→None）。+ 单测。
- [ ] **Step 2:** `search_results` 的 `MediaSearch(Image|Screenshot)` 分支改：`build_image_query` Some→`DocumentIndex.query`→`doc_hit_to_result`；None→空。其余 MediaSearch（audio 已有 / video）维持。
- [ ] **Step 3:** `reindex_with`：文档 `index_dirs` 后，`default_ocr_engine()` Some → `docs.index_image_dirs(image_roots, &*engine)`；签名/返回值扩展为 `(music, doc, image): (IndexStats, IndexStats, IndexStats)`（引擎 None → image 为 `IndexStats::default()`）。`reindex` 公有签名加 `image_roots: &[PathBuf]` 参数。
- [ ] **Step 4:** 测试：MediaSearch(Image) 带 keyword 经 stub OCR 入库后命中、doc_type 框定不返 docx；无 keyword→空；reindex 三元组（引擎 None 时 image=default）。既有 `image_media_returns_empty` 改为「无 keyword 仍空」语义 + 新增「带 keyword 命中」。
- [ ] **Step 5:** fmt + clippy + test（local-index + indexer）。
- [ ] **Step 6:** Commit `feat(local-index): 图片 OCR 检索路由 + reindex 图片轮`。

## Task 4: 桌面接线 + 文档 + 全套 CI（`apps/desktop` + docs）

**Files:** `apps/desktop/src-tauri/src/{search,main}.rs`（reindex/perform_reindex 传 image_roots + summary 计数）、`packages/indexer/README.md`、`packages/search-backends/local-index/README.md`、`docs/{manual-test-scenarios,windows-setup}.md`、ROADMAP/STATUS。

- [ ] **Step 1:** desktop `perform_reindex` / `reindex` 调 `LocalIndexBackend::reindex(music_roots, doc_roots, default_image_roots())`；`IndexStatus.last_summary` 加「图片 K」。既有调用点 + 测试随签名更新。
- [ ] **Step 2:** fmt + clippy + tsc + test（desktop）。
- [ ] **Step 3:** indexer README 加 OCR 节（引擎策略 + CJK 归一 + 逐文件 v1 + 外部工具可选）；local-index README reindex 图片一句。
- [ ] **Step 4:** `tests/real_ocr.rs`（`#[ignore]`，Windows 真机）：随附含已知文字小 PNG fixture，`WindowsOcrEngine::recognize` 断言含关键子串（容忍噪声）。
- [ ] **Step 5:** `bash scripts/ci.sh`（platform-macos 2 预存除外）+ 全 workspace test 零回归。`docs/third-party-licenses.md` 记一句运行期可选外部工具（无新 cargo 依赖）。
- [ ] **Step 6:** manual-test BETA-03（含字截图入图片目录 → reindex → 搜命中）；windows-setup 记 OCR 语言包 / Tesseract 可选。
- [ ] **Step 7:** ROADMAP BETA-03 → done；STATUS 当前阶段 + 会话日志。
- [ ] **Step 8:** 收工 commit + 向用户确认（真机手测留用户）。

---

## 验收对照（spec §4）

- normalize_ocr_text（T1.2）；回收收窄（T2.1）；index_image_dirs 端到端 stub（T2.6）；build_image_query + 路由（T3.4）；doc_types 过滤（T2.3）；真 OCR `#[ignore]`（T4.4）；default_ocr_engine pick 分支（T1.6）；零回归 + 台账（T4.5）；文档（T3/T4）；真机手测（用户）。

## Task 依赖

T1（引擎层，独立）→ T2（索引，用 normalize/OcrEngine）→ T3（路由 + reindex，用 index_image_dirs）→ T4（desktop + docs，用 reindex 新签名）。严格串行（每层依赖下层 API）。
