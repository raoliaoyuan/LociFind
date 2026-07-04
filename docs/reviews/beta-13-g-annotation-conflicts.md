# BETA-13-G parser 无解缺口：标注冲突决策清单

> 日期：2026-06-19 · 工具：Claude Code (Opus 4.8)
> 关联：[spec](../superpowers/specs/2026-06-19-beta-13-g-parser-clean-gaps-design.md) / [plan](../superpowers/plans/2026-06-19-beta-13-g-parser-clean-gaps.md)
>
> **这些缺口 parser 改不动**——动则破 v0.5 byte-equal，或评测集标注本身自相矛盾。
> 需用户**逐条拍板**后另起任务处理（多数涉及评测集 ground-truth re-baseline，会动 byte-equal 锚点）。
> 本会话只产出本清单，**未改任何评测集 fixture**。

---

## 用户决策（2026-06-19 拍板，Claude Code Opus 4.8）

§1.1（video/截图 + size·time）此前已拍「保 v0.5 = media」并落地（见下 §一更新）。本轮就剩余四条产品语义拍板：

| 决策 | 选项 | byte-equal 影响 | 落地方向 |
|---|---|---|---|
| **A. 跨范畴多类型枚举**（§1.2） | **file_search 多值** | 改 parser（扩 cross-category 并列覆盖三类 + image+screenshot 同范畴）；需核 v0.5 是否有同形态锚点 | parser 跟 coverage；coverage 多值标注须与 B 一致 |
| **B. d9 多类型并列**（§二） | **一律 file_type 数组** | 去掉 None 特例（`pdf 和 doc`→[document]）；`music video` 单概念 MV 作 None 例外保留 | 改 coverage d9 标注统一 + parser 对齐；核 v0.5 锚点 |
| **C. 孤立排序句**（§3.1） | **划边界** | 带类型约束排序=新 file_search、纯排序词=refine（维持 v0.5）；若 v0.5 有「sort all my PDFs」类 refine 锚点则破 byte-equal、需 re-baseline | 两套标注按该边界各自对齐 |
| **D. 无上下文破坏性动作**（§3.3） | **维持保守设计** | **byte-equal 安全**——parser 不动，只改 coverage 标注对齐现 Clarify/FileSearch（BETA-13-G5/G7 中度阈值） | 仅改 coverage ground-truth |

**落地路径**：据此另起「评测集 re-baseline + parser 对齐」task（A/B/C 含 coverage 改 + parser 改 + 可能 v0.5 re-baseline；D 仅 coverage 改、最干净）。执行前须逐条核 v0.5 同形态锚点存量，量化 byte-equal 破坏面，再决定 A/B/C 是改 coverage 还是重立 v0.5 基线。

## 本会话已吃掉的干净缺口（前后对照）

| 指标 | 修前 | 修后 | Δ |
|---|---|---|---|
| v0.9 总体 pass | 726 (72.6%) | **765 (76.5%)** | **+39** |
| v0.9 fail | 49 | **36** | −13 |
| v0.9 partial | 225 | 199 | −26 |
| v0.5 byte-equal | 473 | **473（逐字节不变）** | 0 |

三块干净 fix：① 截图内容子句路由（+13）；② 中文尾置类型名词→file_type（+14）；③ 英文 head 类型名词→file_type（+3）；④ artist 自然措辞抽取（+9）。

剩余 v0.9 未通过 = 36 fail + 199 partial（均在 coverage 子集；v0.5 子集 473/25/2 不动）。以下按「为何 parser 改不动」分类。

---

## 一、v0.5 ↔ coverage 契约冲突（最大块，parser 物理无解）

