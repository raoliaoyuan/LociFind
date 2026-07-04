# BETA-15B-7-v2 bake bge-m3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 LociFind 桌面默认 embedding 模型从 `qwen3-embedding-0.6b-q4_k_m.gguf` (model_id `qwen3-embedding-0.6b`) 切到 `bge-m3-q8_0.gguf` (model_id `bge-m3`)，落实 BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869 ⭐ 双过 spec 字面 0.864 的数据指证 = bake bge-m3 推到生产。

**Architecture:** 最窄 wiring 切换 = 改 `apps/desktop/src-tauri/src/search/embedding_model.rs` 5 处（顶部 mod doc + line 10 注释 + line 11 字面值 + line 12 注释 + line 13 字面值）+ 0 行 settings.rs（plan 起手已修订 spec 删 settings.rs 行）。完全依赖 BETA-15B-1 + BETA-15B-2 已建立的 `document_vectors.embed_model` 列自动失效 + spawn_semantic_index 后台 reindex 机制。不动 result-normalizer / packages/evals / packages/spike-retrieval / packages/model-runtime / packages/indexer / desktop UI。

**Tech Stack:** Rust 1.x / `llama-cpp-4 = 0.3.2`（已在 BETA-15B-9 升级）/ 单测全部依赖 `EMBED_MODEL_ID` 编译期常量自动跟随、无需手改字面字符串 / superpowers subagent-driven workflow。

**Spec:** [docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md](../specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md)

**Cycle 范围**（spec §3）：仅桌面 wiring 两常量字面值 + doc 注释。**不动** result-normalizer 三 DEFAULT_* 常量 / packages/evals 整个目录 / packages/spike-retrieval / packages/model-runtime / packages/indexer / desktop UI。

**关键 spec 修订**（plan 起手时发现并落地）：spec §3.1 row 4 假设 `apps/desktop/src-tauri/src/settings.rs:17` doc 注释含具体文件名、需要改；plan 起手实测 line 17 doc 只写「BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。」、**不含文件名**、无需改动；spec §2.1 / §3.1 已就地划掉 settings.rs 改动项。本 cycle 唯一改 desktop 源文件 = `embedding_model.rs`。

---

## Task 0：Spec fixup commit（doc only）

**Files:**
- Modify: `docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md`

**说明**：plan 起手实测发现 spec §3.1 row 4 / §2.1 列的「改 settings.rs:17 doc 注释」假设错误（line 17 doc 不含文件名）；spec 已就地划掉该行（划线 + 修订说明）。本 task = 把已就地修订的 spec 文件 commit、记录 fixup。

### Step 0.1: 验证 spec 文件已修订

- [ ] 查看 spec §3.1 改动清单第 4 行是否含划线（~~apps/desktop/src-tauri/src/settings.rs:17~~）+ 修订说明：

```bash
grep -n "settings.rs:17" docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md
```

Expected: 至少两处命中，含 `~~apps/desktop/src-tauri/src/settings.rs:17~~`（划线）+ `(**spec 修订**：...)` 说明。

### Step 0.2: 验证 spec §2.1 改动项划线

- [ ] 查看 spec §2.1 是否含 settings.rs 划线行：

```bash
grep -n "settings.rs:17.*doc 不含具体文件名" docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md
```

Expected: 1 行命中，含 `~~改 \`apps/desktop/src-tauri/src/settings.rs:17\` doc 注释~~（**spec 修订**：...）` 结构。

### Step 0.3: 实测确认 settings.rs:17 doc 实际内容

- [ ] 跑 `sed -n '15,20p' apps/desktop/src-tauri/src/settings.rs` 确认 line 17 实际是 `/// BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。`（不含 `qwen3` 或具体文件名）

Expected: line 17 输出含 `models/` 但不含 `qwen3` 或 `gguf` 字面字符串、与 spec 修订说明一致。

### Step 0.4: Commit spec fixup

- [ ] 跑：

```bash
git add docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md
git commit -m "$(cat <<'EOF'
BETA-15B-7-v2 T0：spec fixup —— 删 settings.rs:17 改动行

plan 起手实测发现 spec §3.1 row 4 / §2.1 列的「改 settings.rs:17 doc 注释」
假设错误：line 17 doc 实际只写「BETA-15B-1：embedding 模型文件路径覆盖
（None = 默认 app 数据目录 models/）。」、不含具体文件名、无需改动。
line 15（model_path）含 `qwen3-0.6b-q4_k_m.gguf` 但属 fallback chat 模型、
本 cycle 不动。spec §2.1 / §3.1 已就地划掉、附 plan Task 0 修订说明。

本 cycle 唯一改 desktop 源文件 = embedding_model.rs。
EOF
)"
```

