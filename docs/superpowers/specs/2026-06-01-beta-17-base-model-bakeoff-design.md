# BETA-17 基座模型选型实验（bake-off）设计

> 状态：spec（待 writing-plans）
> 日期：2026-06-01
> 作者：Claude Code (Opus 4.8)
> 关联：[ROADMAP §3.3 B3 BETA-17](../../../ROADMAP.md) · [BETA-09a Windows parity](../../reviews/beta-09a-windows-parity.md) · [BETA-08 v1 release notes](../../../training/mlx-lora/releases/v1.md)

## 1. 背景与动机

BETA-09(a) Windows 真机实测暴露 v1 GGUF（Qwen2.5-1.5B Q4_K_M）**纯 CPU/弱核显推理慢到不实用**（单次 fallback ~30-60s，Vulkan p95 ~22s，远超 3000ms 交互门槛）。复盘确认：

- 模型在 hybrid 架构中只承担**窄结构化 patch 任务**（parser 锁 variant、模型填字段）；
- parser-only 已达 472/500（94.4%），模型仅贡献 +8（→480，+1.6pp）；
- 残留质量缺口（artist/new_name）是**训练数据不足**而非模型容量不足（v1 报告 §6 实证）。

**结论：瓶颈是弱硬件上的推理速度，不是模型容量 → 正确方向是换"新一代的更小模型"而非更大。**

## 2. 目标与非目标

**目标**：在 v0.5-patch 数据集 + v1 同配方下，bake-off 对比新一代更小基座模型，验证能否在 hybrid patch 任务上**保住 v1 准确率（480/500）同时更小更快**，为弱硬件默认选型提供决策依据。

**铁律 = 单一变量**：只换基座模型；数据集、超参、LoRA 配方、评测口径全部对齐 v1。

**非目标（Out of scope）**：
- 运行时 wiring（winner 设默认 / 能力感知选模型）—— winner 定后单开任务；
- 升级 `llama-cpp-sys-4`（若 Qwen3.5 需要）—— 超范围则记 backlog；
- Windows 弱硬件延迟实测 —— 交接给后续 Windows 会话；
- 改 v0.5-patch 数据集 / 超参调优。

**本会话范围 = Mac 半**：准确率 bake-off（Metal evals）+ Metal 相对延迟。弱硬件（Windows）延迟绝对达标判定**不在本会话下结论**。

## 3. 候选与基线

候选集经 web 核实（2026-06）确定。landscape：
- **Qwen3**（2025-05）：dense 0.6B / 1.7B / 4B，Apache 2.0；
- **Qwen3.5**（2026-03）：dense 0.8B / 2B / 4B，Apache 2.0，比 Qwen3 新一代；
- **Qwen3.6**（2026-04）：仅 27B dense + 35B-A3B MoE，**无小 dense，与本任务无关**。

| 模型 | 角色 | 处理 |
|---|---|---|
| Qwen2.5-1.5B | v1 基线 | **不重训**，复用 v1 release notes 数字（pass 480 / 字段 96.0% / Metal p95 1586ms） |
| Qwen3-0.6B | 主候选 | 过冒烟门则训 |
| Qwen3-1.7B | 主候选 | 过冒烟门则训 |
| Qwen3.5-0.8B | 冲刺候选 | 过冒烟门则训；否则报告记跳过原因 |
| Qwen3.5-2B | 冲刺候选 | 过冒烟门则训；否则报告记跳过原因 |

所有候选 Apache 2.0，统一 Q4_K_M 量化推理。

## 4. 架构

### 4.1 Task 0 — 工具链冒烟门（任何训练之前）

本实验最大的不确定性收敛点。对每个候选验证完整工具链能消费它，**任一环节失败即把该候选移出 bake-off 并在报告记原因**：

1. **基座获取**：mlx-community 拉到 4bit 基座；确认 0.8B/2B 是纯文本基座（非多模态——web 提示 Qwen3.5 部分变体带 mmproj 视觉文件）。
2. **mlx 训练识别**：mlx-lm 0.29.1 能 load + 跑一次最小 LoRA（几 step），验架构被识别。
3. **GGUF 转换**：`convert_hf_to_gguf.py` 能转 fp16 GGUF + `llama-quantize` 出 Q4_K_M。
4. **关键 — 钉死推理栈**：`llama-cpp-sys-4 0.3.0`（BETA-09a 在用）能 load 该 GGUF 跑一次最小推理（non-thinking）+ 产出合法 JSON。

> 第 4 步是硬门：它把「Qwen3.5 是否需要升级 llama-cpp-sys-4」这个风险在花训练时间**之前**回答掉。若 Qwen3.5 卡在第 4 步且升级 0.3.0 超出本会话范围，老实记录「Qwen3.5 待工具链升级」，本会话以 Qwen3 两候选闭合。

### 4.2 non-thinking 处理

