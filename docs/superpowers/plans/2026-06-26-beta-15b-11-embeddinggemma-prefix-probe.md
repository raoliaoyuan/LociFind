# BETA-15B-11 Implementation Plan：EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用数据指证 EmbeddingGemma-300M 是否能冲过 spec 字面 OVERALL ≥ 0.864 + crosslang ≥ 0.700（v5 bge-m3 baseline 留下的 crosslang -0.014 字面 gap），同时双轴对照「裸 embed vs 标准 prefix」回答「prefix 契约是必要条件还是 nice-to-have」。

**Architecture:** 评测探针 cycle、不动桌面 wiring / 不动 baseline.json / 不动 cosine_threshold。`packages/model-runtime/src/pooling.rs` 扩 `gemma-embedding` arch 白名单 + `packages/evals/src/bin/semantic_quality.rs` 加 `--prefix-mode {none,standard}` flag。Mac Metal 跑双 vectors（裸 + prefix）+ 9 阈值 sweep × 2 mode = 18 次 = 决策矩阵 → 四 Branch GO 判定。GO 候选命中开 follow-up cycle BETA-15B-11-v2 bake 推生产。

**Tech Stack:** Rust 1.x / llama-cpp-4 0.3.2（应急升级 to 最新）/ Mac Metal GGUF q8_0 / clap / TDD（pooling.rs + semantic_quality.rs）

**Spec:** [docs/superpowers/specs/2026-06-26-beta-15b-11-embeddinggemma-prefix-probe-design.md](../specs/2026-06-26-beta-15b-11-embeddinggemma-prefix-probe-design.md)

---

## Task 0：开 cycle 预检（不 commit、几秒）

**Goal:** 起点状态确认 — 仓库干净 + main HEAD 与 STATUS 一致 + 本机模型清单 + llama-cpp-4 当前版本。

- [ ] **Step 0.1: 看仓库状态干净**

```bash
cd /Users/alice/Work/LocalFind
git status
git log --oneline -5
```

Expected: working tree clean, branch main 与 origin/main 一致；HEAD 应为本会话 commit `805f74d`（BETA-15B-11 spec）之后，BETA-15B-10 merge commit `8e707cf` 之前应在 -5 内可见。

- [ ] **Step 0.2: 看本机已有模型**

```bash
ls -la models/*.gguf
```

Expected: 至少含 `qwen3-embedding-0.6b-q8_0.gguf`（639 MB）和 `bge-m3-q8_0.gguf`（634 MB）；**不应**含 `embeddinggemma-300m-q8_0.gguf`（Task 1 才下载）。

- [ ] **Step 0.3: 看 llama-cpp-4 当前版本**

```bash
grep -n "llama-cpp-4" Cargo.toml packages/*/Cargo.toml | grep -v ".lock"
```

Expected: 当前 BETA-15B-9 升级后版本 = `llama-cpp-4 = { version = "0.3.2", ... }` 或 workspace inherit。记下版本号 — Task 4 决策点会用。

- [ ] **Step 0.4: 开 feature branch**

```bash
git checkout -b feat-beta-15b-11-embeddinggemma-prefix-probe
git status
```

Expected: switched to new branch `feat-beta-15b-11-embeddinggemma-prefix-probe`、working tree clean。

---

## Task 1：GGUF 下载 + metadata 抽（不 commit、~30 min）

**Goal:** 把 `embeddinggemma-300m-q8_0.gguf` 放到 `models/`、确认 `general.architecture` = `gemma-embedding`、`embedding_length` = 768。spec §4.3。

**Files:**
- Create: `models/embeddinggemma-300m-q8_0.gguf`（gitignored、不入仓）

- [ ] **Step 1.1: 从 ggml-org 公开转仓下载（优先免登录路径）**

```bash
mkdir -p /tmp/embeddinggemma-download
huggingface-cli download ggml-org/embeddinggemma-300m-qat-q8_0-GGUF \
    --include "*.gguf" \
    --local-dir /tmp/embeddinggemma-download
ls -la /tmp/embeddinggemma-download/
```

Expected: ~329 MB 的 `embeddinggemma-300m-qat-Q8_0.gguf`（可能是这个名、或 `embeddinggemma-300M-Q8_0.gguf` 类似变体）下载完成。

**异常处理**：若 ggml-org 仓 404 或 license 拦截 → 改 `google/embeddinggemma-300m-qat-q8_0-gguf`（需 HF login + license accept），命令同款。若两个都失败 → STOP、报用户决策（spec §5.1 触发）。

