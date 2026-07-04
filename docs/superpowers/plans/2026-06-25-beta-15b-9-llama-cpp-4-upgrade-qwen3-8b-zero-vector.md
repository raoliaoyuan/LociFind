# BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零向量 bug 排查 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 升 `llama-cpp-4 = 0.3.0` → `0.3.2`（latest patch、SemVer 兼容）+ 按 v4 baseline 报告 hypothesis 排序 systematic 排查 qwen3-embedding-8b 全零向量 bug、拿到真水位（或 file upstream issue 留追溯）、解 bake 决策的「同族最大档真水位未知」盲点。

**Architecture:** 早 exit hypothesis ladder：Phase 0 升 version + qwen3-0.6b vectors.json byte-equal 红线 → Phase 1 升 0.3.2 后重测 8b → Phase 2 fallback `gpu_layers=0` CPU 跑 → Phase 3 fallback `context_size=4096` → Phase 4 file upstream issue。任一 hypothesis win 即跑 9 阈值 sweep + 拿真水位、若全 fail 则 file issue + cycle 末报告。

**Tech Stack:** Rust 1.x / `cargo update -p llama-cpp-4 --precise 0.3.2` / `llama-cpp-4 0.3.0 → 0.3.2` / Mac Metal q8_0 推理 / superpowers subagent-driven workflow。

**Spec:** [docs/superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md](../specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md)

**Cycle 范围**（spec §3）：纯 llama-cpp-4 升级 + qwen3-8b 端到端验证。不动 `DEFAULT_EMBEDDING_MODEL_PATH` / `baseline.json` / `gate.rs` / desktop wiring。

**预估 cycle 时长**：0.5-1d（取决于 hypothesis 哪步 win + 是否需 file upstream issue）。

**Early-exit 路径决策图**：

```
Task 1 (Phase 0、必跑) → workspace 三件套 + qwen3-0.6b byte-equal 红线
  ↓ pass
Task 2 (Phase 1、必跑) → 升 0.3.2 后重测 8b health-check
  ├ WIN: Step 2.5 sweep + Step 2.6 commit → 跳 Task 5
  └ FAIL: Task 3
Task 3 (Phase 2-3、fail-only) → hypothesis 2 GPU=0 + hypothesis 3 context=4096
  ├ WIN: 回 Task 2 Step 2.5-2.6 跑 sweep + commit → 跳 Task 5
  └ FAIL: Task 4
Task 4 (Phase 4、fail-only) → file upstream issue + collect URL
  └ 跳 Task 5
Task 5 (Phase 5、必跑) → baseline v4-fixup2 + STATUS / ROADMAP doc-sync + PR + 合 main
```

---

## Task 1: Phase 0 — 升 llama-cpp-4 0.3.0 → 0.3.2 + qwen3-0.6b vectors.json byte-equal 升级回归红线

**Files:**
- Modify: `Cargo.lock`（`cargo update` 自动更新）

**说明**：cycle 入口 + 升级回归保护红线。升 version 后必须验证 qwen3-0.6b 重 embed 后 vectors.json byte-equal、确认升级行为透明、不引入未预期副作用。若 byte-equal 红线 fail → **BLOCKED 不进 Task 2**、调查根因（最可能：0.3.2 改了 default 参数 / GPU kernel 微调输出微差 → 考虑 revert 到 0.3.1 试、或回 0.3.0 调查）。

### Step 1.1: `cargo update` 升 llama-cpp-4 → 0.3.2

- [ ] 运行：

```bash
cd /Users/alice/Work/LocalFind
cargo update -p llama-cpp-4 --precise 0.3.2
```

**Expected**：Cargo.lock 改动（含 `llama-cpp-4 0.3.0 → 0.3.2` + 依赖传递 `llama-cpp-sys-4 0.3.0 → 0.3.2`）。`cargo update` 输出包含 `Updating llama-cpp-4 v0.3.0 -> v0.3.2`。

若 `cargo update` 报「failed to select a version」或 SemVer 不兼容 = `packages/model-runtime/Cargo.toml:20` 锁的 `version = "0.3.0"` 字面要求 0.3.x、应能升到 0.3.2；若 fail 给 coordinator 看错误信息。

### Step 1.2: workspace build + 三件套验证

- [ ] 运行（依次执行）：

```bash
cargo build -p locifind-model-runtime --features llama-cpp
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo fmt --all --check
```

**Expected**：build 成功 / workspace test 0 failed / clippy 两次 0 warning / fmt 净。

若 build / test / clippy 任一失败 → **BLOCKED**、可选 fallback：
- 先 revert 试 0.3.1 中间档：`cargo update -p llama-cpp-4 --precise 0.3.1` 再跑三件套
- 若 0.3.1 也 fail → 回 0.3.0 调查 + 报 coordinator

### Step 1.3: qwen3-0.6b vectors.json byte-equal 升级回归红线（spec §2.2 (7)）

> **关键回归门**：现 qwen3-embedding-0.6b GGUF 声明 `qwen3.pooling_type=3` → 经 BETA-15B-8 落地的 detect_model_pooling 解析 → `LlamaPoolingType::Last` → 升 0.3.2 后**应**与升级前 byte-equal（推理 deterministic、Metal kernel 同款）。若不等 = 升级引入未预期副作用、Branch IV 异常、**阻止 Task 2**。

- [ ] 备份升级前 vectors.json：

```bash
cp packages/evals/fixtures/semantic-recall/vectors.json /tmp/baseline-vectors-pre-0.3.2.json
sha256sum /tmp/baseline-vectors-pre-0.3.2.json
```

- [ ] 跑 qwen3-0.6b 重 embed（Mac Metal + release）：

```bash
cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
  --embed --model models/qwen3-embedding-0.6b-q8_0.gguf
```

注：`--vectors-file` 不带、走默认 `packages/evals/fixtures/semantic-recall/vectors.json`（BETA-15B-7 v4 cycle 已加该 flag、默认值即此路径）。

- [ ] 比对 byte-equal：

```bash
diff -q /tmp/baseline-vectors-pre-0.3.2.json packages/evals/fixtures/semantic-recall/vectors.json
sha256sum packages/evals/fixtures/semantic-recall/vectors.json
```

**Expected**：`diff -q` 无输出（exit 0 = byte-equal）+ SHA256 完全一致。

若 diff 有输出 = 升级引入了 vectors 内容微变、**不进 Task 2**、报 coordinator 看是否：
- 真异常（不该有的副作用）→ revert 0.3.2、试 0.3.1
- 可接受微差（如 Metal kernel 小调输出 1e-7 量级浮点偏差 / 序列化格式微调）→ coordinator 拍板是否接受 + 是否需调整 (7) 红线为 "L2 norm 等价" 而非 byte-equal

