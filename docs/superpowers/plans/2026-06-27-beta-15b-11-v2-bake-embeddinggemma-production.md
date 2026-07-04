# BETA-15B-11-v2 Implementation Plan：bake EmbeddingGemma-300M 推到生产（桌面 wiring 切换最窄版）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把桌面默认 embedding 模型从 bge-m3 切到 EmbeddingGemma-300M（BETA-15B-11 v6 数据指证 no-prefix mode T=0.70 OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026、**无 trade-off 全方面提升** vs v5 bge-m3 baseline）。

**Architecture:** 极窄 wiring 切换。改 `apps/desktop/src-tauri/src/search/embedding_model.rs` 3 处常量字面值 + 顶部 mod doc + 两处常量 doc 注释（diff < 20 行、单文件）。不动 baseline.json / cosine_threshold / result-normalizer / evals / model-runtime / indexer。依赖 BETA-15B-1 + BETA-15B-2 已建立的失效（`vector_is_current`）+ reindex 机制完成旧用户迁移（含 dim 1024 → 768 转换）。

**Tech Stack:** Rust 1.x / llama-cpp-4 0.3.2（已支持 `gemma-embedding` arch 验证完毕、BETA-15B-11 Task 4 决策点已过）/ TDD（编译期常量自动跟随、不写字面字符串单测）

**Spec:** [docs/superpowers/specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md](../specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md)

---

## Task 0：开 cycle 预检 + feature branch（不 commit、几秒）

**Goal:** 起点状态确认 — 仓库干净 + main HEAD 与 STATUS 一致 + BETA-15B-11 spec/plan/cycle 已合 main + embedding_model.rs 现状 = bge-m3。

- [ ] **Step 0.1: 看仓库状态干净**

```bash
cd /Users/alice/Work/LocalFind
git status
git log --oneline -5
```

Expected: working tree clean、branch main 与 origin/main 一致；HEAD 应为本会话 commit `34177de`（BETA-15B-11-v2 spec）；其上 5 行应含 `c5f652d doc-sync：BETA-15B-11 回填 PR #18`、`49b5f4a Merge pull request #18`。

- [ ] **Step 0.2: 看本机已有模型（embeddinggemma 应已就绪）**

```bash
ls -la models/embeddinggemma-300m-q8_0.gguf
sha256sum models/embeddinggemma-300m-q8_0.gguf
```

Expected: 文件大小 ~329 MB（328,577,056 bytes）、SHA256 = `6fa0c02a9c302be6f977521d399b4de3a46310a4f2621ee0063747881b673f67`（BETA-15B-11 Task 1 已下载入仓 `.gitignore` 内）。

- [ ] **Step 0.3: 看 embedding_model.rs 现状（bge-m3、待切）**

```bash
grep -n "DEFAULT_EMBED_MODEL_FILE\|EMBED_MODEL_ID" apps/desktop/src-tauri/src/search/embedding_model.rs
```

Expected: 2 行命中：
- `const DEFAULT_EMBED_MODEL_FILE: &str = "bge-m3-q8_0.gguf";`
- `const EMBED_MODEL_ID: &str = "bge-m3";`

- [ ] **Step 0.4: 开 feature branch**

```bash
git checkout -b feat-beta-15b-11-v2-bake-embeddinggemma-production
git status
```

Expected: switched to new branch、working tree clean。

---

## Task 1：Desktop wiring 切换 embedding_model.rs

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/embedding_model.rs`（5 处：顶部 mod doc + line 17-20 常量 + line 21-24 常量 + 两段 doc）
- Test: `apps/desktop/src-tauri/src/search/embedding_model.rs::tests::model_id_is_stable`（不改单测断言代码、`EMBED_MODEL_ID` 编译期常量自动跟随）

**说明**：唯一改 desktop 源文件 = 改两常量字面值 + 顶部 mod doc + 两处常量 doc 注释。`model_id_is_stable` 单测断言 `assert_eq!(h.model_id(), EMBED_MODEL_ID)` 用编译期常量、不写字面字符串、随 `EMBED_MODEL_ID` 改动自动 == `"embeddinggemma-300m"`、无需手动改单测。

**TDD 节奏**：因单测断言用编译期常量自动跟随、本 task 不是先红后绿的纯 TDD 路径。改为「基线检查 → 实施 → 重测验自动通过」三步走（与 BETA-15B-7-v2 同款）。

**Spec ref:** §3.1

### Step 1.1: 基线检查 —— 跑 desktop 单测确认当前全过

- [ ] 跑：

```bash
cargo test -p locifind-desktop --lib --features semantic-recall search::embedding_model::tests
```

Expected: 4-5 个单测全过（feature 条件 skip）：`feature_off_new_is_unavailable_from_start` / `feature_off_embed_errs` / `model_id_is_stable` / `embedding_model_path_override_from_settings` / `prewarm_feature_off_is_false_and_idempotent`。基线、确认改动前是干净的。

### Step 1.2: 改顶部 mod doc 注释（line 1-9）

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:1-9`：

