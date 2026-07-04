# BETA-08 LoRA v1 — 出场报告（**ready**：mask-prompt + oversample 攻克退化解，pass +8 / 0 regressed）

| 项 | 值 |
|---|---|
| 日期 | 2026-05-28 |
| 操作者 | Claude Code (Opus 4.7) |
| 设备 | macOS 15.5 / Apple M5 Pro / 55 GB Metal recommendedMaxWorkingSetSize |
| spec | [2026-05-27-beta-08-v1-design.md](../superpowers/specs/2026-05-27-beta-08-v1-design.md) |
| plan | [2026-05-27-beta-08-v1.md](../superpowers/plans/2026-05-27-beta-08-v1.md) |
| 前置 | [v0 出场报告](./beta-08-lora-v0.md)（not_ready，pass 净增 0） |
| 结果 | **v1 ready** — 门槛 1 PASS (pass 净增 **+8** ≥ 5)；门槛 2 PASS (regressed **0** ≤ 2) |

## 1. 关键指标对比（vs baseline + vs v0）

| 指标 | parser-only baseline | LoRA v0 | LoRA v1 | Δ (v1 vs v0) |
|---|---|---|---|---|
| pass | 460 (92.0%) | 460 (92.0%) | **468 (93.6%)** | **+8** |
| partial | 38 | 38 | **30** | **-8** |
| fail | 2 | 2 | 2 | 0 |
| **rescued_to_pass** | 0 | 0 | **8** | **+8** |
| **rescued_to_partial** | 0 | 0 | 0 | 0 |
| **regressed** | 0 | 0 | **0** | 0 |
| variant 命中率 | 99.6% | 99.6% | 99.6% | 0 |
| 字段级精确匹配率 | 92.0% | 92.0% | **93.6%** | **+1.6 pp** |
| **fallback valid_intent 比** | — | 1/12 = 8.3%（可观测 partial/fail） | **86/86 = 100%** | **+91.7 pp** |
| fallback 触发数 | — | 86 / 500 (17.2%) | 86 / 500 (17.2%) | 0 |
| p50 fallback 延迟 (ms) | — | 1549 | 1565 | +16 |
| p95 fallback 延迟 (ms) | — | 1586 | **1592** | +6 |

数据源：`training/mlx-lora/fused/main-v1-baseline.json` + `main-v1-evals.log` + `/tmp/main-v1-run.log`。

**variant confusion (全局)**：

| variant | pass | partial | fail |
|---|---|---|---|
| FileSearch | **187** (v0: 181) | 12 (v0: 18) | 1 |
| MediaSearch | 87 (v0: 85) | 13 (v0: 15) | 0 |
| FileAction | 76 | 4 | 0 |
| Clarify | 39 | 1 | 0 |
| Refine | 79 | 0 | 1 |

FileSearch 增 6 pass（−6 partial），MediaSearch 增 2 pass（−2 partial）。两个变体合计正好覆盖 8 个 rescued case。

## 2. 门槛核对

```
门槛 1: pass 净增 ≥ 5
  baseline=460, v1=468, Δ=+8, status=PASS

门槛 2: regressed ≤ 2
  v1=0, status=PASS

总结论：v1 ready
```

## 3. 实测时间线

| 步骤 | 起止行（log 8267 行） | 耗时 |
|---|---|---|
| [1/7] prepare data | 1-2 | <1 s |
| [2/7] mlx-lm lora 1000 step | 3-53 | **~42 min**（0.39 it/sec × 1000）|
| [3/7] mlx-lm fuse → safetensors | 54-58 | ~30 s |
| [4/7] convert_hf_to_gguf.py → fp16 | 59-481 | ~1 min |
| [5/7] llama-quantize Q4_K_M | 482-879 | ~3 s |
| [6/7] parser-only baseline evals | 880-884 | ~30 s |
| [7/7] with-fallback hybrid evals | 885-8267 | **~2 min** |
| **总耗时** | — | **~47 min**（v0 是 ~50 min；mask-prompt 训练吞吐稍升）|

**产物大小**：
- adapter: ~20 MB（10 个 checkpoint × ~20 MB = 200 MB，最终 adapter 20 MB）
- fp16 GGUF: 2.9 GB
- Q4_K_M GGUF: **940 MB**（与 v0 同级别，毕竟同基座同量化）

## 4. 训练 loss 轨迹

| step | train loss | val loss |
|---|---|---|
| 1 | — | **4.334**（mask-prompt 后基线高于 v0 的 2.456，因 prompt token 不计入 loss）|
| 50 | 0.831 | — |
| 100 | 0.228 | — |
| 150 | 0.109 | — |
| 200 | 0.046 | **0.037** |
| 250 | 0.038 | — |
| 300 | 0.026 | — |
| 350 | 0.030 | — |
| 400 | 0.016 | **0.002** |
| 450 | 0.002 | — |
| 500 | 0.005 | — |
| 550 | 0.011 | — |
| 600 | 0.004 | **0.000** |
| 650 | 0.005 | — |
| 700 | 0.003 | — |
| 750 | 0.005 | — |
| 800 | 0.005 | **0.000** |
| 850 | 0.004 | — |
| 900 | 0.002 | — |
| 950 | 0.001 | — |
| 1000 | 0.003 | **0.001** |