### Step 1.4: 还原工作树（vectors.json 实际不变、保险）

- [ ] 运行：

```bash
git checkout packages/evals/fixtures/semantic-recall/vectors.json
git status
```

**Expected**：仅 `Cargo.lock` 改动、其他文件 clean（vectors.json 字节等价、还原是保险动作）。

### Step 1.5: Commit Task 1

- [ ] 运行：

```bash
git add Cargo.lock
git commit -m "$(cat <<'EOF'
BETA-15B-9 task 1：升 llama-cpp-4 0.3.0 → 0.3.2 + qwen3-0.6b vectors.json byte-equal 升级回归红线过

承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项另一半 = qwen3-embedding-8b 全零向量 bug。本 task 是 hypothesis ladder Phase 0 = 升 llama-cpp-4 0.3.0 → 0.3.2（latest patch、SemVer 兼容）+ workspace 三件套验（cargo build / test / clippy 两次 / fmt 全过）+ qwen3-embedding-0.6b 重 embed 后 vectors.json SHA256 byte-equal（spec §2.2 (7) 升级回归保护红线、infra 升级行为透明硬证据、零回归）。Cargo.lock 包含 llama-cpp-4 + 依赖传递 llama-cpp-sys-4 升级。下 task 进 Phase 1 = hypothesis 1 重测 qwen3-8b。
EOF
)"
```

---

## Task 2: Phase 1 — hypothesis 1 升 0.3.2 后重测 qwen3-8b（必跑、决定后续路径）

**Files:**
- Modify (conditional, WIN only): `packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json`（v4 全零版被新版覆盖）

**说明**：升 version 是否直接修 8b 全零。**WIN** → 跑 sweep + Step 2.6 commit + 跳 Task 5；**FAIL** → 还原 vectors-qwen3-8b.json 不动 + 进 Task 3。

### Step 2.1: 备份 v4 全零版本 vectors-qwen3-8b.json

- [ ] 运行：

```bash
cp packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json /tmp/vectors-qwen3-8b-v4-zero.json
sha256sum /tmp/vectors-qwen3-8b-v4-zero.json
```

**Expected**：SHA256 = `b243e2a9c4d508abe9c0672eebf194b0b17872b27b6870c65dce91d49ad80989`（v4 cycle 入仓的全零版）。

### Step 2.2: qwen3-8b 重 embed（Mac Metal + release）

- [ ] 运行：

```bash
cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
  --embed --model models/qwen3-embedding-8b-q8_0.gguf \
  --vectors-file vectors-qwen3-8b.json
```

注：`--vectors-file` 用 basename（BETA-15B-8 cycle 验证：evals `fixt()` 会重新 join `packages/evals/fixtures/semantic-recall/`、传全路径触发 panic）。

**Expected**：
- WIN：文件被覆盖、命令 exit 0、运行时间 **10-90 min 量级**（8b 模型推理慢、202 序列、Mac Metal q8_0 单条 ~3-30s 量级）
- FAIL（v4 同款短路 bug）：~27s 早期短路、文件被覆盖但内容全零

注：若运行时间不符合 8b 推理量级（如 <60s）= 短路 bug 没修、走 FAIL 路径。

### Step 2.3: 健康性验证（Python、抽样）

- [ ] 运行：

```bash
python3 <<'PY'
import json, math
with open("packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json") as f:
    data = json.load(f)
def check(label, vec):
    nz = sum(1 for x in vec if x != 0)
    l2 = math.sqrt(sum(x*x for x in vec))
    print(f"  {label}: dim={len(vec)} nonzero={nz}/{len(vec)} L2={l2:.4f}")
print("=== qwen3-8b health check ===")
print(f"  top keys: {list(data.keys())[:5]}")
for k in data.keys():
    items = data[k] if isinstance(data[k], list) else []
    if items and isinstance(items[0], dict):
        for i, item in enumerate(items[:3]):
            for vk in ("vector", "embedding", "vec"):
                v = item.get(vk)
                if v is not None:
                    check(f"{k}[{i}].{vk}", v)
                    break
PY
```

**Expected**：
- WIN：所有抽样向量 dim=4096、nonzero ≥ 95%、L2 norm = 1.0000 ± 0.001
- FAIL（v4 同款）：dim=4096、nonzero = 0、L2 norm = 0

### Step 2.4: 路径分支判定

- [ ] 判定：

**WIN 路径**（健康性过）：
- 升 0.3.2 直接修 8b bug、hypothesis 1 证实
- 跳 Step 2.5 sweep + Step 2.6 commit
- Task 3、Task 4 全跳过、跳 Task 5

**FAIL 路径**（仍全零）：
- 还原 vectors-qwen3-8b.json：

```bash
cp /tmp/vectors-qwen3-8b-v4-zero.json packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
git diff --stat packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
# Expected: 无 diff（已还原）
```

- 转 Task 3

### Step 2.5: WIN 路径 — 9 阈值 sweep × qwen3-8b

- [ ] 运行：

```bash
mkdir -p /tmp/beta-15b-9-sweep
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== T=$t ===" | tee -a /tmp/beta-15b-9-sweep/qwen3-8b.log
  cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
    --vectors-file vectors-qwen3-8b.json \
    --semantic-weight 10.0 \
    --cosine-threshold $t \
    2>&1 | tee -a /tmp/beta-15b-9-sweep/qwen3-8b.log
done
```

（与 BETA-15B-8 Task 3 同款 sweep 命令、9 阈值 = `{0.0, 0.30, 0.45, 0.60, 0.70, 0.80, 0.90, 0.99, 1.01}`、`semantic-weight 10.0`。）

**Expected**：9 次运行、每次输出含 6 桶 nDCG（exact-name / synonym / concept / crosslang / content-not-name / OVERALL）的 FTS / VEC / HYB / HYBR 多组数据。控制对照：T=0.0 时 HYBR ≈ VEC、T=1.01 时 HYBR ≡ HYB。

- [ ] 提取 sweep 数据：

```bash
cat /tmp/beta-15b-9-sweep/qwen3-8b.log | grep -E "T=|OVERALL|exact-name|synonym|concept|crosslang|content-not-name|HYBR_N|VEC_N|HYB_N|HYBR_R" > /tmp/beta-15b-9-sweep/qwen3-8b-extracted.txt
echo "===== extracted lines count ====="
wc -l /tmp/beta-15b-9-sweep/qwen3-8b-extracted.txt
echo "===== head 100 ====="
head -100 /tmp/beta-15b-9-sweep/qwen3-8b-extracted.txt
```

