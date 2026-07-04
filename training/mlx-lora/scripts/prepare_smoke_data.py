#!/usr/bin/env python3
"""BETA-08 smoke run 数据准备：从 v0.5-patch/v0 切 100/20 train/valid jsonl。

确定性：random.seed(42)。从仓库根运行。
"""
from __future__ import annotations

import json
import random
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
INPUT = REPO_ROOT / "training" / "datasets" / "v0.5-patch" / "v0" / "cases.jsonl"
OUTDIR = REPO_ROOT / "training" / "mlx-lora" / "data" / "smoke"
N_TRAIN = 100
N_VALID = 20
SEED = 42


def main() -> int:
    if not INPUT.exists():
        print(f"❌ 未找到输入文件: {INPUT}", file=sys.stderr)
        return 1

    with INPUT.open("r", encoding="utf-8") as f:
        lines = [json.loads(line) for line in f if line.strip()]

    if len(lines) < N_TRAIN + N_VALID:
        print(
            f"❌ 输入文件只有 {len(lines)} 行，少于需要的 {N_TRAIN + N_VALID}",
            file=sys.stderr,
        )
        return 1

    rng = random.Random(SEED)
    rng.shuffle(lines)
    subset = lines[: N_TRAIN + N_VALID]

    # mlx-lm 只看 prompt + completion；drop 其他字段
    minimal = [{"prompt": rec["prompt"], "completion": rec["completion"]} for rec in subset]

    OUTDIR.mkdir(parents=True, exist_ok=True)
    train_path = OUTDIR / "train.jsonl"
    valid_path = OUTDIR / "valid.jsonl"

    with train_path.open("w", encoding="utf-8") as f:
        for rec in minimal[:N_TRAIN]:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    with valid_path.open("w", encoding="utf-8") as f:
        for rec in minimal[N_TRAIN : N_TRAIN + N_VALID]:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")

    # 自检
    with train_path.open("r", encoding="utf-8") as f:
        train_lines = f.readlines()
    assert len(train_lines) == N_TRAIN, f"train.jsonl 应有 {N_TRAIN} 行"
    sample = json.loads(train_lines[0])
    assert set(sample.keys()) == {"prompt", "completion"}, "train.jsonl 字段集应是 prompt+completion"

    print(f"✅ smoke data prepared: train={N_TRAIN}, valid={N_VALID}, outdir={OUTDIR}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
