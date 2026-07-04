# BETA-35 设计：扫描版 PDF OCR 管线（三场景共同第一缺口）

> 类型：**B 阶段落地卡片**（非探针），产出是一条生产管线 + 命中回页能力 + 页粒度失败留痕。
> 关系：B7 企业检索底座（[ROADMAP §3.3](../../../ROADMAP.md)）三场景共同第一缺口。上游依赖 BETA-02（文档提取）+ BETA-03（OcrEngine 层）。产物给 BETA-41（企业评测语料）扫描 PDF 子集当召回验收基线，命中回页给 BETA-40 场景 playbook 当"取证可用"论据。
> 边界：守 PROJECT.md「不做分析层」——本卡只做**检索层**（文本进 FTS + embed），OCR 出来的段是给外部 LLM/人肉审阅的候选，不在本地做摘要 / 抽字段 / 比对。

## 1. 背景与目标

律所卷宗、企业审计凭证、离职归档三场景压倒性主力都是扫描版 PDF——纸质卷宗扫描件、合同签字页扫描件、老员工遗留的扫描资料。**当前 LociFind 对扫描 PDF 完全失明**：`extract_pdf()`（[doc_extract.rs:239](../../../packages/indexer/src/doc_extract.rs)）用 pdf-extract 0.10 只拿文本层，扫描页返回空串→documents 表进不了正文→FTS 空→embed 触不了 A 层 meaningful_ratio 门槛→**文件既搜不到也召不回**，取证场景直接不可用。

**BETA-35 唯一要解决的问题**：让扫描版 PDF 走「页渲染成图 → 复用 OcrEngine → 分段入库 → 命中回页码」全链路。

**验收 4 条**（ROADMAP.md:372 原文）：
1. 图片型 PDF 页渲染 → OCR → 可检索；
2. 页码/来源映射保留（命中能回到具体页，取证可用）；
3. 失败页记录不静默丢；
4. 命中预览可展示 OCR 段落。

配套：BETA-41 扫描 PDF 子集命中率进 evals report；文本层 PDF 路径 byte-equal 不回归。

## 2. 范围护栏（YAGNI）

**做**：扫描 PDF 检测 → 页渲染 → OCR → 页粒度分段入库 → 命中回页码 → 失败页留痕 → BETA-41 fixture 一条冷归档扫描 PDF 走通端到端。

**不做**（防蔓延）：
- 不做 PDF 里表格结构还原（表格 OCR 出来是行文本流即可，取证读得懂就行）；
- 不做 PDF 里手写签名/印章识别（超出通用 OCR 范围，走场景 playbook 里"人工翻页复核"）；
- 不做本地 LLM 摘要/翻译/比对（PROJECT.md「不做分析层」，走 BETA-40 MCP 工作流）；
- 不补 macOS Vision OCR（BETA-03 遗留 gap，另开 cycle，不进本卡）；
- 不改 `documents` 主表 schema（新表存页级 passage + 失败页，主表保持向后兼容）；
- 不做混合 PDF「部分页有文本层 + 部分页扫描」的**页级智能路由**优化——第一版整文档二分（有文本层→走原路径；无/极少文本层→整份走 OCR），混合优化留后续（§8 open question）。

## 3. 架构

```
 PDF 入库
   ↓
 [文本层探针] pdf-extract::extract_text  (现有路径,复用)
   ↓
   ├─ 文本充足 (整文档 ≥ SCAN_TEXT_FLOOR)
   │    → 走 BETA-02 原路径,byte-equal 不动 ✅
   │
   └─ 文本稀薄 / 空 (视为扫描版)
        ↓
      [PdfRasterizer trait] shell-out pdftoppm → 逐页 PNG (临时目录)
        ↓
      [OcrEngine trait] 复用 BETA-03,逐页 recognize
        ↓
      [Page 结果聚合]
        - 成功页: (page_no, text) → 段
        - 失败页: (page_no, reason) → document_failed_pages 新表
        ↓
      [双层门槛] 复用 BETA-33 cycle 4
        - A 层 meaningful_ratio 0.6 段级过滤 (embed.rs:79)
        - 过槛段进 document_passages 新表 (含 page_no)
        ↓
      [命中回页] 检索命中段 → 携 page_no → UI 显示 "第 N 页"
```

