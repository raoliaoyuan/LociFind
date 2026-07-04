# BETA-37 设计：邮件格式提取（eml 正文 + headers 基础字段 + 附件递交现有提取管线）

> 2026-07-02 spec。关键决策四问已用户确认（全采推荐），见 §7。
> ROADMAP 卡片：BETA-37（packages/indexer，依赖 BETA-02，估时 1-1.5w）。
> 验收原文：eml/msg 进 DOC_EXTS + 提取器（正文 + from/to/date/subject headers 基础字段 + 附件递交现有提取管线）；**pst 明确不在本卡范围**；BETA-41 邮件子集命中率进 evals。

## 1. 背景与目标

B7 三场景（律所卷宗 / 审计取证 / 离职归档）里邮件都是核心材料形态：审计取证的凭证链（审批邮件 + 合同附件）、离职交接的说明邮件 + 账号清单附件。BETA-41 已备好 6 封 eml fixture（2 封带 base64 附件 part）当验收靶，cases.json 的 email / attachment 两桶（各 10 case）等本卡落地后进 evals 报告。

目标：eml 进增量索引管线——正文可 FTS / 语义检索，from/to/date/subject 基础字段可检索可展示，附件内容复用现有各格式提取器一并进索引。

## 2. 范围护栏（YAGNI）

- **不做 pst**（ROADMAP 卡面明确排除，防审计取证场景误期望）。
- **msg 后置**（本次拍板）：Outlook OLE/CFB 复合文档格式、Rust crate 生态弱、BETA-41 无 msg fixture 无验收靶 → 拆 BETA-37b 子卡后置，等真实需求/样本再评估 crate 或 shell-out 方案。本卡 DOC_EXTS 只加 `eml`。
- **不加邮件专属 schema 列**（本次拍板）：目前无按收件人/日期过滤邮件的检索需求，documents 表零变更。
- **附件不单独成 documents 行**（本次拍板）：附件不是磁盘上的独立文件，独立成行会造成幽灵 path（增量回收 / 打开文件 / 预览全要特判）。附件文本并入邮件 body。
- **不动**既有格式提取路径：非 eml 文件的提取与落库逐字节不变（BETA-27 byte-equal 精神）。

## 3. 解析依赖：mail-parser 0.11（本次拍板）

- Stalwart Labs 出品，**Apache-2.0 / MIT 双许可**（律所场景可用，不踩 AGPL 红线），纯 Rust。
- RFC 2047 encoded-word（fixture 的中文主题）/ base64 / quoted-printable / 嵌套 multipart / charset 解码全套开箱即用；HTML-only 邮件自带 html→text 转换。
- 手写最小 MIME 解析被否：真实邮件边角远多于 fixture，遗漏风险与维护成本都高。
- 登记 `docs/third-party-licenses.md`。

## 4. 提取设计

新模块 `packages/indexer/src/email_extract.rs`，由 `doc_extract::extract_document` 按扩展名 `eml` 分派。

### 4.1 字段映射（零 schema 变更）

| 邮件字段 | 落点 | 说明 |
| --- | --- | --- |
| Subject | `DocumentEntry.title` | FTS title 列可检索、UI 现有标题通道直接展示 |
| From | `DocumentEntry.author` | 显示名优先、无则邮箱地址；FTS author 列可检索 |
| From/To/Date/Subject | body 开头文本头块 | 全部可 FTS；Date 用解析后 RFC 3339 |
| Date | **不动** `modified_time` | 增量锚点仍是文件 mtime，语义不混 |
| doc_type | `"eml"` | 走现有 doc_type 通道 |
| page_count / passages / failed_pages | None / 空 | 邮件无页概念；落库只走 documents + FTS |

### 4.2 body 组装

```text
From: 张三 <zhangsan@example.com>
To: team@example.com
Date: 2026-07-02T10:00:00+08:00
Subject: 离职交接安排

<正文文本（text/plain 优先；HTML-only 时 mail-parser 转文本）>

[附件 e00076-db-account-list.txt]
<附件提取正文>
```

