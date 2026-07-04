#!/usr/bin/env bash
# BETA-24：keywords 补全重训。基座/超参完全对齐 BETA-17 winner（Qwen3-0.6B + v1 配方），
# 单一变量 = 训练数据并入 lora-aug-keywords。
set -euo pipefail

REPO_ID="mlx-community/Qwen3-0.6B-4bit"
SLUG="beta24-qwen3-0.6b"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
DATA_DIR="training/mlx-lora/data/main"
ADAPTER_DIR="training/mlx-lora/adapters/$SLUG"
SAFETENSORS_DIR="training/mlx-lora/fused/$SLUG-safetensors"
GGUF_F16="training/mlx-lora/fused/$SLUG-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/$SLUG-q4_k_m.gguf"

echo "==> [1/6] prepare data（v0.5-patch + lora-aug-keywords）"
python3 training/mlx-lora/scripts/prepare_main_data.py \
    --keywords-aug training/datasets/lora-aug-keywords/v1/cases.jsonl

echo "==> [2/6] mlx-lm lora train（v1 配方：1000 step / 16 layers / batch 4 / lr 1e-4 / mask-prompt / seed 42）"
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

echo "==> [3/6] mlx-lm fuse → HF safetensors"
python3 -m mlx_lm fuse \
    --model "$REPO_ID" \
    --adapter-path "$ADAPTER_DIR" \
    --save-path "$SAFETENSORS_DIR" \
    --dequantize

echo "==> [4/6] convert_hf_to_gguf.py → fp16 GGUF"
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" \
    "$SAFETENSORS_DIR" \
    --outfile "$GGUF_F16" \
    --outtype f16

echo "==> [5/6] llama-quantize → Q4_K_M GGUF"
"$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$GGUF_Q4" Q4_K_M

echo "==> [6/6] parser-only baseline（v0.5 应 473 / v0.9 应 726）"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.5 --json > "training/mlx-lora/fused/$SLUG-v05-baseline.json"
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.9 --json > "training/mlx-lora/fused/$SLUG-v09-baseline.json"

echo "==> GGUF 体积 + sha256"
ls -lh "$GGUF_Q4"
shasum -a 256 "$GGUF_Q4"
echo "✅ [$SLUG] 训练完成；with-fallback 三层验收见 plan Task 9"
