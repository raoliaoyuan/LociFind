# BETA-09 v1 量化 baseline + release notes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 v1 fp16 GGUF 上量化出 Q5_K_M + Q6_K 两个变体，跑 v0.5 evals 拿 pass / 延迟 trade-off，所有 v1 artifact（adapter + fp16 + 3 GGUF 量化级别）的 sha256 / size / 训练参数 / 实测指标统一入库 `training/mlx-lora/releases/v1.md`。

**Architecture:** 一个 bash 编排脚本 `quantize_v1_variants.sh` 串行做 `llama-quantize → evals` × 2 variant；产物全 git-ignored。Release notes 是 v1 分发的单一信源（single source of truth），10 节固定模板未来 v2/v3 可复用。`releases/README.md` 一行索引各 release。

**Tech Stack:** llama.cpp `llama-quantize`（量化）、locifind-evals Rust binary（v0.5 fixture，已编译）、`shasum -a 256` + `stat -f "%z"`（macOS sha256 / size 采集）。

**前置 spec:** [docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md](../specs/2026-05-28-beta-09-quantize-baseline-design.md)
**前置 ledger:** [docs/reviews/beta-08-lora-v1.md](../../reviews/beta-08-lora-v1.md) — v1 adapter + fp16 + Q4_K_M 已 ready

---

## 文件结构总览

| 文件 | 操作 | 用途 |
|---|---|---|
| `training/mlx-lora/fused/main-v1-evals.log` | rename → `main-v1-q4_k_m-evals.log` | 命名一致性；文件 git-ignored 直接 mv |
| `training/mlx-lora/scripts/quantize_v1_variants.sh` | 新建 | 量化 + evals × 2 编排脚本 |
| `training/mlx-lora/fused/main-v1-q5_k_m.gguf` | 新生（git-ignored） | Q5_K_M 量化产物 |
| `training/mlx-lora/fused/main-v1-q5_k_m-evals.log` | 新生（git-ignored） | Q5_K_M evals 日志 |
| `training/mlx-lora/fused/main-v1-q6_k.gguf` | 新生（git-ignored） | Q6_K 量化产物 |
| `training/mlx-lora/fused/main-v1-q6_k-evals.log` | 新生（git-ignored） | Q6_K evals 日志 |
| `training/mlx-lora/releases/README.md` | 新建 | release 索引 |
| `training/mlx-lora/releases/v1.md` | 新建 | v1 release notes（单一信源，10 节） |
| `STATUS.md` | 修改 | 当前阶段 + 会话日志（顶部追加） |
| `ROADMAP.md` | 修改 | BETA-09 子项进度备注 |

---

## Task 1: rename `main-v1-evals.log` 保命名一致性

**Files:**
- Rename: `training/mlx-lora/fused/main-v1-evals.log` → `training/mlx-lora/fused/main-v1-q4_k_m-evals.log`

**目标**：让 Q4/Q5/Q6 三个变体的 evals log 命名一致（都带量化级别后缀）。文件 git-ignored 不需要 commit。

- [ ] **Step 1: 确认源文件在**

```bash
ls -lh training/mlx-lora/fused/main-v1-evals.log
```

预期：~356 KB 文件存在（来自 BETA-08 v1）。

- [ ] **Step 2: rename**

```bash
mv training/mlx-lora/fused/main-v1-evals.log \
   training/mlx-lora/fused/main-v1-q4_k_m-evals.log
```

- [ ] **Step 3: 确认 rename 成功 + 旧文件不在**

```bash
ls -lh training/mlx-lora/fused/main-v1-q4_k_m-evals.log
ls training/mlx-lora/fused/main-v1-evals.log 2>&1 || echo "OK: old file gone"
```

预期：新文件存在 + 旧文件 "No such file or directory"。

- [ ] **Step 4: git status 应无任何变化（git-ignored 文件）**

```bash
git status --short training/mlx-lora/fused/
```