Task 5 baseline 报告 v4-fixup2 节直接用这份提取。

### Step 2.6: WIN 路径 — Commit vectors-qwen3-8b.json

- [ ] 运行：

```bash
git add packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
git commit -m "$(cat <<'EOF'
BETA-15B-9 task 2：hypothesis 1 win + qwen3-8b 真水位 sweep + vectors-qwen3-8b.json 覆盖入仓

升 llama-cpp-4 0.3.0 → 0.3.2 后用 Mac Metal 重 embed qwen3-embedding-8b：v4 全零版本（llama-cpp-4 0.3.0 + 8B 推理早期短路 bug、202 序列全零、27s 异常短）被新版（0.3.2 修后健康向量、dim=4096、L2 ≈ 1.0、所有抽样 nonzero ≥ 95%）覆盖。9 阈值 sweep（cosine-threshold ∈ {0.0, 0.30, 0.45, 0.60, 0.70, 0.80, 0.90, 0.99, 1.01}、W=10.0）数据收集到 /tmp/beta-15b-9-sweep/qwen3-8b.log、Task 5 写进 baseline 报告 v4-fixup2 节。v4 cycle hypothesis 1（llama-cpp-4 0.3.0 bug）证实、跳过 hypothesis 2-3 + file upstream issue 排查。
EOF
)"
```

WIN 路径完成后跳 Task 5（Task 3 + Task 4 不进入）。

---

## Task 3: Phase 2-3 — hypothesis 2 (GPU=0) + hypothesis 3 (context=4096) fallback（仅 Task 2 FAIL 路径进入）

**Files:**
- Modify (temporary, NOT committed): `packages/model-runtime/src/llama.rs`（临时改 `ModelLoadParams` 字段默认值用于排查、test 后 git checkout 还原）
- Modify (conditional, WIN only): `packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json`（覆盖 v4 全零版）

**说明**：Task 2 hypothesis 1 fail 时进入。临时硬改 `gpu_layers=0`（Step 3.1）+ 临时硬改 `context_size=4096`（Step 3.2）各验一次。若任一 WIN 则还原临时改动 + 用 LociFind 默认参数（gpu_layers=99 / context_size=2048）跑 sweep + commit；若都 FAIL 转 Task 4。

### Step 3.1: hypothesis 2 — GPU layers=0 CPU 跑

- [ ] 查 `packages/model-runtime/src/llama.rs` 中 `LlamaModelImpl::spawn` 调用的 `model_params.with_n_gpu_layers(gpu_layers)`（约 line 220-223）。临时改 LlamaLoader::load 中 `gpu_layers` 传参 = 0。

最小侵入方案 = 改 [packages/model-runtime/src/llama.rs](packages/model-runtime/src/llama.rs) `LlamaLoader::load` 函数体中 `let runtime = LlamaModelImpl::spawn(self.backend.clone(), path, params.gpu_layers, context_size)?;` 这一行的 `params.gpu_layers` 临时改为 `0`：

```rust
// 临时（BETA-15B-9 hypothesis 2 排查、Step 3.3 还原）：硬改 gpu_layers=0 CPU 跑
let runtime = LlamaModelImpl::spawn(self.backend.clone(), path, 0, context_size)?;
```

- [ ] 跑 8b embed（CPU 模式、慢、可能 30min-2h）：

```bash
cargo build -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release
cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
  --embed --model models/qwen3-embedding-8b-q8_0.gguf \
  --vectors-file vectors-qwen3-8b.json
```

注：仍用 `--features semantic-recall-metal` 是让 build 含 metal feature（不影响 gpu_layers=0 的运行时配置）；运行时层 gpu_layers=0 走 CPU 路径。

- [ ] 健康性验证（与 Task 2 Step 2.3 同款 Python 脚本）。

**WIN 判定**：dim=4096、nonzero ≥ 95%、L2 ≈ 1.0
- 推断 GPU offload bug、记录在 Step 3.3 还原说明里
- **还原 gpu_layers 修改**（git checkout llama.rs）+ 用 gpu_layers=99 跑 Step 3.4 sweep + Step 3.5 commit

**FAIL 判定**：仍全零或异常
- 还原 gpu_layers 修改 → 转 Step 3.2 hypothesis 3

### Step 3.2: hypothesis 3 — context_size=4096

- [ ] 临时改 LlamaLoader::load 中 `context_size` 计算逻辑、把 2048 默认值改 4096：

查 `packages/model-runtime/src/llama.rs` `impl ModelLoader for LlamaLoader::load` 中 `let context_size = if params.context_size > 0 { params.context_size } else { 2048 };`（约 line 63-67）。临时改：

```rust
// 临时（BETA-15B-9 hypothesis 3 排查、Step 3.3 还原）：硬改 context_size=4096
let context_size = 4096;
```

- [ ] 跑 8b embed（默认 gpu_layers、Metal）：

```bash
cargo build -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release
cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
  --embed --model models/qwen3-embedding-8b-q8_0.gguf \
  --vectors-file vectors-qwen3-8b.json
```

- [ ] 健康性验证。

**WIN 判定**：
- 推断 context size 边界 bug
- 还原 context_size 修改（git checkout llama.rs）+ 用 context_size=2048 跑 Step 3.4 sweep + Step 3.5 commit

**FAIL 判定**：
- 还原 context_size 修改 → 转 Task 4 file upstream issue

### Step 3.3: 还原所有临时改动（FAIL 路径必做、WIN 路径跑 sweep 前必做）

- [ ] 运行：

```bash
git checkout packages/model-runtime/src/llama.rs
git status
```

**Expected**：工作树 clean（vectors-qwen3-8b.json 由 Task 2 Step 2.4 还原、本 task 不再动）。

注：WIN 路径下、临时改动还原后用 LociFind **默认参数**重跑 Step 3.4 sweep。这样 vectors-qwen3-8b.json 入仓的是「默认参数 + hypothesis X infra 修复」版本、未来 release 直接复用。

### Step 3.4: WIN 路径 — qwen3-8b 默认参数重 embed + 9 阈值 sweep

- [ ] 用默认参数（gpu_layers=99 / context_size=2048）重 embed 8b：

```bash
cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
  --embed --model models/qwen3-embedding-8b-q8_0.gguf \
  --vectors-file vectors-qwen3-8b.json
```