**关键复用**（不重复造）：
- OcrEngine trait 已完备（[ocr.rs:23](../../../packages/indexer/src/ocr.rs)），WindowsOcrEngine / TesseractOcrEngine 双实现；本卡零改。
- meaningful_ratio 双层门槛已在（[embed.rs:60-85](../../../packages/indexer/src/embed.rs)），段级门槛复用现有 `is_embed_worthy()`。
- `catch_extract` panic 兜底（scan.rs:490）已在，pdf-extract 探针步照旧兜。
- 增量循环 `run_incremental_index` 已在，PDF OCR 走 `IndexError::Tag` 计 failed 语义已就绪。

**关键新增**：
- **PdfRasterizer trait**（并列 OcrEngine，同 pattern）：`fn render_pages(pdf: &Path) -> Result<Vec<(u32, PathBuf)>, IndexError>`——返回 (page_no, 临时 PNG 路径) 列表，调用方遍历后清理。
- **PopplerPdfRasterizer**：shell-out `pdftoppm -r 200 -png <pdf> <prefix>`，200 DPI 平衡 OCR 精度 vs 单页耗时（草案，spec 期不锁死，装机验证再复核）。
- **扫描版检测**：`extract_pdf` 拿到文本后跑 `is_scanned_pdf(text, page_count) -> bool`（阈值 `SCAN_TEXT_FLOOR`：整文档 < 100 chars/page × page_count × 0.1，草案）。
- **document_passages 表**：`(id, doc_id, page_no, seq, text, embed_source_hash)`；扫描 PDF 走这条路径；文本层 PDF **不入本表**、走 body-level 原路径不动（保 byte-equal）。
- **document_failed_pages 表**：`(id, doc_id, page_no, reason, failed_time)`；OCR 失败页记录，UI 侧可查询"这份 PDF 有哪几页读不出来"。

## 4. 关键技术抉择

### 4.1 PDF 页渲染工具（**核心抉择，用户需拍板**）

`unsafe_code = forbid` 是 workspace lint（[ocr.rs:3](../../../packages/indexer/src/ocr.rs) 已注释说明"原生 API 不能直接调用"），**pdfium-render / mupdf-rs 这类 FFI crate 直接排除**。候选如下：