预期：无输出（fused/ 全 git-ignored）。

无 commit 步骤。

---

## Task 2: 新建 `releases/README.md` 索引

**Files:**
- Create: `training/mlx-lora/releases/README.md`

**目标**：建一个空索引文件，后续 v2/v3 加 entry。本会话只加 v1 一行（v1.md 还未写但路径稳定）。

- [ ] **Step 1: 建目录**

```bash
mkdir -p training/mlx-lora/releases
```

- [ ] **Step 2: 写 README.md**

写入文件 `training/mlx-lora/releases/README.md`：

```markdown
# LociFind LoRA Releases

此目录登记每个 LoRA adapter release 的元数据（sha256 / size / 训练参数 / 实测指标 / 使用方法），作为分发追溯的**单一信源**。

训练产物（adapter / GGUF）本身 git-ignored；本目录文档化它们的 hash 与生产参数，便于下载方 verify。

## Releases

| 版本 | 日期 | 状态 | 入口 | 推荐推理 GGUF |
|---|---|---|---|---|
| v1 | 2026-05-28 | ready | [v1.md](./v1.md) | `main-v1-q4_k_m.gguf` (940 MB) |

## 添加新 release

每个新 release 加一个独立 `vN.md`（与 v1.md 同结构），并在上表追加一行。模板字段：

1. 元数据（日期 / commit / 训练机器 / 基座 / spec / plan / 出场报告）
2. 训练参数（dataset + sha256 / mlx-lm 版本 / 超参）
3. Artifacts（sha256 + size 表）
4. v0.5 evals 实测对比表
5. 使用方法
6. 已知限制
7. 变更历史 vs 上一版
8. 依赖与许可
9. 下一版本路标
10. 追溯信息
```

- [ ] **Step 3: 验证文件正确生成**

```bash
ls -l training/mlx-lora/releases/README.md
head -3 training/mlx-lora/releases/README.md
```

预期：文件存在；head 输出 `# LociFind LoRA Releases`。

- [ ] **Step 4: 不 commit（与 Task 5 v1.md 一起 commit）**

---

## Task 3: 新建 `quantize_v1_variants.sh` + 静态验证

**Files:**
- Create: `training/mlx-lora/scripts/quantize_v1_variants.sh`

**目标**：编排 `llama-quantize → evals` × 2 variant 的脚本。脚本输出 + 大小可在事后验证（不在脚本内做强校验）。

- [ ] **Step 1: 前置硬盘空间 check**

```bash
df -h "$HOME" | tail -1
ls -lh training/mlx-lora/fused/main-v1-f16.gguf
```

预期：剩余空间 ≥ 3 GB（Q5 ~1.1 GB + Q6 ~1.3 GB + 余量）；fp16 GGUF 仍在（2.9 GB）。

- [ ] **Step 2: 写脚本**

写入 `training/mlx-lora/scripts/quantize_v1_variants.sh`：

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
BASELINE_JSON="$FUSED/main-v1-baseline.json"

if [[ ! -f "$GGUF_F16" ]]; then
    echo "❌ 缺 v1 fp16 GGUF: $GGUF_F16" >&2
    exit 1
fi
if [[ ! -f "$BASELINE_JSON" ]]; then
    echo "❌ 缺 v1 parser-only baseline: $BASELINE_JSON" >&2
    exit 1
fi
if [[ ! -x "$LLAMA_CPP/build/bin/llama-quantize" ]]; then
    echo "❌ 缺 llama-quantize 工具" >&2
    exit 1
fi

