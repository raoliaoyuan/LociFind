# BETA-01A 全盘音频索引 — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B1 BETA-01A；承接 [BETA-01 音乐索引](./2026-06-02-beta-01-music-metadata-index-design.md) + [spike 报告](../../reviews/spike-disk-wide-audio.md)
> ID：BETA-01A

## 1. 背景与目标

现状 `reindex` 仅扫固定 `dirs::audio_dir()`（`~/Music`）。用户音频散落 OneDrive/下载等处 →
默认 Music 目录空 → 扫 0 条。spike（真机 1249 文件）已验证「发现/提取/存储三层拆分」可行、
搜索跨目录命中 ✅，但暴露两大坑（OneDrive 占位符 24% 失败+触发下载；标签覆盖仅 ~21%）。

BETA-01A 把 spike 产品化：**reindex 覆盖全盘音频，跳过仅在线占位符，并行提取，文件名可搜**。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | 跨平台范围 | **双平台发现**：Windows Everything `es.exe` + macOS Spotlight `mdfind`（`AudioDiscovery` trait + 两 impl）。占位符跳过 Windows 完整、macOS iCloud dataless best-effort（无安全 std API，留后续） |
| ② | 发现不可用 | **优雅回退到目录扫描**：发现层是可选加速（守 PROJECT「不强制依赖 Everything」），工具不在 → 回退 `default_music_roots()` 递归扫；reindex 不报错 |

## 3. 关键约束 / 架构决策

- **占位符检测无 unsafe**：Windows `std::os::windows::fs::MetadataExt::file_attributes()` 读
  `FILE_ATTRIBUTE_OFFLINE`(0x1000) / `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`(0x400000)——只读属性
  **不触发内容水合**，守 workspace `unsafe_code = forbid`，零新平台依赖。macOS dataless（`SF_DATALESS`）
  无安全 std API → `is_online_only` 返 false（best-effort，文档化）。
- **并行分层**（rusqlite `Connection: !Sync`）：① 顺序预检（ext / fs mtime / 占位符属性 / DB
  `modified_time_of`，全是 metadata 读不触发下载）→ 分类 skip / 占位符（仅文件名）/ 待提取；
  ② **rayon 并行** lofty 提取待提取项（纯 CPU+IO，无 DB）→ `MusicEntry`；③ 顺序 upsert（DB 写）。
- **占位符 → 仅文件名入库**：online-only 文件不读 lofty（避失败+避下载），存 `MusicEntry{ path,
  file_name, modified_time, 其余 None }` → 仍按文件名可搜（依赖 file_name 进 FTS）。
- **file_name 进 FTS** + 迁移：`music_fts` 从 (artist,title,album) → (artist,title,album,file_name)；
  旧 db 检测缺列则 drop+recreate，**从 music 主表重填**（不重读文件，秒级）。
- 新依赖 **rayon**（MIT OR Apache-2.0，纯 Rust）。

## 4. 架构（扩展 `packages/indexer` + `local-index`）

### 4.1 发现层（`packages/indexer/src/discovery.rs`，新）

```rust
#[derive(Debug)]
pub enum DiscoveryError { Unavailable { detail: String }, Failed { detail: String } }

/// 全盘音频路径发现（仅枚举路径，不读内容）。
pub trait AudioDiscovery: std::fmt::Debug + Send + Sync {
    fn discover_audio(&self) -> Result<Vec<PathBuf>, DiscoveryError>;
}

/// 平台默认发现器；工具不可用返回 None（调用方回退目录扫描）。
pub fn default_audio_discovery() -> Option<Box<dyn AudioDiscovery>>;
```

- **`EverythingDiscovery`**（`cfg(windows)`）：spawn `es.exe ext:mp3;flac;m4a;aac;ogg;opus;wav;wma;aiff;aif;ape`
  `-export-txt <tmp> -utf8-bom`（规避 GBK stdout 破坏 CJK 路径，spike 已验），读 tmp 去 BOM 按行解析。
  `es.exe` 解析：PATH 优先，fallback winget 安装路径（同 spike）。spawn 失败 → `Unavailable`。
- **`SpotlightDiscovery`**（`cfg(target_os="macos")`）：`mdfind 'kMDItemContentTypeTree == "public.audio"'`，
  stdout 按行（UTF-8）解析为路径。`mdfind` 失败 → `Unavailable`。
- `default_audio_discovery`：Windows 返回 `EverythingDiscovery`（若 es.exe 可定位）、macOS 返回
  `SpotlightDiscovery`，否则 `None`。

### 4.2 占位符检测（`packages/indexer/src/placeholder.rs`，新，平台 gated）

```rust
/// 文件是否为"仅在线"云占位符（读内容会触发下载/被拒）。
/// Windows：查 FILE_ATTRIBUTE_OFFLINE | RECALL_ON_DATA_ACCESS（无 unsafe）。
/// 其它平台：best-effort 返 false（macOS dataless 无安全 std API，留后续）。
pub(crate) fn is_online_only(path: &Path) -> bool;
```

### 4.3 `index_paths` 重构（并行 + 占位符）

```rust
impl MusicIndex {
    /// 索引显式路径列表（发现层用，不递归不回收）。
    /// 顺序预检 → rayon 并行 lofty 提取 → 顺序 upsert。
    /// 仅在线占位符不读标签、只存文件名（计 `skipped`+标记，仍按名可搜）。
    pub fn index_paths(&self, paths: &[PathBuf]) -> Result<IndexStats, IndexError>;
}
```

