# BETA-15B-7：embedding 模型跨族 + 同族最大档探针 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用数据指证两条独立轴上的更强 embedding 模型对 cosine 单维天花板的影响：① 跨族架构轴（bge-m3、~568M 多语言 SOTA）；② 同族最大档轴（qwen3-embedding-8b、~8B 同 Qwen3 系列最大）。判断哪一条能突破 v3 在 qwen3-0.6b 上揭示的 0.864 OVERALL / 0.700 crosslang nDCG 天花板。

**Architecture:** 纯评测探针。`packages/evals/src/bin/semantic_quality.rs` 加 `--vectors-file <path>` flag（向下兼容、默认 `vectors.json`），让 `--embed` 输出和 sweep 读入都可指定路径；Mac Metal 跑 bge-m3 和 qwen3-8b 各一次 embed 产 `vectors-bge-m3.json` + `vectors-qwen3-8b.json` 两新文件 + `cp vectors.json vectors-qwen3-0.6b.json` 命名归位；三模型 9 阈值 sweep 产决策矩阵；按 spec §2.3 四 Branch 决策表落地；baseline 报告追加 v4 节。生产 wiring / 模型分发 / `baseline.json` / `gate.rs` 全不动。

**Tech Stack:** Rust + clap + llama-cpp-4（Metal）+ BAAI/bge-m3 q8_0 GGUF + Qwen3-Embedding-8B q8_0 GGUF；评测层（`packages/evals`）+ result-normalizer wrapper（不动、复用）。

**Spec：** [`docs/superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md`](../specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md)

**HF auth：** qwen3-embedding-8b 需 HF token + license accept。本会话使用用户提供的 token（环境变量 `HF_TOKEN`，本地用、用完即弃、不写盘）；bge-m3 公开免登录。

---

### Task 1: Setup — 起分支 + 下载 bge-m3 + qwen3-embedding-8b 模型

**Files:**
- 无代码改动
- 新增：`models/bge-m3-q8_0.gguf`（gitignored）
- 新增：`models/qwen3-embedding-8b-q8_0.gguf`（gitignored）

- [ ] **Step 1: 起 feature 分支（若已存在则切到）**

```bash
cd /Users/alice/Work/LocalFind
git checkout main
git pull
git checkout feat-beta-15b-7-embedding-model-scaling-probe 2>/dev/null || git checkout -b feat-beta-15b-7-embedding-model-scaling-probe
git status --short
```

Expected: 切到 main 拉新、起新 branch（或切到已有 branch）；`git status` 显示 clean 或仅 spec/plan 的 modified 文件（spec/plan 已在 main 之外修订）。

- [ ] **Step 2: 检查现 0.6b 模型在位、确认 models/ 目录可写**

```bash
ls -lh /Users/alice/Work/LocalFind/models/
```

Expected: 列出 `qwen3-embedding-0.6b-q8_0.gguf`（~610MB）+ `qwen3-0.6b-q4_k_m.gguf` + `qwen2.5-1.5b-instruct-q4_k_m.gguf`（后两个与本 cycle 无关、是 BETA-23 fallback 模型）。

- [ ] **Step 3: 下载 bge-m3 q8_0（公开仓库、无需 auth）**

```bash
cd /Users/alice/Work/LocalFind/models/
curl -L --retry 5 --retry-delay 10 -C - \
  -o bge-m3-q8_0.gguf \
  'https://huggingface.co/gpustack/bge-m3-GGUF/resolve/main/bge-m3-Q8_0.gguf?download=true'
```

Expected: 下载完成、文件 ~605 MB。无需 HF token。若主仓库 503 / 限速、可 fallback `vonjack/bge-m3-gguf` 或 `lm-kit/bge-m3-gguf`。

- [ ] **Step 4: 下载 qwen3-embedding-8b q8_0（gated、需 HF auth）**

注：HF_TOKEN 由用户在本会话提供、controller 已注入 env。subagent 执行此 step 时需 controller 把 token 透传到 prompt 里（或让 controller inline 跑、不走 subagent）。

```bash
cd /Users/alice/Work/LocalFind/models/
curl -L --retry 5 --retry-delay 10 -C - \
  -H "Authorization: Bearer ${HF_TOKEN}" \
  -o qwen3-embedding-8b-q8_0.gguf \
  'https://huggingface.co/Qwen/Qwen3-Embedding-8B-GGUF/resolve/main/Qwen3-Embedding-8B-Q8_0.gguf?download=true'
```

Expected: 下载完成、文件 ~7.7 GB。**4 GB+ 下载、可能超 600s timeout**——若 Bash 工具超时，用 `run_in_background: true` 启动后 `ls -lh` 轮询直到大小稳定 ≥ 7.6 GB。

