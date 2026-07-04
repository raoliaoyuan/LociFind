# BETA-08 LoRA 主体 run — 设计 spec

| 项 | 值 |
|---|---|
| ID | BETA-08（主体 run） |
| 作者 | Claude Code (Opus 4.7) |
| 日期 | 2026-05-27 |
| 阶段 | B 阶段衔接 |
| 依赖 spec | [BETA-08 启动设计](./2026-05-27-beta-08-lora-design.md)、[smoke spike 设计](./2026-05-27-beta-08-smoke-design.md) |
| 依赖 ledger | [smoke spike ledger](../../reviews/beta-08-smoke.md) |
| 后续 | 实施 plan → 跑 → 出 `docs/reviews/beta-08-lora-v0.md` |

## 1. 目标与范围

### 1.1 目标

在 [v0.5-patch/v0 训练集](../../../training/datasets/v0.5-patch/v0/) 上跑正式 LoRA 训练，产 Q4_K_M GGUF adapter，跑 v0.5 evals `--with-fallback --hybrid` 验证**门槛**：

- **pass 净增 ≥ 5**（相对 parser-only baseline 460/500）
- **regressed ≤ 2**

满足两条门槛 = adapter v0 ready；任一未达 = v0 not_ready（仍合法结案，给项目下一步明确方向，不回 brainstorming）。

### 1.2 范围

- **数据**：v0.5-patch/v0 全 498 sample 都进 train（用户决策）
- **基座**：本地 `~/models/qwen25_1.5b_draft/`（Qwen2.5-1.5B-Instruct 4bit MLX 格式，828 MB）
- **训练**：mlx-lm lora，iters 1000 / num-layers 16 / batch 4 / lr 1e-4 / seed 42
- **GGUF 路径**（smoke ledger 已验证至 step 2）：mlx fuse → HF safetensors → llama.cpp `convert_hf_to_gguf.py` → fp16 GGUF → `llama-quantize` Q4_K_M → ~1 GB GGUF
- **评测**：先 parser-only baseline 再 `--with-fallback --hybrid`，跑 v0.5 全 500 case
- **出场报告**：`docs/reviews/beta-08-lora-v0.md`

### 1.3 不在范围

- Q5_K_M / Q8_0 量化变体
- 超参 sweep
- 数据集 Tier 2/3 augmentation
- search.rs → ToolRegistry wiring

### 1.4 工具链前置（已就绪）

- ✅ mlx-lm 0.29.1
- ✅ 本地基座 `~/models/qwen25_1.5b_draft/`
- ✅ `~/tools/llama.cpp/build/bin/llama-quantize`
- ✅ `~/tools/llama.cpp/convert_hf_to_gguf.py`
- ✅ Python torch 2.8.0（convert_hf_to_gguf.py 依赖）

## 2. 文件布局

```
training/mlx-lora/
├── README.md                                # 已有，补 main run 章节
├── scripts/
│   ├── prepare_smoke_data.py                # 已有
│   ├── prepare_main_data.py                 # 新建（~30 行）
│   ├── run_smoke.sh                         # 已有
│   └── run_main.sh                          # 新建（~70 行）
├── data/main/
│   ├── train.jsonl                          # 498 行，git-ignored
│   └── valid.jsonl                          # 50 行（train 末尾复制），git-ignored
├── adapters/main-v0/                        # 训练产物，git-ignored
└── fused/
    ├── main-v0-safetensors/                 # mlx fuse 输出 ~3 GB，git-ignored
    ├── main-v0-f16.gguf                     # llama.cpp convert 输出 ~3 GB，git-ignored
    ├── main-v0-q4_k_m.gguf                  # llama-quantize 输出 ~1 GB，git-ignored，**评测用**
    ├── main-v0-baseline.json                # parser-only evals JSON
    └── main-v0-evals.json                   # with-fallback hybrid evals JSON

docs/superpowers/specs/
└── 2026-05-27-beta-08-main-design.md        # 本 spec
docs/superpowers/plans/
└── 2026-05-27-beta-08-main.md               # 实施 plan
docs/reviews/
└── beta-08-lora-v0.md                       # 出场报告
```

`models/qwen2.5-1.5b-instruct-q4_k_m.gguf` baseline **不动**，apples-to-apples 对比靠两份独立 evals JSON。

## 3. 数据准备脚本

### 3.1 输入 / 输出

| 项 | 值 |
|---|---|
| 输入 | `training/datasets/v0.5-patch/v0/cases.jsonl`（498 行） |
| 输出 | `training/mlx-lora/data/main/train.jsonl`（498 行）+ `valid.jsonl`（50 行复制自 train 末尾） |
| 调用 | `python3 training/mlx-lora/scripts/prepare_main_data.py`（仓库根；无参数） |
| 确定性 | seed=42 |

### 3.2 与 smoke 版区别

