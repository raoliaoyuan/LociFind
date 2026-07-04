# BETA-17 基座模型选型实验（bake-off）Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 v0.5-patch 数据集 + v1 同配方下，对新一代更小基座模型（Qwen3-0.6B/1.7B 主 + Qwen3.5-0.8B/2B 冲刺）做 bake-off，产出准确率/延迟对比报告 + 弱硬件默认推荐，不动运行时 wiring。

**Architecture:** 单一变量实验——只换基座，复用 `run_main_v1.sh` 的 7 步管线参数化为 `run_bakeoff.sh`。任何训练前先过 `smoke_candidate.sh` 工具链冒烟门（钉死 `llama-cpp-sys-4 0.3.0` 推理为硬门）。基线 Qwen2.5-1.5B 不重训，复用 v1 数字。non-thinking 天然成立（inference 走原始 prompt 不套 chat 模板，见 hybrid.rs），冒烟门经验性验证。

**Tech Stack:** mlx-lm 0.29.1（macOS Metal LoRA 训练）、llama.cpp CLI（`convert_hf_to_gguf.py` + `llama-quantize`）、`llama-cpp-sys-4 0.3.0`（Rust 推理，evals `--features model-fallback-metal`）、bash 管线脚本。

**前置约定（所有任务）：**
- 仓库根 = `/Users/alice/Work/LocalFind`，所有脚本从根运行。
- llama.cpp CLI 在 `$HOME/tools/llama.cpp`（`convert_hf_to_gguf.py` + `build/bin/llama-quantize`）。
- GGUF / safetensors / adapter 产物 **gitignore**（沿用 v1）；入库的只有脚本、报告、sha256 登记、STATUS/ROADMAP。
- 候选 mlx-community 仓库 id（冒烟门第 1 步实证确认，不存在则记跳过）：
  - Qwen3-0.6B → `mlx-community/Qwen3-0.6B-4bit`
  - Qwen3-1.7B → `mlx-community/Qwen3-1.7B-4bit`
  - Qwen3.5-0.8B → `mlx-community/Qwen3.5-0.8B-MLX-4bit`
  - Qwen3.5-2B → `mlx-community/Qwen3.5-2B-MLX-4bit`
- 每候选用 slug：`qwen3-0.6b` / `qwen3-1.7b` / `qwen3.5-0.8b` / `qwen3.5-2b`。

---

### Task 1: 工具链冒烟门脚本 `smoke_candidate.sh`

**Files:**
- Create: `training/mlx-lora/scripts/smoke_candidate.sh`

冒烟门验证一个候选的完整工具链：拉基座 → mlx 最小 LoRA → 转 GGUF → 钉死推理栈跑通产合法 JSON。任一步失败即非零退出，调用方据此把候选移出 bake-off。

- [ ] **Step 1: 写脚本**

