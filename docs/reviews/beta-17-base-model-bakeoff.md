# BETA-17 基座模型选型实验（bake-off）报告

> 日期：2026-06-01
> 作者：Claude Code (Opus 4.8)
> 关联：[spec](../superpowers/specs/2026-06-01-beta-17-base-model-bakeoff-design.md) · [plan](../superpowers/plans/2026-06-01-beta-17-base-model-bakeoff.md) · [BETA-08 v1 release notes](../../training/mlx-lora/releases/v1.md) · [BETA-09a Windows parity](./beta-09a-windows-parity.md)

## 0. 范围说明

- **本会话 = Mac 半**：准确率 bake-off（Metal evals）+ Metal 相对延迟。弱硬件（Windows）延迟绝对达标判定**留后续 Windows 会话**复核（BETA-09a 的 Intel Iris Xe / Vulkan）。
- **尺寸约束（2026-06-01 用户指定，为效率）**：仅测 **< 1B** 的候选；**> 1B 的 Qwen3-1.7B / Qwen3.5-2B 本会话不测**，留 BETA-17 后续。
- **单一变量铁律**：只换基座，数据集（v0.5-patch，sha256 锁定）、超参（v1 配方）、评测口径全部对齐 BETA-08 v1。

## 1. 工具链冒烟门结果

任何训练前，先用 `training/mlx-lora/scripts/smoke_candidate.sh` 验证完整工具链能消费候选：拉基座 → 确认纯文本架构 → mlx-lm 最小 LoRA → convert GGUF + 量化 → **钉死的 `llama-cpp-sys-4 0.3.0` 跑最小推理产合法 JSON 无 think 块**。

| 候选 | mlx repo | 尺寸 | 架构 | 冒烟门 | 结论 |
|---|---|---|---|---|---|
| Qwen3-0.6B | `mlx-community/Qwen3-0.6B-4bit` | 0.6B | `Qwen3ForCausalLM`（纯文本） | ✅ EXIT=0 | **进 bake-off** |
| Qwen3.5-0.8B | `mlx-community/Qwen3.5-0.8B-MLX-4bit` | 0.8B | 含 `vision_config`（多模态 VLM） | ❌ EXIT=1（第 1 步） | **排除** |
| ~~Qwen3-1.7B~~ | `mlx-community/Qwen3-1.7B-4bit` | 1.7B | — | ⏭️ 未测 | 尺寸 > 1B，本会话跳过 |
| ~~Qwen3.5-2B~~ | `mlx-community/Qwen3.5-2B-MLX-4bit` | 2B | — | ⏭️ 未测 | 尺寸 > 1B，本会话跳过 |

**关键发现**：

1. **Qwen3 架构被钉死工具链（`llama-cpp-sys-4 0.3.0` + mlx-lm 0.29.1）完整支持** —— Qwen3-0.6B 走完「mlx LoRA → GGUF → 钉死栈推理 → 合法 JSON 无 think 块」全链路。R1 工具链风险对 Qwen3 一代解除，无需升级 `llama-cpp-sys-4`。
2. **Qwen3.5 小尺寸（0.8B）是多模态 VLM**（config 含 `vision_config`），非纯文本基座 —— 与本任务（快速产结构化 JSON 补丁的小文本模型）不匹配，被冒烟门第 1 步正确拦截。印证了选型期 web 调研的「Qwen3.5 带 mmproj 视觉文件」线索。<1B 段未发现纯文本 Qwen3.5 候选。
3. **non-thinking 天然成立**：推理路径不套 chat 模板（`hybrid.rs::build_hybrid_prompt` 产纯文本指令，`llama.rs` 原样 `str_to_token` 喂入），thinking 不被触发；冒烟门第 4 步经验性确认输出无 `<think>` 块。

**结论**：本会话 bake-off 有效新候选 = **Qwen3-0.6B**（对照基线 Qwen2.5-1.5B v1）。

## 2. 指标对比

