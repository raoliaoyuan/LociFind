# BETA-02 Office/PDF 内容索引 — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B1 BETA-02；计划书 §10.2；承接 [BETA-01 音乐索引](./2026-06-02-beta-01-music-metadata-index-design.md)（同 `packages/indexer` crate）
> ID：BETA-02

## 1. 背景与目标

承接 BETA-01（音乐 metadata 索引），做本地索引第二块：**Office / PDF / 纯文本文档的内容全文索引**。
系统搜索（Spotlight / Windows Search）对文档正文的覆盖不一致、不可控；自建 FTS5 全文索引让
「找包含『季度预算』的文档」「找张三写的 PPT」这类**基于正文 / 作者**的查询可用。

与 BETA-01 一致：**只做索引层 + 查询 API**，不接 Agent（融合留 BETA-04 / BETA-05）。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | 格式覆盖 | **现代 OOXML（docx/xlsx/pptx）+ pdf + txt/md/html + 旧版 xls**（calamine 原生带）；旧版二进制 doc/ppt（pre-2007）纯 Rust 难、**defer 为 known limitation** |
| ② | 索引粒度 | **每文档**：整篇正文连接进 FTS5 一条记录，另存页数/幻灯片总数；查询返回文档级命中 + 片段（FTS5 `snippet()`） |
| ③ | 范围 | 索引层 + 查询 API，不接 Agent（同 BETA-01） |
| ④ | 摘要字段 | **不做**（需 LLM，超索引层范围）；用 FTS5 `snippet()` 返回命中上下文片段替代 |

## 3. 技术选型（均 MIT，纯 Rust）

| 格式 | 库 | 说明 |
|---|---|---|
| xlsx / xls / ods | **calamine** 0.35 | 纯 Rust 电子表格读取，**原生支持旧版二进制 xls** → 顺带覆盖 ① 的旧 xls |
| pdf | **pdf-extract** 0.10 | 纯 Rust PDF 文本抽取（`extract_text`） |
| docx / pptx | **zip** 2 + **quick-xml** 0.40 | OOXML = ZIP+XML，自解析：docx 读 `word/document.xml` 取 `<w:t>`；pptx 读 `ppt/slides/slideN.xml` 取 `<a:t>`；`docProps/core.xml` 取 title/author。自解析比 dotext(0.1.1, 2018 未维护) 可靠可控 |
| html | quick-xml 0.40 | 收集 Text 事件、跳过 `script`/`style`；`<title>` 作标题 |
| md | **pulldown-cmark** 0.13 | 解析为纯文本事件流（剥语法），入 FTS |
| txt | std | 直接读 UTF-8（非法字节 lossy） |

> 三方台账登记上述 + 关键间接依赖；calamine/pdf-extract 从无到有。zip 用稳定 2.x（非 9.0.0-preN 预发布）。

## 4. 架构（扩展 `packages/indexer`）

新增与 [`MusicIndex`] 平行的 [`DocumentIndex`]，同 crate、可同 DB 文件（表命名空间隔离）。

### 4.1 数据模型

```rust
pub struct DocumentEntry {
    pub path: String,            // UNIQUE
    pub file_name: String,
    pub title: Option<String>,   // OOXML core.xml dc:title / html <title>，缺省 None
    pub author: Option<String>,  // OOXML core.xml dc:creator，缺省 None
    pub doc_type: String,        // "docx"/"xlsx"/"pptx"/"pdf"/"txt"/"md"/"html"/"xls"/"ods"
    pub page_count: Option<u32>, // pptx 幻灯片数 / xlsx 工作表数 / pdf 页数（best-effort）；docx/txt 等 None
    pub modified_time: i64,
}

/// 查询命中（含可选 FTS 片段）。
pub struct DocumentHit {
    pub entry: DocumentEntry,
    pub snippet: Option<String>, // FTS5 snippet()，仅文本查询时有
}
```

> 正文文本**只存进 FTS5**（不存主表），避免重复；返回片段用 FTS5 `snippet()`。

### 4.2 SQLite schema