- [ ] **Step 1.2: 文件名规范化 + 移到 models/**

```bash
# 文件名可能不同、用 glob 适应
mv /tmp/embeddinggemma-download/*Q8_0*.gguf models/embeddinggemma-300m-q8_0.gguf
ls -la models/embeddinggemma-300m-q8_0.gguf
```

Expected: 文件大小 ~329 MB（330,000,000 bytes ± 1%）。

- [ ] **Step 1.3: SHA256 + GGUF magic 校验**

```bash
sha256sum models/embeddinggemma-300m-q8_0.gguf
xxd models/embeddinggemma-300m-q8_0.gguf | head -1
```

Expected: SHA256 记录到本 task `/tmp/beta-15b-11-gguf-sha256.txt`（后续 baseline 报告 v6 节落库）；GGUF magic = `47 47 55 46`（"GGUF" ASCII）on line 1。

- [ ] **Step 1.4: 抽 GGUF metadata 验 arch + dim + pooling_type**

```bash
# 选项 A：用 Python gguf 包（pip install gguf）
python3 -c "
from gguf import GGUFReader
r = GGUFReader('models/embeddinggemma-300m-q8_0.gguf')
keys = ['general.architecture', 'general.name']
for f in r.fields.values():
    if any(k in f.name for k in ['architecture', 'embedding_length', 'pooling_type', 'context_length']):
        keys.append(f.name)
for k in set(keys):
    f = r.fields.get(k)
    if f:
        print(f'{k}: {f.parts}')
"
```

Expected:
- `general.architecture: ['gemma-embedding']`（字符串数组、实际值 = "gemma-embedding"）
- `gemma-embedding.embedding_length: [768]`
- `gemma-embedding.pooling_type: [1]` (1=Mean) 或不存在（fallback 走 [pooling.rs](../../packages/model-runtime/src/pooling.rs) `default_pooling_for_arch`）

**异常处理**：若 arch ≠ `gemma-embedding` → STOP、走 spec §5.1 异常处理（检 GGUF 是否被替换）。若 Python gguf 包没装 → 用 `pip install gguf` 或备选方案 hex dump 看 metadata。

- [ ] **Step 1.5: 记录 metadata 到本 cycle 决策日志**

```bash
mkdir -p /tmp/beta-15b-11
cat > /tmp/beta-15b-11/gguf-metadata.md <<EOF
# embeddinggemma-300m-q8_0.gguf metadata

- SHA256: $(sha256sum models/embeddinggemma-300m-q8_0.gguf | awk '{print $1}')
- Size: $(stat -f%z models/embeddinggemma-300m-q8_0.gguf) bytes
- arch: gemma-embedding
- embedding_length: 768
- pooling_type: <FILL>（如 1=Mean 或 unset → fallback default_pooling_for_arch）
- context_length: <FILL>
EOF
cat /tmp/beta-15b-11/gguf-metadata.md
```

Expected: metadata 文件落 /tmp、后续 Task 10 baseline 报告 v6 节抄进来。

---

## Task 2：pooling.rs 扩 gemma-embedding 白名单（TDD）

**Goal:** [`packages/model-runtime/src/pooling.rs`](../../packages/model-runtime/src/pooling.rs) `default_pooling_for_arch` 加 `"gemma-embedding" => Mean` 分支 + 1 单测。

**Files:**
- Modify: `packages/model-runtime/src/pooling.rs:32-40`（`default_pooling_for_arch` 函数体）
- Modify: `packages/model-runtime/src/pooling.rs:75-130`（tests 模块）

**Spec ref:** §4.1

- [ ] **Step 2.1: 写 failing 单测**

打开 [`packages/model-runtime/src/pooling.rs`](../../packages/model-runtime/src/pooling.rs)、找到 `mod tests` 块、在最后一个 `#[test]` 之后新加：

```rust
#[test]
fn default_pooling_for_arch_gemma_embedding_is_mean() {
    assert_eq!(
        default_pooling_for_arch("gemma-embedding").unwrap(),
        LlamaPoolingType::Mean
    );
}
```

- [ ] **Step 2.2: 跑测试验证 fail**

```bash
cargo test -p locifind-model-runtime --features llama-cpp default_pooling_for_arch_gemma_embedding_is_mean
```

Expected: FAIL with `unknown architecture 'gemma-embedding'` error from `Err(ModelError::LoadError(...))`.

- [ ] **Step 2.3: 写最小实现**

找到 [`packages/model-runtime/src/pooling.rs:32-40`](../../packages/model-runtime/src/pooling.rs#L32) `default_pooling_for_arch`：

```rust
pub(crate) fn default_pooling_for_arch(arch: &str) -> Result<LlamaPoolingType, ModelError> {
    match arch {
        "bert" | "nomic-bert" | "jina-bert-v2" | "roberta" => Ok(LlamaPoolingType::Cls),
        "t5" => Ok(LlamaPoolingType::Mean),
        "llama" | "qwen2" | "qwen3" | "mistral" => Ok(LlamaPoolingType::Last),
        _ => Err(ModelError::LoadError(format!(
            "unknown architecture '{arch}' and GGUF did not declare <arch>.pooling_type; \
             declare pooling_type in GGUF metadata or extend default_pooling_for_arch heuristic table"
        ))),
    }
}
```

改为（把 `gemma-embedding` 加到 `"t5" => Mean` 同行、用 `|` 联通）：

```rust
pub(crate) fn default_pooling_for_arch(arch: &str) -> Result<LlamaPoolingType, ModelError> {
    match arch {
        "bert" | "nomic-bert" | "jina-bert-v2" | "roberta" => Ok(LlamaPoolingType::Cls),
        "t5" | "gemma-embedding" => Ok(LlamaPoolingType::Mean),
        "llama" | "qwen2" | "qwen3" | "mistral" => Ok(LlamaPoolingType::Last),
        _ => Err(ModelError::LoadError(format!(
            "unknown architecture '{arch}' and GGUF did not declare <arch>.pooling_type; \
             declare pooling_type in GGUF metadata or extend default_pooling_for_arch heuristic table"
        ))),
    }
}
```

- [ ] **Step 2.4: 跑测试验证 pass**

```bash
cargo test -p locifind-model-runtime --features llama-cpp default_pooling_for_arch_gemma_embedding_is_mean
```

Expected: 1 passed。

- [ ] **Step 2.5: 跑全 pooling 模块测试 + workspace 测试 verify 零回归**

```bash
cargo test -p locifind-model-runtime --features llama-cpp pooling
cargo test -p locifind-model-runtime
```

Expected: pooling 模块 10 passed（原 9 + 新 1）+ 0 failed；workspace 不退步。

- [ ] **Step 2.6: clippy + fmt 净**

```bash
cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: 0 warning + diff 净。

**注意：不 commit。Task 5/6 一并 C1 commit。**

---

## Task 3：semantic_quality.rs 加 --prefix-mode flag（TDD）

**Goal:** [`packages/evals/src/bin/semantic_quality.rs`](../../packages/evals/src/bin/semantic_quality.rs) 加 `--prefix-mode {none,standard}` clap flag、default `none`、embed 闭包按 mode inline 包 prefix。

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs`（clap Cli struct + embed 闭包 + 1-2 单测）

**Spec ref:** §4.2

- [ ] **Step 3.1: 读现状定位 clap struct + embed 闭包**

```bash
grep -n "Cli\|prefix\|embed\b" packages/evals/src/bin/semantic_quality.rs | head -30
```

Expected: 看到 `struct Cli`（clap derive）、`embed` 闭包定义（约 line 160-170 范围、与 spec §4.2 引用对齐）、现有 `--vectors-file` flag（BETA-15B-7 加）和 `--embed` flag。

- [ ] **Step 3.2: 写 failing 单测 1（默认值 = None）**

在 `mod tests` 块新加（如无 tests 块、参考 `vectors_file_flag_parses` / `vectors_file_defaults_to_vectors_json` 单测放法）：

```rust
#[test]
fn prefix_mode_defaults_to_none() {
    use clap::Parser;
    let cli = Cli::try_parse_from(["semantic-quality"]).unwrap();
    assert_eq!(cli.prefix_mode, PrefixMode::None);
}
```

- [ ] **Step 3.3: 写 failing 单测 2（standard 解析）**

```rust
#[test]
fn prefix_mode_standard_parses() {
    use clap::Parser;
    let cli = Cli::try_parse_from(["semantic-quality", "--prefix-mode", "standard"]).unwrap();
    assert_eq!(cli.prefix_mode, PrefixMode::Standard);
}
```

- [ ] **Step 3.4: 跑测试验证 fail**

```bash
cargo test -p locifind-evals --bin semantic-quality prefix_mode
```

Expected: 编译失败（`PrefixMode` 未定义 + `Cli.prefix_mode` 字段不存在）。

- [ ] **Step 3.5: 实现 PrefixMode enum + Cli 字段**

在 `Cli` struct 定义之前加：

```rust
#[derive(clap::ValueEnum, Clone, Debug, Default, PartialEq, Eq)]
enum PrefixMode {
    #[default]
    None,
    Standard,
}
```

在 `Cli` struct 内（与 `vectors_file` 等 flag 同段）加字段：

```rust
/// prefix 契约模式：none = 裸 embed、standard = EmbeddingGemma HF 卡 prefix 包装。
/// 默认 none 守 BETA-15B-10 及之前所有 cycle 向下兼容。
#[arg(long, value_enum, default_value_t = PrefixMode::default())]
prefix_mode: PrefixMode,
```

- [ ] **Step 3.6: 跑单测验证 PASS**

```bash
cargo test -p locifind-evals --bin semantic-quality prefix_mode
```

Expected: 2 passed。

- [ ] **Step 3.7: 实现 EmbedRole enum + 改 embed 闭包签名**

在 `Cli` struct 之后、`embed` 闭包之前加：

```rust
#[derive(Clone, Copy, Debug)]
enum EmbedRole {
    Query,
    Doc,
}
```

找到 `embed` 闭包定义（约 line 165 附近、形如 `let embed = |text: &str| -> Vec<f32> { ... };`）、改为接受 `role` 参数：

```rust
let prefix_mode = args.prefix_mode;
let embed = |text: &str, role: EmbedRole| -> Vec<f32> {
    let wrapped = match (prefix_mode, role) {
        (PrefixMode::None, _) => text.to_string(),
        (PrefixMode::Standard, EmbedRole::Query) => {
            format!("task: search result | query: {text}")
        }
        (PrefixMode::Standard, EmbedRole::Doc) => {
            format!("title: none | text: {text}")
        }
    };
    rt.embed(&wrapped).expect("embed 失败")
};
```

- [ ] **Step 3.8: 修改 embed 调用方传 role**

`grep` 出所有 `embed(` 闭包调用：

```bash
grep -n "embed(" packages/evals/src/bin/semantic_quality.rs | grep -v "// \|rt\.embed\|fn embed"
```

每个调用方根据语义传 role：
- corpus 遍历（生成 doc vector 的 for 循环）→ `embed(text, EmbedRole::Doc)`
- cases query 遍历（生成 query vector 的 for 循环）→ `embed(query_text, EmbedRole::Query)`

- [ ] **Step 3.9: cargo check + fmt + clippy**

```bash
cargo check -p locifind-evals --bin semantic-quality
cargo fmt --all --check
cargo clippy -p locifind-evals --all-targets -- -D warnings
```

Expected: 编译过、fmt 净、clippy 0 warning。如果有 unused import、按提示删。

- [ ] **Step 3.10: 跑全 binary tests**

```bash
cargo test -p locifind-evals --bin semantic-quality
```

Expected: 全过、含 prefix_mode 2 个新单测 + 既有单测（vectors_file_* 等）。

**注意：不 commit。Task 6 一并 C1 commit。**

---

## Task 4：本机实测 binding 是否识别 gemma-embedding（决策点、不 commit）

**Goal:** 跑一次 dry-run embed、验 llama-cpp-4 0.3.2 vendored llama.cpp 是否识别 `gemma-embedding` arch。决定是否触发 Task 5 应急升级。

**Spec ref:** §1.4 第 2 件 / §4.4

- [ ] **Step 4.1: 准备 dry-run 命令**

```bash
# 用 embeddinggemma + 一段最短的 corpus、看是否 load model 成功
cd /Users/alice/Work/LocalFind

# 最小 corpus dry-run（只 embed 一条、~5s）
cargo run --release -p locifind-evals --bin semantic-quality -- \
    --model models/embeddinggemma-300m-q8_0.gguf \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --vectors-file /tmp/beta-15b-11/dryrun.json \
    --prefix-mode none \
    --embed 2>&1 | tee /tmp/beta-15b-11/dryrun.log
```

Expected (成功路径)：
- log 含 `llama_model_load_from_file: loading model`、不报 `unknown model architecture`
- log 含 `LlamaPoolingType` 加载（来自 pooling.rs detect_pooling_type 路径）
- 推理开始、若 5-30s 内 worker 正常 spin up = arch 已识别（即使中途 Ctrl-C 也证明 load 过了 arch 阶段）

Expected (失败路径，Task 5 触发)：
- log 含 `unknown model architecture: 'gemma-embedding'` 或 `failed to load model` + 无 ggml_init log

- [ ] **Step 4.2: 决策点 — 是否触发 Task 5**

读 `/tmp/beta-15b-11/dryrun.log`：
- 若 arch 识别成功 → 跳过 Task 5、直接进 Task 6（C1 commit）
- 若 `unknown model architecture: 'gemma-embedding'` → 进 Task 5 应急升级

记录决策到 `/tmp/beta-15b-11/binding-decision.md`：

```bash
cat > /tmp/beta-15b-11/binding-decision.md <<EOF
# binding 识别决策

- 命令：见 Task 4 Step 4.1
- 日志：/tmp/beta-15b-11/dryrun.log
- 决策：<arch_recognized | arch_unknown_need_upgrade>
- 触发 Task 5 升级：<yes | no>
EOF
```

- [ ] **Step 4.3: 清理 dry-run 产物**

```bash
rm -f /tmp/beta-15b-11/dryrun.json
```

理由：dry-run 不入 fixtures、Task 7 才真正写 vectors-embeddinggemma-* 文件。

---

## Task 5（CONDITIONAL）：binding 升级应急

**触发条件:** Task 4 决策 = `arch_unknown_need_upgrade`。若 Task 4 决策 = `arch_recognized`，**跳过本 task**。

**Goal:** 升 `llama-cpp-4` 到最新支持 `gemma-embedding` arch 的版本、重 embed qwen3-0.6b + 主 vectors.json 跑语义等价闸（cos ≥ 0.9999 / max abs ≤ 1e-3）。

**Files:**
- Modify: `Cargo.toml`（workspace 中 llama-cpp-4 version）+ `Cargo.lock`（自动）
- Modify: `packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json`（重 embed、SHA256 可变）
- Modify: `packages/evals/fixtures/semantic-recall/vectors.json`（**主 vectors / bge-m3** 重 embed、SHA256 可变）

**Spec ref:** §4.4

- [ ] **Step 5.1: 查最新 llama-cpp-4 发布版**

```bash
cargo search llama-cpp-4 --limit 5
```

Expected: 看到 0.3.2 / 0.3.3 / 0.4.0 等候选。优先选**已发布的最高 0.3.x patch**、其次 0.4.x（major bump 风险更大）。

- [ ] **Step 5.2: 升 Cargo.toml + 编译**

```bash
# 编辑 workspace root Cargo.toml + 各 package（用 sed 或手改）
grep -rn 'llama-cpp-4 = \(\|{ version = \)"0.3.2"' Cargo.toml packages/*/Cargo.toml