```bash
#!/usr/bin/env bash
# BETA-17 工具链冒烟门：验证一个候选基座能被完整工具链消费。
# 用法: smoke_candidate.sh <mlx_repo_id> <slug>
# 例:   smoke_candidate.sh mlx-community/Qwen3-0.6B-4bit qwen3-0.6b
# 退出码 0=过门, 非0=某环节失败（stderr 记原因）。
set -euo pipefail

REPO_ID="${1:?需要 mlx repo id}"
SLUG="${2:?需要 slug}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
WORK="training/mlx-lora/smoke/$SLUG"
SMOKE_ADAPTER="$WORK/adapter"
SMOKE_FUSED="$WORK/fused-safetensors"
SMOKE_GGUF_F16="$WORK/$SLUG-f16.gguf"
SMOKE_GGUF_Q4="$WORK/$SLUG-q4_k_m.gguf"
mkdir -p "$WORK"

echo "==> [1/4] 拉基座 + 确认纯文本架构: $REPO_ID"
python3 - "$REPO_ID" <<'PY'
import sys
from huggingface_hub import snapshot_download
import json, pathlib
repo = sys.argv[1]
path = snapshot_download(repo)
cfg = json.loads((pathlib.Path(path) / "config.json").read_text())
arch = cfg.get("architectures", [])
# 排除多模态：含 vision/mmproj 字段即判失败
assert "vision_config" not in cfg, f"❌ {repo} 含 vision_config（多模态），跳过"
print(f"   架构={arch} 纯文本 OK，本地路径={path}")
PY

echo "==> [2/4] mlx-lm 最小 LoRA（4 step，验架构识别）"
python3 -m mlx_lm lora \
    --model "$REPO_ID" \
    --train \
    --data training/mlx-lora/data/smoke \
    --fine-tune-type lora \
    --num-layers 4 \
    --iters 4 \
    --batch-size 1 \
    --adapter-path "$SMOKE_ADAPTER" \
    --seed 42

echo "==> [3/4] fuse → GGUF → Q4_K_M"
python3 -m mlx_lm fuse \
    --model "$REPO_ID" \
    --adapter-path "$SMOKE_ADAPTER" \
    --save-path "$SMOKE_FUSED" \
    --dequantize
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" "$SMOKE_FUSED" --outfile "$SMOKE_GGUF_F16" --outtype f16
"$LLAMA_CPP/build/bin/llama-quantize" "$SMOKE_GGUF_F16" "$SMOKE_GGUF_Q4" Q4_K_M

echo "==> [4/4] 钉死 llama-cpp-sys-4 0.3.0 跑最小推理（产合法 JSON / 无 think 块）"
cargo build --release -p locifind-evals --bin smoke_infer --features model-fallback-metal 2>/dev/null || true
# smoke_infer 不存在时用 evals 单 case 探针替代（见 Task 2 备注）
LOCIFIND_MODEL_PATH="$SMOKE_GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals \
        --fixtures v0.5 \
        --with-fallback \
        --hybrid \
        --limit 5 \
        --json > "$WORK/smoke-evals.json"
# 校验：输出是合法 JSON 且不含 <think>
python3 - "$WORK/smoke-evals.json" <<'PY'
import sys, json
data = json.loads(open(sys.argv[1]).read())
raw = open(sys.argv[1]).read()
assert "<think>" not in raw, "❌ 输出含 <think> 块（thinking 未抑制）"
print(f"   ✅ 合法 JSON，无 think 块；fallback 样本通过")
PY

echo "✅ [$SLUG] 冒烟门通过"
```

> 备注：第 4 步若 evals 不支持 `--limit`，改用现有最小 fixture 跑全量但只取首批；或新增 `--limit` flag（见 Task 2）。`smoke/` 目录加入 gitignore。

- [ ] **Step 2: 确认 evals 支持 `--limit`，不支持则加**

Run: `grep -n "limit" packages/evals/src/bin/evals.rs`
Expected: 若无 `--limit` flag，在 evals.rs 的 clap Args 加 `#[arg(long)] limit: Option<usize>` 并在 case 迭代处 `.take(limit.unwrap_or(usize::MAX))`。加后 `cargo build --release -p locifind-evals --bin evals --features model-fallback-metal` 成功。

- [ ] **Step 3: smoke 数据存在性检查**

Run: `ls training/mlx-lora/data/smoke/train.jsonl training/mlx-lora/data/smoke/valid.jsonl`
Expected: 两文件存在（BETA-08 smoke 已建）。不存在则先跑 `python3 training/mlx-lora/scripts/prepare_smoke_data.py`。

- [ ] **Step 4: 加 gitignore + 提交脚本**

```bash
grep -q "training/mlx-lora/smoke/" .gitignore || echo "training/mlx-lora/smoke/" >> .gitignore
chmod +x training/mlx-lora/scripts/smoke_candidate.sh
git add training/mlx-lora/scripts/smoke_candidate.sh .gitignore packages/evals/src/bin/evals.rs
git commit -m "feat(beta-17): 工具链冒烟门脚本 smoke_candidate.sh + evals --limit"
```

