# BETA-08 LoRA smoke run ledger（Path B：精确卡点 + Plan B 已 spike 至 step 2）

| 项 | 值 |
|---|---|
| 日期 | 2026-05-27 |
| 操作者 | Claude Code (Opus 4.7) |
| spec | [2026-05-27-beta-08-smoke-design.md](../superpowers/specs/2026-05-27-beta-08-smoke-design.md) |
| plan | [2026-05-27-beta-08-smoke.md](../superpowers/plans/2026-05-27-beta-08-smoke.md) |
| 设备 | macOS 15.5 (Darwin 25.5.0) / Apple Silicon / mlx 0.29.3 / mlx-lm 0.29.1 |
| 结果 | **Path B：[3/4] mlx-lm `--export-gguf` 不支持 qwen2 架构**。但训练 + 不带 `--export-gguf` 的 fuse 都 ✓。Plan B 路径已 spike 至 step 2，只剩 llama.cpp 工具链。 |

## 实测时间线

| 步骤 | 状态 | 耗时 | 备注 |
|---|---|---|---|
| [1/4] prepare data | ✓ | <1 s | train=100, valid=20，确定性 seed=42 |
| [2/4] lora train 50 step | ✓ | **~50 s** | 见下方训练详情 |
| [3/4] fuse `--dequantize --export-gguf` | ✗ | <5 s 即报错 | `ValueError: Model type qwen2 not supported for GGUF conversion` |
| **Plan B step 2: fuse `--dequantize`（不带 export-gguf）** | ✓ | ~10 s | 输出 HF safetensors 目录（2.9 GB），可作为后续 llama.cpp convert 的输入 |
| [4/4] fallback_probe 验证加载 | ⊘ 跳过 | — | 因 [3/4] 失败、未产出 GGUF；未执行 |
| **总耗时** | — | **75 s**（含 Plan B step 2） | 从仓库根 `bash run_smoke.sh` 开始 |

## 训练详情（[2/4] 完整成功）

mlx-lm lora 在本地 `~/models/qwen25_1.5b_draft/`（Qwen2.5-1.5B-Instruct 4bit MLX 格式，828 MB）上跑 LoRA：

| 项 | 值 |
|---|---|
| Trainable parameters | **2.638 M / 1543.714 M（0.171%）** |
| Peak memory | **5.104 GB**（Mac unified memory，安全裕量大） |
| 训练吞吐 | ~1360 tokens/sec / ~1.0 it/sec |
| 已训练 tokens | 68625 |

**Loss 轨迹**：

| step | train loss | val loss | 备注 |
|---|---|---|---|
| 1 | — | 2.463 | val 起点 — 模型对 LociFind patch 一无所知 |
| 10 | 0.785 | — | |
| 20 | 0.095 | — | |
| 25 | 0.062 | **0.066** | val 跌 ~37× |
| 30 | 0.062 | — | |
| 40 | 0.061 | — | |
| 50 | **0.041** | **0.037** | val 跌 66× — 强信号说明 patch 任务**可学** |

50 step / 100 sample 下 val loss 跌到 0.037 是**强力的过拟合信号**（数据太少模型太大），但 spike 不在乎 — 这只证明 mlx-lm 训练管道与 LoRA 任务设置都正常。

## [3/4] 失败：精确错误

```
Traceback (most recent call last):
  File "/Library/Developer/CommandLineTools/.../runpy.py", line 197, in _run_module_as_main
    return _run_code(code, main_globals, None,
  ...
  File "/Users/alice/Library/Python/3.9/lib/python/site-packages/mlx_lm/fuse.py", line 96, in main
    raise ValueError(
ValueError: Model type qwen2 not supported for GGUF conversion.
Loading pretrained model
Dequantizing model
```

**根因（spec §5 S2 命中）**：`mlx_lm.fuse` 的 `--export-gguf` 路径只支持 llama 类架构（详 `mlx_lm/fuse.py:96`），不支持 qwen2。这是 mlx-lm 自身限制，与我们的代码无关。

