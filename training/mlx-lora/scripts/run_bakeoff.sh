#!/usr/bin/env bash
# BETA-17 bake-off：对一个过门候选跑 v1 同配方训练 + 双轨 evals。
# 用法: run_bakeoff.sh <mlx_repo_id> <slug>
# 超参完全对齐 BETA-08 v1（run_main_v1.sh）：单一变量=只换基座。
set -euo pipefail

REPO_ID="${1:?需要 mlx repo id}"
SLUG="${2:?需要 slug}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
DATA_DIR="training/mlx-lora/data/main"
ADAPTER_DIR="training/mlx-lora/adapters/beta17-$SLUG"
SAFETENSORS_DIR="training/mlx-lora/fused/beta17-$SLUG-safetensors"
GGUF_F16="training/mlx-lora/fused/beta17-$SLUG-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/beta17-$SLUG-q4_k_m.gguf"
BASELINE_JSON="training/mlx-lora/fused/beta17-$SLUG-baseline.json"
EVALS_LOG="training/mlx-lora/fused/beta17-$SLUG-evals.log"

# [1/7] 准备数据（复用 v1 锁定数据集，零改动）
echo "==> [1/7] prepare data（复用 v1 锁定数据集，零改动）"
python3 training/mlx-lora/scripts/prepare_main_data.py

# [2/7] mlx-lm lora train（v1 同配方：1000 step / 16 layers / batch 4 / lr 1e-4 / mask-prompt / oversample 8x）
echo "==> [2/7] mlx-lm lora train (1000 step / num-layers 16 / batch 4, mask-prompt + oversample 8x)"
python3 -m mlx_lm lora \
    --model "$REPO_ID" \
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
    --mask-prompt \
    --seed 42

# [3/7] mlx-lm fuse → HF safetensors
echo "==> [3/7] mlx-lm fuse → HF safetensors"
python3 -m mlx_lm fuse \
    --model "$REPO_ID" \
    --adapter-path "$ADAPTER_DIR" \
    --save-path "$SAFETENSORS_DIR" \
    --dequantize

# [4/7] HF safetensors → fp16 GGUF
echo "==> [4/7] convert_hf_to_gguf.py → fp16 GGUF"
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" \
    "$SAFETENSORS_DIR" \
    --outfile "$GGUF_F16" \
    --outtype f16

# [5/7] fp16 → Q4_K_M 量化
echo "==> [5/7] llama-quantize → Q4_K_M GGUF"
"$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$GGUF_Q4" Q4_K_M

# [6/7] parser-only baseline evals → JSON（应 472/26/2）
echo "==> [6/7] parser-only baseline evals → JSON（应 472/26/2）"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --json \
        > "$BASELINE_JSON"

# [7/7] with-fallback hybrid evals + 自动 diff（文本日志）
echo "==> [7/7] v0.5 evals --with-fallback --hybrid (model=$GGUF_Q4)"
LOCIFIND_MODEL_PATH="$GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --with-fallback \
        --hybrid \
        --baseline "$BASELINE_JSON" \
        2>&1 | tee "$EVALS_LOG"

echo "==> GGUF 体积 + sha256"
ls -lh "$GGUF_Q4"
shasum -a 256 "$GGUF_Q4"
echo "✅ [$SLUG] bake-off run 完成"
