# BETA-15D — Spotlight 谓词形态适配 macOS 26 NSPredicate 回归 — 设计

> 日期：2026-05-30
> 作者：Claude Code (Opus 4.8)
> 阶段：B（Beta）/ B6 产品体验增强（BETA-15D，BETA-15 真机验证暴露）
> 类型：bug 修复 + Spotlight backend 查询层重构（谓词构建策略 + 双查询并集执行）

## 1. 背景与目标

BETA-15（同义词关键词扩展）收尾真机 verify 时，用 `locifind-cli` 复现用户原 case `找一份工作汇报相关的ppt`（同义词扩展产出 keyword `述职` + 扩展名 `ppt`），发现 **mdfind 拒绝**生成的复合谓词，报 `Failed to create query for '...'`。BETA-15 的 parser → predicate wiring 全部正确，问题在 OS 层。

### 1.1 根因（本会话 mdfind 实测确认，macOS 26.5 / Darwin 25.5.0）

**主回归：** Spotlight/MDQuery 谓词 parser **拒绝任何在同一复合谓词（`&&` 或 `||`）中混用「字符串匹配操作符」与「比较操作符」的查询**。

- 字符串匹配类：`CONTAINS` / `LIKE` / `BEGINSWITH` / `ENDSWITH`
- 比较类：`==` / `!=` / `>` / `>=` / `<` / `<=`

实测 2×2 真值表（左操作数固定为 keyword 的 CONTAINS OR 组）：

| 组合 | 例 | 结果 |
|---|---|---|
| str && str | `CONTAINS && LIKE` | ACCEPT |
| str && str && str | 三个 CONTAINS | ACCEPT |
| cmp && cmp | `>= && <`（时间范围） | ACCEPT |
| cmp \|\| cmp | `==glob \|\| ==glob` | ACCEPT |
| **str && cmp** | `CONTAINS && (== "*.ppt"cd)` | **REJECT** |
| **cmp && str** | `(>= 1000) && CONTAINS` | **REJECT** |
| **str && cmp && cmp** | keyword && 时间范围 | **REJECT** |
| **str \|\| cmp** | `CONTAINS \|\| ==glob` | **REJECT** |

→ 影响范围**远超文档原记录的「仅扩展名」**：`keyword + 扩展名`、`keyword + 时间`、`keyword + 大小`、`keyword + 类型` **所有把文本关键词（CONTAINS）与结构化约束（比较）复合的查询全坏**。BETA-15 之前 `find pdf` 真机通过，仅因 parser 未提 keyword、走纯扩展名路径（单 cmp，合法）。

**次生发现（CJK / 文件名索引）：** `CONTAINS[cd]` 对 `kMDItemFSName` / `kMDItemDisplayName` 做的是 word-token 匹配，**匹配不到 CJK 文件名子串**（实测 `购房协议.pdf` 的 FSName 已索引为 `购房协议.pdf`，但 `kMDItemFSName CONTAINS[cd] "购房"` 返回空；`述职` 同样匹配不到 `述职.ppt`；英文 `synthetic-place-note.md` 的 `CONTAINS "note"` 也空）。`== "*kw*"cd` glob 子串匹配可靠命中。

**关键正确性差异：**

| 谓词 | pdf 命中数 | 可与 keyword 复合 |
|---|---|---|
| `kMDItemFSName == "*.pdf"cd` | 55（正确） | 否（cmp 类） |
| `kMDItemFSName LIKE[cd] "*.pdf"` | 0（错误） | 是（str 类） |
| `kMDItemFSName ENDSWITH[cd] ".pdf"` | 0（错误） | 是（str 类） |
| `kMDItemContentTypeTree == "com.adobe.pdf"` | 55（正确） | 否（cmp 类） |

→ 能与 keyword 复合的 string 类操作符对扩展名都返回**错误结果**；唯一正确的 `==`/`!=` glob 和 `ContentTypeTree == UTI` 都是 comparison 类，无法与 keyword 的 CONTAINS 复合。**ROADMAP 原候选 (a) ContentTypeTree、(b) LIKE/去cd 均被实测推翻。**

**可行修复（实测命中）：** 把文件名 keyword 匹配从 `CONTAINS` 改为 `== "*kw*"cd` glob（comparison 类，顺带修 CJK），与扩展名/时间/大小 glob/比较 同为 comparison 类 → 单条 `cmp && cmp` 复合合法且返回正确结果：

```
(kMDItemFSName == "*述职*"cd || kMDItemFSName == "*工作*"cd) && (kMDItemFSName == "*.ppt"cd || kMDItemFSName == "*.pptx"cd)
→ 命中 述职.ppt ✓
```

但 **`kMDItemTextContent` 全文内容匹配只能用 `CONTAINS`（string 类，无法 glob 文件内容）**，无法与比较约束同谓词 → 必须拆独立查询，在 Rust 端合并。

### 1.2 目标

