# BETA-24 重训（keywords 补全 + MediaSearch 覆盖）

> 日期：2026-06-13（macOS 会话）
> 承接：BETA-23 接入桌面默认流程后，现役 LoRA（BETA-17 winner）对 keywords 待填输出空 patch `{}`（训练数据派生自 v0.5，keywords 从不是待填字段）。本轮重训补 keywords 补全样本，问题 4 最后一公里。

## winner 产物（凭以下信息可重建；GGUF/adapter 本身 gitignore）

- **基座**：`mlx-community/Qwen3-0.6B-4bit`（同 BETA-17 winner，架构 `Qwen3ForCausalLM`，纯文本，Apache 2.0）
- **adapter**：`training/mlx-lora/adapters/beta24-qwen3-0.6b/`
- **Q4_K_M GGUF**：`training/mlx-lora/fused/beta24-qwen3-0.6b-q4_k_m.gguf`（378 MB）
  - **sha256**：`3aef6efba88316786d3128a0a19599573eaeafffad492633ab543a1089650e0a`
- **训练超参**（完全对齐 BETA-17/v1，单一变量铁律）：1000 step / num-layers 16 / batch 4 / lr 1e-4 / mask-prompt / nonempty oversample 8× / seed 42；val loss 0.001 / train loss ~0.003
- **复现命令**：`bash training/mlx-lora/scripts/run_beta24.sh`

## 唯一变量：训练数据

- 基座、超参、评测口径全部对齐 BETA-17。**唯一变化 = 训练数据并入 `lora-aug-keywords/v1`**。
- 数据混合（`prepare_main_data.py --keywords-aug`，`KEYWORDS_OVERSAMPLE=3`）：
  - empty 443（v0.5-patch 空 patch）
  - v0.5-nonempty 55×8 = 440
  - **keywords-aug 122×3 = 366**（新增，占总量 29.3%）
  - total = 1249
- keywords-aug 数据集来源见 [packages/evals/fixtures/lora-aug-keywords/v1/README.md](../../../packages/evals/fixtures/lora-aug-keywords/v1/README.md)（模板生成 + 6 手写分片，152 cases，触发分布=推理分布）。

## 三层验收结果

| 层 | 指标 | 结果 | 门 |
|---|---|---|---|
| 1 held-out | with-fallback keywords 补全 pass 率 | **27/30 = 90.0%**（旧模型 0%） | ≥80% ✓ |
| 2 v0.9 | parser-only byte-equal | 726 不动 | 硬门 ✓ |
| 2 v0.9 | with-fallback regressions | **0**（修复后） | 0 ✓ |
| 3 v0.5 | parser-only byte-equal / with-fallback regressions | 473 / 0 | ✓ |
| 性能 | 触发 case 延迟（Metal） | p50 46ms / p95 121ms / max 1170ms | p95≤3s ✓ |

## 回归修复（重要）

首轮 with-fallback 在 v0.9 产生 **86 个回归**——重训后模型对 keywords 过度积极，在 keywords 不该填的查询上幻觉关键词。诊断暴露 BETA-23 `apply_patch` 的契约漏洞（不校验模型输出是否在 fillable 范围内；BETA-17 模型总输出 `{}` 从未暴露）。三处**代码层**修复（非重训）把回归降到 0，且不损 held-out 90%：

1. `apply_patch` keywords 契约强制：keywords∉fillable 时丢弃模型 keywords。
2. `apply_patch` `PARSER_OWNED_FIELDS` denylist：模型无 fillable 类别的字段（extensions/file_type/options 等）一律丢弃。
3. 媒体臂 keywords 检测限定 `media_type=Audio` + `has_uncovered_content` 补剥类型词/曲风/框架词。

详见 [spec 验证后记](../../../docs/superpowers/specs/2026-06-13-beta-24-lora-retrain-keywords-design.md) Task 8-9 节。

## 部署

GGUF 改名放到 app 数据目录 `models/qwen3-0.6b-q4_k_m.gguf`（desktop 期望文件名不变，**桌面侧零代码改动**——换文件即生效）。本会话已部署本机供真机手测。
