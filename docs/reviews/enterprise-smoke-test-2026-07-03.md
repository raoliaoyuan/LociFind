# 企业三场景本机 smoke 测试记录

> 日期：2026-07-03
> 测试对象：`target/debug/locifindd.exe` + `test-materials/enterprise-scenarios-raw`
> 目的：验证三场景测试材料能否被 daemon collection 模式索引，并初步覆盖权限/audit/格式解析链路。

## 1. 环境与限制

- OS：Windows 本机
- daemon：`target/debug/locifindd.exe`
- 测试数据目录：`test-materials/enterprise-scenarios-raw`
- 临时数据目录：`.tmp/enterprise-smoke/data2`
- 模型：使用 `.tmp/enterprise-smoke/dummy.gguf` 触发默认 stub backend
- 运行模式：FTS-only 降级

补充确认：

- 桌面程序使用的真实 embedding 模型已存在：
  `C:\Users\Alice\AppData\Roaming\LociFind\models\embeddinggemma-300m-q8_0.gguf`
  （约 329MB）。此前未找到模型是因为误查了 `LocalAppData` 与仓库 `models/`，而桌面实现的单一信源是 `%APPDATA%\LociFind\models\`。

限制：

1. 仓库当前 `target/debug/locifindd.exe` 编译为默认 stub backend；即使传入真实 GGUF，也会 FTS-only，不能代表桌面安装版的语义状态。
2. 临时编译语义版 daemon 时，`llama-cpp-sys-4` 需要 Clang 19+ 的 `libclang.dll`。本机 Anaconda 自带 `libclang.dll` 版本过旧，MSVC 14.44 STL 报 `expected Clang 19.0.0 or newer`。
3. 已尝试下载 LLVM 20 installer 到 `.tmp`，但安装包运行失败 `0xc0000142`；已尝试 `conda create -p .tmp\conda-clang -c conda-forge libclang=20`，10 分钟超时且未落下 `libclang.dll`。
4. PowerShell 后台托管 daemon 时进程生命周期不稳定，MCP 在线 smoke 未完成；本轮以 daemon 前台日志、e2e 测试和 SQLite 索引库检查作为证据。
5. 扫描式 PDF 被识别并进入 OCR 路径，但本轮未落入 `documents` 表；JPG/PNG 也未落库，需要后续作为真实格式缺口继续排查。

## 2. 自动化 e2e 结果

命令：

```powershell
cargo test -p locifindd --test e2e
```

结果：9/9 通过。

覆盖：

- `/health`
- token 鉴权 401
- `list_collections`
- MCP `search`
- 信息墙：授权集合可搜，未授权集合 denied
- audit 留痕
- read_only collection 指名 reindex 返回 409
- 增量 reindex 可发现新增文件

## 3. 三场景材料首次索引结果

前台启动 daemon 后，7 个 collection 均完成首次索引并监听就绪：

| collection | document_scanned | document_added |
|---|---:|---:|
| `case-2026-blueharbor` | 15 | 12 |
| `case-2026-northfield` | 1 | 1 |
| `audit-2026-procurement` | 15 | 12 |
| `audit-2025-facilities` | 1 | 1 |
| `offboarding-lishili-tech` | 14 | 12 |
| `offboarding-lishili-hr` | 2 | 2 |
| `offboarding-other-tech` | 1 | 1 |

日志中扫描式 PDF 均出现：

```text
扫描版 PDF 检出，走 rasterize + OCR 管线
```

说明扫描 PDF 检测和 OCR 分支触发正常；但落库结果显示扫描 PDF 本轮未成功成为可检索文档。

## 4. 落库格式覆盖

SQLite 检查 `documents` 表后得到：

| collection | doc_type 分布 |
|---|---|
| `case-2026-blueharbor` | `docx=1`, `eml=2`, `md=3`, `pptx=1`, `txt=4`, `xlsx=1` |
| `case-2026-northfield` | `md=1` |
| `audit-2026-procurement` | `docx=1`, `eml=4`, `md=2`, `pptx=1`, `txt=3`, `xlsx=1` |
| `audit-2025-facilities` | `txt=1` |
| `offboarding-lishili-tech` | `docx=2`, `eml=2`, `md=6`, `pptx=1`, `xlsx=1` |
| `offboarding-lishili-hr` | `txt=2` |
| `offboarding-other-tech` | `md=1` |

已确认可落库格式：

- DOCX
- PPTX
- XLSX
- EML
- MD
- TXT

本轮未落库格式：

- PDF（包含文本层 PDF 与扫描式 PDF）
- JPG
- PNG

## 5. FTS 关键词 smoke

直接查询 `documents_fts` 的 3 字以上关键词结果：

| collection | query | 命中示例 |
|---|---|---|
| `case-2026-blueharbor` | `违约金` | `settlement-draft.md`, `lawyer-letter-demand-performance.md`, 判决书副本 |
| `case-2026-blueharbor` | `和解协议` | `settlement-draft.md` |
| `audit-2026-procurement` | `收款账户` | `orion-procurement-contract.md`, `payment-account-mismatch.eml`, `orion-procurement-contract.docx`, `quotation-scoring.xlsx` |
| `audit-2026-procurement` | `Morningstar` | `supplier-aliases.md`, `vendor-quotation.eml` |
| `offboarding-lishili-tech` | `双层鉴权` | `kunpeng-api-authentication.md`, `kunpeng-api-authentication.docx` |
| `offboarding-lishili-tech` | `Lighthouse` | 交接清单、交接邮件、未结缺陷等 |
| `offboarding-lishili-hr` | `保密协议` | `nda-scan-source.txt` |

注意：裸 SQLite FTS 对 2 字中文词（如“账户”“鉴权”“调解”）仍受 trigram 限制；产品搜索链路已有 BETA-42 过滤策略，不应直接用裸 SQL 代表用户搜索体验。

## 6. 结论

本机可以直接开展测试，且已经完成一轮基础 smoke：

- collection/e2e 权限和 audit 能力通过自动化测试；
- 三场景材料可被 daemon 首次索引；
- DOCX/PPTX/XLSX/EML 等真实业务格式可落库并可 FTS 检索；
- 扫描 PDF 检测路径被触发。

但还不能宣称三场景真实验收完全通过：

1. 语义召回未启用：真实 GGUF 已确认存在，但源码侧 daemon 语义构建缺 Clang 19+ `libclang.dll`。
2. PDF/JPG/PNG 未落库，需要排查真实格式材料生成方式、OCR 依赖或索引器支持路径。
3. MCP 在线 smoke 因本轮后台进程托管不稳定未完成，需在稳定 daemon 服务或用户已安装服务进程上重跑。

## 7. 后续建议

1. 优先补一轮 PDF/JPG/PNG 真实格式索引排查，至少让文本层 PDF、扫描 PDF、图片 OCR 三类在材料包中均可落库。
2. 安装或提供 LLVM/Clang 19+（含 `libclang.dll`）后，重建语义版 daemon，使用 `%APPDATA%\LociFind\models\embeddinggemma-300m-q8_0.gguf` 跑语义召回 query，统计 top-5 命中率。
3. 以已安装服务方式启动 daemon 后，按 `expected/queries.tsv` 跑 MCP `search` / `list_collections` / 越权 / audit 导出全链路。
