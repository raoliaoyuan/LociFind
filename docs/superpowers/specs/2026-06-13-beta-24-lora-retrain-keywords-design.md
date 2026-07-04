# BETA-24：LoRA 重训含 keywords 补全样本 + MediaSearch 内容词覆盖（设计 spec）

> 日期：2026-06-13（macOS 会话）
> 状态：已与用户逐节确认（数据生成 / MediaSearch 臂 / 训练与产物 / 测试验收四节均批准）
> 起因：BETA-23 实测——现 LoRA 模型对 keywords 待填输出空 patch `{}`（训练数据派生自 v0.5，keywords 从不是待填字段；few-shot 压不过微调）。接线已就绪（触发→prompt→并集→搜索全链路通），换上重训模型即生效。
> 任务编号：**BETA-24**（[ROADMAP §3.3](../../../ROADMAP.md) 卡片）。

## 1. 背景与现状

### 1.1 已就绪的部件（BETA-23 遗产）

- **触发器**：`fallback.rs::analyze_structural_omissions` 第七类「keywords 内容词覆盖检测」（仅 FileSearch 臂；MediaSearch 臂因 FileSearch 剥离词表未剥媒体框架词、v0.9 媒体模板误触发 +4.7% 被砍）。
- **hybrid 链路**：`apply_patch` keywords 并集语义（draft 在前去重）+ language 锁死；hybrid prompt 含 keywords 速查 + 问题 4 同型 few-shot。
- **桌面编排**：`search/model_fallback.rs` 状态机 + 懒加载 + 3s 超时 + 永不失败；默认模型路径 `app 数据目录/models/qwen3-0.6b-q4_k_m.gguf`。
- **训练管线**：`build_lora_dataset`（evals bin：Case → parser draft → patch diff → `build_hybrid_prompt` 产 JSONL，prompt 与推理期逐字节同源）→ `prepare_main_data.py`（chat 格式 + nonempty 8× 过采样）→ `run_bakeoff.sh`（mlx-lm LoRA → fuse → GGUF Q4_K_M）。
- **现役模型**：Qwen3-0.6B LoRA（BETA-17 winner，sha256 `898c98bc…17df`）；v1 超参配方（1000 step / lr 1e-4 / batch 4 / num-layers 16 / mask-prompt / seed 42）。

### 1.2 缺口

1. **keywords 补全样本量为零级**：v0.5 重跑生成器只能产出 1 条 keywords 触发样本（v0.9 也仅 2 条）——重跑解决不了样本量，必须新造。
2. **MediaSearch 臂覆盖检测缺席**：媒体侧复合查询的内容词遗漏现状不触发 fallback（漏触发回到现状，但媒体臂样本若不进本轮训练，未来启用时还得再训一轮）。
3. **`IntentDraft::from_query` 已走扩展后触发器**——新造的 case 跑生成器即自动产出 keywords 待填样本，管线无需结构性改动。

## 2. 范围决策（已与用户确认）

| 决策点 | 选择 |
|---|---|
| 范围 | **重训 + MediaSearch 臂覆盖检测一起做**（一次训练覆盖两臂，避免未来二次重训） |
| 训练数据 | **程序化生成 + Claude 手写混合**；**v0.9 全程保持干净**（不进训练，验收数字可信） |
| 验收门 | **三层**：held-out 合成集量化 + v0.9 零回退 + 真机问题 4 |
| 实现路线 | **方案 A**：扩展既有管线，数据是唯一训练变量（基座/超参全不动） |

## 3. keywords 补全训练数据生成

### 3.1 数据源（新 fixture，入库）

`packages/evals/fixtures/lora-aug-keywords/v1/cases.json`，与 evals Case 同 schema（id / query / language / variant / expected_intent），两部分组成：

1. **程序模板部分**：`fixtures.rs` 加子命令生成——模板组合「时间/位置/排序框架 × 内容词 × 触发形态」（「文件名包含X」短路型、复合内容词型、媒体内容词型），内容词来自合成词表（会议纪要 / 周报 / 体检报告 / 运维 / annual budget …），构造期即知 expected keywords。**两臂都造**（FileSearch + MediaSearch）。
2. **Claude 手写部分**：子 agent 按 BETA-13 标注纪律（语义轴按意图标、约定轴查约定表）撰写自然口语变体——防 v0.5 式模板过拟合（v0.5 教训：模板 query 94% vs 自然 query 8%）。

