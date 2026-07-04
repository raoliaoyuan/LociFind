# BETA-08 LoRA 启动 — 设计 spec

| 项 | 值 |
|---|---|
| ID | BETA-08（部分启动） |
| 作者 | Claude Code (Opus 4.7) |
| 日期 | 2026-05-27 |
| 阶段 | M 阶段尾 → B 阶段衔接 |
| 依赖 spec | [MVP-17 fallback evals](../../reviews/mvp-17-fallback-evals.md) §13（hybrid 架构） |
| 后续 | 训练实施 plan（writing-plans 产出）→ BETA-08 后续会话 → BETA-09 部署 |

## 1. 目标与范围

> **BETA-08 整体是多会话工作流**（ROADMAP 标 2 weeks）。本 spec 只覆盖**第一次会话**的 scope —— 设计文档 + 数据生成器 + v0 数据集。训练 / 评测 / 合并量化 / 报告归后续会话。这一边界在 §10 表格里完整呈现。

### 1.1 总体目标（BETA-08 整体）

为 Qwen2.5-1.5B-Instruct 训出 **patch 任务**专家 LoRA adapter，让 `--with-fallback --hybrid` 模式在 v0.5 evals 上达成：

- **pass 净增 ≥ 5**（相对 parser-only baseline 460/500）
- **regressed ≤ 2**（hybrid v0.3 实验已把 regressed 压到 1，adapter 不应让它显著反弹）

填补 [MVP-17 §13.5](../../reviews/mvp-17-fallback-evals.md) 留下的"模型路径无净收益"缺口。

### 1.2 本会话目标

本 spec 对应**单次会话**（约 1 工作日），产出两件交付物：

1. **设计文档**：本文件
2. **数据生成器 + v0 数据集**：
   - `packages/evals/src/bin/build_lora_dataset.rs`（Rust binary，~150 行）
   - `training/datasets/v0.5-patch/v0/{cases.jsonl, meta.json, README.md}`（数据集产物，commit 入库）

### 1.3 明确不在范围

- training/mlx-lora/ 训练脚本 / 超参 config
- 实际 LoRA 训练 run
- adapter + base 合并、GGUF 量化
- training/evals/ 模型级 eval harness（暂不建，复用 packages/evals）
- BETA-09 跨平台部署

上述均留给 BETA-08 后续会话或 BETA-09。

## 2. 训练任务定义

### 2.1 输入 / 输出形式

**任务**：给定 `query` 和 parser 输出的 `IntentDraft`，模型生成 `patch`（JSON object），由现有 `apply_patch` 合并到 draft 上得到正确 intent。

**Prompt 格式**：直接复用 [`packages/intent-parser/src/hybrid.rs::build_hybrid_prompt(query, draft)`](../../../packages/intent-parser/src/hybrid.rs)，确保**训练与推理时 prompt 字节完全一致**（零 train-serving skew）。

**Completion 格式**：纯 JSON object，仅含 `fillable_fields` 中的字段。fillable_fields 为空时为 `{}`。

### 2.2 关键约束

| 约束 | 缘由 |
|---|---|
| patch 不含 `intent` 字段 | hybrid 锁定 variant；`apply_patch` 已忽略此字段。训练数据主动剔除该噪声 |
| patch 不含 `schema_version` 字段 | 同上 |
| variant 错位 case 直接丢弃**不进数据集** | hybrid 锁 variant 物理上救不回。强训反而是负样本（教模型"反 variant"幻觉） |
| `language` 字段允许进 patch | v0.5 中 13 个 language-diff partial 属合法学习信号。`build_hybrid_prompt` 已把整 draft 序列化进 prompt，模型能看到 draft.language 现值；patch.language 只在需要纠正时出现 |

### 2.3 v0.5 直转后数据集分布预估

- ~460 case：completion `{}` —— 教模型"什么时候不要乱填"
- ~38 case：completion 含 1-3 字段 patch —— 主要学习信号
- ~2 fail 中 variant 错位丢弃；剩约 0-2 case
- **样本总量 ~498**，有效信号 38。**信号密度低是 Tier 1 已知局限**

## 3. 架构与文件布局

### 3.1 新增 / 修改