把：
```rust
//! BETA-15B-1：embedding 模型懒加载句柄（镜像 model_fallback 的约定目录 + feature 门控）。
//! 实现 indexer 的 `TextEmbedder`，供索引期与查询期共用。
//!
//! BETA-15B-7-v2 (2026-06-26)：默认模型从 qwen3-embedding-0.6b 切到 bge-m3、
//! 落实 BETA-15B-8 v4-fixup 真水位 OVERALL=0.869 ⭐（vs qwen3 0.856 +0.013、
//! 评测层 baseline 仍守 qwen3-0.6b 不动、follow-up cycle 视真机反馈再切换）；
//! cosine 路由阈值 0.70 / 相似度下限 0.30 / 语义臂权重 10.0 保 qwen3 调优值不动、
//! bge-m3 sweep best 在 T*=0.0/0.30/0.45 是 follow-up cycle 工作；
//! crosslang 相对 qwen3-0.6b -0.055 是头号卖点 trade-off（OVERALL 净增 +0.008 cover）。
```

改为（在 BETA-15B-7-v2 段后追加 BETA-15B-11-v2 段）：
```rust
//! BETA-15B-1：embedding 模型懒加载句柄（镜像 model_fallback 的约定目录 + feature 门控）。
//! 实现 indexer 的 `TextEmbedder`，供索引期与查询期共用。
//!
//! BETA-15B-7-v2 (2026-06-26)：默认模型从 qwen3-embedding-0.6b 切到 bge-m3、
//! 落实 BETA-15B-8 v4-fixup 真水位 OVERALL=0.869 ⭐（vs qwen3 0.856 +0.013、
//! 评测层 baseline 仍守 qwen3-0.6b 不动、follow-up cycle 视真机反馈再切换）；
//! cosine 路由阈值 0.70 / 相似度下限 0.30 / 语义臂权重 10.0 保 qwen3 调优值不动、
//! bge-m3 sweep best 在 T*=0.0/0.30/0.45 是 follow-up cycle 工作；
//! crosslang 相对 qwen3-0.6b -0.055 是头号卖点 trade-off（OVERALL 净增 +0.008 cover）。
//!
//! BETA-15B-11-v2 (2026-06-27)：默认模型从 bge-m3 切到 embeddinggemma-300m、
//! 落实 BETA-15B-11 v6 真水位 no-prefix mode T=0.70 OVERALL=0.874 / crosslang=0.716 ⭐⭐
//! 双过 spec 字面 0.864 + 0.700（vs v5 bge-m3 baseline +0.010 / +0.030、**无 trade-off
//! 全方面提升** + content-not-name +0.026 + exact-name 守 1.000）；评测层 baseline.json
//! 仍守 v5 bge-m3 不动、follow-up cycle 视真机反馈再切换；cosine 路由阈值 0.70 /
//! 相似度下限 0.30 / 语义臂权重 10.0 保 v5 调优值不动；embeddinggemma sweep best 在
//! no-prefix T*=0.60 OVERALL=0.882 是 follow-up cycle 工作；prefix mode +0.013~+0.026
//! 加分项留 BETA-15B-11-v3 follow-up（不是 GO 必要条件）；模型分发 313 MB 比 bge-m3
//! 605 MB 净降 292 MB。
```

### Step 1.3: 改 `DEFAULT_EMBED_MODEL_FILE` 常量 + doc 注释（line 17-20）

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:17-20`：

把：
```rust
/// 默认 embedding 模型文件名（BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869 ⭐
/// 双过 spec 字面 0.864、BETA-15B-7-v2 bake 切换；该 -0.032 crosslang gap 为 bge-m3 sweep best
/// vs qwen3-0.6b T*=0.70 对照、生产实际部署的 T*=0.70 + bge-m3 实测 crosslang -0.055、详 mod doc 顶部）。
const DEFAULT_EMBED_MODEL_FILE: &str = "bge-m3-q8_0.gguf";
```

改为：
```rust
/// 默认 embedding 模型文件名（BETA-15B-11 v6 EmbeddingGemma-300M no-prefix mode T=0.70 真水位
/// OVERALL=0.874 + crosslang=0.716 ⭐⭐ 双过 spec 字面 0.864 + 0.700、BETA-15B-11-v2 bake 切换；
/// vs v5 bge-m3 baseline +0.010 / +0.030 / +0.026 全方面提升无 trade-off、详 mod doc 顶部）。
const DEFAULT_EMBED_MODEL_FILE: &str = "embeddinggemma-300m-q8_0.gguf";
```

### Step 1.4: 改 `EMBED_MODEL_ID` 常量 + doc 注释（line 21-24）

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:21-24`：

把：
```rust
/// 模型标识（写入 `document_vectors.embed_model`，换模型→旧向量陈旧）。
/// BETA-15B-7-v2：从 "qwen3-embedding-0.6b" 切到 "bge-m3"、依赖 vector_is_current(model_id)
/// 自动失效旧向量 + spawn_semantic_index 后台 reindex 机制完成迁移。
const EMBED_MODEL_ID: &str = "bge-m3";
```

改为：
```rust
/// 模型标识（写入 `document_vectors.embed_model`，换模型→旧向量陈旧）。
/// BETA-15B-11-v2：从 "bge-m3" 切到 "embeddinggemma-300m"、依赖 vector_is_current(model_id)
/// 自动失效旧向量 + spawn_semantic_index 后台 reindex 机制完成迁移（含 dim 1024 → 768 转换、
/// 由 vector_is_current 守住、双重防御）。
const EMBED_MODEL_ID: &str = "embeddinggemma-300m";
```

### Step 1.5: 重跑 desktop 单测确认自动全过

- [ ] 跑：

