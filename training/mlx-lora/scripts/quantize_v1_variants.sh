#!/usr/bin/env bash
# BETA-09 v1 量化 baseline：Q5_K_M + Q6_K
# 详见 docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
FUSED="training/mlx-lora/fused"
GGUF_F16="$FUSED/main-v1-f16.gguf"
BASELINE_JSON="$FUSED/main-v1-baseline.json"

if [[ ! -f "$GGUF_F16" ]]; then
    echo "❌ 缺 v1 fp16 GGUF: $GGUF_F16" >&2
    exit 1
fi
if [[ ! -f "$BASELINE_JSON" ]]; then
    echo "❌ 缺 v1 parser-only baseline: $BASELINE_JSON" >&2
    exit 1
fi
if [[ ! -x "$LLAMA_CPP/build/bin/llama-quantize" ]]; then
    echo "❌ 缺 llama-quantize 工具" >&2
    exit 1
fi

quantize_and_evals() {
    local label="$1"   # Q5_K_M / Q6_K
    local suffix="$2"  # q5_k_m / q6_k
    local out_gguf="$FUSED/main-v1-${suffix}.gguf"
    local out_log="$FUSED/main-v1-${suffix}-evals.log"

    echo "==> [$label] quantize"
    "$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$out_gguf" "$label"

    echo "==> [$label] evals (--with-fallback --hybrid)"
    LOCIFIND_MODEL_PATH="$out_gguf" \
    DYLD_LIBRARY_PATH="$ROOT/target/release" \
        ./target/release/evals \
            --fixtures v0.5 \
            --with-fallback \
            --hybrid \
            --baseline "$BASELINE_JSON" \
            2>&1 | tee "$out_log"
}

quantize_and_evals "Q5_K_M" "q5_k_m"
quantize_and_evals "Q6_K"   "q6_k"

echo "✅ v1 variants done"
ls -lh "$FUSED/main-v1-q5_k_m.gguf" "$FUSED/main-v1-q6_k.gguf"