- [ ] 健康性验证（仍应 nonzero / L2≈1、确认默认参数下 win）。

- [ ] 跑 9 阈值 sweep（与 Task 2 Step 2.5 同款命令、保存到同款 log 路径供 Task 5 用）：

```bash
mkdir -p /tmp/beta-15b-9-sweep
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== T=$t ===" | tee -a /tmp/beta-15b-9-sweep/qwen3-8b.log
  cargo run -p locifind-evals --features semantic-recall-metal --bin semantic_quality --release -- \
    --vectors-file vectors-qwen3-8b.json \
    --semantic-weight 10.0 \
    --cosine-threshold $t \
    2>&1 | tee -a /tmp/beta-15b-9-sweep/qwen3-8b.log
done

cat /tmp/beta-15b-9-sweep/qwen3-8b.log | grep -E "T=|OVERALL|exact-name|synonym|concept|crosslang|content-not-name|HYBR_N|VEC_N|HYB_N|HYBR_R" > /tmp/beta-15b-9-sweep/qwen3-8b-extracted.txt
```

### Step 3.5: WIN 路径 — Commit vectors-qwen3-8b.json

- [ ] 运行：

```bash
git add packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
git commit -m "$(cat <<'EOF'
BETA-15B-9 task 3：hypothesis 2/3 win + qwen3-8b 真水位 sweep + vectors-qwen3-8b.json 覆盖入仓

[按实际 win 是 hypothesis 2 (GPU=0) 还是 hypothesis 3 (context=4096) 填具体路径]：升 llama-cpp-4 0.3.2 后 hypothesis 1 仍 fail（默认参数 gpu_layers=99 + context_size=2048 下 8b 仍全零）；临时排查 [hypothesis X 配置变更] 后 8b 健康向量出（dim=4096、L2≈1.0、nonzero≥95%）；还原临时改动用 LociFind 默认参数重 embed + 9 阈值 sweep（cosine-threshold ∈ {0.0..1.01}、W=10.0）；vectors-qwen3-8b.json 覆盖 v4 全零版入仓。v4 cycle hypothesis [X] 证实、推断 [GPU offload bug / context size 边界 bug]。注：本 task FAIL 路径下默认参数仍触发原 bug、本 cycle 无生产侧修复（生产仍用 0.6b、不走 8b）；BETA-15B-7-v2 bake 决策若选 8b 需 follow-up cycle 解 infra 默认参数兼容性问题。
EOF
)"
```

WIN 路径完成后跳 Task 5。

---

## Task 4: Phase 4 — file upstream issue（仅 Task 2 + Task 3 全 FAIL 路径进入）

**Files:**
- Create: `/tmp/beta-15b-9-upstream-issue.md`（issue body 模板、不入仓）

**说明**：4 hypothesis 全 fail 时、确定是 llama-cpp-4 上游 bug、file GitHub issue 留追溯。本 task 无本仓改动 / 无 commit、仅外部 issue。

### Step 4.1: 准备 GGUF metadata dump（issue body 引用）

- [ ] 运行（与 BETA-15B-8 cycle explore phase 同款 Python parser）：

```bash
python3 <<'PY' | tee /tmp/beta-15b-9-qwen3-8b-metadata.txt
import struct
GGUF_TYPE = {0:"u8",1:"i8",2:"u16",3:"i16",4:"u32",5:"i32",6:"f32",7:"bool",8:"str",9:"array",10:"u64",11:"i64",12:"f64"}
def read_str(f):
    n = struct.unpack("<Q", f.read(8))[0]
    return f.read(n).decode("utf-8", "replace")
def read_val(f, t):
    if t==4: return struct.unpack("<I", f.read(4))[0]
    if t==5: return struct.unpack("<i", f.read(4))[0]
    if t==6: return struct.unpack("<f", f.read(4))[0]
    if t==7: return f.read(1)[0]!=0
    if t==8: return read_str(f)
    if t==10: return struct.unpack("<Q", f.read(8))[0]
    if t==11: return struct.unpack("<q", f.read(8))[0]
    if t==12: return struct.unpack("<d", f.read(8))[0]
    if t==0: return f.read(1)[0]
    if t==1: return struct.unpack("<b", f.read(1))[0]
    if t==2: return struct.unpack("<H", f.read(2))[0]
    if t==3: return struct.unpack("<h", f.read(2))[0]
    if t==9:
        at = struct.unpack("<I", f.read(4))[0]
        n  = struct.unpack("<Q", f.read(8))[0]
        return f"<array[{n}] of {GGUF_TYPE.get(at,'?')}>"
def dump(path):
    print(f"=== {path} ===")
    with open(path, "rb") as f:
        magic = f.read(4); ver = struct.unpack("<I", f.read(4))[0]
        nt  = struct.unpack("<Q", f.read(8))[0]; nk  = struct.unpack("<Q", f.read(8))[0]
        print(f"  magic={magic!r} v{ver} tensors={nt} kv={nk}")
        for i in range(nk):
            try:
                k = read_str(f); t = struct.unpack("<I", f.read(4))[0]; v = read_val(f, t)
            except Exception as e:
                print(f"  [{i}] PARSE_ERR: {e}"); break
            vs = repr(v) if isinstance(v,str) and len(v)<80 else str(v)[:120]
            print(f"  [{i}] {k} :{GGUF_TYPE.get(t,'?')}= {vs}")
dump("models/qwen3-embedding-8b-q8_0.gguf")
PY
```

### Step 4.2: 准备复现脚本 + 环境信息

- [ ] 写 issue body：

