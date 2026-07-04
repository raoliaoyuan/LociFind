# BETA-15B-7：embedding 模型跨族 + 同族最大档探针（qwen3-0.6b vs bge-m3 vs qwen3-8b）设计 spec

> 承接 BETA-15B-6 v3（content-not-name 桶 30 例 + T\*=0.70 鲁棒、A-5「cosine 单维真破局」结论 v3 进一步确认、认知层主动放弃字面 0.864 / 0.700 spec 目标、移交下 cycle 抓手 = **更大 embedding 模型**）；本 cycle 用数据指证两条独立轴上的 embedding 模型对 cosine 单维天花板的影响：① **跨族架构轴**（同尺寸 vs 0.6b 的 bge-m3、多语言 SOTA、专攻 CN-EN crosslang）；② **同族最大档轴**（qwen3-embedding-8b、Qwen3 系列最大、MTEB 多语言 SOTA 之一）。
>
> **修订说明**：spec 起草时基于错误前提「Qwen3-embedding 系列有 0.6b/1.5b/4b 三档」起初稿；起草后核查发现 Qwen3-embedding 实际为 **0.6b / 4b / 8b** 三档（不存在 1.5b、且 4b/8b 是 HuggingFace gated repo 需登录 + 接 license）。同时新发现 **bge-m3** 是被严重低估的跨族候选——与现 qwen3-0.6b 同尺寸（605 MB ≈ 610 MB）、专攻多语言 retrieval、HuggingFace 下载量 ~3,156 万（vs qwen3-0.6b 18.5 万 = 170×）、广泛工业部署验证。修订后用 **bge-m3 + qwen3-8b 双点** 替代原「qwen3-4b + qwen3-8b 双点」——同时验证「跨族架构」与「同族最大档」两个独立抓手轴、信号更纯净、cycle 时长更可控。
>
> **范围哲学**：纯评测探针。生产 wiring / 模型分发 / baseline.json bake 全部不动。本 cycle 唯一改的 production 代码 = `packages/evals/src/bin/semantic_quality.rs` 加 `--vectors-file` flag（向下兼容、零行为变化、与现 `--embed --semantic-weight --cosine-threshold` 等 flag 同款风格）。
>
> **与 BETA-15B-4 的关系**：ROADMAP §3.3 BETA-15B-4 名义包含「Windows embedding + 媒体/OCR 臂 + sqlite-vec+int8 + **探更大模型天花板**」四独立子项；本 cycle 把第 4 项「探更大模型天花板」拆为独立 sub-cycle ID **BETA-15B-7**（与 BETA-15B-3 A 簇 / BETA-15B-6 v2/v3 同款拆法、命名归位）。BETA-15B-4 余三项（Windows + 媒体/OCR + sqlite-vec）不动、仍归 BETA-15B-4 not_started。

## 1. 背景与动机

### 1.1 v3 cycle 留下的天花板

BETA-15B-6 v3 cycle 在 78 cases / 124 docs / dim 1024 / qwen3-embedding-0.6b-q8_0 数据集上 sweep 9 阈值选定 **T\*=0.70（Branch A 命中、v2 bake 鲁棒不动）**：

- OVERALL HYBR_N **0.856 < 0.864 spec 目标**（差 -0.008、本 cycle 真正抓手）
- crosslang HYBR_N **0.717 > 0.700 spec 目标**（+0.017）✓ **已达**
- content-not-name HYBR_N **0.870**（v3 比 v2 +0.017 显著升、二次扩量后真水位反而升）

v3 cycle 已在 baseline 报告中**主动放弃字面 0.864 / 0.700 spec 目标的字面追求**，认知层结论：

> 在「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下，0.864 OVERALL 目标**结构性不可达**；移交下 cycle 抓手 = 更大 / 更强 embedding 模型（极高优、唯一剩余天花板抓手）。

**关键事实**：crosslang ≥ 0.700 在 A-5 cycle 已达且 v2/v3 上鲁棒、本 cycle 唯一真正抓手是 OVERALL ≥ 0.864（差 0.008）。

