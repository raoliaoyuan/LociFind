#!/usr/bin/env bash
# BETA-17 工具链冒烟门：验证一个候选基座能被完整工具链消费。
# 用法: smoke_candidate.sh <mlx_repo_id> <slug>
# 例:   smoke_candidate.sh mlx-community/Qwen3-0.6B-4bit qwen3-0.6b
# 退出码 0=过门, 非0=某环节失败（stderr 记原因）。
set -euo pipefail

REPO_ID="${1:?需要 mlx repo id}"
SLUG="${2:?需要 slug}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
WORK="training/mlx-lora/smoke/$SLUG"
SMOKE_ADAPTER="$WORK/adapter"
SMOKE_FUSED="$WORK/fused-safetensors"
SMOKE_GGUF_F16="$WORK/$SLUG-f16.gguf"
SMOKE_GGUF_Q4="$WORK/$SLUG-q4_k_m.gguf"
mkdir -p "$WORK"

echo "==> [1/4] 拉基座 + 确认纯文本架构: $REPO_ID"
python3 - "$REPO_ID" <<'PY'
import sys
from huggingface_hub import snapshot_download
import json, pathlib
repo = sys.argv[1]
path = snapshot_download(repo)
cfg = json.loads((pathlib.Path(path) / "config.json").read_text())
arch = cfg.get("architectures", [])
assert "vision_config" not in cfg, f"❌ {repo} 含 vision_config（多模态），跳过"
print(f"   架构={arch} 纯文本 OK，本地路径={path}")
PY

echo "==> [2/4] mlx-lm 最小 LoRA（4 step，验架构识别）"
python3 -m mlx_lm lora \
    --model "$REPO_ID" \
    --train \
    --data training/mlx-lora/data/smoke \
    --fine-tune-type lora \
    --num-layers 4 \
    --iters 4 \
    --batch-size 1 \
    --adapter-path "$SMOKE_ADAPTER" \
    --seed 42

echo "==> [3/4] fuse → GGUF → Q4_K_M"
python3 -m mlx_lm fuse \
    --model "$REPO_ID" \
    --adapter-path "$SMOKE_ADAPTER" \
    --save-path "$SMOKE_FUSED" \
    --dequantize
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" "$SMOKE_FUSED" --outfile "$SMOKE_GGUF_F16" --outtype f16
"$LLAMA_CPP/build/bin/llama-quantize" "$SMOKE_GGUF_F16" "$SMOKE_GGUF_Q4" Q4_K_M

echo "==> [4/4] 钉死 llama-cpp-sys-4 0.3.0 跑最小推理（产合法 JSON / 无 think 块）"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
LOCIFIND_MODEL_PATH="$SMOKE_GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --with-fallback \
        --hybrid \
        --limit 5 \
        --json > "$WORK/smoke-evals.json"
python3 - "$WORK/smoke-evals.json" <<'PY'
import sys, json
raw = open(sys.argv[1]).read()
data = json.loads(raw)  # 必须是合法 JSON，否则抛错
assert "<think>" not in raw, "❌ 输出含 <think> 块（thinking 未抑制）"
print("   ✅ 合法 JSON，无 think 块；fallback 样本通过")
PY

echo "✅ [$SLUG] 冒烟门通过"
