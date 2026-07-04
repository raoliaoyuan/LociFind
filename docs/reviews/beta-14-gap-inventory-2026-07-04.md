# BETA-14 前置：§6 90% 出场线缺口盘点与三刀收割（2026-07-04）

> 作者：Claude Code (Fable 5)
> 输入：v0.9 = **881 pass / 119 partial / 0 fail（§6 88.1%）**（BETA-13 收束后基线，
> [收束记录](./beta-13-rebaseline-decisions.md)）。
> 结果：**927 pass / 73 partial / 0 fail（§6 92.7%）——跨过 90% 出场线**；
> v0.5 锁定基线 **475→478 pass / 22 partial / 0 fail**（3 条 rename partial 由 parser 改进转正，
> 标注零改动）。

## 1. 盘点方法与关键发现

对 119 条 partial 逐条做**字段级 diff → 按根因聚簇**（此前 6-20 盘点按字段切、聚焦
file_type/location/sort 三字段 71 条并已消化见底）。换根因视角后发现两个此前未被盘过的大簇
（media title 残段 17 条、Refine delta 16 条），推翻了「纯 parser/coverage 已见底、
需 BETA-29 草稿 UI 换口径」的前提——**纯 parser + 少量标注对齐即可跨线**。

119 条根因分桶（主根因归属）：

| 根因簇 | 条数 | 处置 |
|---|---|---|
| MediaSearch title 残段过抽 | 17 | ✅ 第 1 刀（parser） |
| Refine delta 标注冲突 + 4 个 parser 缺口 | 16 | ✅ 第 2 刀（标注对齐 12 + parser 4） |
| FileAction 参数（destination/new_name/target_ref） | 14 | ✅ 第 3 刀（标注 5 + parser 9） |
| 时间表达缺口（before/after 绝对日期、区间、措辞） | 19 | 遗留（真产品价值最高，工作量大） |
| keywords 质量（复数归一/停词泄漏/中文短语） | 15 | 遗留（散点小刀 + 复数归一需拍板） |
| language 判定 | 10 | 遗留（v0.5 标注自身口径不一致，需拍板） |
| Clarify 文案逐字比对 | 9 | 遗留（评测口径需拍板；en 回中文文案是真 i18n 缺口） |
| file_type/ext 口径残余 + location 残余等 | ~19 | 遗留（部分卡 §6.5 豁免额度） |

## 2. 三刀落地明细

### 第 1 刀：media title 只收"点名"（+16，881→897）

- **根因**：`extract_simple_title` 把 artist 后残段整体当 title（「找林俊杰的**无损音乐**」
  「Beatles **songs** under 4 minutes」「周杰伦**《范特西》专辑里的歌**」）。
- **修法**（`media_search.rs`）：新增 `is_descriptor_segment`——兜底 title 候选若为
  修饰性描述（质量/流派词 lexicon 单一来源 + 时长比较短语 + 专辑指代/书名号 + 泛指媒体名词
  组合）则不作标题；显式点名路径（叫 X / called X / 《X》 / 播放 X）不经此判定。
  另补 artist 停词「时长」（修「时长不到3分钟的歌曲」误抽 artist）。
- **防误伤锚点**：v0.5 唯一非空兜底 title「找周华健的朋友」→ 朋友 保留；v0.9 全部 10 条
  非空 title 期望均来自显式点名路径。
- **翻正**：d4-zh ×9、d4-en ×3、d4-mixed ×2、d2-zh-028、d2-mixed-010。

### 第 2 刀：Refine delta（+16，897→913）

- **标注对齐 12 条**（d7 分片，Group-A 同款纪律）：v0.9 d7 标注与 v0.5 主流冲突——
  ① v0.5 有 20+ 锚点确立「只看 pdf → extensions+file_type」，d7 标「仅 extensions」；
  ② d7 的 `file_type` 用数组形状，而 BETA-18 wire 约定单值序列化回标量，**数组标注结构性
  永远不可能 pass**。对齐 = format 词带 ext+ft（标量）、category 词仅 ft（标量）。
- **parser 缺口 4 个**（`refine.rs` / `common.rs`）：
  1. 设值 scope：类型匹配限定在设值标记之后（「把 ppt 也排除掉，只看视频」设值对象是
     视频不是 ppt）；
  2. refine 语境 artist 兜底（「只要周杰伦的」/“only the ones by Adele”，仅其它字段全空时启用）;
  3. sort 附加与触发解耦（“just the videos, sorted by size”经 keep 触发后排序词也带上）；
  4. 多字段 clear 按 query 语序（「位置不限，类型也不限」→ [location, file_type]）。
  另：`parse_time_fields` 撤销「from last/from yesterday→created」——v0.5 该形态锚点全是
  screenshot（其路径自身归 created），非 screenshot 的「ones from last month」语义是修改时间。

### 第 3 刀：FileAction 参数（+14，913→927；v0.5 475→478）

- **标注对齐 5 条**（d6 分片）：destination `/Users/me/Desktop` → `~/Desktop`
  （机器相关绝对路径不可参测，G5 决策同理；~ 形态与 parser/产品行为一致）。
