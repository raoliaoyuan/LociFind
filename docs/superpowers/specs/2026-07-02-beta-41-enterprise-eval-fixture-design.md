# BETA-41 设计：企业场景评测语料 fixture（三场景共同验收基线）

> 2026-07-02 spec。关键决策四问已用户确认（全采推荐），见 §8。
> ROADMAP 卡片：BETA-41（packages/evals，依赖 BETA-13，估时 1w）。
> 验收原文：三场景合成语料（扫描 PDF / 邮件 / 附件 / 跨语言别名 / 近重复材料）+ 相关性标注 + query 子集；隐私红线沿用「合成集入仓做 CI 门控」方案（BETA-15B-6 同款）。

## 1. 背景与目标

B7 企业冷归档检索底座三张能力卡（BETA-35 扫描版 PDF OCR / BETA-37 邮件提取 / BETA-38 向量规模化 + doc identity）各自的验收都指向同一句话——"BETA-41 子集命中率进 evals"。没有这份 fixture，BETA-35 已落地的管线只有单测与装机手测，没有可回归的质量基线；BETA-37/38 开工时也没有验收靶子。BETA-40 playbook 同样依赖本卡。

目标：一次会话落地**可入仓、全合成、确定性可回归**的企业场景评测基线，覆盖三场景（律所案件卷宗 / 企业内部审计取证 / 离职员工材料归档）与五类材料形态（扫描件 / 邮件 / 附件 / 跨语言别名 / 近重复）。

## 2. 范围护栏（YAGNI）

- **不做** msg / pst 文件 fixture（msg 留 BETA-37 做提取器时一并补；pst 已被 BETA-37 明确排除）。
- **不做**十万级规模语料（BETA-38 的水位基准用程序生成的压力语料，另行处理；本卡是**质量**基线不是**规模**基线）。
- **不动** semantic-recall 现有 fixture / baseline / gate（v5/v6 锚点原样，byte-equal）。
- **不做**中间格式的真实扫描件采集——一切文本、图像、PDF 均为合成生成。
- 检索质量层评测**不跑 OCR / 不跑真实提取**——语料 body 即"提取后文本"（含轻度拟真 OCR 特征），确定性优先；真实管线由文件层 `--ignored` 测试覆盖。

## 3. 架构：双层结构

```text
packages/evals/fixtures/enterprise-recall/
├── README.md                 # 数据卡：桶分布 / 隐私自查 / 跑法 / bootstrap
├── corpus.json               # 检索质量层：合成文档（doc_id/lang/title/body + scenario/doc_type/dup_group）
├── cases.json                # 检索质量层：graded 相关性 cases（五桶）
├── vectors.json              # embedding 缓存（bootstrap 后提交；缺席时 gate skip）
├── baseline.json             # 分桶锚点（bootstrap 后提交）
└── files/                    # 文件层：驱动 indexer 真实提取管线
    ├── lawfirm/              #   律所卷宗（扫描 PDF + 文本 PDF 对照 + 近重复副本）
    ├── audit/                #   审计取证（eml 邮件 + 附件 + 扫描凭证 PDF）
    └── offboarding/          #   离职归档（杂格式材料 + 近重复副本）
scripts/gen-enterprise-file-fixtures.ps1   # 一次性生成脚本（扫描 PDF + 文本 PDF 对照 + eml；脚本+生成物都入仓）
packages/indexer/tests/real_pdf.rs     # 文件层端到端 --ignored 测试（扩）
```

- **检索质量层**（CI 常跑，确定性）：复用 `semantic_quality` harness 与 `SemanticDoc`/`SemanticCase` 类型；新 `--fixture-set enterprise` 指到本目录；五桶独立于个人场景五桶；vectors/baseline 独立文件，bootstrap 前 gate skip（BETA-15B-6 Phase D 同款语义）。
- **文件层**（装机 `--ignored`）：真实合成文件喂 BETA-35 管线（is_scanned_pdf → pdftoppm rasterize → OCR → passages）；文件名/正文与检索质量层 doc_id 对应（README 列映射表），BETA-37/38 落地后同一批文件继续复用。

## 4. 关键设计

### 4.1 语料 schema 扩展（不破现 fixture）

`SemanticDoc` 加三个可选字段（`#[serde(default, skip_serializing_if = "Option::is_none")]`）：

- `scenario: Option<String>` — `lawfirm | audit | offboarding`
- `doc_type: Option<String>` — `scanned | email | attachment | plain`
- `dup_group: Option<String>` — 近重复组 id（同组 = 同一材料的近似副本，服务 BETA-38 doc identity）

semantic-recall 现有 corpus.json 不含这些字段 → 反序列化 None、序列化跳过，主 fixture 与 gate 零影响。

### 4.2 五桶（enterprise 桶集，与个人场景桶集并列）

| 桶 | 验收对应 | 含义 |
| --- | --- | --- |
| `scanned-pdf` | BETA-35 | ground truth 是扫描件 OCR 文本（body 带轻度拟真 OCR 特征：无版式、偶发识别误差），query 描述内容 |
| `email` | BETA-37 | ground truth 是邮件（body = 正文 + from/to/date/subject headers 文本化） |
| `attachment` | BETA-37 | ground truth 是邮件附件文档，query 只描述附件内容不提邮件 |
| `crosslang-alias` | 三场景共同 | 实体（公司/项目/案号）中英文别名不同，query 用另一语言的别名指称 |
| `near-dup` | BETA-38 | ground truth 是一组近重复副本（dup_group），全组标注相关（主本 grade 3、副本 grade 2）——检索该召回、去重不该丢 |

