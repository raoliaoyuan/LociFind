# BETA-15B-6 v3：content-not-name 桶二次扩量 + T\* 真水位校验 + spec 红线修订 设计 spec

> 承接 BETA-15B-6 v2（content-not-name 桶 11→20、T\* 从 0.60 上移到 0.70 = Branch B 边界 inclusive 上界、揭示 A-5 v1 含运气、v2 真水位 OVERALL 0.854 / crosslang 0.717）；本 cycle 把 content-not-name 桶 20→30 二次扩量、校验 T\*=0.70 鲁棒性 + 测出真水位，并**主动修订 spec §2.2 (4c)(4d) 红线**为「不退步 v2 baseline」（放弃字面 0.864 / 0.700 spec 目标、移交下 cycle）。
>
> **case id 续号**：v2 实际用到 c068（c060-c068）；v3 新 case id 续号 `c069-c078`、corpus 新 doc id 续号 `s00116-s00124`。
>
> **修订说明（起草后核查）**：spec 初稿（§2.2/§3.1/§5/§6/§7）描述「v3 cycle 修订 gate.rs (4c)(4d) 红线断言代码：字面阈值 0.700 / 0.864 → 改为动态读 baseline 字段自锁」**基于错误前提** —— 核查 [`packages/evals/tests/semantic_quality_gate.rs:121-132`](../../../packages/evals/tests/semantic_quality_gate.rs#L121) 发现 (4c)(4d) **早在 A-3 cycle 就已改成动态读 baseline 自锁**（A-3 cycle 引入 HYBR baseline 字段时一并改的、v2 cycle 又升了 doc 注释）。v3 cycle 的「红线修订」**代码层无任何动作**、剩下的纯粹是**认知层 / 文档层修订**：① baseline 报告 v3 节明示「字面 0.864 / 0.700 spec 目标移交下 cycle、本 cycle 主动放弃字面追求」；② lib.rs / gate.rs doc 注释条件性升 v3 字样（随 Branch B/C bake 一起做）。原 spec T7（红线修订 task）删除、task 数从 8 → 7、原 T8 升为新 T7。

## 1. 背景与动机

v2 cycle 在 BETA-15B-6 v2 合成集（68 cases / 5 桶 / 115 corpus docs / dim 1024）上 sweep 选定 **T\*=0.70**：

- OVERALL HYBR_N **0.854 < 0.864 spec §2.2 (4d) 目标**（**字面未达**、走 baseline 自锁路径绕过、技术上不破红线）
- crosslang HYBR_N **0.717 > 0.700 spec §2.2 (4c) 目标**（达 spec 目标、+0.060 vs v2 HYB baseline）
- content-not-name HYBR_N **0.853** = v2 HYB baseline（守住）
- T\*=0.70 偏 v1 T\*=0.60 +0.10、是 v2 spec §2.2 接受标准 Branch B 边界 inclusive 上界
- A-5「cosine 单维真破局」结论**仍成立**（v2 各桶 ≥ baseline、crosslang 仍 ≥ 0.700）、**「双超 spec 目标」结论需修正**（v2 上 OVERALL 0.864 spec 目标不可达）

破局根因：A-5 v1 11 例 content-not-name 桶 vec 强 case 比例偏高、含轻微运气；v2 扩量校验后真水位回落到 0.854。

**v2 调优记录诚实承认**：「T\*=0.70 在 v2 20 例上仍可能含残余运气、扩量到 30+ 才能更精确测出真水位与最优 T\*」。content-not-name 桶 20 例的 cosine 分布**可能仍恰好让 T=0.70 跑赢**，但更大集合（30+）上 T\* 真实位置可能进一步偏移。

本 cycle = **针对性二次扩量 content-not-name 桶 20→30（+10、与 v2 +9 同款节奏）**，重跑 sweep 校验 T\*=0.70 鲁棒性、测出真水位 + **主动修订 spec §2.2 红线**承接 v2 已揭示的「0.864 spec 目标不可达」现实。

## 2. 目标与验收

### 2.1 目标

- content-not-name 桶 20 → 30 例（+10 新 case：6 zh + 4 en；4 边界 + 6 常规；3 zh+en 配对主题 + 4 单语种主题）
- corpus 同步扩 +9 新 docs（5 zh + 4 en；3 配对主题各 zh+en 2 doc + 3 单语种新 doc + 1 单语种复用现有 corpus）
- Mac Metal 本机 `--embed` 重算 vectors.json 全集（78 query + 124 doc / dim 1024 / qwen3-embedding-0.6b-q8_0）
- 重跑 9 阈值 sweep + 三 Branch 决策选 T\*
- baseline.json rewrite 反映 v3 数据集 + 新 T\*（如有微调）
- baseline 报告追加 **v3 数据集节**（扩量记录 + 主题清单 + sweep 全表 + T\* 决定 + **认知层修订理由** + 真水位结论 + 下 cycle 抓手）
- **认知层主动放弃字面 0.864 / 0.700 spec 目标**：gate.rs (4c)(4d) 代码层已自锁（A-3 cycle 改过、无代码动作）、v3 cycle 仅在 baseline 报告 v3 节明示「字面 spec 目标移交下 cycle、本 cycle 主动放弃字面追求」+ lib.rs/gate.rs doc 注释条件性升 v3 字样（随 Branch B/C bake 一起做）

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed
(2) `cargo clippy --workspace --all-targets -D warnings` 0 warning
(3) `cargo fmt --all --check` 净
(4) 回归门 `semantic_quality_gate` 用 v3 baseline pass，四条硬断言：
- (4a) exact-name HYBR_R = 1.0 （**不变**、A 簇守护红线、字面硬断言保留）
- (4b) 各桶 HYBR_N ≥ v3 HYB baseline 同桶（**不变**、动态读 baseline、本就自锁）
- (4c) crosslang HYBR_N ≥ v3 baseline (动态读 `baseline.crosslang.hybrid_routed_ndcg` 字段自锁、A-3 cycle 已改、v3 不动代码)
- (4d) OVERALL HYBR_N ≥ v3 baseline (动态读 `baseline.OVERALL.hybrid_routed_ndcg` 字段自锁、A-3 cycle 已改、v3 不动代码)

注：gate.rs 4 红线**全部自锁 baseline**（A-3 cycle 起已是「自锁完全体」、v3 不动代码）、未来 cycle 调优只要不退步 baseline 即合规；**v3 cycle 的红线修订是认知层 / 文档层动作**——baseline 报告 v3 节明示「字面 0.864 / 0.700 spec 目标主动放弃追求、移交下 cycle（更大 embedding 模型 / cosine + lang 组合信号、已挂 ROADMAP 候选）」

(5) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser）
(6) **零 PII**（沿用 BETA-15B-6 README 自查清单 5 项）：
- (6a) 无真实人名/公司/邮箱/电话/精确薪资/真实路径
- (6b) 人名/公司均为明显虚构占位
- (6c) 金额一律约整数占位
- (6d) doc_id / case id 唯一
- (6e) 跨语言桶（未动）保持词面不共享

**接受标准（不进硬红线、决定 bake 行动）— 三 Branch**：

| Branch | T\* sweep best 落点 | 行动 |
|---|---|---|
| **A** | T\*=0.70 仍 sweep best | 不动 bake 值 `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.70` / 只 rewrite baseline.json |
| **B** | T\*∈[0.60, 0.80] 微调（含 v2 起点 ±0.10、inclusive 边界） | bake 新值 + 升 lib.rs/gate.rs doc 到 v3 + rewrite baseline.json |
| **C** | T\* 偏移 > ±0.10（如跌到 0.55 或冲到 0.85） | 走 spec §5 降级保守 bake `1.01`（路由不生效、HYBR ≡ HYB） + 数据指证 cosine 单维信号在更大数据集上不稳、记 baseline 报告诚实段、移交下 cycle 信号组合或更大模型 |

**v3 cycle 真正目标 = Branch A or B**（T\* 在合理区间稳）。Branch C 是 v3 失败信号、但仍接受合并（spec §5 字面正解；红线修订仍正常进行因为已是自锁 baseline、不依赖 T\* 落点）。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- 写 **10 新 case** 进 `cases.json`（content-not-name 桶、6 zh + 4 en；id `c069-c078` 续号、沿用 v1/v2 主题模式）
- 写 **9 新 corpus docs** 进 `corpus.json`（id `s00116-s00124` 续号、5 zh + 4 en；3 配对主题各 zh+en 2 doc + 3 单语种新 doc）
- 1 case（c077 性能基线监控）**复用现有 corpus** s00011/s00012（控制 doc-level overfitting、验证 query 概念匹配 vs title 词面不匹配 的真实 cosine 分布）
- Mac Metal `--embed` 重算 vectors.json **全集 78 query + 124 doc**
- 跑 9 阈值 sweep + 人工读表选 T\*
- 若 T\* 微调 → bake 新值 `DEFAULT_COSINE_ROUTING_THRESHOLD = N` + 升 lib.rs doc + 升 gate.rs doc 注释（升 v3 字样、纯文档动作、与代码层断言无关）
- baseline.json rewrite（自然反映 v3 数据集分布）
- baseline 报告追加 v3 数据集节（扩量记录 + 主题清单 + sweep 全表 + T\* 决定 + **认知层修订理由（主动放弃字面 0.864/0.700）** + 真水位结论 + 下 cycle 抓手）
- README.md v3 更新（桶分布 20→30、corpus 总数 115→124、cases 总数 68→78）

### 3.2 Out-of-scope（明确 YAGNI）

- ❌ 扩 crosslang / synonym / concept / exact-name 4 桶（YAGNI、v2 sweep 显示它们均 ≥ baseline、不是 T\* 鲁棒性瓶颈；crosslang 桶 13 例的运气问题留下次 cycle 单独 cycle 处理）
- ❌ 改 case 数据 schema（id/bucket/query/relevant/grade 结构保留）
- ❌ corpus.json 字段加扩展（doc_id/lang/title/body 结构保留）
- ❌ 引入新桶（如 stress test 极端低 cosine 桶、混合 query 桶）
- ❌ 改任何融合层代码（result-normalizer wrapper / harness / evals binary 函数体都不动）
- ❌ 改回归门 gate.rs 任何红线断言代码（4 红线 A-3 cycle 起已全部自锁 baseline、本 cycle 仅可能升 doc 注释字样到 v3、不动断言逻辑）
- ❌ 跑 spike-retrieval 真实集校准（gitignored 真实集、本 cycle 不动）
- ❌ 重新设计 evals binary CLI / 加新 flag
- ❌ 改 README.md 「Phase D bootstrap」流程（沿用、bootstrap 已完成、本 cycle 是 v3 数据集更新）
- ❌ 上更大 embedding 模型 qwen3-1.5b/3b（留下 cycle 作 0.864 spec 目标的真正抓手）
- ❌ cosine + lang 组合信号（A-3 jaccard / A-4 detect_lang git history 可恢复、留下 cycle）
- ❌ 原始 query 入 schema（A 簇余项、byte-equal 风险须 router 后置填充不动 parser、留下 cycle）

## 4. 10 新 case 主题设计

新 case **id `c069-c078`** 续号、**bucket = content-not-name**，与 v1/v2 同款主题模式：query 描述正文要点、标题词面不共享。

### 4.1 6 zh case（3 配对 zh side + 3 单 zh）

| id | 设计类 | query 草稿 | 主题 | 预期 cosine | doc 策略 |
|---|---|---|---|---|---|
| c069 | 边界 | 那份说复盘要按 5 Whys 一层层追问、不归责到个人的模板 | 故障复盘 5 Whys / blameless | [0.55, 0.70] | 新 doc s00116（事故复盘报告）+ 跨语言主对 s00117 |
| c070 | 常规 | 提到灰度发布要按 5% / 25% / 100% 三档放量、错误率超阈值自动回滚的方案 | 灰度发布 + 回滚阈值 | [0.60, 0.80] | 新 doc s00118（灰度发布机制说明）+ 跨语言主对 s00119 |
| c071 | 常规 | 讲对外接口废弃要提前两个版本告知、保留至少一个 LTS 周期的约定 | API 版本管理 + 废弃周期 | [0.60, 0.80] | 新 doc s00120（接口版本管理）+ 跨语言主对 s00121 |
| c075 | 边界 | 那份按 P0/P1/P2 分级、P0 半小时升级到 leader 的告警制度 | 告警分级 + 升级链 | [0.55, 0.70] | 新 doc s00122（告警分级流程） |
| c076 | 常规 | 提到异常日志要带 trace_id / span_id / 错误码三个字段的规范 | 异常日志结构化字段 | [0.60, 0.80] | 新 doc s00123（异常日志结构化规范） |
| c077 | 边界 | 那份说首屏指标对比要按 P50 / P95 各画一张图、配版本号纵线的设计 | 性能基线监控（首屏指标对比） | [0.55, 0.75] | **复用 s00011/s00012（首页加载性能优化复盘）**——验证 query 概念匹配 vs title 词面不匹配的真实 cosine 分布 |

### 4.2 4 en case（3 配对 en side + 1 单 en）

| id | 设计类 | query 草稿 | 主题 | 预期 cosine | doc 策略 |
|---|---|---|---|---|---|
| c072 | 边界 | the postmortem template asking five rounds of "why" without naming individuals | 5 Whys postmortem（配对 c069） | [0.55, 0.70] | 配 c069 同主题、对 s00117 |
| c073 | 常规 | the runbook describing canary deployment with auto-rollback when error rate exceeds threshold | 灰度发布（配对 c070） | [0.60, 0.80] | 配 c070 同主题、对 s00119 |
| c074 | 常规 | the policy stating API deprecation must give two-version notice and one LTS cycle | API 版本（配对 c071） | [0.60, 0.80] | 配 c071 同主题、对 s00121 |
| c078 | 常规 | the policy explaining data retention with anonymization after the retention window | 数据保留 + 匿名化 | [0.60, 0.80] | 新 doc s00124（数据保留与匿名化政策） |

注：边界 case 实际 cosine 由 embed 后决定、表格中是预期；sweep 验证才是真理。若边界 case 实测 cosine 偏离预期（如全跌到 0.40-0.50 或全冲到 0.80+）→ baseline 报告 v3 节诚实记录、不视为失败。

### 4.3 corpus 新 doc 清单 + 复用策略

| doc_id | lang | 主题 | 对应 case |
|---|---|---|---|
| s00116 | zh | 事故复盘报告（5 Whys 框架 + 责任分摊原则） | c069 主 + c072 跨语言对 |
| s00117 | en | Blameless postmortem template (5 Whys + action items) | c072 主 + c069 跨语言对 |
| s00118 | zh | 灰度发布与回滚机制说明 | c070 主 + c073 跨语言对 |
| s00119 | en | Canary deployment runbook with rollback gates | c073 主 + c070 跨语言对 |
| s00120 | zh | 对外接口版本管理与废弃周期 | c071 主 + c074 跨语言对 |
| s00121 | en | API versioning and deprecation policy | c074 主 + c071 跨语言对 |
| s00122 | zh | 告警分级（P0/P1/P2）与升级流程 | c075 单 zh 主 |
| s00123 | zh | 异常日志结构化字段规范 | c076 单 zh 主 |
| s00124 | en | Data retention policy with anonymization rules | c078 单 en 主 |

**复用现有 corpus**：c077 性能基线监控 → 复用 s00011/s00012（首页加载性能优化复盘）。c077 query「那份说首屏指标对比要按 P50/P95 各画一张图、配版本号纵线的设计」语义上自然贴合 s00011/s00012 主题、但 query 不直命「性能优化复盘」标题词面、验证 cosine 在「query 概念匹配 vs title 词面不匹配」情况下的真实分布。

**核算**：cases 68→78（content-not-name 20→30）；corpus 115→124（+9 doc、5 zh + 4 en；整体 zh/en 比例 57:58 → 62:62 接近平衡）；vectors 全集 78 query + 124 doc 重算。

**主题与 v1+v2 14 主题的避重原则**：v1+v2 已用「缓存 / 慢测试 / 排序确定性 / 持久化失效 / 入职交接 / 安全凭据警告 / 性能优化 / 休假制度 / 异常告警 / 重试机制 / A/B 实验 / 冷启动池 / IM 表情包审核 / 多语言客服」14 主题、本轮 7 新主题（故障复盘 5 Whys / 灰度发布 canary / API 版本管理 / 告警分级 / 异常日志格式 / 性能基线监控 / 数据保留）均不与之重复（c077 主题虽与 s00011/s00012 corpus 贴合，但 query 描述「首屏指标对比 P50/P95」是新切面、不直命「性能优化复盘」标题）。

**zh/en 分布核算**：
- 新 10 case = 6 zh + 4 en（c069/c070/c071/c075/c076/c077 zh；c072/c073/c074/c078 en）
- 整体 content-not-name 桶 30 case = 13(v1+v2 zh) + 6 = 19 zh + 7(v1+v2 en) + 4 = 11 en（63/37 维持稳）

### 4.4 相关性等级（grade）标注

沿用 v1/v2 标准：
- **grade=3**：query 描述与 doc 正文要点高度对应（主目标）
- **grade=2**：query 与 doc 部分相关但非主目标
- **grade=1**：弱相关、提及但非中心
- **每 case 1-2 relevant**（与 v1/v2 一致）

## 5. 工作流（7 task 颗粒度、沿用 v2 同款节奏；原 spec T7 红线修订删除——核查 gate.rs 已自锁、详顶部修订说明）

```bash
# T1: 写 10 新 case + 9 新 corpus doc 进 fixture（手动编辑 + PII 自查 5 项）
#   - packages/evals/fixtures/semantic-recall/cases.json: 追加 c069-c078
#   - packages/evals/fixtures/semantic-recall/corpus.json: 追加 s00116-s00124

# T2: 跑完整性测试验 case-doc 引用 + lang 标注 + id 唯一
cargo test -p locifind-evals --test semantic_quality_fixtures_integrity
# Expected: pass（验 c077 复用的 s00011/s00012 仍存在）

# T3: Mac Metal --embed 全集重算 vectors.json
cargo run --release -p locifind-evals --bin semantic_quality \
  --features semantic-recall-metal -- --embed
# Expected: 写 vectors.json（78 query + 124 doc embed、dim 1024、qwen3-embedding-0.6b-q8_0）

# T4: 跑 9 阈值 sweep + 控制对照（W=10.0 固定）
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== cosine_threshold = $T ==="
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 --cosine-threshold $T
done | tee /tmp/sweep-cosine-v3.log

# T5: 人工读 sweep-cosine-v3.log + 按 §2.2 三 Branch 决策选 T*
#   Branch A (T*=0.70 仍 best)→ 跳过 T6 bake、直接 T7 rewrite baseline + 验证
#   Branch B (T*∈[0.60, 0.80])→ T6 bake 新值 + 升 lib.rs doc + 升 gate.rs doc 到 v3
#   Branch C (偏移 > ±0.10)→ T6 bake 1.01 spec §5 降级 + 诚实记录 cosine 单维信号在更大数据集上不稳

# T6: （条件性，Branch B/C）bake 新 T* + 升 lib.rs/gate.rs doc 注释（纯文档动作）
#   - packages/result-normalizer/src/lib.rs: DEFAULT_COSINE_ROUTING_THRESHOLD = <新值>
#   - packages/result-normalizer/src/lib.rs: doc 升 v3（含 v3 sweep 结论 + Branch 命中 + 边界 inclusive 上/下界说明）
#   - packages/evals/tests/semantic_quality_gate.rs: doc 注释升 v3（T*=<新值> + 「v3 cycle 主动放弃字面 0.864/0.700 spec 目标」字样、纯注释、不动断言代码）

# T7: rewrite baseline.json + 跑回归门 + 全套验证 + 报告 + 总验收
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --write-baseline
cargo fmt --all --check
cargo clippy --workspace --all-targets -D warnings
cargo test --workspace
cargo test -p locifind-evals --test semantic_quality_gate  # 4 红线全过、动态读 v3 baseline
# v0.5/v0.9 byte-equal 复检（本 cycle 不动 parser 自然过、用规范化逐 case 比对）
#   - packages/evals/fixtures/semantic-recall/README.md: v3 更新（cases 68→78、corpus 115→124、桶 20→30、复用策略说明）
#   - docs/reviews/semantic-recall-quality-baseline.md: 追加 v3 数据集节
#     （扩量记录 + 主题清单 + sweep 全表 + T* 决定 + 认知层修订理由 + 真水位结论 + 下 cycle 抓手优先级）
```

## 6. 代码改动清单（按 plan task 颗粒度）

| Task | 文件 | 改动 |
|---|---|---|
| T1 | `packages/evals/fixtures/semantic-recall/cases.json` | 追加 10 新 case `c069-c078`、bucket=content-not-name；c077 relevant 指向 s00011/s00012 |
| T2 | `packages/evals/fixtures/semantic-recall/corpus.json` | 追加 9 新 doc `s00116-s00124`（5 zh + 4 en；lang 字段对齐）+ `semantic_quality_fixtures_integrity` 完整性测试通过 |
| T3 | `packages/evals/fixtures/semantic-recall/vectors.json` | Mac Metal `--embed` 重算 + 写回（78 query + 124 doc）|
| T4 | （仅跑 sweep）| 产 `/tmp/sweep-cosine-v3.log`、人工读表选 T\* |
| T5 | （仅人工决策）| 选 T\* Branch、决定 T6 是否动 bake |
| T6 | `packages/result-normalizer/src/lib.rs` | **条件性**（Branch B/C）：若 T\* 微调 → 改 `DEFAULT_COSINE_ROUTING_THRESHOLD` 数值 + 升 doc 到 v3；若 Branch A（T\*=0.70 仍 best）→ 不动 |
| T6 | `packages/evals/tests/semantic_quality_gate.rs` | **条件性**（Branch B/C）：若 T\* 微调 → 升 doc 注释到 v3（含「v3 cycle 主动放弃字面 0.864/0.700 spec 目标」字样、纯注释、不动断言代码）；若 Branch A → 不动 |
| T7 | `packages/evals/fixtures/semantic-recall/baseline.json` | `--write-baseline` rewrite、自然反映 v3 数据集分布 |
| T7 | `packages/evals/fixtures/semantic-recall/README.md` | v3 更新（cases 总数 68→78、corpus 总数 115→124、content-not-name 桶 20→30、复用 s00011/s00012 策略说明、跨语言配对主题表不动） |
| T7 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 v3 数据集节（扩量记录 + 主题清单 + sweep 全表 + T\* 决定 + **认知层修订理由（明示主动放弃字面 0.864/0.700 spec 目标、移交下 cycle）** + 真水位结论 + 下 cycle 抓手优先级） |

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `cargo test --workspace`
- 必要 task 追加：
  - T2：`semantic_quality_fixtures_integrity` pass（验 case-doc 引用 + lang 标注 + id 唯一 + 复用 c077 的 s00011/s00012 仍存在）
  - T3：vectors.json 文件大小 / dim=1024 / model_id=qwen3-embedding-0.6b-q8_0 元数据 + `check_vectors` 完整性
  - T7：`cargo test -p locifind-evals --test semantic_quality_gate` 4 红线全过、动态读 v3 baseline
  - T7：evals parser-only byte-equal v0.5=473 / v0.9=877 不变（本 cycle 不动 parser 自然过、用规范化逐 case 比对 v05-\* 子集 0 变化即 byte-equal）
  - T7：baseline 报告 v3 节完整含扩量记录 + 主题清单 + sweep 全表 + T\* 决定理由 + **认知层修订理由（明示主动放弃字面 0.864/0.700）** + 真水位结论 + 下 cycle 抓手优先级

## 8. 风险与已记忆教训

### 8.1 风险

| 风险 | 缓解 |
|---|---|
| **T\*=0.70 在 v3 30 例上不再 sweep best**（v2 20 例确实带运气） | 接受 Branch B 微调（0.60-0.80 + 升 v3 doc）；若偏移 > ±0.10 走 Branch C spec §5 降级、bake 1.01、诚实记录 cosine 单维信号在更大数据集上不稳、移交下 cycle 信号组合或更大模型 |
| **PII 泄漏**（10 新虚构 case 不小心引入真实主体） | 沿用 README 自查清单 5 项 + commit 前再过一遍；新 doc 用明显虚构占位（如「内部 IM 平台」「Acme Postmortem Template」），公司名/人名/邮箱/电话/精确薪资/真实路径全自查 |
| **vectors.json 二进制兼容性**（重算后 dim/model_id 元数据漂移） | 完整性测试 `semantic_quality_fixtures_integrity` + `check_vectors` 覆盖；与 v2 同模型同 dim 1024、漂移概率低 |
| **edge case 实测 cosine 偏离预期**（4 边界 case 全跌到 0.40 或全冲到 0.80+） | baseline 报告 v3 节诚实记录、不视为失败；sweep 才是真理、case 设计预期只是参考；若边界 case 全冲到 0.80+ → T\* 可能上移到 0.80 = Branch B 边界、可接受 |
| **c077 复用 s00011/s00012 corpus 反伤 cosine 信号**（复用 doc 可能让 cosine_top1 偏高、c077 进入「常规」而非「边界」分布） | 复用策略本身是验证「query 概念匹配 vs title 词面不匹配」的合理设计；若实测 c077 cosine 偏离预期 → baseline 报告诚实记录复用 case 的 cosine 漂移、不视为失败；c077 边界设计预期仅作参考 |
| **Mac Metal embed 失败**（模型未就位 / metal feature 异常 / 内存不足） | 按 README Phase D bootstrap 流程；若失败先检查 `models/qwen3-embedding-0.6b-q8_0.gguf` + `--features semantic-recall-metal`；与 v2 同款流程已验证 |
| **byte-equal 风险 v0.5/v0.9 退步** | 本 cycle 完全不动 parser / coverage / model fallback / harness / result-normalizer 函数体；只动 fixture 数据 + 条件性 bake 常量数值（lib.rs DEFAULT_COSINE_ROUTING_THRESHOLD）+ doc 注释；不影响 evals parser 路径；byte-equal 自然保持 |
| **content-not-name 桶 case 风格偏移**（10 新例与 v1+v2 20 例风格不一致让 baseline 错位） | 沿用 v1/v2 query 模式（「那份说...」「提到...的那份」「the doc explaining...」「the spec saying...」「the policy on...」）、主题贴近办公/技术/制度 3 大类 |

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 v0.9 coverage，不触发
- [[project-evals-reporter-nondeterministic]]：T8 byte-equal 闸门用规范化逐 case 比对（spec §2.2 (5) parser-only byte-equal）
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告，三件套全做（gate 4 红线全部自锁 baseline、A-3 cycle 起即「自锁完全体」、v3 cycle 主动放弃字面 0.864/0.700 spec 目标为认知层修订、不动代码）
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring，不触发
- [[project-rrf-weight-tuning-ceiling]]：W=10.0 固定不调
- [[feedback-per-task-verify-include-fmt]]：每 task 验证门必含 fmt + clippy + test ✓
- [[project-pull-full-distribution-before-convention-call]]：扩量前已数全量分布（content-not-name 桶 v1+v2 20 例语种 13 zh + 7 en、v1+v2 14 主题清单已列）
- [[project-g15-ambiguity-disambig]]：本 cycle 不动 parser 类型词消歧逻辑，不触发

## 9. 链接

- v2 spec：[2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md](./2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md)
- v2 plan：[../plans/2026-06-24-beta-15b-6-v2-content-not-name-expansion.md](../plans/2026-06-24-beta-15b-6-v2-content-not-name-expansion.md)
- A-5 spec：[2026-06-23-beta-15b-3a5-cosine-routing-design.md](./2026-06-23-beta-15b-3a5-cosine-routing-design.md)
- BETA-15B-6 v1 README：[../../../packages/evals/fixtures/semantic-recall/README.md](../../../packages/evals/fixtures/semantic-recall/README.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)（本 cycle T8 追加 v3 节）
- 项目状态：[../../../STATUS.md](../../../STATUS.md) §「下一步」候选 ① BETA-15B-6 v3 评测集再扩量