Expected: 1 commit、SHA 写入分支 head、`git log --oneline -1` 显示「BETA-15B-7-v2 T0：spec fixup ...」。

---

## Task 1：Desktop wiring 切换 embedding_model.rs

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/embedding_model.rs`（5 处：顶部 mod doc + line 10 / 11 / 12 / 13）
- Test: `apps/desktop/src-tauri/src/search/embedding_model.rs::tests::model_id_is_stable`（不改单测断言代码、`EMBED_MODEL_ID` 编译期常量自动跟随）

**说明**：唯一改 desktop 源文件 = 改两常量字面值 + 顶部 mod doc / line 10 / line 12 三处注释。`model_id_is_stable` 单测断言 `assert_eq!(h.model_id(), EMBED_MODEL_ID)` 用编译期常量、不写字面字符串、随 `EMBED_MODEL_ID` 改动自动 == `"bge-m3"`、无需手动改单测。

**TDD 节奏**：因单测断言用编译期常量自动跟随、本 task 不是先红后绿的纯 TDD 路径。改为「基线检查 → 实施 → 重测验自动通过」三步走。

### Step 1.1: 基线检查 —— 跑 desktop 单测确认当前全过

- [ ] 跑：

```bash
cargo test -p locifind-desktop --lib --features semantic-recall search::embedding_model::tests
```

Expected: 4 个单测全过（`feature_off_new_is_unavailable_from_start` / `feature_off_embed_errs` / `model_id_is_stable` / `embedding_model_path_override_from_settings` / `prewarm_feature_off_is_false_and_idempotent`、计 4-5 个依 feature 开关条件 skip）。基线、确认改动前是干净的。

### Step 1.2: 改 line 11 —— DEFAULT_EMBED_MODEL_FILE 字面值

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:10-11`：

把：
```rust
/// 默认 embedding 模型文件名（BETA-26 探针胜出者）。
const DEFAULT_EMBED_MODEL_FILE: &str = "qwen3-embedding-0.6b-q4_k_m.gguf";
```

改为：
```rust
/// 默认 embedding 模型文件名（BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869 ⭐
/// 双过 spec 字面 0.864、BETA-15B-7-v2 bake 切换、crosslang 相对 qwen3-0.6b -0.032 trade-off）。
const DEFAULT_EMBED_MODEL_FILE: &str = "bge-m3-q8_0.gguf";
```

### Step 1.3: 改 line 13 —— EMBED_MODEL_ID 字面值

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:12-13`：

把：
```rust
/// 模型标识（写入 `document_vectors.embed_model`，换模型→旧向量陈旧）。
const EMBED_MODEL_ID: &str = "qwen3-embedding-0.6b";
```

改为：
```rust
/// 模型标识（写入 `document_vectors.embed_model`，换模型→旧向量陈旧）。
/// BETA-15B-7-v2：从 "qwen3-embedding-0.6b" 切到 "bge-m3"、依赖 vector_is_current(model_id)
/// 自动失效旧向量 + spawn_semantic_index 后台 reindex 机制完成迁移。
const EMBED_MODEL_ID: &str = "bge-m3";
```

### Step 1.4: 改顶部 mod doc 注释 line 1

- [ ] Edit `apps/desktop/src-tauri/src/search/embedding_model.rs:1`：

把：
```rust
//! BETA-15B-1：embedding 模型懒加载句柄（镜像 model_fallback 的约定目录 + feature 门控）。
//! 实现 indexer 的 `TextEmbedder`，供索引期与查询期共用。
```

改为：
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

### Step 1.5: 重跑 desktop 单测确认自动全过

- [ ] 跑：

```bash
cargo test -p locifind-desktop --lib --features semantic-recall search::embedding_model::tests
```

Expected: 全过、`model_id_is_stable` 通过（`h.model_id()` 现返 `"bge-m3"` == `EMBED_MODEL_ID`）。

### Step 1.6: 跑全 workspace 单测确认无横向回归

- [ ] 跑：

```bash
cargo test --workspace --all-features 2>&1 | tail -20
```

Expected: 0 failed。本改动只动两个常量字面值 + 注释、不动任何函数签名 / trait 实现 / 调用链、其他 crate 单测不应受影响。

### Step 1.7: Clippy + fmt 净

- [ ] 跑：

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -10
cargo fmt --check
```

Expected: 0 clippy warning、fmt 净（无 stdout）。

### Step 1.8: Commit

- [ ] 跑：