两候选用 v1 同配方（1000 step / num-layers 16 / batch 4 / lr 1e-4 / mask-prompt / oversample 8× / seed 42）在同一份 v0.5-patch 数据集（sha256 锁定）上训练，跑 v0.5 evals 双轨（parser-only baseline + `--with-fallback --hybrid` 全 500 case）。**单一变量 = 只换基座。**

| 模型 | 参数量 | GGUF (Q4_K_M) | parser-only (pass/partial/fail) | hybrid pass | partial | fail | rescued | regressed | 字段精度 | Metal p50/p95 (仅 fallback) |
|---|---|---|---|---|---|---|---|---|---|---|
| Qwen2.5-1.5B (v1 基线) | 1.5B | 940 MB | 472/26/2 | 480 (96.0%) | 18 | 2 | 8 | 0 | 96.0% | 1586 ms (p95) |
| **Qwen3-0.6B** | **0.6B** | **378 MB** | **472/26/2** | **480 (96.0%)** | **18** | **2** | **8** | **0** | **96.0%** | **981 / 1049 ms** |

**逐项读数（Qwen3-0.6B）**：
- **准确率与 v1 基线逐项相等**：hybrid pass 480 / partial 18 / fail 2、字段精确匹配 96.0%、parser→fallback 救回 472→480 (+8)、rescued_to_pass 8、**regressed 0**。
- **无退化解**：fallback 触发 86 次，regressed 0 + rescued 8 证明模型产出有效 intent（v0 阶段的「永远输出 `{}`」退化解失败模式不存在），valid_intent 与 v1（100%，86/86）同级。
- **更小**：Q4_K_M GGUF **378 MB vs v1 940 MB**（小 60%）。
- **Metal 上更快**：仅 fallback case p50 981 / **p95 1049 ms**，对比 v1 **p95 1586 ms**（快约 34%）；全 case p95 991 ms。训练 val loss 0.000 / train loss 0.003（与 v1 同款收敛）。

## 3. 推荐结论

按 spec §5 判定规则：

- **准确率对等判定**：Qwen3-0.6B hybrid pass 480，相对 v1（480）**净降 0 ≤ 2** ✓；**regressed 0 ≤ 2** ✓ → **准确率对等成立**。
- **落入分支①（更小候选对等）**：Qwen3-0.6B 在准确率逐项无损的前提下，体积小 60%、Metal 延迟快 34%、参数量不到基线一半。

**推荐：Qwen3-0.6B 作为弱硬件默认推理基座**（替代当前 v1 的 Qwen2.5-1.5B Q4_K_M）。质量零损失，更小更快，正中 BETA-17 立项动机（弱硬件推理速度瓶颈）。

**延迟绝对达标待 Windows 复核（开口项）**：Metal p95 1049 ms 仅提供「比 v1 更快」的相对排序证据；3000ms 交互门槛的**绝对达标判定**必须在目标弱硬件（BETA-09a 的 Intel Iris Xe / Vulkan，CPU/核显后端）复跑。BETA-09a 实测 v1 在该硬件 Vulkan p95 ~22s（远超门槛）——Qwen3-0.6B 体积小 60% 大概率显著改善，但需实测确认是否跨过 3000ms。**在 Windows 复核前，不对弱硬件交互达标性下最终结论。**

> 负结果路径（未触发）：本会话无「无候选对等 → 维持 v1」情形；Qwen3-0.6B 即对等且更优。

## 4. winner 产物登记

**winner = Qwen3-0.6B**（沿用 v1.md 登记模式；GGUF/adapter 本身 gitignore，凭以下信息可重建）：

- **基座**：`mlx-community/Qwen3-0.6B-4bit`（架构 `Qwen3ForCausalLM`，纯文本，Apache 2.0）
- **adapter**：`training/mlx-lora/adapters/beta17-qwen3-0.6b/`
- **Q4_K_M GGUF**：`training/mlx-lora/fused/beta17-qwen3-0.6b-q4_k_m.gguf`（378 MB）
  - **sha256**：`898c98bcaa40489742cbd6586f31e768a5d8d238da70eb58cff25a5eb19117df`