```bash
cat > /tmp/beta-15b-9-upstream-issue.md <<'EOF'
# qwen3-embedding-8b GGUF returns all-zero vectors via llama-cpp-4 0.3.2 on Mac Metal

## Summary

`qwen3-embedding-8b` (q8_0 GGUF) embedding produces all-zero vectors via `llama-cpp-4 0.3.2` (also reproduced on 0.3.0) on Mac Metal. The smaller `qwen3-embedding-0.6b` (q8_0 GGUF) works fine via the same code path.

Tested combinations (all reproduce 8b all-zero, all leave 0.6b working):
- `gpu_layers=99` (Mac Metal default) + `context_size=2048`
- `gpu_layers=0` (CPU) + `context_size=2048`
- `gpu_layers=99` + `context_size=4096`

Total inference time for 8b: ~27 seconds for 202 sequences = early-exit short circuit (8b should take 10–60min with 4096-dim embeddings).

## Environment

- macOS 25.5.0 (Apple M5 Pro)
- `llama-cpp-4 = "0.3.2"` with `features = ["metal"]`
- `default-features = false` (no `dynamic-link`)
- Upstream llama.cpp commit: `94a220cd6` (per `llama-cpp-4 0.3.1` README; 0.3.2 should be near)

## Models

| Model | Size | Works? |
|---|---|---|
| `Qwen/Qwen3-Embedding-0.6B-GGUF` q8_0 | 610 MB | ✅ Healthy vectors, L2≈1 |
| `Qwen/Qwen3-Embedding-8B-GGUF` q8_0 | 7.5 GB | ❌ All-zero vectors, L2=0 |

GGUF metadata for 8b model (from upload-checked file, SHA256 `a48e5033...`):

```
[see /tmp/beta-15b-9-qwen3-8b-metadata.txt]
```

Key fields:
- `general.architecture = "qwen3"`
- `qwen3.embedding_length = 4096`
- `qwen3.pooling_type = 3` (Last, per LLAMA_POOLING_TYPE_LAST)
- tensors = 398, kv_pairs = 36

## Reproduce (minimal Rust)

```rust
use llama_cpp_4::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = LlamaBackend::init()?;
    let model_params = LlamaModelParams::default().with_n_gpu_layers(99);
    let model = LlamaModel::load_from_file(
        &backend,
        Path::new("models/qwen3-embedding-8b-q8_0.gguf"),
        &model_params,
    )?;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(std::num::NonZeroU32::new(2048).unwrap()))
        .with_embeddings(true);
    let mut ctx = model.new_context(&backend, ctx_params)?;
    let tokens = model.str_to_token("hello world", AddBos::Always)?;
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    for (i, &t) in tokens.iter().enumerate() {
        batch.add(t, i as i32, &[0], true)?;
    }
    ctx.decode(&mut batch)?;
    let emb = ctx.embeddings_seq_ith(0)?.to_vec();
    println!("dim={}, nonzero={}/{}, L2={}",
        emb.len(),
        emb.iter().filter(|&&x| x != 0.0).count(),
        emb.len(),
        emb.iter().map(|x| x * x).sum::<f32>().sqrt(),
    );
    Ok(())
}
```

Expected (works for 0.6b, fails for 8b):
- 0.6b: `dim=1024, nonzero=1024/1024, L2=1.000...`
- 8b: `dim=4096, nonzero=0/4096, L2=0` ← **bug**

## Hypotheses tested (all rejected)

1. ❌ Patch bump 0.3.0 → 0.3.2 alone
2. ❌ `gpu_layers=99` → `gpu_layers=0` (CPU mode)
3. ❌ `context_size=2048` → `context_size=4096`

## Open hypothesis

4. ❓ 8B model has special layer structure (Gated Delta Net, sliding-window attention, etc.) not supported in current `llama-cpp-sys-4` binding? GGUF metadata doesn't surface obvious markers (architecture is plain `qwen3`, same as 0.6b), but 8b has 398 tensors vs 0.6b's 310.

Looking for guidance:
- Known issue with qwen3-embedding-8b in llama.cpp upstream `94a220cd6`?
- Required `llama-cpp-sys-4` config tweak we're missing?
- Upstream llama.cpp version bump that would address this?
EOF
```

### Step 4.3: file GitHub issue

- [ ] 方案 A：gh CLI

```bash
gh issue create \
  --repo eugenehp/llama-cpp-rs \
  --title "qwen3-embedding-8b GGUF returns all-zero vectors via llama-cpp-4 0.3.2 on Mac Metal" \
  --body-file /tmp/beta-15b-9-upstream-issue.md
```

- [ ] 方案 B fallback（gh 401 / 不可用）：手动浏览器开

```bash
open "https://github.com/eugenehp/llama-cpp-rs/issues/new"
# 然后从 /tmp/beta-15b-9-upstream-issue.md 复制粘贴
cat /tmp/beta-15b-9-upstream-issue.md
```

- [ ] 收集 issue URL（例：`https://github.com/eugenehp/llama-cpp-rs/issues/NNN`）、保存供 Task 5 baseline 报告引用：

```bash
echo "https://github.com/eugenehp/llama-cpp-rs/issues/NNN" > /tmp/beta-15b-9-upstream-issue-url.txt
```

注：本 task **无 commit**（仅 file 外部 issue、本仓无改动）。

---

## Task 5: Phase 5 — cycle 收口（baseline v4-fixup2 + STATUS / ROADMAP doc-sync + PR + 合 main）

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（追加 v4-fixup2 节）
- Modify: `STATUS.md`（当前 Task / 下一步 / 会话日志顶部追加）
- Modify: `ROADMAP.md`（BETA-15B-9 task 卡片登记 + 状态 done）
- Add to commit: `docs/superpowers/specs/2026-06-25-beta-15b-9-...-design.md` + `docs/superpowers/plans/2026-06-25-beta-15b-9-...-md`（cycle 末统一入仓、与 BETA-15B-8 同款）

### Step 5.1: 总验收 5 项全过

- [ ] 运行：

```bash
cd /Users/alice/Work/LocalFind
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo test --workspace
cargo test -p locifind-evals --test semantic_quality_gate
```

**Expected**：fmt 净 / clippy 两次 0 warning / workspace test 0 failed / semantic_quality_gate 1 passed（baseline.json 未动）。

### Step 5.2: vectors 文件状态验证（spec §2.2 (7)(8)(9)(10)）

- [ ] 运行：

```bash
sha256sum packages/evals/fixtures/semantic-recall/vectors.json
sha256sum packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json
sha256sum packages/evals/fixtures/semantic-recall/vectors-bge-m3.json
sha256sum packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json

# main 5305ee1 基准（BETA-15B-8 merge commit、本 cycle 起点）
git show 5305ee1:packages/evals/fixtures/semantic-recall/vectors.json | sha256sum
git show 5305ee1:packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json | sha256sum
git show 5305ee1:packages/evals/fixtures/semantic-recall/vectors-bge-m3.json | sha256sum
git show 5305ee1:packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json | sha256sum

git diff --stat main..HEAD packages/evals/fixtures/semantic-recall/
```

**Expected**：
- (7) `vectors.json` SHA256 == main 5305ee1（字节不动）
- (8) `vectors-qwen3-0.6b.json` SHA256 == main 5305ee1（字节不动）
- (9) `vectors-bge-m3.json` SHA256 == main 5305ee1（字节不动）
- (10) `vectors-qwen3-8b.json`：
  - WIN 路径：SHA256 != main 5305ee1（v4 全零 `b243e2a9...` → 新非零 SHA256）
  - FAIL 路径：SHA256 == main 5305ee1（仍 `b243e2a9...`、cycle 间状态一致）