# 假设升 0.3.3（按 Step 5.1 实际值替换）
# 用 sed 或 Edit 工具改 version
# 编译验证
cargo build --workspace
```

Expected: 编译过；若编译失败 = API breaking change、回滚版本试更低 patch。

- [ ] **Step 5.3: workspace test + clippy + fmt 净**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: 全过、0 warning。若失败 = binding upgrade 有 breaking change、需要 fix。

- [ ] **Step 5.4: 重 embed qwen3-0.6b**

```bash
cargo run --release -p locifind-evals --bin semantic-quality -- \
    --model models/qwen3-embedding-0.6b-q8_0.gguf \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json \
    --prefix-mode none \
    --embed
```

Expected: 全集 embed 完成、文件 SHA256 可变化。

- [ ] **Step 5.5: 重 embed 主 vectors.json（bge-m3）**

```bash
cargo run --release -p locifind-evals --bin semantic-quality -- \
    --model models/bge-m3-q8_0.gguf \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors.json \
    --prefix-mode none \
    --embed
```

Expected: 81 query + 127 doc embed 完成、SHA256 可变（与 BETA-15B-10 v5 状态 `4f0de346...` 不同）。

- [ ] **Step 5.6: 语义等价闸（cos ≥ 0.9999 + max abs ≤ 1e-3）**

写一个临时 Python 脚本验证（参考 BETA-15B-9 Phase 0 同款做法）：

```bash
git stash  # 临时 stash 升级 vectors 看 git 旧版本
git show HEAD:packages/evals/fixtures/semantic-recall/vectors.json > /tmp/beta-15b-11/vectors-pre-upgrade.json
git show HEAD:packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json > /tmp/beta-15b-11/vectors-qwen3-0.6b-pre-upgrade.json
git stash pop