- **parser 缺口**（`file_action.rs`）：
  1. rename 混排介词（「rename 为/成」「重命名成」「改名成」）——**v0.5 的 3 条
     `把第5个 rename 为 synthetic-final` partial 因此转正**；
  2. 目的地介词（到/去/to/into）引导的显式路径 → destination 原样保留（不再被路径内
     pictures/documents 词误映射 `~/Pictures`），target 归指代/序数（「把它们移动到
     /Users/me/Pictures/2026」→ all + 路径 destination）；裸路径 target（「打开 /Users/me/x.pdf」）
     语义不变；
  3. external drive → `/Volumes/External`（对齐 G12 ③′ U盘→/Volumes/USB 先例）。

### 闸门（每刀均过）

- intent-parser 215 tests（新增 15：title 5 / refine 6 / file_action 4）全绿；
- evals 包全部 gate（含 v09_integrity 分片一致性）全绿；server 88 全绿；
- clippy(`-D warnings`) / fmt 净；
- **v0.5 逐条零回归**：三刀间每轮对比逐 case 状态，无任何 pass→partial/fail 移动；
  v0.5 标注文件全程未动（d6/d7 改的均为 v0.9 coverage 分片）。

## 3. 剩余 73 条 partial 的去向

92.7% 已过线，剩余项按性质分三类（详单：本次会话 scratchpad `remaining-partials.txt`，
组合分布 keywords 14 / language 10 / options+question 8 / 时间类 ~17 / 其余散点）：

1. **继续可做的 parser 刀**（不急，按用户反馈驱动）：时间表达（before/after 绝对日期
   「2026年1月之前创建」、区间「5月20到24号」、措辞「这周改的/最近拍的」，~17 条）、
   keywords 停词泄漏与中文短语保持（~10 条）。
2. **需拍板的口径项**：
   - language 判定（10 条，全在 v0.5）：v0.5 标注自身互相矛盾（「会议 Excel」期望 mixed，
     「budget pdf」期望 zh），建议统一口径或将 language 降出严格匹配；
   - Clarify 文案逐字比对（8 条）：建议 question 不参与逐字匹配（variant/reason/options
     结构参与）；en query 返回中文 clarify 文案是真 i18n 缺口，可另立小卡；
   - 英文复数归一（4+ 条）：keyword 是否词形还原（invoices→invoice）。
   - `documents`/`pictures` 位置义时不再附 file_type（d5-en-020/mixed-014，G15 谓词
     覆盖缺口）；「Excel/Word」类型词 v0.9 个别条期望无 ext（疑似漏改，与 v0.5 146 锚点冲突）。
3. **卡 §6.5 豁免额度的 v0.5 老账**（累计已用 2/25，剩 23）：hint `下载` vs `downloads`
   2 条、markdown file_type 1 条、schema-11 before 日期 1 条等——单条价值低，攒批处理。

## 3.5 追记（2026-07-04 IX/X 会话：两轮收割 + 四项口径拍板落地）

**第一轮（时间表达簇 + keywords 小刀 + 3 条标注离群对齐）**：v0.9 927→952（95.2%）、
v0.5 478→479。**第二轮（用户四项拍板全按推荐落地）**：

1. **英文复数归一**（做）——`singularize_en_keyword` 在关键词装配终点统一做（不进
   residual 抽取面，fallback 遗漏分析复用该面）；复数专有名词（minutes/news/series）例外；
   report(s) 半内容名词落单保留（mirror zh 报告）。
2. **Clarify question 口径**——核实为**既定实现**（v0.5 起 question 全忽略、options 只查
   结构，`is_clarify_question_equal`/`is_clarify_options_equal`），本项零代码变更。剩余 8 条
   clarify partial 全部是 **options 结构差异**（d6 危险动作 4 条：标注期望无 options、parser
   给「在访达显示/取消」；d8 模糊查询 4 条：标注期望类型/动作 options、parser 不给）——
   属产品行为拍板，非文案口径，另行决策。
3. **language 降出严格匹配**——`compare_json` 跳过 `language` 字段（分语言统计不受影响）；
   v0.5 +11 转正（490/10/0）。
4. **ext-ft 标注对齐**——d5-zh-006/021（补 ft=document）、d5-zh-037 / d5-mixed-002（补
   extensions）、d5-mixed-012（裸「今年的」created→modified 对齐 en-008）、d1-zh-024（Excel
   补 ext，同族）；另修 G15 谓词覆盖缺口（`in the <kw>` 限定词 + 句首「documents 里」位置义
   闸门 + 位置义 pictures 不作 Image 信号，en-020/mixed-014 转正）+「几百KB」→ <1MB 启发。

**终态：v0.9 = 977/23/0（97.7%）、v0.5 = 490/10/0**，全程逐 case 零回归。剩 23 条：
clarify options 结构差异 8（待产品拍板）、v0.5 老账 10（hint 双语形态 ×3、synthetic-artist
×4、markdown ft、项目归档 location、几个G sort，卡 §6.5 豁免额度）、零星 5（碳中和分词 /
保密协议 ft 标注（d3 自身不一致）/ 备份文件 已记录两难 / 裸 no 标记 / music 目录 hint 形态）。

## 4. 对 BETA-14 的影响

- §6「总体 evals >90%」指标在**本机 parser-only 口径已达 92.7%**；出场判定仍需双平台
  基准画像真机复跑（Class A 外部条件不变）。
- BETA-14 卡上「再上需 BETA-29 草稿 UI 或新一轮缺口盘点」前提已被本盘点替代：
  草稿 UI 不再是过线依赖，回归其产品价值本位。