1. macOS 26+ 真机上，所有 `keyword + 扩展名/时间/大小` 复合 query 被 mdfind 接受、不再 `Failed to create query`。
2. BETA-15 spec §7 scenario 1（`找一份工作汇报相关的ppt → 述职.ppt`）真机端到端命中。
3. 顺带修复 CJK 文件名子串匹配（`述职` 单 keyword 也能命中 `述职.ppt`）。
4. 保留内容全文搜索能力（「找含 X 的 ppt」不漏内容命中）。
5. 既有 evals v0.5 parser-only 维持 byte-equal **472/26/2**；纯扩展名路径（`find pdf`）回归不变。

### 1.3 非目标

- WindowsSearchBackend / EverythingBackend 适配（BETA-15C，本 spec 只动 Spotlight）。
- 等 Apple 修复（候选 d，被动，不采纳）。
- 改 parser / common schema / desktop wiring / evals 源（本 spec 只动 Spotlight backend）。
- 召回质量定量评测（BETA-15A）。

## 2. 核心架构决策

### 决策 1：统一双查询并集（所有 keyword 搜索）

每次含 keyword 的搜索拆成两条**单操作符类别**子查询并发执行，Rust 端合并。纯扩展名 / 纯时间等无 keyword 的查询维持单条不变。

- **Q1 — 结构化查询（纯 comparison 类，单条 mdfind）**
  `(文件名 glob 关键词 OR 组) && 扩展名 glob && 时间范围 && 大小范围`
  - 关键词项：每个词产 `kMDItemFSName == "*kw*"cd || kMDItemDisplayName == "*kw*"cd`（覆盖原 `keyword_predicate` 的 FSName + DisplayName 两个文件名侧字段），多词/同义词组继续用 `||` 串联（cmp || cmp 合法）。
  - 扩展名：沿用 `kMDItemFSName == "*.ext"cd`（negate 用 `!=`）。
  - 时间 / 大小：沿用现有 `>=`/`<` 谓词。
  - 全部 comparison 类 → 整条 `cmp (&&|\|\|) cmp` 合法。

- **Q2 — 全文/字段查询（纯 string 类，单条 mdfind）**
  - 文件搜索：`kMDItemTextContent CONTAINS[cd] kw`（多词 `||`）。
  - 媒体搜索：`kMDItemAuthors / kMDItemTitle / kMDItemAlbum / kMDItemMusicalGenre CONTAINS[cd] kw`（沿用 media_search 现有字段集）。
  - 纯 string 类 → 合法。**Q2 不含任何比较约束**。

- **合并**：`Q1 ∪ Q2` → 按 canonical path 去重 → Q2 结果在 Rust 端按 Q1 同款扩展名/时间/大小约束**后置过滤** → 沿用现有 post-sort → 最后截 `limit`。

**为何统一（而非仅在有约束时拆）**：文件名改 glob 后，即使纯 keyword 查询也无法把 `文件名glob OR 内容CONTAINS` 放进单条（跨类 `cmp || str` 被拒）。统一双查询使代码路径一致，且纯 keyword 查询下 CJK 文件名也修好。代价：每次 keyword 搜索 +1 次 mdfind，两条**并发执行**（线程）使延迟≈1×。

### 决策 2：文件名 keyword 用 glob `== "*kw*"cd`，内容用 CONTAINS

- 文件名/显示名匹配（`kMDItemFSName` + `kMDItemDisplayName`）：`== "*kw*"cd`（comparison 类，glob 子串，cd 大小写+变音不敏感）。修 CJK + 可与比较约束复合。归 Q1。
- 内容匹配（`kMDItemTextContent`）：`CONTAINS[cd]`（string 类，无法 glob）。归 Q2。
- 二者分属 Q1 / Q2，不在同一谓词。原 `keyword_predicate` 的三字段（DisplayName/TextContent/FSName）按操作符类别一分为二，覆盖不丢。

### 决策 3：Q2 在 Rust 端复刻扩展名/时间/大小过滤（最高风险点）

Q2（string 类）不能携带比较约束，故其结果须在 Rust 端按与 Q1 相同的约束过滤后再并入：

- **扩展名**：比对 path 后缀（大小写不敏感；negate 取反）。
- **时间**：复刻 `TimeExpression → (from, to)` 区间逻辑，比对文件相应时间字段（`result_from_path` 已取 `modified/created/accessed`）。
- **大小**：复刻 `SizeExpression → 字节区间`，比对文件 size。

这是**与谓词语义产生分歧的唯一风险源**。plan 单列 task + 专门测试锁定「Rust 后置过滤判定 == Q1 谓词语义」，对同一组约束 + 构造文件断言两路一致。

### 决策 4：keyword glob 元字符转义

