#!/usr/bin/env python3
"""BETA-08 主体 run 数据准备：v0.5-patch/v0 全 498 进 train，末尾 50 复制到 valid。

确定性：random.seed(42)。从仓库根运行。

BETA-24 扩展：`--keywords-aug <path>` 可选旗标，混入 keywords 补全数据集。
不传旗标时行为与 v1 现状逐字节一致（向后兼容硬约束）。
"""
from __future__ import annotations

import argparse
import json
import sys
import random
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[3]
INPUT = REPO_ROOT / "training" / "datasets" / "v0.5-patch" / "v0" / "cases.jsonl"
OUTDIR = REPO_ROOT / "training" / "mlx-lora" / "data" / "main"
N_VALID_TAIL = 50
SEED = 42
NONEMPTY_OVERSAMPLE = 8  # v1: nonempty 重复 8× 达 ~50/50 balance；设 1 等价 v0
KEYWORDS_OVERSAMPLE = 3  # BETA-24：keywords-aug 重复倍数（122×3=366，占总量约 29%，三桶均衡）


def _is_empty_patch(completion: str) -> bool:
    """v0.5-patch/v0 dataset 中 empty patch completion 是字面 "{}"（验证：443/498）。"""
    return completion.strip() == "{}"


def main() -> int:
    ap = argparse.ArgumentParser(
        description="准备 mlx-lm LoRA 训练数据（chat 格式 JSONL）"
    )
    ap.add_argument(
        "--keywords-aug",
        type=Path,
        default=None,
        metavar="PATH",
        help="BETA-24：keywords 补全数据集 cases.jsonl（不传=v1 现状行为，逐字节一致）",
    )
    cli = ap.parse_args()

    if not INPUT.exists():
        print(f"❌ 未找到输入文件: {INPUT}", file=sys.stderr)
        return 1

    with INPUT.open("r", encoding="utf-8") as f:
        lines = [json.loads(line) for line in f if line.strip()]

    rng = random.Random(SEED)
    # chat 格式：mlx-lm 0.29.1 CompletionsDataset.process 在 --mask-prompt 分支有 bug
    # (datasets.py:112 把单 dict 当 list 传给 apply_chat_template 致 jinja 崩溃)
    # 用 ChatDataset path 绕过：每条 record 包成 {"messages": [user, assistant]}
    minimal = [
        {
            "messages": [
                {"role": "user", "content": rec["prompt"]},
                {"role": "assistant", "content": rec["completion"]},
            ]
        }
        for rec in lines
    ]

    def _completion(rec: dict) -> str:
        return rec["messages"][-1]["content"]

    empty = [r for r in minimal if _is_empty_patch(_completion(r))]
    nonempty = [r for r in minimal if not _is_empty_patch(_completion(r))]
    oversampled = nonempty * NONEMPTY_OVERSAMPLE

    # BETA-24：keywords-aug 混合（默认 keywords_records=[]，拼接结果与现状完全相同）
    keywords_records: list[dict] = []
    if cli.keywords_aug is not None:
        if not cli.keywords_aug.exists():
            print(f"❌ 未找到 keywords-aug: {cli.keywords_aug}", file=sys.stderr)
            return 1
        with cli.keywords_aug.open("r", encoding="utf-8") as f:
            kw_lines = [json.loads(line) for line in f if line.strip()]
        kw_minimal = [
            {
                "messages": [
                    {"role": "user", "content": rec["prompt"]},
                    {"role": "assistant", "content": rec["completion"]},
                ]
            }
            for rec in kw_lines
        ]
        # keywords-aug 数据为 draft ⊕ keywords 差异，completion 必非空
        assert all(not _is_empty_patch(_completion(r)) for r in kw_minimal), \
            "keywords-aug 不应含 empty patch（expected = draft ⊕ keywords 必非空）"
        keywords_records = kw_minimal * KEYWORDS_OVERSAMPLE

    # 拼接顺序固定：empty + oversampled + keywords_records
    # keywords_records 默认空列表 → 不传旗标时 all_records 与 v1 完全相同，shuffle 结果逐字节一致
    all_records = empty + oversampled + keywords_records
    rng.shuffle(all_records)

    OUTDIR.mkdir(parents=True, exist_ok=True)
    train_path = OUTDIR / "train.jsonl"
    valid_path = OUTDIR / "valid.jsonl"

    with train_path.open("w", encoding="utf-8") as f:
        for rec in all_records:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    with valid_path.open("w", encoding="utf-8") as f:
        # valid 是 train 末尾 50 条复制（仅满足 mlx-lm 接口约束；评测靠 packages/evals 全 500）
        for rec in all_records[-N_VALID_TAIL:]:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")

    with train_path.open("r", encoding="utf-8") as f:
        train_lines = f.readlines()
    assert len(train_lines) == len(all_records), f"train 应有 {len(all_records)} 行"
    sample = json.loads(train_lines[0])
    assert set(sample.keys()) == {"messages"}, "字段集应仅含 messages（chat format）"
    assert len(sample["messages"]) == 2, "messages 应有 2 轮 (user + assistant)"
    assert sample["messages"][0]["role"] == "user", "first role 应为 user"
    assert sample["messages"][1]["role"] == "assistant", "second role 应为 assistant"

    kw_count = len(keywords_records) // max(KEYWORDS_OVERSAMPLE, 1) if keywords_records else 0
    print(
        f"✅ main data prepared: "
        f"empty={len(empty)}, "
        f"nonempty={len(nonempty)}×{NONEMPTY_OVERSAMPLE}={len(oversampled)}, "
        f"keywords={kw_count}×{KEYWORDS_OVERSAMPLE}={len(keywords_records)}, "
        f"total={len(all_records)}, valid_tail={N_VALID_TAIL}, outdir={OUTDIR}"
    )
    return 0


if __name__ == "__main__":
    # self-check（v1 引入 oversample 后保证 _is_empty_patch 的判定语义稳定）
    assert _is_empty_patch("{}") is True, "literal {} should be empty"
    assert _is_empty_patch('{"x":1}') is False, "non-empty json should be nonempty"
    assert _is_empty_patch("  {}  ") is True, "{} with whitespace should still be empty"
    sys.exit(main())