```bash
git add apps/desktop/src-tauri/src/search/embedding_model.rs
git commit -m "$(cat <<'EOF'
BETA-15B-7-v2 T1：bake bge-m3 wiring 切换 desktop embedding_model.rs

改动（5 处、单文件、diff < 20 行）：
- 顶部 mod doc 加 BETA-15B-7-v2 切换说明 + trade-off 一行
- line 10 注释从「BETA-26 探针胜出者」改为「BETA-15B-8 v4-fixup CLS pooling
  真水位 OVERALL=0.869 + BETA-15B-7-v2 bake」
- line 11 DEFAULT_EMBED_MODEL_FILE: "qwen3-embedding-0.6b-q4_k_m.gguf"
  → "bge-m3-q8_0.gguf"
- line 12 注释加 BETA-15B-7-v2 切换 + 失效 / reindex 机制说明
- line 13 EMBED_MODEL_ID: "qwen3-embedding-0.6b" → "bge-m3"

不动：result-normalizer 三 DEFAULT_* 常量 / packages/evals / spike-retrieval /
model-runtime / indexer / desktop UI / settings.rs。

验证：workspace test 0 failed、clippy 0 warning、fmt 净、
model_id_is_stable 单测自动通过（EMBED_MODEL_ID 编译期常量、不需手改断言）。
EOF
)"
```

Expected: 1 commit、`git log --oneline -1` 显示「BETA-15B-7-v2 T1：bake bge-m3 wiring 切换 ...」。

---

## Task 2：全套验证门 §2.2 红线 1-7

**Files:**（无 file 改动、纯跑命令验证）

**说明**：spec §2.2 红线 1-7 一次性跑全、收集证据。重点验「评测层零变化」= vectors-*.json / baseline.json SHA256 与 main 完全等价、evals byte-equal v0.5=473 / v0.9=877 不变、gate 仍守 qwen3-0.6b 数据全过。

### Step 2.1: §2.2 红线 1 —— workspace 单测全过

- [ ] 跑：

```bash
cargo test --workspace --all-features 2>&1 | tail -5
```

Expected: 全过、输出末尾 `test result: ok. N passed; 0 failed; ...`、N 应 ≥ BETA-15B-9 收口时的 862（实际数取最新一次 main run 数 + 本 cycle 新增 0 单测）。

### Step 2.2: §2.2 红线 2 —— clippy 0 warning

- [ ] 跑：

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -5
```

Expected: 0 warning、exit code 0、输出不含 `warning:` 或 `error:`。

### Step 2.3: §2.2 红线 3 —— fmt 净

- [ ] 跑：

```bash
cargo fmt --check
echo "fmt exit: $?"
```

Expected: 无 stdout、`fmt exit: 0`。

### Step 2.4: §2.2 红线 4 —— TypeScript / vite build 净

- [ ] 跑：

```bash
cd apps/desktop && pnpm tsc --noEmit 2>&1 | tail -10
cd apps/desktop && pnpm vite build 2>&1 | tail -10
cd ../..
```

Expected: tsc 无 error 输出、vite build 成功（含 `built in Xs` 或类似）。

### Step 2.5: §2.2 红线 5 / 7 —— evals fixture SHA256 与 main 等价

- [ ] 跑：

```bash
shasum packages/evals/fixtures/semantic-recall/vectors.json \
       packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json \
       packages/evals/fixtures/semantic-recall/vectors-bge-m3.json \
       packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json \
       packages/evals/fixtures/semantic-recall/baseline.json \
       packages/evals/fixtures/semantic-recall/cases.json \
       packages/evals/fixtures/semantic-recall/corpus.json
```

记录输出 SHA256、然后跑：

```bash
git stash
shasum packages/evals/fixtures/semantic-recall/vectors.json \
       packages/evals/fixtures/semantic-recall/vectors-qwen3-0.6b.json \
       packages/evals/fixtures/semantic-recall/vectors-bge-m3.json \
       packages/evals/fixtures/semantic-recall/vectors-qwen3-8b.json \
       packages/evals/fixtures/semantic-recall/baseline.json \
       packages/evals/fixtures/semantic-recall/cases.json \
       packages/evals/fixtures/semantic-recall/corpus.json
git stash pop
```

Expected: 两次 SHA256 完全等价（本 cycle 不动任何评测 fixture）。

注：如果 `git stash` 提示 nothing to stash（因为前面 commit 已落、working tree clean），改用 `git show main:packages/evals/fixtures/semantic-recall/vectors.json | shasum` 等命令逐文件验。

### Step 2.6: §2.2 红线 6 —— evals parser byte-equal v0.5 / v0.9

实际 evals binary CLI（已 verify、`packages/evals/src/bin/evals.rs`）：`--fixtures v0.5|v0.9 --json`（不是 `--version` / `--output`）；reporter 输出含 `elapsed_ms` 等非确定字段（详记忆 [[project-evals-reporter-nondeterministic]]）、不能裸 diff、按 BETA-15B-8 plan 同款规范化对比方法。

- [ ] 跑：

```bash
# 当前 feature branch 头跑
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/evals-v05-current.json 2>/tmp/evals-v05-current.stderr
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json > /tmp/evals-v09-current.json 2>/tmp/evals-v09-current.stderr

