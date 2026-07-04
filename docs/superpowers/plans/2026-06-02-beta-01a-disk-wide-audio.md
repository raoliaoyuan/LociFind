# BETA-01A 全盘音频索引 Implementation Plan

> Steps use checkbox (`- [ ]`). 每 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** reindex 覆盖全盘音频（发现层枚举，超越固定 Music 目录）：跳过 OneDrive 仅在线占位符（不触发下载）、rayon 并行提取、file_name 进 FTS、发现不可用回退目录扫描。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-01a-disk-wide-audio-design.md) §4。indexer 加 discovery + placeholder + 重构 index_paths + music_fts file_name 迁移；local-index reindex 发现优先。

**Tech Stack:** Rust；rayon（并行）；es.exe/mdfind spawn（发现）；std::os::windows MetadataExt（占位符）。

---

## Task 1: music_fts 加 file_name + 迁移

**Files:** `packages/indexer/src/db.rs`（SCHEMA / upsert FTS / from_conn 迁移）+ 测试。

- [ ] **Step 1:** SCHEMA music_fts 列加 `file_name`。
- [ ] **Step 2:** `upsert_entry` FTS INSERT 加 `file_name`（`e.file_name`）。
- [ ] **Step 3:** `from_conn` 加迁移：建表后 `PRAGMA table_info(music_fts)` 检测无 `file_name` 列 → `DROP TABLE music_fts` + 重建（execute_batch SCHEMA 的 fts 部分）+ `INSERT INTO music_fts(rowid,artist,title,album,file_name) SELECT id,artist,title,album,file_name FROM music`。helper `migrate_music_fts(&Connection)`。
- [ ] **Step 4:** 测试：① 按文件名子串（≥3 字符 CJK/英文）FTS 命中（upsert entry，query text=文件名片段）；② 迁移——手建旧 3 列 music_fts + music 数据（直接 SQL）→ `MusicIndex::open` → 查文件名命中（证明迁移重填）。
- [ ] **Step 5:** 既有 db 测试零回归（trigram / 转义 / 删除等）。
- [ ] **Step 6:** fmt + clippy + test。
- [ ] **Step 7:** Commit `feat(indexer): music_fts 加 file_name 列 + 旧库迁移（按文件名可搜）`。

## Task 2: 占位符检测 + index_paths 并行重构

**Files:** `packages/indexer/src/placeholder.rs`（新）+ `src/scan.rs`（index_paths 重构）+ `Cargo.toml`（rayon）+ `lib.rs`（mod placeholder）+ 测试。

- [ ] **Step 1:** Cargo.toml 加 `rayon`。
- [ ] **Step 2:** `placeholder.rs`：`attrs_indicate_online_only(attrs: u32) -> bool`（纯函数，查 OFFLINE 0x1000 | RECALL_ON_DATA_ACCESS 0x400000）+ `is_online_only(path) -> bool`（cfg(windows) 读 file_attributes 调纯函数；else false）。单测纯函数（两 bit + 普通）。
- [ ] **Step 3:** `index_paths` 重构（spec §4.3）：
  - 顺序预检：ext / fs mtime（failed）/ `modified_time_of`（skipped）/ `is_online_only` → 分 `placeholder`（仅文件名 entry）/ `to_extract`（path+mtime）。
  - rayon：`to_extract.par_iter().map(|(p,mt)| extract_metadata(p,mt)).collect::<Vec<_>>()`。
  - 顺序 upsert：placeholder entries + 提取 Ok entries → upsert（added/updated）；提取 Err → failed。
  - helper `filename_only_entry(path, mtime) -> MusicEntry`。
- [ ] **Step 4:** 测试（temp 真文件）：多 txt（用 stub extract？不行，index_paths 内部直调 extract_metadata）→ 用真 WAV（lofty 往返，复用 extract WAV helper 思路）多文件并行命中；非音乐扩展名跳过；mtime 未变 skip；损坏文件 failed。**并行确定性**（同输入多次结果一致）。占位符路径——非 Windows 无法真造，测 `is_online_only` 纯函数 + `filename_only_entry` 构造正确（仅文件名、无 artist）。
- [ ] **Step 5:** fmt + clippy + test。
- [ ] **Step 6:** Commit `feat(indexer): index_paths 并行提取 + 占位符跳过（仅文件名入库）`。

## Task 3: 发现层（AudioDiscovery + Everything/Spotlight）