若 (7)(8)(9) 任一不等 = Branch IV 异常、阻止合并、回头排查（最可能：某 task 误把别的模型 embed 写到了错的文件）。

### Step 5.3: 写 baseline 报告 v4-fixup2 节（按 cycle 实际路径走）

- [ ] 编辑 `docs/reviews/semantic-recall-quality-baseline.md`、在 v4-fixup 节末尾之后追加新节（用 spec §4.3 模板）。

**WIN 路径模板**（任一 hypothesis win、有 sweep 数据）：

```markdown
### v4-fixup2 数据集节 — llama-cpp-4 升级 + qwen3-embedding-8b 真水位（BETA-15B-9）

承接 v4 cycle 的 Branch IV-B 推断（qwen3-embedding-8b 全零向量、推断 llama-cpp-4 0.3.0 + 8B 推理 bug）。BETA-15B-9 cycle 升 llama-cpp-4 0.3.0 → 0.3.2 + 按 v4 hypothesis 排序排查：

- **Phase 0**：升 0.3.0 → 0.3.2、workspace 三件套全过、qwen3-0.6b vectors.json SHA256 byte-equal（升级回归保护硬证据）
- **Phase 1 (hypothesis 1)**：[按实测填 WIN / FAIL] 升 0.3.2 直接修 8b bug
- **Phase 2 (hypothesis 2)**：[按实测填路径走没走 + WIN / FAIL]
- **Phase 3 (hypothesis 3)**：[按实测填路径走没走 + WIN / FAIL]

**qwen3-8b 真水位 sweep**（v3 数据集 78 cases / 124 docs / dim 4096、默认参数 gpu_layers=99 / context_size=2048、W=10.0）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| [按 /tmp/beta-15b-9-sweep/qwen3-8b-extracted.txt 数据 9 行填] | ... |

**控制对照核验**：T=0.0 时 HYBR_OVERALL=VEC_OVERALL ✓；T=1.01 时 HYBR_OVERALL=HYB_OVERALL ✓。

**三方对照（v3 cases、W=10.0）**：

| 指标 | qwen3-0.6b T\*=0.70（生产锚）| bge-m3 best (BETA-15B-8) | **qwen3-8b best (本 cycle)** | 推荐 bake |
|---|---|---|---|---|
| OVERALL | 0.856 | 0.869 | [填] | [按实测拍] |
| crosslang | 0.717 | 0.685 | [填] | [按实测拍] |
| content-not-name | 0.870 | 0.875 | [填] | [按实测拍] |
| exact-name | 1.000 | 1.000 | [填] | [按实测拍] |
| 分发成本（GGUF q8_0）| 610 MB | 605 MB | 7.5 GB | [按 bake 拍] |

**bake 推荐意见**（按数据走、给 BETA-15B-7-v2 follow-up cycle 决策依据）：

- 若 qwen3-8b OVERALL > bge-m3 + crosslang ≥ qwen3-0.6b：**推荐 bake qwen3-8b**（无 trade-off）；需接受 7.5 GB 分发成本（首次启动下载 / 进度条 / 离线 fallback UX）
- 若 qwen3-8b 全桶 ≤ bge-m3 / qwen3-0.6b：**仍推荐 bake bge-m3**（同尺寸零分发成本、OVERALL +0.013 vs 0.6b、crosslang -0.032 trade-off 接受）
- 若 qwen3-8b 部分桶胜部分输：**bake 决策仍为 trade-off**，由用户拍板

**下 cycle 抓手优先级修正（v4-fixup2 数据指证）**：

[按实测三方对照填、bake 候选 + 备选路径]

**链接**：[BETA-15B-9 spec](../superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md) / [BETA-15B-9 plan](../superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md) / [v4-fixup 节 (bge-m3)](#v4-fixup-数据集节--model-runtime-pooling-type-detection-修复后-bge-m3-真水位beta-15b-8)
```

**FAIL 路径模板**（4 hypothesis 全 fail、file issue）：

```markdown
### v4-fixup2 数据集节 — llama-cpp-4 升级 + qwen3-embedding-8b 全零 bug 上游 issue（BETA-15B-9）

承接 v4 cycle 的 Branch IV-B 推断（qwen3-embedding-8b 全零向量）。BETA-15B-9 cycle 升 llama-cpp-4 0.3.0 → 0.3.2 + 按 v4 hypothesis 排序排查、4 hypothesis 全 FAIL、已 file upstream issue。

- **Phase 0**：升 0.3.0 → 0.3.2、workspace 三件套全过、qwen3-0.6b vectors.json SHA256 byte-equal ✓
- **Phase 1 (hypothesis 1)**：升 0.3.2 重测 8b → 仍全零（27s 早期短路、L2=0）
- **Phase 2 (hypothesis 2)**：临时 `gpu_layers=0` CPU 跑 → 仍全零
- **Phase 3 (hypothesis 3)**：临时 `context_size=4096` 跑 → 仍全零
- **Phase 4**：file upstream issue → [URL]

**upstream issue 摘要**：见 [URL 链接]。关键开放 hypothesis：8B 模型可能含 llama-cpp-sys-4 binding 不支持的特殊层结构（Gated Delta Net / SWA）、GGUF metadata 未直接看到（架构标识与 0.6b 同为 `qwen3`、仅 tensors 398 vs 310）。

**bake 决策**：qwen3-8b 真水位仍未知、本 cycle 无新数据让 bake 决策从 BETA-15B-8 状态改变：
- 仍推荐 bake bge-m3（同尺寸零分发成本、OVERALL +0.013 vs 0.6b、crosslang -0.032 trade-off 接受）
- qwen3-8b 评估推到上游 fix 后的 follow-up cycle（监控 upstream issue 进展）

**下 cycle 抓手优先级修正（v4-fixup2 fail 路径）**：

| 抓手 | 优先级 |
|---|---|
| BETA-15B-7-v2 bake bge-m3 推到生产 | 最高优（不再等 qwen3-8b 真水位、上游 fix 半年没回应不阻塞 bake）|
| 监控 llama-cpp-4 upstream issue | 中优（被动等、不主动推） |
| 跨厂替代候选（bge-multilingual-gemma2 9B / EmbeddingGemma-300M）| 低优（bge-m3 已部分破局）|
| 评测扩量 | 低优 |

**链接**：[BETA-15B-9 spec](../superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md) / [BETA-15B-9 plan](../superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md) / [upstream issue URL] / [v4-fixup 节 (bge-m3)](#v4-fixup-数据集节--model-runtime-pooling-type-detection-修复后-bge-m3-真水位beta-15b-8)
```