> **更新 2026-06-19（已部分解决）**：用户拍板「media-type + size/time → **media_search**，以 v0.5 契约为准」。据此**修正 coverage 8 条 video/screenshot 标注**（file_search→media_search，从 v0.5 同形态锚点逐字段推导、非照抄 parser）→ v0.9 fail 34→26（−8）、pass +5。剩 3 条因 parser 字段 bug 转 partial（见下「parser follow-up」），image 相关（v0.5 倾向 file）与跨范畴多类型未动。独立审查确认契约对齐、非凑指标。
>
> **由此暴露的 3 个 screenshot parser bug（新登记 follow-up）**：① screenshot+time 泄漏 keyword（`最近三天的截图`→`["三天的截图"]`，根因 `extract_screenshot_keywords` 粗暴，**触 v0.5 19 条 screenshot 锚点、有 byte-equal 风险**）；② 英文 `last three days`（词非数字）未解析为 time（`screenshots from the last three days`）；③ media 路径不认 `按名字排`→`name_asc`（`上个月的截图按名字排`，parser 给 created_desc）。修这 3 条可把 partial→pass。

同一查询形态，v0.5 与 coverage 给了**互相矛盾**的标注。任何 parser 路由规则都不能两全；改任一方都破对应锚点的 byte-equal。

### 1.1 video / 截图 / 图片 + size·time → media vs file（约 11 FAIL + 大量 partial）

| 样例 query | v0.5 现判 | coverage 期望 |
|---|---|---|
| `大于100MB的视频` / `videos larger than 100MB` | media_search | **file_search** |
| `最近三天的截图` / `screenshots from the last three days` | media_search | **file_search** |
| `创建于上个月的图片` / `上个月的 screenshots` | media_search | **file_search** |
| `下载目录里大于200MB的视频` / `downloads 里大于100MB的 video` | media_search | **file_search** |
| `桌面上 smaller than 1MB 的图片` | media_search | **file_search** |
| `截图目录里的图片` | media_search | **file_search** |

v0.5 含 **50+ 条** `视频/截图 + size/time → media_search` 锚点（如 `找下载目录中大于 100MB 的视频`→media、`find screenshots from last month`→media）。coverage 把同形态判 file_search。**改路由让 coverage 过 → 必破这 50+ 条 v0.5 → byte-equal 崩。**

**决策选项**（三选一，影响 ~11 FAIL + 数十 partial）：
- **(a) 保留 v0.5 标注为准**：视频/截图+size/time 归 media_search，把 coverage 这些条改判 media（改 coverage ground-truth）。
- **(b) 改判 coverage 为准**：承认「视频+大小」更像「带类型过滤的文件搜索」，把 v0.5 这 50+ 锚点 re-baseline 成 file_search（**破 byte-equal，需重立 v0.5 基线**）。
- **(c) 二者分流**：定义明确边界（如「纯媒体元数据词→media，size/time 约束→file」），同时改两套标注对齐该规则。

> 推荐先讨论产品语义：用户说「大于100MB的视频」时，想要的是「媒体浏览」还是「文件管理」？这决定 (a)/(b)。

### 1.2 跨范畴多媒体类型 → media vs file 多值（4 FAIL）

| 样例 | 实际(parser) | coverage 期望 |
|---|---|---|
| `图片、视频跟音乐，我全都要` | media_search(单 media_type) | file_search(file_type 多值) |
| `图片和截图` | media_search | file_search 多值 |
| `audio and image files` / `audio 和 image 文件` | media_search | file_search 多值 |

parser 已对部分跨范畴（如「音乐和视频」）做了 file_search 分流，但这几条触发词组合未覆盖。**注意**：这类**可能** parser 可修（扩 `has_cross_category_media_conjunction` 覆盖三类并列 / image+screenshot 同范畴并列），但需先确认与 1.1 的路由不打架、且 coverage 多值标注与 d9（见二）不矛盾——故归为「需先定标注规则再动」。

> **更新 2026-06-19（决策 A 已落地，Claude Code Opus 4.8）**：扩 `has_cross_category_media_conjunction`——连词补 `、`/`跟`、IMAGE 补单数 `image`、截图独立成类（`图片和截图`→[image,screenshot]）。这几条跨范畴枚举改路由到 file_search 多值。**v0.5 byte-equal 零回归（0 条 v05 锚点）**。详见下「§A/B 合并落地」。