- [ ] **Step 5: 记录 SHA256 + 体积**

```bash
cd /Users/alice/Work/LocalFind/models/
shasum -a 256 bge-m3-q8_0.gguf qwen3-embedding-8b-q8_0.gguf > /tmp/beta-15b-7-models-sha256.txt
ls -lh bge-m3-q8_0.gguf qwen3-embedding-8b-q8_0.gguf >> /tmp/beta-15b-7-models-sha256.txt
echo "---" >> /tmp/beta-15b-7-models-sha256.txt
shasum -a 256 qwen3-embedding-0.6b-q8_0.gguf >> /tmp/beta-15b-7-models-sha256.txt
ls -lh qwen3-embedding-0.6b-q8_0.gguf >> /tmp/beta-15b-7-models-sha256.txt
cat /tmp/beta-15b-7-models-sha256.txt
```

Expected: 三行 SHA256 + 三行 `ls -lh`（含 0.6b baseline 锚）。把这份信息留着 T6 baseline 报告 v4 节用。

- [ ] **Step 6: 确认 gitignore 兜住**

```bash
cd /Users/alice/Work/LocalFind
git status --short
git check-ignore models/bge-m3-q8_0.gguf models/qwen3-embedding-8b-q8_0.gguf
```

Expected: `git status` 不显示 `models/` 下任何 .gguf 文件；`git check-ignore` 退出码 0 + 输出两文件名。本步无 commit（模型不进 git）。

---

### Task 2: 加 `--vectors-file` flag (TDD)

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs:18-42`（Cli struct 加字段）
- Modify: `packages/evals/src/bin/semantic_quality.rs:95-96`（sweep 路径用新参）
- Modify: `packages/evals/src/bin/semantic_quality.rs:175`（embed 输出用新参）
- Modify: `packages/evals/src/bin/semantic_quality.rs:178-208`（cli_tests mod 加 2 新测）

- [ ] **Step 1: 写两条失败单测（先红）**

在 `packages/evals/src/bin/semantic_quality.rs:208` `cli_tests` mod 末尾、最后一个 `}` 前加：

```rust
    #[test]
    fn vectors_file_flag_parses() {
        let cli = Cli::parse_from(["semantic_quality", "--vectors-file", "vectors-bge-m3.json"]);
        assert_eq!(cli.vectors_file, "vectors-bge-m3.json");
    }

    #[test]
    fn vectors_file_defaults_to_vectors_json() {
        let cli = Cli::parse_from(["semantic_quality"]);
        assert_eq!(cli.vectors_file, "vectors.json");
    }
```

- [ ] **Step 2: 跑测确认红**

```bash
cd /Users/alice/Work/LocalFind
cargo test -p locifind-evals --bin semantic_quality cli_tests::vectors_file 2>&1 | tail -20
```

Expected: FAIL with "no field `vectors_file` on type `Cli`"（编译错误、说明 Cli struct 还没字段）。

- [ ] **Step 3: 加 Cli 字段**

在 `packages/evals/src/bin/semantic_quality.rs:41` `cosine_threshold` 字段后、第 42 行 `}` 前插入：

```rust
    /// vectors 文件相对路径（相对 `fixtures/semantic-recall/`）。
    /// 默认 = `vectors.json`（与现 baseline.json / gate 守护对象一致）。
    /// sweep 多模型时用：`--vectors-file=vectors-bge-m3.json` 等。
    /// 同时影响 `--embed`（输出位置）和默认 sweep（输入位置）。BETA-15B-7。
    #[arg(long, default_value = "vectors.json")]
    vectors_file: String,
```

- [ ] **Step 4: 替换 sweep 路径硬编码**

`packages/evals/src/bin/semantic_quality.rs:95-96` 把：

```rust
    let vectors = load_vectors(&fixt("vectors.json"))
        .expect("读 vectors.json（缺则先跑 --embed，见 README）");
```

改成：

```rust
    let vectors = load_vectors(&fixt(&cli.vectors_file))
        .unwrap_or_else(|_| panic!("读 {}（缺则先跑 --embed，见 README）", cli.vectors_file));
