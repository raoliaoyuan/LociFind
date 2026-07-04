# BETA-09 v1 量化 baseline + release notes — 设计 spec

| 项 | 值 |
|---|---|
| ID | BETA-09 子 task — 量化 baseline + release notes |
| 作者 | Claude Code (Opus 4.7) |
| 日期 | 2026-05-28 |
| 阶段 | B 阶段（承接 BETA-08 v1 done） |
| 前置 | [BETA-08 v1 出场报告](../../reviews/beta-08-lora-v1.md) — adapter + fp16 GGUF + Q4_K_M GGUF 已 ready |
| 后续 | BETA-09 (a) Windows 跨平台部署验证（需 Windows 真机，本会话不做）|

## 1. 目标与范围

### 1.1 目标

把 v1 fp16 GGUF 在量化精度 spectrum 上拓展：除已有的 Q4_K_M（940 MB / p95 fallback 1592 ms / pass 468）外，再生成 Q5_K_M + Q6_K 两个变体，跑 v0.5 evals 拿 pass / 延迟 trade-off 数据。所有 artifact（adapter + 3 个量化级别 + fp16）的 sha256 / size / 训练参数 / 实测指标统一写入 `training/mlx-lora/releases/v1.md` 作为 v1 release 的**单一信源 (single source of truth)**。

### 1.2 出场标准

- **必达**：
  - Q5_K_M + Q6_K 两个 GGUF 都成功生成（llama-quantize 退出 0 + 文件大小合理）
  - 两个变体都跑通 v0.5 evals `--with-fallback --hybrid` 500 case（无 crash / 无 timeout）
  - `releases/v1.md` 覆盖所有 artifact（adapter + 3 GGUF + fp16）的 sha256 + size + 实测指标 + 训练参数 + commit hash + 使用示例
  - `releases/README.md` 索引 v1
- **不必达**：Q5/Q6 pass > Q4_K_M。验证性 task，不是优化性 task；Q5/Q6 与 Q4 持平本身是有用信号（"Q4_K_M 已达精度天花板"）

### 1.3 范围

- **量化目标**：Q5_K_M + Q6_K（**不**做 Q8_0，**不**做 v0 对照）
- **评测口径**：v0.5 fixture 全 500 case，`--with-fallback --hybrid`；parser-only baseline 仍取 v1 现有的 main-v1-baseline.json（parser 不变 → baseline 固定）
- **release 入库位置**：`training/mlx-lora/releases/v1.md` + `releases/README.md` 索引
- **量化脚本**：`training/mlx-lora/scripts/quantize_v1_variants.sh`（编排 quantize + evals × 2 变体）
- **训练 / parser / hybrid 架构**：完全不动

### 1.4 不在范围

- Q8_0 量化变体
- v0 fp16 量化对比
- BETA-09 (a) Windows 跨平台部署验证（需 Windows 真机）
- adapter 重训练 / 数据集变动
- Tier 2 数据 augmentation
- search.rs → ToolRegistry wiring（MVP-26 时一起）
- 改 evals 流程 / hybrid 架构 / parser

### 1.5 工具链前置（已就绪）

- ✅ v1 fp16 GGUF：`training/mlx-lora/fused/main-v1-f16.gguf` 2.9 GB
- ✅ v1 adapter：`training/mlx-lora/adapters/main-v1/`
- ✅ v1 Q4_K_M baseline：`training/mlx-lora/fused/main-v1-q4_k_m.gguf` 940 MB + `main-v1-baseline.json`
- ✅ `~/tools/llama.cpp/build/bin/llama-quantize`
- ✅ locifind-evals binary（`target/release/evals`，BETA-08 v1 已重编）

## 2. 与 BETA-08 v1 的边界

| 维度 | BETA-08 v1 | 本 spec |
|---|---|---|
| LoRA adapter | 训练完成 | 不动，复用 |
| fp16 GGUF | 生成完成 | 不动，复用作量化输入 |
| Q4_K_M GGUF | 生成 + evals | 不动，复用 evals 数据写入 release notes |
| Q5_K_M / Q6_K | 未生成 | **本 spec 目标** |
| release notes | 无 | **本 spec 目标** |