python3 - <<'EOF'
import json, math
def load(p):
    with open(p) as f: return json.load(f)
def cos_min_maxabs(a_path, b_path):
    a = load(a_path); b = load(b_path)
    a_vecs = a["vectors"] if "vectors" in a else a  # 兼容两种结构
    b_vecs = b["vectors"] if "vectors" in b else b
    cos_min = 1.0; max_abs = 0.0
    for k, va in a_vecs.items():
        vb = b_vecs.get(k)
        if vb is None: continue
        # cos
        dot = sum(x*y for x, y in zip(va, vb))
        na = math.sqrt(sum(x*x for x in va)); nb = math.sqrt(sum(x*x for x in vb))
        c = dot / (na * nb) if na > 0 and nb > 0 else 0.0
        cos_min = min(cos_min, c)
        # max abs
        for x, y in zip(va, vb):
            max_abs = max(max_abs, abs(x - y))
    print(f"cos_min={cos_min:.6f}  max_abs={max_abs:.6e}")
    print(f"PASS: {cos_min >= 0.9999 and max_abs <= 1e-3}")

print("=== bge-m3 主 vectors ===")
cos_min_maxabs("/tmp/beta-15b-11/vectors-pre-upgrade.json",
               "packages/evals/fixtures/semantic-recall/vectors.json")
print("=== qwen3-0.6b reference vectors ===")
cos_min_maxabs("/tmp/beta-15b-11/vectors-qwen3-0.6b-pre-upgrade.json",
               "packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json")
EOF
```

Expected: 两组都 `PASS: True`（cos_min ≥ 0.9999、max_abs ≤ 1e-3）。

**异常处理**：若 PASS=False（语义等价闸破）→ 回滚升级（`git checkout Cargo.toml Cargo.lock packages/evals/fixtures/semantic-recall/vectors*.json`）、走 spec §5.2 Branch IV-infra 路径：file upstream issue draft 入仓、停 cycle、走 doc-sync 收口（与 BETA-15B-9 同款节奏）。

- [ ] **Step 5.7: 跑 gate 守 v5 baseline 不退步**

```bash
cargo test -p locifind-evals --test semantic_quality_gate
```

Expected: 1 passed（4 红线动态读 baseline、本 cycle 不改 baseline.json、升级后语义等价 → 数值微调内不应破红线）。

- [ ] **Step 5.8: 记录升级决策**

```bash
cat >> /tmp/beta-15b-11/binding-decision.md <<EOF

## 升级执行（Step 5.x）
- 从：0.3.2
- 到：<填新版本>
- vectors.json SHA256 变：<old> → <new>
- vectors-qwen3-0.6b.json SHA256 变：<old> → <new>
- 语义等价闸：PASS（cos_min=<>, max_abs=<>）
- gate test：PASS
EOF
```

**注意：不 commit。Task 6 一并 C1 commit。**

---

## Task 6：C1 commit（infra de-risk + flag prep + 可能含 binding 升级）

**Goal:** 把 Task 2 + 3 + (5) 改动一次 commit 落库；不含 GGUF 下载（Task 1 不入仓）+ 不含 vectors-embeddinggemma-* 文件（Task 7 才产）。

- [ ] **Step 6.1: 看本 task 改动覆盖**

```bash
git diff --stat
git status
```

Expected:
- `packages/model-runtime/src/pooling.rs`（Task 2）
- `packages/evals/src/bin/semantic_quality.rs`（Task 3）
- 若 Task 5 触发：`Cargo.toml` / `Cargo.lock` / `packages/evals/fixtures/semantic-recall/vectors.json` / `packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json`

- [ ] **Step 6.2: 全 workspace 验证门**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: 全过、0 warning、fmt 净。

- [ ] **Step 6.3: parser-only byte-equal verify（红线 6）**

```bash
cargo run --release -p locifind-evals -- v05 > /tmp/beta-15b-11/v05-now.json
git stash
cargo run --release -p locifind-evals -- v05 > /tmp/beta-15b-11/v05-main.json
git stash pop
diff /tmp/beta-15b-11/v05-main.json /tmp/beta-15b-11/v05-now.json
# v0.9 同款
cargo run --release -p locifind-evals -- v09 > /tmp/beta-15b-11/v09-now.json
git stash
cargo run --release -p locifind-evals -- v09 > /tmp/beta-15b-11/v09-main.json
git stash pop
diff /tmp/beta-15b-11/v09-main.json /tmp/beta-15b-11/v09-now.json
```

Expected: 两 diff 都空 = byte-equal。

**注**：BETA-15B-10 v5 baseline 已规范化 byte-equal 比对法（reporter JSON 非确定输出问题）。若 reporter JSON 含 elapsed_ms / HashMap key 顺序 → 用 `--json` + jq 排序后再 diff。具体参考 [记忆 project-evals-reporter-nondeterministic](../../../../.claude/projects/-Users-roger-Work-LocalFind/memory/project_evals_reporter_nondeterministic.md) 或 BETA-15B-10 plan Task 7 节同款脚本。

- [ ] **Step 6.4: commit C1**

```bash
git add packages/model-runtime/src/pooling.rs \
        packages/evals/src/bin/semantic_quality.rs