---

## 二、d9 多类型组：标注自相矛盾（parser 无法对齐）

coverage d9 桶对「多类型并列」的标注**自相矛盾**，没有任何一致规则可同时满足：

| query | file_type 期望 | extensions 期望 |
|---|---|---|
| `找 pdf 和 doc 文件` | **None** | [doc,docx,pdf] |
| `把 pdf 和图片都找出来` | **[document,image]** | [pdf] |
| `word 或 excel 文档` | [document,spreadsheet] | None |
| `音乐和视频` | [audio,video] | None |
| `音乐视频` | **None**（视为单概念 MV） | None |
| `find pdf and doc files` | **None** | [doc,docx,pdf] |
| `music and videos` | [audio,video] | None |
| `music video` | **None** | None |

矛盾点：`pdf 和 doc`→file_type=None 但 `pdf 和图片`→[document,image]；同是两类并列，一个判 None 一个判数组。parser 无法同时拟合。

**决策选项**：统一 d9 标注规则后由 parser 跟随。建议规则候选：
- **多类型并列一律 → file_type 数组**（`pdf 和 doc`→[document]，去掉 None 特例），`音乐视频/music video` 作为单概念 MV 例外保留 None。
- 或 **一律保留 extensions、file_type=None**（走扩展名而非类型）。
需用户定一种，再改 coverage 标注 + parser 对齐。

> **更新 2026-06-19（决策 B + extensions 规则已落地，Claude Code Opus 4.8）**：用户拍板 B=「一律 file_type 数组」+ 补充拍板「多类型并列 extensions 一律 None、只留 file_type」。规则定为：**同范畴多词（pdf和doc 同为 document）→ 单 file_type 标量 + 扩展名并集；跨范畴（≥2 不同 file_type）→ file_type 数组（query 语序）+ ext=None**。详见下「§A/B 合并落地」。

---

## §A/B 合并落地（2026-06-19，Claude Code Opus 4.8）

A 与 B 同根（d2/d9 多类型路由 + extensions 一致性），合一刀。

**parser 改 2 处（均 byte-equal 安全，0 条 v05 多-file_type 锚点）**：
1. `has_cross_category_media_conjunction`（media_search.rs）：连词补 `、`/`跟`；IMAGE 补单数 `image`；截图独立成类。
2. `merge_extensions`（file_search.rs）：file_types ≥2 时 extensions=None（跨范畴只靠 file_type 表达类别，不列部分/不对称扩展名）。

**coverage re-baseline 15 条**（d2:12、d9:3，**仅 diff 在 file_type/extensions、严格遵循规则、非凑指标**）：同范畴 `pdf和doc`/`png和jpg` 等 file_type None→标量类别（保扩展名）；跨范畴 `word或ppt`/`mp3和mp4`/`pdf和图片` 等 → file_type 数组 + ext=None。BETA-18 单测 `cross_category_ppt_and_pdf_keeps_both_types` 按新规则更新（断言 ext=None）。

**结果**：v0.9 **778→803 pass（+25）、fail 23→19、partial 199→178**；**v0.5 byte-equal 零回归（0 变化）**；`cargo test --workspace` 775 passed/0 failed；clippy 0、fmt 净。