---

### Task 2: 跑冒烟门，确定过门候选集

**Files:**
- Create: `docs/reviews/beta-17-base-model-bakeoff.md`（先建骨架，记冒烟门结果）

- [ ] **Step 1: 对四候选逐个跑冒烟门**

```bash
bash training/mlx-lora/scripts/smoke_candidate.sh mlx-community/Qwen3-0.6B-4bit     qwen3-0.6b   ; echo "EXIT=$?"
bash training/mlx-lora/scripts/smoke_candidate.sh mlx-community/Qwen3-1.7B-4bit     qwen3-1.7b   ; echo "EXIT=$?"
bash training/mlx-lora/scripts/smoke_candidate.sh mlx-community/Qwen3.5-0.8B-MLX-4bit qwen3.5-0.8b ; echo "EXIT=$?"
bash training/mlx-lora/scripts/smoke_candidate.sh mlx-community/Qwen3.5-2B-MLX-4bit   qwen3.5-2b   ; echo "EXIT=$?"
```

Expected: 每候选打印 `✅ [<slug>] 冒烟门通过` 且 `EXIT=0`，或在某步失败非零退出。**记录每个候选的 EXIT 码与失败步骤**。

> 关键决策点：若某候选第 4 步（钉死 0.3.0 推理）失败 → 该候选**移出 bake-off**，报告记「待工具链升级」。若 Qwen3.5 两个都卡且升级 llama-cpp-sys-4 超范围 → 本会话以 Qwen3 两候选闭合（符合 spec §4.1）。

- [ ] **Step 2: 写报告骨架，记冒烟门结果**

创建 `docs/reviews/beta-17-base-model-bakeoff.md`，含：标题/日期/关联 spec、§1 冒烟门结果表（候选 | mlx repo | 架构 | EXIT | 过门? | 失败步骤/原因）、§2 待填（指标对比表占位 = `（Task 4 填）`）、§3 待填（推荐结论占位）。**过门候选清单**写明，作为 Task 3 的输入。

- [ ] **Step 3: 提交报告骨架**

```bash
git add docs/reviews/beta-17-base-model-bakeoff.md
git commit -m "docs(beta-17): 冒烟门结果 + 报告骨架（过门候选集确定）"
```

---

### Task 3: 参数化 bake-off 管线 `run_bakeoff.sh`

**Files:**
- Create: `training/mlx-lora/scripts/run_bakeoff.sh`

把 `run_main_v1.sh` 的 7 步参数化为「输入 mlx repo id + slug，输出 adapter/GGUF + 双轨 evals JSON」。超参完全对齐 v1。

- [ ] **Step 1: 写脚本**