# 若 Task 5 触发
git add Cargo.toml Cargo.lock \
        packages/evals/fixtures/semantic-recall/vectors.json \
        packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json
git status
```

Expected: staged changes 清晰、无 untracked（GGUF 自动 gitignored）。

Commit message（不含升级 = 50 字以内、含升级 = 50 字内可接受）：

```bash
# 不含升级
git commit -m "BETA-15B-11 C1：gemma-embedding pooling + prefix-mode flag"
# 含升级（语义等价闸 PASS、参考 BETA-15B-9 节奏）
git commit -m "BETA-15B-11 C1：升 llama-cpp-4 + gemma-embedding pooling + prefix-mode"
```

Expected: commit hash 记入 `/tmp/beta-15b-11/commits.md`。

---

## Task 7：Mac Metal --embed 双 mode 全集（产 vectors-embeddinggemma-*.json）

**Goal:** 跑两次全集 embed、产 `vectors-embeddinggemma-300m-no-prefix.json` + `vectors-embeddinggemma-300m-prefix.json`。

**Files:**
- Create: `packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json`
- Create: `packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json`

**Spec ref:** §3.1 #4 + #5、§5.3 推理短路异常处理

- [ ] **Step 7.1: 跑 prefix-mode=none 全集**

```bash
time cargo run --release -p locifind-evals --bin semantic-quality -- \
    --model models/embeddinggemma-300m-q8_0.gguf \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json \
    --prefix-mode none \
    --embed 2>&1 | tee /tmp/beta-15b-11/embed-no-prefix.log
```

Expected:
- 推理耗时 ≥ 30s（spec §5.3 短路红线 < 10s = Branch IV 触发）
- log 含 `LlamaPoolingType::Mean`（来自 pooling.rs gemma-embedding 分支）
- 文件创建、size 合理（81 query + 127 doc × 768 dim × ~4 bytes ≈ 600 KB ± 50%）

**异常处理**：若 < 10s 全部 embed 完 → STOP、走 spec §5.3 / Branch IV-infra；查看 vectors 文件 L2 norm 分布（参考 BETA-15B-9 qwen3-8b 全零诊断脚本）。

- [ ] **Step 7.2: 跑 prefix-mode=standard 全集**

```bash
time cargo run --release -p locifind-evals --bin semantic-quality -- \
    --model models/embeddinggemma-300m-q8_0.gguf \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json \
    --prefix-mode standard \
    --embed 2>&1 | tee /tmp/beta-15b-11/embed-prefix.log
```

Expected: 同 Step 7.1、推理耗时同量级。

- [ ] **Step 7.3: 红线 8 — check_vectors（schema + dim 抽样验）**

```bash
# 让 sweep 命令隐式跑 check_vectors（不真 sweep、用 --help 或 no-op flag）
# 备选：直接 jq 看
jq '.dim, .vectors | length' packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json
jq '.dim, .vectors | length' packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json
```

Expected:
- `dim` 字段（若 vectors.json schema 含 dim 字段）= 768
- `.vectors | length` 与 cases + corpus 总数对齐（应为 81 + 127 = 208 或类似）

如 jq path 不对，参考 BETA-15B-7 Task 3 同款 check_vectors 跑法。

- [ ] **Step 7.4: 红线 10 — 双 mode vectors 区分性 sanity check**

```bash
sha256sum packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-*.json
```

Expected: 两文件 SHA256 NOT equal（prefix 真包到 query/doc）。

**异常处理**：若 SHA256 equal → 走 spec §5.4：检 `embed` 闭包是否真按 PrefixMode 包 prefix、是否 EmbedRole 区分错位、修后重 Task 7。

- [ ] **Step 7.5: L2 norm 抽样验**

```bash
python3 - <<'EOF'
import json, math
for p in ["vectors-embeddinggemma-300m-no-prefix.json", "vectors-embeddinggemma-300m-prefix.json"]:
    full_p = f"packages/evals/fixtures/semantic-recall/{p}"
    with open(full_p) as f: data = json.load(f)
    vecs = data["vectors"] if "vectors" in data else data
    keys = list(vecs.keys())[:5]  # 前 5 抽样
    print(f"\n=== {p} ===")
    print(f"total: {len(vecs)}")
    for k in keys:
        v = vecs[k]
        norm = math.sqrt(sum(x*x for x in v))
        print(f"  {k} dim={len(v)} L2={norm:.4f}")
EOF
```

Expected:
- 两文件 total = 208（cases 81 + corpus 127）
- 每个 vector dim = 768
- L2 norm 抽样 ≈ 1.0（绝大多数 embedding 模型默认输出已 L2-normalized）；若 ≠ 1.0、记录但不阻塞 sweep（HYBR sweep 用 cosine、归一与否对排序无影响）

- [ ] **Step 7.6: 记录 SHA256 + 体量**

```bash
cat >> /tmp/beta-15b-11/gguf-metadata.md <<EOF

## vectors-embeddinggemma-300m-no-prefix.json
- SHA256: $(sha256sum packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json | awk '{print $1}')
- Size: $(stat -f%z packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json) bytes
- inference time: <填 Step 7.1 time 输出>