**范围外、登记的 parser bug（27 条 d2/d9 脏 case，本刀不动——改 coverage 对齐会是凑指标）**：
- **cross-category 路由后 keyword 残留**：`截图、视频和音乐都要`→keywords=[都要]、`图片、视频跟音乐我全都要`→[我全都要]、`音乐和图片文件都列一下`→[文件都列] 等（file_search keyword 抽取对枚举尾缀清理不净）。**【2026-06-19 已修】**`都要`/`我全都要`（G12 第一刀）+ `都列`（本刀）入框架词 → 这三条已 PASS。
- **location 误判**：`music, videos and pictures`→location={hint:pictures}（pictures 被当位置词）。**未修**——file_type 已对、仅 location 虚假，触 location parser 有 byte-equal 风险，登记 ②′ 隔离做。
- **英文复数/范畴类型词漏抽**：`documents and images`→只抽到 image、`videos and archives`→只 video、`code files and documents`→只 code（file_search 未识别 documents/archives 为类型词）。**部分修**：`videos and archives` 已 PASS；`documents`/`code files` 触 documents-location 消歧（④，高风险，留隔离 task）。
- **否定/排除未处理**：`要图片不要视频`/`images but not videos`→把否定类型也并进 file_type（**G12 第一刀已修**）、`视频和音频不含 mkv`→exclude_extensions 丢失（**本刀已修**：否定段字面扩展名 token→exclude_extensions；**en `no mkv` 推后**，需裸 `no` 标记 byte-equal 风险 + en-020 ground-truth 自相矛盾）。**【2026-06-19 决策 G 已落地】**用户拍板 G=「只对齐规则 B（改 coverage）」：删除 en-020 多余的 audio 扩展名列表（`videos and audio, no mkv` 原标 file_type=[video,audio] + ext=[9 个 audio 扩展名]，违反规则 B「跨范畴 ext=None」、且与 zh-020 不一致）→ 改 d2.json shard 走 assemble-coverage→generate-evals-v09。标注现与 zh-020 一致。**裸 `no` 标记按决策推后**——en-020 仍 partial（缺 exclude_extensions），从「2 处 diff」收窄为「1 处 diff」。
- **§3.2 数量/程度修饰**（另条决策）：`找几个视频`/`短视频`/`some videos`→file_search，coverage 期望 media_search。
- **MV/title**：`周杰伦的音乐视频`/`Eason 的 music video`→media title 字段差异。
- **refine 路由**：`文档和图片，排除压缩包`→refine（与 §3.1/C 同源）。**【2026-06-19 本刀已修】**决策 C 同构：refine 加约束门 `is_fresh_positive_then_exclude`（前向 `排除TYPE` + 排除前有正向类型→file_search 带 exclude_file_type；裸 `排除视频`/尾置 `把 ppt 也排除掉` 仍 refine）→ 已 PASS、v0.5 排除锚点零回归。

---

## 三、其余标注边界（需澄清规则，多与 v0.5 共享路由有冲突）

### 3.1 孤立 sort → FileSearch vs Refine（6 FAIL）
`sort all my PDFs by name` / `按名字排序` / `下载里最近一周的压缩包按大小排` / `videos in downloads bigger than 200MB sorted by size`：coverage 期望 **FileSearch（带 sort）**，parser 判 **Refine**（视「排序」为对上一次结果的细化）。
- 冲突：v0.5 把孤立「排序词」当 refine 触发（避免误捕 file_search）。改判会动 refine 路由 → byte-equal 风险。本会话 brainstorming 已**主动从干净 fix 中剔除此项**（与 refine 语义纠缠）。
- 决策：定「带文件类型约束的排序句（sort all my PDFs）= 新搜索 file_search」vs「纯排序（按名字排序）= refine」的边界，再改两套标注。