按 cycle 实际走的路径选 WIN / FAIL 模板填、不预设结论。

### Step 5.4: STATUS.md doc-sync

按 BETA-15B-8 同款模板（参 [STATUS.md](STATUS.md) 当前 Task / 会话日志节）。

**(a) 「当前阶段」节**：在 BETA-15B-8 后追加「**BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查 done**（[按 win/fail 路径填一句结论]）」。

**(b) 「当前 Task」节**：整段替换为 BETA-15B-9 done 摘要（按 cycle 实际路径含「做了什么」+「hypothesis 路径」+「结论」+「下 cycle 抓手优先级修正」+「未尽事宜」五段）。

**(c) 「会话日志」节顶部追加新条**：

```markdown
### 2026-06-25 — Claude Code (Opus 4.7) — BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查 done + PR #<编号> 已合 main（merge commit `<sha>`）⭐ [按 win/fail 加一句]

**承接**：BETA-15B-8 cycle merge 后用户「按推荐路径走」→ 启动 v4 数据指证次高优抓手 = 解 qwen3-8b 全零（前置 bake 决策的「同族最大档真水位未知」盲点）。完整 superpowers 全流程：brainstorming Q1-Q2 → spec → plan 5 task → subagent-driven 驱动 + 每 task 双审 + final integration review。

**关键决策（brainstorming Q1-Q2 收敛）**：① 范围 = 4-hypothesis 完整 systematic 排查（不只升 version、不补 4b 中间档）；② 不含 bake（与 BETA-15B-8 同款节奏、留 BETA-15B-7-v2）。

**执行路径**：[按实际走的 Phase 0 → 1 (win/fail) → 2-3 (if needed) → 4 (if needed) → 5 路径填]

**产出（5 task / [N] commits）**：
- T1 (`<sha>`) Phase 0 升 llama-cpp-4 0.3.0 → 0.3.2 + qwen3-0.6b vectors.json byte-equal 红线过
- T2 (`<sha>`) Phase 1 hypothesis 1 [win/fail][按实测填 sweep / 还原]
[按路径补 T3 / T4 / T5 commits]

**qwen3-8b 真水位 vs bge-m3 vs qwen3-0.6b 三方对照**（仅 WIN 路径填、FAIL 路径写「8b 仍未知、推到上游 fix 后 follow-up cycle」）：[按实测填]

**bake 推荐**：[按实测拍 bge-m3 / qwen3-8b / trade-off 由用户拍板]

**下 cycle 抓手优先级修正**：[按 win/fail 路径填]

**未尽事宜**：① 真机手测：纯 infra 修复 + 评测端到端覆盖、按 spec §7 判平凡未安排桌面手测剧本；② [FAIL 路径] 监控 upstream issue 进展、follow-up cycle 必要时跳转；③ bake 决策由用户拍板下 cycle 启动。
```

按 cycle 实际数据填、不预设结论。

### Step 5.5: ROADMAP.md doc-sync

- [ ] 编辑 `ROADMAP.md` §3.3 BETA-15B 行（BETA-15B-8 卡片描述之后追加 BETA-15B-9 卡片）。

参 BETA-15B-8 卡片同款风格、按 cycle 实际 win / fail 路径填：

```markdown
**BETA-15B-9 llama-cpp-4 升级 / qwen3-embedding-8b 全零 bug 排查 done（2026-06-25 Claude Code、PR #<编号>、merge commit `<sha>`）⭐ [按 win/fail 一句]**——承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项另一半。升 llama-cpp-4 0.3.0 → 0.3.2（latest patch、SemVer 兼容、`Cargo.lock` 含 llama-cpp-sys-4 依赖传递）+ 按 v4 hypothesis 排序排查（Phase 0 升级 + qwen3-0.6b vectors.json byte-equal 红线 → Phase 1 升 0.3.2 重测 → Phase 2 GPU=0 fallback → Phase 3 context=4096 fallback → Phase 4 file upstream issue）。[按 cycle 实际路径填详情]。**下 cycle 抓手 = BETA-15B-7-v2 bake [bge-m3 / qwen3-8b]（按实测拍）**。[BETA-15B-9 spec](docs/superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md) / [BETA-15B-9 plan](docs/superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md) / [baseline 报告 v4-fixup2 节](docs/reviews/semantic-recall-quality-baseline.md)
```

### Step 5.6: cycle 末 commit（含 spec / plan / baseline v4-fixup2 / STATUS / ROADMAP）

- [ ] 运行：

```bash
git add docs/reviews/semantic-recall-quality-baseline.md \
        docs/superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md \
        docs/superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md \
        STATUS.md ROADMAP.md

git status  # 验确认只动这 5 文件

git commit -m "$(cat <<'EOF'
BETA-15B-9 task 5：cycle 收口 + baseline 报告追加 v4-fixup2 节 + STATUS/ROADMAP doc-sync + PR 合 main

承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项另一半 = qwen3-embedding-8b 全零向量 bug。完整 superpowers 全流程（brainstorming Q1-Q2 → spec → plan 5 task → subagent-driven 驱动 + 每 task 双审 + final integration review）。[按实际路径填：执行了 Phase 0-X、hypothesis X win / 全 fail file upstream issue、qwen3-8b 真水位拿出 / 上游 issue 留追溯、bake 推荐意见]。本 commit 含本 cycle spec / plan 作 cycle 文献 + baseline 报告 v4-fixup2 节 + STATUS 当前 Task / 会话日志 / ROADMAP §3.3 BETA-15B-9 task 卡片 done 登记。验证：workspace test 0 failed、clippy 0、fmt 净、semantic_quality_gate 1 passed（baseline.json 未动）、evals parser byte-equal v0.5=473/v0.9=877 精确不变、vectors.json/vectors-qwen3-0.6b.json/vectors-bge-m3.json 三文件 SHA256 与 main 5305ee1 完全等价、vectors-qwen3-8b.json [WIN 路径：新非零版覆盖 v4 全零 / FAIL 路径：仍 v4 全零版]。
EOF
)"
```

### Step 5.7: PR + 合 main

按 LociFind 历史 cycle 惯例（参 BETA-15B-8 v8 cycle PR #13 流程）：

