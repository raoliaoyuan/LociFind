# v0.5-patch / v0

BETA-08 LoRA 微调用 patch 任务训练集，第一版（v0）。

## 来源

- 源 fixture：`packages/evals/fixtures/v0.5/cases.json`（500 case）
- 生成器：[`packages/evals/src/bin/build_lora_dataset.rs`](../../../../packages/evals/src/bin/build_lora_dataset.rs)
- 生成方法：`parser-diff` — 跑 parser 拿 IntentDraft，与 fixture expected 做 top-level 字段 diff
- 设计 spec：[`docs/superpowers/specs/2026-05-27-beta-08-lora-design.md`](../../../../docs/superpowers/specs/2026-05-27-beta-08-lora-design.md)

## 文件

| 文件 | 内容 |
|---|---|
| `cases.jsonl` | 每行一个训练样本，含 `prompt` / `completion` / `case_id` / `fillable_fields` / `draft_variant` 五字段 |
| `meta.json` | 数据集元信息：source sha256 / generator git rev / 统计 |

## 统计（v0 实测）

- 总 case：500
- 跳过（variant 错位，hybrid 锁定无法救）：2
- 有效训练样本：**498**
  - empty patch `{}`：443（教模型"无事可做"）
  - non-empty patch：55（主要学习信号）

详 `meta.json` `stats` 字段。

## 训练用法（留 BETA-08 主体会话实施）

mlx-lm 默认接受 JSONL 含 `prompt` / `completion` 字段；其他字段是辅助元数据用于训练时按 variant / bucket 分桶分析或 weighted sampling。

## 不可变性

v0 已冻结。改生成器、改源 fixture、改 parser 都应升 v1（路径 `training/datasets/v0.5-patch/v1/`），不要覆盖 v0。

## 重生成

```bash
cargo run --release -p locifind-evals --bin build_lora_dataset -- \
    --input packages/evals/fixtures/v0.5/cases.json \
    --output training/datasets/v0.5-patch/v0/
```

输出对当前 git rev + fixture sha256 是确定性的；同 input 永远产同 output（cases.jsonl byte-equal）。