- **训练超参**（对齐 v1）：1000 step / num-layers 16 / batch 4 / lr 1e-4 / mask-prompt / nonempty oversample 8× / seed 42；val loss 0.000 / train loss 0.003 / peak mem 8.97 GB / 0.79 it/sec
- **复现命令**：`bash training/mlx-lora/scripts/run_bakeoff.sh mlx-community/Qwen3-0.6B-4bit qwen3-0.6b`

## 5. Windows 待办（交接）—— 已于 2026-06-02 闭合，见 §6

- ~~winner GGUF 传 Windows（校 sha256）→ BETA-09a 已验的 Vulkan 流程跑 v0.5 evals → 实测弱硬件（Intel Iris Xe）p50/p95 fallback 延迟，判定 3000ms 交互门槛绝对达标性。~~ ✅ 已做。
- 复用工作流：Mac 训练 → 传 GGUF（校 sha256）→ Windows 推理，同架构同量化免重编 —— 本次正是如此（GGUF sha256 一致，免重编，仅设 `LOCIFIND_MODEL_PATH`）。

## 6. Windows 延迟复核 + 推理优化（2026-06-02，Intel Iris Xe / Vulkan 真机）

**winner GGUF（sha256 `898c98bc…17df` 校验一致）在 Intel Iris Xe / Vulkan 跑完整 500 case v0.5 evals `--with-fallback --hybrid`，准确率与 Mac 逐项 0pp（pass 480/partial 18/fail 2/valid_intent 86/86/rescued 8/regressed 0）—— 跨平台一致性继续成立。**

延迟方面，初测发现弱核显单次 fallback 远超 3000ms 门槛，遂在不动准确率的前提下做了两项推理优化，三阶段实测如下：

| 阶段 | fallback p50 | fallback p95 | 全 case p95 | pass/partial/fail |
|---|---|---|---|---|
| 基线（Qwen3-0.6B） | 10758 ms | 13764 ms | 11297 ms | 480/18/2 |
| **+ ① `stop_at_json`** | 2075 ms | 2832 ms | 2480 ms | 480/18/2 |
| **+ ② prefix KV 复用** | **439 ms** | **1197 ms** | **898 ms** | 480/18/2 |

**优化 ①（首个 JSON 对象闭合即停）**：小模型输完 patch 后常"复读"到 `max_tokens=256`，而调用方（hybrid/full 路径）只取第一个 JSON 对象——多出的 ~200 token 在弱核显上是纯浪费的 decode。`GenerateParams::stop_at_json` 在生成循环里数花括号深度（正确忽略字符串内括号与转义），首个对象闭合即停。decode 主导段 ~9s→~1.5s。

**优化 ②（固定前缀 KV 复用）**：①之后 prefill 成主瓶颈——hybrid prompt 的 ~700 token 固定指令前缀**每次新建 context 全量重算**。`llama.rs` 改为**专用推理线程**（绕开 `LlamaContext` 的 `!Send`）持有常驻 context，固定前缀只 prefill 一次，每条 query 仅 `clear_kv_cache_seq` 丢上一条 suffix + decode 本条尾巴（`llama-cpp-4 0.3.0` 原生支持，无需升级）。prefill ~1.4s→~0.3s。

**整体快 11.5×（13764ms→1197ms），p95 距 3000ms 门槛有 60% 余量。准确率全程 byte-identical（regressed 0）。**

### 结论翻转

弱核显（Intel Iris Xe）跑模型 fallback 从 BETA-09a 记录的「准确但太慢 ~22s（v1）→ 必须能力感知降级到纯 parser」**翻转为「准确且 p95 ~1.2s，交互完全可用」**。三招叠加：Qwen3-0.6B 选型（22s→13.8s）+ `stop_at_json`（→2.8s）+ prefix KV 复用（→1.2s）。**能力感知降级从「硬性必需」降级为「可选优化」**——最弱核显也能交互式跑模型补全。

> 两项优化是**平台无关**的代码层改动（在 llama.rs 通用生成路径 / worker 线程），Mac Metal 同样受益（同省 token 比例 + 同省 prefill），但因 Metal 单 token 快约 10×，绝对收益小（Mac 本就 ~1s，远低于门槛）；Mac 实测复核留下个 Mac 会话。
</content>