- 预检（顺序）：非音乐扩展名跳过；取 fs mtime（失败 `failed`）；`modified_time_of` 相等 `skipped`；
  否则 `is_online_only`？→ 占位符项（仅文件名 `MusicEntry`，计入新增/更新但标记来源）；否则待提取项。
- 并行（rayon `par_iter`）：待提取项 → `extract_metadata` → `Ok(entry)` / `Err`（计 `failed`）。
- 顺序：占位符项 + 提取成功项逐个 upsert（`added`/`updated`）。
- 统计：占位符计入正常 added/updated（它们是有效记录，只是无标签）；`failed` 仅真提取失败。

> `IndexStats` 复用；占位符不单列计数（v1 简化，spike 报告的"避失败"目标达成——占位符不再计 failed）。

### 4.4 `music_fts` + file_name

- SCHEMA：`music_fts(artist, title, album, file_name, tokenize='trigram')`。
- upsert FTS 同步加 `file_name`。
- `query` 的 `text` FTS MATCH 现也命中 file_name（无需改 query SQL，列已含）。
- 迁移（`from_conn`）：`PRAGMA table_info(music_fts)` 无 `file_name` 列 → `DROP TABLE music_fts` +
  按新 schema 重建 + `INSERT INTO music_fts(rowid,...) SELECT id,artist,title,album,file_name FROM music`。

### 4.5 `LocalIndexBackend::reindex` 发现优先

```rust
pub fn reindex(&self, music_roots, doc_roots) -> Result<(IndexStats, IndexStats), SearchError> {
    let music = MusicIndex::open(&db)?;
    let music_stats = match default_audio_discovery() {
        Some(disc) => match disc.discover_audio() {
            Ok(paths) => music.index_paths(&paths)?,        // 全盘发现
            Err(_)    => music.index_dirs(music_roots)?,    // 发现失败 → 回退
        },
        None => music.index_dirs(music_roots)?,             // 无发现器 → 回退
    };
    // 文档不变（doc_roots 递归扫）
    let docs = DocumentIndex::open(&db)?;
    let doc_stats = docs.index_dirs(doc_roots)?;
    Ok((music_stats, doc_stats))
}
```

desktop `reindex` 命令签名不变（仍传 default roots），发现逻辑在 backend 内。`IndexStats` JSON 已含
scanned/added/updated/skipped——UI 提示自动反映全盘结果。

## 5. 验收 / 验证门

1. **discovery**：`AudioDiscovery` trait + Everything/Spotlight impl 编译；`default_audio_discovery`
   平台返回正确类型 / 工具不可用返回 None。EverythingDiscovery 真机 `#[ignore]` 测试（es.exe 枚举非空）。
2. **placeholder**：`is_online_only` 非 Windows 返 false（单测）；Windows 普通文件返 false（真机普通文件）；
   纯逻辑（属性位判定）可单测（构造 attr bitmask 的纯函数 `attrs_indicate_online_only(u32)`）。
3. **index_paths**：in-memory + 真 txt/WAV——并行提取多文件命中；占位符（mock/真机）只存文件名仍按名搜；
   非音乐扩展名跳过；mtime 未变 skip；提取失败计 failed 不中断。**并行结果与顺序一致**（确定性）。
4. **file_name FTS**：upsert 后按文件名子串（≥3 字符）FTS 命中（含 CJK）；**迁移**：旧 3 列 db 打开后
   自动升级 4 列且从 music 重填、按文件名可搜（单测：手建 3 列 fts + music 数据 → open → 查文件名命中）。
5. **reindex**：`LocalIndexBackend::reindex` 发现优先 + 回退（mock discovery 注入或 None 路径单测）；
   既有 local-index 测试不回归。
6. **零回归**：BETA-01 既有音乐测试（trigram 等）+ 全 workspace test 全过（除 platform-macos 预存
   Windows 失败）；fmt + clippy `-D warnings`。三方台账加 rayon。
7. **真机手测**（用户驱动）：Windows 设置页「立即索引」→ 全盘音频入库（OneDrive 占位符跳过、不触发下载）
   → 跨目录 artist/文件名搜命中。落 manual-test-scenarios。
8. **文档**：discovery README 段 + ROADMAP BETA-01A done + STATUS。

## 6. 非目标（YAGNI）

- 不做删除回收（discovery 每次全量，stale 记录留 BETA-07 调度处理）。
- 不做 macOS iCloud dataless 检测（无安全 std API；best-effort false，留后续）。
- 不把 file_name FTS 推广到 documents（spike 建议留 BETA-02 后续）。
- 不做后台自动调度（BETA-07）；仍显式 reindex。
- 不做占位符单独计数列（IndexStats 不扩字段，占位符计入 added/updated）。
- 不复用 everything backend crate 的 es.exe 执行器（discovery 自含最小 spawn，避跨 crate 依赖）。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 读 file_attributes 是否触发下载 | 不会——属性是 metadata，仅读内容才水合；spike 实测 lofty（读内容）才触发 |
| rayon 并行 DB 竞争 | DB 读写全在顺序阶段；并行阶段只 lofty（无 DB），无竞争 |
| FTS 迁移丢数据 | 从 music 主表重填（music 有 file_name 列），不重读文件；迁移失败兜底重建空 fts（下次 reindex 重填） |
| 发现器跨平台差异 | trait 抽象 + 工具不可用回退目录扫描，保证 reindex 永不报错 |
| es.exe 路径定位失败 | PATH + winget fallback；都失败 → Unavailable → 回退目录扫描 |
| 全盘发现误纳系统/缓存音频 | v1 接受（Everything ext: 枚举全盘）；后续可加排除目录（留 BETA-07/设置） |