| 方案 | 覆盖 | 许可 | 装机 | 评估 |
|---|---|---|---|---|
| **pdftoppm**（poppler-utils）| Win/Mac/Linux | GPL-2/LGPL | 用户装 or bundle | **推荐**：与 Tesseract 同 pattern（shell-out + onboarding 引导用户装），Windows 有 [poppler-windows](https://github.com/oschwartz10612/poppler-windows) 预编译包，macOS `brew install poppler` |
| mupdf `mutool` | 全平台 | **AGPL-3** | 类似 | ❌ **红牌**：AGPL 对律所/企业客户是明确风险（PROJECT.md 定位含企业冷归档） |
| ghostscript `gs` | 全平台 | **AGPL-3** | 类似 | ❌ 同上 |
| ImageMagick `convert` | 全平台 | Apache-2 兼容 | 类似 | 可作 pdftoppm 兜底、不做首选（需 ghostscript delegate 才能读 PDF，绕回 AGPL） |

**推荐**：`PopplerPdfRasterizer` 首实现，onboarding 加"检测 pdftoppm、未装引导 winget / brew"步（沿用 Everything / Tesseract 已建立的 UX 套路）；ImageMagick 与其他不进第一版。

### 4.2 扫描版检测策略

**推荐**（第一版）：**整文档二分**。
- `is_scanned_pdf(text_len, page_count) = text_len < 100 * page_count * 0.1`（每页平均 < 10 字符视为扫描版）
- 完全无文本层 → 整份走 OCR
- 有充分文本层 → 走原路径不动（byte-equal 保护）
- 混合 PDF（如首页封面是扫描图 + 正文有文本层）第一版按"有文本层"判、走原路径——**接受漏 OCR 封面**的第一版妥协，避免混合路由复杂度

**混合优化留 §8 open question**：可在下个 cycle 加"每页 stream 探测"（pdf-extract 目前不给页粒度接口，需换库或 patch，风险高、非本卡范围）。

### 4.3 页粒度存储：新表 vs 塞 body

**推荐**：新表 `document_passages`（含 page_no）。
- 扫描 PDF 走 passage-level embed；命中段直接带 page_no → UI 回页
- 文本层 PDF **不进 passages 表**，保持 body-level 旧路径，`document_vectors` 主向量表原逻辑不动
- 好处：BETA-27 byte-equal 保护自动成立（文本层路径零改）；page_no 从数据源头保留，不用回反解 body 里的位置
- 代价：段级 embed 会让扫描 PDF 单文档 embed 计算量上升——但扫描 PDF 本来就是 OCR 后才有正文、无 embed 就等于没索引，这是必须付的成本

### 4.4 失败页记录（验收 ③）

新表 `document_failed_pages(id, doc_id, page_no, reason, failed_time)`：
- OCR 超时 / 引擎错 / 解码失败 → 逐页记录
- 增量循环仍按现有约定：整份 PDF 只要有 ≥1 页成功入 documents，不计文件 failed（BETA-01A 语义）；全部页失败则整份计 failed（与现有约定一致）
- UI 侧留一个 read-only query 接口（面板不做，先做 SQL）：`SELECT page_no, reason FROM document_failed_pages WHERE doc_id = ?`——BETA-40 playbook 里可以给"取证复核用"的引导

### 4.5 命中预览展示 OCR 段（验收 ④）

命中段带 `page_no` → 桌面命中卡加一行 `第 N 页 · OCR` 标签。
- passage 检索返回时 join `document_passages.page_no`
- 段文本本来就在 passages 表，直接展示（复用 explain_passages 通路）
- 不改现有搜索卡结构，只增标签

## 5. 与现有 task 的关系

- **上游**：BETA-02（文档提取管线 + doc_extract.rs 现有 PDF 分支）；BETA-03（OcrEngine trait + Windows/Tesseract 双实现）。两者均已 done。
- **下游**：BETA-41（企业场景评测语料）——本卡产出后，语料里的扫描 PDF 子集才有意义；本卡产 fixture 一条端到端 case 到 BETA-41 语料。
- **旁支**：BETA-38（向量检索规模化）——扫描 PDF 大量入 passages 表会推动向量数量增长，BETA-38 sqlite-vec 迁移时把 passages 一起纳入；本卡先用暴力扫描，规模问题不本卡解。
- **不影响**：
  - BETA-27 parser byte-equal：本卡不动 parser、不动 intent 侧，评测 baseline 天然不影响
  - BETA-33 cycle 4 双层门槛：复用，段级 is_embed_worthy 直接跑
  - BETA-40 playbook：本卡产出「取证可回页」能力后，playbook 才能写"命中→翻到卷宗第 N 页人工复核"话术

## 6. 分 cycle 拆解与节奏

| Cycle | 内容 | 估时 |
|---|---|---|
| **cycle 1** | `PdfRasterizer` trait + `PopplerPdfRasterizer`（shell-out pdftoppm）+ 单测 fixture 扫描 PDF + `detect_pdftoppm()` 探测 | 2-3 天 |
| **cycle 2** | `is_scanned_pdf` 检测 + `extract_pdf` 分支路径（文本层→原路径 / 扫描版→新路径） | 1-2 天 |
| **cycle 3** | pipeline 整合：逐页渲染 → 复用 OcrEngine → 页级结果聚合 → 临时目录清理 | 2-3 天 |
| **cycle 4** | 新表 `document_passages` + `document_failed_pages` + schema 迁移 + 段级入库路径 | 2-3 天 |
| **cycle 5** | 命中回页 wiring：passage 检索携 page_no + 桌面命中卡"第 N 页 · OCR"标签 | 1-2 天 |
| **cycle 6** | BETA-41 fixture 一条端到端 case + evals 扫描 PDF 子集 + 文本层 PDF byte-equal 校验 + onboarding pdftoppm 引导 | 2 天 |

**估时复核**：ROADMAP 原写"1-2w（偏乐观，spec 期复估）"→ **复估 2.5-3w**（12-16 天）。偏乐观确认成立，主要低估在 cycle 3-4（页粒度 pipeline 改造 + schema 迁移），以及 cycle 5 UI wiring（passages 需要接过来渲染）。

## 7. 验收落地（对齐 ROADMAP 4 条）

| ROADMAP 验收 | 本 spec 落地 | 证据 |
|---|---|---|
| ① 图片型 PDF 页渲染 → OCR → 可检索 | cycle 1-3 pipeline 通 | fixture 扫描 PDF → `search "关键字"` 命中该文件 |
| ② 页码/来源映射保留 | cycle 4-5 passages.page_no + 命中卡标签 | 命中卡显示"第 N 页 · OCR"，SQL 层 page_no 非 NULL |
| ③ 失败页记录不静默丢 | cycle 4 document_failed_pages 表 | 故意断 OCR 引擎跑一次 → SQL `SELECT * FROM document_failed_pages` 有记录 |
| ④ 命中预览可展示 OCR 段落 | cycle 5 复用 explain_passages 通路 | 命中卡点开显示 OCR 出来的段文本，含 page_no |

**附加**：
- BETA-41 扫描 PDF 子集 Recall@10 进 evals report（cycle 6 落地）
- 文本层 PDF 路径 byte-equal：evals 现有 fixture 里所有非扫描 PDF 命中集与 v0.9.9 baseline 一致（cycle 6 校验）

## 8. 待用户拍板（open questions）

以下 3 个抉择 spec 期给了推荐但不锁死，请在 review 时确认，或提出替代方案：

**Q1（架构最关键）**：PDF 页渲染工具是否走 **pdftoppm shell-out + onboarding 引导**（沿用 Everything / Tesseract 已建立的 pattern）？
- 推荐：是
- 备选：bundling pdftoppm.exe 到 Windows 安装包（省用户装 poppler，但安装包体积 +30MB，且 poppler GPL-2 需在 [third-party-licenses.md](../../third-party-licenses.md) 声明）；macOS bundle 类似（体积影响小）
- 涉及：CONVENTIONS §7 隐私红线不受影响（本地渲染，无外发）；BETA-40 playbook 部署门槛（律所内网机能不能拉 poppler-windows）

**Q2**：扫描版检测第一版是否走**整文档二分**（每页平均 < 10 chars 就整份走 OCR）？
- 推荐：是（简单、覆盖 90% 场景）
- 备选：加每页判——需要换 PDF 库或 patch pdf-extract，风险显著上升
- 已知代价：混合 PDF 封面扫描页会漏 OCR，接受第一版妥协

**Q3**：BETA-35 是否顺带补 **macOS Apple Vision OCR**？
- 推荐：**不**（BETA-03 遗留 gap，另开 cycle）
- 理由：Vision 走 Swift/Obj-C，shell-out 通路需要写 Swift script + 编译进 app bundle，工作量单独一个 cycle；BETA-35 已 3w 估时，再加会溢出。macOS 用户第一版可先用 Tesseract shell-out（BETA-03 已支持）
- 若同意，另开卡 BETA-42 或直接补 BETA-03 后续

## 9. 产出

1. `packages/indexer/src/pdf_rasterizer.rs`：`PdfRasterizer` trait + `PopplerPdfRasterizer` + `default_pdf_rasterizer()`（同 ocr.rs 结构）
2. `packages/indexer/src/doc_extract.rs` PDF 分支改造：文本层探测 → 分路径
3. schema 迁移 + 新表 `document_passages` / `document_failed_pages`（doc_db.rs）+ `version.rs` 版本 bump
4. 桌面命中卡"第 N 页 · OCR"标签（apps/desktop）
5. onboarding pdftoppm 引导步（复用 BETA-31 EverythingCheckStep 结构，加 `PdftoppmCheckStep`）
6. BETA-41 fixture 一条扫描 PDF 端到端 case
7. evals 扫描 PDF 子集 Recall@10 报告一份（进 `docs/reviews/`）