quantize_and_evals() {
    local label="$1"   # Q5_K_M / Q6_K
    local suffix="$2"  # q5_k_m / q6_k
    local out_gguf="$FUSED/main-v1-${suffix}.gguf"
    local out_log="$FUSED/main-v1-${suffix}-evals.log"

    echo "==> [$label] quantize"
    "$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$out_gguf" "$label"

    echo "==> [$label] evals (--with-fallback --hybrid)"
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

- [ ] **Step 3: 静态验证**

```bash
chmod +x training/mlx-lora/scripts/quantize_v1_variants.sh
bash -n training/mlx-lora/scripts/quantize_v1_variants.sh && echo "syntax OK"
grep -E "Q5_K_M|Q6_K|llama-quantize|with-fallback|--hybrid" training/mlx-lora/scripts/quantize_v1_variants.sh | head
```

预期：syntax OK + 五个关键 token 都出现。

- [ ] **Step 4: commit**

```bash
git add training/mlx-lora/scripts/quantize_v1_variants.sh
git commit -m "BETA-09：新建 quantize_v1_variants.sh，量化 + evals × 2 编排"
```

---

## Task 4: 跑 quantize_v1_variants.sh（~12 min, background）

**Files:**
- Run: `training/mlx-lora/scripts/quantize_v1_variants.sh`
- Output (git-ignored): `training/mlx-lora/fused/main-v1-q5_k_m.gguf` + `*-evals.log` + 同样 Q6_K 两个文件

**目标**：从仓库根跑脚本，后台 ~12 min；脚本退出 0 即成功。任何 step 异常立停。

- [ ] **Step 1: 启动后台 bash（必须 background；Bash tool 10 min timeout < 脚本 12 min）**

用 `run_in_background=true` 起 Bash：

```bash
bash training/mlx-lora/scripts/quantize_v1_variants.sh > /tmp/v1-quantize.log 2>&1
```

记录 background ID。系统将在完成时通知。**不要 poll / sleep**。

- [ ] **Step 2: 快验启动正常（启动 5s 后看 log head 确认进了第一 quantize 段）**

```bash
sleep 5 && head -10 /tmp/v1-quantize.log
```

预期：第一行 `==> [Q5_K_M] quantize`，紧跟 llama-quantize 输出。任何 ❌ 立刻 kill background + 报告。

- [ ] **Step 3: 等系统通知完成（~12 min）**

完成通知 status 应为 `completed` (exit 0)。如 `failed`，立即读 /tmp/v1-quantize.log 全文诊断。

- [ ] **Step 4: 验证四个产物落地**

```bash
ls -lh training/mlx-lora/fused/main-v1-q5_k_m.gguf \
       training/mlx-lora/fused/main-v1-q5_k_m-evals.log \
       training/mlx-lora/fused/main-v1-q6_k.gguf \
       training/mlx-lora/fused/main-v1-q6_k-evals.log
```

预期：q5_k_m.gguf ~1.1 GB / q6_k.gguf ~1.3 GB；两个 log 各 ~300 KB。

- [ ] **Step 5: 验证两个 evals log 都包含 "总览" 段**

```bash
grep -c "^总览：" training/mlx-lora/fused/main-v1-q5_k_m-evals.log
grep -c "^总览：" training/mlx-lora/fused/main-v1-q6_k-evals.log
```

预期：两个都输出 1（表示 evals 跑完整 + summary 段出现）。

- [ ] **Step 6: 提取每个变体的关键指标供 Task 5 用**

```bash
for variant in q5_k_m q6_k; do
  echo "=== $variant ==="
  grep -E "^  pass:|^  partial:|^  fail:|rescued_to_pass|regressed|valid_intent|^    p50:|^    p95:|variant 命中率|字段级精确" \
       training/mlx-lora/fused/main-v1-${variant}-evals.log | head -20
done
```

记下数字，Task 5 写表用。

**Task 4 无 commit**（所有产物 git-ignored）。

---

## Task 5: 写 `releases/v1.md`（10 节，实测数据填齐）

**Files:**
- Create: `training/mlx-lora/releases/v1.md`

**目标**：v1 release 的单一信源。10 节固定模板，所有占位用 Task 4 实测数据 + `shasum` / `stat` 命令实测填齐，不留 TBD。

- [ ] **Step 1: 采集所有 artifact 的 sha256 + size**

```bash
echo "=== sha256 ==="
shasum -a 256 \
  training/mlx-lora/adapters/main-v1/adapters.safetensors \
  training/mlx-lora/fused/main-v1-f16.gguf \
  training/mlx-lora/fused/main-v1-q4_k_m.gguf \
  training/mlx-lora/fused/main-v1-q5_k_m.gguf \
  training/mlx-lora/fused/main-v1-q6_k.gguf
echo "=== size (bytes) ==="
stat -f "%z %N" \
  training/mlx-lora/adapters/main-v1/adapters.safetensors \
  training/mlx-lora/fused/main-v1-f16.gguf \
  training/mlx-lora/fused/main-v1-q4_k_m.gguf \
  training/mlx-lora/fused/main-v1-q5_k_m.gguf \
  training/mlx-lora/fused/main-v1-q6_k.gguf
echo "=== human-readable size ==="
ls -lh \
  training/mlx-lora/adapters/main-v1/adapters.safetensors \
  training/mlx-lora/fused/main-v1-f16.gguf \
  training/mlx-lora/fused/main-v1-q4_k_m.gguf \
  training/mlx-lora/fused/main-v1-q5_k_m.gguf \
  training/mlx-lora/fused/main-v1-q6_k.gguf
```

把输出复制下来 Task 5 Step 3 用。

- [ ] **Step 2: 采集 dataset sha256 + commit hash**

```bash
cat training/datasets/v0.5-patch/v0/meta.json | python3 -c "import json,sys; m=json.load(sys.stdin); print('source_sha256:', m.get('source_sha256','?')); print('total:', m.get('stats',{}).get('total','?'))"
git rev-parse HEAD
```

记下 source_sha256 + current commit hash。

- [ ] **Step 3: 写 release notes（10 节）**

写入 `training/mlx-lora/releases/v1.md`：

```markdown
# LociFind LoRA v1 — Release Notes

> 单一信源：v1 release 的元数据、训练参数、artifact hash、实测指标、使用方法、限制全在此文件。
> 训练产物（adapter / GGUF）git-ignored；本文件记录它们的 sha256 与生产参数，便于 verify。

## 1. 元数据

| 项 | 值 |
|---|---|
| 版本 | v1 |
| 日期 | 2026-05-28 |
| 状态 | **ready** |
| commit (训练时) | `<Step 2 实测 git rev-parse 输出>` |
| commit (本 release notes) | `<本 commit hash，最后 commit 后回填或留 "见 git log">` |
| 训练机器 | macOS 15.5 / Apple M5 Pro / 55 GB Metal recommendedMaxWorkingSetSize |
| 基座模型 | Qwen2.5-1.5B-Instruct (MLX 4bit), `~/models/qwen25_1.5b_draft/` (828 MB) |
| Spec | [BETA-08 v1 spec](../../docs/superpowers/specs/2026-05-27-beta-08-v1-design.md) |
| Plan | [BETA-08 v1 plan](../../docs/superpowers/plans/2026-05-27-beta-08-v1.md) |
| 出场报告 | [beta-08-lora-v1.md](../../docs/reviews/beta-08-lora-v1.md) |
| BETA-09 量化 spec | [2026-05-28-beta-09-quantize-baseline-design.md](../../docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md) |

## 2. 训练参数（reproducible）

- **数据集**：[`training/datasets/v0.5-patch/v0/cases.jsonl`](../datasets/v0.5-patch/v0/cases.jsonl)
- **dataset source_sha256**：`<Step 2 实测>`
- **dataset 大小**：498 record（443 empty patch + 55 nonempty patch）
- **prepare 脚本**：`training/mlx-lora/scripts/prepare_main_data.py`（chat format + `NONEMPTY_OVERSAMPLE=8` → 443 + 440 = 883 record train）
- **训练命令**：`training/mlx-lora/scripts/run_main_v1.sh`
- **mlx-lm 版本**：0.29.1
- **超参**：mask-prompt / num-layers 16 / batch 4 / lr 1e-4 / iters 1000 / seed 42
- **训练耗时**：~42 min for 1000 step（0.39 it/sec / peak mem 12.4 GB）

## 3. Artifacts (sha256 + size)

| 文件 | sha256 | size | 备注 |
|---|---|---|---|
| `training/mlx-lora/adapters/main-v1/adapters.safetensors` | `<Step 1 实测>` | 20 MB | LoRA adapter (MLX 格式) |
| `training/mlx-lora/fused/main-v1-f16.gguf` | `<Step 1 实测>` | 2.9 GB | 合并 fp16 GGUF (量化基线) |
| `training/mlx-lora/fused/main-v1-q4_k_m.gguf` | `<Step 1 实测>` | 940 MB | **推荐量化**（pass 468 / p95 fallback 1592 ms） |
| `training/mlx-lora/fused/main-v1-q5_k_m.gguf` | `<Step 1 实测>` | ~1.1 GB | 高精度变体 |
| `training/mlx-lora/fused/main-v1-q6_k.gguf` | `<Step 1 实测>` | ~1.3 GB | 最高精度量化变体 |

字节级 size 见各 artifact 旁注。

## 4. v0.5 evals 实测对比（with-fallback + hybrid，500 case）

Baseline：parser-only 460 pass / 38 partial / 2 fail。

| 变体 | pass | partial | fail | rescued_to_pass | regressed | variant 命中率 | 字段精确匹配 | p50 fallback (ms) | p95 fallback (ms) | valid_intent 比 |
|---|---|---|---|---|---|---|---|---|---|---|
| Q4_K_M | 468 (93.6%) | 30 | 2 | 8 | 0 | 99.6% | 93.6% | 1565 | 1592 | 100% (86/86) |
| Q5_K_M | `<Task 4 Step 6 实测>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` |
| Q6_K | `<Task 4 Step 6 实测>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` | `<>` |

**结论**（写实测后）：`<根据 Q5/Q6 数据写一句话：例如 "Q5_K_M pass 与 Q4 持平、延迟 +X ms → Q4 已是 sweet spot" 或 "Q5_K_M 多救 1 case 但延迟 +X ms → 看场景取舍"。如出现 regression 实事求是记录>`

## 5. 使用方法

```bash
# 直接加载 GGUF 跑 v0.5 evals (CLI)
LOCIFIND_MODEL_PATH=training/mlx-lora/fused/main-v1-q4_k_m.gguf \
DYLD_LIBRARY_PATH=target/release \
    ./target/release/evals --fixtures v0.5 --with-fallback --hybrid

# 走 ModelDaemon (常驻进程，hybrid 架构)
# - parser 锁 variant
# - 模型只产生 patch 字段
# - apply_patch 合并
# 详见 packages/intent-parser/src/hybrid.rs
```

可选量化级别：

- **Q4_K_M (940 MB)**: 默认推荐，pass 468 / p95 1592 ms，对延迟敏感场景
- **Q5_K_M (~1.1 GB)**: `<本 release Step 4 实测决定推荐场景>`
- **Q6_K (~1.3 GB)**: `<本 release Step 4 实测决定推荐场景>`

## 6. 已知限制（残留 30 partial buckets）

来自 v0.5 evals 残留：30 partial / 2 fail。按 diff 字段分桶（v1 Q4_K_M 实测）：

| 字段 | 数量 | 典型 diff | 救援前景 |
|---|---|---|---|
| keywords | 7 | screenshot keyword 误填位置词 | LoRA 救不动（数据集 nonempty case 不含此模式） |
| language | 6 | mixed↔zh 检测分歧 | 检测器 trade-off 边缘 case |
| file_type | 6 | parser 主动加 file_type 但 fixture 期望 null | parser 侧策略问题 |
| artist | 4 | `find songs by synthetic-artist` 未识别 | LoRA 救不动（55 nonempty 中 artist=0） |
| new_name | 3 | "把第5个 rename 为 X" 未提取 | LoRA 救不动（55 nonempty 中 new_name=2） |
| location | 3 | hint 中英形态分歧 | 已收敛到边缘 |
| title | 1 | media title | 边缘 |
| modified_time | 1 | absolute range vs before type | 边缘 |

**结论**：v1 后再推 pass 需 Tier 2 数据 augmentation 合成 nonempty patch case（keywords/artist/new_name 三个 bucket 各补 ~20 case），单纯加 LoRA 参数 / 调超参不会再有突破。

## 7. 变更历史（vs v0）

- **v1（本 release）**：mask-prompt + nonempty oversample 8× 攻克 v0 退化解；pass 460→**468 (+8)** / regressed 0 / fallback valid_intent 比 8.3%→**100%**
- **v0 (2026-05-27)**：管线全通但模型学到"永远输出 `{}`" 退化解，pass 净增 0，标 not_ready；详 [v0 报告](../../docs/reviews/beta-08-lora-v0.md)

## 8. 依赖与许可

| 组件 | 版本 | License | 用途 |
|---|---|---|---|
| 基座模型 Qwen2.5-1.5B-Instruct | mlx 4bit 格式 | Apache 2.0 (Qwen) | 训练起点 |
| mlx-lm | 0.29.1 | MIT | LoRA 训练 |
| llama.cpp | 含 `convert_hf_to_gguf.py` + `llama-quantize` | MIT | GGUF 转换 + 量化 |
| llama-cpp-rs | 0.3.x | MIT | Rust 推理 binding |
| locifind-evals | 本仓库 | （仓库 LICENSE） | 评测 binary |

详见 [docs/third-party-licenses.md](../../docs/third-party-licenses.md)。

## 9. 下一版本路标

候选改进方向（按 ROI 排序，不绑定时间）：

1. **Tier 2 数据 augmentation**：合成 keywords/artist/new_name 三个 bucket 各 ~20 nonempty patch case，预期 +10-15 pass
2. **更大量化变体 Q8_0**：本 release 未跑，待用户提需求时再扩
3. **基座升级 Qwen3-1.7B**：基座 backlog，需评估 v0.5 evals base 表现是否值得切

## 10. 追溯信息

- spec：[2026-05-27-beta-08-v1-design.md](../../docs/superpowers/specs/2026-05-27-beta-08-v1-design.md)
- plan：[2026-05-27-beta-08-v1.md](../../docs/superpowers/plans/2026-05-27-beta-08-v1.md)
- 出场报告：[beta-08-lora-v1.md](../../docs/reviews/beta-08-lora-v1.md)
- BETA-09 量化 spec：[2026-05-28-beta-09-quantize-baseline-design.md](../../docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md)
- BETA-09 量化 plan：[2026-05-28-beta-09-quantize-baseline.md](../../docs/superpowers/plans/2026-05-28-beta-09-quantize-baseline.md)
```

- [ ] **Step 4: 把 Step 1 + Step 2 + Task 4 Step 6 实测数据填入对应 `<...>` 占位**

人工核对：

- §1 commit (训练时) ← Step 2 `git rev-parse HEAD` 输出
- §2 dataset source_sha256 ← Step 2 meta.json 提取
- §3 五行 sha256 ← Step 1 输出
- §4 Q5_K_M / Q6_K 两行 11 列 ← Task 4 Step 6 提取
- §4 结论一行 ← 根据 Q5/Q6 vs Q4 实测下结论
- §5 Q5/Q6 推荐场景两行 ← 根据 §4 数据决定

- [ ] **Step 5: 自检无 `<...>` / `TBD` / `TODO` 残留**

```bash
grep -nE "<.*>|TBD|TODO" training/mlx-lora/releases/v1.md
```

预期：无输出（所有占位填齐）。如有残留，回 Step 4 补。

- [ ] **Step 6: commit（含 Task 2 README + Task 5 v1.md）**

```bash
git add training/mlx-lora/releases/
git commit -m "BETA-09：v1 release notes 入库（10 节单一信源 + README 索引）"
```

---

## Task 6: 收工同步 STATUS + ROADMAP + ci.sh 兜底

**Files:**
- Modify: `STATUS.md`
- Modify: `ROADMAP.md`

按 [CONVENTIONS.md §3 收工流程](../../../CONVENTIONS.md)。

- [ ] **Step 1: 跑 ci.sh 兜底**

```bash
bash scripts/ci.sh
```

预期：fmt + clippy + build + test 全过（本会话无 Rust 改动）。

- [ ] **Step 2: 改 `STATUS.md` 顶部当前阶段段**

在第 22 阶段 BETA-08 v1 段后追加新一段：

```
> **BETA-09 量化 baseline + release notes（第 23 阶段，done）**：在 v1 fp16 GGUF 上量化 Q5_K_M + Q6_K，跑 v0.5 evals `--with-fallback --hybrid` 拿 pass / 延迟 trade-off。Q5_K_M `<size>` / pass `<X>` / p95 fallback `<Y> ms`；Q6_K `<size>` / pass `<X>` / p95 `<Y> ms`。结论：`<根据数据写一句话>`。v1 所有 artifact（adapter + fp16 + 3 GGUF）的 sha256 + 训练参数 + 实测指标统一入库 [training/mlx-lora/releases/v1.md](./training/mlx-lora/releases/v1.md)，未来分发追溯单一信源。BETA-09 (b)+(c) 标 done；BETA-09 (a) 跨平台部署仍卡 Windows 真机。
```

把"当前 Task"段改为：
```
无进行中。本会话第 23 阶段：**BETA-09 量化 baseline + release notes**（done）。
```

把"下一步" Class A 段第 1 条 `BETA-09 模型量化与跨平台部署` 改为：
```
1. **BETA-09 (a) 跨平台部署** — Windows 真机加载 v1 GGUF（推荐 Q4_K_M 940 MB 或按 release notes §4 选择级别）验证推理路径与 macOS 一致；跑 v0.5 evals 与 release notes §4 实测对比。需 Windows 真机。
```

- [ ] **Step 3: 改 `STATUS.md` 会话日志（顶部追加）**

在 BETA-08 v1 条目之前插入：

```
### 2026-05-28 — Claude Code (Opus 4.7) — BETA-09 量化 baseline + release notes：v1 fp16 → Q5_K_M + Q6_K + 单一信源入库（第 23 阶段，主会话 inline）

**关键决策**

- 承接 BETA-08 v1 done，推 BETA-09 纯代码部分（用户从 4 候选中选 A）
- 走 superpowers 流程：brainstorming → writing-plans → executing-plans
- 4 个用户决策：量化 Q5_K_M + Q6_K（不 Q8）/ 不做 v0 对照 / release 入库 `training/mlx-lora/releases/v1.md` + README 索引 / 出场标准不要求 Q5/Q6 > Q4
- spec [2026-05-28-beta-09-quantize-baseline-design.md](./docs/superpowers/specs/2026-05-28-beta-09-quantize-baseline-design.md)
- plan [2026-05-28-beta-09-quantize-baseline.md](./docs/superpowers/plans/2026-05-28-beta-09-quantize-baseline.md)
- 量化产物全 git-ignored；release notes 入库的是 metadata（sha256 + 实测指标），artifact 本身不入库

**实测**（写完后填）

- Q5_K_M `<size>` / pass `<X>` / p95 fallback `<Y> ms`
- Q6_K `<size>` / pass `<X>` / p95 `<Y> ms`
- 结论：`<>`

**产出**

- N commit（main 分支）：spec + plan + quantize 脚本 + release notes(含 README) + 收工
- `training/mlx-lora/scripts/quantize_v1_variants.sh`（~50 行）
- `training/mlx-lora/releases/{README.md, v1.md}` 入库
- `training/mlx-lora/fused/main-v1-{q5_k_m,q6_k}.gguf` + `*-evals.log` 全 git-ignored

**未尽事宜 → 已转入下一步**

- BETA-09 (a) Windows 跨平台部署：需 Windows 真机
- MVP-26 跨平台一致性：可与 BETA-09 (a) 合并启动
- 长周期事项不变
```

- [ ] **Step 4: 改 `ROADMAP.md` BETA-09 状态**

BETA-09 一行的"状态"列从：
```
not_started
```
改为：
```
in_progress（v1 量化 baseline + release notes done 2026-05-28：Q5_K_M + Q6_K 实测入库 training/mlx-lora/releases/v1.md；跨平台部署 (a) 仍卡 Windows 真机）
```

- [ ] **Step 5: commit**

```bash
git add STATUS.md ROADMAP.md
git commit -m "BETA-09 量化 baseline 收工：STATUS + ROADMAP 同步"
```

- [ ] **Step 6: 向用户报告本会话所有 commit**

```bash
git log --oneline -10
```

把 commit 列表给用户做收工确认。

---

## Self-Review（plan 写完后回查）

**1. Spec coverage**：

| Spec 章节 | Plan task |
|---|---|
| §1.1 目标 + §1.2 出场标准 | Task 4 Step 4-6 + Task 5 全部 step |
| §1.3 范围（量化目标 / 评测口径 / release 位置 / 量化脚本） | Task 3 (脚本) + Task 4 (跑) + Task 5 (notes) |
| §1.4 不在范围 | 全部 task 不动训练 / parser / hybrid / search.rs / Tier 2 |
| §1.5 工具链前置 | Task 3 Step 1 + Task 4 隐式（脚本头部判定）|
| §2 边界（不影响 BETA-08 v1 已有 artifact） | Task 1 (rename 不动内容) + Task 3 (脚本不动现有 GGUF) |
| §3 文件布局 | 文件结构总览表 |
| §4 量化脚本 | Task 3 全部 step（代码块即 spec §4 内容）|
| §5 release notes 10 节 | Task 5 Step 3（10 节齐）|
| §6 实测指标采集 | Task 4 Step 6（grep 提取） |
| §7 sha256 + size 采集 | Task 5 Step 1（shasum + stat）|
| §8 风险 R1-R6 | Task 3 Step 1（R6 空间）+ Task 4 Step 2-5（R3/R4 量化失败）+ Task 5 Step 5（R5 漂移）|
| §9 验收清单 | Task 1-6 全部 step 覆盖 |
| §10 决策摘要 | spec 内化，plan 无需重复 |

**全部覆盖**。

**2. Placeholder scan**：

- Task 5 Step 3 release notes 模板内含 `<...>` 占位 — 这些是**运行后才填的实测数据**，plan 已明确 Step 4 把 Step 1/2/Task 4 Step 6 数据填入对应占位，Step 5 自检无残留。**非 plan 占位 bug，是预期模板**。
- 无 "TODO" / "TBD" / "implement later"。
- 每个 code/script step 均有具体 code block。

**3. Type consistency**：

- 变量名 `LLAMA_CPP` / `FUSED` / `GGUF_F16` / `BASELINE_JSON` / `out_gguf` / `out_log` 在 Task 3 脚本 + Task 4 验证 + Task 5 采集 全部一致
- 量化级别命名 `Q5_K_M` (label) / `q5_k_m` (suffix) 在 Task 3 / Task 4 / Task 5 全部一致
- 文件路径 `training/mlx-lora/fused/main-v1-{q5_k_m,q6_k}.gguf` 在 Task 3 / Task 4 / Task 5 全部一致
- release notes 路径 `training/mlx-lora/releases/v1.md` 在 Task 2 / Task 5 / STATUS 引用 全部一致

**无类型不一致**。