```sql
CREATE TABLE IF NOT EXISTS documents (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  file_name     TEXT NOT NULL,
  title         TEXT,
  author        TEXT,
  doc_type      TEXT NOT NULL,
  page_count    INTEGER,
  modified_time INTEGER NOT NULL,
  indexed_time  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_modified ON documents(modified_time);
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
  title, author, body,
  tokenize='unicode61'
);
```

独立 FTS5 表（同 BETA-01 设计，rowid 对齐 `documents.id`，删除直接 `DELETE WHERE rowid`）。

### 4.3 公共 API

```rust
impl DocumentIndex {
    pub fn open(db_path: &Path) -> Result<Self, IndexError>;
    pub fn open_in_memory() -> Result<Self, IndexError>;
    pub fn index_dirs(&self, roots: &[PathBuf]) -> Result<IndexStats, IndexError>;
    pub fn query(&self, q: &DocumentQuery) -> Result<Vec<DocumentHit>, IndexError>;
    pub fn count(&self) -> Result<u64, IndexError>;
}

#[derive(Default)]
pub struct DocumentQuery {
    pub text: Option<String>,     // FTS：title/author/body
    pub author: Option<String>,   // 结构化子串
    pub doc_type: Option<String>, // 精确（大小写不敏感），如 "pdf"
    pub limit: Option<u32>,       // 默认 50
}
```

`IndexStats`、`default_music_roots` 复用 BETA-01；文档默认根目录用 `default_document_roots()`
（`dirs::document_dir()` + 可配置额外目录，同 BETA-01 多目录策略）。

### 4.4 增量复用（重构 BETA-01 的扫描骨架为泛型）

BETA-01 的 `index_dirs_with`（walkdir + mtime 比对 + 回收）逻辑与文档完全同构，仅「扩展名白名单
+ 提取器 + 存储表」不同。**抽出泛型** `IncrementalStore` trait 复用，避免重复：

```rust
pub(crate) trait IncrementalStore {
    type Entry;
    fn modified_time_of(&self, path: &str) -> Result<Option<i64>, IndexError>;
    fn upsert_entry(&self, e: &Self::Entry) -> Result<bool, IndexError>;
    fn paths_under(&self, roots: &[String]) -> Result<Vec<String>, IndexError>;
    fn delete_by_path(&self, path: &str) -> Result<bool, IndexError>;
}

/// 通用增量索引：walkdir + mtime + 回收。BETA-01 / BETA-02 共用。
pub(crate) fn run_incremental_index<S, F>(
    store: &S, roots: &[PathBuf], exts: &[&str], extract: F,
) -> Result<IndexStats, IndexError>
where S: IncrementalStore, F: Fn(&Path, i64) -> Result<S::Entry, IndexError>;
```

`MusicIndex` / `DocumentIndex` 各 impl `IncrementalStore`（其方法已存在，仅集中到 trait）。
`MusicIndex::index_dirs` 改为调 `run_incremental_index(self, roots, MUSIC_EXTS, extract_metadata)`。
**BETA-01 既有 22 测试守护不回归**（行为字节等价）。

### 4.5 文档提取（`doc_extract.rs`）

```rust
pub(crate) fn extract_document(path: &Path, modified_time: i64)
    -> Result<(DocumentEntry, String /*body*/), IndexError>;
```

按扩展名 dispatch：
- **docx**：zip 打开 → `word/document.xml` quick-xml 收集 `<w:t>` 文本（段落间补空格）；`docProps/core.xml` 取 `dc:title`/`dc:creator`；page_count None。
- **pptx**：zip → 枚举 `ppt/slides/slide*.xml` 收集 `<a:t>`；幻灯片数 = slide 文件数 → page_count；core.xml meta。
- **xlsx/xls/ods**：calamine 打开 → 遍历每 sheet 每 cell `to_string()` 连接；page_count = sheet 数。
- **pdf**：`pdf_extract::extract_text(path)`；page_count best-effort（v1 None，文档化；后续可加 lopdf 计页）。
- **html**：quick-xml 收集 Text 事件、跳过 `script`/`style`；`<title>` → title。
- **md**：pulldown-cmark `Parser` 收集 `Text`/`Code` 事件拼纯文本。
- **txt**：读 UTF-8（lossy）。
- body 文本统一 **cap 1 MiB 字符**（超大文档截断，防 FTS 巨行），截断计入 known limitation。
- 任一格式解析失败 → `Err(IndexError::Tag{..})`（沿用错误类型），由增量循环计 `failed` 跳过、不中断。