```
packages/evals/
├── Cargo.toml                              # 加 [[bin]] build_lora_dataset
└── src/bin/build_lora_dataset.rs           # 新增 ~150 行

training/
├── datasets/
│   ├── README.md                           # 更新：v0 落地说明
│   └── v0.5-patch/v0/
│       ├── cases.jsonl                     # 本会话产出（commit 入库）
│       ├── meta.json                       # 元信息
│       └── README.md                       # 数据集卡片
└── generators/README.md                    # 更新：指向 packages/evals binary

docs/superpowers/specs/
└── 2026-05-27-beta-08-lora-design.md       # 本文件
```

`training/mlx-lora/` 和 `training/evals/` 本会话不动。

### 3.2 Binary 接口

```
cargo run --release --bin build_lora_dataset -- \
    --input packages/evals/fixtures/v0.5/cases.json \
    --output training/datasets/v0.5-patch/v0/
```

确定性：同 input → 同 output 字节相等；无随机性 / 无网络。

### 3.3 JSONL 行格式

```json
{
  "prompt": "<full hybrid prompt incl query + draft + fillable_fields>",
  "completion": "{\"time\": {\"value\": 7, \"unit\": \"day\"}}",
  "case_id": "v05-xxx-NNN",
  "fillable_fields": ["time"],
  "draft_variant": "FileSearch"
}
```

`prompt` 和 `completion` 是 mlx-lm 消费字段；后三个是辅助元数据用于训练时按 variant / bucket 分桶分析。

### 3.4 meta.json schema

```json
{
  "dataset_name": "v0.5-patch",
  "version": "v0",
  "source": "packages/evals/fixtures/v0.5/cases.json",
  "source_sha256": "<64-hex>",
  "license": "internal",
  "generation_method": "parser-diff",
  "generator_version": "build_lora_dataset@<git rev>",
  "privacy_review_status": "synthetic-no-pii",
  "created_at": "2026-05-27T..Z",
  "reviewer": "Claude Code",
  "stats": {
    "total_cases": 500,
    "skipped_variant_mismatch": 2,
    "empty_patch": 460,
    "nonempty_patch": 38,
    "by_fillable_field": {"time": "<count>", "size": "<count>", "language": "<count>", "...": "..."}
  }
}
```

数据集**不可变**：v0 一旦 commit 即冻结，改生成器或 fixture 都升 v1。

### 3.5 依赖

均已在 workspace 中，无新增 crate：

- `intent-parser`: `IntentDraft::from_query` / `build_hybrid_prompt` / `analyze_structural_omissions`
- `serde_json`: 已在
- `clap`: evals 已用，沿用
- `sha2`: 计算 source_sha256；evals 未必已用，**如需新增需在 STATUS 标注 third-party 登记**

## 4. 数据生成流程

### 4.1 核心算法（伪代码）

```rust
for case in load_v05_cases() {
    let query = case.query;
    let expected_intent: Value = case.expected_intent_json;

    // 1. 跑 parser 拿 draft
    let draft = IntentDraft::from_query(&query);

    // 2. variant 错位 → 跳过
    if draft.intent.variant() != expected_intent["intent"]["variant"] {
        stats.skipped_variant_mismatch += 1;
        continue;
    }

    // 3. 计算 patch
    let draft_value = serde_json::to_value(&draft.intent)?;
    let patch = compute_patch(&draft_value, &expected_intent);

    // 4. 构造 prompt（与推理同一函数）
    let prompt = build_hybrid_prompt(&query, &draft);

    // 5. 写一行 JSONL
    writeln!(out, ...)?;
}
```

### 4.2 `compute_patch(draft, expected) -> Value` 规则

对 expected 的每个 top-level 字段：

| 情况 | 处理 |
|---|---|
| 字段名是 `intent` / `schema_version` | **不进 patch** |
| draft 缺该字段 / 该字段是 `null` | patch[字段] = expected[字段] |
| draft[字段] == expected[字段] | 不进 patch |
| draft[字段] != expected[字段] | patch[字段] = expected[字段]（整字段替换，不做嵌套 diff） |