```bash
#!/usr/bin/env bash
# BETA-17 bake-off：对一个过门候选跑 v1 同配方训练 + 双轨 evals。
# 用法: run_bakeoff.sh <mlx_repo_id> <slug>
# 超参完全对齐 BETA-08 v1（run_main_v1.sh）：单一变量=只换基座。
set -euo pipefail

REPO_ID="${1:?需要 mlx repo id}"
SLUG="${2:?需要 slug}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

LLAMA_CPP="$HOME/tools/llama.cpp"
DATA_DIR="training/mlx-lora/data/main"
ADAPTER_DIR="training/mlx-lora/adapters/beta17-$SLUG"
SAFETENSORS_DIR="training/mlx-lora/fused/beta17-$SLUG-safetensors"
GGUF_F16="training/mlx-lora/fused/beta17-$SLUG-f16.gguf"
GGUF_Q4="training/mlx-lora/fused/beta17-$SLUG-q4_k_m.gguf"
BASELINE_JSON="training/mlx-lora/fused/beta17-$SLUG-baseline.json"
EVALS_LOG="training/mlx-lora/fused/beta17-$SLUG-evals.log"

echo "==> [1/7] prepare data（复用 v1 锁定数据集，零改动）"
python3 training/mlx-lora/scripts/prepare_main_data.py

echo "==> [2/7] mlx-lm lora train（v1 同配方：1000 step / 16 layers / batch 4 / lr 1e-4 / mask-prompt / oversample 8x）"
python3 -m mlx_lm lora \
    --model "$REPO_ID" --train --data "$DATA_DIR" \
    --fine-tune-type lora --num-layers 16 --iters 1000 \
    --batch-size 4 --learning-rate 1e-4 \
    --steps-per-report 50 --steps-per-eval 200 \
    --adapter-path "$ADAPTER_DIR" --mask-prompt --seed 42

echo "==> [3/7] fuse → HF safetensors"
python3 -m mlx_lm fuse --model "$REPO_ID" --adapter-path "$ADAPTER_DIR" \
    --save-path "$SAFETENSORS_DIR" --dequantize

echo "==> [4/7] convert_hf_to_gguf → fp16 GGUF"
python3 "$LLAMA_CPP/convert_hf_to_gguf.py" "$SAFETENSORS_DIR" --outfile "$GGUF_F16" --outtype f16

echo "==> [5/7] llama-quantize → Q4_K_M"
"$LLAMA_CPP/build/bin/llama-quantize" "$GGUF_F16" "$GGUF_Q4" Q4_K_M

echo "==> [6/7] parser-only baseline evals → JSON（应 472/26/2）"
cargo build --release -p locifind-evals --bin evals --features model-fallback-metal
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.5 --json > "$BASELINE_JSON"

echo "==> [7/7] with-fallback hybrid evals（model=$GGUF_Q4）+ 对 v1 diff"
LOCIFIND_MODEL_PATH="$GGUF_Q4" \
DYLD_LIBRARY_PATH="$ROOT/target/release" \
    ./target/release/evals --fixtures v0.5 --with-fallback --hybrid \
        --baseline "$BASELINE_JSON" 2>&1 | tee "$EVALS_LOG"

echo "==> GGUF 体积 + sha256"
ls -lh "$GGUF_Q4"
shasum -a 256 "$GGUF_Q4"
echo "✅ [$SLUG] bake-off run 完成"
```

- [ ] **Step 2: 提交脚本**

```bash
chmod +x training/mlx-lora/scripts/run_bakeoff.sh
git add training/mlx-lora/scripts/run_bakeoff.sh
git commit -m "feat(beta-17): 参数化 bake-off 管线 run_bakeoff.sh（v1 同配方）"
```

---

### Task 4: 逐候选跑训练 + 评测，采集指标

**Files:**
- 产物：`adapters/beta17-<slug>/`、`fused/beta17-<slug>-*.gguf`（gitignore）、evals log

- [ ] **Step 1: 对每个过门候选跑 bake-off（~47 min/候选）**

对 Task 2 确定的每个过门 slug：

```bash
bash training/mlx-lora/scripts/run_bakeoff.sh <mlx_repo_id> <slug> 2>&1 | tee /tmp/beta17-<slug>.log
```

Expected: 每候选打印 `✅ [<slug>] bake-off run 完成`。[6/7] parser-only **必须 472/26/2**（不破 parser）；[7/7] 打印 hybrid pass/partial/fail/rescued/regressed + 对 v1 的 diff。**记录每候选**：pass / partial / fail / rescued_to_pass / regressed / 字段精确匹配 / p50·p95 fallback 延迟 / 常驻内存（evals 输出）/ GGUF 体积 / sha256。

- [ ] **Step 2: 校验单一变量不变量**

Run: `grep -c "" training/mlx-lora/data/main/train.jsonl`（确认每次 prepare_main_data 产出行数一致）
Expected: 各候选共用同一份 train.jsonl（确定性 seed 42），证「只换基座」。若某候选 parser-only baseline ≠ 472/26/2，停下排查（wiring 被污染）。

---

### Task 5: 填报告 + 推荐结论

**Files:**
- Modify: `docs/reviews/beta-17-base-model-bakeoff.md`

- [ ] **Step 1: 填 §2 指标对比表**