本 spec 的产出不影响 BETA-08 v1 任何已有 artifact；BETA-08 v1 done 状态不变。

## 3. 文件布局

```
training/mlx-lora/
├── fused/
│   ├── main-v1-f16.gguf              # 已有 2.9 GB（量化输入）
│   ├── main-v1-q4_k_m.gguf           # 已有 940 MB（不动）
│   ├── main-v1-q4_k_m-evals.log      # 已有（不动；rename: main-v1-evals.log → main-v1-q4_k_m-evals.log 保命名一致性）
│   ├── main-v1-q5_k_m.gguf           # 新生 ~1.1 GB，git-ignored
│   ├── main-v1-q5_k_m-evals.log      # 新生，git-ignored
│   ├── main-v1-q6_k.gguf             # 新生 ~1.3 GB，git-ignored
│   └── main-v1-q6_k-evals.log        # 新生，git-ignored
├── releases/
│   ├── README.md                     # 新建，索引各 release
│   └── v1.md                         # 新建，v1 release notes（单一信源）
└── scripts/
    └── quantize_v1_variants.sh       # 新建 ~30 行，量化 + evals × 2 编排
```

**注**：`main-v1-evals.log` rename 为 `main-v1-q4_k_m-evals.log` 仅为命名一致性，文件内容不变。git-ignored 文件 rename 不需要 commit 操作；脚本直接 mv 即可。

## 4. 量化脚本（`quantize_v1_variants.sh`）

```bash
#!/usr/bin/env bash
# BETA-09 v1 量化 baseline：Q5_K_M + Q6_K
# 详见 docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
FUSED="training/mlx-lora/fused"
GGUF_F16="$FUSED/main-v1-f16.gguf"
BASELINE_JSON="$FUSED/main-v1-baseline.json"  # 复用 v1 parser-only baseline

if [[ ! -f "$GGUF_F16" ]]; then
    echo "❌ 缺 v1 fp16 GGUF: $GGUF_F16" >&2; exit 1
fi
if [[ ! -f "$BASELINE_JSON" ]]; then
    echo "❌ 缺 v1 parser-only baseline: $BASELINE_JSON" >&2; exit 1
fi

quantize_and_evals() {
    local label="$1"   # Q5_K_M / Q6_K
    local suffix="$2"  # q5_k_m / q6_k
    local out_gguf="$FUSED/main-v1-${suffix}.gguf"
    local out_log="$FUSED/main-v1-${suffix}-evals.log"

    echo "==> quantize $label"
    "$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$out_gguf" "$label"

    echo "==> evals (--with-fallback --hybrid) on $label"
    LOCIFIND_MODEL_PATH="$out_gguf" \
    DYLD_LIBRARY_PATH="$ROOT/target/release" \
        ./target/release/evals \
            --fixtures v0.5 \
            --with-fallback \
            --hybrid \
            --baseline "$BASELINE_JSON" \
            2>&1 | tee "$out_log"
}

quantize_and_evals "Q5_K_M" "q5_k_m"
quantize_and_evals "Q6_K"   "q6_k"

echo "✅ v1 variants done"
ls -lh "$FUSED/main-v1-q5_k_m.gguf" "$FUSED/main-v1-q6_k.gguf"
```

**输出**：每个变体 ~6 min evals + ~10s 量化 = 总共 ~12 min。

## 5. release notes 结构（`releases/v1.md`）

10 节，固定模板（未来 v2/v3 可复用）：

1. **元数据表**：日期 / commit hash / 训练机器 / 基座模型 / spec / plan / 出场报告链接
2. **训练参数**（可 reproducible）：数据集 + sha256、mlx-lm 版本、超参（mask-prompt / num-layers / batch / lr / iters / seed）
3. **Artifacts**（sha256 + size 表）：adapter / fp16 / Q4 / Q5 / Q6 五行
4. **v0.5 evals 实测对比表**：变体 × {pass / partial / fail / rescued / regressed / p50 / p95}
5. **使用方法**：`LOCIFIND_MODEL_PATH=...` 加载示例 + adapter 直接挂 mlx 示例
6. **已知限制**：从 v1 报告 §6 引用残留 30 partial 分桶 + 适用场景
7. **变更历史**：v0 → v1 一句话差异（mask-prompt + oversample）
8. **依赖与许可**：基座模型 license（Qwen2.5 Apache 2.0）+ llama.cpp + mlx-lm 版本登记
9. **下一版本路标**：Tier 2 数据 augmentation 计划（不绑定时间）
10. **追溯信息**：spec / plan / 出场报告 / 本 release notes 自身 commit hash