```bash
git push -u origin feat-beta-15b-9-llama-cpp-4-upgrade

# 方案 A: gh CLI
gh pr create --title "BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查" \
  --body "$(cat <<'EOF'
## Summary
BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项另一半 = 修 qwen3-embedding-8b 全零向量 bug、解 bake 决策「同族最大档真水位未知」盲点。

升 llama-cpp-4 0.3.0 → 0.3.2 + 按 v4 baseline 报告 hypothesis 排序 systematic 排查（4 hypothesis ladder + early-exit）。[按实际 cycle 路径填结论]。

详 [spec](docs/superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md) / [plan](docs/superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md) / [baseline 报告 v4-fixup2 节](docs/reviews/semantic-recall-quality-baseline.md)。

## Test Plan
- [x] cargo test --workspace 0 failed
- [x] cargo clippy --workspace + cargo clippy -p locifind-model-runtime --features llama-cpp 0 warning
- [x] cargo fmt --all --check 净
- [x] cargo test -p locifind-evals --test semantic_quality_gate 1 passed (baseline.json unchanged)
- [x] evals parser-only byte-equal v0.5=473 / v0.9=877 精确不变
- [x] vectors.json / vectors-qwen3-0.6b.json / vectors-bge-m3.json SHA256 与 main 5305ee1 完全等价（零回归硬证据）
- [x] vectors-qwen3-8b.json [WIN: 新非零版入仓 / FAIL: 仍 v4 全零版]

## 下 cycle 抓手
[按 v4-fixup2 数据指证填]
EOF
)"

# 方案 B fallback（gh CLI 401 凭据）：本地 merge
# 参 BETA-15B-8 v8 PR #13 同款流程（git checkout main + git pull + git merge --no-ff + git push origin main + git branch -d + git push origin --delete）
```

报告 PR 编号 / merge commit SHA / 选用方案（A 或 B）。

### Step 5.8: 收工

向 coordinator 报告 cycle done、含：
- PR 编号 / merge commit SHA
- 执行路径（哪步 win / 哪步 fail / 是否 file upstream issue）
- qwen3-8b 真水位摘要（win 路径有 / fail 路径无）
- bake 推荐
- 下 cycle 抓手建议
- 未尽事宜

---

## Self-Review

**Spec coverage check** — spec §2.1 目标 vs task：
1. ✅ cargo update 升 0.3.2 + workspace 三件套 → Task 1 Step 1.1-1.2
2. ✅ qwen3-0.6b 重 embed byte-equal 升级回归红线 → Task 1 Step 1.3-1.4
3. ✅ 4 hypothesis 排查（升级 / GPU=0 / context=4096 / file issue）→ Task 2 + Task 3 + Task 4
4. ✅ win 路径 9 阈值 sweep → Task 2 Step 2.5 / Task 3 Step 3.4
5. ✅ vectors-qwen3-8b.json 覆盖入仓（win）或不动（fail）→ Task 2 Step 2.6 / Task 3 Step 3.5 / Task 5 Step 5.2 验证
6. ✅ baseline v4-fixup2 节 → Task 5 Step 5.3
7. ✅ STATUS / ROADMAP doc-sync → Task 5 Step 5.4-5.5

**spec §2.2 验收 12 红线**：
- (1)(2)(3)(4) cargo test/clippy/fmt → Task 1 Step 1.2 + Task 5 Step 5.1
- (5) gate.rs baseline.json 不动 → Task 5 Step 5.1
- (6) evals parser byte-equal → 本 cycle 不动 parser、自然守住、Task 5 Step 5.1 workspace test 含 parser_eval
- (7) vectors.json byte-equal → Task 1 Step 1.3 + Task 5 Step 5.2
- (8) vectors-qwen3-0.6b.json 不动 → Task 5 Step 5.2
- (9) vectors-bge-m3.json 不动 → Task 5 Step 5.2
- (10) vectors-qwen3-8b.json win/fail 双路径 → Task 2 Step 2.6 (win) / Task 2 Step 2.4 (fail 还原) / Task 5 Step 5.2 验证
- (11) Cargo.lock 必变 → Task 1 Step 1.1 + 1.5
- (12) Mac Metal 真用 → Task 1 Step 1.3 / Task 2 Step 2.2 命令含 `--features semantic-recall-metal`

**Placeholder scan**：
- Task 4 issue 模板含 `[see /tmp/beta-15b-9-qwen3-8b-metadata.txt]` 引用（合理：内容在 Step 4.1 生成、issue body 引用即可）
- Task 5 Step 5.3 baseline 节模板含 `[按实测填]` / `[填]` / `[URL]` 占位（合理：与 BETA-15B-8 Task 4 baseline 节 `<填>` 同款风格、implementer 按实际数据 / win/fail 路径填）
- Task 5 Step 5.4-5.6 / 5.7 含 `<编号>` / `<sha>` 占位（cycle 末 doc-sync 时 PR 编号 + merge commit 才落定、参 BETA-15B-8 Task 5 doc-sync 补丁 `7ab4067` 同款节奏：合 main 后 follow-up commit 回填）
- 其他无 TBD / TODO / "implement later"

**Type consistency**：
- vectors-qwen3-8b.json / vectors-bge-m3.json / vectors.json / vectors-qwen3-0.6b.json 文件名全文一致
- `--vectors-file vectors-qwen3-8b.json` 用 basename 全文一致（BETA-15B-8 验过 evals `fixt()` 需 basename）
- `--features semantic-recall-metal` 全文一致
- main 基准 commit `5305ee1`（BETA-15B-8 merge）全文一致
- llama-cpp-4 版本 `0.3.0` → `0.3.2` 全文一致（含 fallback `0.3.1` 中间档）
- `gpu_layers=99` / `context_size=2048` 默认参数全文一致

**修订记录**：plan 写作过程中无 spec 描述与代码不符的发现（spec 已基于 cargo info 实测、Task 1 cargo update 命令应直接成立）。若 implementer 在 Task 1 Step 1.1 实际跑时遇 `cargo update` 失败、参 Step 1.2 fallback 路径（先 0.3.1 再 0.3.0 调查）。

---

## 执行选项

**Plan complete and saved to `docs/superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md`。**

两种执行模式（按 LociFind 历史 cycle 惯例推荐 1）：

1. **Subagent-Driven（推荐）** — fresh subagent per task + 每 task spec/code-quality 双审 + final integration review、与 BETA-15B-8 同款节奏
2. **Inline Execution** — 当前会话连续执行 5 task、checkpoint 在 Task 1 末 byte-equal 红线 / Task 2-3 末 sweep / Task 5 末 PR

**选哪个？**