Qwen3 / Qwen3.5 为 dual-mode。本任务要快速产结构化 JSON 补丁，thinking 会暴涨延迟、是负担，必须 `enable_thinking=False`。

- **llama.cpp 限制**：硬开关（chat template 的 enable_thinking=False）**不在 llama.cpp 暴露**，workaround = 自定义 chat template 经 `--chat-template-file` 或运行期等价模板强制 non-thinking；
- **训练侧**：v0.5-patch 的 assistant completion 本就是纯 JSON patch（无 thinking 块），训练数据天然 non-thinking；
- **冒烟门第 4 步必须验证**：推理产出无 `<think>` 块、是合法 JSON。

### 4.3 训练配方（对齐 v1，单一变量）

- **数据**：复用 `training/mlx-lora/data/main/{train,valid}.jsonl`（v0.5-patch，sha256 锁定，零改动）。数据是 model-agnostic 的 chat messages 格式，mlx-lm 对每个基座套各自 chat 模板。
- **超参**（完全维持 v1）：1000 step / num-layers 16 / batch 4 / lr 1e-4 / mask-prompt / nonempty oversample 8× / seed 42。
- **管线**：复用 `run_main_v1.sh` 的 7 步结构，**参数化**模型名/路径产出 `run_bakeoff.sh`（或 per-model 调用）。每候选独立 adapter 目录 `adapters/beta17-<model>/`、独立 fused/GGUF 路径。

### 4.4 评测（对齐 v1 口径）

每个过门候选跑两轨：

- **parser-only baseline**：应与 v1 同 **472/26/2**（验 wiring 不破，parser 不动）；
- **`--with-fallback --hybrid`**：全 500 case。

采集指标：pass / partial / fail / rescued_to_pass / regressed / 字段精确匹配 / **Metal p50·p95 fallback 延迟** / 常驻内存 / GGUF 体积。

## 5. Winner 判定规则

- **首要门槛**：候选 hybrid pass 数相对 v1（480）**净降 ≤2** 视为「准确率对等」；`regressed` 必须 ≤2（不制造新错）。
- **选优**：在所有「准确率对等」候选中，取**最小 + 弱硬件最快**者作弱硬件默认推荐。
- **分支结论**：
  - 有更小候选（0.6B/0.8B）对等 → 推荐为弱硬件默认（质量无损、更快更小）；
  - 小候选掉质量、仅 1.7B/2B 对等 → 推荐为「新一代质量更稳」升级，弱硬件延迟需 Windows 复核；
  - **无候选对等**（都比 v1 差）→ 结论「维持 Qwen2.5-1.5B v1」。这是**有效负结果**——实验排除了一批选项。
- **延迟终判留 Windows**：Mac Metal 延迟只给相对排序；3000ms 交互门槛的绝对达标判定必须在弱硬件（BETA-09a 的 Intel 核显）复跑——报告标注为「待 Windows 复核」开口项。

## 6. 交付物（本会话）

1. **`docs/reviews/beta-17-base-model-bakeoff.md`** —— 候选表 + 冒烟门结果（含跳过原因）+ 各模型指标对比表 + 推荐结论 + 弱硬件默认建议 + Windows 待办。
2. **winner（及对等候选）产物登记** —— adapter / fp16+Q4_K_M GGUF / sha256 + 训练参数入库（GGUF 本身 gitignore，sha256 进报告，沿用 v1.md 模式）。
3. **`run_bakeoff.sh`** —— 参数化训练+评测管线 + 为 non-thinking 准备的自定义 chat template 文件（如需）。
4. **收工同步** —— STATUS / ROADMAP（BETA-17 状态 + Windows 延迟复核作明确下一步）。

## 7. 风险与已知限制

- **R1 工具链**：`llama-cpp-sys-4 0.3.0` / mlx-lm 0.29.1 可能不识别 Qwen3/3.5 架构 → 冒烟门（§4.1）前置拦截，降级闭合。
- **R2 non-thinking**：硬开关 llama.cpp 不暴露 → 自定义 chat template 强制；冒烟门验证产出无 thinking 块、是合法 JSON（§4.2）。
- **R3 延迟不可终判**：Mac Metal ≠ 弱硬件，绝对达标留 Windows，诚实标注，不在本会话下结论。
- **R4 GGUF 单点本地依赖**：沿用 v1 已知限制，产物 gitignore，靠 sha256 + 脚本可重建。

## 8. 成功标准

- 所有过冒烟门的候选完成同配方训练 + 双轨 evals，指标入对比表；
- 报告给出明确推荐（含负结果路径）+ Windows 延迟复核开口项；
- 各候选 parser-only baseline 472/26/2 byte-equal（证 wiring/parser 不破）；
- 收工前 `cargo fmt --check` / `clippy -D warnings` / 既有 evals 回归门绿（若本会话触及任何 Rust 代码；纯训练+脚本+文档则不涉及）。
</content>
</invoke>