# 摘 pass/partial/fail 计数粗对比（首次过滤 reporter 噪声）
jq '[.[] | .status] | group_by(.) | map({status: .[0], n: length})' /tmp/evals-v05-current.json
jq '[.[] | .status] | group_by(.) | map({status: .[0], n: length})' /tmp/evals-v09-current.json
```

Expected: v0.5 pass=473（与 main 一致）、v0.9 pass=877（与 main 一致）。

如要更严格 byte-equal 闸门（与 BETA-15B-8 / 9 同款）：

```bash
git stash 2>&1 || echo "nothing to stash"
git checkout main -- apps/desktop/src-tauri/src/search/embedding_model.rs
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/evals-v05-main.json 2>/dev/null
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json > /tmp/evals-v09-main.json 2>/dev/null

# 规范化对比（按 id 排序 + 去 elapsed_ms 等非确定字段、详 [[project-evals-reporter-nondeterministic]]）
jq -S '[.[] | {id, status, expected, actual_json}] | sort_by(.id)' /tmp/evals-v05-main.json    > /tmp/v05-main-norm.json
jq -S '[.[] | {id, status, expected, actual_json}] | sort_by(.id)' /tmp/evals-v05-current.json > /tmp/v05-curr-norm.json
diff /tmp/v05-main-norm.json /tmp/v05-curr-norm.json
echo "v0.5 diff exit: $?"

jq -S '[.[] | {id, status, expected, actual_json}] | sort_by(.id)' /tmp/evals-v09-main.json    > /tmp/v09-main-norm.json
jq -S '[.[] | {id, status, expected, actual_json}] | sort_by(.id)' /tmp/evals-v09-current.json > /tmp/v09-curr-norm.json
diff /tmp/v09-main-norm.json /tmp/v09-curr-norm.json
echo "v0.9 diff exit: $?"

git checkout HEAD -- apps/desktop/src-tauri/src/search/embedding_model.rs
git stash pop 2>&1 || true
```

Expected: 两个 diff 全无输出、两个 `diff exit: 0`。本 cycle 纯 desktop wiring 改、parser 完全不动、必然 byte-equal。

### Step 2.7: §2.2 红线 7 —— semantic_quality_gate 单测过

- [ ] 跑：

```bash
cargo test -p locifind-evals --test semantic_quality_gate --features semantic-recall -- --include-ignored
```

Expected: `test result: ok. 1 passed; 0 failed`。gate 仍守 qwen3-0.6b 数据全过（本 cycle 不动 baseline.json）。

### Step 2.8: 收集证据、记录到 cycle 工作笔记 / 提交说明

- [ ] 在 `/tmp/beta-15b-7-v2-verification-evidence.txt` 写入证据摘要（cycle 末 PR 描述要用）：

```
BETA-15B-7-v2 §2.2 红线 1-7 验证证据 ($(date))

