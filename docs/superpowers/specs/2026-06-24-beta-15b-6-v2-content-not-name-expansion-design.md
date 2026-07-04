# BETA-15B-6 v2：content-not-name 桶扩量 + T\* 鲁棒性校验 设计 spec

> 承接 BETA-15B-3 A-5（cosine 阈值路由、T\*=0.60 bake、A 簇 5 cycle 首破 spec §5 降级）；本 cycle 把 content-not-name 桶从 11 → 20 例扩量、校验 T\*=0.60 在更大集合上仍 sweep best。
>
> **实施时 case id 修正**：spec §4.1/§4.2 起草时写 9 新 case id 为 `c049-c057`，但 v1 实际 case id 范围中 c049-c059 已被 exact-name 桶占用、新 case 必须续号 ≥ c060。**plan + 实施使用真实续号 `c060-c068`**，spec 内文未同步替换（保留草案痕迹便于追溯 brainstorming 现场）。

## 1. 背景与动机

A-5 cycle 在 BETA-15B-6 v1 合成集（59 cases / 5 桶 / 108 corpus docs）上 sweep 选定 **T\*=0.60**：

- OVERALL HYBR_N **0.871 > 0.864 spec §2.2 (4d) 目标**
- crosslang HYBR_N **0.726 > 0.700 spec §2.2 (4c) 目标**
- content-not-name HYBR_N **0.930** = HYB baseline（守住）
- A-3/A-4 单维信号都走 spec §5 降级、A-5 cosine 单维**首破降级**

破局根因：cosine 信号方向与不对称失败模式同构——crosslang 桶 vec 强 cosine 高跳 FTS、content-not-name 桶 vec 弱 cosine 低保留 FTS。

**但 A-5 调优记录诚实承认**：「合成集 11 例可能让 T\*=0.60 带运气、扩量校验鲁棒性」。content-not-name 桶 11 例的 cosine 分布**可能恰好让 T=0.60 全部跑赢**，但更大集合上可能暴露 T\* 真实位置偏移到 0.55 或 0.65。

本 cycle = **针对性扩量 content-not-name 桶到 20 例（+9）**，重跑 sweep 验证 T\*=0.60 仍是 sweep best、或诚实接受 0.55-0.65 之间微调。

## 2. 目标与验收

### 2.1 目标

- content-not-name 桶 11 → 20 例（+9 新 case：5 zh + 4 en；4-5 边界 case 预期 cosine_top1 在 0.50-0.70 区间、4-5 常规 case 分布更广）
- corpus 同步扩 +5~9 新 docs（防 doc-level overfitting、部分新 case 复用现有 docs）
- Mac Metal 本机 `--embed` 重算 vectors.json 全集
- 重跑 9 阈值 sweep + 选 T\*（v1 sweep 全表见 A-5 调优记录）
- baseline.json rewrite 反映 v2 数据集 + 新 T\*（如有微调）
- baseline 报告追加 **v2 数据集节**（扩量记录 + 新 sweep 全表 + T\* 决定）

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed
(2) `cargo clippy --workspace --all-targets -D warnings` 0 warning
(3) `cargo fmt --all --check` 净
(4) 回归门 `semantic_quality_gate` 用新 baseline pass，含四条硬断言：
- (4a) exact-name HYBR_R = 1.0
- (4b) 各桶 HYBR_N ≥ HYB baseline 同桶（不退步）
- (4c) crosslang HYBR_N ≥ 0.700 spec 目标
- (4d) OVERALL HYBR_N ≥ 0.864 spec 目标
- 注：gate.rs 4 红线动态读 baseline、A-6 数值自动跟随、不需改 gate 代码

(5) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser）
(6) **零 PII**（沿用 BETA-15B-6 README 自查清单 5 项）：
- (6a) 无真实人名/公司/邮箱/电话/精确薪资/真实路径
- (6b) 人名/公司均为明显虚构占位
- (6c) 金额一律约整数占位
- (6d) doc_id / case id 唯一
- (6e) 跨语言桶（未动）保持词面不共享

