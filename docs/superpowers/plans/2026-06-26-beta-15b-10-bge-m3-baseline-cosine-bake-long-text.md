# BETA-15B-10 bge-m3 baseline 重锚 + cosine_threshold sweep & bake + 评测集长文本扩量 + evals embed 截断解除 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 LociFind 评测层（baseline.json + cosine_threshold）从守 qwen3-0.6b 彻底重锚到 bge-m3 真水位 + 评测合成集首次覆盖 > 512 token 长文本 case + 解除 [`packages/evals/src/bin/semantic_quality.rs:165`](../../../packages/evals/src/bin/semantic_quality.rs#L165) 的 1200 char 字符截断（让 evals embed 路径与 desktop indexer 真实路径对齐）。

**Architecture:** 3 commit 一刀切（spec §8 方案 B）—— **C1**：corpus + cases + vectors + 截断解除（数据扩量、行为零变）；**C2**：cosine_threshold bake + baseline.json rewrite + gate.rs 注释升 v5（融合参数 bake 与 baseline 锚移一次性绑定避免中间状态 gate FAIL）；**C3**：doc-sync（baseline 报告 v5 节 + fixtures README v5 + STATUS + ROADMAP）。不动 desktop wiring / model-runtime / result-normalizer 其他 DEFAULT_* 常量 / wrapper API。

**Tech Stack:** Rust 1.x / `llama-cpp-4 = 0.3.2`（已 BETA-15B-9 升级）/ Mac Metal Q8_0 推理 / `bge-m3-Q8_0.gguf` 605 MB（已 BETA-15B-7-v2 切到桌面默认）/ superpowers subagent-driven workflow.

**Spec:** [docs/superpowers/specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md](../specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md)

**Cycle 范围**（spec §3）：① 解除 evals embed 1200 char 截断（1 行删除）；② cases.json + corpus.json 扩 3 条长文本 case；③ 重 embed vectors.json；④ bake cosine_threshold；⑤ rewrite baseline.json；⑥ gate.rs 注释升 v5；⑦ baseline 报告 v5 节；⑧ fixtures README v5；⑨ STATUS + ROADMAP doc-sync。**不动**：desktop wiring / model-runtime / result-normalizer 其他 DEFAULT_* / wrapper API / RouteVerdict / FanoutOutcome / qwen3-0.6b/v4-fixup snapshot 文件。

**关键 spec 已修订**（v1 → v2）：发现 evals embed 截断后从「防 panic」改 framing 为「让 sweep 出的 cosine_threshold 在长文本场景有真实数据支持 + evals embed 路径与 desktop indexer 行为对齐」、spec §1.2 / §3.1 row 1 / §4.1 / §7.5 已落地、plan T1 实施该改动。

**前置**：当前分支 `feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text` 已开（spec commit `e744b34` 已落）；Mac Metal 环境就绪（`models/bge-m3-q8_0.gguf` 605 MB 已下载、SHA256 `950f4a8e...`）；`feature semantic-recall` 已开（BETA-15B-1 起就在）。

---

## Task 0：开 cycle 预检（不 commit、几秒）

**Files:** 无（只读）

**说明**：subagent 起手前 sanity check 当前分支 / git tree clean / 模型文件就位 / spec 已 commit。

### Step 0.1：确认分支与 git tree 状态

- [ ] 跑：

```bash
git rev-parse --abbrev-ref HEAD
```

Expected：`feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text`

- [ ] 跑：

```bash
git status --short
```

Expected：空输出（tree clean）

- [ ] 跑：

```bash
git log --oneline -3
```

Expected：最新 commit 是 `e744b34 BETA-15B-10 spec：bge-m3 baseline 重锚 + cosine_threshold sweep & bake + ...`

### Step 0.2：确认模型文件就位

- [ ] 跑：

```bash
ls -lh models/bge-m3-q8_0.gguf
```

Expected：文件存在、约 605 MB（`-rw-... 605M ...`）。

- [ ] 若文件不存在或非 605 MB、**停止并报告**：模型需重新下载（参考 BETA-15B-7 plan T1 aria2c 路径、HF token + license accept 流程）。

### Step 0.3：确认 spec 在仓库内可解析

- [ ] 跑：

```bash
test -f docs/superpowers/specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md && echo OK
```

Expected：`OK`

---

## Task 1：解除 evals embed 1200 char 截断（TDD）

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs:163-167`
- Test: `packages/evals/src/bin/semantic_quality.rs`（同文件 `#[cfg(test)] mod cli_tests`、无新单测、依赖现有覆盖 + 集成验证）

**说明**：删 `let t: String = text.chars().take(1200).collect();` 一行 + 把 `rt.embed(&t)` 改为 `rt.embed(text)`。改动域仅在 `embed_and_write` 闭包内、不动 CLI flag / 不动 `Cli` struct / 不动 `embed_and_write` 签名。**TDD 不适用**（无新逻辑需测、改动是「移除字符串截断」的等价化简、依赖 T3 集成 embed 验证）。沿 spec §4.1。

### Step 1.1：定位当前截断代码

- [ ] 跑：

```bash
sed -n '163,168p' packages/evals/src/bin/semantic_quality.rs
```

Expected 输出含：

```rust
    let embed = |text: &str| -> Vec<f32> {
        let t: String = text.chars().take(1200).collect();
        rt.embed(&t).expect("embed 失败")
    };
```

### Step 1.2：用 Edit tool 删截断行 + 改 embed 调用

- [ ] 改 `packages/evals/src/bin/semantic_quality.rs:163-167`：

```rust
    let embed = |text: &str| -> Vec<f32> {
        rt.embed(text).expect("embed 失败")
    };
```

（去掉 `let t: String = ...` 行、`rt.embed(&t)` 改为 `rt.embed(text)`）

### Step 1.3：跑 cargo check 验证编译过

- [ ] 跑：

```bash
cargo check -p locifind-evals --features semantic-recall --bin semantic_quality
```

Expected：`Finished` 无 error。

### Step 1.4：跑 clippy 验证 0 warning

- [ ] 跑：

```bash
cargo clippy -p locifind-evals --features semantic-recall --bin semantic_quality --all-targets -- -D warnings
```

Expected：`Finished` 无 warning。

### Step 1.5：跑 fmt 检查

- [ ] 跑：

```bash
cargo fmt --all --check
```

Expected：无输出（净）。

---