## 6. 实测指标采集

每个变体跑 evals 后从 `--with-fallback --hybrid` 输出中提取：
- pass / partial / fail counts
- rescued_to_pass / rescued_to_partial / regressed
- p50 / p95 fallback 延迟
- variant 命中率 / 字段精确匹配率
- fallback valid_intent 比

数据格式：直接从 `*-evals.log` 用 grep / awk 提取入表。

## 7. sha256 与 size 采集

`releases/v1.md` 写入流程：
```
sha256 main-v1-{q4_k_m,q5_k_m,q6_k,f16}.gguf adapters/main-v1/adapters.safetensors
stat -f "%z" 同上文件
```
（macOS: `shasum -a 256 <file>`；`stat -f "%z" <file>` 出字节数）

## 8. 风险与缓解

| ID | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R1 | Q5/Q6 推理延迟超 MVP-25 §6.2 阈值（p95 < 3000ms） | 低 | 中 | release notes 实际记录；超阈值则在 §4 表内标 ⚠️ |
| R2 | Q5/Q6 pass 数实际比 Q4_K_M 低（量化模型间细微方差） | 低 | 低 | 不影响 done 判定；release notes 如实记录；结论"Q4 已饱和" |
| R3 | llama-quantize 对 Q5_K_M / Q6_K target 报错 | 极低 | 低 | 标准 K-quant level，llama.cpp 全量支持；若报错回 Q5_K_S |
| R4 | evals 跑某 variant 时 hybrid model load 失败 | 低 | 中 | 单步前置 ensure baseline 在；fail 立即停 |
| R5 | release notes 与实际 sha256 漂移（手动复制错） | 中 | 低 | 用 `shasum` 直接 pipe 输出，不手抄 |
| R6 | 量化产物 ~2.4 GB 撑爆 git-ignored 区或硬盘 | 低 | 低 | 已 `.gitignore` 全 `training/mlx-lora/fused/`；硬盘剩余 check 在 plan 中 |

## 9. 验收清单

实施完毕需满足全部 ✅：

- [ ] `training/mlx-lora/scripts/quantize_v1_variants.sh` 落地 + 可执行
- [ ] 跑脚本成功生成 `main-v1-q5_k_m.gguf` + `main-v1-q6_k.gguf`
- [ ] 两个变体 evals 退出 0，log 含 "总览" 段（与 v1 evals.log 同结构）
- [ ] `training/mlx-lora/releases/v1.md` 含 10 节，所有占位（sha256 / size / pass / 延迟）实测填齐
- [ ] `training/mlx-lora/releases/README.md` 索引 v1
- [ ] adapter + fp16 + 3 GGUF 的 sha256 与 stat 输出一致（验证步：`shasum` 重跑比对）
- [ ] STATUS + ROADMAP 同步（BETA-09 子项进度）
- [ ] `bash scripts/ci.sh` 通过

## 10. 决策摘要

| 决策 | 选择 | 来源 |
|---|---|---|
| 量化变体数 | Q5_K_M + Q6_K（不 Q8_0）| 用户确认 |
| v0 量化对比 | 不做 | 用户确认 |
| release 入库位置 | `training/mlx-lora/releases/v1.md` + `releases/README.md` 索引 | 用户确认 |
| 出场标准 | 验证性 task，不要求 Q5/Q6 > Q4 | spec §1.2 |
| baseline.json 复用 | 复用 main-v1-baseline.json（parser 不变 → 同一 baseline 适用全量化变体） | spec §1.3 |
| Q4_K_M log rename | main-v1-evals.log → main-v1-q4_k_m-evals.log 仅命名一致性，内容不变 | spec §3 |