**接受标准（不进硬红线）**：
- **T\* = 0.60 仍 sweep best（最佳）**：直接复用 bake 值、不动常量、只 rewrite baseline.json
- **T\* 微调 0.55-0.65 之间（可接受）**：bake 新值 + 更新 lib.rs doc + rewrite baseline.json + gate.rs doc 升 v2
- **T\* 偏移 > ±0.10**（如跌到 0.45 或冲到 0.75）：**走 spec §5 降级路径**（保守 T\*=1.01）+ 数据指证有更深问题、记 baseline 报告诚实段、下 cycle 再做更大扩量 / 信号组合

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- 写 **9 新 case** 进 `cases.json`（content-not-name 桶、5 zh + 4 en；id `c049-c057` 续号、沿用 v1 主题模式）
- 写 **5-9 新 corpus docs** 进 `corpus.json`（id `s00109+` 续号、zh 列与 en 列继续交替）
- 部分新 case 复用现有 108 docs（控制 doc-level overfitting）
- Mac Metal `--embed` 重算 vectors.json **全集 68 query + 113-117 doc**
- 跑 9 阈值 sweep + 人工读表选 T\*
- 若 T\* 微调 → bake 新值 `DEFAULT_COSINE_ROUTING_THRESHOLD = N` + 升 lib.rs doc + 更新 gate.rs doc
- baseline.json rewrite（自然反映 v2 数据集分布）
- baseline 报告追加 v2 数据集节（扩量记录 + 主题清单 + sweep 全表 + T\* 决定 + 鲁棒性结论）
- README.md v2 更新（桶分布 11→20、corpus 总数）

### 3.2 Out-of-scope（明确 YAGNI）

- ❌ 扩 crosslang / synonym / concept / exact-name 4 桶（YAGNI、A-5 sweep 显示它们均 ≥ baseline、不是 T\* 鲁棒性瓶颈）
- ❌ 改 case 数据 schema（id/bucket/query/relevant/grade 结构保留）
- ❌ corpus.json 字段加扩展（doc_id/lang/title/body 结构保留）
- ❌ 引入新桶（如 mixed query 桶、边界探针专属桶）
- ❌ 改任何代码（result-normalizer / harness / evals binary 都不动）
- ❌ 改回归门 gate.rs 断言代码逻辑（动态读 baseline、A-6 数值自动跟随）
- ❌ 跑 spike-retrieval 真实集校准（gitignored 真实集、本 cycle 不动）
- ❌ 重新设计 evals binary CLI / 加新 flag
- ❌ 改 README.md 「Phase D bootstrap」流程（沿用、bootstrap 已完成、本 cycle 是 v2 数据集更新）

## 4. 9 新 case 主题设计

新 case **id `c049-c057`** 续号、**bucket = content-not-name**，与 v1 11 例（c038-c048）同款主题模式：query 描述正文要点、标题词面不共享。

### 4.1 5 zh case（3 边界 + 2 常规）

| id | query 草稿 | 主题 | 预期 cosine 倾向 | doc 策略 |
|---|---|---|---|---|
| c049 | 那份说重试机制要用指数退避配合抖动二者结合的方案 | 重试机制设计 | 边界 (0.55-0.65) | 新 doc（虚构「分布式任务重试规约」） |
| c050 | 提到 A/B 实验最小样本量按统计功效算的复盘记录 | A/B 实验设计 | 边界 (0.55-0.65) | 新 doc（虚构「实验平台样本量计算说明」） |
| c051 | 讲冷启动数据稀疏用人工冷启池规则的方案 | 推荐冷启动 | 边界 (0.55-0.65) | 新 doc（虚构「冷启动池规则设计」） |
| c052 | 那篇关于多语言客服话术规约的内容 | 多语言客服 | 常规（cosine 可能高） | 复用现有 c024 系列 doc |
| c053 | 提到内部 IM 表情包审核制度的那份 | IM 表情包审核 | 常规（cosine 可能高） | 新 doc（虚构「内部沟通工具内容审核制度」） |

### 4.2 4 en case（2 边界 + 2 常规）

| id | query 草稿 | 主题 | 预期 cosine 倾向 | doc 策略 |
|---|---|---|---|---|
| c054 | the spec saying retries must use truncated exponential backoff with jitter | 重试机制（en 配对 c049） | 边界 (0.55-0.65) | 配 c049 同主题 zh+en 配对、或复用 c049 doc 跨语言 |
| c055 | the doc explaining cold-start data sparsity with manual cold pool rules | 冷启动（en 配对 c051） | 边界 (0.55-0.65) | 配 c051 同主题 zh+en 配对、或复用 |
| c056 | the policy on internal IM emoji approval workflow | IM 表情包（en 配对 c053） | 常规 | 配 c053 同主题、或复用 |
| c057 | the writeup on multilingual customer support phrasing conventions | 多语言客服（en 配对 c052） | 常规 | 配 c052 同主题、或复用 |

