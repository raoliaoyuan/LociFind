#!/usr/bin/env bash
# BETA-08 LoRA v1 主体 run (mask-prompt + nonempty oversample 8x)
# 详见 docs/superpowers/specs/2026-05-27-beta-08-v1-design.md
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
ADAPTER_DIR="training/mlx-lora/adapters/main-v1"
SAFETENSORS_DIR="training/mlx-lora/fused/main-v1-safetensors"
GGUF_F16="training/mlx-lora/fused/main-v1-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/main-v1-q4_k_m.gguf"
BASELINE_JSON="training/mlx-lora/fused/main-v1-baseline.json"
EVALS_LOG="training/mlx-lora/fused/main-v1-evals.log"

# [1/7] 准备数据
echo "==> [1/7] prepare data"
python3 training/mlx-lora/scripts/prepare_main_data.py

# [2/7] mlx-lm lora train
echo "==> [2/7] mlx-lm lora train (1000 step / num-layers 16 / batch 4, mask-prompt + oversample 8x)"
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
    --mask-prompt \
    --seed 42

# [3/7] mlx-lm fuse → HF safetensors
echo "==> [3/7] mlx-lm fuse → HF safetensors"
python3 -m mlx_lm fuse \
    --model "$MODEL" \
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

# [6/7] parser-only baseline evals → JSON
# 注：evals binary 编时静态链 llama-cpp，运行时即便 parser-only 也需 DYLD_LIBRARY_PATH
echo "==> [6/7] parser-only baseline evals → JSON"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --json \
        > "$BASELINE_JSON"

# [7/7] with-fallback hybrid evals + 自动 diff（文本日志）
echo "==> [7/7] v0.5 evals --with-fallback --hybrid (model=main-v1-q4_k_m.gguf)"
LOCIFIND_MODEL_PATH="$GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --with-fallback \
        --hybrid \
        --baseline "$BASELINE_JSON" \
        2>&1 | tee "$EVALS_LOG"

echo "✅ main run complete"
ls -lh "$GGUF_Q4"