- 全 498 都进 train（不切 holdout）
- valid 是 train 末尾 50 条**复制**，仅满足 mlx-lm 接口约束。**不**承担真实 holdout 校验职责（评测靠 packages/evals 全 500）
- 字段裁剪：仅保留 `prompt` + `completion`，drop case_id / fillable_fields / draft_variant

### 3.3 算法

```python
1. 读 cases.jsonl 498 行
2. random.seed(42); random.shuffle(list)
3. 每行裁剪到 {prompt, completion}
4. 全 498 写 train.jsonl
5. 末尾 50 行复制到 valid.jsonl
6. 自检：行数 + 字段集
```

## 4. 训练 + fuse + GGUF + evals 编排脚本

### 4.1 run_main.sh 7 步（注意：含 parser-only baseline）

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

if [[ -d "$HOME/models/qwen25_1.5b_draft" ]]; then
    MODEL="$HOME/models/qwen25_1.5b_draft"
else
    MODEL="mlx-community/Qwen2.5-1.5B-Instruct-4bit"
fi
LLAMA_CPP="$HOME/tools/llama.cpp"
DATA_DIR="training/mlx-lora/data/main"
ADAPTER_DIR="training/mlx-lora/adapters/main-v0"
SAFETENSORS_DIR="training/mlx-lora/fused/main-v0-safetensors"
GGUF_F16="training/mlx-lora/fused/main-v0-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/main-v0-q4_k_m.gguf"
BASELINE_JSON="training/mlx-lora/fused/main-v0-baseline.json"
EVALS_JSON="training/mlx-lora/fused/main-v0-evals.json"

# [1/7] prepare data
echo "==> [1/7] prepare data"
python3 training/mlx-lora/scripts/prepare_main_data.py

# [2/7] lora train
echo "==> [2/7] lora train (1000 step / num-layers 16 / batch 4)"
python3 -m mlx_lm lora \
    --model "$MODEL" \
    --train \
    --data "$DATA_DIR" \
    --fine-tune-type lora \
    --num-layers 16 \
    --iters 1000 \
    --batch-size 4 \
    --learning-rate 1e-4 \
    --steps-per-report 50 \
    --steps-per-eval 200 \
    --adapter-path "$ADAPTER_DIR" \
    --seed 42

# [3/7] fuse → HF safetensors
echo "==> [3/7] mlx-lm fuse → HF safetensors"
python3 -m mlx_lm fuse \
    --model "$MODEL" \
    --adapter-path "$ADAPTER_DIR" \
    --save-path "$SAFETENSORS_DIR" \
    --dequantize

# [4/7] safetensors → fp16 GGUF
echo "==> [4/7] convert_hf_to_gguf.py → fp16 GGUF"
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" \
    "$SAFETENSORS_DIR" \
    --outfile "$GGUF_F16" \
    --outtype f16

# [5/7] fp16 → Q4_K_M
echo "==> [5/7] llama-quantize → Q4_K_M GGUF"
"$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$GGUF_Q4" Q4_K_M

# [6/7] parser-only baseline JSON（用 baseline GGUF）
echo "==> [6/7] parser-only baseline evals → JSON"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
./target/release/evals \
    --fixtures v0.5 \
    --json \
    > "$BASELINE_JSON" 2> training/mlx-lora/fused/main-v0-baseline.log

# [7/7] with-fallback --hybrid evals + 自动 diff
echo "==> [7/7] v0.5 evals --with-fallback --hybrid (model=main-v0-q4_k_m.gguf)"
LOCIFIND_MODEL_PATH="$GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --with-fallback \
        --hybrid \
        --baseline "$BASELINE_JSON" \
        --json \
        > "$EVALS_JSON" 2> training/mlx-lora/fused/main-v0-evals.log