红线 1 workspace test:  N passed / 0 failed (N=___)
红线 2 clippy:           0 warning
红线 3 fmt:              净
红线 4 tsc + vite:       净 + built in __s
红线 5 fixture SHA256:   与 main 完全等价（7 个 fixture）
红线 6 evals byte-equal: v0.5=473 / v0.9=877 与 main 完全一致（diff 0）
红线 7 gate:             1 passed / 0 failed
红线 8 Mac 真机手测:     [按 Task 3 判 done / deferred]
```

实际数填入并 commit 此证据文件作 PR 引用，**或** 直接在 PR 描述写入摘要（任择其一、与 BETA-15B-8 / 9 风格一致）。

### Step 2.9: 验证 task 无代码改动需 commit

- [ ] 跑：

```bash
git status
```

Expected: working tree clean（本 task 0 file 改动）。无需 commit。

---

## Task 3 [按 §2.3 GO with documented gap 路径可标 deferred]：Mac 真机最小化手测

**Files:**（无 file 改动、真机操作）

**说明**：spec §2.2.8 + §2.2.3 GO with documented gap 路径明示「未在 Mac 真机会话时按 documented gap 处理、不阻塞 cycle 收口」。本 task 仅当当前会话在 Mac 真机（用户能起 LociFind app）才做；否则在 STATUS「下一步」标 TODO、cycle 末 PR 描述写「Mac 真机手测留 follow-up」。

### Step 3.1: 检查当前会话是否在 Mac 真机环境

- [ ] 跑：

```bash
uname -s  # 应输出 Darwin（macOS）
ls apps/desktop/src-tauri/target/release/bundle/dmg/ 2>/dev/null  # 检查有无打包过的 .dmg
```

Expected: Darwin（如非 Darwin、跳到 Step 3.6 标 deferred）。

### Step 3.2: 起 app（feature semantic-recall 开、bge-m3 文件不在 models/）

- [ ] 在 apps/desktop 跑：

```bash
cd apps/desktop
pnpm tauri dev --features semantic-recall 2>&1 | tee /tmp/locifind-tauri-dev.log &
sleep 30  # 等待启动完成
```

或用 `cargo tauri dev` 视项目实际命令。

- [ ] 在 app 内打开「设置页」、查看 embedding model status

Expected: 显示 `NotFound`、`expected_path` 文本含 `bge-m3-q8_0.gguf`、不含 `qwen3-embedding-0.6b-q4_k_m.gguf`。

### Step 3.3: 手动 copy bge-m3 模型到 models/

- [ ] 找到 bge-m3-q8_0.gguf 本机路径（BETA-15B-7 T1 下载到、可能在用户 home 或 ~/Downloads）：

```bash
find ~ -name "bge-m3-q8_0.gguf" 2>/dev/null | head -3
```

- [ ] 找到 LociFind app data dir（macOS 约定）：

```bash
ls "$HOME/Library/Application Support/LociFind/models/" 2>/dev/null
```

- [ ] copy：

```bash
cp <bge-m3 实际路径>/bge-m3-q8_0.gguf "$HOME/Library/Application Support/LociFind/models/"
```

- [ ] 重启 app（杀进程 + 重起 pnpm tauri dev、或 cmd-Q + 再起）

### Step 3.4: 验证 status 变 Ready

- [ ] 设置页 status 应变 `Ready`、控制台 / tracing 应显示模型加载成功（`pooling.rs::detect_pooling_type` 返 `Cls`、prewarm 完成）。

Expected: `EmbedStatus::Ready` 显示在设置页。

### Step 3.5: 跨语言查询验证 + 「按意思找到」徽标 + cosine 分数

- [ ] 在 app 主搜索栏跑「年假和休假规定」（中文 query、期望召回英文 leave policy 类文档）。

Expected:
- 至少前 3 名包含至少 1 个英文文档
- 该英文文档显示「按意思找到」徽标
- 该结果 cosine 分数显示在 0.6 以上（bge-m3 cosine top1 分布 mean=0.719、详 v4-fixup 节）

### Step 3.6: 标 done 或 deferred

- [ ] 在 `/tmp/beta-15b-7-v2-verification-evidence.txt` 末尾追加 Task 3 结论：

如果 Step 3.1-3.5 全过：

```
红线 8 Mac 真机手测: PASS
  - Step 3.2 NotFound expected_path 含 bge-m3-q8_0.gguf ✓
  - Step 3.3-3.4 status 变 Ready ✓
  - Step 3.5 跨语言查询「年假...」召回英文文档 + 徽标 + cosine 0.XX ✓
```

如果非 Mac 真机环境 / 来不及做：

```
红线 8 Mac 真机手测: DEFERRED（按 spec §2.3 GO with documented gap 路径）
  - 本会话非 Mac 真机环境 / 时间不足
  - cycle 收口仍走 GO 路径、cycle 末 STATUS「下一步」加 follow-up TODO
```

---

## Task 4：cycle 末 doc-sync

**Files:**
- Modify: `STATUS.md`（当前 task、会话日志、本机可立即上手列表）
- Modify: `ROADMAP.md`（BETA-15B-7 task 卡片追加 v2 段）
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`（追加 v4-fixup3 节、3-5 行精简）

### Step 4.1: 更新 STATUS.md「当前 Task」段

- [ ] Edit `STATUS.md` 顶部「当前 Task」段、把现 BETA-15B-9 done 描述改为 BETA-15B-7-v2 done 描述（仿照 BETA-15B-8 / 9 风格、含 PR 号占位符 / merge commit 占位符 / 关键决策 / 数据指证 / 下 cycle 抓手）。

样板：