> **更新 2026-06-19（决策 C 已部分落地，Claude Code Opus 4.8）**：用户拍板 C=「划边界」。`try_parse_refine`（refine.rs）加**约束门**：`raw_sort && !(match_extensions || 位置信号)` 才作 refine 触发——裸排序词（`按大小倒序`/`sort by size`/`按名字排序`）仍 refine；带文件类型/位置约束的排序句交 file_search。另 sort 触发词 + delta.sort 补 `按名字`（对齐 `按名称`）。**v0.5 锚点盘点确认 6 条裸 `sort by size`/`按大小倒序` 全保持 refine、0 条 v0.5 是「sort+约束」→ byte-equal 零回归**。
> - **转 pass**：`下载里最近一周的压缩包按大小排`、`按名字排序`(d7→refine)、`按名字排序所有的 PDF`、`sort 一下所有 PDF by name`（后两条 coverage 补 file_type=document/ext=[pdf]）。
> - **范围外登记（parser gap / §1.1 冲突）**：`sort all my PDFs by name`（英文复数 `PDFs` 未被 match_extensions 识别 → 约束门漏判 → 仍 refine，**与 d2 英文复数类型词同源**）；`screenshots from last month sorted by name`、`videos in downloads bigger than 200MB sorted by size`（被 §1.1「screenshot/video+time·size→media」先截走，属 §1.1 契约冲突非 C）；`10到100MB之间的 archive 按 size 排`（`按 size` 英文+空格未识别为 sort + keyword 残留）。

### 3.2 数量/程度修饰 → MediaSearch vs FileSearch（3 FAIL）
`找几个视频` / `短视频` / `some videos`：coverage 期望 **MediaSearch**，parser 判 **FileSearch**（「几个/短/some」修饰干扰媒体路由）。可能 parser 可修（这些应保持 media），但需确认不破 v0.5「视频+约束→...」的现有判定。

> **更新 2026-06-19（决策 E 已落地，Claude Code Opus 4.8）**：用户拍板 E=「保 media_search（改 parser）」。`has_visual_media_with_abstract_modifier`（media_search.rs）在 cross-category 门后新增 `has_quantity_degree_modifier`（`几个/几张/几段/一些/若干/某些/短/some/a few/short` + 已 gating 的视觉媒体词→media_search）。`找几个视频`/`短视频`/`some videos` 三条转 pass。**gating**：v0.5 唯一同形态锚点 `找几个 G的视频` 经 `几个 g`（抽象 size）已进 media、不受影响；**v0.5 byte-equal 零回归（500 case 0 diff）**。

### 3.3 孤立 file_action（无上下文）→ FileAction vs FileSearch/Clarify（6 FAIL）

> **更新 2026-06-19（决策 D 已部分落地，Claude Code Opus 4.8）**：用户拍板 D=「维持保守设计，安全优先」。据此**仅改 3 条干净的「批量 move 全部」case**（`全部移动到归档文件夹` / `move all of them to the archive folder` / `把这些全部 move 到 Documents 文件夹`）：coverage 由 file_action 改为 **clarify(ambiguous_action)** + options 数组 → 对齐 parser 有意的批量安全确认行为（**非凑指标**：reason 是语义正确的 ambiguous_action，question/options 自撰非照抄 parser）。**v0.9 fail 26→23、pass 775→778、v0.5 byte-equal 零回归**。
>
> **另 3 条 NOT 改（守住非凑指标纪律）**：`移动到文档文件夹` / `重命名为 终稿` / `把第1、3、5个复制到U盘` parser 给的是 **file_search + 垃圾 keyword 片段**（`[移动到]` / `[重命名为,终稿]` / `[把第,个复制到]`），这是 **parse 失败/bug 而非保守设计**——file_action 才是正确 intent（尤其 zh-008 有显式 indices [1,3,5]=强约束）。改 coverage 对齐会把 parser bug 写进 ground-truth。→ **保 coverage、登记为 parser bug**（§四第 5 条）。**【2026-06-20 已修 parser，+5 pass】**详见 §四第 5 条（默认目标 + 多序数 + U盘 destination + d6 Documents 路径对齐 v0.5 约定）。

`移动到文档文件夹` / `重命名为 终稿` / `把第1、3、5个复制到U盘`（→FileSearch）；`全部移动到归档文件夹` / `move all of them to the archive folder`（→Clarify）。
- coverage 期望直接 FileAction；parser 在无前置结果上下文时判 FileSearch（动作词当 keyword）或 Clarify（要求确认）。
- 这是**设计取舍**：无上下文的破坏性动作该直接执行还是先 clarify？BETA-13-G5/G7 已定「中度阈值 + 有约束不拦」。改判涉及安全语义，需产品决策。