注：边界 case 实际 cosine 由 embed 后决定、表格中是预期；sweep 验证才是真理。若边界 case 实测 cosine 偏离预期（如全跌到 0.40-0.50 或全冲到 0.70+）→ baseline 报告 v2 节诚实记录、不视为失败。

### 4.3 corpus 新 doc 数量 + 复用策略

- **新 doc 5-9 篇**（具体数视 case 复用决策）：
  - c049/c054 重试机制：zh+en 配对 2 新 doc（`s00109` zh、`s00110` en）
  - c050 A/B 实验：1 新 doc（`s00111` zh）
  - c051/c055 冷启动：zh+en 配对 2 新 doc（`s00112` zh、`s00113` en）
  - c053/c056 IM 表情包：zh+en 配对 2 新 doc（`s00114` zh、`s00115` en）
  - c052/c057 多语言客服：**复用现有 corpus**（如客服 FAQ 类 doc `s00023/s00024`）
- 共 **+7 新 doc**（s00109-s00115）、corpus 总数 108→115
- 复用策略：c052/c057 同主题复用客服类 doc、验证 cosine_top1 在「query 概念匹配 vs title 词面不匹配」情况下的真实分布

注：上方为预期方案；实际实施时若发现某主题与现有 corpus 重合度高、可调整 zh+en 配对 doc 数（5-9 区间内）。

### 4.4 相关性等级（grade）标注

沿用 v1 标准：
- **grade=3**：query 描述与 doc 正文要点高度对应（主目标）
- **grade=2**：query 与 doc 部分相关但非主目标
- **grade=1**：弱相关、提及但非中心
- **每 case 1-2 relevant**（与 v1 一致）

## 5. 工作流（spec 阶段执行）

```bash
# T1: 写 9 新 case + 7 新 corpus docs 进 fixture
# 手动编辑：
# - packages/evals/fixtures/semantic-recall/cases.json（追加 c049-c057）
# - packages/evals/fixtures/semantic-recall/corpus.json（追加 s00109-s00115）

# T2: 跑完整性测试验 case-doc 引用 + lang 标注 + id 唯一
cargo test -p locifind-evals --test semantic_quality_fixtures_integrity
# Expected: pass

# T3: Mac Metal --embed 重算 vectors.json 全集
cargo run --release -p locifind-evals --bin semantic_quality --features semantic-recall-metal -- --embed
# Expected: 写 vectors.json（68 query + 115 doc embed）

# T4: 跑 9 阈值 sweep
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== cosine_threshold = $T ==="
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --cosine-threshold $T
done | tee /tmp/sweep-cosine-v2.log

# T5: 人工读 sweep-cosine-v2.log，按 spec §2.2 红线 + 接受标准选 T*
#   ① T*=0.60 仍 sweep best → 不动 bake 值、只 rewrite baseline.json
#   ② T*∈[0.55, 0.65] 微调 → bake 新值 + 升 lib.rs doc + 升 gate.rs doc
#   ③ T* 偏移 > ±0.10 → 走 spec §5 降级、bake 1.01、记诚实段

# T6: rewrite baseline.json（不传 --cosine-threshold 走 DEFAULT 值、--write-baseline）
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --write-baseline

# T7: 跑回归门 + 全套验证
cargo test --workspace
cargo test -p locifind-evals --test semantic_quality_gate
# v0.5 / v0.9 byte-equal 复检
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json | ...
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json | ...

# T8: 写 baseline 报告 v2 节 + README v2 更新 + commit + PR
```

## 6. 代码改动清单（按 plan task 颗粒度）