### 3.2 转训练 JSONL

复用 `build_lora_dataset`，新增 `--require-fillable keywords` 过滤——只保留 parser 真触发 keywords 的 case（不触发的 case 推理期到不了模型，训进去反而教模型补没被要求的字段）。patch 形态约束沿用：不含 `intent` / `schema_version`。

### 3.3 held-out 切片

生成期按 id **确定性**切出 ~20%（约 80-100 条）永不进训练，作第一层验收的量化集。切片与训练份零重叠由测试断言。

### 3.4 规模与隐私

- keywords 触发样本 ~300-400 条（两臂合计，训练份）；与既有 v0.5-patch（empty 443 + nonempty 55×8）合并后三类大致均衡，过采样系数实现期按比例调。
- 词表全合成，无真实文件名/路径/搜索词（[training/datasets README](../../../training/datasets/README.md) 强制项）；数据集元信息（dataset_name / version / source / sha256 / generation_method / privacy_review_status …）照登记。

## 4. MediaSearch 臂内容词覆盖检测（parser 纯函数）

### 4.1 改动点

`fallback.rs::analyze_structural_omissions` 的 MediaSearch 臂启用 `has_uncovered_content` 检查，剥离词表在 FileSearch 基础上**叠加媒体噪声词表**——songs / by / 一首 / 几首 / 无损 / 高品质 / 时长 / 专辑 / 单曲 / 播放 / tracks / playlist 等媒体框架词（BETA-23 实测正是这些词没剥导致 +4.7% 误触发）。

覆盖值口径不变：残留段对照 keywords / artist / album / title / genre / location.hint 双向子串覆盖（BETA-23 spec §3.2 既有逻辑，媒体字段本就在清单内）。

### 4.2 两条硬门

1. **parser-only byte-equal 不动**：本改动只碰触发器路径，`parse()` 本体零改动——v0.5 473 / v0.9 726 机械保证。
2. **误触发门**：fire-rate 报告 before/after——v0.9 媒体模板里 parser 已全覆盖 case（「周杰伦的歌」「songs by Taylor Swift」类）**新增触发 ≈0**（对照被砍时 +4.7% 基线）；媒体臂触发须集中在真遗漏 case。词表调不到位就收紧到达标为止；**调不平则该臂保持关闭并如实记录**（同 BETA-23 砍臂先例，不硬上）。

## 5. 训练与产物

- **单一变量铁律**：基座 Qwen3-0.6B + v1 超参（1000 step / lr 1e-4 / batch 4 / num-layers 16 / mask-prompt / seed 42）全部不动，唯一变量 = 训练数据并入 keywords-aug。
- `prepare_main_data.py` 升级多源输入：v0.5-patch（empty + nonempty×8 现状）+ keywords-aug 训练份；过采样系数作脚本常量、确定性 seed。
- 训练跑法沿用 `run_bakeoff.sh` 模式（新增 `run_beta24.sh` 或参数化）；产物 `adapters/beta24-qwen3-0.6b/` + `fused/beta24-qwen3-0.6b-q4_k_m.gguf`，sha256 登记 releases 文档。
- **部署即换文件**：fused GGUF 改名放 `models/qwen3-0.6b-q4_k_m.gguf`（desktop 期望文件名不变，**桌面侧零代码改动**）。
- 首轮不达标按既有哲学迭代 2-4 轮（按 held-out 错误补样本），每轮如实记录。

## 6. 测试与验收

### 6.1 单元/集成测试

- parser：媒体臂触发正/反例集 + 中英混合；既有 keywords 触发测试零回归。
- 数据生成器：模板生成确定性、`--require-fillable` 过滤行为、held-out 与训练份零重叠断言、patch 形态合法。
- fmt + clippy（`-D warnings`，feature 开/关两形态）双 0；全 workspace test 零回归。

### 6.2 评测三层验收

1. **held-out 合成集量化**：~80-100 条切片跑 with-fallback hybrid，keywords 补全准确率 **≥80%**（现状空 patch = 0%）；逐 case 报告。
2. **v0.9 全集**：parser-only byte-equal 726 不动（硬门）；with-fallback **regressions=0**（红线）+ keywords/media 触发 case 净增益 ≥0 逐 case 列出；fire-rate before/after（媒体臂误触发门 ≈0）。v0.5 同口径 473 不动。
3. **真机问题 4**：「2025年的会议纪要文件名包含运维」端到端——触发 → 模型补出「会议纪要」→「模型补全」徽标 → 命中目标文件。

