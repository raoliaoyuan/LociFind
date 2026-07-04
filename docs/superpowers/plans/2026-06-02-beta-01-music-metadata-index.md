# BETA-01 音乐 metadata 索引 Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking. 每个 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** 新建 `packages/indexer` crate，提供音乐 metadata 索引层 + 查询 API：扫描音乐目录（可配置多目录）→ lofty 提取标签 → rusqlite/FTS5 增量存储 → 查询。只做索引层，不接 Agent（spec §1）。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-01-music-metadata-index-design.md) §4。`MusicIndex`（持 rusqlite `Connection`）+ `MusicEntry` 数据模型 + `MusicQuery`；`extract_metadata`（lofty 适配）；`index_dirs`（walkdir + mtime 增量 + removal）；`default_music_roots`（dirs::audio_dir）。

**Tech Stack:** Rust；rusqlite(bundled, FTS5)；lofty；walkdir；dirs；thiserror。

---

## File Structure

- `packages/indexer/Cargo.toml` — 新 crate manifest。
- `packages/indexer/src/lib.rs` — 公共 API 重导出 + `IndexError`。
- `packages/indexer/src/model.rs` — `MusicEntry` / `MusicQuery` / `IndexStats`。
- `packages/indexer/src/db.rs` — `MusicIndex` 存储层（open/schema/upsert/query/count/delete + FTS 同步 + 转义）。
- `packages/indexer/src/scan.rs` — `index_dirs`（walkdir + 增量）+ 扩展名白名单 + `default_music_roots`。
- `packages/indexer/src/extract.rs` — `extract_metadata`（lofty）。
- `packages/indexer/README.md` — 职责 / API / schema / known limitation。
- `Cargo.toml`（根）— members 加 `packages/indexer`。
- `docs/third-party-licenses.md` — 登记新依赖。

---

## Task 1: Crate 骨架 + workspace 接入 + schema

**Files:** `packages/indexer/Cargo.toml`（新）、`src/lib.rs`（新）、`src/model.rs`（新）、`src/db.rs`（新，仅 open/schema/count）、根 `Cargo.toml`。

- [ ] **Step 1: 根 Cargo.toml members 追加** `"packages/indexer",`（在 model-runtime 后）。
- [ ] **Step 2: 写 `packages/indexer/Cargo.toml`**：

```toml
[package]
name = "locifind-indexer"
description = "本地音乐 metadata 索引（BETA-01）：lofty 标签提取 + SQLite/FTS5 存储 + 增量"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true

[lints]
workspace = true

[dependencies]
rusqlite = { version = "0.32", features = ["bundled"] }
lofty = "0.21"
walkdir = "2"
dirs = "5"
thiserror = "2"

[dev-dependencies]
tempfile = "3"
```

> 版本以 `cargo build` 实解为准，解析后回填 Cargo.lock；若 0.32/0.21 不可用取最近兼容版并记 README。

- [ ] **Step 3: `src/model.rs`** — 定义 `MusicEntry`（spec §4.1，全 `Option` 除 path/file_name/modified_time）、`MusicQuery`（`#[derive(Default)]`）、`IndexStats`。均 `#[derive(Debug, Clone, PartialEq)]`；`MusicEntry`/`IndexStats` 加 `Default` 便于测试构造。
- [ ] **Step 4: `src/lib.rs`** — 模块声明 + 重导出 + `IndexError`（thiserror）：

```rust
#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("读取标签失败 {path}: {detail}")]
    Tag { path: String, detail: String },
    #[error("IO 错误 {path}: {detail}")]
    Io { path: String, detail: String },
}
```

- [ ] **Step 5: `src/db.rs` 起步** — `MusicIndex { conn: rusqlite::Connection }`；`open(&Path)` / `open_in_memory()` 调 `init_schema`（执行 spec §4.2 建表 SQL，`execute_batch`）；`count()`。
- [ ] **Step 6: 编译 + 一个 smoke 测试**：`open_in_memory()` 成功 + `count()==0`。
  Run: `cargo test -p locifind-indexer 2>&1 | tail -15`
- [ ] **Step 7: fmt + clippy**：`cargo fmt -p locifind-indexer --check && cargo clippy -p locifind-indexer --all-targets -- -D warnings 2>&1 | tail -5`
- [ ] **Step 8: Commit** `feat(indexer): BETA-01 crate 骨架 + SQLite/FTS5 schema`

---

## Task 2: 存储 / 查询核心（upsert + FTS 同步 + query + 转义）

**Files:** `src/db.rs`（扩展）、测试同文件 `#[cfg(test)] mod tests`。

- [ ] **Step 1: 写失败测试**（in-memory，直接喂 `MusicEntry`）：
  - `upsert` 两条不同 artist → `count()==2`；
  - `query{ text: Some("周华健") }` 命中 artist 含「周华健」的记录（CJK FTS）；
  - `query{ artist: Some("hua") }` 子串大小写不敏感命中；
  - `query{ format: Some("flac") }` 精确大小写不敏感；
  - `query{ limit: Some(1) }` 截断；
  - 同 path 第二次 upsert（改 title）→ `count()` 不变且 query 返回新 title（FTS 同步刷新）；
  - `query{ text: Some("a\" OR b *") }`（含 FTS 语法字符）不 panic、不报错。