## Task 2：cases.json + corpus.json 扩 3 条长文本 case

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/cases.json`（+3 case：c079 / c080 / c081）
- Modify: `packages/evals/fixtures/semantic-recall/corpus.json`（+3 doc：s00125 / s00126 / s00127）

**说明**：沿 spec §4.2 / §4.3 + BETA-15B-6 v2/v3 同款约定（全虚构 + 零 PII + 主题与 case 对齐）。token 数验证留 T3（embed log 抽）。

### Step 2.1：读现有 cases.json 末尾确认 schema

- [ ] 跑：

```bash
tail -30 packages/evals/fixtures/semantic-recall/cases.json
```

Expected：JSON array 结尾、最后一个 case `c078` 含 `id / query / lang / bucket / expected_top_doc_ids / comment` 字段。

### Step 2.2：读现有 corpus.json 末尾确认 schema + 现有 doc 数

- [ ] 跑：

```bash
tail -30 packages/evals/fixtures/semantic-recall/corpus.json
jq '. | length' packages/evals/fixtures/semantic-recall/corpus.json
jq '. | length' packages/evals/fixtures/semantic-recall/cases.json
```

Expected：corpus = 124、cases = 78、最后 doc 是 `s00124`。

### Step 2.3：cases.json 加 c079 / c080 / c081

- [ ] 在 `packages/evals/fixtures/semantic-recall/cases.json` 末尾、`]` 之前、`c078` 后加：

```json
,
  {
    "id": "c079",
    "query": "故障复盘怎么用 5 Whys 找到根因",
    "lang": "zh",
    "bucket": "content-not-name",
    "expected_top_doc_ids": ["s00125"],
    "comment": "BETA-15B-10 长文本 case 1 / zh / 故障复盘 5 Whys 延伸版 / 触发 BERT encode > 512 token path"
  },
  {
    "id": "c080",
    "query": "canary release strategy with traffic splitting and rollback",
    "lang": "en",
    "bucket": "content-not-name",
    "expected_top_doc_ids": ["s00126"],
    "comment": "BETA-15B-10 长文本 case 2 / en / canary release strategy 延伸版 / 触发 BERT encode > 512 token path"
  },
  {
    "id": "c081",
    "query": "日志保留多久才合规，过期后怎么处理",
    "lang": "zh",
    "bucket": "crosslang",
    "expected_top_doc_ids": ["s00127"],
    "comment": "BETA-15B-10 长文本 case 3 / zh→en 跨语言 / 日志保留策略 vs log retention policy"
  }
```

（具体 query 文本可微调、保 5W1H 自然中文 + en 自然英文）

### Step 2.4：corpus.json 加 s00125 / s00126 / s00127

- [ ] 在 `packages/evals/fixtures/semantic-recall/corpus.json` 末尾、`]` 之前、`s00124` 后加（body 字段是长文本、目标 [600, 1800] token、全虚构）：

```json
,
  {
    "doc_id": "s00125",
    "title": "事故复盘 5 Whys 模板与示例",
    "body": "<zh 长文本约 800-1500 char / 包含 5 Whys 框架介绍 + 虚构事故场景（如『某虚构服务凌晨 3 点告警延迟 8 分钟』）+ 5 层 why 推演 + 改进 action item / 全虚构、零 PII / 出现『5 Whys』『根因』『故障复盘』『改进 action』等关键词与 c079 query 匹配>",
    "lang": "zh"
  },
  {
    "doc_id": "s00126",
    "title": "Canary release strategy and traffic splitting guidelines",
    "body": "<en 长文本约 2500-6000 char / 包含 canary deploy concept + traffic splitting (1% → 5% → 25% → 100%) + rollback triggers + metrics gates + 虚构 deploy playbook 步骤 / 全虚构、零 PII / 出现『canary』『traffic splitting』『rollback』『progressive rollout』等关键词与 c080 query 匹配>",
    "lang": "en"
  },
  {
    "doc_id": "s00127",
    "title": "Log retention policy and compliance considerations",
    "body": "<en 长文本约 2500-4800 char / 包含 log retention tiers (hot / warm / cold / archive) + 合规要求示例 (GDPR / SOX / HIPAA 等虚构政策) + 自动 purge 与归档流程 + 安全/法务 review 节奏 / 全虚构、零 PII / 出现『retention period』『compliance』『archive』『purge』等关键词与 c081 zh query 跨语言匹配>",
    "lang": "en"
  }
```

（body 全虚构、零真实 PII / 文件名 / URL；目标 token 数由 T3 embed log 抽校验。）

### Step 2.5：JSON schema 与 check_integrity 验

- [ ] 跑：

```bash
jq -e '. | length == 81' packages/evals/fixtures/semantic-recall/cases.json
jq -e '. | length == 127' packages/evals/fixtures/semantic-recall/corpus.json
jq -e '[.[] | .id] | unique | length == 81' packages/evals/fixtures/semantic-recall/cases.json
jq -e '[.[] | .doc_id] | unique | length == 127' packages/evals/fixtures/semantic-recall/corpus.json
```

Expected：4 个命令全 exit 0（数量与唯一性都对）。

- [ ] 跑 cargo test 验 `check_integrity` 过（无新单测、依赖 `semantic_quality_gate.rs` 自动跑前置）：

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | head -20
```

Expected：因 vectors.json 还没重 embed、新 case 缺向量、test FAIL 在 `check_vectors` 上（不在 `check_integrity`）。看错误信息确认是 `s00125` / `s00126` / `s00127` / `c079` / `c080` / `c081` 中某个 id 缺 vector，**不是** `check_integrity` 失败（schema 不对 / bucket 不识别 / expected_top_doc_ids 引用不存在 doc）。

若是 `check_integrity` 失败、回 Step 2.3/2.4 修；若是 `check_vectors` 失败、是正常状态、进 T3。

### Step 2.6：fmt 检查（cases / corpus 是 json、不影响 cargo fmt）

- [ ] 跑：

```bash
cargo fmt --all --check
```

Expected：净（json 改动不影响）。

---

## Task 3：Mac Metal `--embed` 重跑 vectors.json + 抽长文本 token 数

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/vectors.json`（重写、bge-m3 全集 81 query + 127 doc）
- Read: stderr log（抽 token 数）

**说明**：跑 `--embed --model models/bge-m3-q8_0.gguf` 重 embed 全集；从 stderr `llama_perf_context_print` 输出抽 s00125/s00126/s00127 实际 token 数验红线 9 在 [513, 2048]。沿 spec §4.4。

### Step 3.1：跑 `--embed` 重 embed 全集（Mac Metal、~15-20s）

- [ ] 跑：

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality --release --features semantic-recall -- \
  --embed \
  --model models/bge-m3-q8_0.gguf \
  --vectors-file vectors.json \
  2> /tmp/beta-15b-10-embed.log
echo "--- exit code: $? ---"
echo "--- vectors.json size ---"
ls -lh packages/evals/fixtures/semantic-recall/vectors.json
echo "--- stderr tail ---"
tail -40 /tmp/beta-15b-10-embed.log
```

Expected：
- exit code 0
- vectors.json 重写、约 2.4-2.6 MB（127 doc × 1024 dim × ~16 byte JSON 序列化）
- stderr 含 `已写 vectors.json`

### Step 3.2：抽长文本 doc 的 prompt eval token 数

- [ ] 跑（grep llama 输出找 s00125/s00126/s00127 对应 prompt eval 次数）：

```bash
grep -A2 'prompt eval' /tmp/beta-15b-10-embed.log | tail -60
```

Expected：能看到多个 `prompt eval` 段、每段含 `n_eval = <N>` 这样的 token 数。

- [ ] 若 log 格式不容易直接对应到 doc_id、用更粗的统计：跑：