### 6.3 性能门

触发 case p95 ≤3s 不变（模型同尺寸同量化，预期与 BETA-23 实测 601ms 同级，记录实测值）。

### 6.4 如实记录纪律

任何一层不达标不凑数：held-out 不到 80% 按轮次迭代并记录每轮数字；媒体臂调不平保持关闭如实登记。

## 7. 非目标（防范围蔓延）

- 不换基座 / 不调超参（基座升级留未来独立 bake-off）。
- 不动 `parse()` 本体、不动 v0.5/v0.9 锚点（parser language 检测缺陷的 re-baseline 决策维持 ROADMAP 登记不动手）。
- 不做应用内模型下载/分发（BETA-25 及后续）。
- 不拿 v0.9 进训练（评测集污染红线）。
- 不做 embedding 召回补强（留 BETA-15B）。

## 验证后记

### Task 2：媒体臂 fire-rate 误触发门（v0.9 before/after 量化）

**测法**：`evals --fixtures v0.9 --fire-rate`（纯 parser，无模型）。before = Task 1 父提交 `561044a`（媒体臂关），after = 词表收紧后 HEAD。

**总数对比**：

| 指标 | before（媒体臂关） | after Task 1（媒体臂开，词表未收紧） | after 词表收紧（最终） |
|---|---|---|---|
| fire-rate | 269/1000 (26.9%) | 288/1000 (28.8%) | 281/1000 (28.1%) |
| omission.keywords | 2 | 42 | 32 |
| 新增触发 id 数 | — | 19 | 12 |

before 与 BETA-23 实测基线一致（269 / keywords=2），基线可信。

**首轮（Task 1，词表未收紧）19 条新增触发逐条审**：9 条真遗漏 + 6 条误触发 + 4 条真遗漏重复模板（`v05-media-template-286/290/294/298` 同 query「find songs by synthetic-artist」，parser artist=null → 真遗漏）。

6 条**误触发**（parser 已全覆盖内容，残留纯属未剥的媒体框架词）：

| id | query | 残留泄漏词 | 判定理由 |
|---|---|---|---|
| v09-d4-en-014 | find high quality songs | high / quality | quality=high 已捕获，品质框架词未剥 |
| v09-d4-en-016 | songs longer than 5 minutes | longer | duration 已捕获，时长框架词未剥 |
| v09-d4-en-017 | audio files longer than 2 hours | longer / hours | 同上，含单位 hours |
| v09-d4-en-020 | high quality songs by Bruno Mars | high / quality | artist+quality 均已捕获，品质框架词未剥 |
| v09-d4-zh-011 | 来点摇滚歌曲 | 来点 | genre=摇滚 已捕获，口语框架词「来点」未剥 |
| v09-d4-zh-016 | 无损格式的歌曲 | 格式 | quality=lossless 已捕获，「格式」未剥 |

另有 `v09-d4-zh-002`「放首李荣浩唱的」首轮触发（残留「放首/唱」），parser 已捕获 artist=李荣浩 → 亦判误触发（口语框架词「放首」未剥）。

9 条**真遗漏**（parser 真漏内容词，媒体臂正确捕获）：synthetic-artist（artist=null）、Shape of You by Ed Sheeran（漏 artist=Ed Sheeran）、Taylor Swift 的无损歌曲（漏 artist）、周杰伦《范特西》（artist 误抽「专辑里」、漏周杰伦）、陈奕迅 浮夸（漏 artist=陈奕迅）、薛之谦《绅士》专辑（漏 artist=薛之谦）、王菲的爵士风格歌曲（漏 artist=王菲）、毛不易《消愁》（漏 artist=毛不易）、Taylor Swift 的歌（artist 仅抽到「Swift」、漏 Taylor）。

**词表迭代（1 轮）**：向 `MEDIA_COVERAGE_NOISE_WORDS` 补入泄漏的框架词——英文 `quality / longer / hours / high`，中文 `格式 / 来点 / 放首`（维持字符数降序）。收紧后重测：

- 新增触发 19 → 12，**6 条误触发 + 放首李荣浩唱的全部消除**，12 条全为真遗漏。
- omission.keywords 42 → 32；fire-rate 288 → 281。
- 真遗漏不受影响（artist 残留段仍触发）；逐条 trace 复核 6 条误触发 query 残留段全空。