```bash
cargo test -p locifind-desktop --lib --features semantic-recall search::embedding_model::tests
```

Expected: 全过、`model_id_is_stable` 通过（`h.model_id()` 现返 `"embeddinggemma-300m"` == `EMBED_MODEL_ID`）。

### Step 1.6: 跑全 workspace 单测确认无横向回归

- [ ] 跑：

```bash
cargo test --workspace 2>&1 | grep "test result:" | grep -v "0 failed" | wc -l
```

Expected: 0（0 failed test result lines = 全过）。本改动只动两个常量字面值 + 注释、不动任何函数签名 / trait 实现 / 调用链、其他 crate 单测不应受影响。

### Step 1.7: Clippy + fmt 净

- [ ] 跑：

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
cargo fmt --all --check
```

Expected: 0 clippy warning（tail 显示 Finished）、fmt 净（无 stdout）。

### Step 1.8: Commit

- [ ] 跑：

```bash
git add apps/desktop/src-tauri/src/search/embedding_model.rs
git commit -m "$(cat <<'EOF'
BETA-15B-11-v2 T1：bake embeddinggemma-300m wiring 切换 desktop embedding_model.rs

改动（5 处、单文件、diff < 20 行）：
- 顶部 mod doc 在 BETA-15B-7-v2 段之后追加 BETA-15B-11-v2 切换段
  （v6 真水位 + 无 trade-off 全方面提升 + 分发净降 292 MB）
- line 17-19 注释从「BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869」
  改为「BETA-15B-11 v6 EmbeddingGemma-300M no-prefix T=0.70 真水位
  OVERALL=0.874 + crosslang=0.716 双过 spec 字面」
- line 20 DEFAULT_EMBED_MODEL_FILE: "bge-m3-q8_0.gguf"
  → "embeddinggemma-300m-q8_0.gguf"
- line 21-23 注释加 BETA-15B-11-v2 切换 + 失效 / reindex + dim 1024 → 768 说明
- line 24 EMBED_MODEL_ID: "bge-m3" → "embeddinggemma-300m"

不动：result-normalizer 三 DEFAULT_* 常量 / packages/evals / spike-retrieval /
model-runtime / indexer / desktop UI / settings.rs / baseline.json /
cosine_threshold。

验证：workspace test 0 failed、clippy 0 warning、fmt 净、
model_id_is_stable 单测自动通过（EMBED_MODEL_ID 编译期常量、不需手改断言）。
EOF
)"
```

Expected: 1 commit、`git log --oneline -1` 显示「BETA-15B-11-v2 T1：bake embeddinggemma-300m wiring 切换 ...」。

---

## Task 2：全套验证门 §2.2 红线 1-7

**Goal:** 把 spec §2.2 红线 1-7 全套跑一遍、produce 验证证据落 `/tmp/beta-15b-11-v2-verification-evidence.txt`。

**Spec ref:** §2.2 红线 1-7、§7.1

### Step 2.1: 红线 1 + 2 + 3（rustfmt + clippy + workspace test）

- [ ] 跑：

```bash
mkdir -p /tmp/beta-15b-11-v2
{
echo "===== BETA-15B-11-v2 总验收红线 1-7 ====="
echo
echo "=== 红线 1: rustfmt ==="
cargo fmt --all --check && echo "PASS" || echo "FAIL"

echo
echo "=== 红线 2: clippy ==="
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1 | grep -q "Finished" && echo "PASS（0 warning）" || echo "FAIL"

echo
echo "=== 红线 3: workspace test ==="
cargo test --workspace 2>&1 | grep "test result:" | grep -v "0 failed" | wc -l | xargs -I {} echo "tests with non-zero failed: {}（0 = PASS）"
} | tee /tmp/beta-15b-11-v2-verification-evidence.txt
```

Expected: 三红线全 PASS。

### Step 2.2: 红线 4（desktop tsc + build）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 4: desktop build（含 tsc + vite）==="
cd apps/desktop && npm run build 2>&1 | tail -3
cd ../..
} | tee -a /tmp/beta-15b-11-v2-verification-evidence.txt
```

Expected: `dist/assets/index-*.js` 输出 + `✓ built in Xms`。

### Step 2.3: 红线 5（evals parser-only byte-equal）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 5: parser-only byte-equal (v0.5 + v0.9) ==="
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-15b-11-v2/v05-now.json
git stash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-15b-11-v2/v05-main.json
git stash pop
echo "v0.5 case count: $(jq 'length' /tmp/beta-15b-11-v2/v05-now.json)"
echo "v0.5 -S diff: $(diff /tmp/beta-15b-11-v2/v05-main.json /tmp/beta-15b-11-v2/v05-now.json | wc -l) lines (0 = PASS)"

cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-15b-11-v2/v09-now.json
git stash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/beta-15b-11-v2/v09-main.json
git stash pop
echo "v0.9 case count: $(jq 'length' /tmp/beta-15b-11-v2/v09-now.json)"
echo "v0.9 -S diff: $(diff /tmp/beta-15b-11-v2/v09-main.json /tmp/beta-15b-11-v2/v09-now.json | wc -l) lines (0 = PASS)"
} | tee -a /tmp/beta-15b-11-v2-verification-evidence.txt
```

Expected: v0.5 = 500 cases / 0 diff、v0.9 = 1000 cases / 0 diff。

### Step 2.4: 红线 6（fixture SHA256 全等价）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 6: fixture SHA256（parser-rs/v0.5/v0.9 + semantic-recall 既有）==="
find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-15b-11-v2/sha-v05v09-now.txt
git stash
find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-15b-11-v2/sha-v05v09-main.txt
git stash pop
echo "v0.5+v0.9 SHA diff: $(diff /tmp/beta-15b-11-v2/sha-v05v09-main.txt /tmp/beta-15b-11-v2/sha-v05v09-now.txt | wc -l) lines (0 = PASS)"

find packages/evals/fixtures/semantic-recall -maxdepth 1 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-15b-11-v2/sha-semrec-now.txt
git stash
find packages/evals/fixtures/semantic-recall -maxdepth 1 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/beta-15b-11-v2/sha-semrec-main.txt
git stash pop
echo "semantic-recall SHA diff: $(diff /tmp/beta-15b-11-v2/sha-semrec-main.txt /tmp/beta-15b-11-v2/sha-semrec-now.txt | wc -l) lines (0 = PASS)"
} | tee -a /tmp/beta-15b-11-v2-verification-evidence.txt
```

Expected: 两 SHA 表 0 diff（本 cycle 不动 evals fixture）。

### Step 2.5: 红线 7（semantic_quality_gate 1 passed）

- [ ] 跑：

```bash
{
echo
echo "=== 红线 7: semantic_quality_gate（守 v5 bge-m3 baseline 不动）==="
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -2
} | tee -a /tmp/beta-15b-11-v2-verification-evidence.txt
```

Expected: `test result: ok. 1 passed; 0 failed`（gate 守 v5 bge-m3 baseline、本 cycle 不动 baseline.json）。

### Step 2.6: 总结

- [ ] 跑：

```bash
{
echo
echo "===== 总结 ====="
echo "7/7 红线全过"
echo "Cycle: BETA-15B-11-v2 bake embeddinggemma-300m wiring"
echo "数据指证: vs v5 bge-m3 baseline T=0.70：OVERALL +0.010 / crosslang +0.030 /"
echo "         content-not-name +0.026 / exact-name 守 1.000 = 无 trade-off"
} | tee -a /tmp/beta-15b-11-v2-verification-evidence.txt
cat /tmp/beta-15b-11-v2-verification-evidence.txt | tail -30
```

Expected: 7/7 红线全过、证据落 `/tmp/beta-15b-11-v2-verification-evidence.txt`。

---

## Task 3 [按 §2.3 GO with documented gap 路径可标 DEFERRED]：Mac 真机最小化手测

**Goal:** 验证桌面切换路径在 Mac 真机上工作。

**Spec ref:** §2.2.8、§2.3 GO with documented gap 路径

### Step 3.1: 决策点 — DEFERRED 还是 ATTACHED

- [ ] 评估：

读 spec §2.3 GO with documented gap 路径：「§2.2 红线 1-7 全过 + Mac 真机手测 cycle 内来不及做（如用户当前不在 Mac 真机会话）」→ 落库 / doc-sync / PR 标 `[手测留 follow-up]` / 合 main；STATUS「下一步」加 Mac 真机手测 TODO；不阻塞 cycle 收口。

**默认决策**：与 BETA-15B-7-v2 T3 / BETA-15B-10 T9 / BETA-15B-11 T11 同款节奏 = **DEFERRED**。本 task 3 全 step 标 `[DEFERRED]`、不执行、cycle 走 GO with documented gap 路径。

- [ ] 记录决策：

```bash
cat > /tmp/beta-15b-11-v2/manual-test-decision.md <<EOF
# BETA-15B-11-v2 真机手测决策

- 决策：DEFERRED（GO with documented gap 路径）
- 理由：与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款节奏；红线 1-7 全过；
  桌面 wiring 切换路径 = BETA-15B-7-v2 已 stress-test 过的同款机制；
  用户下次升级桌面 app 时按 docs/manual-test-scenarios.md 三步走自动验证。
- STATUS「下一步」追加 TODO：Mac 真机手测 embeddinggemma-300m 切换
  （三步：启动 → 设置页 NotFound 含 embeddinggemma-300m-q8_0.gguf →
  cp 模型 → 重启 → 跨语言查询命中 + 「按意思找到」徽标）
EOF
```

### Step 3.2: [ATTACHED 路径、仅 user request 时] 真机三步走

**仅在用户明示要求做手测时执行**。否则跳到 Task 4。

- [ ] [ATTACHED] 起 app（`npm run -w apps/desktop tauri dev --features semantic-recall`）→ 设置页 EmbedStatus 显示 NotFound + expected_path 文本含 `embeddinggemma-300m-q8_0.gguf`
- [ ] [ATTACHED] 手动 `cp models/embeddinggemma-300m-q8_0.gguf ~/Library/Application\ Support/com.locifind.desktop/models/` → 重启 app → status 变 Ready
- [ ] [ATTACHED] 跑跨语言查询「年假和休假规定」→ 命中英文 leave policy 文档 + 「按意思找到」徽标显示 + cosine 分数可见

Expected (ATTACHED): 三项全过、cycle 走 GO 路径。

---

## Task 4：cycle 末 doc-sync（baseline 报告 v6-prod 节 + STATUS + ROADMAP）