### 1.2 本 cycle 的命题

> **两条独立轴上的更强 embedding 模型，能否突破 OVERALL 0.864 / cosine 单维天花板？**
>
> - **跨族架构轴**：bge-m3（BAAI、~568M、多语言专精）
> - **同族最大档轴**：qwen3-embedding-8b（Qwen 官方、~8B、Qwen3 系列最大）

v4 cycle 通过两条独立轴的对照实验把「模型」拆成两个变量做单独验证。**字面 spec 目标在 v4 cycle 复活**作为「GO 候选」判定指标——这就是本 cycle 存在的意义；不复活就没有判定基准。

### 1.3 为什么选 bge-m3 + qwen3-8b 双点

> **修订（v4 cycle 末实测后）**：本节 rationale 是**起草时**的模型选型理由（多语言 SOTA、MTEB 顶榜、HF 下载量量级等）。**实测两候选均落 Branch IV-infra**（bge-m3 因 pooling 类型 model-runtime 硬编码 Last 与 bge-m3 GGUF 声明的 CLS 错配、qwen3-8b 因 llama-cpp-4 + 8B 推理 bug 全零向量）—— **本节 rationale 留作 follow-up infra fix 后重跑 cycle 的入口参考**，不构成对模型层能力的有效结论。完整 v4 实测 + 诊断见 [baseline 报告 v4 节](../../reviews/semantic-recall-quality-baseline.md#v4-数据集节--embedding-模型跨族--同族最大档探针beta-15b-7)。BETA-15B-8 cycle fact-check 校正：`bert.pooling_type=2` 实际是 CLS（非起草时误标的 MEAN）；修复方向不变（硬编码 Last 错配 bge-m3 声明的 CLS）。

#### bge-m3（跨族架构轴）

- **同尺寸**：q8_0 GGUF ~605 MB ≈ 现 qwen3-0.6b 610 MB（**潜在零分发成本破局**——若 bge-m3 在我们任务上比 qwen3-0.6b 强，根本不需要更大模型，分发包大小不变）
- **多语言 SOTA**：BAAI 2024 发布、专门为多语言 retrieval 设计、CN-EN crosslang 业界长期标杆
- **广泛验证**：HuggingFace 下载量 3,156 万、远超 qwen3-embedding-0.6b 的 18.5 万（170×）；M3 = Multi-Lingual + Multi-Functionality + Multi-Granularity 三轴覆盖
- **公开免登录**：`gpustack/bge-m3-GGUF` 公共仓库可直接 curl
- **GGUF 格式**：q8_0 量化、llama-cpp-4 推理栈直接复用、零基础设施改动

#### qwen3-embedding-8b（同族最大档轴）

- **同厂同系列**：与现 qwen3-embedding-0.6b 同来源（Qwen 官方 GGUF q8_0）、同族缩放对照实验中量化 / 训练数据 / tokenizer 不是干扰变量
- **MTEB 多语言 SOTA**：Qwen3 系列最大档、官方报告在 MTEB 多语言基准上 top-3
- **充当上限锚**：若 bge-m3 不能破局、qwen3-8b 能告诉我们「Qwen3 单族的真实上限」、定调下 cycle 抓手（要不要跨族 + 训练？）
- **q8_0 GGUF 大小**：~7.7 GB（需 HF login + license accept、但本机已就绪）

#### 决策完整性

| 实测情景 | 解读 | 下 cycle 抓手 |
|---|---|---|
| **bge-m3 双过 spec 目标** | 跨族架构在同尺寸上已破局 | bake bge-m3 推到生产、零分发成本 |
| **bge-m3 不过 + qwen3-8b 双过** | 同族放大有效但跨族 SOTA 不行（特定任务 fit） | 接受 8b 分发成本、follow-up cycle 推到生产 |
| **bge-m3 不过 + qwen3-8b 不过** | 两条独立轴都不行 = cosine 单维真见顶 | 换抓手：跨族 + 更大（bge-multilingual-gemma2 9B） / 评测扩量 / 微调 |
| **bge-m3 过 + qwen3-8b 不过** | 跨族架构胜过同族放大 = 训练数据/架构 > 缩放 | bake bge-m3、收 8b 实验得诚实边界 |

#### YAGNI 拒绝的备选

- ❌ **qwen3-embedding-4b**：原 spec 中档、但 bge-m3 占了「同尺寸跨族架构」位、4b 落在「同族中档」位、信息冗余（8b 已锚同族上限）；YAGNI 守住三点结构
- ❌ **EmbeddingGemma-300M**（Google 2025-Q3）：半尺寸候选有趣但与 bge-m3 同属「跨族」轴、加它会变多变量
- ❌ **jina-embeddings-v3 / v5**：跨族但 multilingual MTEB 表现 vs bge-m3 没有压倒性优势、验证少
- ❌ **bge-multilingual-gemma2**（9B）：跨族 + 大、范围过大、若 bge-m3 + qwen3-8b 双不过再上

## 2. 目标与验收

### 2.1 目标

- 下载 **bge-m3-Q8_0.gguf**（~605 MB、公开免登录、`gpustack/bge-m3-GGUF`）+ **qwen3-embedding-8b-q8_0.gguf**（~7.7 GB、需 HF login + license）放 `models/`
- `packages/evals/src/bin/semantic_quality.rs` 加 `--vectors-file <path>` flag（向下兼容、默认仍 `packages/evals/fixtures/semantic-recall/vectors.json`）
- Mac Metal `--embed` 跑 bge-m3 → 产 `packages/evals/fixtures/semantic-recall/vectors-bge-m3.json`
- Mac Metal `--embed` 跑 qwen3-8b → 产 `packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json`
- `cp vectors.json vectors-qwen3-0.6b.json`（命名归位、方便 T4 三模型同款 path 模式 sweep）
- 原 `vectors.json`（0.6b 内容）**字节不动**
- 跑 9 阈值 sweep × 3 模型 = 完整决策矩阵（6 桶 × 9 阈值 × 3 模型 nDCG）
- 按四 Branch 决策表（§2.3）判定结果、记录数据指证
- baseline 报告**追加** v4 节（不重写 v3 节、与 v2/v3 累加同款）：三模型 sweep 全表 + Branch 判定 + 下 cycle 抓手优先级修正

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed
(2) `cargo clippy --workspace --all-targets -D warnings` 0 warning
(3) `cargo fmt --all --check` 净
(4) 回归门 `semantic_quality_gate` 用**现 baseline.json**（v3 0.6b 数据）pass、4 红线全过、本 cycle 不改 baseline.json、不改 gate.rs 任何断言代码
(5) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser）
(6) `--vectors-file` flag 向下兼容：不带 flag 时所有现有 binary 行为字节一致（默认值 = 现 `vectors.json` 路径）
(7) 新 vectors-bge-m3.json + vectors-qwen3-8b.json + vectors-qwen3-0.6b.json **零 PII**：纯模型 embedding、对象是合成集 124 doc + 78 query，结构上不可能含 PII，但 commit 前仍按 BETA-15B-6 README 自查清单第 (6a) 项过一遍（防误把 corpus.json 改动一起塞进）

### 2.3 四 Branch 决策表（GO 判定）

| Branch | 字面 spec 目标 | 各桶 ≥ 0.6b HYB baseline | 行动 |
|---|---|---|---|
| **I-a ⭐ bge-m3 GO** | **bge-m3** 满足：OVERALL ≥ 0.864 **且** crosslang ≥ 0.700（无论 qwen3-8b 是否双过） | 全过 | **首选 bake bge-m3**（同尺寸零分发成本、跨族架构破局）；开 follow-up cycle BETA-15B-7-v2 推到生产 wiring（替换 `DEFAULT_EMBEDDING_MODEL_PATH`） |
| **I-b ⭐ qwen3-8b GO** | bge-m3 不达 **且** qwen3-8b 双过 | 全过 | **bake qwen3-8b**、接受 ~7.7 GB 分发成本；开 follow-up cycle BETA-15B-7-v2 推到生产 + 模型分发 UX（首次启动下载、进度条、离线 fallback） |
| **II NO GO** | 仅某一桶 OVERALL 过、crosslang 在新模型上反退 < 0.700 | 全过 | **不 bake**——任一新模型让 crosslang 退步则可疑（v3 已 0.717 ≥ 0.700）；记录数据、移交下 cycle 抓手 = 评测扩量 + 更大跨族（bge-multilingual-gemma2 9B） |
| **III 见顶** | 双不过字面 spec 目标 | 全过 | 认知结论 = **cosine 单维 + 现合成集组合下、两条独立轴都见顶**；移交下 cycle 最高优 = 评测扩量（专攻 OVERALL 弱桶）+ 跨族更大（bge-multilingual-gemma2 9B、~9 GB） / Linq-Embed-Mistral / 更新代 jina-v5 |
| **IV 异常** | 任意 | 任一桶破 ≥ HYB baseline | **不应发生**——更强模型反而退步说明 bug（vectors 文件错位 / 模型加载错 / dim 计算错），调查不发布、不写 baseline 报告 v4 节、留 STATUS 异常记录 + 下次会话排查 |

**v4 cycle 真正目标 = Branch I-a（bge-m3 GO，最优路径）或 I-b（qwen3-8b GO，次优路径）**。Branch II/III 仍接受合并（数据指证有价值、下 cycle 抓手定调）。Branch IV 是 bug 信号、阻止合并。

**"GO 候选" 不等于 bake**：本 cycle 是探针、即便 Branch I-a/I-b 命中也**不**改 `DEFAULT_EMBEDDING_MODEL_PATH` / 不动 desktop wiring / 不动模型分发。bake 推到生产是 follow-up cycle BETA-15B-7-v2 的事，需独立 spec / plan / 模型分发 UX 设计 / Windows 适配。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- **下载 2 个模型 GGUF**：① `gpustack/bge-m3-GGUF/bge-m3-Q8_0.gguf`（~605 MB、公开免登录、curl 直拉）；② `Qwen/Qwen3-Embedding-8B-GGUF/Qwen3-Embedding-8B-Q8_0.gguf`（~7.7 GB、需 HF login + license accept、curl 带 `Authorization: Bearer <HF_TOKEN>` header）；放 `models/` 目录、文件名规范化为 `bge-m3-q8_0.gguf` + `qwen3-embedding-8b-q8_0.gguf`（与现 `qwen3-embedding-0.6b-q8_0.gguf` 命名风格一致）
- **改 `semantic_quality.rs` binary**：加 `--vectors-file <path>` flag（默认 `packages/evals/fixtures/semantic-recall/vectors.json`），同时影响 `--embed`（输出位置）和 sweep（输入位置）；clap 解析 + 默认值常量 + 单测覆盖向下兼容
- **Mac Metal `--embed` × 2**：① `--model models/bge-m3-q8_0.gguf --vectors-file packages/evals/fixtures/semantic-recall/vectors-bge-m3.json --embed`；② 同款命令换 qwen3-8b；产 2 个新 vectors 文件、原 `vectors.json` 不动
- **`cp vectors.json vectors-qwen3-0.6b.json`**：命名归位（T4 三模型同款 path 模式 sweep）
- **9 阈值 sweep × 3 模型**：对每个 vectors 文件跑现有 9 阈值（0.0 / 0.30 / 0.45 / 0.60 / 0.70 / 0.80 / 0.90 / 0.99 / 1.01），产 6 桶 × 9 阈值矩阵 × 3 模型
- **决策表落地**：人工读三模型表选 Branch I-a/I-b/II/III/IV、写决策依据
- **baseline 报告追加 v4 节**：三模型 sweep 全表 + Branch 判定 + 下 cycle 抓手优先级修正 + Branch I-a/I-b 时附 follow-up cycle BETA-15B-7-v2 的工作清单提纲
- **README.md v4 更新**：仅加 1-2 行注明 v4 模型探针节存在、链到 baseline 报告 v4 节、不重写 v3 节

### 3.2 Out-of-scope（明确 YAGNI）

- ❌ 改 `DEFAULT_EMBEDDING_MODEL_PATH` / 任何 desktop 生产 wiring（推到生产是 follow-up cycle BETA-15B-7-v2）
- ❌ 改模型分发 / `tauri.conf.json` / app bundle / 首次启动 UX
- ❌ 改 `baseline.json`（v3 0.6b 数值不动、当下 gate 仍守护 0.6b）
- ❌ 改 `gate.rs` 任何红线断言代码或 doc 注释
- ❌ 改 `DEFAULT_COSINE_ROUTING_THRESHOLD`（v3 bake 的 0.70 不动）
- ❌ 改 `result-normalizer` wrapper / `harness` / 任何融合层代码
- ❌ 改 cases.json / corpus.json（沿用 v3 数据集 78 cases / 124 corpus）
- ❌ qwen3-embedding-4b（YAGNI、bge-m3 占了「同尺寸跨族」位、8b 占了「同族最大档」位、4b 落「同族中档」位信息冗余）
- ❌ EmbeddingGemma-300M / jina-v3 / jina-v5（跨族但与 bge-m3 同轴、加它们变多变量）
- ❌ bge-multilingual-gemma2 9B（更大跨族、若 Branch III 见顶后才作下 cycle 候选）
- ❌ 同时跑多量化档对比（q4_K_M / q4_0、量化对照实验是独立 cycle）
- ❌ LoRA 微调 / 全参微调（YAGNI、本 cycle 测「模型」而非「微调」、eval 集 78 query 不足以 fine-tune）
- ❌ 改 evals binary `--embed` 路径默认值（默认值不动、仅加新 flag）
- ❌ 改 `semantic_quality_gate` 集成测试逻辑（gate 仍读默认 `vectors.json` = 0.6b 内容）
- ❌ 真实集（gitignored）校准（本 cycle 是合成集探针、与真实集解耦）
- ❌ Windows / Linux 平台跑 embed（本 cycle Mac Metal only、Windows GPU 在 BETA-15B-4 范围）

## 4. 实施步骤

| Task | 工作内容 | 验证 |
|---|---|---|
| T1 | ① 下载 bge-m3 q8_0（公开仓库 `gpustack/bge-m3-GGUF`、curl 直拉）；② 下载 qwen3-8b q8_0（gated `Qwen/Qwen3-Embedding-8B-GGUF`、curl 带 HF_TOKEN auth header）；规范化为 `bge-m3-q8_0.gguf` + `qwen3-embedding-8b-q8_0.gguf`；放 `models/`；记录 SHA256 | `ls -lh models/*.gguf` 列出 4 个文件（含现有 0.6b 等）；新 2 个文件大小符合（~605 MB / ~7.7 GB） |
| T2 | `semantic_quality.rs` 加 `--vectors-file <path>` flag：clap 解析、默认值 `vectors.json`、文档 doc 注释、`--embed` 路径分支用新参、sweep 路径分支用新参 | 单测 2 条：① 不带 flag 时默认值 = "vectors.json"；② 带 flag 时 override；外加 `--help` 实跑显示 flag |
| T3 | Mac Metal 跑两次 `--embed`：① `--model models/bge-m3-q8_0.gguf --vectors-file vectors-bge-m3.json --embed`；② 换 qwen3-8b。跑完 `cp packages/evals/fixtures/semantic-recall/vectors.json packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json`（命名归位、方便 T4 三模型同款 path 模式 sweep） | 产 3 个 vectors 文件（含 copy 出来的 0.6b 命名归位文件）、`jq '.dim, .model_id' vectors-{bge-m3,qwen3-8b,qwen3-0.6b}.json` 显示 dim 字段（0.6b = 1024 已知、bge-m3 实测大概率 1024、qwen3-8b 实测大概率 4096、最终值入 baseline 报告 v4 节）+ model_id 含正确 GGUF 文件名；现 `vectors.json` **byte-equal 不变**（`git diff packages/evals/fixtures/semantic-recall/vectors.json` 净） |
| T4 | sweep 9 阈值 × 3 模型 = 跑 3 次 `semantic_quality --vectors-file vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json --json` 各 9 次阈值（vectors-qwen3-0.6b.json 由 T3 末尾 copy 而来、内容 ≡ 现 vectors.json） | 完整三模型 9 阈值 6 桶 nDCG 表落进 baseline 报告 v4 节 |
| T5 | 按 §2.3 决策表读表选 Branch、记决策依据 | 写出 Branch I-a/I-b/II/III/IV 判定 + 依据；若 Branch IV 异常则停在此 step、写排查记录、不进 T6/T7 |
| T6 | baseline 报告追加 v4 节：① 模型清单 + 维度 + 文件大小（含跨族架构 bge-m3 与同族 0.6b/8b 区分）；② 三模型 sweep 全表（与 v3 节同款表格风格）；③ Branch 判定 + 数据指证；④ 下 cycle 抓手优先级修正（Branch I-a/I-b → follow-up bake cycle、II → 评测扩量 + bge-multilingual-gemma2、III → 评测扩量 + 跨族更大、IV → bug 排查不发布）；⑤ Branch I-a 或 I-b 时附 follow-up cycle BETA-15B-7-v2 工作清单提纲（推 default 模型、Mac/Windows 真机手测、模型分发 UX、暖机时长重测） | baseline 报告 v4 节通过整体 reviewer 审、与 v2/v3 节风格一致 |
| T7 | README.md v4 注脚 + 总验收：① `cargo test --workspace` 0 failed；② clippy 0 warning；③ fmt 净；④ `cargo test -p locifind-evals --test semantic_quality_gate` 1 passed；⑤ evals parser-only byte-equal v0.5=473 / v0.9=877 精确不变；⑥ vectors.json byte-equal 不变 | 全 6 项验收过、写 commit message、准备 PR |

## 5. 异常分支降级

**Branch IV（更强模型反而退步）排查清单**：

- 检查 vectors-{bge-m3,qwen3-8b}.json 的 `model_id` 字段是否对应正确 GGUF
- 检查 `dim` 字段是否合理（0.6b=1024、bge-m3≈1024、qwen3-8b≈4096；若反小说明加载错）
- 检查 doc_vectors / query_vectors 长度是否 = 124 / 78（缺失说明 embed 中断）
- 检查 vector 是否归一化（cosine 计算前提）：抽样 10 个向量算 L2 norm、应接近 1.0
- 检查 GGUF 文件 SHA256 是否与 HuggingFace 官方一致（防下载损坏）
- 若所有检查过仍退步 → 记 baseline 报告 v4 节「Branch IV 异常调查无结论、不发布 + 留 STATUS 异常记录 + 下次会话深排」

## 6. 验证矩阵

| 验证项 | 命令 | 期望 |
|---|---|---|
| workspace test | `cargo test --workspace` | 0 failed（与 v3 同款 860 passed 基线、`semantic_quality.rs` 加 flag 后新增 2 单测、约 862 passed） |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings` | 0 warning |
| fmt | `cargo fmt --all --check` | 净 |
| evals gate | `cargo test -p locifind-evals --test semantic_quality_gate` | 1 passed（gate 仍读默认 vectors.json = 0.6b 内容、4 红线全过） |
| parser byte-equal | `cargo run -p locifind-evals --bin parser_eval -- run v0.5 / v0.9 --json` | v0.5=473/25/2 + v0.9=877/119/4 精确不变 |
| vectors byte-equal | `git diff packages/evals/fixtures/semantic-recall/vectors.json` | 净（0.6b 内容不动） |
| `--vectors-file` flag | 单测 2 条 + 实跑两种情景 | 默认值回退 + 自定义路径 |

## 7. Mac 真机手测

**结论：不安排手测剧本**（与 v1/v2/v3 同款）。理由：

- 本 cycle 范围 = 纯评测探针、零 production wiring 改动、零 UI 改动
- 改的唯一 production 代码 = `semantic_quality.rs` binary 加 flag，向下兼容、零行为变化
- 不改 desktop / 不改 `DEFAULT_EMBEDDING_MODEL_PATH` / 不改模型分发
- 评测层端到端覆盖（sweep + gate 1 passed）即为本 cycle 真正的「真机验证」

若 Branch I-a 或 I-b 命中 → follow-up cycle BETA-15B-7-v2 必含 Mac + Windows 真机手测剧本（暖机时长重测、首查询冷启动、温查询延迟、跨语言召回真机命中、bge-m3 / qwen3-8b 模型加载延迟）。

## 8. 风险与 mitigation

| 风险 | 概率 | 影响 | mitigation |
|---|---|---|---|
| HuggingFace 拉 qwen3-8b ~7.7 GB 慢 / 中断 | 中 | 进度阻塞 | 用 `curl --retry 5 --retry-delay 10 -C -` 断点续传；`run_in_background` 模式启动、定期 `ls -lh` 看进度 |
| HF_TOKEN 失效 / license 未接 | 低 | qwen3-8b 401 | T1 step 0 先做 HEAD 探针验 auth 通；若 401 提醒用户去 HF web 接 license |
| `--vectors-file` flag 引入 binary 行为变化 | 低 | 破 byte-equal | 单测覆盖默认值 = 现 vectors.json 路径、CI 中现 sweep 仍跑默认参数无变化 |
| qwen3-8b embed 推理时间过长 | 中 | cycle 拖到多天 | Mac Metal q8_0 实测：0.6b ~1-3 min/124+78、bge-m3 估 ~3-5 min、qwen3-8b 估 ~30-50 min；可接受单次 cycle 时长 |
| qwen3-8b dim ~4096 导致 vectors-qwen3-8b.json 体积过大 | 中 | 仓库膨胀 | f32 序列化：bge-m3 dim ~1024 → ~2.5 MB、qwen3-8b dim ~4096 → ~10 MB；可接受不需 gitignore；若超 20 MB 再考虑 .gitattributes large 或 git-lfs |
| Branch IV 异常说明 vectors-{bge-m3,qwen3-8b}.json 有 bug | 低 | 误判跨族 / 同族见顶 | §5 排查清单 6 条逐一过、SHA256 校验 GGUF、抽样 L2 norm 校验、不发布异常结果 |
| 模型加载失败（GGUF 兼容性、llama-cpp-4 版本） | 低 | T3 停滞 | 0.6b 已用同款 llama-cpp-4 加载成功、bge-m3 是 llama.cpp 主流支持模型、qwen3-8b 同 Qwen3 系列同款 GGUF schema、风险低；若真出错则记录依赖兼容性问题、上 ROADMAP 风险表 |
| bge-m3 token max length 不够（最大 8192）截断我们文档 | 低 | bge-m3 评测偏差 | 现 embed code（`semantic_quality.rs:153`）已 `text.chars().take(1200)` 截断、远低于 bge-m3 8192 上限、零影响 |

## 9. 与 v1/v2/v3 节奏的对照

| 维度 | v1/v2/v3 cycle | v4 cycle（本 spec） |
|---|---|---|
| 范围 | 评测数据扩量 / 重构 | 评测模型跨族 + 同族最大档探针 |
| 动数据 | cases.json + corpus.json | 不动 cases / corpus、加 vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json 三新文件 |
| 动代码 | 不动（数据 + bake 值 + doc） | `semantic_quality.rs` 加 `--vectors-file` flag |
| 动 baseline.json | rewrite（v2/v3 各 rewrite 一次） | 不动 |
| 动 gate.rs | 不动（A-3 起自锁 baseline） | 不动 |
| Mac Metal `--embed` | 1 次（全集 124+78） | 2 次（bge-m3 + qwen3-8b 各一次） |
| Sweep 阈值数 | 9 | 9 × 3 模型 |
| 真机手测 | 不安排 | 不安排 |
| superpowers 流程 | brainstorming → spec → plan → subagent + 双审 + final review | 同款 |
| cycle 估时 | v1 ~1d、v2 ~1d、v3 ~1d | ~1.5-2d（含模型下载 ~30-60 min + bge-m3 embed ~5 min + qwen3-8b embed ~30-50 min + sweep ~30 min + 文档 1-2 h） |

## 10. 推荐工作流（subagent-driven）

按 v1/v2/v3 同款 subagent-driven-development 模式：

- T1 拆 → 一个下载 subagent（不需 code review、需 HF_TOKEN 注入）
- T2 → 写 `--vectors-file` flag 实现 subagent + spec/code-quality 双审
- T3 → Mac Metal embed 跑两次（人工驱动 `cargo run`、记录命令日志）
- T4 → sweep 三模型 9 阈值（人工驱动、产矩阵）
- T5 → 决策表落地（人工读表、写 Branch 判定）
- T6 → 写 baseline 报告 v4 节 subagent + content review
- T7 → 总验收 + README + commit（人工驱动）

每 task 走 ROADMAP §1 字段约定的 "subject / description / activeForm / verification" 模板。

## 11. 完成定义（DoD）

- [ ] T1 三模型 GGUF 在 `models/`（含原 0.6b、新 bge-m3、新 qwen3-8b）、SHA256 记录
- [ ] T2 `--vectors-file` flag 加 + 2 单测过
- [ ] T3 vectors-bge-m3.json + vectors-qwen3-8b.json 产出、vectors-qwen3-0.6b.json copy 完成、原 vectors.json byte-equal
- [ ] T4 三模型 9 阈值 sweep 决策矩阵
- [ ] T5 Branch I-a/I-b/II/III/IV 判定写出
- [ ] T6 baseline 报告 v4 节追加（含三模型表 + Branch 判定 + 下 cycle 抓手）
- [ ] T7 §6 验证矩阵全过
- [ ] README.md v4 注脚加
- [ ] PR 标题 / 描述符合 CONVENTIONS §8
- [ ] STATUS.md 当前 task / 下一步 / 会话日志顶部追加
- [ ] ROADMAP.md BETA-15B-7 新 task 卡片 + BETA-15B-4 注「探更大模型天花板已拆为 BETA-15B-7」

## 12. 参考

- [BETA-15B-6 v3 spec](2026-06-24-beta-15b-6-v3-content-not-name-second-expansion-design.md)
- [BETA-15B-6 v2 spec](2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md)
- [BETA-15B-3 A-5 cosine 路由 spec](2026-06-23-beta-15b-3a5-cosine-routing-design.md)
- [BETA-15B-6 v1 spec](2026-06-21-beta-15b-6-semantic-recall-quality-eval-design.md)
- [Semantic recall quality baseline 报告](../../reviews/semantic-recall-quality-baseline.md)
- [BETA-26 探针报告（embedding 路径去风险）](../../reviews/spike-semantic-retrieval.md)
- [ROADMAP §3.3 BETA-15B](../../../ROADMAP.md)
- [BAAI/bge-m3 HuggingFace](https://huggingface.co/BAAI/bge-m3)
- [gpustack/bge-m3-GGUF（公共 GGUF 仓库）](https://huggingface.co/gpustack/bge-m3-GGUF)
- [Qwen/Qwen3-Embedding-8B-GGUF（官方 GGUF、gated）](https://huggingface.co/Qwen/Qwen3-Embedding-8B-GGUF)