拟真 OCR 特征的**度**：必须过 BETA-33 双层门槛（meaningful_ratio ≥ 0.6），即"干净文本 + 少量字符级误识"，不是乱码——否则语料自身会被生产管线 A 层拦掉，评测失真。

### 4.3 规模（用户已拍板：中档）

~90-120 docs（三场景各 30-40，zh 为主 + 每场景若干 en 配对），~45-60 cases（五桶各 9-12）。graded 相关性沿用 1-3 三档。

### 4.4 隐私红线（BETA-15B-6 同款，加严）

- 全合成零 PII：人名「李示例 / 王样本」式占位、公司虚构（含中英文别名对）、邮箱一律 `@example.com`、案号/凭证号用明显虚构格式、金额约整数。
- README 隐私自查清单 commit 前逐项勾。
- 完整性测试加**机器可查的启发式门**：corpus/cases 全文出现 `@` 时域名必须是 `example.com|example.org`；禁真实常见公司词表（spot check）。

### 4.5 文件层生成（用户已拍板：脚本 + 生成物都入仓）

- **扫描 PDF**：PowerShell + .NET `System.Drawing` 把合成文本渲染成页图（微软雅黑，模拟 200 DPI 扫描），JPEG 编码后打包 image-only PDF（DCTDecode，手工组装 PDF 结构，零新依赖）。5-8 份、每份 1-3 页、总体积 < 2MB。脚本入仓可复现；生成物入仓保确定性（换机重跑字体渲染有像素差，OCR 结果可能漂移——以入仓物为准）。
- **eml**：手写 RFC 5322 纯文本（headers + text/plain 正文 + base64 MIME 附件 part），内容与检索质量层 email/attachment 桶 doc 对应。
- **近重复副本**：同一文本的 2-3 个变体文件（改日期/落款/极小编辑），md/txt 格式。
- **文本层 PDF 对照**：1-2 份真文本 PDF（走原 pdf-extract 路径），守 BETA-27 byte-equal 端到端不回归。

### 4.6 端到端测试（文件层）

`real_pdf.rs` 扩一条 `--ignored` case：遍历 `fixtures/enterprise-recall/files/**` 的扫描 PDF → 断言 `is_scanned_pdf` = true → `default_pdf_rasterizer` + `default_ocr_engine`（Windows.Media.Ocr / tesseract，探测缺席则跳过）→ 断言至少 1 页 OCR 成功、页码从 1 起、OCR 文本命中该 doc 的合成关键词（宽松子串，容忍识别误差）。本机（Windows + pdftoppm + Windows OCR）可实跑。

## 5. 与现有 task 的关系

- **BETA-35（done）**：本卡补齐其"BETA-41 扫描 PDF 子集命中率进 evals report + fixture 端到端"验收尾巴。
- **BETA-37/38（not_started）**：email/attachment 桶与 near-dup 桶 + dup_group 标注是它们开工时的现成验收靶；eml 文件层已就位。
- **BETA-40**：playbook 示例 query 可直接从 cases.json 三场景子集选。
- **BETA-15B-6 semantic-recall**：类型与 harness 复用、fixture/baseline 完全隔离。

## 6. 分 cycle 拆解

| Cycle | 内容 | 验证 |
| --- | --- | --- |
| 1 | corpus.json + cases.json + README（三场景五桶语料手写） | 隐私自查清单全勾 |
| 2 | harness `--fixture-set` + schema 可选字段 + enterprise 完整性门 | integrity 测试过 + semantic-recall gate 零回归 |
| 3 | 生成脚本 + 扫描 PDF 生成物 + eml + 近重复副本入仓 | 生成物可被 pdftoppm 打开、体积 < 2MB |
| 4 | real_pdf.rs 端到端 case | 本机实跑：扫描判定 + OCR 命中关键词 |
| 5 | 向量 bootstrap 尝试（本机 CPU embed）+ baseline；不成留 bootstrap 说明 | gate 转真跑 或 skip 语义明确 |

## 7. 验收落地（对齐 ROADMAP）

1. 三场景合成语料 ✅ corpus.json（scenario 字段三值齐）
2. 扫描 PDF / 邮件 / 附件 / 跨语言别名 / 近重复材料 ✅ 五桶 + 文件层实体文件
3. 相关性标注 + query 子集 ✅ cases.json graded 1-3
4. 隐私红线「合成集入仓做 CI 门控」✅ README 数据卡 + 完整性门启发式检查

## 8. 已拍板决策（2026-07-02 用户确认，全采推荐）

- **Q1 结构**：双层（检索质量层 JSON + 文件层真实文件）。
- **Q2 扫描 PDF 入仓**：生成脚本 + 生成物都入仓，总 < 2MB。
- **Q3 规模**：中档 ~90-120 docs / 45-60 cases。
- **Q4 邮件格式**：本卡只造 eml，msg 留 BETA-37。

## 9. 产出

- fixtures/enterprise-recall/ 全套（语料 + 文件层 + 数据卡）
- semantic_quality `--fixture-set enterprise` + enterprise 完整性门
- scripts/gen-enterprise-file-fixtures.ps1
- real_pdf.rs 端到端 case
- （视 bootstrap 成败）vectors.json + baseline.json