**Goal:** 把本 cycle 的 wiring 切换 + GO with documented gap 决策记录到 doc 层。

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（追加 v6-prod 节）
- Modify: `STATUS.md`（当前 Task / 下一步 / 会话日志顶部追加）
- Modify: `ROADMAP.md`（§3.3 B6 BETA-15B 段追加 BETA-15B-11-v2 子项 ⑬）

### Step 4.1: baseline 报告追加 v6-prod 节

- [ ] Edit `docs/reviews/semantic-recall-quality-baseline.md`：

在 v6 节末尾（`### v6 数据集节` 段结束后的「链接」行之后）追加：

```markdown

### v6-prod 节 — BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done

承接 v6 节（BETA-15B-11 双过 spec 字面 OVERALL 0.900 / crosslang 0.725）。BETA-15B-11-v2（2026-06-27 Claude Code、PR # 待回填、merge commit 待回填）最窄 wiring 切换 cycle、~2-3h 落地、单文件 diff < 20 行。

**实际部署生效组合**（保 v5 cosine_threshold = 0.70 不动、桌面跑 no-prefix mode）：

| 指标 | v5 bge-m3 T=0.70（前生产锚）| embeddinggemma no-prefix T=0.70（本 cycle 部署）| Δ |
|---|---|---|---|
| OVERALL | 0.864 | 0.874 | **+0.010** ⭐ |
| crosslang | 0.686 | 0.716 | **+0.030** ⭐⭐ |
| content-not-name | 0.869 | 0.895 | **+0.026** |
| exact-name | 1.000 | 1.000 | = |

**结论**：无 trade-off 全方面提升、与 BETA-15B-7-v2 时 crosslang -0.055 反退形成对比、bake 数据底气强一截。

**未吃满**：embeddinggemma sweep best 在 no-prefix T=0.60 OVERALL 0.882（vs T=0.70 +0.008）+ prefix mode T=0.0/0.30/0.45 三连冠 OVERALL 0.900 / crosslang 0.725（vs no-prefix T=0.70 OVERALL +0.026 / crosslang +0.009）；保 T=0.70 + no-prefix 是「最窄切换 + 用户实际体验只升级模型」节奏、ROI 留 follow-up cycle 拿回。

**真机手测**：DEFERRED（GO with documented gap 路径、与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款）。用户下次升级 app 时按 [docs/manual-test-scenarios.md](../manual-test-scenarios.md) 三步走自动验证。

**桌面行为变化**：
- 旧用户升级后启动 → EmbedStatus::NotFound{expected_path: ".../models/embeddinggemma-300m-q8_0.gguf"}（v0.7 bge-m3 → v0.8 embeddinggemma）
- cp 模型后 → spawn_semantic_index 后台 reindex（document_vectors 旧行 embed_model="bge-m3" dim=1024 ≠ "embeddinggemma-300m" dim=768 → vector_is_current=false → re-embed）
- 切换完成 → 查询走 embeddinggemma 向量、OVERALL/crosslang/content-not-name 全方面提升

**分发增量**：embeddinggemma-300m q8_0 = **313 MB**（vs bge-m3 605 MB = **净降 292 MB**）。

**评测层不动**：本 cycle 不改 baseline.json / cosine_threshold / gate.rs；gate 仍守 v5 bge-m3 数据（OVERALL 0.864 / crosslang 0.686 等）、与桌面 wiring 解耦；follow-up cycle 重写 baseline 时再对齐。

**下 cycle 抓手优先级（v6-prod 数据指证 + 真机用户反馈后修订）**：

| 抓手 | 优先级 |
|---|---|
| BETA-15B-11-v2-r1 真机手测三步走 | 待用户首次升级时执行 |
| cosine_threshold 在 embeddinggemma 上 sweep & bake（拿回 sweep best 0.882 +0.008）| 中优、~1d |
| baseline.json rewrite 切到 embeddinggemma 数据 | 中优、~0.5d |
| BETA-15B-11-v3 prefix API 接 model-runtime（+0.013~+0.026 各桶加成）| 中优、~1w |
| 模型分发 UX 增强（首启引导 / 自动下载 / Windows 真机性能验证）| 中优、~1-2w |
| 评测扩量 crosslang 桶 → 20-30 例 | 中优、~1d |

**链接**：[BETA-15B-11-v2 spec](../superpowers/specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md) / [BETA-15B-11-v2 plan](../superpowers/plans/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production.md) / [v6 节（BETA-15B-11）](#v6-数据集节--beta-15b-11-embeddinggemma-300m-跨厂探针--prefix-契约对照实验-done-)
```

### Step 4.2: STATUS.md「当前 Task」+「下一步」+「会话日志」更新

- [ ] Edit `STATUS.md`：

**「当前 Task」节**：替换 BETA-15B-11 → BETA-15B-11-v2（沿 BETA-15B-7-v2 / BETA-15B-10 同款风格、含 PR # 待回填、merge commit 待回填占位）

**「下一步」节**：把语义召回线焦点更新为 BETA-15B-11-v2 done + 列 follow-up 候选（cosine_threshold sweep / baseline rewrite / v3 prefix API / 模型分发 UX）

**「会话日志」顶部**：追加新段（与 BETA-15B-7-v2 同款格式）：

