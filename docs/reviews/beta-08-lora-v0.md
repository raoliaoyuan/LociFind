# BETA-08 LoRA v0 — 出场报告（**not_ready**：训练完美但学到退化解）

| 项 | 值 |
|---|---|
| 日期 | 2026-05-27 |
| 操作者 | Claude Code (Opus 4.7) |
| 设备 | macOS 15.5 / Apple M5 Pro / 55 GB Metal recommendedMaxWorkingSetSize |
| spec | [2026-05-27-beta-08-main-design.md](../superpowers/specs/2026-05-27-beta-08-main-design.md) |
| plan | [2026-05-27-beta-08-main.md](../superpowers/plans/2026-05-27-beta-08-main.md) |
| 前置 | [启动 spec](../superpowers/specs/2026-05-27-beta-08-lora-design.md) + [smoke ledger](./beta-08-smoke.md) |
| 结果 | **v0 not_ready** — 门槛 1 失败 (pass 净增 0 < 5)；门槛 2 通过 (regressed 0 ≤ 2) |

## 1. 关键指标对比

| 指标 | parser-only baseline | LoRA v0 | Δ |
|---|---|---|---|
| pass | 460 (92.0%) | **460 (92.0%)** | **+0** |
| partial | 38 (7.6%) | 38 (7.6%) | +0 |
| fail | 2 (0.4%) | 2 (0.4%) | +0 |
| **regressed** | 0 | **0** | — |
| **rescued_to_pass** | 0 | **0** | — |
| **rescued_to_partial** | 0 | **0** | — |
| variant 命中率 | 99.6% | 99.6% | +0 |
| valid_intent 比率（fallback 触发 86 case 中可观测的 12 个 partial/fail） | — | **1 / 12 = 8.3%** | — |
| p95 fallback 延迟 (ms) | — | 1586 | — |
| p50 fallback 延迟 (ms) | — | 1549 | — |
| fallback 触发数 | — | 86 / 500 (17.2%) | — |

数据源：`training/mlx-lora/fused/main-v0-baseline.json` + `main-v0-evals.log`。

**variant confusion (全局)**：

| variant | pass | partial | fail |
|---|---|---|---|
| FileSearch | 181 | 18 | 1 |
| MediaSearch | 85 | 15 | 0 |
| FileAction | 76 | 4 | 0 |
| Clarify | 39 | 1 | 0 |
| Refine | 79 | 0 | 1 |

完全等同 baseline。adapter 没有引入 variant 错位。

## 2. 门槛核对

```
门槛 1: pass 净增 ≥ 5
  baseline=460, v0=460, Δ=0, status=FAIL

门槛 2: regressed ≤ 2
  v0=0, status=PASS

总结论：v0 not_ready（门槛 1 未达，pass 完全没动）
```

## 3. 实测时间线

| 步骤 | 耗时 |
|---|---|
| [1/7] prepare data | <1 s |
| [2/7] mlx-lm lora 1000 step | **~42 min**（0.4 it/sec × 1000 step）|
| [3/7] mlx-lm fuse → safetensors | ~30 s |
| [4/7] convert_hf_to_gguf.py → fp16 | ~1 min |
| [5/7] llama-quantize Q4_K_M | 3.4 s |
| [6/7] parser-only baseline evals | ~30 s |
| [7/7] with-fallback hybrid evals | ~6 min |
| **总耗时** | **~50 min** |

**产物大小**：
- adapter: 20 MB（每 100 step checkpoint × 10 = 200 MB，但最终 adapter 仅 20 MB）
- fp16 GGUF: 2.9 GB
- Q4_K_M GGUF: **940 MB**（与 baseline 1.0 GB 同级别）

**注**：[6/7] 首次失败（`libggml-base.0.dylib` 未在 DYLD_LIBRARY_PATH），手动 retry 通过。script 已 patch。

## 4. 训练 loss 轨迹

| step | train loss | val loss |
|---|---|---|
| 1 | — | **2.456** |
| 50 | 0.177 | — |
| 100 | 0.026 | — |
| 200 | 0.015 | **0.015** |
| 300 | 0.012 | — |
| 400 | 0.012 | **0.011** |
| 500-700 | 0.011-0.012 | (no eval) |
| 800 | 0.010 | **0.010** |
| 1000 | 0.011 | **0.010** |

**收敛速度极快**：iter 200 时 val loss 已经 0.015，到 iter 800 进一步降到 0.010 后基本停滞。这是过度拟合训练数据的强烈信号。

## 5. 失败分桶（按 diff 字段，partial=38 中典型）

| 桶 | 数量 | 典型 diff | 示例 case |
|---|---|---|---|
| language: mixed→zh | 13 | `.language: expected "mixed", actual "zh"` | "找最近的 会议 Excel" / "找 synthetic-place 里的文件" |
| screenshot keywords 误填 | 7 | `.keywords: expected null, actual ["downloads"/"documents"]` | "find screenshots from last week in downloads" |
| modified_time 范围 vs 类型不符 | 4 | `.modified_time: expected before, actual absolute` | "找 2026 年 5 月 1 日之前修改的 zip" |
| artist 未识别 | 4 | `.artist: expected "synthetic-artist", actual null` | "find songs by synthetic-artist" |
| location 误填 | 3 | `.location: expected null, actual {"hint":"下载"}` | "找上个月下载的周华健无损音乐" |
| new_name 未识别 | 3 | `.new_name: expected "synthetic-final", actual null` | "把第5个 rename 为 synthetic-final" |