把 Task 4 各候选 + v1 基线（复用 release notes：pass 480 / 字段 96.0% / Metal p95 1586ms / 940 MB）填入对比表，列：模型 | 参数量 | pass | partial | fail | rescued | regressed | 字段精度 | Metal p50/p95 | 内存 | GGUF 体积。

- [ ] **Step 2: 填 §3 推荐结论（按 spec §5 判定规则）**

应用判定：净降 ≤2 + regressed ≤2 = 对等；对等候选中取最小+最快推荐为弱硬件默认。覆盖三分支（小候选对等 / 仅大候选对等 / 无候选对等=维持 v1 负结果）。**明确标注「Metal 延迟仅相对排序，3000ms 绝对达标待 Windows（BETA-09a Intel 核显）复核」**。

- [ ] **Step 3: 填 §4 winner 产物登记（sha256 + 训练参数）**

winner（及对等候选）的 adapter 路径 / Q4_K_M GGUF sha256 / 训练超参 / 复现命令，沿用 `training/mlx-lora/releases/v1.md` 模式登记进报告（GGUF 本身 gitignore）。

- [ ] **Step 4: 提交报告**

```bash
git add docs/reviews/beta-17-base-model-bakeoff.md
git commit -m "docs(beta-17): bake-off 指标对比 + 推荐结论 + winner 产物登记"
```

---

### Task 6: 收工同步（STATUS / ROADMAP）

**Files:**
- Modify: `STATUS.md`、`ROADMAP.md`

- [ ] **Step 1: 更新 ROADMAP BETA-17 状态**

把 ROADMAP §3.3 B3 BETA-17 行状态从 `not_started` 改为 `done`（或 `partial — Mac 半完成，Windows 延迟复核 pending`，按实际），追加结论一句话 + 报告链接。若 Qwen3.5 被冒烟门挡掉，加 backlog「BETA-17 后续：Qwen3.5 待 llama-cpp-sys-4 升级」。

- [ ] **Step 2: 更新 STATUS（顶部当前 Task / 下一步 / 会话日志）**

按 CONVENTIONS §3：当前 Task 段更新、「下一步」加「BETA-17 winner Windows 弱硬件延迟复核」、会话日志顶部追加本会话条目（署名 `Claude Code (Opus 4.8)`，记冒烟门结果/各候选指标/推荐/Windows 待办）。

- [ ] **Step 3: 收工 commit + 向用户确认**

```bash
git add STATUS.md ROADMAP.md
git commit -m "收工: BETA-17 基座选型 bake-off（Mac 半）— <结论一句话>"
```

向用户报告：过门候选、各候选 pass/延迟、推荐结论、Windows 待办，确认提交内容。

---

## 自审记录（writing-plans self-review）

- **Spec 覆盖**：§3 候选→Task1/2 冒烟门；§4.1 冒烟门→Task1/2；§4.2 non-thinking→Task1 Step1 第4步经验性验证（已确认 inference 走原始 prompt 不套 chat 模板，天然 non-thinking）；§4.3 配方→Task3（v1 同参）；§4.4 评测→Task3/4 双轨；§5 判定→Task5 Step2；§6 交付→报告 Task2/5 + 脚本 Task1/3 + 收工 Task6；§7 风险 R1→冒烟门，R3 延迟→Task5 Step2 标注。全覆盖。
- **占位符**：报告 §2/§3 在 Task2 建骨架时标 `（Task 4/5 填）`，Task5 填实，非计划占位。
- **类型/命名一致**：slug 命名（qwen3-0.6b 等）贯穿 smoke/run_bakeoff/adapters/fused 路径；`--limit` flag 在 Task1 Step2 定义后 Task1 Step1 第4步使用（注：Step2 是前置确认，执行顺序上先做 Step2 再依赖）。
- **执行顺序提醒**：Task1 Step2（确认/加 `--limit`）应在 Task1 Step1 脚本实际运行前完成，因 smoke 第4步用到。
</content>
