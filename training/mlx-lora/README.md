# training/mlx-lora

在 Mac 上用 MLX / mlx-lm 做 LoRA 微调。

**状态**：**BETA-24 重训完成**（2026-06-13，**ready**：基座/超参对齐 BETA-17 winner，唯一变量=并入 keywords-aug 训练数据；held-out keywords 补全 90%（旧 0%）/ v0.9·v0.5 with-fallback regressions=0 / byte-equal 不动；详 [releases/beta24.md](./releases/beta24.md)）。当前推荐推理 GGUF `fused/beta24-qwen3-0.6b-q4_k_m.gguf`（378MB，部署即换名 `models/qwen3-0.6b-q4_k_m.gguf`）。

此前里程碑：BETA-17 基座 bake-off 选定 Qwen3-0.6B（[reviews/beta-17-base-model-bakeoff.md](../../docs/reviews/beta-17-base-model-bakeoff.md)）；BETA-08 主体 run v1（2026-05-28，`--mask-prompt` + nonempty oversample 8× 攻克 v0 退化解，pass 460→468 / regressed 0；[reviews/beta-08-lora-v1.md](../../docs/reviews/beta-08-lora-v1.md)）。

## 计划职责

- 基座模型：Qwen2.5-1.5B-Instruct（首版），Qwen3-1.7B 备选
- 训练流水线：数据准备 → LoRA 训练 → 合并/挂载 adapter → 评测 → 导出 GGUF Q4_K_M
- 多轮迭代（2–4 轮），按评测错误补样本
- 训练产物（adapter / 合并模型 / 量化模型）跨平台通用，可在 Windows 上推理

## 平台约束

- 仅在 macOS（Apple Silicon）上训练
- 推理在 [packages/model-runtime](../../packages/model-runtime/) 跨平台进行

## 大文件管理

`adapters/` 和 `checkpoints/` 已在 `.gitignore` 中排除。最终量化产物分发方式另行决定（HuggingFace / 自有 CDN / 安装包内嵌）。

## smoke run（BETA-08 spike）

验证 mlx-lm LoRA → fuse → GGUF → llama-cpp-4 加载端到端可行性。

- 设计 spec：[`docs/superpowers/specs/2026-05-27-beta-08-smoke-design.md`](../../docs/superpowers/specs/2026-05-27-beta-08-smoke-design.md)
- 实施 plan：[`docs/superpowers/plans/2026-05-27-beta-08-smoke.md`](../../docs/superpowers/plans/2026-05-27-beta-08-smoke.md)
- 实测 ledger：[`docs/reviews/beta-08-smoke.md`](../../docs/reviews/beta-08-smoke.md)

跑法（从仓库根）：

```bash
bash training/mlx-lora/scripts/run_smoke.sh
```

**spike 结论（2026-05-27）**：训练管道 ✓；mlx-lm `fuse --export-gguf` ✗ qwen2 不支持；fallback 走 `fuse --dequantize` → HF safetensors → llama.cpp convert_hf_to_gguf.py → GGUF 路径 ✓（路径已 spike 至 safetensors 阶段，剩 llama.cpp 工具链需用户在 BETA-08 主体会话前安装）。

产物：`adapters/smoke-v0/`（10 MB，仅作 spike 凭证）。`data/` + `fused/` 均 git-ignored，可由 `prepare_smoke_data.py` + seed=42 完全确定性重生。

## main run（BETA-08 v0）

正式 LoRA 训练 + Q4_K_M GGUF 量化 + v0.5 evals 验门槛。

- 设计 spec：[`docs/superpowers/specs/2026-05-27-beta-08-main-design.md`](../../docs/superpowers/specs/2026-05-27-beta-08-main-design.md)
- 实施 plan：[`docs/superpowers/plans/2026-05-27-beta-08-main.md`](../../docs/superpowers/plans/2026-05-27-beta-08-main.md)
- 出场报告：[`docs/reviews/beta-08-lora-v0.md`](../../docs/reviews/beta-08-lora-v0.md)（**not_ready**，模型学退化解）

跑法（从仓库根，需先安装 llama.cpp 工具链与 torch）：

```bash
bash training/mlx-lora/scripts/run_main.sh
```

**v0 结果**：训练完美收敛（val loss 0.010），但 evals pass/partial/fail 与 parser-only baseline 完全相同（460/38/2，净增 0）。86 个 fallback 触发全为 no-op。根因：训练数据 88% empty patch，模型学到"永远输出 `{}`" 的退化解。

**v1 计划**：在 `--mask-prompt` flag + nonempty patch oversample 上重训。详报告 §7。

产物（全 git-ignored）：
- `adapters/main-v0/`（adapter ~20 MB + 10 个 checkpoint）
- `fused/main-v0-q4_k_m.gguf`（评测用 940 MB）
- `fused/main-v0-baseline.json` + `main-v0-evals.log`（实测数据）

## main run（BETA-08 v1，**ready**）

承接 v0 not_ready，按 v0 报告 §7(a) 推荐：`--mask-prompt` + nonempty oversample 8× 重训。

- 设计 spec：[`docs/superpowers/specs/2026-05-27-beta-08-v1-design.md`](../../docs/superpowers/specs/2026-05-27-beta-08-v1-design.md)
- 实施 plan：[`docs/superpowers/plans/2026-05-27-beta-08-v1.md`](../../docs/superpowers/plans/2026-05-27-beta-08-v1.md)
- 出场报告：[`docs/reviews/beta-08-lora-v1.md`](../../docs/reviews/beta-08-lora-v1.md)（**ready**）

跑法（从仓库根，需 llama.cpp 工具链与 torch）：

```bash
bash training/mlx-lora/scripts/run_main_v1.sh
```

**v1 与 v0 差异**（隔离单一变量原则）：
- 训练 loss：`--mask-prompt` 只在 completion token 算 loss
- 数据平衡：`prepare_main_data.py` 内 `NONEMPTY_OVERSAMPLE=8` 让 55 nonempty patch case 重复 8× 达 ~50/50
- 数据格式：chat format `{messages: [...]}` 绕开 mlx-lm 0.29.1 `CompletionsDataset` 在 mask-prompt 分支的 jinja bug（详 v1 报告 §9）
- 超参完全维持 v0（1000 step / lr 1e-4 / batch 4 / num-layers 16）

**v1 结果**：
- pass 460→**468 (+8)** / partial 38→30 / fail 2→2 / regressed 0 / rescued_to_pass 8
- 字段级精确匹配 92.0%→**93.6%**
- fallback valid_intent 比从 v0 的 8.3% 飙到 **100%**（86/86），证明退化解被攻克
- 训练耗时 ~42 min，总管线 ~47 min（M5 Pro）

产物（全 git-ignored）：
- `adapters/main-v1/`（adapter ~20 MB + 10 个 checkpoint）
- `fused/main-v1-q4_k_m.gguf`（**当前推荐推理 GGUF，940 MB**）
- `fused/main-v1-baseline.json` + `main-v1-evals.log`