- [ ] **Step 2: 跑测试确认失败**（方法未实现）。
- [ ] **Step 3: 实现 `upsert_entry(&MusicEntry)`** —— 事务内 `INSERT INTO music(...) ON CONFLICT(path) DO UPDATE SET ...` 取回 rowid（`SELECT id FROM music WHERE path=?`）；`DELETE FROM music_fts WHERE rowid=?` + `INSERT INTO music_fts(rowid,artist,title,album) VALUES(?,?,?,?)`。返回是新增还是更新（供 scan 计数）。
- [ ] **Step 4: 实现 `delete_by_path(&str)`** —— 先删 fts 行（按 rowid）再删 music 行。
- [ ] **Step 5: 实现 `query(&MusicQuery)`** ——
  - 有 `text`：先 `SELECT rowid FROM music_fts WHERE music_fts MATCH ?`，MATCH 输入经 `fts_sanitize`（见 Step 6）；与主表 join 取字段。
  - `artist`/`album`：`AND artist LIKE '%'||?||'%' COLLATE NOCASE`（参数绑定，非拼接）。
  - `format`：`AND format = ? COLLATE NOCASE`。
  - `ORDER BY artist, title`，`LIMIT ?`（默认 50）。
  - 各分支用参数绑定，杜绝 SQL 注入。
- [ ] **Step 6: 实现 `fts_sanitize(&str) -> String`** —— 把用户文本包成 FTS5 单个 "双引号短语" 并对内部 `"` 转义为 `""`，末尾去引号后加 `*` 前缀通配（`"周华健"*`）。保证任意输入都是合法 FTS5 query，不触发语法/注入。
- [ ] **Step 7: 跑测试确认通过** + 全 crate 测试。
- [ ] **Step 8: fmt + clippy。**
- [ ] **Step 9: Commit** `feat(indexer): 存储/查询核心——upsert + FTS5 同步 + 转义查询`

---

## Task 3: 增量索引（walkdir + mtime + removal）

**Files:** `src/scan.rs`（新）、`src/lib.rs`（加 `pub mod scan` + 重导出 `default_music_roots`）、`src/db.rs`（加内部查询 helper：`modified_time_of(&str)`、`paths_under(&[PathBuf])`）、测试 `src/scan.rs` `#[cfg(test)] mod tests`。

- [ ] **Step 1: 扩展名白名单常量** `const MUSIC_EXTS: &[&str]`（spec §4.4，小写比较）。
- [ ] **Step 2: `default_music_roots() -> Vec<PathBuf>`** —— `dirs::audio_dir().into_iter().collect()`。
- [ ] **Step 3: 写失败测试**（用 `tempfile::tempdir` + 真 fs，但**不依赖音频解析**——通过把 extract 抽象为可注入闭包，见 Step 4）：
  - 建 3 个假音乐文件（`.mp3`/`.flac` 扩展名，内容任意）+ 1 个 `.txt`；用 **stub 提取器**（测试注入，返回固定 `MusicEntry`）；
  - 首次 `index_dirs_with(roots, stub)` → `added==3 skipped==0`，`.txt` 不计入 `scanned`；
  - 不改文件再跑 → `skipped==3 added==0`；
  - `touch` 一个文件改 mtime 再跑 → `updated==1 skipped==2`；
  - 删一个文件再跑 → `removed==1`；
  - root 外另建一条记录（直接 upsert）→ 再 `index_dirs` 不应 `removed` 它。
- [ ] **Step 4: 实现 `index_dirs`** —— 内部委托 `index_dirs_with(roots, extract_metadata)`；`index_dirs_with` 接受 `extract: impl Fn(&Path, i64) -> Result<MusicEntry, IndexError>`（让测试注入 stub，生产传真 `extract_metadata`）：
  1. walkdir 每个 root（`follow_links(false)`，`filter_map` 忽略遍历 Err 项），过滤白名单扩展名文件；
  2. 取 mtime（unix 秒）；查 `modified_time_of(path)`：相等→`skipped`；否则 `extract` 成功则 upsert（`added`/`updated`）、`extract` Err 则 `failed`；
  3. 记录本轮见到的 path `HashSet`；
  4. `paths_under(roots)` 取 DB 中前缀落在任一 root 的 path，差集（DB 有但本轮未见）逐个 `delete_by_path` → `removed`。
- [ ] **Step 5: 跑测试确认通过** + 全 crate 测试。
- [ ] **Step 6: fmt + clippy。**
- [ ] **Step 7: Commit** `feat(indexer): 增量索引——walkdir + mtime 比对 + 磁盘删除回收`

---

## Task 4: 标签提取（lofty）+ WAV 往返测试