头块 → 空行 → 正文 → 每个附件一段（`[附件 文件名]` 标记行 + 提取文本）。整体过既有 `MAX_BODY_CHARS`（1MB chars）截断。

### 4.3 附件递交现有提取管线（本次拍板）

- 附件字节解码（mail-parser 已做 transfer-encoding 解码）→ 按**原始文件名扩展名**写入 `tempfile::TempDir`（复用 BETA-35 RAII 模式）→ 调 `extract_document` 复用 docx/pdf/xlsx/txt/… 全部现有提取器（扫描 PDF 附件自然继承 OCR 管线）→ 提取出的 body 以 `[附件 文件名]` 段追加进邮件 body。
- **深度限 1**：附件里的 eml 只提正文 + headers，不再展开其附件（`extract_eml` 带 `expand_attachments` 开关，防嵌套邮件炸弹）。
- **失败不中断**：单附件提取失败（不支持类型 / 损坏）→ 只留 `[附件 文件名]` 标记行（文件名本身可检索），tracing warn，继续下一附件；邮件整体不计 failed。
- **护栏**：单附件 > 32MB 跳过（只记文件名）；附件数 > 32 截断。

## 5. 测试

- **单测**（email_extract 内嵌）：合成 eml 字符串覆盖——encoded-word 中文主题、base64 正文、multipart+附件、HTML-only、无附件纯文本、损坏输入计 Tag err、深度限 1、附件失败留标记行。
- **fixture 集成测试**（`packages/indexer/tests/real_eml.rs`，**常跑不 --ignored**——纯 Rust 无外部依赖）：BETA-41 的 6 封 eml 全数提取，断言 subject→title、from→author、正文关键词、2 封附件的附件正文关键词命中。
- **回归**：indexer 既有全部单测 + clippy `-D warnings` + fmt。
- evals email/attachment 桶命中率报告仍待 enterprise 向量 bootstrap（Mac），不阻塞本卡。

## 6. 前置修复：BETA-41 eml fixture Subject 头残缺

`scripts/gen-enterprise-file-fixtures.ps1` 第 158 行 `"=?UTF-8?B?$subjB64?="` 里 PowerShell 把 `$subjB64?` 贪婪解析为（未定义的）变量名 → 6 封 eml 的 Subject 全成了残缺的 `=?UTF-8?B?=`，中文主题丢失。修法：`${subjB64}?=` + 按脚本会输出的内容修补 6 封 eml 的 Subject 行（不重跑整脚本——扫描 PDF 部分字体渲染有像素差，README 已注明生成物为准）。

## 7. 关键决策（2026-07-02 用户确认，全采推荐）

| # | 问题 | 拍板 |
| --- | --- | --- |
| Q1 | 解析依赖 | mail-parser crate（Apache-2.0/MIT 双许可、RFC 全套） |
| Q2 | msg 范围 | eml 全链路，msg 拆 BETA-37b 后置 |
| Q3 | headers 落库 | 复用现有列（subject→title、from→author、头块进 body），零 schema 变更 |
| Q4 | 附件处理 | 解码→临时文件→递归 extract_document（深度 1），文本并入邮件 body，不单独成行 |

## 8. cycle 划分

1. **cycle 0**：§6 fixture Subject 修复（脚本 + 6 eml）。
2. **cycle 1**：mail-parser 依赖 + `email_extract.rs`（headers + 正文，无附件）+ 单测。
3. **cycle 2**：附件递交管线（临时文件 + 递归 + 深度限 1 + 护栏）+ 单测。
4. **cycle 3**：`doc_extract` 分派 + `DOC_EXTS` 加 `eml` + `real_eml.rs` fixture 集成测试。
5. **cycle 4**：文档同步（third-party-licenses / indexer README / ROADMAP BETA-37 状态 + BETA-37b 登记）。
