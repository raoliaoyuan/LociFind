# BETA-15B-3 A-2 语义臂权重调优 + UI 暴露 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 调出最优 `DEFAULT_SEMANTIC_WEIGHT`、bake 默认 + 暴露 `AppSettings.semantic_weight` 让用户可调（与 `semantic_similarity_floor` 对称），新水位写入 baseline.json 锁进回归门。

**Architecture:** `result-normalizer::DEFAULT_SEMANTIC_WEIGHT`（单一默认源）→ `AppSettings.semantic_weight: Option<f64>`（用户覆盖）→ `resolve_semantic_weight`（unwrap_or 默认 + clamp[0.5, 50.0]）→ `read_semantic_weight`（settings.json live-read 闭包）→ `run_fanout_merge_rrf` 新签名透传 → `fuse_rrf`。

**Tech Stack:** Rust (workspace) + Tauri 2 + React (TS) + clap + serde + rusqlite。

**Spec:** [2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md](../specs/2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md)

**前置事实（已合 main 2026-06-22）：**
- baseline OVERALL nDCG: VEC 0.864 / HYB 0.832（HYB 略低于 VEC，详 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md)）
- exact-name HYB_R = 1.0 是硬约束
- `floor` 模板（apps/desktop/src-tauri/src/settings.rs:96-114, 140-197）= 完整 live-read provider 实现可照搬