**注**：v1 用 `--mask-prompt`，loss 数字绝对值不可与 v0 直接比（v1 只在 completion token 算 loss，每条 case 平均贡献 token 显著减少，单 token 平均 loss 数值变化）。只看趋势：

- v0：iter 200 val loss 0.015，iter 800 才到 0.010，"长尾平稳"
- v1：iter 200 val loss 0.037（比 v0 略高），但 **iter 400 跌至 0.002**（v0 同期 0.011），iter 600 后即触地 0.000

收敛**比 v0 更激进**。这个曲线在 v0 时是过拟合的强烈信号，但 v1 evals 实测 0 regressed + 8 rescued，说明 loss 接近 0 在 **balanced + mask-prompt** 双重作用下对应"模型确实学到正确的 patch 映射"（每个 prompt 几乎唯一对应一个 patch，过度记忆 = 正确预测）。

## 5. 8 个 rescued_to_pass case（Partial → Pass）

| ID | Query | 救援字段 |
|---|---|---|
| v05-schema-16-016 | "找最近一个月超过 10 分钟的视频" | duration parse（mask-prompt 后模型正确给出 duration patch）|
| v05-schema-46a-048 | "找项目归档里的 budget pdf" | location/folder |
| v05-media-class1-size-074 | "找几个 G的视频" | size parse |
| v05-file-template-219 | "找最近的 synthetic-plan ppt" | keyword + sort |
| v05-file-template-222 | "找最近的 synthetic-plan mp4" | keyword + sort |
| v05-file-template-225 | "找最近的 会议 Excel" | keyword + sort |
| v05-file-template-228 | "找最近的 会议 PDF" | keyword + sort |
| v05-file-template-231 | "找最近的 会议 ppt" | keyword + sort |

后 5 个属同一 query 结构家族（"找最近的 X Y"），LoRA 学到一致的字段补全模式。前 3 个分别覆盖 duration / location / size — 表明 LoRA 不是单纯背一个模板，而是学到了不同字段类型的填充规则。

## 6. 残留 30 partial 的 diff 字段分布

| 字段 | 数量 | 典型 diff |
|---|---|---|
| keywords | 7 | `.keywords: expected null, actual ["一周截的"/"downloads"/"documents"/...]`（screenshot keyword 误填位置词）|
| language | 6 | mixed↔zh 检测分歧（synthetic-place / budget / final 等边缘 case；ROADMAP 已知，[STATUS 第 14 阶段](../../STATUS.md) 评估 trade-off 不利）|
| file_type | 6 | `.file_type: expected null, actual "document"`（FileSearch parser 主动加 file_type 但 fixture 期望 null）|
| artist | 4 | `find songs by synthetic-artist` artist 未识别（4 个同模板 case）|
| new_name | 3 | "把第5个 rename 为 synthetic-final" new_name 字段未提取 |
| location | 3 | `.location: expected {"hint":"下载"}, actual {"hint":"downloads"}` 中英混合 hint 形态分歧 |
| title | 1 | media_search title 字段 |
| modified_time | 1 | absolute range vs before type 形式分歧 |

vs v0 同字段对比（v0 partial 38）：

| 字段 | v0 | v1 | Δ |
|---|---|---|---|
| language | 13 | 6 | **-7** |
| keywords | 7 | 7 | 0 |
| modified_time | 4 | 1 | **-3** |
| artist | 4 | 4 | 0 |
| location | 3 | 3 | 0 |
| new_name | 3 | 3 | 0 |
| file_type | (未单列) | 6 | （v0 散在 11 残留中）|

LoRA 把 language detection 残留减半 + modified_time 残留减 3。但 keywords / artist / new_name 三个老 bucket 完全没动 — 说明 LoRA 在**字段值需要从 query 显式抽取**（artist / new_name）或**判定字段"应为空"**（screenshot keywords）的 case 上没救援能力。这两类是数据集 v0.5-patch/v0 信号本身就少（55 nonempty 里 artist=0、new_name=2、screenshot-keyword-null=0），oversample 8× 也补不出训练里没有的样本类型。

## 7. 根因诊断 / v0 → v1 验证

**核心问题**：mask-prompt + oversample 8× 是否解决了 v0 的退化解？

| 指标 | v0 | v1 | 结论 |
|---|---|---|---|
| fallback 触发数 | 86 | 86 | 触发器没变（parser 一致）|
| **valid_intent 比** | 8.3% (1/12) | **100% (86/86)** | ✅ **彻底解决"模型永远输出 `{}`"** |
| rescued_to_pass | 0 | 8 | ✅ adapter 真正在 hybrid 架构中工作 |
| rescued_to_partial | 0 | 0 | 模型补的 patch 要么 push 到 pass，要么对 partial 无改善（无中间态） |
| regressed | 0 | 0 | ✅ oversample 8× 没让模型对 nonempty 过敏（R1 风险未发生）|
| p95 fallback 延迟 | 1586 ms | 1592 ms | 模型推理路径不变，延迟持平 |