```

- [ ] **Step 5: 替换 embed 输出路径硬编码 + 函数签名扩参**

`packages/evals/src/bin/semantic_quality.rs` 函数 `embed_and_write` 签名 (line 131-135) 加 `vectors_file: &str` 参：

```rust
#[cfg(feature = "semantic-recall")]
fn embed_and_write(
    corpus: &[locifind_evals::semantic_quality::data::SemanticDoc],
    cases: &[locifind_evals::semantic_quality::data::SemanticCase],
    model: &str,
    vectors_file: &str,
) {
```

`line 175` 函数内的 `std::fs::write` 改成：

```rust
    std::fs::write(fixt(vectors_file), json)
        .unwrap_or_else(|_| panic!("写 {}", vectors_file));
```

`main()` 中 `if cli.embed` 分支 (line 79-93) 改为：

```rust
    if cli.embed {
        #[cfg(feature = "semantic-recall")]
        {
            embed_and_write(&corpus, &cases, &cli.model, &cli.vectors_file);
            eprintln!("已写 {}", &cli.vectors_file);
            return ExitCode::SUCCESS;
        }
        #[cfg(not(feature = "semantic-recall"))]
        {
            eprintln!(
                "--embed 需 feature semantic-recall（且放好模型）。见 fixtures/semantic-recall/README.md"
            );
            return ExitCode::from(2);
        }
    }
```

- [ ] **Step 6: 跑测确认绿**

```bash
cd /Users/alice/Work/LocalFind
cargo test -p locifind-evals --bin semantic_quality cli_tests 2>&1 | tail -20
```

Expected: 全部 6 测 passed（vectors_file_flag_parses / vectors_file_defaults_to_vectors_json / semantic_weight_flag_parses / semantic_weight_defaults_to_const / cosine_threshold_flag_parses / cosine_threshold_defaults_to_const）。

- [ ] **Step 7: 跑 workspace test + clippy + fmt 闸**

```bash
cd /Users/alice/Work/LocalFind
cargo test --workspace 2>&1 | tail -10
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --all --check 2>&1 | tail -5
```

Expected: `cargo test --workspace` 报 0 failed（新增 2 测、应 ~862 passed）；clippy 0 warning；fmt 净。

- [ ] **Step 8: 跑 evals byte-equal 闸 + gate 闸**

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | jq '{passed: map(select(.result.type=="pass")) | length, partial: map(select(.result.type=="partial")) | length, failed: map(select(.result.type=="fail")) | length}'
cargo run -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | jq '{passed: map(select(.result.type=="pass")) | length, partial: map(select(.result.type=="partial")) | length, failed: map(select(.result.type=="fail")) | length}'
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```

Expected: v0.5 报 `{"passed": 473, "partial": 25, "failed": 2}` + v0.9 报 `{"passed": 877, "partial": 119, "failed": 4}` 精确符合 baseline；gate 1 passed。

- [ ] **Step 9: 验 `--embed --vectors-file` flag 列入 `--help`**

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality -- --help 2>&1 | grep -A1 vectors-file
```

Expected: 列出 `--vectors-file <VECTORS_FILE>` + 默认值 `vectors.json`。

- [ ] **Step 10: commit**

```bash
cd /Users/alice/Work/LocalFind
git add packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-7 task 2：semantic_quality binary 加 --vectors-file flag（向下兼容、默认 vectors.json；同时影响 --embed 输出与 sweep 输入、用于本 cycle 三模型 sweep；2 新单测 vectors_file_flag_parses + vectors_file_defaults_to_vectors_json；workspace 862 passed / clippy 0 / fmt 净 / evals parser-only byte-equal v0.5=473 + v0.9=877 不变 / gate 1 passed）"
```

Expected: 单个 commit、git log 上一条。

---

### Task 3: Mac Metal `--embed` × 2 (bge-m3 + qwen3-8b) + 0.6b copy 命名归位

**Files:**
- 新增：`packages/evals/fixtures/semantic-recall/vectors-bge-m3.json`（commit）
- 新增：`packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json`（commit）
- 新增：`packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json`（commit、内容 ≡ 现 vectors.json）

- [ ] **Step 1: 跑 bge-m3 embed**

```bash
cd /Users/alice/Work/LocalFind
time cargo run -p locifind-evals --bin semantic_quality \
  --features semantic-recall-metal --release -- \
  --embed \
  --model models/bge-m3-q8_0.gguf \
  --vectors-file vectors-bge-m3.json
```

Expected: 估 ~3-5 min；末尾 `已写 vectors-bge-m3.json`；exit 0。`--release` 是为了 embed 性能（debug 编译会很慢）。

- [ ] **Step 2: 跑 qwen3-8b embed**

```bash
cd /Users/alice/Work/LocalFind
time cargo run -p locifind-evals --bin semantic_quality \
  --features semantic-recall-metal --release -- \
  --embed \
  --model models/qwen3-embedding-8b-q8_0.gguf \
  --vectors-file vectors-qwen3-8b.json
```

Expected: 估 ~30-50 min；末尾 `已写 vectors-qwen3-8b.json`；exit 0。Mac Metal q8_0 推理、202 序列。

- [ ] **Step 3: 验两新 vectors 文件 schema 正确**

```bash
cd /Users/alice/Work/LocalFind/packages/evals/fixtures/semantic-recall/
for f in vectors-bge-m3.json vectors-qwen3-8b.json; do
  echo "=== $f ==="
  jq '{model_id, dim, n_docs: (.doc_vectors | length), n_queries: (.query_vectors | length)}' "$f"
done
```

Expected: 两文件各显示 `model_id` 含正确 GGUF 路径、`dim` > 0（bge-m3 大概率 1024、qwen3-8b 大概率 4096）、`n_docs=124`、`n_queries=78`。把 dim 实测值留着 T6 baseline 报告 v4 节用。

- [ ] **Step 4: copy 现 vectors.json 为 vectors-qwen3-0.6b.json（命名归位）**

```bash
cd /Users/alice/Work/LocalFind/packages/evals/fixtures/semantic-recall/
cp vectors.json vectors-qwen3-0.6b.json
ls -lh vectors-*.json
```

Expected: 三文件列出、0.6b 与 vectors.json 同体积（2.5MB）、bge-m3 ~2.5MB、qwen3-8b ~10MB（具体看 dim）。

- [ ] **Step 5: 验 vectors.json byte-equal 不动**

```bash
cd /Users/alice/Work/LocalFind
git diff packages/evals/fixtures/semantic-recall/vectors.json
git status --short packages/evals/fixtures/semantic-recall/
```

Expected: `git diff` 净（vectors.json 字节不动）；`git status --short` 显示 3 个 `??` 新文件（vectors-{bge-m3,qwen3-8b,qwen3-0.6b}.json）。

- [ ] **Step 6: 抽样验向量归一化（L2 norm ≈ 1.0）**

```bash
cd /Users/alice/Work/LocalFind/packages/evals/fixtures/semantic-recall/
for f in vectors-bge-m3.json vectors-qwen3-8b.json; do
  echo "=== $f L2 norm 抽样 ==="
  jq -r '.doc_vectors | to_entries | .[0:3] | .[] | "\(.key): \([(.value[] | . * .)] | add | sqrt)"' "$f"
done
```

Expected: 每个抽样向量的 L2 norm 在 [0.98, 1.02] 区间（cosine 假设的归一化前提）。若超出 → §5 Branch IV 排查。注：若发现 bge-m3 vector 未归一化（model 输出 raw embedding 不归一），需在 §5 排查清单标记。

- [ ] **Step 7: commit 三新 vectors 文件**

```bash
cd /Users/alice/Work/LocalFind
git add packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json
git add packages/evals/fixtures/semantic-recall/vectors-bge-m3.json
git add packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json
git commit -m "BETA-15B-7 task 3：Mac Metal 重算 vectors × 三模型（qwen3-embedding-0.6b 已知锚 / bge-m3 跨族同尺寸候选 / qwen3-embedding-8b 同族最大档；0.6b ≡ 现 vectors.json copy 命名归位、bge-m3/8b 全集 124 doc + 78 query 实测产出；schema {model_id, dim, doc_vectors, query_vectors} 验证全过 + L2 norm ≈ 1.0 抽样验过；现 vectors.json byte-equal 不动；模型 .gguf gitignore 兜住不进库）"
```

Expected: 单个 commit、git log 上又一条。

---

### Task 4: sweep 9 阈值 × 3 模型 = 决策矩阵

**Files:**
- 无代码改动
- 临时：`/tmp/beta-15b-7-sweep/*.json`（不进 git、只用于 T5/T6 读表）

- [ ] **Step 1: sweep 0.6b 9 阈值**

```bash
cd /Users/alice/Work/LocalFind
mkdir -p /tmp/beta-15b-7-sweep
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file vectors-qwen3-0.6b.json \
    --semantic-weight 10.0 \
    --cosine-threshold "$T" \
    --json > "/tmp/beta-15b-7-sweep/0.6b-T${T}.json" 2>/dev/null
  echo "0.6b T=$T done"
done
```

Expected: 9 个 JSON 文件、每个含 6 桶 nDCG/Recall。

- [ ] **Step 2: sweep bge-m3 9 阈值**

```bash
cd /Users/alice/Work/LocalFind
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file vectors-bge-m3.json \
    --semantic-weight 10.0 \
    --cosine-threshold "$T" \
    --json > "/tmp/beta-15b-7-sweep/bge-m3-T${T}.json" 2>/dev/null
  echo "bge-m3 T=$T done"
done
```

Expected: 9 个 JSON 文件。

- [ ] **Step 3: sweep qwen3-8b 9 阈值**

```bash
cd /Users/alice/Work/LocalFind
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file vectors-qwen3-8b.json \
    --semantic-weight 10.0 \
    --cosine-threshold "$T" \
    --json > "/tmp/beta-15b-7-sweep/qwen3-8b-T${T}.json" 2>/dev/null
  echo "qwen3-8b T=$T done"
done
```

Expected: 9 个 JSON 文件。

- [ ] **Step 4: 聚合三模型 sweep 决策矩阵**

```bash
cd /tmp/beta-15b-7-sweep
for MODEL_LABEL in "0.6b:0.6b" "bge-m3:bge-m3" "qwen3-8b:qwen3-8b"; do
  pfx="${MODEL_LABEL%%:*}"
  label="${MODEL_LABEL#*:}"
  echo "============================================="
  echo "Model: $label"
  echo "T      | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N"
  echo "---------------------------------------------------------------------"
  for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
    jq -r --arg T "$T" '
      [
        (.[] | select(.bucket == "exact-name") | .hybrid_routed_recall),
        (.[] | select(.bucket == "OVERALL") | .hybrid_routed_ndcg),
        (.[] | select(.bucket == "crosslang") | .hybrid_routed_ndcg),
        (.[] | select(.bucket == "content-not-name") | .hybrid_routed_ndcg)
      ] | "T=\($T) | \(.[0]) | \(.[1]) | \(.[2]) | \(.[3])"' "${pfx}-T${T}.json"
  done
done > /tmp/beta-15b-7-sweep/decision-matrix.txt
cat /tmp/beta-15b-7-sweep/decision-matrix.txt
```

Expected: 三模型 × 9 阈值 × 4 关键桶的扁平表、留 T5/T6 读。

- [ ] **Step 5: 完整 6 桶视图（人工审用）**

```bash
cd /Users/alice/Work/LocalFind
for MODEL in 0.6b bge-m3 qwen3-8b; do
  echo "=========================================================================================="
  echo "Model: $MODEL, T = 0.70 (v3 bake)"
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file "vectors-${MODEL}.json" \
    --semantic-weight 10.0 \
    --cosine-threshold 0.70 2>/dev/null
done > /tmp/beta-15b-7-sweep/full-table-T0.70.txt
# 注：上述会因 0.6b copy 命名归位、文件叫 vectors-qwen3-0.6b.json、需另跑
cat /tmp/beta-15b-7-sweep/full-table-T0.70.txt
```

注：第一行 MODEL=0.6b 文件名应是 `vectors-qwen3-0.6b.json`。简化：

```bash
cd /Users/alice/Work/LocalFind
for f in vectors-qwen3-0.6b.json vectors-bge-m3.json vectors-qwen3-8b.json; do
  echo "=========================================================================================="
  echo "Vectors file: $f, T = 0.70"
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file "$f" \
    --semantic-weight 10.0 \
    --cosine-threshold 0.70 2>/dev/null
done > /tmp/beta-15b-7-sweep/full-table-T0.70.txt
cat /tmp/beta-15b-7-sweep/full-table-T0.70.txt
```

Expected: 三模型在 T=0.70 的完整 6 桶 × FTS/VEC/HYB/HYBR 4 算法表（v3 已实测 0.6b 这一行作为对照锚点）。

- [ ] **Step 6: 本 task 无 commit**（sweep 输出在 /tmp/ 不进 git）

---

### Task 5: 四 Branch 决策表落地

**Files:**
- 写到 `/tmp/beta-15b-7-sweep/branch-decision.md`（不进 git、留 T6 读）

- [ ] **Step 1: 读 sweep 矩阵、识别每模型的 T\* sweep best**

打开 `/tmp/beta-15b-7-sweep/decision-matrix.txt`，对 bge-m3 和 qwen3-8b 各找：
- 满足 (4a) exact-name HYBR_R = 1.000
- 满足 (4b) 各桶 HYBR_N ≥ 0.6b HYB baseline 同桶（参考 spec §1.1 baseline 值 + 0.6b sweep T=1.01 的 6 桶数为 HYB baseline）
- 最大化 OVERALL HYBR_N 的那个 T

记到 `/tmp/beta-15b-7-sweep/branch-decision.md`：

```markdown
## bge-m3 模型 T* 决定
- sweep best T* = ___
- exact-name HYBR_R = ___ (≥1.000?)
- 各桶 HYBR_N vs 0.6b HYB baseline 全表
- OVERALL HYBR_N = ___ (≥ 0.864?)
- crosslang HYBR_N = ___ (≥ 0.700?)

## qwen3-8b 模型 T* 决定（同样 schema）
- ...

## Branch 判定（按 spec §2.3）
- bge-m3 落 Branch: I-a / II / III / IV
- qwen3-8b 落 Branch: I-b / II / III / IV
- **整体决策**（spec §2.3 优先级：I-a > I-b > II > III）：___
```

- [ ] **Step 2: 应急 Branch IV 异常排查**（仅当任一模型破红线时进入）

若 bge-m3 或 qwen3-8b 任一桶 HYBR_N < 0.6b HYB baseline → 走 spec §5 异常清单 6 条：
- 检查 model_id 字段（`jq .model_id vectors-{bge-m3,qwen3-8b}.json`）是否对应正确 GGUF
- 检查 dim 字段合理（bge-m3 ≈ 1024、qwen3-8b ≈ 4096）
- 检查 n_docs=124、n_queries=78
- 检查 L2 norm ≈ 1.0（T3 step 6 重跑）
- 检查 GGUF SHA256 vs HuggingFace 官方
- 若 6 条全过仍退步 → 记 baseline 报告 v4 节 Branch IV、不发布、留 STATUS 异常记录

写排查结论到 `/tmp/beta-15b-7-sweep/branch-decision.md` 末尾。

- [ ] **Step 3: 本 task 无 commit**（决策记录走 T6 baseline 报告 v4 节）

---

### Task 6: baseline 报告追加 v4 节

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（在末尾追加 v4 节）
- Modify: `packages/evals/fixtures/semantic-recall/README.md`（v4 注脚 1-2 行）

- [ ] **Step 1: 在 baseline 报告末尾追加 v4 节**

打开 `docs/reviews/semantic-recall-quality-baseline.md`，在末尾追加：

```markdown
## v4 数据集节 — embedding 模型跨族 + 同族最大档探针（BETA-15B-7）

> 承接 v3 cycle 主动放弃字面 0.864 / 0.700 spec 目标后的认知层结论：「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下结构性不可达、移交下 cycle 抓手 = 更大 / 更强 embedding 模型。本 cycle 用数据指证两条独立轴上的 embedding 模型：① 跨族架构轴 = bge-m3（BAAI、~568M、多语言 SOTA）；② 同族最大档轴 = qwen3-embedding-8b（Qwen 官方、~8B、Qwen3 系列最大）。

### 模型清单

| 模型 | 族 | 文件大小 | dim | 来源 |
|---|---|---|---|---|
| qwen3-embedding-0.6b q8_0 | Qwen3 | 610 MB | 1024 | Qwen 官方（已有锚） |
| **bge-m3 q8_0** | BAAI BGE | ___（T3 实测填） | ___（T3 实测填） | `gpustack/bge-m3-GGUF`（公开） |
| **qwen3-embedding-8b q8_0** | Qwen3 | ___（T3 实测填） | ___（T3 实测填） | `Qwen/Qwen3-Embedding-8B-GGUF`（gated） |

SHA256 见 `/tmp/beta-15b-7-models-sha256.txt`（不入仓、本地校验用）。

### 三模型 sweep 全表（v3 数据集 78 cases / 124 docs / W=10.0 固定）

#### qwen3-embedding-0.6b q8_0（控制对照、v3 已知锚点）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
（从 T4 sweep 0.6b 9 行填入）

#### bge-m3 q8_0（跨族架构轴）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
（从 T4 sweep bge-m3 9 行填入、标 T\* sweep best ⭐）

#### qwen3-embedding-8b q8_0（同族最大档轴）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
（从 T4 sweep qwen3-8b 9 行填入、标 T\* sweep best ⭐）

### Branch 判定（按 spec §2.3）

- bge-m3 T\* = ___ 落 Branch ___（OVERALL ___、crosslang ___）
- qwen3-8b T\* = ___ 落 Branch ___（OVERALL ___、crosslang ___）
- **整体决策**：___

### 数据指证与下 cycle 抓手优先级修正

（按 Branch I-a/I-b/II/III/IV 落不同段落）

**Branch I-a（bge-m3 GO ⭐）**：跨族架构在同尺寸（605 MB ≈ 610 MB）上破局、潜在零分发成本破局。本 cycle 不 bake、开 follow-up cycle BETA-15B-7-v2 推到生产。工作清单提纲：
- 模型推到 desktop `DEFAULT_EMBEDDING_MODEL_PATH` = `bge-m3-q8_0.gguf`
- Mac + Windows 真机暖机时长重测（bge-m3 加载时间 vs qwen3-0.6b）
- llama-cpp-4 兼容性确认（bge-m3 GGUF 与现 0.6b 走同款推理栈）
- 重算 baseline.json 反映 bge-m3 新数据 + 升 gate.rs 红线断言到新 baseline
- 同分发包大小（605 ≈ 610 MB）= 模型分发 UX 零增量改动

**Branch I-b（qwen3-8b GO）**：同族最大档破局、接受 ~7.7 GB 分发成本。本 cycle 不 bake、开 follow-up cycle BETA-15B-7-v2 推到生产 + 模型分发 UX。工作清单提纲：
- 模型推到 desktop `DEFAULT_EMBEDDING_MODEL_PATH` = `qwen3-embedding-8b-q8_0.gguf`
- 模型分发 UX：首次启动下载提示、进度条、离线 fallback
- Windows GPU 路径校验（BETA-15B-4 关联）
- Mac + Windows 真机暖机时长重测（qwen3-8b 加载时间预计 +5-10s）
- 重算 baseline.json 反映 qwen3-8b 新数据 + 升 gate.rs 红线断言到新 baseline

**Branch II（NO GO、crosslang 在新模型上反退）**：移交下 cycle = 评测扩量（crosslang 桶 13 → 25+）+ 跨族更大（bge-multilingual-gemma2 9B）作下个抓手。

**Branch III（两条独立轴都见顶）**：移交下 cycle 最高优 = ① 评测扩量专攻 OVERALL 弱桶；② 跨族更大（bge-multilingual-gemma2 9B、~9 GB）/ Linq-Embed-Mistral / EmbeddingGemma-300M（新代小模型 SOTA）。

**Branch IV（异常）**：本 cycle 不发布、STATUS 留异常排查记录。

### 认知层修订小结

（按 Branch 写不同结论）：

- Branch I-a 时：「v3 cycle 主动放弃的字面 spec 目标在 v4 cycle 重新激活、bge-m3 在同尺寸上破天花板 = `跨族架构` 抓手数据指证 GO；qwen3-0.6b 不是因为「太小」而是因为「Qwen3 训练数据 / 架构对多语言 retrieval 不如 BAAI bge-m3 专精」；移交 follow-up cycle BETA-15B-7-v2 推到生产、分发零增量」
- Branch I-b 时：「v3 cycle 主动放弃的字面 spec 目标在 v4 cycle 重新激活、qwen3-8b 在同族最大档上破天花板 = `同族放大` 抓手数据指证 GO、但 bge-m3 同尺寸跨族架构未达；移交 follow-up cycle BETA-15B-7-v2 推到生产 + ~7.7 GB 分发成本」
- Branch II/III 时：「v4 cycle 数据指证两条独立轴上更强 embedding 模型在合成集上都不足以突破 0.864 OVERALL 字面目标；移交下 cycle = 评测扩量 + 跨族更大（bge-multilingual-gemma2 9B）作 0.864 字面 spec 目标真正破局抓手」

### v4 评测的边界

- 本 cycle 不动 `baseline.json` / `gate.rs` / `DEFAULT_COSINE_ROUTING_THRESHOLD` / desktop wiring / 模型分发
- gate 仍守护 v3 0.6b baseline、本 cycle 任何模型升级 bake 都在 follow-up cycle
- 三 vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json 入仓作研究产物、与 `vectors.json` 解耦不影响 gate
```

填表数据从 `/tmp/beta-15b-7-sweep/decision-matrix.txt` + `/tmp/beta-15b-7-sweep/branch-decision.md` 抠出。

- [ ] **Step 2: 更新 `packages/evals/fixtures/semantic-recall/README.md` v4 注脚**

在 README.md 开头「dataset_name」前后找合适位置，加 1-2 行：

```markdown
- vectors-multi-model: vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json（BETA-15B-7 v4 模型跨族 + 同族最大档探针、参 [baseline 报告 v4 节](../../../../docs/reviews/semantic-recall-quality-baseline.md)；与 gate 守护对象 `vectors.json` 解耦）
```

- [ ] **Step 3: commit baseline 报告 v4 节 + README 注脚**

```bash
cd /Users/alice/Work/LocalFind
git add docs/reviews/semantic-recall-quality-baseline.md
git add packages/evals/fixtures/semantic-recall/README.md
git commit -m "BETA-15B-7 task 6：baseline 报告追加 v4 节（embedding 模型跨族 + 同族最大档探针：qwen3-0.6b/bge-m3/qwen3-8b 三模型 9 阈值 sweep 全表 + Branch ___ 判定 + 数据指证 + 下 cycle 抓手优先级修正 + 认知层修订小结）+ README v4 注脚（vectors-* 三研究产物文件、与 gate 守护对象解耦）"
```

把 commit message 里的 `Branch ___` 换成实际判定（I-a/I-b/II/III/IV）。

---

### Task 7: 总验收 + STATUS/ROADMAP 收尾准备

**Files:**
- 无新文件
- 准备 PR 描述（不开 PR、用户走收工流程时再决）

- [ ] **Step 1: workspace 测全过**

```bash
cd /Users/alice/Work/LocalFind
cargo test --workspace 2>&1 | tail -10
```

Expected: 0 failed、约 862 passed（v3 860 + 本 cycle T2 加 2 测）。

- [ ] **Step 2: clippy + fmt 净**

```bash
cd /Users/alice/Work/LocalFind
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -10
cargo fmt --all --check 2>&1 | tail -5
```

Expected: clippy 0 warning、fmt 无输出。

- [ ] **Step 3: evals parser byte-equal v0.5 + v0.9 精确**

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | jq '{passed: map(select(.result.type=="pass")) | length, partial: map(select(.result.type=="partial")) | length, failed: map(select(.result.type=="fail")) | length}'
cargo run -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | jq '{passed: map(select(.result.type=="pass")) | length, partial: map(select(.result.type=="partial")) | length, failed: map(select(.result.type=="fail")) | length}'
```

Expected: v0.5 `{"passed": 473, "partial": 25, "failed": 2}`、v0.9 `{"passed": 877, "partial": 119, "failed": 4}` 精确不变。

- [ ] **Step 4: gate 1 passed**

```bash
cd /Users/alice/Work/LocalFind
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```

Expected: `test result: ok. 1 passed; 0 failed`。

- [ ] **Step 5: vectors.json byte-equal 不动**

```bash
cd /Users/alice/Work/LocalFind
git diff main..HEAD -- packages/evals/fixtures/semantic-recall/vectors.json
git log --oneline main..HEAD -- packages/evals/fixtures/semantic-recall/vectors.json
```

Expected: 空输出（vectors.json 自分支起点到 HEAD 字节不动）。

- [ ] **Step 6: branch 状态总核**

```bash
cd /Users/alice/Work/LocalFind
git log --oneline main..HEAD
git status --short
```

Expected: 3 个 commit（T2 flag、T3 vectors 三文件、T6 baseline 报告 v4 + README）；`git status` clean。

- [ ] **Step 7: 准备 PR 描述（不开 PR、留收工时用户决）**

写出 PR title + body 草稿到对话里（或留 `/tmp/beta-15b-7-pr-draft.md`），含：
- title：`BETA-15B-7：embedding 模型跨族 + 同族最大档探针 + Branch ___ 判定`
- body：复述 spec 目标、Branch 判定 + 数据指证 + 下 cycle 抓手、3 commit 清单、§6 验证矩阵全过证据

收工时用户拍板「合 main」还是「再迭代一轮」。

---

## Self-Review

After writing the complete plan, look at the spec with fresh eyes and check the plan against it.

**1. Spec coverage（spec §2.1 目标 / §3.1 In-scope / §4 实施步骤）：**

| Spec 条目 | Plan task |
|---|---|
| 下载 bge-m3 + qwen3-8b 两 GGUF | Task 1 Step 3-5 |
| 加 `--vectors-file` flag + 2 单测 | Task 2 全部 |
| Mac Metal embed × 2 + copy 0.6b | Task 3 全部 |
| 9 阈值 sweep × 3 模型 | Task 4 全部 |
| 四 Branch 判定 + 数据指证 | Task 5 全部 |
| baseline 报告 v4 节 | Task 6 step 1 |
| README.md v4 更新 | Task 6 step 2 |
| 总验收 7 项（§6 验证矩阵） | Task 7 step 1-5 |
| `--vectors-file` flag 向下兼容（§2.2 (6)） | Task 2 step 6+8（默认值测 + parser/gate byte-equal 验） |
| vectors.json byte-equal（§2.2 (4) 间接） | Task 3 step 5 + Task 7 step 5 |
| Branch IV 排查清单（spec §5） | Task 5 step 2 + Task 3 step 6（L2 norm 抽样） |

**所有 spec 条目均有 task 覆盖。**

**2. Placeholder scan：**

- 表格里 `___` 占位 = Task 5/6 实测数据填入位、不是 plan 失败（spec 已明示这些是 cycle 运行时产生的实测数据、不能预先写死）
- Task 1 step 4 注释了 HF_TOKEN 透传问题（controller 责任、非 plan 失败）
- Task 5 step 2 「应急 Branch IV」分支条件清晰

**3. Type consistency：**

- `cli.vectors_file: String` 在 T2 step 3 定义 → 在 T2 step 4 / step 5 / step 6 / cli_tests 一致使用
- `embed_and_write(corpus, cases, model, vectors_file: &str)` 签名在 T2 step 5 修订一次 + main() 调用同步
- `--vectors-file vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json` 文件名在 T3 / T4 / T6 全程一致
- 模型文件名 `bge-m3-q8_0.gguf` + `qwen3-embedding-8b-q8_0.gguf` 在 T1 / T3 全程一致

**类型一致性无问题。**