```markdown
**BETA-15B-7-v2 bake bge-m3 推到生产 = done，已合 main（2026-06-26 Claude Code、[PR #XX](https://github.com/raoliaoyuan/LociFind/pull/XX)、merge commit \`<hash>\`、分支已删）⭐ 最窄 wiring 切换、OVERALL +0.008 vs qwen3-0.6b、crosslang -0.055 trade-off 文档明示**。

[…承接 BETA-15B-8 v4-fixup 数据指证、改 desktop 两常量、不动评测层、依赖 model_id 列自动失效 + reindex…]

**关键决策**：① 范围 = 仅桌面 wiring 切换（diff < 20 行、5 task 含 spec fixup + 真机手测可 deferred）；② baseline.json + gate.rs 保 qwen3-0.6b 不动、与桌面解耦；③ 旧用户依赖 model_id 列自动失效 + 后台 reindex；④ cosine_threshold 保 0.70 不动、bge-m3 sweep best 在 T*=0.0/0.30/0.45 留 follow-up；⑤ 真机手测可 deferred（GO with documented gap 路径）。

**下 cycle 抓手**：① cosine_threshold 在 bge-m3 上重 sweep & bake（拿回 +0.023）；② evals baseline.json + gate.rs 红线重锚到 bge-m3 数据；③ 模型分发 UX（首启引导 / 自动下载 / Windows 真机性能验证）；④ 跨厂替代候选（EmbeddingGemma / jina-v3 / bge-multilingual-gemma2 9B、若想冲 crosslang 0.700）。
```

### Step 4.2: 在 STATUS.md「会话日志」顶部追加新条目

- [ ] 仿照最近 2-3 条会话日志结构、追加新条目（标题 = `### 2026-06-26 — Claude Code (Opus 4.7) — BETA-15B-7-v2 bake bge-m3 done + PR #XX 已合 main`）、包括承接 / 关键决策 / 5 task 产出 / 数据指证 / 真机手测路径（PASS or DEFERRED）/ 未尽事宜。

### Step 4.3: 更新 STATUS.md「本机可立即上手」列表

- [ ] 找到 STATUS.md「本机可立即上手的代码层候选」段、移除 BETA-15B-7-v2 条目、把 follow-up 4 候选加进去（cosine sweep / baseline rewrite / 分发 UX / Windows 性能）作新条目。

### Step 4.4: 更新 ROADMAP.md BETA-15B-7 卡片

- [ ] Edit ROADMAP.md 的 BETA-15B-7 task 卡片末尾、追加：

```markdown
**BETA-15B-7-v2 bake bge-m3 推到生产 done（2026-06-26 Claude Code、[PR #XX](https://github.com/raoliaoyuan/LociFind/pull/XX)、merge commit \`<hash>\`）⭐ 最窄 wiring 切换**——5 task / 单文件 diff < 20 行 / spec fixup 删 settings.rs 改动行 / cosine_threshold 保 0.70 不动 / baseline.json + gate.rs 保 qwen3 不动；OVERALL +0.008 vs qwen3-0.6b（数据指证 v4-fixup 表 T=0.70 行）、content-not-name +0.005、exact-name = 1.0、crosslang -0.055 trade-off 文档明示。follow-up cycle 候选：① cosine_threshold 在 bge-m3 上重 sweep & bake / ② evals baseline.json + gate.rs 红线重锚到 bge-m3 数据 / ③ 模型分发 UX / ④ 跨厂替代候选。[spec](docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md) / [plan](docs/superpowers/plans/2026-06-26-beta-15b-7-v2-bake-bge-m3-production.md) / [v4-fixup 节](docs/reviews/semantic-recall-quality-baseline.md)
```

### Step 4.5: 在 baseline 报告追加 v4-fixup3 节

- [ ] Edit `docs/reviews/semantic-recall-quality-baseline.md`、在 v4-fixup2 节末尾追加：