```bash
grep -E 'prompt eval|n_eval|n_tokens' /tmp/beta-15b-10-embed.log | sort -u | head -30
```

Expected：能看到 token 数分布。长文本 doc embed call 的 token 数应 > 512 且 < 2048。

- [ ] **若 token 数实测 < 513**（如所有 doc 都被未发现的别处截断、或长文本 case body 写得太短）：回 T2 Step 2.4 把 s00125/s00126/s00127 body 加长、重 T3。

- [ ] **若 token 数实测 > 2048**：触发 spec §5.2 Branch D、回 T2 Step 2.4 缩 body、重 T3。

### Step 3.3：vectors.json schema 抽查

- [ ] 跑：

```bash
jq '.dim' packages/evals/fixtures/semantic-recall/vectors.json
jq '.model_id' packages/evals/fixtures/semantic-recall/vectors.json
jq '.doc_vectors | keys | length' packages/evals/fixtures/semantic-recall/vectors.json
jq '.query_vectors | keys | length' packages/evals/fixtures/semantic-recall/vectors.json
```

Expected：
- `dim` = 1024
- `model_id` = `"models/bge-m3-q8_0.gguf"`
- `doc_vectors` 数 = 127
- `query_vectors` 数 = 81

### Step 3.4：L2 norm 抽样验

- [ ] 跑（抽 s00125 doc vector 的 L2 norm）：

```bash
jq -r '.doc_vectors.s00125 | [.[]] | map(. * .) | add | sqrt' packages/evals/fixtures/semantic-recall/vectors.json
jq -r '.doc_vectors.s00001 | [.[]] | map(. * .) | add | sqrt' packages/evals/fixtures/semantic-recall/vectors.json
jq -r '.query_vectors.c079 | [.[]] | map(. * .) | add | sqrt' packages/evals/fixtures/semantic-recall/vectors.json
```

Expected：3 个数值都接近 1.0（bge-m3 输出 L2-normalized、容差 ±0.02）。

### Step 3.5：vectors.json SHA256 落记录

- [ ] 跑：

```bash
sha256sum packages/evals/fixtures/semantic-recall/vectors.json
```

记下输出（前 8 hex 字符）、后续 T7 baseline 报告 v5 节会用、commit message 也写入。

### Step 3.6：跑 gate test 确认 check_vectors 过（gate 仍守旧 baseline、可能退步、不阻塞 C1）

- [ ] 跑：

```bash
cargo test -p locifind-evals --test semantic_quality_gate -- --nocapture 2>&1 | tail -40
```

Expected：
- 若 gate 1 passed：理论上不退步 baseline（新 case/doc 加进来 / cosine_threshold 0.70 不变 / 现有 doc 向量等价）、PASS
- 若 gate 1 FAILED：看哪一桶 / 哪个红线 fail。常见原因：① 长文本 case 拖低某桶 ndcg / recall；② 解除截断后某现有 doc 向量微变（实际现有 doc 全 < 1200 char 不太可能）。Fail 不阻塞 C1 commit（C2 会 rewrite baseline）、但需在 T4 commit message 中记录「C1 gate FAIL 因 X、由 C2 rewrite baseline 解决」。

**重要**：本 Step gate FAIL 不退回 T2/T3；C1 commit 范围是「数据扩量 + 截断解除」、gate 重新对齐留给 C2。但若 gate FAIL 是因 `check_vectors` / `check_integrity` 错（如 vectors.json 缺某 case 向量）、回 T3 重 embed。

---

## Task 4：C1 全验证门 + commit（数据扩量 + 截断解除）

**Files:** 无新改动（commit T1 + T2 + T3 产物）

**说明**：跑 spec §2.1 红线 1/2/3/8/9 全套 + commit C1。红线 6（v0.5/v0.9 byte-equal）和红线 7（parser-rs fixture SHA）也跑、predictably 不变（C1 没碰 parser/coverage 相关代码）。

### Step 4.1：fmt 红线 1

- [ ] 跑：

```bash
cargo fmt --all --check
```

Expected：净。

### Step 4.2：clippy 红线 2