**为什么整字段替换**：与现有 `apply_patch` 行为一致（top-level merge）；模型要学的 patch 语法集更小。

### 4.3 确定性与排序

- `serde_json::to_value` 字段顺序由 struct 定义决定（serde 默认） — 稳定
- expected_intent JSON 已是排序后（fixtures.rs 生成时保证）
- JSONL 写出**按 case_id 排序后**写，避免 hash 顺序差异

### 4.4 异常处理 — 全部 fail-fast

| 情况 | 处置 |
|---|---|
| parser panic | 不捕获，让 binary 自然挂掉（暴露 parser bug） |
| expected_intent 缺 `intent.variant` | stderr + exit code 非 0 |
| patch 含 `intent` / `schema_version` | `assert!` 阻断（防御性，应在 compute_patch 已剔除） |
| source_sha256 与 meta.json 中已记录不符 | 重生成场景下，输出新 sha 即可（首次生成不校验，因为还没 meta） |

### 4.5 单元测试（写入同 binary 文件 `#[cfg(test)]`）

1. `test_skip_variant_mismatch` — 构造 mock case，draft variant 与 expected 不同，验证 skip
2. `test_empty_patch_when_fully_correct` — draft == expected → patch = `{}`
3. `test_patch_fills_missing_field` — draft.time = None, expected.time = {...} → patch 含 time
4. `test_patch_excludes_intent_and_schema_version` — 防御性断言
5. `test_deterministic_output` — 跑两次产物 byte-equal

### 4.6 性能预估

500 case × ~5 ms parser ≈ 2.5 s。可忽略。

## 5. 数据集版本化

完整 schema 见 §3.4。规则：

- **路径**：`training/datasets/<name>/<version>/`，本次 `v0.5-patch/v0/`
- **不可变**：v0 一旦入库即冻结；改生成器 / 改 fixture 都升 v1
- **源 fixture 锚定**：meta.json 记 `cases.json` 的 sha256；生成器启动时**仅做读取记录**，不强校验（首次生成场景）
- **生成器版本锚定**：meta.json 记 `git rev-parse HEAD`
- **train/val 切分**：**留给训练脚本**，数据集本身不预切，避免污染版本

## 6. 训练配置（占位，本会话不实施）

`training/mlx-lora/` 下脚本 + config 文件留下一会话。计划起点：

| 项目 | 计划取值 | 备注 |
|---|---|---|
| 基座模型 | Qwen2.5-1.5B-Instruct (HF safetensors) | `training/mlx-lora/README` 已定 |
| 框架 | mlx-lm latest | Apple Silicon only |
| LoRA rank | 8-16 | 待实测调 |
| LoRA alpha | 2 × rank | 常规起点 |
| target_modules | q_proj, v_proj, k_proj, o_proj | mlx-lm 默认 |
| Learning rate | 1e-4 | mlx-lm 推荐起点 |
| Batch size | 4-8 | 视 unified memory |
| Iterations | 500-1000 step | Tier 1 数据量小 |
| 验证集切分 | 80/20 (seed=42) | 训练脚本处理 |
| 早停 | val loss patience=3 | mlx-lm 内置 |

## 7. 评测策略与成功标准

### 7.1 主指标（决定 v0 是否 ready）

```bash
./target/release/evals \
    --fixture packages/evals/fixtures/v0.5/cases.json \
    --with-fallback --hybrid \
    --model-path <merged + quantized gguf path> \
    --baseline <parser-only baseline json>
```

**门槛**：
- **pass 净增 ≥ 5**（相对 parser-only baseline 460）
- **regressed ≤ 2**

### 7.2 辅助观察（不作门槛）

- model 输出 patch 的 `valid_intent` 比率 ≥ 95%
- p95 fallback 延迟 ≤ 2500 ms（v0.3 baseline 1617 ms 上浮空间）
- variant confusion matrix 应 0 变化（hybrid 锁 variant；变化即 `apply_patch` 出 bug）

### 7.3 不建 training/evals

`packages/evals --with-fallback --hybrid` 已覆盖 patch 任务质量评测。patch invalid 会反映在 valid_intent + pass 上。等 v1/v2 adapter 需更细粒度调试再建。

## 8. 部署假设（衔接 BETA-09）