```markdown

### v4-fixup3 节 — BETA-15B-7-v2 bake bge-m3 推到生产 done

承接 v4-fixup 节（BETA-15B-8 infra 修复 + bge-m3 真水位 OVERALL=0.869 ⭐）+ v4-fixup2 节（BETA-15B-9 qwen3-8b 4 hypothesis 全 FAIL）的下 cycle 最高优抓手 = bake bge-m3 推到生产。BETA-15B-7-v2 (2026-06-26 Claude Code、[PR #XX](https://github.com/raoliaoyuan/LociFind/pull/XX)、merge commit \`<hash>\`) 走最窄 wiring 切换路径：改 `apps/desktop/src-tauri/src/search/embedding_model.rs` 两常量字面值 + 3 处 doc 注释、不动 evals 层 / spike-retrieval / model-runtime / indexer / desktop UI / cosine_threshold / floor / weight。

**bake 后实际 ROI（保 cosine_threshold=0.70）**：OVERALL +0.008（0.856→0.864）、content-not-name +0.005（0.870→0.875）、exact-name =（1.000 守住）、crosslang -0.055（0.717→0.662、头号卖点 trade-off、文档明示）。**未吃满 v4-fixup 表 bge-m3 sweep best**（T*=0.0/0.30/0.45 OVERALL=0.869 / crosslang=0.685、约 +0.005 / +0.023 ROI 留 follow-up cycle 重 sweep cosine_threshold 拿回）。

**评测层零变化**：baseline.json + gate.rs + vectors-*.json + cases/corpus 全部保 qwen3-0.6b 数据、gate 仍守 qwen3-0.6b、SHA256 与 main 等价。桌面与评测两条独立路径解耦、follow-up cycle 重写 baseline 时再对齐。

**下 cycle 抓手**：① **cosine_threshold 在 bge-m3 上重 sweep & bake**（最高优、~1d、数据齐全、ROI +0.005~+0.023）；② **evals baseline.json + gate.rs 红线重锚到 bge-m3 数据**（中优、与 ① 同时或之后、~0.5d）；③ **模型分发 UX**（首启引导 / 自动下载 / Windows 真机性能验证、中优、独立 cycle、~1-2w）；④ **跨厂替代候选**（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B、若想冲 crosslang 0.700 spec 字面、低优、~1-2w）。

**链接**：[BETA-15B-7-v2 spec](../superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md) / [BETA-15B-7-v2 plan](../superpowers/plans/2026-06-26-beta-15b-7-v2-bake-bge-m3-production.md) / [v4-fixup 节 (bge-m3 真水位)](#v4-fixup-数据集节--model-runtime-pooling-type-detection-修复后-bge-m3-真水位beta-15b-8)
```

### Step 4.6: doc-sync commit（占位符版本、PR/merge commit 待后续 Task 5 回填）

- [ ] 跑：

```bash
git add STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
git commit -m "$(cat <<'EOF'
BETA-15B-7-v2 T4：cycle 末 doc-sync（PR/merge commit 占位符待 T5 回填）

- STATUS.md 当前 task 改为 BETA-15B-7-v2 done、会话日志追加新条目、
  本机可立即上手列表移除本 cycle + 加 follow-up 4 候选
- ROADMAP.md BETA-15B-7 卡片末尾追加 v2 done 段
- docs/reviews/semantic-recall-quality-baseline.md 追加 v4-fixup3 节

实际 ROI（保 cosine_threshold=0.70）：
- OVERALL +0.008 (0.856→0.864)
- content-not-name +0.005 (0.870→0.875)
- exact-name = (1.000 守住)
- crosslang -0.055 (0.717→0.662) trade-off 文档明示

评测层零变化、gate 仍守 qwen3-0.6b、follow-up cycle 重锚到 bge-m3 数据。

下 cycle 抓手：① cosine_threshold 在 bge-m3 上重 sweep & bake (~1d、+0.005~+0.023)
② evals baseline + gate 红线重锚到 bge-m3 (~0.5d)
③ 模型分发 UX (~1-2w)
④ 跨厂替代候选 (~1-2w)
EOF
)"
```

Expected: 1 commit、`git log --oneline -1` 显示「BETA-15B-7-v2 T4：cycle 末 doc-sync ...」。

---

## Task 5：PR + merge main + 占位符回填

**Files:**
- Modify: `STATUS.md` / `ROADMAP.md` / `docs/reviews/semantic-recall-quality-baseline.md`（回填 PR 号 + merge commit hash）

### Step 5.1: Push feature branch 到 origin

- [ ] 跑：

```bash
git push -u origin feat-beta-15b-7-v2-bake-bge-m3
```

Expected: 5 commit push 成功（T0 spec fixup / T1 wiring / T4 doc-sync）+ 远程分支 `feat-beta-15b-7-v2-bake-bge-m3` 建好。

### Step 5.2: 创建 PR

- [ ] 跑：

