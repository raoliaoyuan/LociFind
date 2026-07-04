# BETA-01 音乐 metadata 索引 — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B1 BETA-01；计划书 §10.1 音乐 metadata；PROJECT.md「索引存储：SQLite + FTS5」
> ID：BETA-01

## 1. 背景与目标

Beta 阶段第一块本地索引。当前搜索只覆盖文件名 + 系统索引 metadata（Spotlight / Windows Search / Everything），无法回答「找一首周华健的歌」「找周华健的朋友」这类基于**音频内嵌标签**（artist / title / album）的查询——系统索引对音频 tag 的覆盖不一致且不可控。

BETA-01 目标：**自建一个轻量的音乐 metadata 索引层**——扫描音乐目录、用纯 Rust 库提取音频标签、存入跨平台 SQLite/FTS5、对外暴露查询 API。

本任务**只做索引层 + 查询 API**（brainstorming 决策①）。接入 Agent / SearchBackend / 与系统搜索结果融合，留给 BETA-04 Result Normalizer + BETA-05 Ranker（ROADMAP 依赖图：BETA-04 依赖 BETA-01/02/03）。后台调度留 BETA-07。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 | 影响 |
|---|---|---|---|
| ① | 交付范围 | **索引层 + 查询 API**（不接 Agent / 不包 SearchBackend） | 范围清晰，与 ROADMAP 依赖图一致；查询接口形态为 BETA-04 预留 |
| ② | 索引策略 | **mtime 增量**（path+mtime 比对，跳过未变文件） | 重读标签昂贵，增量从一开始就避免重复全盘扫描；BETA-07 复用 |
| ③ | 扫描范围 | **系统音乐目录默认 + 可配置额外目录** | `index_dirs(roots)` 接受任意目录列表；提供 `default_music_roots()` 取系统音乐目录 |

## 3. 技术选型

| 组件 | 选择 | License | 理由 |
|---|---|---|---|
| 标签提取 | **lofty** | MIT OR Apache-2.0 | 纯 Rust，统一 API 覆盖 ID3v2(mp3/wav)/Vorbis(flac/ogg/opus)/MP4(m4a/aac)/APE 等；同时提供 `properties()`（duration / audio_bitrate / format）。计划书列的 mutagen/TagLib#/music-metadata 均为他语言，lofty 是 Rust 等价物 |
| 存储 | **rusqlite（`bundled` feature）** | MIT | 同进程内嵌 SQLite，`bundled` 自带源码编译并启用 FTS5；同步 API 适合索引器；PROJECT.md 既定「SQLite + FTS5」。`unsafe` 收敛在依赖内（同 dirs 模式），本 crate 维持 `unsafe_code = forbid` |
| 目录遍历 | **walkdir** | MIT OR Apache-2.0 | 递归遍历 + 符号链接控制 + 错误不中断；比手写 fs 递归稳健 |
| 默认音乐目录 | **dirs::audio_dir()** | MIT OR Apache-2.0 | 已在 workspace 依赖树（MVP-13）；跨平台返回系统 Music 目录，避免 indexer 耦合 search-backend 的 `LocationResolver` trait |

> 编译前置：rusqlite `bundled` 需 C 编译器。项目已因 llama.cpp 要求 macOS clang / Windows MSVC，无新增系统前置。三方台账（docs/third-party-licenses.md）同步登记 lofty / rusqlite / walkdir（+ 间接依赖）。

## 4. 架构

新建 crate `packages/indexer`（package 名 `locifind-indexer`），加入 workspace members。

### 4.1 数据模型

```rust
/// 一条音乐索引记录（计划书 §10.1 字段）。
pub struct MusicEntry {
    pub path: String,           // 绝对路径，UNIQUE 键
    pub file_name: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub format: Option<String>, // 容器格式，如 "MP3" / "FLAC" / "MP4"
    pub bitrate: Option<u32>,   // 音频码率 kbps
    pub modified_time: i64,     // 文件 mtime（unix 秒），增量比对锚点
}
```

### 4.2 SQLite schema