本 spec 不解决部署问题，但需明确**adapter 产物形态约束**，否则 BETA-09 无法对接：

- 训练产物：mlx-lm LoRA adapter（`adapter.safetensors` + `adapter_config.json`），落 `training/mlx-lora/adapters/v0/`，git-ignored
- **Mac 路径**：MLX runtime 可直接加载（如未来切 MLX 推理）
- **Windows 路径**：MLX 不可用，必须走 "合并 + GGUF 量化"。BETA-09 负责 base + adapter → merged → llama.cpp convert.py → GGUF Q4_K_M
- **BETA-08 内部评测前置**：训练在 Mac，评测时**先合并量化**再走现有 llama-cpp-4 跑 evals（避免引入 MLX runtime 依赖 evals）。**即 BETA-08 主体会话内部要跑一次合并量化**

## 9. 风险

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | Tier 1 信号密度低（38 个 nonempty patch） | 高 | adapter v0 学不动 | 接受。v0 主要验证 pipeline + 收敛；不达门槛升 Tier 2 |
| R2 | 38 个 nonempty 在 same bucket 偏（如 13 language） | 中 | 过拟合 language 修正 | meta.json 输出 stats by-bucket；训练用 weighted sampling 平衡 |
| R3 | hybrid prompt 含 4 few-shot 太长 → batch 受限 | 中 | 训练慢 / OOM | 接受；如 OOM 实测后再考虑精简 few-shot |
| R4 | parser 升级让 draft 变，旧 v0 失效 | 中 | dataset 需重生成 | meta.json sha256 + generator_version 锚定；parser 改了应升 v1 |
| R5 | mlx-lm 训练完发现 llama.cpp convert.py 不认 LoRA / 合并产物 | 中 | BETA-08 评测卡住 | spec 阶段无法消除；下一会话训练**前**先验证合并 pipeline |
| R6 | 模型学到 "reject all" 永远输出 `{}` | 中 | adapter 无净增 | 训练脚本对 nonempty/empty 加权或 oversample；spec 列入 must-handle |

## 10. 本会话 vs 后续会话边界

| 工作项 | 本会话 | 下一会话（BETA-08 主体） | BETA-09 |
|---|---|---|---|
| spec 文档 | ✅ | — | — |
| 数据生成器（Rust binary） | ✅ | — | — |
| v0.5-patch/v0/cases.jsonl + meta.json | ✅ | — | — |
| `training/datasets/v0.5-patch/v0/README.md` | ✅ | — | — |
| 更新 `training/datasets/README.md` + `training/generators/README.md` | ✅ | — | — |
| `training/mlx-lora/train.py` 训练脚本 | — | ✅ | — |
| `training/mlx-lora/configs/v0.yaml` 超参 | — | ✅ | — |
| 跑 LoRA 训练 → adapter v0 | — | ✅（需 Mac 在场） | — |
| adapter + base 合并 → GGUF 量化 | — | ✅（评测前置） | 跨平台正式部署再做 |
| 跑 v0.5 evals `--with-fallback --hybrid` 验门槛 | — | ✅ | — |
| 出场报告 `docs/reviews/beta-08-lora-v0.md` | — | ✅ | — |
| STATUS / ROADMAP 同步 BETA-08 状态 | spec 阶段同步 | training 完成同步 | done 同步 |

## 11. 验收清单（本会话）

实施 plan 完成后，必须满足：

- [ ] `cargo run --release --bin build_lora_dataset -- --input packages/evals/fixtures/v0.5/cases.json --output training/datasets/v0.5-patch/v0/` 跑通且 exit 0
- [ ] `training/datasets/v0.5-patch/v0/cases.jsonl` 存在，500 行减去 skipped_variant_mismatch 数
- [ ] `training/datasets/v0.5-patch/v0/meta.json` 存在，schema 完整（含 source_sha256 + generator_version + stats）
- [ ] 5 个单元测试全部通过
- [ ] 重跑生成器产物 byte-equal（确定性）
- [ ] `bash scripts/ci.sh` 全过
- [ ] spec 文档 commit 入库
- [ ] STATUS / ROADMAP 在收工时同步
