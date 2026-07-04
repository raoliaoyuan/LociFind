#!/usr/bin/env bash
# BETA-08 LoRA smoke run（spike Level 1）
# 详见 docs/superpowers/specs/2026-05-27-beta-08-smoke-design.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

# 优先用本地缓存（~828 MB Qwen2.5-1.5B-Instruct 4bit MLX 格式），
# 不在场则 fallback 到 HF 仓库（首次跑会下载）。
if [[ -d "$HOME/models/qwen25_1.5b_draft" ]]; then
    MODEL="$HOME/models/qwen25_1.5b_draft"
else
    MODEL="mlx-community/Qwen2.5-1.5B-Instruct-4bit"
fi
DATA_DIR="training/mlx-lora/data/smoke"
ADAPTER_DIR="training/mlx-lora/adapters/smoke-v0"
FUSED_GGUF="training/mlx-lora/fused/smoke-v0-f16.gguf"

# [1/4] 准备数据
echo "==> [1/4] prepare data"
python3 training/mlx-lora/scripts/prepare_smoke_data.py

# [2/4] LoRA 训练
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

# [3/4] fuse + export GGUF
echo "==> [3/4] fuse & export GGUF (fp16)"
mkdir -p "$(dirname "$FUSED_GGUF")"
python3 -m mlx_lm fuse \
    --model "$MODEL" \
    --adapter-path "$ADAPTER_DIR" \
    --dequantize \
    --export-gguf \
    --gguf-path "$FUSED_GGUF"

# [4/4] 验证 llama-cpp-4 加载
echo "==> [4/4] verify GGUF loads in llama-cpp-4"
cargo build --release -p locifind-evals --bin fallback_probe --features model-fallback-metal
LOCIFIND_MODEL_PATH="$FUSED_GGUF" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/fallback_probe "查找昨天编辑过的 ppt" \
    2>&1 | tail -40

echo "✅ smoke run complete: $FUSED_GGUF"
ls -lh "$FUSED_GGUF"