**最终判定**：**达标，零误触发**（迭代 1 轮）。对照 BETA-23 砍臂时 +4.7%（≈47 条媒体模板误触发），本轮净增 12 条全集中在真遗漏，媒体臂保持启用。

**回归门**：parser-only byte-equal v0.5=473 / v0.9=726 不动；`cargo fmt --check` / `clippy -D warnings` / `cargo test -p locifind-intent-parser` 全绿。新增 6 条误触发 query 已固化进 `media_framework_words_do_not_trigger_keywords_omission` 反例集防回归。

**代码改动**：`packages/intent-parser/src/fallback.rs`——`MEDIA_COVERAGE_NOISE_WORDS` 补 7 词 + 媒体框架反例测试扩 6 条。

### Task 8-9：重训 + 三层验收（含回归修复）

**训练（Task 8）**：基座 Qwen3-0.6B + v1 配方完全不动，唯一变量=训练数据并入 lora-aug-keywords（empty 443 + v0.5-nonempty 55×8 + keywords-aug 122×3=366，total 1249）。val loss 0.001（与 BETA-17 同级收敛），GGUF 378MB，sha256 `3aef6efba88316786d3128a0a19599573eaeafffad492633ab543a1089650e0a`。复现：`bash training/mlx-lora/scripts/run_beta24.sh`。

**第一层 held-out（核心新能力，达标）**：80 条 held-out 切片中 30 条（永不进训练）跑 with-fallback hybrid——**keywords 补全 pass 27/30 = 90.0%**（parser-only 基线 0% pass / 30 全 partial）。**模型从「永远输出空 patch `{}`」→ 90% 补对 keywords**，BETA-24 核心目标达成；fallback_invoked 30/30、valid_intent 30/30，无退化解。

**第二层 v0.9 回归门（红线，修复后达标）**：首轮 with-fallback **86 个回归**（pass 726→640）——重训后模型对 keywords **过度积极**，在 keywords 不该填的 size/sort/time 查询上幻觉关键词（「找最大的的视频」吐「大小单位/热门」）。诊断：84/86 是 keywords 字段，75 个 keywords∉fillable（模型违反 hybrid 契约）。**三处原则性修复**（非重训）：
1. **apply_patch keywords 契约强制**（commit `a3702cc`）：keywords∉fillable 时丢弃模型 keywords → 86→13。
2. **apply_patch PARSER_OWNED_FIELDS denylist** + **媒体臂限定 media_type=Audio** + **has_uncovered_content 补剥类型词/曲风/框架词**（commit `785ba8e`）：13→0。
   - denylist（extensions/file_type/exclude_*/options/question/reason）：模型无 fillable 类别的字段一律不应填。
   - 媒体臂 audio-only：自由主题词只在音乐查询有意义，screenshot 内容 v0.9 标注不作 keywords（held-out 媒体全是 audio，零损失）。
   - 框架词剥离：FILE_TYPE_NOISE（文档/视频…）+ MEDIA_GENRE_NOISE（古典/摇滚…）+ COVERAGE 补「开头/结尾」+ MEDIA_COVERAGE 补「放点/放些」——parser 残留碎片误触发的根因。

   **最终 v0.9 with-fallback：REGRESSIONS=0**（pass 726 = baseline parser-only）。

**第三层 v0.5 + byte-equal（达标）**：v0.5 with-fallback REGRESSIONS=0（pass 473）；parser-only byte-equal v0.5=473 / v0.9=726 全程不动。

**性能门（达标）**：触发 281 条，p50=46ms / p95=121ms / max=1170ms（Metal，≤3s 门大幅余量）。

**质量门**：parser 153 单测全过（含降序三表不变量 + 非audio负向 + apply 契约/denylist 测试）；fmt + clippy（`--workspace -D warnings`）+ 全 workspace test 零回归；held-out 90% 经回归修复后保持不变。

**关键洞察**：BETA-24 暴露并修复了 BETA-23 `apply_patch` 的契约漏洞——它不校验模型输出是否在 fillable 范围内（BETA-17 模型总输出 `{}` 从未暴露）。修复使系统对模型过度积极鲁棒：**模型只能填被要求填的字段**。问题 4 最后一公里达成，接线就绪、换上本模型即生效。
