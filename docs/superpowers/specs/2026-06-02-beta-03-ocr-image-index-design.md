# BETA-03 图片 OCR 内容索引 — 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B1 BETA-03；承接 BETA-02（DocumentIndex / FTS5）+ BETA-04（LocalIndexBackend 的 `MediaSearch(Image) → 空（留 BETA-03 OCR）` 分支）
> ID：BETA-03

## 1. 背景与目标

本地索引拼图缺最后一块：**图片里的文字搜不到**。截图、扫描件、含字海报当前完全不可检索。
BETA-04 的 `LocalIndexBackend::search_results` 在 `MediaSearch(Image)` 分支显式返回空，注释「留 BETA-03 OCR」。

BETA-03：**对图片做 OCR、把识别出的文字存进现有 FTS、让「找含某词的截图/图片」端到端命中**。

**Windows 先行**（你当前在 Windows，B 阶段一贯 Windows-first）；引擎 trait 抽象好，macOS Vision 后续插入（镜像 BETA-01A：Windows 完整 / macOS best-effort）。

### Spike 去风险（2026-06-02 Windows 11 真机）

整个设计押在「PowerShell 5.1 能否调通 Windows.Media.Ocr WinRT、识别中文」。已用最小 spike 验证 **通过**：

- `[Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages` 本机含 `zh-Hans-CN`；
- `AsTask` await 辅助 + `BitmapDecoder.CreateAsync → GetSoftwareBitmapAsync → OcrEngine.RecognizeAsync` 全链路跑通；
- 测试图「会议纪要 2024年第三季度 / Invoice Total: 8800 RMB」→ 识别出「会 议 纪 要 2024 年 第 三 季 度 lnvoice Total: 8800 RMB」（中文全对；英文 `Invoice→lnvoice` 是 OCR 固有小瑕疵）。
- **暴露一个必做归一化**：Windows OCR 在 **CJK 字符间插空格**（`会 议 纪 要`），不折叠会破坏 trigram FTS 对「会议」的匹配。

零安装、零 unsafe、复刻 WindowsSearch 的 ADODB / Everything 的 es.exe **shell-out 拿结构化输出**套路。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | OCR 引擎策略 | **原生优先 + Tesseract 兜底**：Windows 走 PowerShell + Windows.Media.Ocr WinRT；不可用回退 shell-out `tesseract`；macOS Vision 留后续 |
| ② | 平台范围 | **Windows 先行，trait 留 macOS**（`OcrEngine` trait + `WindowsOcrEngine` / `TesseractOcrEngine`，`MacosVisionOcr` 后续） |
| ③ | 存储 | **复用 `DocumentIndex`**：图片当一种 doc_type（png/jpg…），OCR 文字当 body 进现有 `documents_fts` |
| ④（spec 定） | 检索路由 | `MediaSearch(Image/Screenshot)` **带 keyword** → 查 image doc_types；无 keyword → 空（交系统后端按文件名/类型搜）。`FileSearch` 内容查询天然也覆盖图片（同一 FTS，无需改） |
| ⑤（spec 定） | 性能 | v1 **逐文件 OCR**走现有 `run_incremental_index`（mtime 增量 skip → 重跑秒级；首跑慢但 BETA-07 后台非阻塞）。批量 / 并行 OCR 留后续 |
| ⑥（spec 定） | 脚本分发 | OCR `.ps1` 用 `include_str!` 嵌进 indexer 二进制（免运行期定位文件，规避 synonym 词典曾踩的 dev/.app 路径坑） |

## 3. 架构

### 3.1 OCR 引擎抽象（`packages/indexer/src/ocr.rs`，新建）

```rust
/// 单图 OCR 引擎。跨平台 + 跨实现（Windows WinRT / Tesseract / 后续 macOS Vision）。
pub trait OcrEngine: Send + Sync + std::fmt::Debug {
    /// 识别单张图片的全部文字（已做 CJK 空格折叠）。
    /// 失败（解码错 / 引擎错 / 超时）返回 Err → 增量循环计 failed、跳过、不中断整轮。
    fn recognize(&self, image: &Path) -> Result<String, IndexError>;
    /// 引擎名（trace / 诊断用）。
    fn name(&self) -> &str;
}

/// 选默认引擎：Windows.Media.Ocr 可用 → WindowsOcrEngine；
/// 否则 PATH 上有 tesseract → TesseractOcrEngine；都没有 → None（优雅跳过图片索引）。
pub fn default_ocr_engine() -> Option<Box<dyn OcrEngine>>;
```

**`WindowsOcrEngine`**（仅 `#[cfg(windows)]`）：
- 构造时一次性探测 `AvailableRecognizerLanguages` 非空（经一次轻量 PowerShell 调用）→ 决定 `default_ocr_engine` 是否选它。
- `recognize`：`spawn` `powershell -NoProfile -NonInteractive -File <临时落地的内嵌 .ps1>`，**图片路径经环境变量 `LOCIFIND_OCR_IMAGE` 传入**（脚本不插值用户数据 → 杜绝注入，照搬 ADODB 套路），脚本 stdout 打印识别文本（UTF-8）。同步 spawn + 超时 kill（照搬 spotlight/windows-search 执行器）。
- 脚本 = spike 验证过的那段（`AsTask` await + `BitmapDecoder → SoftwareBitmap → RecognizeAsync`），`include_str!("ocr/win_ocr.ps1")` 内嵌，运行期写入临时文件再调用。