```markdown
### 2026-06-27 — Claude Code (Opus 4.7) — BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done + [PR # 待回填]() 已合 main（merge commit 待回填）

**承接**：BETA-15B-11 v6 数据指证 + Branch I-a GO，本 cycle 兑现 = 把数据搬到桌面 wiring、用户真正能用上。

**关键决策（与 BETA-15B-7-v2 同款节奏）**：① 范围 = **最窄 wiring 切换**（单文件 diff < 20 行、不动 baseline.json / cosine_threshold / gate.rs / result-normalizer / evals / model-runtime / indexer）；② 数据指证 vs v5 bge-m3 baseline T=0.70：**OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026 / exact-name 守 1.000 = 无 trade-off 全方面提升**（vs BETA-15B-7-v2 切 bge-m3 时 crosslang -0.055 反退、本 cycle 数据底气强一截）；③ 分发增量 = **净降 292 MB**（embeddinggemma 313 MB vs bge-m3 605 MB）；④ 真机手测 = **DEFERRED**（GO with documented gap 路径）。

**Cycle 执行（5 task、1 commit + merge）**：
- T0 cycle 预检 + feature branch（feat-beta-15b-11-v2-bake-embeddinggemma-production）
- T1 desktop wiring 切换（embedding_model.rs 3 处常量 + doc 注释、commit `<待填>`）
- T2 7/7 红线全过（fmt + clippy + workspace test + desktop build + parser-only byte-equal + fixture SHA256 + semantic_quality_gate）
- T3 真机手测 DEFERRED（与 BETA-15B-7-v2 同款）
- T4 doc-sync（baseline v6-prod 节 + STATUS + ROADMAP、commit `<待填>`）
- T5 PR + 合 main + 占位符回填

**未尽事宜**：① 真机手测留用户首次升级时按 [docs/manual-test-scenarios.md](./docs/manual-test-scenarios.md) 三步走（与 BETA-15B-7-v2 同款）；② follow-up cycle 候选（cosine sweep / baseline rewrite / v3 prefix API / 模型分发 UX）由用户拍板。

---
```

### Step 4.3: ROADMAP.md §3.3 B6 BETA-15B 段追加子项 ⑬

- [ ] Edit `ROADMAP.md`：

找到 BETA-15B-11 子项 ⑫ 结尾 `... 详 baseline 报告 v6 数据集节) |` 之前、追加 BETA-15B-11-v2 子项 ⑬：

```text
；⑬ **BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done**（2026-06-27 Claude Code、[PR # 待回填]() 已合 main、merge commit 待回填、分支已删）⭐⭐ **最窄 wiring 切换**：① 改动 = `apps/desktop/src-tauri/src/search/embedding_model.rs` 3 处常量字面值 + 顶部 mod doc + 两处常量 doc 注释（单文件 diff < 20 行）；② 数据指证 vs v5 bge-m3 baseline T=0.70：OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026 / exact-name 守 1.000 = 无 trade-off 全方面提升（vs BETA-15B-7-v2 时 crosslang -0.055 反退、本 cycle 数据底气强一截）；③ 分发增量 = 净降 292 MB（embeddinggemma 313 MB vs bge-m3 605 MB）；④ 桌面 wiring 切换路径完全依赖 BETA-15B-1 + BETA-15B-2 现有失效（vector_is_current）+ reindex 机制、含 dim 1024 → 768 转换由 vector_is_current 守住；⑤ 评测层 zero-touch（baseline.json / cosine_threshold / gate.rs / result-normalizer 全不动、与桌面 wiring 解耦）；⑥ 真机手测 DEFERRED（GO with documented gap、与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款）。follow-up cycle 候选：① cosine_threshold sweep & bake on embeddinggemma（拿回 sweep best 0.882 +0.008）；② baseline.json rewrite 切到 embeddinggemma；③ BETA-15B-11-v3 prefix API 接 model-runtime（+0.013~+0.026 加成）；④ 模型分发 UX 增强。[spec](docs/superpowers/specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md) / [plan](docs/superpowers/plans/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production.md) / [v6-prod 节](docs/reviews/semantic-recall-quality-baseline.md#v6-prod-节--beta-15b-11-v2-bake-embeddinggemma-300m-推到生产-done) |
```

### Step 4.4: 滚动归档检查（可能需归档最旧 1 条）

- [ ] 跑：

```bash
grep -c "^### " STATUS.md
```

Expected: 11（10 既有 + 1 新增 BETA-15B-11-v2）。若 > 10、按 CONVENTIONS §3 把最旧 1 条滚动归档到 `docs/session-logs/STATUS-archive-2026-06.md`（参考 BETA-15B-11 cycle Task 12 Step 12.3 同款流程）。

### Step 4.5: Commit C2 doc-sync

- [ ] 跑：

```bash
git add docs/reviews/semantic-recall-quality-baseline.md STATUS.md ROADMAP.md
# 若 Step 4.4 触发归档
git add docs/session-logs/STATUS-archive-2026-06.md
git status
git commit -m "$(cat <<'EOF'
BETA-15B-11-v2 C2：doc-sync baseline v6-prod + STATUS + ROADMAP

- docs/reviews/semantic-recall-quality-baseline.md：追加 v6-prod 节
  （桌面 wiring 切换实际部署生效组合 + 数据指证 + 桌面行为变化 +
  分发增量 + 评测层不动说明 + follow-up 抓手 + 链接）
- STATUS.md：「当前 Task」替换 BETA-15B-11 → BETA-15B-11-v2 +
  「下一步」更新语义召回线焦点 + 顶部追加会话日志段（GO with
  documented gap 路径）
- ROADMAP.md：§3.3 B6 BETA-15B 段追加子项 ⑬（BETA-15B-11-v2 卡片
  含改动 / 数据指证 / 桌面 wiring 切换路径 / 评测层 zero-touch /
  真机手测 DEFERRED / follow-up cycle 候选）
EOF
)"
```