```sql
CREATE TABLE IF NOT EXISTS music (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  file_name     TEXT NOT NULL,
  artist        TEXT,
  title         TEXT,
  album         TEXT,
  duration_secs REAL,
  format        TEXT,
  bitrate       INTEGER,
  modified_time INTEGER NOT NULL,
  indexed_time  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_music_modified ON music(modified_time);

-- 全文索引：artist/title/album（external content，rowid 对齐 music.id）
CREATE VIRTUAL TABLE IF NOT EXISTS music_fts USING fts5(
  artist, title, album,
  content='music', content_rowid='id',
  tokenize='unicode61'
);
```

FTS 与主表同事务手动同步（v1 不用 trigger，逻辑显式可读）：
- upsert：`INSERT … ON CONFLICT(path) DO UPDATE` 写 music；拿到 rowid 后 `DELETE FROM music_fts WHERE rowid=?` + `INSERT INTO music_fts(rowid,artist,title,album)`。
- delete：先删 music_fts 行再删 music 行。

> `unicode61` tokenizer 对 CJK 按码点切分，能命中「周华健」整串（artist 字段存整名，查询用 `artist:周华健*` 前缀或子串）。中文分词非本任务目标（B 阶段后续 / 向量检索 BETA-11B 范畴）。

### 4.3 公共 API

```rust
pub struct MusicIndex { /* conn */ }

impl MusicIndex {
    /// 打开（或创建）索引数据库，建表。
    pub fn open(db_path: &Path) -> Result<Self, IndexError>;
    /// 内存库（测试用）。
    pub fn open_in_memory() -> Result<Self, IndexError>;

    /// 增量索引给定根目录（递归）。跳过 mtime 未变的文件；
    /// 删除 roots 范围内已不存在于磁盘的记录。返回统计。
    pub fn index_dirs(&mut self, roots: &[PathBuf]) -> Result<IndexStats, IndexError>;

    /// 查询：FTS 文本 + 结构化过滤。
    pub fn query(&self, q: &MusicQuery) -> Result<Vec<MusicEntry>, IndexError>;

    /// 记录总数（诊断 / 测试）。
    pub fn count(&self) -> Result<u64, IndexError>;
}

/// 默认音乐根目录（系统 Music 目录，经 dirs::audio_dir()）。空表示无法确定。
pub fn default_music_roots() -> Vec<PathBuf>;

pub struct IndexStats {
    pub scanned: usize,   // 命中音乐扩展名的文件数
    pub added: usize,
    pub updated: usize,
    pub skipped: usize,   // mtime 未变
    pub removed: usize,   // 磁盘已删 → 索引删
    pub failed: usize,    // 标签读取失败（不中断）
}

#[derive(Default)]
pub struct MusicQuery {
    pub text: Option<String>,        // FTS：artist/title/album 任一匹配（前缀）
    pub artist: Option<String>,      // 结构化：artist 子串（大小写不敏感）
    pub album: Option<String>,
    pub format: Option<String>,      // 精确（大小写不敏感），如 "FLAC"
    pub limit: Option<u32>,          // 默认 50
}
```

`MusicQuery` 各字段 AND 组合；全空表示「全部」（受 limit）。`text` 经 FTS5 MATCH（输入做转义 + 加 `*` 前缀，防注入与语法错误）；`artist`/`album`/`format` 走主表 `LIKE`/`=`。

### 4.4 标签提取

```rust
/// 从单个音频文件提取 metadata。
fn extract_metadata(path: &Path, modified_time: i64) -> Result<MusicEntry, IndexError>;
```

- `lofty::read_from_path(path)` → `TaggedFile`。
- artist/title/album：`primary_tag()`（无则 `first_tag()`）的 `Artist` / `Title` / `Album` item。
- duration：`properties().duration().as_secs_f64()`。
- bitrate：`properties().audio_bitrate()`。
- format：`TaggedFile::file_type()` 映射为短名（MP3/FLAC/MP4/Vorbis/…）。
- 读取失败（非音频 / 损坏）→ `Err`，由 `index_dirs` 记 `failed += 1` 跳过，不中断整轮。

音乐扩展名白名单（大小写不敏感）：`mp3 flac m4a aac ogg opus wav wma aiff aif ape`。walkdir 仅对白名单内文件调 lofty（避免对整盘非音频文件无谓 IO）。