**`TesseractOcrEngine`**（跨平台兜底）：
- 构造时探测 `tesseract --version` 在 PATH。
- `recognize`：`tesseract <image> stdout -l chi_sim+eng`（结构化参数、超时 kill），stdout 即文本。语言数据缺失 → Err（计 failed）。

**CJK 空格折叠** `normalize_ocr_text(&str) -> String`（纯函数，两引擎共用）：相邻两个 CJK 表意字符之间的空白丢弃；拉丁词间空格保留。单测覆盖。

### 3.2 图片索引入口（`packages/indexer`）

图片扩展名白名单：
```rust
const IMAGE_EXTS: &[&str] = &["png","jpg","jpeg","bmp","tif","tiff","gif","webp","heic"];
```

`DocumentIndex` 新增（复用现有表 + `IncrementalStore` 骨架）：
```rust
impl DocumentIndex {
    /// 增量 OCR 索引图片目录（递归 + mtime skip + 回收）。
    /// 每图：OCR → 文字归一 → DocumentEntry{ doc_type=扩展名, title=None, author=None, body=文字 }。
    /// OCR 失败计 failed（坏图 / 引擎错），不中断整轮。
    pub fn index_image_dirs(
        &self,
        roots: &[PathBuf],
        ocr: &dyn OcrEngine,
    ) -> Result<IndexStats, IndexError>;
}
```
实现：`run_incremental_index(self, roots, IMAGE_EXTS, |path, mtime| { let text = ocr.recognize(path)?; Ok((image_entry(path, mtime), normalize_ocr_text(&text))) })`。
（`run_incremental_index` 已含 mtime skip + `catch_unwind` panic 兜底 + 回收。）

**回收按扩展名收窄**（修一个共享表潜在 bug）：现 `run_incremental_index` 回收「roots 下本轮未见」的所有记录——若图片与文档**同根目录**，文档轮会把图片误回收（反之亦然）。改为回收时额外要求**记录扩展名 ∈ 本轮 `exts`**：
```rust
// 回收循环内：
if !seen.contains(&p) && has_ext(Path::new(&p), exts) && store.delete_by_path(&p)? { ... }
```
此改动对既有音乐/文档（各跑各的扩展名）严格更安全，既有测试（删 mp3 → mp3 ∈ MUSIC_EXTS）不变。

`default_image_roots()`：`dirs::picture_dir()`（含截图子目录递归覆盖）。无法确定返回空。

### 3.3 检索路由（`packages/search-backends/local-index`）

`DocumentQuery` 加可选 image-type 过滤（默认 None = 全类型，向后兼容）：
```rust
pub struct DocumentQuery {
    // …现有字段…
    /// 仅返回 doc_type ∈ 此集合的记录（None = 不限）。MediaSearch(Image) 用它框定图片。
    pub doc_types: Option<Vec<String>>,
}
```
`DocumentIndex::query` 的 filters 加 `(:has_types = 0 OR d.doc_type IN (…))`（或在 Rust 端按集合过滤，二选一，spec 倾向 SQL `IN` 动态构造 + 参数绑定）。

`LocalIndexBackend::search_results` 的 `MediaSearch(Image)` 分支（现返回空）改为：
```rust
SearchIntent::MediaSearch(m) if matches!(m.media_type, MediaType::Image | MediaType::Screenshot) => {
    match build_image_query(m) {           // 有 keyword → Some；无 → None
        None => Ok(Vec::new()),            // 无 keyword 交系统后端按文件名/类型搜
        Some(q) => { /* DocumentIndex.query(q) → doc_hit_to_result */ }
    }
}
```
`build_image_query`：keyword 非空 → `DocumentQuery{ text, doc_types: Some(IMAGE_DOC_TYPES), limit }`。
`FileSearch` 内容查询**无需改**——同一 FTS，关键词搜文档时天然也会命中含该词的图片（理想行为：「找关于 X 的东西」连截图一起浮出）。

`reindex_with`：文档 `index_dirs` 之后，若 `default_ocr_engine()` 返回 `Some` → 追加 `docs.index_image_dirs(image_roots, &*engine)`；返回值扩展为 `(music, doc, image)` 三组 `IndexStats`（或 image 并入 doc 统计 + 单独 count，spec 定为三元组，调用方按需取）。引擎 `None`（无 OCR 能力）→ 跳过图片，不报错。

### 3.4 桌面接线（`apps/desktop`）

- `reindex` / `perform_reindex`（BETA-07）传入 `default_image_roots()`，图片计数并入 `IndexStatus.last_summary`（「音乐 N / 文档 M / 图片 K」）。
- 无 UI 新组件（搜索结果已有图片会经现有列表呈现）；图片 OCR 命中复用文档 `snippet` 显示。