### 3.4 孤立时间/模糊 → Clarify vs FileSearch（1 FAIL）
`昨天的`：coverage 期望 Clarify(ambiguous_type)，parser 判 FileSearch(modified_time=yesterday)。与 BETA-13-G7 clarify 阈值设计相关。

> **更新 2026-06-19（决策 F 已落地，Claude Code Opus 4.8）**：用户拍板 F=「保 clarify（改 parser）」。`detect_vague_clarify`（lib.rs）在 `has_concrete` 门后加 `bare_relative_time_only`——剥前导搜索动词 + 尾「的」后**精确等于**单个相对时间词（昨天/今天/前天/明天/yesterday/today）才触发 clarify(ambiguous_type)。`昨天的` 转 pass。**gating**：v0.5 含 31 条「昨天/yesterday」锚点**全部带宾语名词**（ppt/视频/截图/pdf…），被 has_concrete（扩展名/媒体词）或残留非空（精确等于失败）挡住；`昨天的 pdf`/`昨天的视频`/`昨天的会议纪要` 均不误触发；**v0.5 byte-equal 零回归**。

---

## 四、parser 可修的近邻缺口（本会话范围外，建议后续单独 task）

> **【2026-06-20 ②′ image+约束 误路由 done，Claude Code Opus 4.8，+4 pass】**§1.1 决策（video/screenshot+size/time 留 media）的 **image carve-out** 落地：`创建于上个月的图片`/`桌面上 smaller than 1MB 的图片`/`截图目录里的图片` 由 media 转 file_search。改动：① media_search `has_visual_media_with_abstract_modifier` 加 image-only 守护（仅 image 无 video→file）；② `screenshot_dir_is_location`（提至 common.rs）——`截图目录/文件夹/夹`=location，media 路由 + file_search file_type 双抑制 Screenshot + 新 LocationAlias `截图目录`→`截图`；③ file_search 补 `创建`→created_time、LT 前缀 size 正则（smaller/less than/小于…，v0.5 零暴露）、`less_than`→size_asc；④ coverage zh-015 sort `modified_desc`→`created_desc`（对齐 v0.5 22 条 created 锚点全 created_desc 的约定）。**gating**：coverage 0 条 image→media、v0.5 唯一 image 锚点 `find images on desktop` 即 file、v0.5 0 条 `截图目录`/`小于+size`。**v0.9 831→835 pass、fail 7→4、§6 83.1%→83.5%；v0.5 byte-equal 零回归**。剩 2 fail（`screenshots…sorted by name`/`videos…sorted by size`）= §1.1↔§3.1 契约冲突，需评测集 re-baseline。

以下**不是**标注冲突，是 parser 真能修但超出当时 scope 的近邻缺口，作为 follow-up 候选登记：