keyword 含 `*` / `?` / `\` 时须转义为字面量（否则被当 glob 通配）。扩展 `escape_predicate_string`（或新增 glob 专用转义），保证 `report*.pdf` 这类 keyword 不误展开。已有 shell-injection 转义测试不破。

## 3. 各组件改动清单

仅 `packages/search-backends/spotlight/src/lib.rs` 一文件：

| 区块 | 改动 |
|---|---|
| `keyword_predicate` / `keyword_predicate_expanded` | 拆为「文件名 glob（Q1 项）」与「内容/字段 CONTAINS（Q2 项）」两套构建函数 |
| `translate_file_search` / `_expanded` | 产出 (Q1, Q2) 两个 `SpotlightQuery` + 一组 Rust 端约束（PostFilter） |
| `translate_media_search` / `_expanded` | 同上，Q2 用 author/title/album/genre 字段 |
| `extension_predicate` / `time_predicate` / `size_predicate_*` | 形态不变（已是 comparison 类），新增对应的 Rust 端 PostFilter 构造 |
| `QueryBuilder` / `SpotlightQuery` | 产出双查询 + PostFilter；`finish` 调整 |
| `search` / `search_expanded` | 并发跑 Q1/Q2 → 合并 → 去重 → 后置过滤 → post-sort → limit |
| `escape_predicate_string` | 扩展 glob 元字符转义 |
| 单测 | 谓词形态 byte-equal 断言改新形态；fake-mdfind 集成测试改为期望 2 次调用 + 合并；新增 PostFilter 语义一致性测试 |

**不动**：parser / common / desktop / evals 源。

## 4. 数据流（keyword + 扩展名，以 `述职 + ppt` 为例）

```
intent { keyword:"述职", extensions:["ppt","pptx"] }
        │
   translate_*  ─┬─> Q1 = (FSName=="*述职*"cd) && (FSName=="*.ppt"cd || FSName=="*.pptx"cd)   [cmp，合法]
                 ├─> Q2 = (kMDItemTextContent CONTAINS[cd] "述职")                              [str，合法]
                 └─> PostFilter = { ext:["ppt","pptx"], time:None, size:None }
        │
   search: 并发 run_mdfind(Q1), run_mdfind(Q2)
        │
   merge: 路径集合并 → 去重(canonical) → Q2 子集按 PostFilter 过滤(ext 后缀匹配) → post-sort → limit
        │
   述职.ppt（来自 Q1）∪ 任何内容含"述职"的 .ppt（来自 Q2 过滤后）
```

## 5. 错误处理

- 任一子查询 mdfind 报 BackendUnavailable / Timeout → 沿用现有 SearchError 路径（双查询任一失败如何处理：**任一失败即整体失败**，保持与单查询一致的错误语义；并发其一超时则整体 Timeout）。
- mdfind 拒绝查询时把 `Failed to create query` 打到 **stdout 且 rc=0**（本会话发现）——本修复使谓词不再被拒，但**附带加固**：`run_mdfind` 检测 stdout 首行 `Failed to create query` sentinel，转 `SearchError::Io`，避免把错误行当文件路径喂给 `result_from_path`。

## 6. 测试

- 谓词构建单测：Q1/Q2 形态 byte-equal（文件名 glob、内容 CONTAINS、media 字段、negate 扩展名、时间、大小、同义词多组）。
- PostFilter 语义一致性单测（**最高风险点**）：对给定约束 + 构造文件元数据，断言 Rust 过滤判定与 Q1 谓词应得结果一致（扩展名/时间区间/大小区间各边界）。
- glob 元字符转义单测（`*`/`?`/`\` 不误展开 + shell-injection 既有测试不破）。
- fake-mdfind 集成测试：双查询调用次数、合并去重、Q2 后置过滤、并发、cancel、timeout。
- `run_mdfind` stdout sentinel 检测单测。

## 7. 验证门

- `bash scripts/ci.sh` 全过（fmt + clippy + test 全套，按 [[feedback_per_task_verify_include_fmt]]）。
- evals v0.5 parser-only **byte-equal 472/26/2**（不依赖 Spotlight；实跑确认）。
- hybrid Q4_K_M pass 480 理论维持（不沾 fallback/harness）。
- **真机端到端（用户驱动）**：`(FSName=="*述职*"cd) && (FSName=="*.ppt"cd)` 已实测命中 述职.ppt；dev build 跑 `docs/manual-test-scenarios.md` BETA-11 scenario 1 + 新增 keyword+时间 / keyword+大小 / 纯 keyword(CJK) / 纯扩展名(回归) case。

## 8. 风险与回退

- **R1 PostFilter 与谓词语义分歧**（最高）：缓解=专门一致性测试 + Q1 谓词与 Rust 过滤共用同一约束结构体派生，禁止两处手写。
- **R2 双查询延迟翻倍**：缓解=并发执行；实测单查询 p95 远低于 3000ms 阈值，2× 并发仍有余裕。
- **R3 glob 子串过度匹配**（`*述职*` 比 word-token 宽）：对文件名搜索可接受（更高召回），post-sort + limit 控制；若反馈过宽再 BETA-15A 量化。
- 回退：本改动局限单文件，git revert 即回到 BETA-15 状态（代码层仍 ready，仅真机被 OS bug 阻塞）。
