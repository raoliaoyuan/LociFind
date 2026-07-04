# BETA-08 LoRA smoke run — 设计 spec

| 项 | 值 |
|---|---|
| ID | BETA-08（spike 子任务） |
| 作者 | Claude Code (Opus 4.7) |
| 日期 | 2026-05-27 |
| 阶段 | M 阶段尾 → B 阶段衔接（BETA-08 主体之前的 spike） |
| 依赖 spec | [BETA-08 LoRA 启动设计](./2026-05-27-beta-08-lora-design.md) §9 R5 |
| 后续 | 实施 plan（writing-plans 产出）→ BETA-08 主体下一会话 |

> **本 spec 是 spike，不是产品 task**。目标是**学习**，不是**成功**。任何 failure mode 都是有效产出。

## 1. 目标与范围

### 1.1 目标

验证 **mlx-lm LoRA 训练 → fuse 出 GGUF → 被 llama-cpp-4 加载** 这条完整链路在我们的数据 + 基座上跑得通；为 BETA-08 主体 run 排除 [设计 spec §9 R5 risk](./2026-05-27-beta-08-lora-design.md#9-风险)（mlx-lm 训练完发现 llama.cpp 工具链不认 LoRA / 合并产物，导致主体 run 评测卡住）。

### 1.2 范围

- **数据**：`training/datasets/v0.5-patch/v0/cases.jsonl` 随机切 train=100 / valid=20（seed=42）
- **基座**：`mlx-community/Qwen2.5-1.5B-Instruct-4bit`（HF 缓存已有，零下载）
- **训练**：mlx-lm lora，50 step，rank=8（`--num-layers 8`），batch=2，lr=1e-4，预计 <5 min
- **GGUF 输出**：mlx-lm fuse `--dequantize --export-gguf` 直接出 fp16 GGUF（绕过 llama.cpp 工具链）
- **验证**：`fallback_probe` binary 加载 fp16 GGUF 不 crash + 能输出 token；**不**验证 patch 准确度
- **总时长预算**：1-1.5 小时（含偶尔 retry）

### 1.3 不在范围

- Q4_K_M 量化（fp16 GGUF 够 evals 用，量化是产品体积优化属正式 run）
- 正式 500-1000 step 训练
- v0.5 evals --with-fallback --hybrid 验门槛
- 出场报告 `docs/reviews/beta-08-lora-v0.md`（这是主体 run 的产出，spike 只出 ledger）

## 2. 文件布局

```
training/mlx-lora/
├── README.md                                 # 更新：smoke 章节
├── scripts/
│   ├── prepare_smoke_data.py                 # 新建（~40 行 Python）
│   └── run_smoke.sh                          # 新建（~30 行 shell，串联 4 步）
├── data/
│   └── smoke/
│       ├── train.jsonl                       # 100 行，git-ignored（合成衍生）
│       └── valid.jsonl                       # 20 行，git-ignored
├── adapters/
│   └── smoke-v0/                             # mlx-lm 训练输出，git-ignored
│       └── adapters.safetensors
└── fused/
    └── smoke-v0-f16.gguf                     # mlx-lm fuse 输出，git-ignored

docs/superpowers/specs/
└── 2026-05-27-beta-08-smoke-design.md        # 本 spec，commit 入库

docs/reviews/
└── beta-08-smoke.md                          # smoke 跑完后的简短 ledger，commit 入库
```

### 2.1 .gitignore 新增

追加到根 `.gitignore`（项目其他 ignored 路径已在那里维护，集中一处更清晰）：

```
training/mlx-lora/data/
training/mlx-lora/adapters/
training/mlx-lora/fused/
```

**理由**：
- `data/` 由 `prepare_smoke_data.py` + seed=42 完全确定性可重生
- `adapters/` 与 `fused/` 是训练产物，体积大、不入库
- `scripts/` 留 commit 入库，是真"代码"层

## 3. 数据准备脚本

### 3.1 输入 / 输出

| 项 | 值 |
|---|---|
| 输入 | `training/datasets/v0.5-patch/v0/cases.jsonl`（498 行） |
| 输出 | `training/mlx-lora/data/smoke/train.jsonl`（100 行）+ `valid.jsonl`（20 行） |
| 调用 | `python3 training/mlx-lora/scripts/prepare_smoke_data.py`（从仓库根；无参数） |

### 3.2 算法

```python
1. 读 cases.jsonl 全部 498 行 → List[dict]
2. random.seed(42); random.shuffle(list)
3. 取前 120 行
4. 每行只保留 {"prompt": ..., "completion": ...}（drop case_id / fillable_fields / draft_variant）
5. 前 100 行写 train.jsonl，后 20 行写 valid.jsonl
6. 排序：不排序（保留 shuffle 顺序，让 train/valid 是随机划分）
```

### 3.3 确定性

`seed=42` + 输入文件 sha256 锚定（cases.jsonl 来自 BETA-08 设计 spec §3.4 锚定的 dataset v0）→ 输出永远字节一致。

### 3.4 自检

脚本末尾内联：

```python
assert os.path.exists("training/mlx-lora/data/smoke/train.jsonl")
with open("training/mlx-lora/data/smoke/train.jsonl") as f:
    lines = f.readlines()
    assert len(lines) == 100
    sample = json.loads(lines[0])
    assert set(sample.keys()) == {"prompt", "completion"}
print("✅ smoke data prepared")
```

## 4. 训练与 fuse 脚本（run_smoke.sh）

### 4.1 编排

单一 shell 脚本 4 步，`set -euo pipefail` 任一失败立即退出：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

MODEL="mlx-community/Qwen2.5-1.5B-Instruct-4bit"
DATA_DIR="training/mlx-lora/data/smoke"
ADAPTER_DIR="training/mlx-lora/adapters/smoke-v0"
FUSED_GGUF="training/mlx-lora/fused/smoke-v0-f16.gguf"

# 1. 准备数据
echo "==> [1/4] prepare data"
python3 training/mlx-lora/scripts/prepare_smoke_data.py

# 2. 训练
echo "==> [2/4] lora train (50 step, rank 8)"
python3 -m mlx_lm lora \
    --model "$MODEL" \
    --train \
    --data "$DATA_DIR" \
    --fine-tune-type lora \
    --num-layers 8 \
    --iters 50 \
    --batch-size 2 \
    --learning-rate 1e-4 \
    --steps-per-report 10 \
    --steps-per-eval 25 \
    --adapter-path "$ADAPTER_DIR" \
    --seed 42

# 3. fuse + export GGUF
echo "==> [3/4] fuse & export GGUF (fp16)"
mkdir -p "$(dirname "$FUSED_GGUF")"
python3 -m mlx_lm fuse \
    --model "$MODEL" \
    --adapter-path "$ADAPTER_DIR" \
    --dequantize \
    --export-gguf \
    --gguf-path "$FUSED_GGUF"

# 4. 验证 llama-cpp-4 能加载
echo "==> [4/4] verify GGUF loads in llama-cpp-4"
cargo run --release -p locifind-evals --bin fallback_probe \
    --features model-fallback-metal -- \
    --model "$FUSED_GGUF" \
    --query "查找昨天编辑过的 ppt" \
    2>&1 | tail -20

echo "✅ smoke run complete: $FUSED_GGUF"
```

### 4.2 关键超参选择

| 参数 | 值 | 理由 |
|---|---|---|
| `--num-layers` | 8 | 默认 16，砍半减训练时间。Spike 不求收敛 |
| `--iters` | 50 | 50 step 足够暴露 OOM / NaN / format 类问题 |
| `--batch-size` | 2 | 1.5B + Mac unified memory，保守起步 |
| `--learning-rate` | 1e-4 | mlx-lm 推荐起点 |
| `--steps-per-eval` | 25 | 跑 2 次 val 看 loss 是否下降 |

### 4.3 第 4 步验证策略

复用现成 `fallback_probe` binary（packages/evals/src/bin/fallback_probe.rs），已知能加载 GGUF 跑单 query 推理。spike 看到它 print 出某种 JSON-shaped 输出（不必合法 / 不必准确）即算"GGUF 能被 llama-cpp-4 加载"。

**关键点**：复用现有 wiring，不写新验证代码 — 与 BETA-08 设计 §1.3 不在范围一致。

## 5. 风险与失败处置

| # | 风险 | 概率 | spike 时处置 |
|---|---|---|---|
| S1 | mlx-lm 训练 OOM（Mac unified memory 不足） | 低 | `--batch-size` 降到 1；如仍 OOM → smoke 失败，记录设备 spec 与上限 |
| S2 | mlx-lm fuse `--export-gguf` 不支持 Qwen2 架构 | 中 | spike 结论：fp16 GGUF 直转不可行，正式 run 改走 mlx → safetensors → llama.cpp convert 路径 |
| S3 | mlx-lm 出的 GGUF 与 llama-cpp-4 0.3.0 不兼容（GGUF 版本差异） | 中 | spike 结论：需对齐 GGUF 版本，可能要升级 llama-cpp-4 crate；记录确切错误信息 |
| S4 | 训练 50 step 后 loss = NaN | 低 | `--learning-rate` 降到 5e-5 重跑；如仍 NaN → spike 失败，记录原因 |
| S5 | 4bit 模型 LoRA 训练报 "frozen layers can't backprop" 类错误 | 低 | 二次尝试：切到 fp16 HF 原版（需 ~3GB 下载）；本会话不一定来得及做 |

**核心原则**：spike 是为了**学习**，不是为了**成功**。任何 failure mode 都是有效产出，只要 ledger 里记录清楚。

## 6. 验收清单

满足以下**任一** Path：

### 6.1 Path A（理想路径）

- [ ] `bash training/mlx-lora/scripts/run_smoke.sh` 跑到第 4 步且 exit 0
- [ ] `fallback_probe` 加载 fp16 GGUF 输出某种 JSON-shaped 文本（不需合法 / 不需准确）
- [ ] `docs/reviews/beta-08-smoke.md` 落地，含 4 步实测时间 + adapter 大小 + GGUF 大小 + 一句"Path A 成功"

### 6.2 Path B（失败但有结论）

- [ ] 在第 N 步失败，错误信息完整记录
- [ ] `docs/reviews/beta-08-smoke.md` 落地，含失败步骤 + 错误原因分析 + 正式 run 建议的 Plan B 路径（如 S2 切 fp16 原模型）

任一 Path 满足都算 spike 收工。

## 7. 本会话 vs BETA-08 主体边界

| 工作项 | 本 spike 会话 | BETA-08 主体下一会话 |
|---|---|---|
| spec 文档（本文件） | ✅ | — |
| 数据准备脚本 | ✅ | — |
| 训练编排 shell 脚本 | ✅（smoke 50 step 版） | 改写或追加正式 run 版（500-1000 step） |
| 跑 smoke 训练 + fuse + 验证加载 | ✅ | — |
| smoke ledger | ✅ | — |
| 跑正式 500-1000 step 训练 | — | ✅ |
| Q4_K_M 量化 | — | ✅（如 spike 验证 Path A 可行）|
| 跑 v0.5 evals --with-fallback --hybrid 验门槛 | — | ✅ |
| 出场报告 `docs/reviews/beta-08-lora-v0.md` | — | ✅ |
| STATUS / ROADMAP 同步 | spike 阶段同步（"smoke done / Path A or B"）| 主体阶段同步 |