1. **内容截图边缘变体**：`截图里同时出现订单号和金额的` / `screenshot with both order id and tracking number`——本会话 `detect_content_clause` 覆盖了 `里写着/写着/提到`，但未含 `同时出现` / `with both`。扩这两个引导词即可（注意 byte-equal）。（2 FAIL）
2. **英文 head documents 被 location parser 误判**：`documents that mention quarterly revenue` 等 file_type 已修对，但 location 仍被填 `{hint:documents}`（期望 None）→ 仍 partial。需在 location 抽取处对「已被 head 类型名词消费的 documents」抑制位置义。（~3 partial，location 共享 v0.5，有 byte-equal 风险）
3. **`archives between X and Y` 的 between-size 解析**：`archives between 10 and 100 MB` 的 file_type 已修对，但 `between…and…` size 区间未解析 → 仍 partial。（size parser 缺 between 形态）
4. **artist 抽取剩余重构债**（来自 Task 4 代码审查）：① ~~把 artist 抽取拆到独立 `artist.rs` 子模块~~ **done（2026-06-19，media_search.rs 2002→1635 行，零行为变化）**；② duration marker 列表与 size-expr parser 的 marker 集去重——**评估后跳过**（media marker 在 artist regex 字面量内、与 size-parser contains-list 形态不同，强合有行为风险、不干净）。
5. **~~【2026-06-19 决策 D 暴露】无上下文动作命令被误路由到 file_search（3 FAIL，纯 parser bug）~~ done（2026-06-20，Claude Code Opus 4.8，+5 pass）**：
   - `v09-d6-zh-004` `移动到文档文件夹` → 期望 file_action(move)，parser 旧给 file_search keywords=`[移动到]`。
   - `v09-d6-zh-005` `重命名为 终稿` → 期望 file_action(rename,new_name=终稿)，parser 旧给 file_search keywords=`[重命名为,终稿]`。
   - `v09-d6-zh-008` `把第1、3、5个复制到U盘` → 期望 file_action(copy,indices=[1,3,5])，parser 旧给 file_search keywords=`[把第,个复制到]`（**有显式 indices 强约束**）。
   - **根因 + 修法（file_action.rs，3 处 parser + 1 处 coverage）**：① **默认目标**——`try_parse_file_action` 重排为先抽 destination/new_name，无显式目标但**已抽到 destination 或 new_name** 时默认 `last_results Index{1}`（门控在「抽到 dest/new_name」，裸动作词如孤立「移动」仍落 file_search）；② **多序数** `第1、3、5个`——新 `RE_ZH_MULTI`（`第\d+(?:[、,]\d+)+`）抓 第 引导的顿号/逗号数字列表→Indices；③ **U盘 destination**——`U盘/优盘/usb`→`/Volumes/USB`（外接卷无 ~ 形态）。④ **coverage 对齐 v0.5 约定**：d6 三条 `move/copy→Documents` 的 destination `/Users/me/Documents`（与 v0.5 锁定的 6+ 条 `~/Documents` 锚点不一致的离群标注）改为 `~/Documents`，顺带把 zh-020/en-004 两条 partial 转 pass。
   - **gating**：v0.5 全部 file_action 锚点带显式目标、无「动作+无目标」期望 file_search；`把这些都复制到桌面`/`copy all of them` 走 clarify（上游拦截）；v0.5 无 `第N、M` 多序数 / 无 `U盘/usb`。**v0.9 826→831 pass、fail 10→7、partial 164→162；v0.5 byte-equal 零回归（500 case 0 diff）**。

---

## 附：评测集 pipeline drift 修复（2026-06-19，Claude Code Opus 4.8）

做决策 D 时发现 **BETA-13-G10 当时直接手改了 `coverage-cases.json`（8 条 video/截图 → media_search）但未同步 `_authoring/d5.json` shards**，导致 `assemble-coverage`（从 shards 重生 coverage-cases.json）会**静默回退** G10 的修正 → 潜伏 landmine。本次已把 d5.json 那 8 条 shard 同步成 media_search（对最终 `cases.json` 逐字节 no-op，仅恢复 `shards → assemble-coverage → coverage-cases.json` 的幂等性）。此后改 coverage 应走「改 shards → assemble-coverage → generate-evals-v09」标准 pipeline，勿再直接手改 coverage-cases.json。

---

## 五、结论

- **§6「总体 evals >90%」靠纯 parser 不可达**：剩余 36 fail 里约 21 条（一、二、3.1）是 v0.5↔coverage 契约冲突或标注自相矛盾，必须先做产品/标注决策才能动；其余（3.3/3.4）涉及安全语义设计取舍。
- **下一步建议**：本清单第一、二节（契约冲突 + d9 矛盾）是最大块，建议优先与用户敲定标注规范（尤其 1.1 video+size 的产品语义），再起一个「评测集 re-baseline + parser 对齐」task；第四节近邻缺口可在不触标注的前提下随时单独清。