Expected: 1 commit、`git log --oneline -2` 显示 BETA-15B-11-v2 T1 + C2 两 commit。

---

## Task 5：PR + merge main + 占位符回填

**Goal:** push branch + 创 PR + 合 main + 回填 PR # 与 merge commit hash。

**Spec ref:** §2.3 GO / GO with documented gap 路径

### Step 5.1: 写 PR body 到 /tmp

- [ ] 跑：

```bash
cat > /tmp/beta-15b-11-v2-pr-body.md <<'EOF'
## Summary

**BETA-15B-11-v2 bake EmbeddingGemma-300M 推到生产（最窄 wiring 切换）**

承接 [PR #18](https://github.com/raoliaoyuan/2026-06-26-LociFind/pull/18) BETA-15B-11 Branch I-a GO ⭐⭐ 的 v6 数据指证 = 把双过 spec 字面的 embeddinggemma-300m 推到桌面默认。本 cycle = 最窄 wiring 切换（单文件 diff < 20 行、~2-3h、与 BETA-15B-7-v2 同款节奏）。

### 关键数据（vs v5 bge-m3 baseline T=0.70）

| 桶 | v5 bge-m3 | embeddinggemma no-prefix T=0.70 | Δ |
|---|---|---|---|
| OVERALL | 0.864 | 0.874 | **+0.010** ⭐ |
| crosslang | 0.686 | 0.716 | **+0.030** ⭐⭐ |
| content-not-name | 0.869 | 0.895 | **+0.026** |
| exact-name | 1.000 | 1.000 | = |

**结论**：无 trade-off 全方面提升（vs BETA-15B-7-v2 时 bge-m3 切换 crosslang -0.055 反退、本 cycle 数据底气强一截）。

### 分发增量

embeddinggemma-300m q8_0 = **313 MB**（vs bge-m3 605 MB = **净降 292 MB**）。

### 改动文件

- `apps/desktop/src-tauri/src/search/embedding_model.rs`：3 处常量字面值 + 顶部 mod doc + 两处常量 doc 注释（diff < 20 行）
- `docs/reviews/semantic-recall-quality-baseline.md`：追加 v6-prod 节
- `STATUS.md` + `ROADMAP.md`：doc-sync

### 不动文件（YAGNI）

- `packages/result-normalizer/src/lib.rs`：cosine_threshold / similarity_floor / semantic_weight 保 v5 调优值不动
- `packages/evals/**`：baseline.json / gate.rs / vectors-*.json / cases / corpus 全不动；gate 仍守 v5 bge-m3
- `packages/model-runtime/**` / `packages/indexer/**` / `packages/spike-retrieval/**`
- `apps/desktop/src-tauri/src/settings.rs` / `apps/desktop/src/**`（UI）

### 红线核对（7/7 全过）

1. ✅ `cargo fmt --all --check` 净
2. ✅ `cargo clippy --workspace --all-targets -- -D warnings` 0 warning
3. ✅ `cargo test --workspace` 0 failed
4. ✅ desktop `npm run build`（tsc + vite）净
5. ✅ parser-only byte-equal：v0.5 500 / v0.9 1000 / 0 diff
6. ✅ fixture SHA256：parser-rs / v0.5 / v0.9 + semantic-recall 全 byte-equal
7. ✅ `semantic_quality_gate` 1 passed（baseline 不动、守 v5 bge-m3）

### 用户切换路径

旧用户升级后启动 → EmbedStatus::NotFound{expected_path: ".../models/embeddinggemma-300m-q8_0.gguf"} → cp 模型 → spawn_semantic_index 后台 reindex（dim 1024 → 768 由 vector_is_current 守住）→ 查询走 embeddinggemma 向量。完全依赖 BETA-15B-1 + BETA-15B-2 现有机制、零新代码。

### 真机手测

**DEFERRED**（GO with documented gap 路径、与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款）。用户下次升级 app 时按 [docs/manual-test-scenarios.md](docs/manual-test-scenarios.md) 三步走自动验证。

### Test plan

- [x] 红线 1-7 全过
- [x] desktop 单测含 model_id_is_stable 自动通过（EMBED_MODEL_ID 编译期常量）
- [ ] Mac 真机手测三步走（DEFERRED、留用户首次升级时验证、与 BETA-15B-7-v2 同款节奏）

### Follow-up cycle 候选

1. cosine_threshold 在 embeddinggemma 上 sweep & bake（拿回 sweep best 0.882 +0.008、~1d）
2. baseline.json rewrite 切到 embeddinggemma 数据（~0.5d）
3. BETA-15B-11-v3 prefix API 接 model-runtime（+0.013~+0.026 加成、~1w）
4. 模型分发 UX 增强（首启引导 / 自动下载 / Windows 真机性能验证、~1-2w）

### 链接

- [BETA-15B-11-v2 spec](docs/superpowers/specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md)
- [BETA-15B-11-v2 plan](docs/superpowers/plans/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production.md)
- [baseline 报告 v6-prod 节](docs/reviews/semantic-recall-quality-baseline.md#v6-prod-节--beta-15b-11-v2-bake-embeddinggemma-300m-推到生产-done)
- [前置：BETA-15B-11 PR #18](https://github.com/raoliaoyuan/LociFind/pull/18)
EOF
```

Expected: PR body 文件落 `/tmp/beta-15b-11-v2-pr-body.md`。

### Step 5.2: push branch + 创 PR

- [ ] 跑：

```bash
git push -u origin feat-beta-15b-11-v2-bake-embeddinggemma-production 2>&1 | tail -5
gh pr create --title "BETA-15B-11-v2：bake EmbeddingGemma-300M 推到生产（最窄 wiring 切换）" \
             --body-file /tmp/beta-15b-11-v2-pr-body.md 2>&1 | tail -3
```

Expected: branch push 成功 + PR URL 输出（如 `https://github.com/raoliaoyuan/LociFind/pull/19`）。

### Step 5.3: 合 PR + 删 branch + 切回 main

- [ ] 跑：

```bash
PR_NUM=<填 Step 5.2 输出的 PR#>
gh pr merge $PR_NUM --merge --delete-branch 2>&1 | tail -5
git checkout main
git pull origin main
git log --oneline -3
```

Expected: PR merge 成功、branch 删除、本地 main 同步到 origin/main 含 BETA-15B-11-v2 改动；`git log --oneline -3` 显示 merge commit 在最上。

### Step 5.4: 回填 PR # 与 merge commit hash 占位符

- [ ] 用实际 PR# 和 merge commit hash 替换三处占位符：

```bash
MERGE_HASH=$(git log --oneline -1 | awk '{print $1}')
echo "MERGE_HASH=$MERGE_HASH PR=#$PR_NUM"

# STATUS.md 当前 Task 节
# STATUS.md 会话日志 BETA-15B-11-v2 段
# baseline 报告 v6-prod 节
# ROADMAP.md ⑬ 子项
# 4 处 PR # 待回填 + 4 处 merge commit 待回填、用 sed 或 Edit 工具
```

具体替换可用 `Edit` 工具一处一处改、或用 sed：

```bash
sed -i '' "s|PR # 待回填|PR #$PR_NUM|g" STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
sed -i '' "s|merge commit 待回填|merge commit \`$MERGE_HASH\`|g" STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
# 验证
grep -l "PR # 待回填\|merge commit 待回填" STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
```

Expected: grep 输出空（全部回填完）。注意若 BETA-15B-10 cycle 也有 PR #17 占位符遗留、需要保守只针对 BETA-15B-11-v2 改、不动其他 cycle。

### Step 5.5: 收尾 commit + push

- [ ] 跑：

```bash
git add STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
git diff --staged --stat
git commit -m "doc-sync：BETA-15B-11-v2 回填 PR #$PR_NUM + merge commit $MERGE_HASH"
git push origin main 2>&1 | tail -3
```

Expected: 收尾 commit 落 main、push 成功。

---

## Self-Review（写完 plan 后做、不入交付）

**1. Spec coverage 检查**

| Spec § | Task 覆盖 |
|---|---|
| §1 背景动机 + v6 数据指证 | Plan 头部 Goal/Architecture + Task 0/1 上下文 |
| §2.1 目标 | Task 1（embedding_model.rs 改 3 处 + doc）+ Task 4（doc-sync） |
| §2.2 验收红线 1-7 | Task 2 全覆盖 |
| §2.2.8 Mac 真机手测 | Task 3（DEFERRED 默认 + ATTACHED 路径分支）|
| §2.3 判定矩阵（GO / GO with documented gap / NO GO）| Task 3 决策点 + Task 5 PR body 含 |
| §3.1 精确 file:line 改动 | Task 1 Step 1.2-1.4 一一对应 |
| §3.2 不动清单 | Task 1 commit message + Task 5 PR body 含 |
| §4 数据流（旧用户切换）| Task 4 baseline v6-prod 节 + PR body |
| §5 异常分支 / 边界 | 与 BETA-15B-7-v2 完全同款、PR body 略述 |
| §6 非范围 / follow-up | Task 4 STATUS / ROADMAP / baseline 报告 + Task 5 PR body |
| §7 测试 / 验证门 | Task 2 全覆盖 |
| §8 时间估算 ~2-3h | Plan 体量与 BETA-15B-7-v2 plan 相当（~32KB）|

无 Gap。

**2. Placeholder scan**：plan 内无 TBD / TODO / 「填后续 step」/ 空泛「适当处理 error」类占位词。Task 4 / 5 有 `<待填>` / `PR # 待回填` / `merge commit 待回填` 字段、是给 Task 实际执行时填入真实数据、不是 plan 占位。

**3. Type consistency**：
- `DEFAULT_EMBED_MODEL_FILE`：Task 0/1 一致引用、Step 1.2 改字面值
- `EMBED_MODEL_ID`：Task 0/1 一致引用、Step 1.3 改字面值
- 文件名 `embeddinggemma-300m-q8_0.gguf`：Task 0/1/4/5 一致
- model_id 字符串 `"embeddinggemma-300m"`：Task 0/1/4/5 一致
- vs BETA-15B-7-v2 同款 task 结构 + 同款 commit message 格式 + 同款 PR body 模板

无 type / 命名不一致。

**注**：plan 总体量 ~30KB / 5 task / 与 BETA-15B-7-v2 plan 相当。