### 4.6 查询

- `text` → FTS5 `MATCH`（`fts_sanitize` 复用 BETA-01），join documents 取 metadata + `snippet(documents_fts, 2, '[', ']', '…', 10)`（body 列，命中片段）。
- `author` → 主表 `LIKE '%…%'`（ASCII ci）。
- `doc_type` → `= ? COLLATE NOCASE`。
- AND 组合；`limit` 默认 50；排序 `modified_time DESC`（文档场景近期优先）。无 `text` 时 `snippet=None`。

## 5. 验收 / 验证门

1. **`DocumentIndex` 编译 + API 落地**；`cargo build -p locifind-indexer` 通过。
2. **泛型重构零回归**：BETA-01 既有 22 测试全过（行为不变）。
3. **存储/查询单测（in-memory，确定性，直接喂 `DocumentEntry`+body）**：FTS 命中 title/author/body（含 CJK）；author 子串 / doc_type 精确 / limit；重 upsert 刷新 FTS；删除回收；snippet 非空（文本查询）/ None（结构化查询）；FTS 转义。
4. **提取单测（真实样本，测试内生成）**：
   - docx / pptx / xlsx：测试内用 zip/calamine 写最小样本（或 zip 手工构造 OOXML 部件）→ `extract_document` 断言正文含已知词 + doc_type + page_count；
   - md / html / txt：内联字符串写临时文件 → 断言剥语法后含正文词、不含标签/语法符；
   - pdf：若纯 Rust 生成最小 PDF 成本过高，用 `#[ignore]` 真机样本测试覆盖（仿 BETA-01 real_music 模式），CI 跑可生成的格式。
5. **增量单测**：复用泛型路径，文档扩展名白名单命中正确（doc 扩展名计入、音频/图片不计）；added/skipped/updated/removed/failed 计数正确（stub 提取器）。
6. **`#[ignore]` 真机集成**：`index_dirs(&default_document_roots())` 跑通、`count()>0`、抽样 query 返回非空 + 片段。
7. **`bash scripts/ci.sh` 全套绿**（platform-macos 在 Windows 的预存失败除外）；indexer fmt + clippy `-D warnings`；全 workspace test 零回归（evals 472/26/2、harness 等不沾）。
8. **三方台账 + README + ROADMAP/STATUS** 同步。

## 6. 非目标（YAGNI）

- 不做旧版二进制 doc/ppt（pre-2007）。
- 不做每页/每幻灯片粒度（每文档级，存总数即可）。
- 不做 OCR（BETA-03）/ 摘要（需 LLM）/ 向量（BETA-11B）。
- 不接 Agent / SearchBackend（BETA-04）。
- 不做并发提取（单线程；大库性能优化按需）。
- pdf page_count 精确计数（v1 None，后续按需加 lopdf）。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 泛型重构动 BETA-01 | trait 方法是既有方法的集中；BETA-01 22 测试守护字节等价；先重构跑测试再加文档逻辑 |
| OOXML 各生产者 XML 结构差异 | 只抽 `<w:t>`/`<a:t>` 文本叶子（最稳的公共子集），不解析样式/结构；失败计 failed 不崩 |
| pdf-extract 对扫描件/加密 PDF 返回空或报错 | 空文本 → body 空、仍入索引（title/路径可搜）；报错 → failed 跳过；扫描件正文留 BETA-03 OCR |
| 超大文档撑爆 FTS | body cap 1 MiB 字符 |
| 提取库引入较多间接依赖 / 编译变重 | 均 MIT 纯 Rust；台账登记；首次编译时长记 README |
| calamine/quick-xml/zip API 版本差异 | 实现期以编译为准，pin 解析到的版本回填 Cargo.lock |