**Files:** `src/extract.rs`（新）、`src/lib.rs`（加 `mod extract` + 重导出）、测试 `src/extract.rs` `#[cfg(test)] mod tests`、`tests/real_music.rs`（`#[ignore]` 真机）。

- [ ] **Step 1: 实现 `extract_metadata(&Path, modified_time: i64) -> Result<MusicEntry, IndexError>`**（spec §4.4）：
  - `lofty::read_from_path` → `TaggedFile`；err 映射 `IndexError::Tag`；
  - `primary_tag().or(first_tag())` 取 `Artist`/`Title`/`Album`（`tag.get_string(&ItemKey::TrackArtist)` 等，按 lofty 0.21 API）；
  - `properties().duration().as_secs_f64()`（>0 才存，否则 None）；
  - `properties().audio_bitrate()` → `Option<u32>`；
  - format = `file_type()` 短名（`FileType::Mpeg`→"MP3"、`Flac`→"FLAC"、`Mp4`→"MP4"、`Vorbis`/`Opus`/`Wav`/`Aiff` 等映射，未知用 Debug 名）；
  - file_name = path 文件名。
- [ ] **Step 2: WAV 测试辅助 `fn write_silent_wav(path)`** —— 纯 Rust 写最小合法 WAV（44 字节 RIFF/`fmt `/`data` 头 + 极短静音 PCM，采样率 8000、单声道、16bit、数百样本使 duration 可测 >0）。
- [ ] **Step 3: 写测试**：
  - `write_silent_wav` → lofty 打开写入 `Artist="周华健"`/`Title="朋友"`/`Album="试音"`（`Tag::new(TagType::RiffInfo)` 或 lofty 默认 tag，`save_to_path`）→ `extract_metadata` 读回断言三字段 + `duration_secs > 0` + `format=="WAV"`；
  - 不存在路径 → `Err(IndexError::Tag)`；
  - 非音频内容（写 `.mp3` 扩展名但内容是纯文本）→ `Err`。
  > 若 lofty 0.21 对 RiffInfo 写回字段读取有出入，改用 lofty 默认 primary tag 类型并相应断言；保持「写入什么读回什么」的往返性质。
- [ ] **Step 4: `tests/real_music.rs`（`#[ignore]`）** —— `index_dirs(&default_music_roots())` 跑通、`count()>0`、抽样 query 非空（无音乐目录则跳过断言）。仿 windows-search `real_*` 真机测试模式，CI 不跑。
- [ ] **Step 5: 跑测试确认通过**（含 WAV 往返）+ 全 crate 测试。
- [ ] **Step 6: fmt + clippy。**
- [ ] **Step 7: Commit** `feat(indexer): lofty 标签提取 + WAV 往返测试 + 真机 ignore 测试`

---

## Task 5: 台账 + README + 全套 CI + 文档收尾

**Files:** `docs/third-party-licenses.md`、`packages/indexer/README.md`（新）、`ROADMAP.md`、`STATUS.md`、`scripts/ci.sh`（确认 workspace 全量已覆盖新 crate，通常无需改）。

- [ ] **Step 1: 三方台账** —— 正式表追加 lofty / rusqlite / libsqlite3-sys / walkdir / fallible-iterator / hashbrown 等实际新增（`cargo tree -p locifind-indexer` 列出直接 + 关键间接依赖；版本以 Cargo.lock 为准）；把「SQLite / FTS5」「lofty 等价」从「预期清单」迁移；`tempfile` 标「否随产品分发（dev-only）」。
- [ ] **Step 2: `packages/indexer/README.md`** —— 职责（BETA-01 范围）、公共 API、SQLite schema、增量语义、扩展名白名单、known limitation（CJK 分词 / bundled 编译需 C 编译器 / 单线程 / 未接 Agent）。
- [ ] **Step 3: 全套 CI** `bash scripts/ci.sh 2>&1 | tail -20` —— fmt + clippy(-D warnings) + build + test 全过；确认既有 crate 零回归（indexer 是新增、不动既有代码，evals 472/26/2 不受影响，跑一次确认）。
- [ ] **Step 4: ROADMAP** BETA-01 状态 `not_started → done`，补验收实证一行。
- [ ] **Step 5: STATUS** 顶部当前阶段更新 + 会话日志追加 BETA-01 段（署名 Claude Code (Opus 4.8)）。
- [ ] **Step 6: 收工 commit**（中文，无 AI 自夸签名）+ 向用户确认提交内容。

---

## 验收对照（spec §5）

- 新 crate 编译 + workspace 接入（Task 1）。
- 存储/查询/增量/删除/FTS 转义单测，in-memory 确定性（Task 2/3）。
- lofty 标签提取 WAV 往返 + `#[ignore]` 真机测试（Task 4）。
- `bash scripts/ci.sh` 全套绿 + 既有零回归（Task 5 Step 3）。
- 三方台账 + README + ROADMAP/STATUS 同步（Task 5）。