**全部是 baseline 的原有 partial**。adapter 没救回任何一个，也没引入新的失败。

## 6. 根因诊断

**核心发现**：86 次 fallback 触发中，可观测的 12 个 partial/fail case 里 **valid_intent=false 占 11 / 12 (91.7%)**。即模型大多数时候输出**无效 JSON**（不能反序列化为 patch）。

**Why**：训练数据 498 个样本中，**443 个 (88%) completion = `{}`**（empty patch）。模型最简单的 loss 最小化路径是**永远输出 `{}`**：
- 对 443 个空 patch case：完美命中 ✓
- 对 55 个 nonempty patch case：仍输出 `{}`，loss 略大但被 majority 摊薄

最终 val loss 0.010 主要是 majority class 贡献，**nonempty patch 信号完全淹没**。模型学到的不是"如何 patch 字段"，而是"如何输出 empty JSON"。

**为什么 [7/7] 还显示 86 fallback 触发**：fallback 触发器（`is_fallback_candidate`）在 parser 输出有"不确定信号"时调模型。但模型输出 `{}` 或退化文本都被 `apply_patch` 视为"无修改"，最终 intent 与 parser-only 一致 → pass/partial/fail 结果不变。

**adapter 实质是 "no-op"**：等价于不开 fallback。

## 7. 候选下一步（按推荐顺序）

### (a) `--mask-prompt` + nonempty over-sampling（推荐 v1 起点）

mlx-lm 有 `--mask-prompt` flag — 训练时仅在 completion token 上计算 loss，prompt token loss=0。结合**up-sample nonempty patch 数据**（如 55 个 nonempty 每个重复 8 次 → 440 nonempty + 443 empty → 50/50 平衡），让模型不再能靠"永远输出 `{}`" 偷懒。

预期：模型被迫真正学习 patch 字段填法。但要警惕 over-sampling 让模型对 nonempty 过敏（regressed 升高）。

实施成本：~30 min（改 prepare_main_data.py 加 oversample + run_main.sh 加 `--mask-prompt`），再跑一次完整 main run。

### (b) 改训练目标设计 — 让模型只看 nonempty case

只用 55 个 nonempty patch case 训练（剔除 443 empty）。模型只学"如何 fill 字段"，永不学"输出空"。推理时由 parser 决定是否调 fallback（已经如此），所以空 case 根本不进推理。

预期：信号最纯净。但 55 sample 训练量太小，过拟合风险大。需要 augmentation 凑 200-500 sample 才稳。

实施成本：~1 工作日（含 augmentation 设计）。

### (c) 降学习率 + 减 iters（不推荐）

iter 200 时 val loss 0.015，到 iter 1000 跌到 0.010，跌幅不大但**模型对 majority class 的偏好可能就是在这后 800 step 加深的**。早停 iter 200 可能保留一点 nonempty 信号。但本质问题（数据 class 失衡 + prompt loss 太大）没解决。

### (d) 升 Tier 2 数据（中期路线，不在 v1 范围）

合成更多 nonempty patch case，把 nonempty 比例提到 40-50%。改 BETA-08 启动 spec 的 Tier 2 augmentation 部分。但 v1 应先验证 (a) / (b) 在现有数据上的潜力。

## 8. 推荐决策

**v1 优先试 (a) `--mask-prompt` + oversample**：
- 单一 mlx-lm flag 加上数据准备一改
- 复用现有所有 spec / plan / 脚本
- 1-2 小时一次循环，可快速验证假设

若 (a) v1 仍 not_ready，再上 (b) 改目标设计。

**ROADMAP BETA-08 维持 in_progress**，标注 "v0 not_ready, v1 待"。

## 9. 收获与教训（不属于 ROADMAP，但写下来给项目）

1. **patch 任务的 class imbalance 问题被 spec 低估**。设计 spec §2.3 预测 38 nonempty patch，实际 55，仍只 11%。在没有 prompt masking 或 class weighting 时，模型几乎必然学退化解
2. **smoke spike 早就在暗示这个风险**：smoke 时 100/20 数据 val loss 跌到 0.037，比 main run 1000 step / 498 数据的 0.010 高 3.7×；smoke 信号更"健康"（更少 majority class 主导）反而预测能力更强。Spike 数据少不是缺陷，可能是优点
3. **mlx-lm 的 train/serving 一致**：fallback_probe 在 smoke 时 GGUF 加载成功的预测正确传导到 main run 7/7 跑通。Plan B 路径完全 de-risk
4. **Apple M5 Pro 性能数据**：1.5B 4bit base + LoRA train at batch 4 / num-layers 16 → 0.4 it/sec / 12.4 GB peak / ~42 min for 1000 step。未来同类训练可作 baseline

## 10. 产物清单

| 路径 | 大小 | 入库 |
|---|---|---|
| `training/mlx-lora/adapters/main-v0/` | 20 MB | git-ignored |
| `training/mlx-lora/fused/main-v0-safetensors/` | 2.9 GB | git-ignored，可删 |
| `training/mlx-lora/fused/main-v0-f16.gguf` | 2.9 GB | git-ignored，v1 时会重生成 |
| `training/mlx-lora/fused/main-v0-q4_k_m.gguf` | 940 MB | git-ignored，保留作 v1 baseline 对比 |
| `training/mlx-lora/fused/main-v0-baseline.json` | 497 KB | git-ignored |
| `training/mlx-lora/fused/main-v0-evals.log` | 357 KB | git-ignored |
| `/tmp/main-run.log` | 大约 5 MB | 本地临时 |