- [ ] 跑：

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```

Expected：`Finished` 无 warning。

### Step 4.3：workspace test 红线 3

- [ ] 跑：

```bash
cargo test --workspace 2>&1 | tail -10
```

Expected：所有 test passed / 0 failed（除 `semantic_quality_gate` 可能因 gate 守旧 baseline + 新 case 拖低而 FAIL、本 step 接受、由 C2 rewrite baseline 解决）。

- [ ] 若除 `semantic_quality_gate` 外有其他 test FAIL、**停止排查**。

### Step 4.4：v0.5 / v0.9 byte-equal 红线 6

- [ ] 跑 v0.5：

```bash
cargo run -p locifind-evals --release --bin evals -- v05 --json > /tmp/v05-now.json 2>&1
git show main:packages/evals/fixtures/v05-baseline.json 2>/dev/null > /tmp/v05-baseline.json || echo "（main 上无 v05-baseline.json、跳过）"
```

- [ ] v0.5 verify：跑 BETA-15B-3 A-5 / BETA-15B-6 v3 同款 byte-equal 流程（详 `docs/reviews/semantic-recall-quality-baseline.md` 历史节）。Expected: 473 case / 0 diff。

- [ ] v0.9 verify：同款流程、877 case / 0 diff。

（若工具链已有 `--byte-equal` 子命令、直接跑；否则用 `jq` 规范化后逐 case 比 `actual_json` 子字段。规则参考记忆 `project-evals-reporter-nondeterministic` + `project-evals-coverage-pipeline-drift`。）

### Step 4.5：parser-rs / v05 / v09 fixture SHA256 红线 7

- [ ] 跑：

```bash
sha256sum packages/parser-rs/fixtures/*.json packages/evals/fixtures/v05-*.json packages/evals/fixtures/v09-*.json 2>/dev/null
git show main -- packages/parser-rs/fixtures/ packages/evals/fixtures/v05-*.json packages/evals/fixtures/v09-*.json 2>/dev/null | head
```

Expected：上述 fixture（不含 semantic-recall/）SHA 与 main 完全等价。本 cycle 不动 parser/coverage。

### Step 4.6：semantic-recall 新 vectors.json schema 红线 8 + 长文本 token 红线 9

- [ ] 已由 T3 Step 3.3 / 3.4 / 3.2 覆盖、本 step 重抽确认入 commit message。

### Step 4.7：commit C1

- [ ] 跑：

```bash
git status --short
```

Expected：4 文件改动 / 新增：
- `M packages/evals/src/bin/semantic_quality.rs`
- `M packages/evals/fixtures/semantic-recall/cases.json`
- `M packages/evals/fixtures/semantic-recall/corpus.json`
- `M packages/evals/fixtures/semantic-recall/vectors.json`

- [ ] 跑：

```bash
git add packages/evals/src/bin/semantic_quality.rs \
        packages/evals/fixtures/semantic-recall/cases.json \
        packages/evals/fixtures/semantic-recall/corpus.json \
        packages/evals/fixtures/semantic-recall/vectors.json
git commit -m "$(cat <<'EOF'
BETA-15B-10 C1：解除 evals embed 1200 char 截断 + cases/corpus 扩 3 条长文本

- 删 packages/evals/src/bin/semantic_quality.rs:165 `text.chars().take(1200)` 字符截断、embed 全文（与 desktop indexer 真实路径对齐）
- cases.json 加 c079/c080/c081（content-not-name × 2 + crosslang × 1、目标 token > 512）
- corpus.json 加 s00125/s00126/s00127（zh 5 Whys / en canary release / en log retention 全虚构 + 零 PII）
- vectors.json Mac Metal --embed 重跑全集（81 query + 127 doc、dim 1024、bge-m3-q8_0、SHA256 <填> ）
- token 数实测：s00125 <N> / s00126 <N> / s00127 <N> 全在 [513, 2048] 安全区间
- 验证：fmt 净 / clippy 0 warning / workspace test 全过（semantic_quality_gate 因 C2 未 rewrite baseline 可能 FAIL、C2 解决）

Spec: docs/superpowers/specs/2026-06-26-beta-15b-10-...-design.md §3.1 row 1-4 + §4.1 + §4.2 + §4.3 + §4.4
EOF
)"
```

（commit message 中 `<N>` 替换为 T3 实测值、SHA256 替换为 T3 Step 3.5 抽样值前 16 hex）

Expected：commit 落地、显示 4 文件 changed。

### Step 4.8：commit 后再跑全验证门确认 main 状态稳定

- [ ] 跑：

```bash
git log --oneline -3
cargo test --workspace 2>&1 | tail -5
```

Expected：commit hash 出现、test 状态与 Step 4.3 一致（gate 可能 FAIL、其他全过）。

---

## Task 5：跑 9 阈值 sweep + 按 Branch 决策表选 bake T

**Files:**
- Read: 现有 vectors.json + cases.json + corpus.json（C1 commit 后状态）
- Output: `/tmp/beta-15b-10-sweep.log`（不入仓、T7 baseline 报告 v5 节会用）

**说明**：跑 9 阈值 sweep on 新数据集、按 spec §5.2 决策表选 bake T*。沿 spec §5.1。

### Step 5.1：跑 9 阈值 sweep

- [ ] 跑：

```bash
cd /Users/alice/Work/LocalFind
rm -f /tmp/beta-15b-10-sweep.log
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== T=$t ===" | tee -a /tmp/beta-15b-10-sweep.log
  cargo run -p locifind-evals --bin semantic_quality --release -- \
    --vectors-file vectors.json \
    --semantic-weight 10.0 \
    --cosine-threshold $t \
    2>&1 | tee -a /tmp/beta-15b-10-sweep.log
done
echo "=== sweep done ==="
ls -lh /tmp/beta-15b-10-sweep.log
```

Expected：9 段 `=== T=... ===` 输出、每段含完整表格（FTS_R / FTS_N / VEC_R / VEC_N / HYB_R / HYB_N / HYBR_R / HYBR_N × 6 桶 + OVERALL 行）。log ~30-50KB。

### Step 5.2：抽 OVERALL / crosslang / content-not-name / exact-name 4 桶 HYBR 指标

- [ ] 跑（从 log 提关键指标）：

```bash
echo "T       OVERALL_HYBR_N  crosslang_HYBR_N  content-not-name_HYBR_N  exact-name_HYBR_R"
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  block=$(awk -v t="=== T=$t ===" 'BEGIN{p=0} $0==t{p=1;next} /^=== T=/{p=0} p' /tmp/beta-15b-10-sweep.log)
  overall=$(echo "$block" | awk '/^OVERALL/ {print $NF}')
  crosslang=$(echo "$block" | awk '/^crosslang/ {print $NF}')
  cnn=$(echo "$block" | awk '/^content-not-name/ {print $NF}')
  exact=$(echo "$block" | awk '/^exact-name/ {print $(NF-1)}')
  printf "%-7s %-15s %-17s %-23s %s\n" "$t" "$overall" "$crosslang" "$cnn" "$exact"
done | tee /tmp/beta-15b-10-sweep-summary.txt
```

Expected：9 行表格、每行 5 列。把输出保存、T6 + T7 都会用。

### Step 5.3：按 Branch 决策表选 bake T*

- [ ] 看 sweep summary、按 spec §5.2 决策：
  - **Branch A**：T ∈ {0.0, 0.30, 0.45} 中至少有一个 HYBR_N OVERALL 最大、其他档不超过它、（最大 OVERALL 落在 ≤ 0.45）→ bake T*=0.45（取上限）
  - **Branch B**：HYBR_N OVERALL 最大落在 T = 0.60 或 0.70 → bake 该档
  - **Branch C**：所有档 HYBR_N OVERALL < 现行（守 qwen3-0.6b）baseline OVERALL 0.856 → NO GO、不进 T6、回报用户

- [ ] 记下 bake T*：`BAKE_T = <选定值>`（如 `0.45` / `0.60` / `0.70`）。也记下对应那一档的 OVERALL / crosslang / content-not-name / exact-name 4 个数（T6 baseline 数值确认会用）。

- [ ] 同时检查 (4a) exact-name HYBR_R = 1.000 + (4b) 各桶 HYBR_N ≥ 现行 baseline HYB（不退步）：
  - 若 (4a) FAIL（exact-name < 1.0）：Branch C / NO GO
  - 若 (4b) 某桶退步：尝试别的 T；若所有档都退步某同一桶 → NO GO

### Step 5.4：写 sweep 决策小结到 commit-prep 临时文件

- [ ] 跑：

```bash
cat > /tmp/beta-15b-10-bake-decision.md <<EOF
## BETA-15B-10 sweep + bake 决策

dataset: 81 cases / 127 docs / model = bge-m3-q8_0 / W = 10.0

sweep 4 桶关键指标：
\`\`\`
$(cat /tmp/beta-15b-10-sweep-summary.txt)
\`\`\`

**bake T*** = <BAKE_T>（Branch <A/B/C>）
**理由**：<sweep best 落点 + spec §5.2 决策依据>
**接受标准核对**：
- (4a) exact-name HYBR_R = <实测> （= 1.000 → PASS / < 1.000 → FAIL）
- (4b) 各桶 HYBR_N ≥ 现行 baseline HYB：<逐桶比 + PASS/FAIL>
- (4c) crosslang HYBR_N = <实测>（自锁、不追 0.700）
- (4d) OVERALL HYBR_N = <实测>（bake 后 = 新 baseline）
EOF
cat /tmp/beta-15b-10-bake-decision.md
```

T7 baseline 报告 v5 节会引用此小结。

---

## Task 6：bake DEFAULT_COSINE_ROUTING_THRESHOLD + rewrite baseline.json + gate.rs 注释升 v5

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`（`DEFAULT_COSINE_ROUTING_THRESHOLD` 字面值 0.70 → `<BAKE_T>`）
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`（rewrite 全 6 bucket）
- Modify: `packages/evals/tests/semantic_quality_gate.rs`（doc comment 段升 v5）

**说明**：T5 已选定 `BAKE_T`。本 task 三件 atomic 改动绑同一 commit（避 gate 中间状态 FAIL）。沿 spec §4.5 / §4.6 / §4.7。

### Step 6.1：定位 lib.rs 中 `DEFAULT_COSINE_ROUTING_THRESHOLD`

- [ ] 跑：

```bash
grep -n DEFAULT_COSINE_ROUTING_THRESHOLD packages/result-normalizer/src/lib.rs
```

Expected：至少 1 行命中（const 定义）+ 可能多行（doc 注释 / 单测引用）。

### Step 6.2：改 const 字面值 0.70 → BAKE_T

- [ ] 用 Edit tool 把 `pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 0.70;` 改为 `pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = <BAKE_T>;`（替换 `<BAKE_T>` 为 T5 选定值）

- [ ] 同文件附近 doc 注释若含「BETA-15B-6 v2 T*=0.70 bake」类历史标注、追加 `→ BETA-15B-10 v5 T*=<BAKE_T> bake`（不删旧、追加）。

### Step 6.3：cargo check + clippy 验编译

- [ ] 跑：

```bash
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```

Expected：编译过 + clippy 0 warning。

### Step 6.4：跑 `--write-baseline` rewrite baseline.json

- [ ] 跑：

```bash
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality --release -- \
  --vectors-file vectors.json \
  --semantic-weight 10.0 \
  --cosine-threshold <BAKE_T> \
  --write-baseline \
  2>&1 | tail -10
echo "--- baseline.json size ---"
ls -lh packages/evals/fixtures/semantic-recall/baseline.json
```

Expected：stderr 含 `已写 baseline.json（7 桶含 OVERALL）`（synonym / concept / crosslang / content-not-name / exact-name + 任何 misc 桶 + OVERALL）；文件约 1-3KB。

**注**：`--cosine-threshold` 用 lib.rs 默认就够、但显式传 `<BAKE_T>` 与 lib.rs 字面值一致更安全。也可省 flag（默认 = 改后的字面值）。

### Step 6.5：baseline.json 抽查关键数

- [ ] 跑：

```bash
jq '.[] | select(.bucket == "OVERALL") | {hybrid_routed_ndcg, hybrid_routed_recall, hybrid_ndcg, hybrid_recall}' packages/evals/fixtures/semantic-recall/baseline.json
jq '.[] | select(.bucket == "crosslang") | {hybrid_routed_ndcg}' packages/evals/fixtures/semantic-recall/baseline.json
jq '.[] | select(.bucket == "exact-name") | {hybrid_routed_recall}' packages/evals/fixtures/semantic-recall/baseline.json
```

Expected：数值与 T5 sweep summary 对应 `<BAKE_T>` 那行一致（容差 ± 0.001 浮点）。exact-name hybrid_routed_recall = 1.000。

### Step 6.6：gate.rs 注释升 v5

- [ ] 用 Edit tool 改 `packages/evals/tests/semantic_quality_gate.rs:1`：

```rust
//! BETA-15B-6 → ... → BETA-15B-10 回归门：合成集 hybrid 在关键桶不跌破提交 baseline。
//! 跑 checked-in 缓存向量（确定性）。vectors.json / baseline.json 未提交（Phase D 前）→ 跳过。
```

（line 1 注释从 `BETA-15B-6 回归门：...` 升为 `BETA-15B-6 → ... → BETA-15B-10 回归门：...`、保 line 2 不变）

- [ ] 同文件改 line 85-89 段落 doc（A-5 红线 + v2/v3/v5 校验注释）：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2 → v3 → BETA-15B-10 v5 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=0.60 → v2/v3 T*=0.70 → v5 T*=<BAKE_T> bake 后数值无需替换。
    // 诚实边界：bge-m3 真水位 crosslang ~<crosslang HYBR_N from T5> < 0.700 spec 字面、移交未来 cycle = 更大 / 跨厂 embedding 模型。
    // 详 docs/reviews/semantic-recall-quality-baseline.md v5 节。
```

（替换 `<BAKE_T>` 与 `<crosslang HYBR_N from T5>` 为 T5 实测值）

### Step 6.7：跑 gate test 确认 4 红线全过新 baseline

- [ ] 跑：

```bash
cargo test -p locifind-evals --test semantic_quality_gate -- --nocapture 2>&1 | tail -20
```

Expected：`test hybrid_does_not_regress_key_buckets_vs_baseline ... ok`、`test result: ok. 1 passed; 0 failed`。

**若 FAIL**：
- 看 panic message 哪一桶 / 哪一指标退步。
- 常见原因：① T5 sweep summary 与 `--write-baseline` 输出有差（如 sweep 用 release 模式、`--write-baseline` 用 debug 模式、浮点误差）→ 重 Step 6.4；② BAKE_T 选错（Branch C 不应进 T6）→ 回 T5 重决策；③ baseline.json schema 与 gate 期望字段不匹配 → 看 gate.rs 字段名核对。

### Step 6.8：fmt 检查 + clippy 终验

- [ ] 跑：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```

Expected：fmt 净 + clippy 0 warning。

---

## Task 7：C2 全验证门 + commit（bake cosine + baseline rewrite + gate 注释）

**Files:** 无新改动（commit T6 产物）

**说明**：跑 spec §2.1 红线 1-7 全套 + commit C2。

### Step 7.1：fmt / clippy / workspace test / gate（红线 1-4）

- [ ] 跑：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
cargo test --workspace 2>&1 | tail -10
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```

Expected：全 PASS（gate 4 红线全过新 baseline）。

### Step 7.2：desktop tsc + vite build（红线 5）

- [ ] 跑：

```bash
cd apps/desktop
npm run typecheck 2>&1 | tail -10
npm run build 2>&1 | tail -10
cd /Users/alice/Work/LocalFind
```

Expected：tsc 净、vite build 成功（`built in ...ms` 行）。

### Step 7.3：v0.5 / v0.9 byte-equal（红线 6）

- [ ] 跑（沿 T4 Step 4.4 同款）：cosine 改动不应影响 v0.5/v0.9 parser/coverage 任何字段。

Expected：v0.5=473 / 0 diff、v0.9=877 / 0 diff。

### Step 7.4：parser-rs / v05 / v09 fixture SHA256（红线 7）

- [ ] 跑（沿 T4 Step 4.5 同款）：

Expected：parser-rs/v05/v09 fixture（不含 semantic-recall/）SHA 与 main 完全等价。

### Step 7.5：git status 确认改动文件

- [ ] 跑：

```bash
git status --short
```

Expected：3 文件改动：
- `M packages/result-normalizer/src/lib.rs`
- `M packages/evals/fixtures/semantic-recall/baseline.json`
- `M packages/evals/tests/semantic_quality_gate.rs`

### Step 7.6：commit C2

- [ ] 跑：

```bash
git add packages/result-normalizer/src/lib.rs \
        packages/evals/fixtures/semantic-recall/baseline.json \
        packages/evals/tests/semantic_quality_gate.rs
git commit -m "$(cat <<'EOF'
BETA-15B-10 C2：bake cosine_threshold + rewrite baseline + gate.rs 注释升 v5

- DEFAULT_COSINE_ROUTING_THRESHOLD: 0.70 → <BAKE_T>（Branch <A/B/C> bake、sweep best on bge-m3 + 新 corpus）
- baseline.json rewrite：dataset 81 cases / 127 docs / bge-m3-q8_0 / T*=<BAKE_T> 单 T 跑
  - OVERALL HYBR_N = <值> / crosslang HYBR_N = <值> / content-not-name HYBR_N = <值> / exact-name HYBR_R = 1.000
- gate.rs doc comment 升 v5：BETA-15B-6 → ... → BETA-15B-10、注明 v5 T*=<BAKE_T> bake + 诚实边界
- 验证：fmt 净 / clippy 0 warning / workspace test 全过 / semantic_quality_gate 4 红线全过新 baseline / desktop tsc + vite 净 / v0.5+v0.9 parser byte-equal 0 diff / parser-rs+v05+v09 fixture SHA 与 main 等价

Spec: docs/superpowers/specs/2026-06-26-beta-15b-10-...-design.md §3.1 row 5-7 + §4.5 + §4.6 + §4.7 + §5
EOF
)"
```

（commit message 中 `<BAKE_T>` / `<Branch>` / 各 `<值>` 替换为 T5 + T6 实测值）

Expected：commit 落地、3 文件 changed。

---

## Task 8：baseline 报告 v5 节 + fixtures README v5 + commit

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（追加 v5 节、不删旧节）
- Modify: `packages/evals/fixtures/semantic-recall/README.md`（升 v5 版本说明）

**说明**：沿 BETA-15B-6 v2/v3 / BETA-15B-8 v4-fixup 同款结构。本 task 不独立 commit、与 T9 STATUS/ROADMAP 合并 C3 commit。

### Step 8.1：baseline 报告追加 v5 节

- [ ] 跑：

```bash
tail -40 docs/reviews/semantic-recall-quality-baseline.md
```

确认末尾是 v4-fixup3 节（BETA-15B-7-v2 数据），找到合适插入点。

- [ ] 用 Write/Edit tool 追加 v5 节，结构沿 v3 节：

```markdown
## v5（BETA-15B-10、2026-06-26、Claude Code Opus 4.7）

**承接**：BETA-15B-7-v2 bake bge-m3 到桌面默认（[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15)）+ hotfix BERT encode n_ubatch panic（[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16)）后、follow-up 三件套合并为单 cycle 完成。

**关键改动**：
1. 评测层 baseline 从守 qwen3-0.6b 彻底重锚到 bge-m3
2. `DEFAULT_COSINE_ROUTING_THRESHOLD`：0.70 → <BAKE_T>（Branch <A/B/C>）
3. 评测合成集首次覆盖 > 512 token 长文本 case（c079/c080/c081 + s00125/s00126/s00127）
4. 解除 [`packages/evals/src/bin/semantic_quality.rs:165`](../../packages/evals/src/bin/semantic_quality.rs#L165) 1200 char 字符截断、与 desktop indexer embed 路径对齐

**dataset**：81 cases（v3 78 + c079/c080/c081）/ 127 docs（v3 124 + s00125/s00126/s00127）/ bge-m3-q8_0 / dim 1024 / W=10.0 固定 / Mac Metal

**Sweep 全表**（T = cosine_threshold）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
（从 /tmp/beta-15b-10-sweep-summary.txt 抄入、9 行）

**bake T*** = `<BAKE_T>`（Branch <A/B/C>）

**spec §2.2 接受标准核对**：
- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ 新 baseline HYB（不退步）✓（逐桶比）
- (4c) crosslang HYBR_N = <值>（自锁、本 cycle 不追 0.700 spec 字面、bge-m3 真水位限制）
- (4d) OVERALL HYBR_N = <值>（bake 后 = 新 baseline）

**vs v3（qwen3-0.6b T*=0.70）对比**：
- OVERALL HYBR_N：v3 0.856 → v5 <值>（Δ <+/->）
- crosslang HYBR_N：v3 0.717 → v5 <值>（Δ <+/-、bge-m3 跨族 trade-off 预期 < 0、文档明示）
- content-not-name HYBR_N：v3 0.870 → v5 <值>（Δ <+/->）
- exact-name HYBR_R：v3 1.000 → v5 1.000（守恒）

**1200 char 截断解除影响分析**：v3 / v4-fixup 数据集 124 doc 全部 < 1200 char（合成 corpus 设计本就守 < 512 token 规模）、改前改后向量理论 byte-equal；本 cycle 新 3 doc（s00125 zh ~800-1500 char / s00126 en ~2500-6000 char / s00127 en ~2500-4800 char）是截断解除后才能完整 embed 的真长文本。

**诚实边界**：
- crosslang HYBR_N <值> 仍 < 0.700 spec 字面、本 cycle 主动放弃字面追求、移交未来 cycle = 更大 / 跨厂 embedding 模型（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B、STATUS 已登记）
- 评测合成 corpus 与真机用户 doc 分布有 gap、评测 ndcg 是结构性上限、不代表真机用户体验绝对值（BETA-30 失败样本箱在 STATUS 登记、未来真实闭环）

**vectors.json**：SHA256 `<from T3 Step 3.5>`（前 16 hex）

**与 v4-fixup 系列 reference snapshot 区别**：本 cycle 主 active vectors = `vectors.json` = bge-m3 + 81/127 + 截断解除后状态；`vectors-bge-m3.json`（BETA-15B-8 v4-fixup snapshot）+ `vectors-qwen3-0.6b.json`（v3 reference snapshot）均不动、作为历史对照保留。

**下 cycle 抓手优先级**（v5 数据指证）：①〔...〕（按本 cycle 实测结果填）...
```

### Step 8.2：fixtures README 升 v5

- [ ] 跑：

```bash
tail -30 packages/evals/fixtures/semantic-recall/README.md
```

- [ ] 在 README 末尾或合适位置加 v5 段落：

```markdown
## v5（BETA-15B-10、2026-06-26）

- dataset：81 cases / 127 docs（新增 c079/c080/c081 + s00125/s00126/s00127 三条长文本）
- model：bge-m3-q8_0（与桌面默认对齐、BETA-15B-7-v2 切到生产）
- cosine_threshold bake：v3 T*=0.70 → v5 T*=<BAKE_T>
- baseline.json：守 bge-m3 + 81/127 + T*=<BAKE_T>
- 长文本 case 设计意图：覆盖 > 512 token BERT encode path、与 desktop indexer 真实路径对齐；token 数 [513, 2048] 安全区间（hotfix n_ubatch=2048 上限内）
- evals binary embed 路径解除 1200 char 截断（与 desktop indexer 等价）
```

### Step 8.3：fmt 检查 + verify doc 改动不破任何 link

- [ ] 跑：

```bash
cargo fmt --all --check
grep -E '\[.*\]\(.*\)' docs/reviews/semantic-recall-quality-baseline.md | tail -5
grep -E '\[.*\]\(.*\)' packages/evals/fixtures/semantic-recall/README.md | tail -5
```

Expected：fmt 净、link 路径形式正常。

---

## Task 9：STATUS.md + ROADMAP.md 更新 + commit C3 + push branch + PR

**Files:**
- Modify: `STATUS.md`
- Modify: `ROADMAP.md`
- 同时 commit T8 改动作 C3

**说明**：cycle 末收工、按 CONVENTIONS §3 收工流程更新 STATUS + ROADMAP。本 task 落地 C3 commit + push + 开 PR。

### Step 9.1：更新 STATUS.md 当前 Task / 下一步 / 会话日志

- [ ] 用 Read tool 读 STATUS.md 顶部 + 当前 Task 段 + 会话日志最新条。

- [ ] Edit STATUS.md：
  - 「当前 Task」段：从 BETA-15B-7-v2 + hotfix 改为 BETA-15B-10（cycle 主题 + 关键数据 + bake T* + 4 红线核对结果 + 未尽事宜）
  - 「下一步」段：本 cycle 后下 cycle 抓手优先级修正（v5 数据指证后的最新视角）
  - 「会话日志」**顶部**追加一段 BETA-15B-10 收口（沿 BETA-15B-7-v2 / BETA-15B-8 / BETA-15B-9 同款结构：承接 + 关键决策 + 5 task 产出 + 验证 + bake 数据 + 诚实边界 + 未尽事宜）

- [ ] 若 STATUS.md 会话日志超 10 条上限、滚动归档最旧若干条到 [docs/session-logs/STATUS-archive-2026-06.md](../../session-logs/STATUS-archive-2026-06.md)（CONVENTIONS §3 要求）。

### Step 9.2：更新 ROADMAP.md BETA-15B-10 task 状态

- [ ] 跑：

```bash
grep -n 'BETA-15B-10\|BETA-15B-9\|BETA-15B-8' ROADMAP.md | head -10
```

- [ ] 找 BETA-15B 系列 task 列表位置、追加 BETA-15B-10 卡片（结构沿 BETA-15B-9 / BETA-15B-8 同款）：

```markdown
| **BETA-15B-10** | **bge-m3 baseline 重锚 + cosine_threshold sweep & bake + 评测集长文本扩量 + evals embed 截断解除** | **done**（2026-06-26 Claude Code、[PR #<N>](https://github.com/raoliaoyuan/LociFind/pull/<N>) merged、merge commit `<hash>`、分支已删）⭐ 承接 BETA-15B-7-v2 follow-up 三件套合并执行。dataset v3 78/124 → v5 81/127、cosine T*=0.70 → <BAKE_T>、解除 evals embed 1200 char 截断。OVERALL HYBR_N = <值>（v3 0.856 → <Δ>）、crosslang HYBR_N = <值>（v3 0.717 → <Δ>、bge-m3 跨族 trade-off 预期）、exact-name HYBR_R = 1.000 守恒。诚实边界：crosslang < 0.700 spec 字面、移交未来 cycle = 更大 / 跨厂 embedding 模型。详 docs/reviews/semantic-recall-quality-baseline.md v5 节 + STATUS 会话日志。 | packages/evals/* + packages/result-normalizer/src/lib.rs | BETA-15B-7-v2 + BETA-15B-8 | 2d |
```

（替换 `<N>` / `<hash>` / `<BAKE_T>` / `<值>` / `<Δ>` 为实测值；merge 后回填）

### Step 9.3：commit C3（doc-sync 全套）

- [ ] 跑：

```bash
git status --short
```

Expected：4 文件改动：
- `M docs/reviews/semantic-recall-quality-baseline.md`
- `M packages/evals/fixtures/semantic-recall/README.md`
- `M STATUS.md`
- `M ROADMAP.md`

（若 STATUS 滚动归档触发、再加 `M docs/session-logs/STATUS-archive-2026-06.md`）

- [ ] 跑：

```bash
git add docs/reviews/semantic-recall-quality-baseline.md \
        packages/evals/fixtures/semantic-recall/README.md \
        STATUS.md \
        ROADMAP.md \
        docs/session-logs/STATUS-archive-2026-06.md  # 若触发滚动
git commit -m "$(cat <<'EOF'
BETA-15B-10 C3：doc-sync —— baseline 报告 v5 节 + README v5 + STATUS + ROADMAP 收口

- docs/reviews/semantic-recall-quality-baseline.md：追加 v5 节（dataset 81/127 + sweep 全表 + 4 红线核对 + vs v3 对比 + 1200 char 截断解除影响分析 + 诚实边界）
- packages/evals/fixtures/semantic-recall/README.md：升 v5 版本说明（长文本 case 设计意图 + token [513,2048] 安全区间 + evals embed 与 desktop indexer 对齐）
- STATUS.md：当前 Task → BETA-15B-10 done / 下一步 / 会话日志顶部追加 BETA-15B-10 收口段
- ROADMAP.md：BETA-15B-10 task 卡片（PR 合并后回填 N + hash）
EOF
)"
```

Expected：commit 落地、4-5 文件 changed。

### Step 9.4：跑全验证门 final（C3 doc-only、不该破任何红线）

- [ ] 跑：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
cargo test --workspace 2>&1 | tail -5
```

Expected：fmt 净 + clippy 0 warning + test 全过。

### Step 9.5：push branch + 开 PR

- [ ] 跑：

```bash
git push -u origin feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text
```

Expected：push 成功、远程分支建立。

- [ ] 用 gh CLI 开 PR：

```bash
gh pr create \
  --title "BETA-15B-10：bge-m3 baseline 重锚 + cosine_threshold sweep & bake + 评测集长文本扩量 + evals embed 截断解除" \
  --body "$(cat <<'EOF'
## 范围

承接 BETA-15B-7-v2 bake bge-m3（[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15) merged）+ hotfix BERT encode n_ubatch panic（[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16) merged）后的 follow-up 三件套合并 cycle：

1. **评测层 baseline 重锚**：baseline.json + `DEFAULT_COSINE_ROUTING_THRESHOLD` 从守 qwen3-0.6b 切换到 bge-m3 真水位
2. **cosine_threshold sweep & bake**：9 阈值 sweep + 按 Branch 决策表 bake T*=`<BAKE_T>`（Branch `<A/B/C>`）
3. **评测集长文本扩量**：cases.json +3 case（c079/c080/c081）+ corpus.json +3 doc（s00125/s00126/s00127）、首次覆盖 > 512 token BERT encode path
4. **evals embed 截断解除**：删 `packages/evals/src/bin/semantic_quality.rs:165` 的 `text.chars().take(1200)` 字符截断、evals 路径与 desktop indexer 真实路径对齐

## 关键数据

| 指标 | v3（qwen3-0.6b T*=0.70）| v5（bge-m3 T*=<BAKE_T>）| Δ |
|---|---|---|---|
| OVERALL HYBR_N | 0.856 | <值> | <+/-> |
| crosslang HYBR_N | 0.717 | <值> | <+/-、bge-m3 跨族 trade-off 预期> |
| content-not-name HYBR_N | 0.870 | <值> | <+/-> |
| exact-name HYBR_R | 1.000 | 1.000 | = |

## 接受标准核对（spec §2.2）

- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ 新 baseline HYB（不退步）✓
- (4c) crosslang HYBR_N <值> ≥ 新 baseline HYBR（自锁、本 cycle 不追 0.700 spec 字面）
- (4d) OVERALL HYBR_N <值> = 新 baseline HYBR ✓

## 验证门（spec §2.1）

- fmt 净 / clippy 0 warning / workspace test 全过 / semantic_quality_gate 4 红线全过新 baseline / desktop tsc + vite 净 / v0.5+v0.9 parser byte-equal 0 diff / parser-rs+v05+v09 fixture SHA 与 main 等价 / vectors.json schema 验过 / 长文本 token 数 s00125 <N> / s00126 <N> / s00127 <N> 全在 [513, 2048]

## Mac 真机手测

**DEFERRED**（spec §7.2、GO with documented gap 路径、与 BETA-15B-7-v2 T3 同款）。bake 后桌面 cosine 路由触发更宽松、属数值微调、路由架构不变、tsc + vite 编译过已覆盖；留用户首次升级时按 [docs/manual-test-scenarios.md](docs/manual-test-scenarios.md) 走三步验证。

## 未尽事宜

- cosine_threshold 字面 0.700 crosslang spec 目标移交未来 cycle（更大 / 跨厂 embedding 模型：EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B）
- crosslang 桶 14 例仍偏小、扩量到 20+ 在 STATUS follow-up 登记
- BETA-30 真实失败样本箱与本 cycle 解耦、未来真实闭环

## Spec & Plan

- Spec: [docs/superpowers/specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md](docs/superpowers/specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md)
- Plan: [docs/superpowers/plans/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text.md](docs/superpowers/plans/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text.md)
EOF
)"
```

（PR body 中 `<BAKE_T>` / `<A/B/C>` / 各 `<值>` / `<N>` 替换为实测值）

Expected：PR 创建、返回 URL。

### Step 9.6：合 main（gh CLI 401 fallback 走本地 merge）

- [ ] 优先跑：

```bash
gh pr merge <PR-N> --merge --delete-branch
```

Expected：merge 成功、本地切回 main、远程分支删除。

- [ ] **若 gh CLI 401**（凭据问题）：fallback 走本地 merge：

```bash
git checkout main
git pull origin main
git merge --no-ff feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text -m "Merge pull request #<PR-N> from raoliaoyuan/feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text"
git push origin main
git branch -d feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text
git push origin :feat-beta-15b-10-bge-m3-baseline-cosine-bake-long-text
git fetch --prune
```

Expected：merge commit 生成、push 成功、本地+远程分支删除。

### Step 9.7：回填 ROADMAP.md BETA-15B-10 卡片的 PR# + merge hash

- [ ] 跑：

```bash
git log --oneline -5
```

抄取 merge commit hash。

- [ ] Edit ROADMAP.md BETA-15B-10 卡片把 `<N>` 与 `<hash>` 替换为实际值。

- [ ] 跑：

```bash
git add ROADMAP.md
git commit -m "ROADMAP：BETA-15B-10 卡片回填 PR # + merge commit hash"
git push origin main
```

Expected：单独的 doc-only commit 回填。

---

## Self-Review

写完 plan 后用 fresh eyes 核对 spec 覆盖：

### Spec 覆盖核对

- spec §1.1（背景）：plan header 「关键 spec 已修订」 + T0 / T1 都关联 ✓
- spec §1.2（evals embed 截断）：T1 完整覆盖 ✓
- spec §1.3（核心数据指证）：T5 sweep summary + T8 baseline 报告 v5 节比对 ✓
- spec §2.1（验证门 1-9）：T4 + T7 + T9.4 覆盖全 9 红线 ✓
- spec §2.2（接受标准 4a-4d）：T5 Step 5.3 决策 + T6 Step 6.7 gate test + T8 baseline 报告核对表 三处 ✓
- spec §2.3（控制对照不变性 T=0.0 / T=1.01）：T5 Step 5.2 9 阈值表自动覆盖（首末两档可对照）✓
- spec §3.1（做什么 9 row）：T1（row 1）+ T2（row 2-3）+ T3（row 4）+ T6（row 5-7）+ T8（row 8-9）全覆盖 ✓
- spec §3.2（不做什么）：每个 task 范围明确、不溢出 ✓
- spec §4.1（截断解除）：T1 ✓
- spec §4.2（cases.json）：T2 Step 2.3 ✓
- spec §4.3（corpus.json）：T2 Step 2.4 + T3 token 数验 ✓
- spec §4.4（vectors.json 重 embed）：T3 ✓
- spec §4.5（cosine_threshold bake）：T6 Step 6.1-6.3 ✓
- spec §4.6（baseline.json rewrite）：T6 Step 6.4-6.5 ✓
- spec §4.7（gate.rs 注释升 v5）：T6 Step 6.6 ✓
- spec §5.1（sweep 流程）：T5 Step 5.1 ✓
- spec §5.2（Branch 决策表）：T5 Step 5.3 ✓
- spec §5.3（与 §2.2 关系）：T5 Step 5.3 + T6 Step 6.7 ✓
- spec §6（验证产物）：T8 baseline 报告 + T9 STATUS/ROADMAP ✓
- spec §7（风险）：T5/T6/T8 都在对应 step 注明 R1-R7 应对 ✓
- spec §8（commit 颗粒度方案 B）：T4 C1 + T7 C2 + T9.3 C3 三 commit 严格按 §8 ✓
- spec §9（PR 路径）：T9.5/9.6 ✓
- spec §10（修订记录）：spec 已写、plan 不复述 ✓

### Placeholder 扫

- T1-T9 所有运行时变量（`<BAKE_T>` / `<N>` / `<值>` / `<hash>` / `<PR-N>`）都在 step 描述中明确说明替换来源（T5 sweep / T3 embed log / git log / gh pr create 输出）、不是「TBD」。
- T2 Step 2.4 body 内容用 `<zh 长文本...>` 占位、但 step 注明「全虚构、零 PII、关键词与 c079/c080/c081 query 匹配」给出明确写作指南、不是「TODO」。

### Type 一致性核对

- `DEFAULT_COSINE_ROUTING_THRESHOLD` 在 T6 Step 6.1 / 6.2 + T6 Step 6.6 注释 + T8 baseline 报告 + T9 STATUS/ROADMAP 全部一致 ✓
- `BAKE_T` 在 T5 Step 5.3 选定 + T6 / T7 / T8 / T9 都正确引用 ✓
- `vectors.json` / `baseline.json` / `cases.json` / `corpus.json` 路径在所有 task 一致 ✓
- gate.rs assert 字段名（`hybrid_recall` / `hybrid_ndcg` / `hybrid_routed_recall` / `hybrid_routed_ndcg`）与 T6 Step 6.5 baseline.json 抽查字段名一致 ✓

无 placeholder / type 不一致问题。