```bash
gh pr create --title "BETA-15B-7-v2 bake bge-m3 推到生产" --body "$(cat <<'EOF'
## Summary

BETA-15B-7-v2 bake bge-m3 推到生产 cycle 最窄 wiring 切换版。承接 BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869 ⭐ 双过 spec 字面 0.864 + BETA-15B-9 qwen3-8b 4 hypothesis 全 FAIL（真水位仍未知、不阻塞 bake）的累积数据指证。

改 `apps/desktop/src-tauri/src/search/embedding_model.rs` 5 处（两常量字面值 + 3 处 doc 注释）、依赖 `document_vectors.embed_model` 列自动失效 + spawn_semantic_index 后台 reindex 机制完成迁移、零代码新增。

**不动**：result-normalizer 三 DEFAULT_* 常量 / packages/evals 整个目录 / packages/spike-retrieval / packages/model-runtime / packages/indexer / desktop UI。

## 实际 ROI（保 cosine_threshold=0.70）

| 指标 | qwen3-0.6b T*=0.70（旧）| bge-m3 @ T*=0.70（新）| Δ |
|---|---|---|---|
| OVERALL | 0.856 | 0.864 | **+0.008** |
| content-not-name | 0.870 | 0.875 | **+0.005** |
| exact-name | 1.000 | 1.000 | = |
| crosslang | 0.717 | 0.662 | **-0.055**（trade-off 文档明示）|

**未吃满 v4-fixup 表 bge-m3 sweep best**（T*=0.0/0.30/0.45 OVERALL=0.869 / crosslang=0.685、约 +0.005 / +0.023 ROI 留 follow-up cycle 重 sweep cosine_threshold 拿回）。

## §2.2 验证门红线 1-7 全过

[在此粘贴 /tmp/beta-15b-7-v2-verification-evidence.txt 的内容]

## 真机手测

[按 Task 3 结论 = PASS or DEFERRED 写]

## 下 cycle 抓手

1. **cosine_threshold 在 bge-m3 上重 sweep & bake**（最高优、~1d、ROI +0.005~+0.023）
2. **evals baseline.json + gate.rs 红线重锚到 bge-m3 数据**（中优、~0.5d）
3. **模型分发 UX**（首启引导 / 自动下载 / Windows 真机性能验证、中优、~1-2w）
4. **跨厂替代候选**（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B、低优、~1-2w）

## 链接

- [spec](docs/superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md)
- [plan](docs/superpowers/plans/2026-06-26-beta-15b-7-v2-bake-bge-m3-production.md)
- [baseline 报告 v4-fixup3 节](docs/reviews/semantic-recall-quality-baseline.md)
- 承接 PR：[#13 BETA-15B-8](https://github.com/raoliaoyuan/LociFind/pull/13) / [#14 BETA-15B-9](https://github.com/raoliaoyuan/LociFind/pull/14)
EOF
)"
```

Expected: 输出 PR URL、`gh pr list --state open` 应能查到本 PR。

### Step 5.3: Merge main + 删本地+远程分支

- [ ] 跑：

```bash
gh pr merge --merge --delete-branch 2>&1 | tail -5
```

或如 `gh pr merge` 因 GitHub CLI auth 失败、走本地 fallback（与 BETA-15B-7 / 8 / 9 同款流程）：

```bash
git checkout main
git pull origin main
git merge feat-beta-15b-7-v2-bake-bge-m3 --no-ff -m "Merge pull request #XX from raoliaoyuan/feat-beta-15b-7-v2-bake-bge-m3"
git push origin main
git branch -d feat-beta-15b-7-v2-bake-bge-m3
git push origin --delete feat-beta-15b-7-v2-bake-bge-m3
```

Expected: main 分支前进 1 merge commit、feature 分支本地 + 远程都删除、`gh pr view <XX>` 显示 MERGED。

### Step 5.4: 占位符回填 + 终 commit

- [ ] 记下 PR 号（XX）+ merge commit hash（X 字符 SHA）

- [ ] Edit `STATUS.md` / `ROADMAP.md` / `docs/reviews/semantic-recall-quality-baseline.md` 把 `#XX` 替换为实际 PR 号、`<hash>` 替换为实际 merge commit hash：

```bash
sed -i '' 's/#XX/#27/g; s/<hash>/abc1234/g' STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
```

（实际 PR 号和 hash 用 Step 5.3 输出）

- [ ] 跑：

```bash
git add STATUS.md ROADMAP.md docs/reviews/semantic-recall-quality-baseline.md
git commit -m "doc-sync：BETA-15B-7-v2 收口（PR #XX 已合 main、merge commit <hash> 占位符回填）"
git push origin main
```

Expected: main 分支前进 1 doc-sync commit、占位符全部回填实际值。

### Step 5.5: 收工

- [ ] 跑：

```bash
git status
git log --oneline -8
```

Expected: working tree clean、最近 8 条 commit 含本 cycle 的 T0 / T1 / T4 / T5 + merge commit + 占位符回填、main HEAD 与 origin/main 同步。

---

## 完成判据

按 spec §8 出场标准：

- ✅ Task 1-2 全过、§2.2 红线 1-7 全过
- ✅ Task 3 PASS or DEFERRED（按 §2.3 GO with documented gap 路径）
- ✅ Task 4-5 doc-sync + PR + merge main + 占位符回填全过
- ✅ working tree clean、ROADMAP / STATUS / baseline 报告均含 BETA-15B-7-v2 done 记录

cycle 收工。