## 4. 验收 / 验证门

1. **`normalize_ocr_text` 单测**：`会 议 纪 要` → `会议纪要`；`Hello World` 不变；中英混排 `图 片 abc 文 字` → `图片 abc 文字`（CJK 间折叠、拉丁词间保留）。
2. **回收扩展名收窄单测**（scan.rs）：同根目录下 1 图 + 1 txt，跑文档轮（DOC_EXTS）→ 图片**不被回收**；跑图片轮（IMAGE_EXTS）→ txt 不被回收。既有删除回收测试不回归。
3. **`index_image_dirs` 端到端**（用 **stub `OcrEngine`** 隔离真 OCR，返回固定文字）：扫描计数只数图片扩展名；OCR 文字进 FTS 可被 `query` 命中；mtime 未变跳过；删图回收；stub 返回 Err → 计 failed 不中断。
4. **`build_image_query` + 路由单测**（local-index）：MediaSearch(Image) 带 keyword → 命中图片记录、doc_type 框定只返图片不返 docx；无 keyword → 空。
5. **`DocumentQuery.doc_types` 过滤单测**（doc_db）：插 png + docx，`doc_types=Some([png…])` 只返 png。
6. **真 OCR 集成测试**（`tests/real_ocr.rs`，`#[ignore]`，仅 Windows 真机）：进程内用 System.Drawing 不便 → 测试随附一张含已知文字的小 PNG fixture，`WindowsOcrEngine::recognize` 返回含该文字（容忍 OCR 噪声，断言关键子串）。CI 不跑（无 OCR 语言包环境跳过）。
7. **`default_ocr_engine` 探测单测**：可控 stub 验证「Windows 可用→选 WinRT / 否则 tesseract / 都无→None」分支（探测函数注入 mock，不真调系统）。
8. **零回归**：indexer / local-index / desktop / 全 workspace test（除 platform-macos 2 个预存 Windows 失败）；fmt + clippy `-D warnings`。
9. **依赖台账**：本设计**无新第三方 crate**（PowerShell / tesseract 是外部进程，非 cargo 依赖）；`docs/third-party-licenses.md` 无需新增，但需记一句「运行期可选外部工具：Windows.Media.Ocr（系统自带）/ tesseract（用户可选装）」。
10. **文档**：indexer README 加 OCR 节；ROADMAP BETA-03 → done；STATUS；`docs/manual-test-scenarios.md` 加 BETA-03 真机手测（截图含字 → 搜得到）；`docs/windows-setup.md` 记 OCR 语言包/Tesseract 可选项。
11. **真机手测**（用户）：放一张含「会议纪要」字样的截图到图片目录 → 后台/手动 reindex → 搜「会议纪要」命中该截图。

## 5. 非目标（YAGNI）

- 不做 macOS Vision（trait 留接口，`MacosVisionOcr` 后续；需 Swift helper + 签名）。
- 不做批量 / 并行 OCR（v1 逐文件；首跑慢由 BETA-07 后台非阻塞兜底，批量留后续优化）。
- 不存图片特有元数据（分辨率 / 文字坐标框 / 置信度）——只取纯文本进 FTS。
- 不做 OCR 语言可配置（Windows 用 `TryCreateFromUserProfileLanguages`、Tesseract 固定 `chi_sim+eng`；多语言/可选语言留后续）。
- 不做扫描件 PDF 的 OCR（BETA-02 已取数字 PDF 正文；扫描件无正文这一窄路径留后续，可复用本引擎）。
- 不做 OCR 进度/取消细粒度（沿用 BETA-07 的 `indexing` 粗粒度状态）。
- 不引入新 cargo 依赖。

## 6. 风险与缓解

| 风险 | 缓解 |
|---|---|
| PowerShell WinRT 异步路径不通 | **已 spike 验证通过**（2026-06-02 真机，含中文） |
| CJK 字符间空格破坏 FTS | `normalize_ocr_text` 折叠 CJK 间空格（spike 实测发现，单测覆盖） |
| 共享表回收误删（图/文同根） | 回收按本轮 `exts` 收窄（§3.2），既有更安全 |
| 无 OCR 语言包 / 无 tesseract | `default_ocr_engine` 返回 None → 跳过图片索引、不报错（优雅降级，镜像发现器不可用回退） |
| 逐文件 PowerShell 启动慢（~0.5-1s/图） | mtime 增量重跑秒级 + BETA-07 后台非阻塞；批量留后续 |
| 坏图 / 解码失败 / 引擎 panic | `recognize` 返 Err 计 failed + `run_incremental_index` 的 `catch_unwind` 兜 panic，不崩整轮 |
| 注入（图片路径含特殊字符） | 路径经环境变量传入、脚本不插值（照搬 ADODB）；tesseract 走结构化参数 |
| 超大图（宽/高 > WinRT MaxImageDimension） | 真机实锤「The parameter is incorrect. Image dimensions are too large!」→ `win_ocr.ps1` 解码时按 `OcrEngine.MaxImageDimension` 等比缩放（`BitmapTransform` + 缩放版 `GetSoftwareBitmapAsync`）后再识别 |