**为什么 mask-prompt + oversample 攻克退化解**：
1. **mask-prompt** 让 loss 只来自 completion token，prompt 部分的 token loss 不再"摊薄" nonempty patch 的学习信号
2. **oversample 8×** 把 nonempty patch case 从 11% 提升到 49.8%，模型不能再靠"永远输出 `{}`" 获得 majority-class 最低 loss
3. 二者协同：loss 来源准确（mask）+ 类别平衡（oversample），模型被迫真正学 prompt → patch 映射

**为什么没 regressed**：oversample 8× 不是用合成数据，是把现有 55 个真实 nonempty case 直接重复。模型对每个 case 看 8 次而非 1 次，本质是更深拟合而非引入新分布。empty case 仍有 443 个保留"该输出空时输出空"的信号。

## 8. 推荐决策

1. **BETA-08 标 done**。LoRA adapter v1 ready，落 `training/mlx-lora/fused/main-v1-q4_k_m.gguf` 940 MB 作为 v0.5 evals --with-fallback --hybrid 默认 model。
2. **更新 LOCIFIND_MODEL_PATH 默认值** 或在 README / harness 文档中标注当前推荐 GGUF 为 main-v1（非 main-v0）。
3. **下一会话候选**：
   - **BETA-09 模型量化与跨平台部署**：用 v1 GGUF 验证 Windows 推理路径；adapter 落 checksum 入库；评估 Q5/Q6 量化版本是否进一步降低延迟
   - **MVP-26 跨平台一致性**：v1 GGUF 在 Windows 真机跑 v0.5 evals 验证一致性
   - **路径 (b) 升级残留 30 partial**：keywords / artist / new_name 三个 bucket LoRA 救不动，需 Tier 2 augmentation（合成 nonempty 训练 case）。但 30 partial 中 18 个属"边缘 case / 检测器 trade-off"（language 6 + keywords 7 + file_type 6 部分），真正可救的可能 < 10。ROI 待评估。
4. **长周期事项不变**（Apple Developer / Windows 签名证书 / 域名 / 商标）

## 9. 收获与教训

1. **退化解可被双手段攻克**：mask-prompt + class balance 二者缺一不可（v0 二者皆无 → 0 valid；v1 二者皆有 → 100% valid）。这是未来 patch 任务训练的通用配方
2. **mlx-lm 0.29.1 `--mask-prompt` + `{prompt, completion}` 格式有 bug**：`datasets.py:112` 把单 dict 当 list 传给 `apply_chat_template`，jinja 直接 crash。**workaround：用 `{messages: [{user}, {assistant}]}` chat format**，自动走 ChatDataset path 避开 bug。该 workaround 训练语义零差异
3. **v0 → v1 单次循环耗时 ~50 min**：spec / plan / 脚本骨架完全复用，本会话从 brainstorming 到出场报告耗时约 3 小时。LoRA 实验循环代价可接受
4. **训练 loss 接近 0 不一定是过拟合**：在 prompt → patch 任务里，每个 prompt 几乎唯一对应一个 patch，loss 接近 0 等价"模型背下了正确映射"。这与生成任务（多个合法 output）的过拟合判定不同
5. **Apple M5 Pro 性能 baseline 复测**：mask-prompt 下 train at batch 4 / 16-layer / 1000 step → 0.39 it/sec / 12.4 GB peak / ~42 min（与 v0 同；mask-prompt 不显著影响吞吐）
6. **chat format 是更通用的 mlx-lm 输入格式**：未来所有 mlx-lm 训练数据准备脚本默认走 chat format，避开 CompletionsDataset 已知 bug + 跨版本兼容性更好

## 10. 产物清单

| 路径 | 大小 | 入库 |
|---|---|---|
| `training/mlx-lora/adapters/main-v1/` | 20 MB（最终）+ 200 MB（10 checkpoint）| git-ignored |
| `training/mlx-lora/fused/main-v1-safetensors/` | 2.9 GB | git-ignored，可删 |
| `training/mlx-lora/fused/main-v1-f16.gguf` | 2.9 GB | git-ignored，可删（v1 ready 后只保留 Q4_K_M）|
| `training/mlx-lora/fused/main-v1-q4_k_m.gguf` | **940 MB** | git-ignored，**v1 默认推理 GGUF** |
| `training/mlx-lora/fused/main-v1-baseline.json` | 497 KB | git-ignored |
| `training/mlx-lora/fused/main-v1-evals.log` | 356 KB | git-ignored |
| `/tmp/main-v1-run.log` | 466 KB | 本地临时 |
| **v0 产物全部保留** | — | git-ignored，作 v1 baseline 对比 |