## vectors-embeddinggemma-300m-prefix.json
- SHA256: $(sha256sum packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json | awk '{print $1}')
- Size: $(stat -f%z packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json) bytes
- inference time: <填 Step 7.2 time 输出>
EOF
```

**注意：不 commit。Task 9 才 C2 commit vectors 入仓。**

---

## Task 8：9 阈值 × 2 mode sweep + Branch 决策（不 commit）

**Goal:** 跑 sweep 矩阵 = 9 阈值 × 2 prefix mode = 18 次、人工读决策表选 Branch I-a / I-b / II / III / IV。

**Spec ref:** §2.3 GO 判定 + §6 T4-T5

- [ ] **Step 8.1: 跑 9 阈值 sweep — prefix-mode=none**

```bash
cargo run --release -p locifind-evals --bin semantic-quality -- \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --sweep-cosine-thresholds 0.0,0.30,0.45,0.60,0.70,0.80,0.90,0.99,1.01 \
    2>&1 | tee /tmp/beta-15b-11/sweep-no-prefix.log
```

Expected: 9 阈值 × 6 桶（synonym / concept / crosslang / content-not-name / exact-name / OVERALL）× 3 臂（VEC_N / HYB / HYBR）的 sweep 表打印。参考 BETA-15B-10 plan Task 5 同款 sweep flag 名（若 flag 名不对、用 `--help` 看实际命令）。

- [ ] **Step 8.2: 跑 9 阈值 sweep — prefix-mode=standard**

```bash
cargo run --release -p locifind-evals --bin semantic-quality -- \
    --vectors-file packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json \
    --corpus packages/evals/fixtures/semantic-recall/corpus.json \
    --cases packages/evals/fixtures/semantic-recall/cases.json \
    --sweep-cosine-thresholds 0.0,0.30,0.45,0.60,0.70,0.80,0.90,0.99,1.01 \
    2>&1 | tee /tmp/beta-15b-11/sweep-prefix.log
```

Expected: 同款。

- [ ] **Step 8.3: 控制对照核验（§2.4）**

读 `sweep-no-prefix.log` 和 `sweep-prefix.log`：
- T=0.0 时 HYBR_OVERALL ≈ VEC_OVERALL（六桶 HYBR_N 与 VEC_N 相等）
- T=1.01 时 HYBR_OVERALL ≡ HYB_OVERALL（六桶完全相等）

若控制对照破 → STOP、可能 sweep 命令 / vectors 文件错位、查后重跑。

- [ ] **Step 8.4: 整合决策矩阵**

写决策矩阵 markdown：

```bash
cat > /tmp/beta-15b-11/sweep-matrix.md <<'EOF'
# BETA-15B-11 sweep 矩阵（embeddinggemma-300m）

## prefix-mode=none

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 | ? | ? | ? | ? |
| 0.30 | ? | ? | ? | ? |
| 0.45 | ? | ? | ? | ? |
| 0.60 | ? | ? | ? | ? |
| 0.70 | ? | ? | ? | ? |
| 0.80 | ? | ? | ? | ? |
| 0.90 | ? | ? | ? | ? |
| 0.99 | ? | ? | ? | ? |
| 1.01 | ? | ? | ? | ? |

## prefix-mode=standard

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 | ? | ? | ? | ? |
| ... | ... | ... | ... | ... |

## 控制对照
- T=0.0 时 HYBR ≈ VEC：<PASS / FAIL>
- T=1.01 时 HYBR ≡ HYB：<PASS / FAIL>

## v5 bge-m3 baseline 对比锚
| | OVERALL | crosslang | content-not-name | exact-name |
|---|---|---|---|---|
| v5 bge-m3 T=0.70 | 0.864 | 0.686 | 0.869 | 1.000 |
| spec 字面 | ≥ 0.864 | ≥ 0.700 | — | = 1.000 |
EOF
```

从 sweep log 抽数值填入。

- [ ] **Step 8.5: Branch 决策**

按 spec §2.3 表读决策矩阵：

```bash
cat > /tmp/beta-15b-11/branch-decision.md <<EOF
# BETA-15B-11 Branch 决策

## prefix-mode=none 字面达成
- OVERALL ≥ 0.864：<YES / NO>
- crosslang ≥ 0.700：<YES / NO>

## prefix-mode=standard 字面达成
- OVERALL ≥ 0.864：<YES / NO>
- crosslang ≥ 0.700：<YES / NO>

## 各桶不退步 v5 baseline 0.6x（gate (4b)）
- prefix-mode=none：<PASS / FAIL>（哪桶 FAIL）
- prefix-mode=standard：<PASS / FAIL>

## 决策
- Branch：<I-a / I-b / II / III / IV>
- 理由：<简短说明>
- 下 cycle 抓手：<follow-up bake / 评测扩量 + jina-v3 spike / 评测扩量 + bge-multilingual-gemma2 / file upstream issue>
EOF
```

**注**：决策需 coordinator 拍板（即用户 or 主会话）；subagent-driven mode 下、本 task 输出决策矩阵 + 推荐 Branch、最终选 Branch 由主会话拍板。

---

## Task 9：C2 commit（vectors-embeddinggemma-* 入仓）

**Goal:** 把 Task 7 产的两个 vectors 文件入仓、不动 baseline.json / 不动其他 fixture。

- [ ] **Step 9.1: stage 新增 vectors 文件**

```bash
git add packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-no-prefix.json \
        packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-prefix.json