### 4.5 增量逻辑

`index_dirs`：
1. walkdir 遍历每个 root（跟随目录、不跟随 symlink、忽略遍历错误项）。
2. 对每个白名单扩展名文件：取 `fs::metadata().modified()` → unix 秒。
3. 查 DB 该 path 的 `modified_time`：相等 → `skipped`；否则 `extract_metadata` + upsert（新增 `added` / 已存在 `updated`）。
4. 收集本轮见到的 path 集合；遍历完后，删除 **DB 中 path 落在任一 root 子树下、但本轮未见到**的记录（`removed`）——即磁盘已删除的文件。root 外的记录不动。

## 5. 验收 / 验证门

1. **新增 crate 编译 + 接入 workspace**：`cargo build -p locifind-indexer` 通过；根 `Cargo.toml` members 含 `packages/indexer`。
2. **单元测试（in-memory，确定性，不依赖真实音频）**——直接对内部 upsert/query 喂 `MusicEntry`：
   - FTS 文本检索命中 artist/title/album（含 CJK「周华健」）；
   - 结构化过滤 artist/album 子串、format 精确、limit 截断；
   - 增量：同 path 同 mtime → skipped；mtime 变 → updated 且字段刷新；
   - 删除：磁盘文件消失 → removed；root 外记录不受影响；
   - FTS 输入转义（含 `"`、`*`、`OR` 等 FTS 语法字符）不报错不注入。
3. **标签提取测试（lofty 真往返，无外部二进制）**：测试内纯 Rust 生成最小合法 WAV（RIFF 头 + 静音 PCM），用 lofty 写入 artist/title/album（RIFF INFO / ID3 tag），`extract_metadata` 读回断言字段 + duration > 0 + format = "WAV"。
4. **真实库集成测试（`#[ignore]`，仿 Windows backend 真机测试模式）**：`index_dirs(default_music_roots())` 在有音乐的机器上跑通、`count() > 0`、抽样 `query` 返回非空——CI 不跑，开发者真机手动验。
5. **`bash scripts/ci.sh` 全套绿**（fmt + clippy `-D warnings` + build + test）。clippy 对 indexer 同样 `unwrap_used`/`expect_used`/`panic` = warn（生产路径不得触发）。
6. **三方台账**：docs/third-party-licenses.md 登记 lofty / rusqlite / libsqlite3-sys / walkdir / fallible-iterator 等实际引入项（版本以 Cargo.lock 为准），并将 SQLite/FTS5 从「预期清单」迁入正式表。
7. **文档同步**：`packages/indexer/README.md` 落库（职责 / API / schema / 增量语义 / known limitation）；ROADMAP BETA-01 状态置 done + STATUS 收工日志。

## 6. 非目标（YAGNI）

- 不接 Agent / 不实现 SearchBackend（BETA-04/05）。
- 不做后台调度 / 文件系统监听（BETA-07）。
- 不做中文分词 / 同义词 / 模糊匹配（向量检索 BETA-11B）。
- 不做 Office/PDF（BETA-02）、OCR（BETA-03）。
- 不做并发索引（单线程顺序；性能优化按需，音乐库通常千级文件量级）。
- 不做封面图 / 歌词 / 流派等扩展字段（先锁计划书 §10.1 九字段）。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| rusqlite `bundled` 在 Windows 首次编译需 MSVC + 可能拉长编译 | 项目已有 MSVC 前置（llama.cpp）；首次编译时长可接受，记入 README 备注 |
| lofty 对个别冷门容器返回部分字段缺失 | 字段全 `Option`，缺失存 NULL，不阻断；失败计入 `failed` |
| FTS5 注入 / 语法字符 | `text` 输入统一转义并加前缀通配；专项单测覆盖 |
| 相对/本地时区与 mtime 锚点 | 本任务只存 mtime 原值（unix 秒），不做时间语义解析，无时区歧义 |
| CJK FTS 召回（unicode61 按码点切） | 满足「artist 整名查询」；中文分词为后续向量检索范畴，记 known limitation |
| 跨平台路径大小写 / 分隔符 | path 以 OS 原生绝对路径字符串存储；查询用 path 精确比对，不跨平台共享 DB |