| Task | 文件 | 改动 |
|---|---|---|
| T1 | `packages/evals/fixtures/semantic-recall/cases.json` | 追加 9 新 case `c049-c057`、bucket=content-not-name |
| T1 | `packages/evals/fixtures/semantic-recall/corpus.json` | 追加 7 新 doc `s00109-s00115`（5 zh + 2 en；lang 字段对齐） |
| T2 | （仅跑测试）| `semantic_quality_fixtures_integrity` 完整性测试通过 |
| T3 | `packages/evals/fixtures/semantic-recall/vectors.json` | Mac Metal `--embed` 重算 + 写回（68 query + 115 doc）|
| T4 | （仅跑 sweep）| 产 `/tmp/sweep-cosine-v2.log`、人工读表选 T\* |
| T5 | `packages/result-normalizer/src/lib.rs` | **条件性**：若 T\* 微调 → 改 `DEFAULT_COSINE_ROUTING_THRESHOLD` 数值 + 升 doc；若 T\*=0.60 仍 best → 不动 |
| T5 | `packages/evals/tests/semantic_quality_gate.rs` | **条件性**：若 T\* 微调 → 升 doc 注释 `T*=新值`；若 T\*=0.60 → 不动 |
| T6 | `packages/evals/fixtures/semantic-recall/baseline.json` | `--write-baseline` rewrite、自然反映 v2 数据集分布 |
| T7 | `packages/evals/fixtures/semantic-recall/README.md` | 更新桶分布表（content-not-name 11→20）+ corpus 总数（108→115）+ cases 总数（59→68）+ 跨语言配对主题（未动）|
| T7 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加 v2 数据集节（扩量记录 + 主题清单 + sweep 全表 + T\* 决定 + 鲁棒性结论） |

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -D warnings`
- `cargo test --workspace`
- 必要 task 追加：
  - T2：`semantic_quality_fixtures_integrity` pass（验 case-doc 引用 + lang 标注 + id 唯一）
  - T3：vectors.json 文件大小 / dim / model_id 元数据合理（与 v1 同模型 qwen3-embedding-0.6b-q8_0.gguf）
  - T6：`semantic_quality_gate` 1 passed（4 红线全过、动态读新 baseline）
  - T7：v0.5/v0.9 byte-equal v0.5=473 / v0.9=877 不变（本 cycle 不动 parser 自然过）
  - T7：baseline 报告 v2 节完整含 sweep 全表 + T\* 决定理由 + 鲁棒性结论

## 8. 风险与已记忆教训

### 8.1 风险

| 风险 | 缓解 |
|---|---|
| **T\*=0.60 在 v2 不再 sweep best**（A-5 11 例确实带运气） | 接受 0.55-0.65 微调 + 诚实 bake 新值；若 T\* 偏移大于 ±0.10 走 spec §5 降级 1.01 |
| **PII 泄漏**（虚构 case 不小心引入真实主体） | 沿用 README 自查清单 5 项 + commit 前再过一遍；新 doc 用明显虚构占位（如「张示例」「Acme」） |
| **vectors.json 二进制兼容性**（重算后 dim/model_id 元数据漂移） | 完整性测试 `semantic_quality_fixtures_integrity` + `check_vectors` 覆盖 |
| **edge case 实测 cosine 偏离预期**（4-5 边界 case 全跌到 0.40 或全冲到 0.70+） | baseline 报告 v2 节诚实记录、不视为失败；本身 sweep 才是真理、case 设计预期只是参考 |
| **Mac Metal embed 失败**（模型未就位 / metal feature 异常 / 内存不足） | 按 README Phase D bootstrap 流程；若失败先检查 `models/qwen3-embedding-0.6b-q8_0.gguf` + `--features semantic-recall-metal` |
| **byte-equal 风险 v0.5/v0.9 退步** | 本 cycle 完全不动 parser / coverage / model fallback / harness / result-normalizer 函数体；只动 fixture 数据 + 可选 bake 常量数值；byte-equal 自然保持 |
| **content-not-name 桶 case 类型偏移**（新 9 例与 v1 11 例风格不一致让 baseline 错位）| 沿用 v1 query 模式（「那份说...」「提到...的那份」「the doc explaining...」），主题贴近办公/技术/制度 3 大类 |

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 v0.9 coverage，不触发
- [[project-evals-reporter-nondeterministic]]：T7 byte-equal 闸门用 status 计数（spec §2.2 (5) parser-only byte-equal）
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告、三件套全做（gate 4 红线已动态读 baseline、本 cycle 自动跟随）
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring，不触发
- [[project-rrf-weight-tuning-ceiling]]：W=10.0 固定不调
- [[feedback-per-task-verify-include-fmt]]：每 task 验证门必含 fmt + clippy + test ✓
- [[project-pull-full-distribution-before-convention-call]]：扩量前已数全量分布（11 例 content-not-name 主题汇总在 §4 已列）

## 9. 链接

- A-5 spec：[2026-06-23-beta-15b-3a5-cosine-routing-design.md](./2026-06-23-beta-15b-3a5-cosine-routing-design.md)
- BETA-15B-6 v1 README：[../../../packages/evals/fixtures/semantic-recall/README.md](../../../packages/evals/fixtures/semantic-recall/README.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)（本 cycle T7 追加 v2 节）
- 项目状态：[../../../STATUS.md](../../../STATUS.md) §「下一步」候选 ① 评测集扩量