**每 task 验证门必含三件套**（[[feedback-per-task-verify-include-fmt]]）：
```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## Task 1：semantic_quality binary 加 `--semantic-weight` CLI flag

**Goal:** 让评测 binary 能 sweep 不同 weight 值，无须改 const 重建。

**Files:**
- Modify: `packages/evals/src/bin/semantic_quality.rs`（Cli struct + main 调用 score_case）

- [ ] **Step 1：写失败测试（验证 binary CLI 接受 --semantic-weight 且参数透传）**

加到 `packages/evals/src/bin/semantic_quality.rs` 末尾（在 `embed_and_write` 函数后），文件本身已无单测：

```rust
#[cfg(test)]
mod cli_tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn semantic_weight_flag_parses() {
        let cli = Cli::parse_from(["semantic_quality", "--semantic-weight", "4.0"]);
        assert!((cli.semantic_weight - 4.0).abs() < f64::EPSILON);
    }

    #[test]
    fn semantic_weight_defaults_to_const() {
        use locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT;
        let cli = Cli::parse_from(["semantic_quality"]);
        assert!((cli.semantic_weight - DEFAULT_SEMANTIC_WEIGHT).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 2：跑失败测试确认编译错（Cli 还没有 semantic_weight 字段）**

```
cargo test -p locifind-evals --bin semantic_quality 2>&1 | tail -20
```
Expected: `error[E0609]: no field semantic_weight on type Cli`。

- [ ] **Step 3：实现——给 `Cli` 加 `semantic_weight` 字段 + main 用它**

在 `Cli` struct 中（第 16-31 行附近，加在 `model` 字段后）：

```rust
    /// 融合层语义臂权重（默认 = result-normalizer::DEFAULT_SEMANTIC_WEIGHT）。
    /// sweep 用：`--semantic-weight=3.0` 等。
    #[arg(long, default_value_t = DEFAULT_SEMANTIC_WEIGHT)]
    semantic_weight: f64,
```

`main()` 中把 `score_case` 调用的 `DEFAULT_SEMANTIC_WEIGHT` 改成 `cli.semantic_weight`：

```rust
    let scores: Vec<_> = cases
        .iter()
        .map(|c| {
            score_case(
                c,
                &corpus,
                &vectors,
                EVAL_SIMILARITY_FLOOR,
                cli.semantic_weight,   // 原: DEFAULT_SEMANTIC_WEIGHT
                DEFAULT_RRF_K,
                TOP_K,
            )
        })
        .collect();
```

- [ ] **Step 4：跑测试 + 三件套**

```
cargo test -p locifind-evals --bin semantic_quality
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 所有 pass、新增 2 单测 pass、clippy 0、fmt 净。

- [ ] **Step 5：冒烟跑 binary 看新 flag 生效**

```
cargo run -p locifind-evals --bin semantic_quality -- --semantic-weight=3.0 2>&1 | tail -15
```
Expected: 打印 9 行 bucket 表（5 桶 + OVERALL + header）。`HYB_R` / `HYB_N` 应与 baseline（weight=2.0）数字**不同**（验证参数真生效）。

- [ ] **Step 6：commit**

```
git add packages/evals/src/bin/semantic_quality.rs
git commit -m "BETA-15B-3 A-2 task 1：semantic_quality binary 加 --semantic-weight CLI flag

让评测 binary sweep 不同 weight 无须改 const 重建。默认沿用 result-normalizer::DEFAULT_SEMANTIC_WEIGHT 保持向后兼容；CLI 透传到 score_case → hybrid_rank → fuse_rrf。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 2：跑 sweep 选 w*（手动工作流，不入仓 commit）

**Goal:** 用 task 1 的新 flag 跑 sweep，记录数据，按硬约束选 w*。

**Files:** 无代码改动；产出 `/tmp/sweep-15b3a2.log`（不入仓）+ 选定 w* 数值（task 3 用）。

- [ ] **Step 1：跑 sweep**

```bash
cd /Users/alice/Work/LocalFind
rm -f /tmp/sweep-15b3a2.log
for w in 2.0 3.0 4.0 6.0 10.0 20.0; do
  echo "=== weight=$w ===" | tee -a /tmp/sweep-15b3a2.log
  cargo run -p locifind-evals --bin semantic_quality --quiet -- \
    --semantic-weight=$w 2>/dev/null | tee -a /tmp/sweep-15b3a2.log
done
```
Expected: log 文件含 6 段「=== weight=X ===」 + 各段 8 行表（header + 5 桶 + OVERALL）。

- [ ] **Step 2：人工读 sweep.log，按硬约束选 w***

约束顺序（必须按此顺序应用）：
1. **硬红线**：exact-name `HYB_R` = 1.000（任何 w 不满足直接淘汰）
2. **最大化** OVERALL `HYB_N`
3. **附加目标** crosslang `HYB_N` ≥ 0.700（若无 w 满足，记下证据，归 15B-3 簇 A 路由子项；选 OVERALL 最优的 w）

输出格式（手写到 commit message + 后续报告）：
```
sweep 结果摘要（exact-name HYB_R 应恒 1.0）：
  weight=2.0  OVERALL nDCG=0.832  crosslang nDCG=0.582  ← baseline
  weight=3.0  OVERALL nDCG=X.XXX  crosslang nDCG=X.XXX
  weight=4.0  ...
  ...

选定 w* = X.X（OVERALL 0.XXX，crosslang 0.XXX）
理由：满足 exact-name=1.0 + OVERALL 最高 + crosslang ≥0.7（或 [若不达] 显著优于 0.582）
```

- [ ] **Step 3：异常分支处理**

- **任何 w 让 exact-name HYB_R < 1.0**：abort，回 brainstorming 复查 fuse_rrf 实现或合成集设计。**不要继续 task 3**。
- **没有 w 让 crosslang nDCG ≥ 0.700**：选 OVERALL 最大的 w；在 task 8 报告中写「crosslang 路由必要性证据」归下 cycle。**继续 task 3**。

- [ ] **Step 4：把 sweep 表 + 选定 w* 数值记下来**

写到一个临时草稿（笔记 / 暂存 commit message），task 3 commit message 引用、task 8 报告写正式版。**这一步无 commit。**

---

## Task 3：bake `DEFAULT_SEMANTIC_WEIGHT = w*`

**Goal:** 把 task 2 选定的 w* bake 为生产默认。

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs:90-92`

- [ ] **Step 1：写测试验证新默认值（占位测试，task 2 选出 w* 后填实际数字）**

加到 `packages/result-normalizer/src/lib.rs` 的 `mod tests` 末尾（line 308 附近）：

```rust
    #[test]
    fn default_semantic_weight_is_baked_value() {
        // BETA-15B-3 A-2：bake 后的默认值（task 2 sweep 选定）。
        // 修改此处须同步 baseline.json + 跑回归门确认不退化。
        assert!(
            (DEFAULT_SEMANTIC_WEIGHT - /* w* */).abs() < f64::EPSILON,
            "DEFAULT_SEMANTIC_WEIGHT 已变，须同步 baseline.json"
        );
    }
```

把 `/* w* */` 替换为 task 2 选定值（例如 `4.0`）。

- [ ] **Step 2：跑测试确认失败（当前 const 仍 = 2.0）**

```
cargo test -p locifind-result-normalizer default_semantic_weight_is_baked_value 2>&1 | tail
```
Expected: `assertion failed: DEFAULT_SEMANTIC_WEIGHT 已变`。

- [ ] **Step 3：bake 新 default**

修改 `packages/result-normalizer/src/lib.rs:90-92`，旧：

```rust
/// 默认语义臂权重（BETA-26 §4.6「偏向量」默认；FTS 臂权重固定 1.0）。
/// 调优 + 置信度路由属 15B-3，本 MVP 用固定默认。
pub const DEFAULT_SEMANTIC_WEIGHT: f64 = 2.0;
```

新（把 `4.0` 替换为 task 2 选定 w*）：

```rust
/// 默认语义臂权重（FTS 臂权重固定 1.0）。BETA-15B-3 A-2 sweep 选定值
/// （详 docs/reviews/semantic-recall-quality-baseline.md 调优记录节）。
/// 用户可经 AppSettings.semantic_weight 覆盖；clamp[0.5, 50.0]。
pub const DEFAULT_SEMANTIC_WEIGHT: f64 = 4.0;
```

- [ ] **Step 4：跑测试 + 三件套 + 评测回归门**

```
cargo test -p locifind-result-normalizer
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass。**注意**：`cargo test -p locifind-evals --test semantic_quality_gate` 此时**仍 pass**（baseline 还没更新，新 default 把 hybrid 抬到新水位，新水位 ≥ 旧 baseline → gate pass；task 7 才更新 baseline 锁进新水位）。

- [ ] **Step 5：commit**

```
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-3 A-2 task 3：bake DEFAULT_SEMANTIC_WEIGHT = w*

Task 2 sweep 选定 w=X.X（exact-name=1.0 守护、OVERALL nDCG 0.XXX、crosslang nDCG 0.XXX）。
sweep 表 + 取舍详 task 8 baseline 报告调优记录节。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4：AppSettings 加 `semantic_weight` 字段 + resolve/read 函数

**Goal:** settings.json 接纳 `semantic_weight`、`AppSettings` 增字段、`resolve` 规整 + clamp、`read` live-read。与 floor 模板对称。

**Files:**
- Modify: `apps/desktop/src-tauri/src/settings.rs`

- [ ] **Step 1：写失败测试（4 类：default/clamp/NaN/round-trip）**

加到 `apps/desktop/src-tauri/src/settings.rs` 的 `mod tests`（line 125+，在现有 floor 测试旁）：

```rust
    #[test]
    fn resolve_semantic_weight_clamps_and_defaults() {
        assert!(
            (resolve_semantic_weight(None)
                - locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT)
                .abs()
                < f64::EPSILON
        );
        assert!((resolve_semantic_weight(Some(3.0)) - 3.0).abs() < f64::EPSILON);
        // clamp 下界 0.5（< 0.5 拉到 0.5）
        assert!((resolve_semantic_weight(Some(0.1)) - 0.5).abs() < f64::EPSILON);
        // clamp 上界 50.0
        assert!((resolve_semantic_weight(Some(100.0)) - 50.0).abs() < f64::EPSILON);
        // NaN → 默认
        assert!(
            (resolve_semantic_weight(Some(f64::NAN))
                - locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT)
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn old_settings_without_semantic_weight_parses_ok() {
        let json = r#"{"global_shortcut":"Ctrl+Space","search_scope":["~"],"enable_model_fallback":true,"enable_tracing":false}"#;
        let s: AppSettings = serde_json::from_str(json).unwrap();
        assert!(s.semantic_weight.is_none());
    }

    #[test]
    fn read_semantic_weight_reads_or_defaults() {
        assert!(
            (read_semantic_weight(&None)
                - locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT)
                .abs()
                < f64::EPSILON
        );
        let dir = std::env::temp_dir().join(format!("locifind-weight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("settings.json");
        std::fs::write(&f, r#"{"semantic_weight":3.5}"#).unwrap();
        assert!((read_semantic_weight(&Some(f)) - 3.5).abs() < f64::EPSILON);
        std::fs::remove_dir_all(&dir).ok();
    }
```

注意：`locifind-result-normalizer` 在 desktop crate 是否已是 dependency？应该是（fanout 用），但先确认（grep Cargo.toml）。如不是，**Step 3** 里同时改 Cargo.toml。

- [ ] **Step 2：跑测试确认编译失败**

```
cargo test -p locifind-desktop --lib settings:: 2>&1 | tail -15
```
Expected: `error[E0599]: no function or associated item resolve_semantic_weight` 等。

- [ ] **Step 3：实现 AppSettings 字段 + resolve + read**

在 `apps/desktop/src-tauri/src/settings.rs` 的 `AppSettings` struct 加字段（line 20 后）：

```rust
    /// BETA-15B-3 A-2：融合层语义臂权重覆盖（None = 默认 DEFAULT_SEMANTIC_WEIGHT）。
    /// clamp[0.5, 50.0]：下限防 FTS 倒挂、上限防无意义大值。
    pub semantic_weight: Option<f64>,
```

在 `Default for AppSettings` impl 加字段初始化（line 40 附近）：

```rust
            semantic_weight: None,
```

在 settings.rs line 96（DEFAULT_SIMILARITY_FLOOR 后），加 weight 的 const/resolve/read 三件套：

```rust
/// 把设置里的原始 weight 值规整：有限值 clamp 到 [0.5, 50.0]；None / 非有限 → 默认。
pub(crate) fn resolve_semantic_weight(raw: Option<f64>) -> f64 {
    match raw {
        Some(v) if v.is_finite() => v.clamp(0.5, 50.0),
        _ => locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT,
    }
}

/// 从 settings.json live-read 语义臂权重（每次查询调）。读/解析失败 → 默认。
pub(crate) fn read_semantic_weight(settings_path: &Option<std::path::PathBuf>) -> f64 {
    let raw = settings_path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
        .and_then(|v| v.semantic_weight);
    resolve_semantic_weight(raw)
}
```

如果 `locifind-result-normalizer` 不在 desktop Cargo.toml dependencies，加：

```toml
locifind-result-normalizer = { path = "../../../packages/result-normalizer" }
```

- [ ] **Step 4：跑测试 + 三件套**

```
cargo test -p locifind-desktop --lib settings::
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass，新增 3 单测 pass。

- [ ] **Step 5：commit**

```
git add apps/desktop/src-tauri/src/settings.rs apps/desktop/src-tauri/Cargo.toml
git commit -m "BETA-15B-3 A-2 task 4：AppSettings.semantic_weight + resolve/read live-read

与 semantic_similarity_floor 对称：Option<f64> 字段 + Default None +
resolve_semantic_weight clamp[0.5, 50.0]（NaN/None→默认）+ read_semantic_weight
从 settings.json 实时读。3 单测覆盖 clamp 上下界/NaN/None/round-trip/旧文件兼容。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5：`run_fanout_merge_rrf` 签名加 `semantic_weight` + caller 改 live-read

**Goal:** harness `run_fanout_merge_rrf` 不再固定用 `DEFAULT_SEMANTIC_WEIGHT`，改为接受调用方传入的 weight；desktop caller 改传 `read_semantic_weight()` 闭包结果（每查询 live-read）。

**Files:**
- Modify: `packages/harness/src/fanout_merge.rs`（函数签名 + 单测）
- Modify: `apps/desktop/src-tauri/src/search/fanout.rs`（caller 传 weight）
- Modify: `apps/desktop/src-tauri/src/main.rs`（构造 weight_provider 类似 floor_provider，传到调 fanout 的地方）

- [ ] **Step 1：先看 caller 完整签名 + 现有调用形态**

```
grep -nB2 -A3 "run_fanout_merge_rrf" packages/harness/src/fanout_merge.rs apps/desktop/src-tauri/src/search/fanout.rs apps/desktop/src-tauri/src/main.rs
```

记下：
- harness 签名：`pub async fn run_fanout_merge_rrf<R>(backends, expanded, cancel, on_result) -> FanoutOutcome`
- desktop fanout.rs:74 调用点（仅 1 处生产 caller）

- [ ] **Step 2：写失败测试（harness 验证 weight 真透传 fuse_rrf）**

修改 `packages/harness/src/fanout_merge.rs` 的现有 `rrf_fuses_ranks_semantic_weighted` 测试（line 362）：让它显式传入两个 weight 值，断言结果**不同**。先找到现有测试：

```
grep -nA20 "fn rrf_fuses_ranks_semantic_weighted" packages/harness/src/fanout_merge.rs
```

加新测试（在它后面）：

```rust
    #[tokio::test]
    async fn rrf_respects_semantic_weight_parameter() {
        // 同一组后端 + 同一 query，weight=1.0 vs weight=10.0 应产出**可观测不同**的排名。
        // （文档已被语义臂命中第一位时，weight 抬高让它在结果中相对位置更稳）
        let backends = make_two_backend_setup();  // 复用既有 helper 或新建
        let expanded = sample_expanded_intent();
        let mut out_low = Vec::new();
        let _ = run_fanout_merge_rrf(&backends, &expanded, CancellationToken::new(), &mut |m| out_low.push(m), 1.0).await;
        let mut out_high = Vec::new();
        let _ = run_fanout_merge_rrf(&backends, &expanded, CancellationToken::new(), &mut |m| out_high.push(m), 10.0).await;
        // 至少 score 应不同（path 一致也行——证明 weight 参数真用了）
        assert_ne!(
            out_low.iter().map(|m| m.result.score).collect::<Vec<_>>(),
            out_high.iter().map(|m| m.result.score).collect::<Vec<_>>(),
            "不同 weight 应产生不同 RRF 累加 score"
        );
    }
```

**实施时**：若 `make_two_backend_setup` / `sample_expanded_intent` 既有 helper 不存在，写最小新 helper（用 stub backend 返固定两条结果即可）。看既有 `rrf_fuses_ranks_semantic_weighted` 测试代码风格照搬。

- [ ] **Step 3：跑失败测试**

```
cargo test -p locifind-harness rrf_respects_semantic_weight_parameter 2>&1 | tail
```
Expected: 编译错——签名仍是旧的 4 参（多传第 5 个参数）。

- [ ] **Step 4：改 `run_fanout_merge_rrf` 签名**

修改 `packages/harness/src/fanout_merge.rs:102-107`，旧：

```rust
pub async fn run_fanout_merge_rrf<R>(
    backends: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut R,
) -> FanoutOutcome
```

新：

```rust
pub async fn run_fanout_merge_rrf<R>(
    backends: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut R,
    semantic_weight: f64,
) -> FanoutOutcome
```

函数体 line 146 改：

```rust
    let merged = fuse_rrf(lists, DEFAULT_RRF_K, semantic_weight);
```

更新 doc-comment（line 94-100）把「用 DEFAULT_SEMANTIC_WEIGHT」改为「由调用方指定 semantic_weight」。

- [ ] **Step 5：改既有 `rrf_fuses_ranks_semantic_weighted` 测试传 `DEFAULT_SEMANTIC_WEIGHT`**

现有测试不再编译；找到（line 362+）按新签名传第 5 参 `DEFAULT_SEMANTIC_WEIGHT`（行为不变）：

```rust
        let _ = run_fanout_merge_rrf(
            &backends,
            &expanded,
            CancellationToken::new(),
            &mut |m| out.push(m),
            DEFAULT_SEMANTIC_WEIGHT,  // 新增
        ).await;
```

- [ ] **Step 6：给 `SearchDeps` 加 `weight_provider` 字段 + builder method（模仿 `with_model` / `with_embedding` 模式）**

修改 `apps/desktop/src-tauri/src/search.rs:113-134` 的 `SearchDeps` struct，加字段（在 `embedding` 后）：

```rust
    /// BETA-15B-3 A-2 融合层语义臂权重 provider（live-read settings.json）。
    /// `new()` 默认返 `DEFAULT_SEMANTIC_WEIGHT`；main.rs 经 [`with_weight_provider`] 注入
    /// `settings::read_semantic_weight` 闭包，每次查询读最新值。
    weight_provider: std::sync::Arc<dyn Fn() -> f64 + Send + Sync>,
```

`SearchDeps::new()`（line 136-162）的 struct literal 加默认初始化：

```rust
            weight_provider: std::sync::Arc::new(|| {
                locifind_result_normalizer::DEFAULT_SEMANTIC_WEIGHT
            }),
```

加 `with_weight_provider` builder method（紧跟 `with_embedding` 之后，约 line 181 处）：

```rust
    /// 注入 weight provider 闭包（main.rs 用，每次查询 live-read settings.json）。
    #[must_use]
    pub fn with_weight_provider(
        mut self,
        provider: std::sync::Arc<dyn Fn() -> f64 + Send + Sync>,
    ) -> Self {
        self.weight_provider = provider;
        self
    }

    /// 只读：取当前 semantic weight（调闭包 → live-read 设置文件）。
    pub(crate) fn semantic_weight(&self) -> f64 {
        (self.weight_provider)()
    }
```

如 desktop Cargo.toml 还没把 `locifind-result-normalizer` 列依赖，加（task 4 应已加）。

- [ ] **Step 7：改 desktop caller `apps/desktop/src-tauri/src/search/fanout.rs:74`**

旧（line 74）：
```rust
        run_fanout_merge_rrf(&backends, &expanded, cancel, &mut on_result).await
```

新（`deps` 已在 `run_fanout_search` 签名里，line 34）：
```rust
        let semantic_weight = deps.semantic_weight();
        run_fanout_merge_rrf(&backends, &expanded, cancel, &mut on_result, semantic_weight).await
```

**注**：`semantic_weight` 在 fanout 启动时取一次即可（一次查询周期内 weight 不需变；live-read 已发生在闭包调用那一刻，下次查询会再读一次拿到 settings 的最新值）。

- [ ] **Step 8：改 `apps/desktop/src-tauri/src/main.rs:303` 构造点注入 weight_provider**

在 `SearchDeps::new(...)` 链式 `.with_*` 列表里加（模仿 `floor_settings_path` 模板，main.rs:83 附近）：

```rust
            let weight_settings_path = settings_path.clone();
            let deps = search::SearchDeps::new(/* ... 现有参数 ... */)
                /* 其他 .with_* */
                .with_weight_provider(std::sync::Arc::new(move || {
                    settings::read_semantic_weight(&weight_settings_path)
                }));
```

**实施时**：先看 main.rs:303 附近现有 `SearchDeps::new` 的链式调用形态（具体哪几个 `.with_*` 已链），在末尾加 `.with_weight_provider(...)`。注意 `settings_path` 的 clone 时机（floor 已 clone 过、weight 需独立 clone）。

- [ ] **Step 9：跑测试 + 三件套**

```
cargo test -p locifind-harness
cargo test -p locifind-desktop
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass，新 `rrf_respects_semantic_weight_parameter` 单测 pass、既有 `rrf_fuses_ranks_semantic_weighted` 修签名后仍 pass。

- [ ] **Step 10：commit**

```
git add packages/harness/src/fanout_merge.rs apps/desktop/src-tauri/src/search/fanout.rs apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/search.rs
git commit -m "BETA-15B-3 A-2 task 5：run_fanout_merge_rrf 签名加 semantic_weight + live-read wiring

harness:
- run_fanout_merge_rrf 加 semantic_weight: f64 参数（不再固定用 DEFAULT_SEMANTIC_WEIGHT）
- doc-comment 更新
- 新测试 rrf_respects_semantic_weight_parameter 验证 weight 参数真透传 fuse_rrf

desktop:
- main.rs 构造 weight_provider Arc 闭包（模仿 floor_provider）
- fanout.rs/SearchDeps 透传 provider，每查询调一次取最新 weight（live-read settings.json）

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6：设置页加「语义臂权重」UI

**Goal:** 前端 `SettingsPage.tsx` 加数字输入框；`AppSettings` TS 类型加字段；与 floor UI 对称。

**Files:**
- Modify: `apps/desktop/src/pages/SettingsPage.tsx`

- [ ] **Step 1：扩 TS 类型 + 加输入框（一次完成，前端无 unit test 框架，靠 tsc + 真机即可）**

修改 `apps/desktop/src/pages/SettingsPage.tsx:4-17` 的 `AppSettings` interface 加字段：

```typescript
interface AppSettings {
  global_shortcut: string;
  search_scope: string[];
  enable_model_fallback: boolean;
  enable_tracing: boolean;
  // BETA-23：模型文件路径覆盖（null = 默认数据目录 models/）。
  model_path: string | null;
  // BETA-15B-3 簇A-1：语义相似度下限覆盖（null = 默认 0.30，越高越严）。
  semantic_similarity_floor: number | null;
  // BETA-15B-3 簇A-2：融合层语义臂权重覆盖（null = 默认 DEFAULT_SEMANTIC_WEIGHT，clamp[0.5, 50.0]）。
  semantic_weight: number | null;
  // BETA-27：自定义索引根目录（留空 = 系统音乐/文档/图片默认目录）。
  index_roots: string[];
  // BETA-27：排除目录名通配符（留空 = 默认排除 node_modules/.git 等）。
  exclude_globs: string[];
}
```

加输入框紧跟在 floor 输入块（line 249-272）后：

```tsx
        {/* BETA-15B-3 簇A-2：语义臂权重覆盖（live-read，改后重搜即生效）。 */}
        <div style={{ marginBottom: '16px' }}>
          <label style={{ display: 'block', marginBottom: '8px', fontWeight: 500 }}>
            语义臂权重（融合 FTS vs 向量，越高越偏向量，默认 W_STAR）
          </label>
          <input
            type="number"
            min={0.5}
            max={50}
            step={0.5}
            value={settings.semantic_weight ?? W_STAR}
            onChange={e =>
              setSettings({
                ...settings,
                semantic_weight:
                  e.target.value === '' ? null : parseFloat(e.target.value),
              })
            }
            style={{ width: '120px', padding: '8px', borderRadius: '4px', border: '1px solid #ccc' }}
          />
          <p style={{ fontSize: '12px', color: '#666', marginTop: '4px' }}>
            融合时语义臂（按意思）相对 FTS 臂（关键词）的权重。0.5–50；改后重新搜索即生效。
          </p>
        </div>
```

**实施时**：把 `W_STAR` 替换为 task 2 选定的 w*（如 `4.0`）。两处都换：注释里的「默认 W_STAR」和 `value` 的 `?? W_STAR`。

- [ ] **Step 2：tsc + vite build 验证**

```
cd apps/desktop
npx tsc --noEmit
npm run build
```
Expected: 无 TS error、vite 产出 dist/。

- [ ] **Step 3：rust 端再过一遍三件套（前端无 cargo 测试，但要确保 settings 改动不破后端）**

```
cd /Users/alice/Work/LocalFind
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全 pass（task 5 已验过，此处只是兜底）。

- [ ] **Step 4：commit**

```
git add apps/desktop/src/pages/SettingsPage.tsx
git commit -m "BETA-15B-3 A-2 task 6：设置页加「语义臂权重」数字输入框

与「语义相似度下限」UI 对称：number input、min=0.5 max=50 step=0.5、默认显示 w*、
null = 用 DEFAULT_SEMANTIC_WEIGHT。修改后 settings.json 实时被 read_semantic_weight 闭包读到。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7：写新 `baseline.json` + 回归门 pass

**Goal:** 用新 `DEFAULT_SEMANTIC_WEIGHT` 跑 `--write-baseline`、把新水位锁进 `baseline.json` + `semantic_quality_gate` 测试 pass。

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`（cargo run 重写）
- Optional modify: `packages/evals/tests/semantic_quality_gate.rs`（加 exact-name 硬断言）

- [ ] **Step 1：用新默认（task 3 已 bake）跑 `--write-baseline`**

```
cd /Users/alice/Work/LocalFind
cargo run -p locifind-evals --bin semantic_quality -- --write-baseline 2>&1 | tail -10
```
Expected: stderr 「已写 baseline.json（6 桶含 OVERALL）」。

- [ ] **Step 2：diff baseline.json 看变化**

```
git diff packages/evals/fixtures/semantic-recall/baseline.json | head -80
```
Expected: hybrid 字段值变化（HYB_R / HYB_N 应都涨或与 VEC 持平）；exact-name 桶 hybrid_recall 仍 1.0。

**检验红线**：
- exact-name 桶 `"hybrid_recall": 1.0`（必须）
- OVERALL `"hybrid_ndcg"` ≥ 0.864（不达回 task 2 选更激进 w）
- crosslang `"hybrid_ndcg"` 显著高于 0.582（最好 ≥ 0.700）

- [ ] **Step 3：跑回归门 + workspace test**

```
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail
cargo test --workspace 2>&1 | grep "^test result:" | tail -5
```
Expected: gate pass、workspace 全绿。

- [ ] **Step 4：加 exact-name 硬断言保护（防未来调 weight 不小心破红线）**

读现有 `packages/evals/tests/semantic_quality_gate.rs` 看结构（grep `aggregate` 等找模式），在现有断言旁加：

```rust
    // BETA-15B-3 A-2 红线：exact-name 桶 hybrid recall 必须 = 1.0。
    // 调权重/下限/路由的任何改动若破坏这条，应在 evals 阶段就被门挡下。
    let exact_name = aggs.iter()
        .find(|a| a.bucket == "exact-name")
        .expect("exact-name 桶必须存在");
    assert!(
        (exact_name.hybrid_recall - 1.0).abs() < 1e-6,
        "exact-name hybrid recall 跌破 1.0：{} ← 硬红线（FTS 对精确名约束）",
        exact_name.hybrid_recall
    );
```

读现有文件确认 `aggs` 变量名 + `BucketAgg` 字段名 `bucket`/`hybrid_recall` 一致再 paste。

- [ ] **Step 5：再跑回归门确认新断言 pass**

```
cargo test -p locifind-evals --test semantic_quality_gate
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: pass、clippy/fmt 净。

- [ ] **Step 6：commit（baseline + gate 两文件一起）**

```
git add packages/evals/fixtures/semantic-recall/baseline.json packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-3 A-2 task 7：baseline.json 锁进新水位 + exact-name 硬断言

bake DEFAULT_SEMANTIC_WEIGHT=w* 后用 --write-baseline 重写。
新 OVERALL hybrid nDCG=0.XXX（旧 0.832）、crosslang=0.XXX（旧 0.582）、exact-name 恒 1.0。
semantic_quality_gate 新增 exact-name hybrid recall=1.0 硬断言，
防止未来调权重/路由不小心破坏 FTS 精确名约束的红线。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8：追加调优记录到 baseline 报告 + 总验收

**Goal:** 把 task 2 sweep 数据 + 决策证据 + 路由必要性（若有）写入 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md)；跑全套验收 checklist。

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`

- [ ] **Step 1：追加「调优记录（2026-06-22，A-2）」节到 baseline 报告**

在文件末尾（line 59 后）追加：

```markdown
## 调优记录（2026-06-22，BETA-15B-3 A-2）

> 起于 Phase D 关键发现「hybrid 略低于纯向量」。本节记本轮 weight 调优全表 + 取舍 + 与原 baseline 对照。

### Sweep 表

semantic_weight=W 各桶 hybrid recall / nDCG（FTS/VEC 不随 W 变，省）：

| W | exact-name HYB_R | OVERALL HYB_N | crosslang HYB_N | concept HYB_N | synonym HYB_N | content-not-name HYB_N |
| --- | --- | --- | --- | --- | --- | --- |
| 2.0 | 1.000 | 0.832 | 0.582 | 0.795 | 0.905 | 0.920 |
| 3.0 | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX |
| 4.0 | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX |
| 6.0 | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX |
| 10.0 | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX |
| 20.0 | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX | X.XXX |

### 选定 w*

`DEFAULT_SEMANTIC_WEIGHT = X.X`（bake 进 result-normalizer，UI 可覆盖）

理由：
- exact-name HYB_R = 1.000（硬红线守住）
- OVERALL nDCG = 0.XXX（达成「≥纯向量 0.864」目标）/（或：[若不达] 最接近纯向量的 W，差 X.XX）
- crosslang nDCG = 0.XXX（达成「≥0.700」目标）/（或：[若不达] 显著优于 baseline 0.582，详下节路由必要性证据）

### 与原 baseline 对照

| 桶 | 旧 HYB_N (W=2.0) | 新 HYB_N (W=X.X) | Δ |
| --- | --- | --- | --- |
| OVERALL | 0.832 | 0.XXX | +0.XXX |
| crosslang | 0.582 | 0.XXX | +0.XXX |
| ... | ... | ... | ... |

### 路由必要性（按数据决下 cycle 是否上）

[若 crosslang HYB_N ≥ 0.700：]
本轮纯调 weight 已让 crosslang 达标，FTS 置信度路由暂无必要——纳入 15B-3 簇 A backlog，待真实负载反馈再评估。

[若 crosslang HYB_N < 0.700：]
本轮纯调 weight 把 crosslang 从 0.582 抬到 0.XXX，但仍距纯向量 0.726 有 0.XX 距离。证据指向 **FTS 臂在 crosslang 桶给 hybrid 添噪**（FTS 命中同语言 g1 次级相关污染 nDCG）。下 cycle FTS 置信度路由（FTS 命中弱时跳过 FTS 臂）是必要抓手，已挂 15B-3 簇 A 路由子项。

### 已待跟进项更新

- 「待跟进 1（floor 字面量漂移）」未消化——本刀只调 weight，下刀消。
- 「待跟进 2（语料 108 < 150）」未消化——本刀不扩量；若未来 crosslang 区分度退化再扩。
```

**实施时**：表格里的 `X.XXX` 全替换为 task 2 sweep.log 实际数字；选 [若…] 分支并删另一支。

- [ ] **Step 2：总验收三件套 + evals byte-equal 守护**

```
cd /Users/alice/Work/LocalFind
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace 2>&1 | grep "^test result:" | python3 -c '
import sys, re
p, f = 0, 0
for line in sys.stdin:
  mp = re.search(r"(\d+) passed", line)
  mf = re.search(r"(\d+) failed", line)
  if mp: p += int(mp.group(1))
  if mf: f += int(mf.group(1))
print(f"  passed={p}  failed={f}")
'
```
Expected: `passed=830+N failed=0`（N=本 plan 新加测试数：task 1+2、task 4+3、task 5+1、task 7+1 ≈ 7）。

- [ ] **Step 3：parser-only evals byte-equal 守护（最重要的不可回归）**

```
# v0.5 byte-equal（应仍 = 473 pass）
cargo run -p locifind-evals --bin run_evals -- --schema v0.5 --json 2>/dev/null | jq '[.[] | select(.expectation == "pass")] | length'
# v0.9 byte-equal（应仍 = 877 pass）
cargo run -p locifind-evals --bin run_evals -- --schema v0.9 --json 2>/dev/null | jq '[.[] | select(.expectation == "pass")] | length'
```

Expected: 输出 `473` 和 `877`（精确）。

**实施时**：若 binary 名 / 参数与本 step 不符（如 `run_evals` 实际叫别的），先 `grep -l "fn main" packages/evals/src/bin/` 找对名字。

- [ ] **Step 4：前端 tsc + build 兜底**

```
cd apps/desktop && npx tsc --noEmit && npm run build
```
Expected: 净 + dist 产出。

- [ ] **Step 5：commit（最后一刀 + 报告）**

```
git add docs/reviews/semantic-recall-quality-baseline.md
git commit -m "BETA-15B-3 A-2 task 8：baseline 报告追加调优记录 + 总验收过

记 sweep 6 个 weight × 6 桶 hybrid 实测、选 w*=X.X 理由、与旧 baseline 对照、
路由必要性[依实测决定是否归下 cycle]。

总验收：cargo test workspace 全绿、clippy/fmt 净、evals v0.5=473/v0.9=877 byte-equal 不变、
exact-name hybrid recall=1.0 硬约束守住、前端 tsc+build 净。

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## 验收 Checklist 汇总

实施完毕回到这个清单确认每条勾上：

- [ ] Task 1：semantic_quality 加 `--semantic-weight` CLI flag
- [ ] Task 2：sweep + 选 w*（手动，证据落到 task 8 报告）
- [ ] Task 3：`DEFAULT_SEMANTIC_WEIGHT` bake 为 w*
- [ ] Task 4：`AppSettings.semantic_weight` + resolve + read（含 3 单测）
- [ ] Task 5：`run_fanout_merge_rrf` 加 `semantic_weight` 参数 + desktop live-read wiring（含 1 新单测）
- [ ] Task 6：设置页加「语义臂权重」UI
- [ ] Task 7：`baseline.json` 锁新水位 + `semantic_quality_gate` 加 exact-name 硬断言
- [ ] Task 8：baseline 报告追加调优记录 + 总验收

总最终红线：
- [ ] `cargo test --workspace` 全 pass，包含 `semantic_quality_gate`（用新 baseline）
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` = 0
- [ ] `cargo fmt --all -- --check` 净
- [ ] **evals parser-only byte-equal**：v0.5 pass=473 / v0.9 pass=877 精确不变
- [ ] **exact-name 桶 hybrid recall = 1.000**（硬约束，回归门 + spec §2.2 §5）
- [ ] **OVERALL nDCG ≥ 0.864**（纯向量基准）
- [ ] **crosslang nDCG ≥ 0.700**（或诚实归 15B-3 簇 A 路由子项）
- [ ] 前端 tsc + vite build 净

不在本 plan 范围（明确 YAGNI，spec §3.2）：
- ❌ FTS 置信度阈值路由（按 task 2 数据决下 cycle）
- ❌ sweep 子命令 / 曲线图（shell loop 够）
- ❌ 暴露 RRF k（BETA-26 已证不敏感）
- ❌ 原始 query 入 schema（另一刀，独立 byte-equal 风险）
- ❌ 真机手测（纯后端旋钮 + 一格 UI，平凡）