echo "✅ main run complete"
ls -lh "$GGUF_Q4"
echo "=== baseline pass / partial / fail（提取自 JSON）==="
python3 -c "import json; d=json.load(open('$BASELINE_JSON')); print(d.get('summary'))" 2>&1
echo "=== with-fallback pass / partial / fail（提取自 JSON）==="
python3 -c "import json; d=json.load(open('$EVALS_JSON')); print(d.get('summary'))" 2>&1
```

### 4.2 关键点

- [6/7] 跑 parser-only baseline（不带 --with-fallback）。**不**用 baseline GGUF，因为 parser-only 路径根本不调模型
- [7/7] 用我们训出的 `main-v0-q4_k_m.gguf` + `--with-fallback --hybrid` + `--baseline` 自动 diff
- `--baseline` flag 让 evals 自己输出 rescued/regressed/pass-changes 统计
- stderr 走 log 文件，stdout 走 JSON（避免日志噪声混入 JSON）

### 4.3 预计耗时

| 步骤 | 耗时 |
|---|---|
| [1] prepare data | <1 s |
| [2] lora train 1000 step | 5-15 min |
| [3] fuse | 10-30 s |
| [4] convert_hf_to_gguf.py | 1-3 min |
| [5] llama-quantize | 30-60 s |
| [6] parser-only evals | 1-2 min |
| [7] with-fallback evals | 10-30 min（取决于 fallback 触发率与推理延迟）|
| **总** | **20-50 min** |

## 5. 出场报告（docs/reviews/beta-08-lora-v0.md）

### 5.1 关键指标对比表

| 指标 | parser-only baseline | BETA-08 LoRA v0 | Δ |
|---|---|---|---|
| pass | <baseline.json> | <evals.json> | <Δ> |
| partial | <baseline.json> | <evals.json> | <Δ> |
| fail | <baseline.json> | <evals.json> | <Δ> |
| **regressed**（baseline pass→现 fail/partial） | 0 | **<evals.json>** | — |
| **rescued_to_pass**（baseline fail/partial→现 pass） | 0 | **<evals.json>** | — |
| variant 命中率 | <baseline.json> | <evals.json> | <Δ> |
| valid_intent 比率（fallback 触发部分） | — | <evals.json> | — |
| p95 fallback 延迟 (ms) | — | <evals.json> | — |
| fallback 触发率 | — | <evals.json> | — |

### 5.2 门槛核对

```
门槛 1: pass 净增 ≥ 5
  baseline=<X>, v0=<Y>, Δ=<Y-X>, status=<PASS|FAIL>

门槛 2: regressed ≤ 2
  v0=<X>, status=<PASS|FAIL>

总结论：<v0 ready|v0 not_ready，原因>
```

### 5.3 实测时间线

| 步骤 | 耗时 |
|---|---|
| [1/7] prepare data | <X> s |
| [2/7] lora train 1000 step | <X> min |
| [3/7] fuse | <X> s |
| [4/7] convert_hf_to_gguf.py | <X> min |
| [5/7] llama-quantize | <X> s |
| [6/7] parser-only evals | <X> min |
| [7/7] with-fallback evals | <X> min |
| **总耗时** | **<X> min** |

含 adapter 大小、fp16 GGUF 大小、Q4_K_M GGUF 大小。

### 5.4 失败案例分桶

按 evals.json 里 `partial` / `fail` 的 `diff` 字段分类列前 5 大桶，每桶 1-2 个 case_id + query 示例：

- 若 v0 ready：列"虽达门槛但仍有的弱点" + 下一步建议
- 若 v0 not_ready：列"主要回归方向" + 是否值得继续 v1

### 5.5 下一步

**如 v0 ready**：
- 评估 Q5_K_M / Q8_0 量化看精度上限（可选）
- v1 数据集是否值得（依新失败模式）
- 集成 fallback 到产品默认行为（修 MVP-17 §9.5）

**如 v0 not_ready**：
- 诊断回归方向：(a) 调超参重跑（lr / iters）(b) 升 v1 数据 (c) 改训练目标设计

## 6. 风险与失败处置

| # | 风险 | 概率 | 处置 |
|---|---|---|---|
| M1 | mlx-lm 1000 step OOM | 低 | smoke batch=2/layers=8 peak 5.1 GB；双倍预估 ~10-12 GB 应安全。OOM 则 batch 降 2 重跑 |
| M2 | convert_hf_to_gguf.py 不识别 Qwen2 | 低 | llama.cpp 主流支持。失败则检查 mlx fuse 的 config.json 完整性 |
| M3 | llama-quantize 失败 | 低 | 主流路径成熟 |
| M4 | evals --json 输出格式与 §5 表抽取脚本不符 | 中 | plan 写之前用空跑 evals --json 看实际 schema；如需调整 plan 的 `.summary` 路径 |
| M5 | [7/7] evals 跑全 500 case 超 1 小时 | 中 | 仅作 ledger 记录，不视为失败。可考虑 `--fallback-subset` 但会失去全量门槛判定 |
| M6 | adapter regressed > 2 | 中 | 接 not_ready 结论，§5.5 选 not_ready 分支 |
| M7 | adapter pass 净增 < 5 | 中 | 同 M6 |

## 7. 验收清单

| 项 | 必满足 |
|---|---|
| `bash training/mlx-lora/scripts/run_main.sh` 完整 exit 0 | ✓ |
| `training/mlx-lora/fused/main-v0-q4_k_m.gguf` 存在，~1 GB | ✓ |
| `training/mlx-lora/fused/main-v0-{baseline,evals}.json` 都存在 | ✓ |
| `docs/reviews/beta-08-lora-v0.md` 落地，§5.1-§5.5 全填实测数字 | ✓ |
| 报告判定 v0 ready / not_ready 明确（门槛 1+2 同时满足 = ready） | ✓ |
| STATUS / ROADMAP 同步 BETA-08 状态（done if ready，否则 in_progress + 下一步建议） | ✓ |
| 单次中文 commit（spec + plan + 脚本 + ledger + STATUS + ROADMAP）| ✓ |

**ready 与 not_ready 都是合法结案**。