git status
```

Expected: 仅 2 个新文件 staged。无 fixture 既有文件改动（corpus.json / cases.json / vectors.json / baseline.json / vectors-bge-m3.json / vectors-qwen3-0.6b.json 都不动）。

- [ ] **Step 9.2: 验红线 6 + 7 byte-equal**

```bash
# parser-only byte-equal v0.5/v0.9 见 Task 6 Step 6.3 同款脚本
# fixture SHA256（parser-rs / v0.5 / v0.9 不动验）
sha256sum packages/parser-rs/fixtures/*.json packages/evals/fixtures/v0.5/*.json packages/evals/fixtures/v0.9/*.json | tee /tmp/beta-15b-11/fixture-sha256-after.txt

git stash
sha256sum packages/parser-rs/fixtures/*.json packages/evals/fixtures/v0.5/*.json packages/evals/fixtures/v0.9/*.json | tee /tmp/beta-15b-11/fixture-sha256-main.txt
git stash pop

diff /tmp/beta-15b-11/fixture-sha256-main.txt /tmp/beta-15b-11/fixture-sha256-after.txt
```

Expected: 两 SHA256 表完全一致（diff 空）。

- [ ] **Step 9.3: commit C2**

```bash
git commit -m "BETA-15B-11 C2：vectors-embeddinggemma-300m 双 mode 入仓"
```

Expected: commit hash 记入 `/tmp/beta-15b-11/commits.md`。

---

## Task 10：baseline 报告 v6 节 + README v6 + commit C3

**Goal:** [`docs/reviews/semantic-recall-quality-baseline.md`](../../docs/reviews/semantic-recall-quality-baseline.md) 追加 v6 节、`packages/evals/fixtures/semantic-recall/README.md` 升 v6 段。

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（追加 v6 节、不重写 v5）
- Modify: `packages/evals/fixtures/semantic-recall/README.md`（追加 v6 版本说明）

**Spec ref:** §3.1 #7 + #8

- [ ] **Step 10.1: 读 v5 节作模板**

```bash
sed -n '/^### v5 数据集节/,/^### v[6-9]\|^## /p' docs/reviews/semantic-recall-quality-baseline.md | head -100
```

Expected: 看 v5 节结构（背景 + 数据集 + sweep 全表 + spec §2.2 红线核对 + ROI + 诚实边界 + vectors.json SHA256 + 下 cycle 抓手 + 链接）作 v6 节同款结构模板。

- [ ] **Step 10.2: 写 v6 节**

在 v5 节之后追加 v6 节（标题 `### v6 数据集节 — BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验`）。沿用 v5 同款结构：

1. **承接**：v5 留下的 crosslang 字面 -0.014 gap、本 cycle 用 EmbeddingGemma-300M 单点 + 双 prefix mode 对照冲字面
2. **dataset**：81 cases / 127 docs / dim 768 / 模型 embeddinggemma-300m / pooling = Mean / context = 2048
3. **infra de-risk 落地**：pooling.rs 加 `"gemma-embedding" => Mean` + binding `<原版本> → <若升级填新版本>`
4. **prefix 契约**：标准 query 包 `task: search result | query: ...` / doc 包 `title: none | text: ...`
5. **sweep 全表**（从 Task 8 sweep-matrix.md 抄过来、prefix-mode=none + prefix-mode=standard 两张表）
6. **控制对照核验**
7. **spec §2.2 红线核对**（at sweep best T）
8. **vs v5 bge-m3 baseline 对照表**（OVERALL / crosslang / content-not-name / exact-name 四指标 × 两 mode）
9. **Branch 决策**（从 Task 8 branch-decision.md 抄）
10. **诚实边界**（若 Branch I-a/I-b/II/III/IV、对应说理）
11. **prefix 契约价值数据指证**：标准 prefix vs 裸 mode 在每桶上的 delta（这是本 cycle 独有的子命题贡献）
12. **下 cycle 抓手优先级（v6 数据指证）**：根据 Branch 实际命中、给 follow-up cycle 推荐
13. **vectors-embeddinggemma-300m-{no-prefix,prefix}.json**：SHA256 + 体量 + 推理时长（从 /tmp/beta-15b-11/gguf-metadata.md 抄）
14. **链接**：本 cycle spec / plan + GGUF 仓库 + EmbeddingGemma HF 卡

- [ ] **Step 10.3: 更新 fixtures/README.md**

[`packages/evals/fixtures/semantic-recall/README.md`](../../packages/evals/fixtures/semantic-recall/README.md) 现 v5 段之后追加 v6 段（沿 v5 同款风格）：

- v6 = 81 cases / 127 docs（与 v5 同结构、本 cycle 不扩 case）
- vectors-embeddinggemma-300m-no-prefix.json / vectors-embeddinggemma-300m-prefix.json 入仓（新增 reference snapshot、不替换 vectors.json 主文件）
- prefix mode 含义说明
- 链到 baseline 报告 v6 节

- [ ] **Step 10.4: commit C3**

```bash
git add docs/reviews/semantic-recall-quality-baseline.md \
        packages/evals/fixtures/semantic-recall/README.md
git status
git commit -m "BETA-15B-11 C3：baseline 报告 v6 节 + fixtures README v6"
```

Expected: commit hash 记入 `/tmp/beta-15b-11/commits.md`。

---

## Task 11：总验收红线 1-10 全过（不 commit）

**Goal:** 把 spec §2.1 红线 1-10 全套跑一遍、produce 验证证据落 `/tmp/beta-15b-11-verification-evidence.txt`。

**Spec ref:** §2.1

- [ ] **Step 11.1: 红线 1-3 + 5（fmt + clippy + workspace test + desktop tsc）**

```bash
{
    echo "=== 红线 1: rustfmt ==="
    cargo fmt --all --check && echo "PASS" || echo "FAIL"

    echo "=== 红线 2: clippy ==="
    cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
    cargo clippy --workspace --all-targets -- -D warnings && echo "PASS" || echo "FAIL"

    echo "=== 红线 3: workspace test ==="
    cargo test --workspace 2>&1 | tail -5

    echo "=== 红线 5: desktop tsc + build ==="
    cd apps/desktop && npm run typecheck && npm run build
    cd ../..
} | tee /tmp/beta-15b-11-verification-evidence.txt
```

Expected: 全 PASS、tail 5 行 workspace test 显示 `X passed; 0 failed`。

- [ ] **Step 11.2: 红线 4（gate 守 v5 baseline 不退步）**

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tee -a /tmp/beta-15b-11-verification-evidence.txt
```

Expected: `1 passed`。

- [ ] **Step 11.3: 红线 6（parser-only byte-equal）**

参考 Task 6 Step 6.3 / Task 9 Step 9.2 同款命令、把输出追加 `/tmp/beta-15b-11-verification-evidence.txt`。

Expected: v0.5 + v0.9 diff 空。

- [ ] **Step 11.4: 红线 7（fixture SHA256）**

```bash
{
    echo "=== 红线 7: fixture SHA256 ==="
    sha256sum packages/parser-rs/fixtures/*.json \
              packages/evals/fixtures/v0.5/*.json \
              packages/evals/fixtures/v0.9/*.json
    echo "=== semantic-recall fixture（含本 cycle 新 vectors）==="
    sha256sum packages/evals/fixtures/semantic-recall/*.json
} | tee -a /tmp/beta-15b-11-verification-evidence.txt
```

Expected: parser-rs / v0.5 / v0.9 SHA256 = main；semantic-recall 仅新增 vectors-embeddinggemma-300m-no-prefix.json + vectors-embeddinggemma-300m-prefix.json + 若 Task 5 触发则 vectors.json 和 vectors-qwen3-0.6b.json SHA256 变。

- [ ] **Step 11.5: 红线 8-10（vectors schema / 语义等价 / prefix 区分性）**

```bash
{
    echo "=== 红线 8: vectors schema（Task 7 Step 7.3-7.5 已验、抄过来）==="
    cat /tmp/beta-15b-11/gguf-metadata.md
    echo "=== 红线 9: 若升级 binding 跑语义等价闸（Task 5 Step 5.6 已验）==="
    cat /tmp/beta-15b-11/binding-decision.md 2>/dev/null || echo "Task 5 未触发"
    echo "=== 红线 10: 双 mode vectors 区分性（Task 7 Step 7.4 已验）==="
    sha256sum packages/evals/fixtures/semantic-recall/vectors-embeddinggemma-300m-{no-prefix,prefix}.json
} | tee -a /tmp/beta-15b-11-verification-evidence.txt
```

Expected: 全 PASS（红线 9 仅 Task 5 触发时填）。

- [ ] **Step 11.6: 总结**

```bash
{
    echo
    echo "=== 总验收：BETA-15B-11 ==="
    echo "10 / 10 红线全过"
    echo "Branch：$(grep 'Branch：' /tmp/beta-15b-11/branch-decision.md)"
} | tee -a /tmp/beta-15b-11-verification-evidence.txt
```

Expected: 验证证据完整落库 /tmp/beta-15b-11-verification-evidence.txt。

---

## Task 12：STATUS + ROADMAP doc-sync + commit C4 + push branch + PR + 合 main

**Goal:** doc-sync 收尾、commit C4、push、PR、合 main。沿 BETA-15B-10 同款流程。

**Files:**
- Modify: `STATUS.md`（顶部「当前 Task」+ 顶部「会话日志」追加一段）
- Modify: `ROADMAP.md`（B6 段加 BETA-15B-11 task 卡片、状态 = done）

**Spec ref:** §6 T8-T9

- [ ] **Step 12.1: 更新 STATUS.md「当前 Task」+「下一步」+「会话日志」**

- 「当前 Task」节：替换为 `BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验 = done`、含 Branch / 关键数据 / merge commit hash 占位
- 「下一步」节：根据 Branch 决策更新 follow-up 抓手优先级；保留其他下一步项
- 「会话日志」顶部追加新段（沿 BETA-15B-10 同款格式）

- [ ] **Step 12.2: 更新 ROADMAP.md**

ROADMAP §3.3 B6 段加 BETA-15B-11 task 卡片（参考 BETA-15B-10 / BETA-15B-7 同款 task 卡片模板）。状态 = done。

- [ ] **Step 12.3: 滚动归档检查**

```bash
grep -c "^### " STATUS.md
```

Expected: 会话日志条数。若 > 10 → 把最旧的几条剪到 `docs/session-logs/STATUS-archive-2026-06.md`（CONVENTIONS §3 step 1）。

- [ ] **Step 12.4: commit C4**

```bash
git add STATUS.md ROADMAP.md docs/session-logs/STATUS-archive-2026-06.md
git status
git commit -m "BETA-15B-11 doc-sync：STATUS + ROADMAP v6"
```

Expected: commit hash 记入 `/tmp/beta-15b-11/commits.md`。

- [ ] **Step 12.5: push branch + 开 PR**

```bash
git push -u origin feat-beta-15b-11-embeddinggemma-prefix-probe
gh pr create --title "BETA-15B-11：EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验" \
             --body-file /tmp/beta-15b-11/pr-body.md
```

PR body（写到 /tmp/beta-15b-11/pr-body.md）参考 BETA-15B-10 PR #17 同款结构：cycle 概况 / 关键数据 / Branch 决策 / 改动文件 / 红线核对 / 链接。

- [ ] **Step 12.6: 合 main**

```bash
gh pr merge <PR#> --merge --delete-branch
# 若 gh CLI 401（凭据问题）→ 走 BETA-15B-9 同款本地 merge fallback：
# git checkout main && git pull && git merge --no-ff feat-... -m "..." && git push origin main && git push origin --delete feat-...
```

记录 merge commit hash 到 /tmp/beta-15b-11/commits.md。

- [ ] **Step 12.7: 回填 STATUS / baseline 报告 merge commit hash**

```bash
# 在 STATUS.md / baseline 报告 v6 节中、把 「PR # 待回填、merge commit 待回填」替换为实际 PR # + commit hash
# 用 sed 或 Edit
```

- [ ] **Step 12.8: 收尾 commit（仅 hash 回填，可选）**

```bash
git add STATUS.md docs/reviews/semantic-recall-quality-baseline.md
git diff --staged
git commit -m "BETA-15B-11 收尾：回填 PR # + merge hash"
git push origin main
```

Expected: 仓库已落 BETA-15B-11 全套改动、main HEAD 包含本 cycle 5 commit + merge commit、远程 feature branch 已删。

---

## Self-Review（写完 plan 后做、不入交付）

**1. Spec coverage 检查**

| Spec § | Task 覆盖 |
|---|---|
| §1 背景动机 | Plan 头部 Goal/Architecture + Task 0/1 上下文 |
| §2.1 验证门 红线 1-10 | Task 6 / 9 / 11 全覆盖 |
| §2.2 接受标准 (4a)-(4d) | gate 自锁、Task 6 / 11 验 |
| §2.3 GO 判定四 Branch 表 | Task 8 决策矩阵 |
| §2.4 控制对照不变性 | Task 8 Step 8.3 |
| §3 范围（YAGNI）| Task 全覆盖、不动文件均明示 |
| §4.1 pooling.rs 扩 | Task 2（TDD）|
| §4.2 --prefix-mode flag | Task 3（TDD）|
| §4.3 GGUF 下载 + metadata | Task 1 |
| §4.4 binding 升级应急 | Task 4 决策 + Task 5 conditional |
| §5 异常处理 | 各 Task 异常处理段落 + 5.3 短路 in Task 7 |
| §6 操作清单 T0-T9 | Task 0-12 对齐 |
| §7 真机手测策略 | 平凡未安排（合 PR body 即可）|

无 Gap。

**2. Placeholder scan**：plan 内无 TBD / TODO / 「填后续 step」/ 空泛「适当处理 error」类占位词。Task 8 / 10 / 12 有 `<填>` 字段、是给 Task 实际执行时填入真实数据、不是 plan 占位。

**3. Type consistency**：
- `PrefixMode::{None, Standard}`：Task 3 定义、Task 6 / 7 调用一致
- `EmbedRole::{Query, Doc}`：Task 3 定义、Task 3 / 7 调用一致
- `default_pooling_for_arch`：Task 2 改、与 spec §4.1 引用 line 32-40 一致
- vectors-embeddinggemma-300m-{no-prefix,prefix}.json：Task 7 / 9 / 10 / 11 一致

无 type / 命名不一致。