**Files:** `packages/indexer/src/discovery.rs`（新）+ `lib.rs`（mod + 重导出）+ 测试 + `tests/real_discovery.rs`（`#[ignore]`）。

- [ ] **Step 1:** `discovery.rs`：`DiscoveryError` + `AudioDiscovery` trait + `EverythingDiscovery`(cfg windows) + `SpotlightDiscovery`(cfg macos) + `default_audio_discovery()`。
  - Everything：spawn es.exe `ext:...` `-export-txt tmp -utf8-bom` → 读 tmp 去 BOM 解析（移植 spike `discover`）；es.exe 定位 PATH + winget fallback。
  - Spotlight：`mdfind 'kMDItemContentTypeTree == "public.audio"'` → stdout 行解析。
  - 解析逻辑抽 `parse_paths_lines(&str) -> Vec<PathBuf>`（纯函数，去空行/trim）单测。
- [ ] **Step 2:** lib.rs `mod discovery; pub use discovery::{AudioDiscovery, DiscoveryError, default_audio_discovery};`
- [ ] **Step 3:** 单测：`parse_paths_lines`（含 BOM 去除 / 空行 / CJK 路径）；`default_audio_discovery` 在当前平台返回 Some/None（不 panic）。
- [ ] **Step 4:** `tests/real_discovery.rs`（`#[ignore]`）：`default_audio_discovery().discover_audio()` 真机非空（Windows 需 Everything / macOS Spotlight）。
- [ ] **Step 5:** fmt + clippy + test。
- [ ] **Step 6:** Commit `feat(indexer): AudioDiscovery 发现层（Everything/Spotlight 全盘枚举）`。

## Task 4: LocalIndexBackend.reindex 发现优先 + desktop

**Files:** `packages/search-backends/local-index/src/lib.rs`（reindex）+ 测试；desktop 无需改（命令签名不变，验证编译）。

- [ ] **Step 1:** `reindex` 改：music 走 `default_audio_discovery()`——Some+Ok(paths)→`index_paths`，否则 `index_dirs(music_roots)`（spec §4.5）。文档不变。
- [ ] **Step 2:** 为可测，抽内部 `reindex_with(discovery: Option<&dyn AudioDiscovery>, music_roots, doc_roots)`：发现优先逻辑纯粹、可注入 mock discovery。`reindex` = `reindex_with(default_audio_discovery().as_deref(), ...)`。
- [ ] **Step 3:** 测试（temp db + 真文件）：注入 mock discovery 返回显式音频路径 → reindex 走 index_paths 命中；mock 返回 Err → 回退 index_dirs(roots)；None → 回退。
- [ ] **Step 4:** desktop 编译确认（`cargo build -p locifind-desktop`）；既有 local-index/desktop 测试零回归。
- [ ] **Step 5:** fmt + clippy + test。
- [ ] **Step 6:** Commit `feat(local-index): reindex 发现优先（全盘音频）+ 回退目录扫描`。

## Task 5: 台账 + README + manual-test + 全套 CI + 收尾

**Files:** `docs/third-party-licenses.md`（rayon）、`packages/indexer/README.md`（discovery/placeholder/file_name FTS）、`packages/search-backends/local-index/README.md`（reindex 发现优先）、`docs/manual-test-scenarios.md`（BETA-01A 节）、`ROADMAP.md`、`STATUS.md`。

- [ ] **Step 1:** 台账加 rayon（+ 关键间接 rayon-core/crossbeam-*，版本以 Cargo.lock）。
- [ ] **Step 2:** indexer README：发现层 / 占位符跳过 / file_name FTS / known limitation（macOS dataless / 不回收 / Everything 可选）；local-index README reindex 发现优先一句。
- [ ] **Step 3:** manual-test-scenarios BETA-01A 节（Windows：立即索引→全盘入库→OneDrive 占位符跳过不下载→跨目录 artist/文件名搜命中）。
- [ ] **Step 4:** `bash scripts/ci.sh`（platform-macos Windows 预存失败除外）+ 全 workspace test 零回归。
- [ ] **Step 5:** ROADMAP BETA-01A → done + 实证；STATUS 当前阶段 + 会话日志。
- [ ] **Step 6:** 收工 commit + 向用户确认（含真机手测留用户）。

---

## 验收对照（spec §5）

- file_name FTS + 迁移（T1）；占位符 + 并行 index_paths（T2）；发现层（T3）；reindex 发现优先（T4）；台账/README/manual-test + ci 零回归（T5）。真机全盘手测留用户。
