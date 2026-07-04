# BETA-13 评测集 re-baseline 决策清单（走向 §6 90%）

> **✅ 收束（2026-07-04，Claude Code，用户三项拍板）**：剩余 **4 fail 全部消化**，
> v0.9 = **881 pass / 119 partial / 0 fail（§6 87.7%→88.1%）**、variant 命中率 100%；
> v0.5 = 475/25/**0**。三项决策与落地：
>
> 1. **v05-schema-39a-039「把这些 pdf 复制到桌面」**：Refine → **FileAction**（copy /
>    last_results·all / `~/Desktop`）。该 query 与 39b-040 逐字相同、后者早已标 FileAction——
>    a/b 是同句双读法设计，无状态 parser 只能取一；G5 已拍板「指示代词+动作动词→FileAction」，
>    39a 老标注与之矛盾。**动 v0.5 锁定基线 1 条**，§6.5 豁免记录之一。
> 2. **v05-schema-45b-047「排除压缩包合并后」**：合并终态 FileSearch → **Refine**（delta
>    exclude archive，与 45a-046 同款）。原期望是「refine 应用到前一轮后的合并结果」，
>    但 evals runner 单轮无状态、query 字面不含下载/100MB/排序信息——结构性不可测。
>    合并终态正确性归 harness 层测试。**动 v0.5 锁定基线 1 条**，§6.5 豁免记录之二。
> 3. **v09-d5-en-024/025（§1.1↔§3.1 契约冲突）**：coverage 改标 **MediaSearch**（字段平移）。
>    关键事实：MediaSearch schema 本就有 sort/size/location/created_time 全部字段，可无损
>    表达；§1.1（视觉媒体+size·time→media）与 §3.1（带约束排序→新搜索）不再矛盾——
>    后者适用于非视觉媒体类型。不动 v0.5。顺手修 2 个 media 路径 parser 缺口
>    （`sorted by X` 短语漏进 screenshot keywords；`bigger than` 不入 `parse_size` GT 正则），
>    两者 v0.5 零暴露、byte-equal 安全，2 条从 partial 转 pass，各带回归单测。
>
> **§6.5 豁免账**：v0.5 锁定基线累计改 2 条（上限 25 条 = 5%），理由如上；v0.5 其余 498 条
> 逐字未动、pass 473→475 无任何 collateral 移动（partial 25 不变）。
> 距 §6 90% 出场线还差 1.9pp，全部在 119 条 partial 里——纯 parser/coverage 已见底，
> 后续路径 = BETA-29 查询草稿 UI（产品化）或新一轮缺口盘点（见 BETA-14 卡）。

> 作者：Claude Code (Opus 4.8)，2026-06-20
> 输入：v0.9 parser-only = **835 pass / 161 partial / 4 fail（§6 = 83.5%）**。本清单聚焦
> 占据缺口主体的三个争议字段 partial：**file_type 32 + location 20 + sort 19 ≈ 71 条**。
> 前置阅读：[beta-13-g-annotation-conflicts.md](./beta-13-g-annotation-conflicts.md)（G8 起的标注冲突底账）、
> 记忆 `project-stale-hybrid-fallback`（hybrid 已与 parser-only 持平，模型不再是变量）。

## 方法：用 v0.5 锚点把「争议」证伪

把每条争议 partial 的 `expected` 与 `parser 实际` 对照，再去 v0.5 的 500 条**锁定基线**里数同形态锚点。
结论与直觉相反——**这 ~71 条不全是「需要动 v0.5 的硬决策」**，按「该改谁」分三组：

| 组 | 性质 | 谁动手 | byte-equal 风险 | 约 cases |
|---|---|---|---|---|
| **A** | v0.9 coverage 标注与 v0.5 主流约定**不一致**（标错了） | 改 coverage 对齐 v0.5（G10 同法） | **低**（parser 不动） | ~26 |
| **B** | **parser 缺口**（v0.5 约定明确，parser 没覆盖某些措辞） | 改 parser（TDD + byte-equal 闸门） | 中（动 parser，需守 v0.5） | ~18 |
| **C** | **真·产品决策**（v0.5 自身锚点与期望冲突，或语义边界） | **需你拍板** | 高（动 v0.5 锁定基线） | ~22 |

> 注：很多 case 同时差多个字段（如 content-clause 既漏 file_type 又把类型词泄漏进 keywords），
> 多数耦合字段同根，一个决策一并解决；故每组的「pass 增益」是估算，真实翻 pass 以全字段消差为准。

---

## Group A — coverage 对齐 v0.5（建议：批准我直接执行，低风险）

这三项的共性：**v0.5 有压倒性的同形态锚点确立了约定，parser 也照此输出，唯独 v0.9 coverage 标反了**。
修法 = 按 v0.5 锚点逐字段改 coverage（走 shards→assemble-coverage→generate-evals-v09，G10 已验证的纪律），
**不碰 parser、不破 v0.5 byte-equal**。

### A1. 显式类型词 → 设 file_type（v0.5：146 锚点）

- **现象**：`yesterday's pdf` / `这个月新增的 PDF` / `比50MB还大的 PDF` / `png and jpg pictures` /
  `图片文件夹里的壁纸` 等，coverage 期望 `file_type=None`，parser 给 `document`/`image`。
- **v0.5 证据**：有 extensions 的锚点中 **146 条同时设 file_type**（`ppt→presentation`、`pdf→document`、
  `excel→spreadsheet`），仅 **2 条** None（其一是 `markdown→None`，见下例外）。parser 与这 146 条一致。
- **判断**：v0.9 这些 `pdf→None` 标注**与 v0.5 主流相悖**，属标注错误。
- **建议**：✅ coverage 改为 `file_type` 跟随类型词（pdf→document、png/jpg→image…），对齐 v0.5。
- **例外**：`markdown`/`md` 在 v0.5 唯一锚点是 `file_type=None`（`找最近一周访问过的 markdown`）。
  这条保留 None，**parser 对 markdown 给 document 反而是 over-reach**，归 Group B 一并修。
- **受影响**：约 13 条（v09-d3-zh-004/030、d3-en-002/016/024、d3-mixed-004、d5-zh-006/021、
  d5-en-013、d5-mixed-001/014、d8-en-009 等）。多数还耦合 keyword/sort（见 A2/B1）。

### A2. created / accessed 时间查询 → 时间匹配排序（v0.5：22 + 1 锚点）

- **现象**：`上周创建的 Word` / `2026年1月之前创建的合同` / `presentations created this year` /
  `PDFs accessed this week` 等，coverage 期望 `sort=modified_desc`，parser 给 `created_desc`/`accessed_desc`。
- **v0.5 证据**：含 `created_time` 的锚点 **22 条全是 `created_desc`**（无一例外）；含 `accessed_time` 的
  1 条是 `accessed_desc`。parser 与之一致。
- **判断**：v0.9 d5 把「创建/访问」查询的 sort 标成 `modified_desc`，**直接违反 v0.5 的 22 条 created_desc**。
- **建议**：✅ coverage 改为 `created_desc`/`accessed_desc`（跟时间维度走），对齐 v0.5。
  **切勿反向改 parser→modified_desc**：那会破 22 条 v0.5 byte-equal。
- **受影响**：约 11 条（v09-d5-zh-005/010/013、d5-en-005/007/008/013、d5-mixed-008、d4-en-024 等）。

### A3. 英文位置词 hint 保留英文（v0.5：36 条 `downloads`）

- **现象**：`find the biggest/largest ppt in downloads`，coverage 期望 `hint='下载'`，parser 给 `hint='downloads'`。
- **v0.5 证据**：location.hint 分布里 `下载`=48、`downloads`=36、`desktop`=24…——**v0.5 英文查询保留英文 hint**。
- **判断**：v0.9 这 2 条把英文 `downloads` 标成中文 `下载`，与 v0.5 的 36 条 `downloads` 不一致。
- **建议**：✅ coverage 改回 `downloads`，对齐 v0.5。
- **受影响**：2 条（v05-file-class1-sort-059/061）。

> **Group A 小结**：约 26 条标注对齐，**零 parser 改动、零 v0.5 byte-equal 风险**。这是走向 90% 最干净的一步。

---

## Group B — parser 缺口（建议：我按 TDD 修，不动 v0.5）

v0.5 约定明确、parser 只是没覆盖某些措辞。改 parser，每刀过 v0.5 byte-equal 闸门。

### B1. 内容子句里的类型名词 → file_type=document（且不泄漏进 keywords）

- **现象**：`里面有提到甲方乙方的协议文件` / `the contract that mentions John Smith` /
  `the resume that mentions Sarah Lee`：期望 `file_type=document, keywords=[内容词]`，
  parser 给 `file_type=None, keywords=[类型词 + 内容词]`（把「协议文件/contract/resume」当 keyword）。
- **判断**：parser 缺口——G8 已做「`…的报告`」尾名词映射，但 `协议文件`/`劳动合同`/`contract that…`/
  `resume that…` 这些形态没覆盖。修后**一并解决 file_type 漏设 + keyword 泄漏**（同根）。
- **风险**：动 content-clause keyword 抽取，触 v0.5 共享路径，需 byte-equal 守。中等。
- **受影响**：约 8 条（v09-d3-zh-009/036/040、d3-en-003/006/009/014/023）。

### B2. 「X 文件夹里 / 目录里」→ location（parser 漏抽）

- **现象**：`图片文件夹里的壁纸` / `影片文件夹里的电影` / `music 目录里的 lossless 歌曲`：
  期望 `location={hint:图片/影片/music}`，parser 给 `None`。
- **判断**：parser 缺口——显式「文件夹里/目录里」是强位置标记，parser 没把前缀名词当 location hint。
- **风险**：低（显式标记，不与 Group C 的「裸 documents」歧义重叠）。
- **受影响**：约 4 条（v09-d5-zh-026/028、d5-mixed-010 等）。

### B3. size 约束 → size 排序（v0.5：87 锚点）

- **现象**：`比50MB还大的 PDF`→size_desc、`小于1个G的安装包`→size_asc、`huge files over 2 gigs`→size_desc，
  parser 给 `modified_desc`。
- **v0.5 证据**：含 size 约束的锚点 **87 条全是 `size_desc`**。说明「size 约束→size 排序」是确立约定，
  parser 对部分措辞（尤其「小于…」size_asc、口语「比…还大」）没套上。
- **判断**：parser 缺口（G12 ②′ 已为 image 加 `less_than→size_asc`，此处推广到通用 file_search）。
- **风险**：中——须保住 v0.5 那 87 条已 pass 的 size_desc，新增 size_asc 分支别误伤。
- **受影响**：约 6 条（v09-d5-zh-017/019/021/039、d5-en-017、d5-mixed-012 等，多数耦合 A1 的 file_type）。

> **Group B 小结**：约 18 条，纯 parser，延续 G 系列已验证打法，可在 Group A 之后逐刀推进。

---

## Group C — 真·产品决策（需你拍板，动 v0.5 锁定基线或语义边界）

这三项才是「§6 见底」的真障碍：v0.5 锚点本身与 v0.9 期望相左，**改任一边都要付代价**。

> **✅ 已落地（2026-06-20，BETA-13-G15）**：用户拍板方案 (b) 上下文消歧。单一谓词
> `en_ambiguous_noun_is_location`——英文 `documents`/`pictures` 仅 `in documents`/`documents里`
> 标记才作 location，否则类型义（→ file_type）。**v0.9 863→871（+8）、§6 86.3%→87.1%、v0.5
> byte-equal diffs=0**。13 条 C1 中 8 条翻 pass，C1 核心（location 消除 + document 注入）全部正确。
> **剩 5 partial 全非 C1**：d2-en-005/016 = 多类型 ext 约定冲突（见 C2，需用户拍板）；
> d2-en-010 keyword 泄漏「code」/ d2-en-019 keyword 泄漏「excluding」/ d5-mixed-009
> `opened`→应 accessed 却判 modified = 相邻字段 backlog（keyword 抽取 / 时间维度，各自单独一刀）。

### C1.〔最大块〕`documents`/`pictures` 等：类型义 vs 位置义消歧

- **现象**：`documents and images` / `documents that mention quarterly revenue` 等，
  parser 把 `documents`/`pictures` 当 `location={hint:documents}`，期望 `location=None`（这里是**类型/复数名词**）。
- **冲突核心**：v0.5 有 **28 条把 `documents` 当位置**（`… in documents` = 文稿夹），parser 据此把
  `documents` 列为位置词。但 v0.9 这些是「documents = 类型」。**同一个词两种义，parser 无法纯靠词形分辨**。
- **选项**：
  - (a) **维持现状**：`documents` 优先位置义（保 v0.5 28 锚点 byte-equal），v0.9 这 ~13 条继续 partial。
  - (b) **上下文消歧**（改 parser）：句首/并列/带内容子句时判类型义、`in/里` 引导时判位置义。
    **高风险**——动 location parser，可能误伤 v0.5 那 28 条，须从干净基线单独起 task + 大量 byte-equal 验证。
  - (c) **re-baseline v0.5**：重新定义 `documents` 默认义。代价最大（动锁定基线）。
- **建议**：倾向 **(b) 但单独立项、严守 v0.5 28 锚点**；若你认为 ROI 不足，(a) 接受这 ~13 条永久 partial。
  **需你定方向**。
- **受影响**：约 13 条（v09-d2-en-005/007/010/011/013/016/019、d3-en-001/008/017/021、d5-en-001、d5-mixed-009）。

### C2. 多类型跨范畴查询的标注约定（array vs None，且 coverage 自相矛盾）

> **✅ 已落地（2026-06-20，BETA-13-G16）**：用户拍板 **Option C「≥2 file_type→ext=None」**。
> 全量分布证伪了「保留 ext」取向——**命名具体格式的多类型查询 13 条里 12 条本就 ext=None**（zh-005/014/015/035、
> en-014/015/025、mixed-004、d9-zh-002/003…），`word or powerpoint documents`（en-005）是**唯一**「保留 ext」孤例，
> 且语义上不对称多类型（如 `mp3 and mp4`）保留 ext 反而误导。落地=coverage 重标 2 条孤例对齐主流：
> en-005→ext=None、en-016→file_type=image（单范畴 png/jpg），parser 不动、0 byte-equal 风险。**+2 pass**。
> 同刀清相邻字段 backlog（en-010「code」/en-019「excluding」keyword 泄漏 + d5-mixed-009/d5-en-012 opened→accessed），
> 合计 **v0.9 871→877（+6）、§6 87.1%→87.7%**。

- **现象**：`documents and images`→期望 `['document','image']`（parser 给单值 `image`）；
  但 `pdf、ppt 和 excel 三种都找`→期望 `file_type=None`（parser 给 `['document','presentation','spreadsheet']`）。
- **冲突核心**：**coverage 自相矛盾**——同是「列举多类型」，一处要数组、一处要 None。需先统一标注约定，
  再决定 parser 是「全列数组」还是「≥N 类型→None（不收窄）」。G11 决策 A/B 处理过一部分，但残留这些边界。
- **建议**：需你定**多类型的统一语义**（推荐：≤3 类列数组、显式「都/三种」可特殊化为 None？）。定了我对齐 coverage + parser。
- **受影响**：约 6 条（v09-d2-en-007/010/013/016/019、d2-zh-035）。多数耦合 C1 的 location。

### C3. 特殊 sort 语义（relevance / oldest-first）

- **现象**：`find a song called Yesterday`→期望 `relevance_desc`（parser `modified_desc`）；
  `screenshots from yesterday`→期望 `relevance_desc`（parser `created_desc`）；
  `the oldest photos first`→期望 `created_asc`（parser `modified_asc`）。
- **冲突核心**：① 「精确点名 / 纯时间筛」时默认排序该是 relevance 还是时间序？v0.5 倾向时间序（见 A2）。
  这与 A2 张力相关——**需与 A2 一起定「默认 sort 何时 relevance、何时时间维度」总则**。
  ② `oldest first` → 该用 created_asc 还是 modified_asc？（维度选择）
- **建议**：把它并入 A2 的总则一起拍（A2 是「有时间维度就跟维度」，C3 是「无明确维度/点名时回 relevance」边界）。
- **受影响**：约 3 条（v09-d4-en-005、d4-en-024、d8-en-012、d5-en-022）。

---

## 路线建议

| 步骤 | 内容 | 预估 | 风险 | 需谁 |
|---|---|---|---|---|
| **第 1 刀** | Group A 全部（coverage 对齐 v0.5：A1+A2+A3） | +~20 pass（耦合解后） | 低 | 我执行，你批准方向 |
| **第 2 刀** | Group B1（content-clause 类型名词） | +~8 | 中 | 我执行 |
| **第 3 刀** | Group B2 + B3（文件夹位置 + size 排序） | +~10 | 中 | 我执行 |
| **决策点** | Group C1/C2/C3（你拍板后才能动） | +~22 | 高 | **你** |

**乐观估计**：A + B 落地后 835 → **~870（≈87%）**，**不需任何 v0.5 锁定基线改动**。
跨过 **90%** 的最后 ~3pp 卡在 Group C（尤其 C1 的 documents 消歧），那是真正需要你产品决策的部分。

**下一步取决于你**：① 批准 Group A 我立即开第 1 刀；② 对 Group C1/C2/C3 给方向（哪怕先定 C2/C3、C1 暂(a) 维持）。