## Plan B step 2 已 spike 通过

为给主体 run 提供完整路线图，本会话额外跑了一次：

```bash
python3 -m mlx_lm fuse \
    --model ~/models/qwen25_1.5b_draft \
    --adapter-path training/mlx-lora/adapters/smoke-v0 \
    --save-path training/mlx-lora/fused/smoke-v0-safetensors \
    --dequantize
# 不带 --export-gguf
```

**结果 ✓**：输出 HF 格式目录，含 `model.safetensors`（2.9 GB）+ `config.json` + `tokenizer.json` + 其他 tokenizer 文件。可被 llama.cpp `convert_hf_to_gguf.py` 直接消费。

**spike 后已删除**该 2.9 GB safetensors 目录（保留 10 MB adapter）。

## R5 风险结论

**R5（mlx-lm 一站路径是否可行）：未排除 — 但 Plan B 已 de-risk 至 step 2**。

- **不可走**：mlx-lm `--export-gguf`（明确不支持 qwen2）
- **可走的完整路径**：mlx-lm `fuse --dequantize` → HF safetensors → llama.cpp `convert_hf_to_gguf.py` → fp16 GGUF → llama.cpp `llama-quantize` → Q4_K_M GGUF → llama-cpp-4 加载

剩余未验证的只有 llama.cpp 工具链那一段（convert_hf_to_gguf.py + llama-quantize）。这是主流路径，业界文档充分，BETA-08 主体可放心走。

## 对 BETA-08 主体的建议（更新 spec §8）

设计 spec §8 部署假设需更新为以下路径：

```
mlx-lm fuse --dequantize （MLX adapter + base → HF safetensors，~3GB）
   ↓
llama.cpp/convert_hf_to_gguf.py （HF safetensors → fp16 GGUF，~3GB）
   ↓
llama.cpp/llama-quantize （fp16 GGUF → Q4_K_M GGUF，~1GB）
   ↓
llama-cpp-4 0.3.0 加载 + evals --with-fallback --hybrid
```

**用户准备工作（BETA-08 主体会话前）**：

```bash
# 安装 llama.cpp 工具链（一次性，~30 min）
git clone https://github.com/ggml-org/llama.cpp.git ~/tools/llama.cpp
cd ~/tools/llama.cpp
make -j  # 编译出 llama-quantize 等工具
# 试一次：python convert_hf_to_gguf.py --help
```

**或者**：使用 `llama-cpp-python` 包的 bundled converter（avoid C++ 编译），但量化阶段仍需 C++ 工具。

## 训练超参与正式 run 的关系

smoke 用：
- `--num-layers 8 / --iters 50 / --batch-size 2 / --learning-rate 1e-4`

正式 run 建议起点：
- `--num-layers 16`（mlx-lm 默认；smoke 砍半为加速）
- `--iters 500-1000`（让数据通过若干 epoch）
- `--batch-size 4`（peak 5.1 GB 还有大量裕量；可试 8）
- `--learning-rate 1e-4` 不变
- 全部 v0.5-patch/v0 498 sample，**80/20 切 train/valid 替换 smoke 的 100/20**

## 未尽事宜

- 主体会话前需准备 llama.cpp 工具链（用户工作）
- 主体会话 spec §8 需按本 ledger "对主体的建议"段更新
- ROADMAP BETA-09 备注无需变化（Q4_K_M 量化仍属 BETA-09 主流职责）

## 文件产物

- 保留：`training/mlx-lora/adapters/smoke-v0/`（10 MB，仅作 spike 凭证；正式 run 时丢弃，重头训练）
- 已删：`training/mlx-lora/fused/smoke-v0-safetensors/`（2.9 GB Plan B step 2 凭证）
- log：`/tmp/smoke-run.log`（本地，不入库）
