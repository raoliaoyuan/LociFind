# BETA-15B-3 簇 A-2：语义臂权重调优 + UI 暴露 设计 spec

> 「语义召回质量做到顶」线下一刀。前置 = BETA-15B-6 持久化评测设施 + baseline 已就绪（合 main 2026-06-22）。范围严格限定为**只调 `DEFAULT_SEMANTIC_WEIGHT`**——FTS 置信度路由按本 cycle 调出的数据决定下 cycle 是否上（YAGNI 防御）。

## 1. 背景与动机

BETA-15B-6 Phase D baseline（OVERALL Recall@10 / nDCG@10）实测：

| 桶 | FTS_N | VEC_N | HYB_N |
|---|---|---|---|
| crosslang | 0.100 | **0.726** | 0.582 |
| concept | 0.326 | 0.877 | 0.795 |
| content-not-name | 0.849 | 0.833 | 0.920 |
| exact-name | 1.000 | 1.000 | **1.000** |
| OVERALL | 0.481 | **0.864** | 0.832 |

**关键发现**：当前 hybrid（`DEFAULT_SEMANTIC_WEIGHT=2.0`）整体略低于纯向量（OVERALL nDCG 0.832 vs 0.864；crosslang 0.582 vs 0.726）——在这套语义占优评测集上，FTS 臂给 hybrid **添了噪声**（错把同语言 g1 次级相关排上去污染 nDCG）。

**这是诚实的调优窗口**：抬高 `DEFAULT_SEMANTIC_WEIGHT` 让 hybrid 逼近/达到纯向量水位，**同时严守 exact-name=1.0 不退化**（FTS 对同语言精确查询仍重要的约束）。

**为什么不能直接砍 FTS 臂**：exact-name 桶 FTS=HYB=1.000 是约束。这只是融合权重的取舍，不是去掉一臂。

## 2. 目标与验收

### 2.1 目标

- **抬高 hybrid 水位**：OVERALL nDCG 至少与纯向量持平（≥ 0.864）；crosslang nDCG 显著向纯向量靠拢（≥ 0.700）。
- **不破红线**：exact-name HYB_R = 1.000 不变（硬断言）。
- **不留遗物**：调优结果 bake 进 `DEFAULT_SEMANTIC_WEIGHT` 默认 + `baseline.json` 新水位，回归门从此守护新水位。
- **UI 对称**：把 `semantic_weight` 加入 `AppSettings`（与 `semantic_similarity_floor` 同模式），设置页可调，高级用户可覆盖默认。

### 2.2 验收红线（不可回归）

1. `cargo test --workspace` 0 failed；含回归门 `semantic_quality_gate` 用**新 baseline** pass。
2. clippy `-D warnings` 0；fmt 净；前端 tsc + vite build 净。
3. **evals parser-only byte-equal 不变**（v0.5=473 / v0.9=877）——本刀不动 parser/索引/融合算法签名。
4. **新 baseline 实测**：exact-name HYB_R=1.0；OVERALL nDCG ≥ 0.864；crosslang nDCG ≥ 0.700（按 sweep 实际结果调整目标值并写 baseline）。
5. **clamp 守护**：`AppSettings::resolve_semantic_weight()` clamp[0.5, 50.0]，越界值不进 `fuse_rrf`。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- `semantic_quality` binary 加 `--semantic-weight=<f64>` CLI flag（不改 default，沿用 `DEFAULT_SEMANTIC_WEIGHT`）。
- 跑 sweep（`2.0, 3.0, 4.0, 6.0, 10.0, 20.0`）人工选 w*。
- bake `DEFAULT_SEMANTIC_WEIGHT = w*`（单一默认源）。
- `AppSettings.semantic_weight: Option<f64>` + `resolve_semantic_weight() -> f64`（unwrap_or 默认 + clamp）。
- 生产侧调 `fuse_rrf` 处改为 live-read（与 floor 同模式：`weight_provider` 闭包每查询读 settings）。
- 设置页加「语义臂权重（融合：FTS vs 向量）」数字输入。
- 写 `baseline.json` 新水位。
- 追加调优记录到 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md)。

### 3.2 Out-of-scope（明确 YAGNI）

- **FTS 置信度阈值路由**：按本 cycle weight 调优数据决定下 cycle 是否上。如果纯调 weight 已让 OVERALL/crosslang 达到/接近纯向量、且 exact-name 不破，路由就是 YAGNI。
- **sweep 子命令 / 自动曲线图**：shell loop + 人工读表够。
- **暴露 `DEFAULT_RRF_K`**：BETA-26 已证 k 不敏感。
- **原始 query 入 schema**：另一刀（15B-3 簇 A 列表中独立子项，byte-equal 风险须 router 后置填充）。
- **真机手测**：纯后端旋钮 + 设置项 + UI 一格数字，平凡，不安排手测剧本。

## 4. 架构（与 floor 对称）

```
AppSettings.semantic_weight: Option<f64>   ← settings.json 字段 + 设置页 UI
        │
        ▼ (live-read 闭包，与 floor 同模式)
weight_provider: Arc<dyn Fn() -> f64 + Send + Sync>
        │
        ▼ (查询时每次读)
SemanticIndexBackend / fanout 调用点 → fuse_rrf(lists, k, weight)
        │
        ▼
DEFAULT_SEMANTIC_WEIGHT（result-normalizer 常量，单一默认源）+ clamp[0.5, 50.0]
```

**单一默认源**：常量 `result-normalizer::DEFAULT_SEMANTIC_WEIGHT`。`AppSettings::resolve_semantic_weight()` = `override.unwrap_or(DEFAULT) + clamp`。

**Clamp 取舍**：[0.5, 50.0]
- 下限 0.5：FTS 倒挂（FTS:VEC = 2:1）也无意义，下沉到 0.5 就够探索；< 0.5 等价 FTS 主导，与本特性目标背道而驰。
- 上限 50.0：在 RRF 公式 `weight / (k + rank + 1)` 中，k=60、rank=0 时单条贡献 ≈ weight/61，weight=50 时 VEC 贡献已远超任何 FTS 命中，实质等价于纯向量；更高无意义。

**Live-read 模式**（沿用 floor 实现，2026-06-18 已验证）：`SemanticIndexBackend::new` 或调用点接受 `weight_provider` 闭包，每次查询读 `settings.json` 反序列化最新值——用户改设置即时生效、无须重启 app。

## 5. 调优工作流（spec 阶段执行）

```bash
# 步骤 0：在分支上加 --semantic-weight CLI flag（task 1）
# 步骤 1：跑 sweep（用现有 vectors.json 缓存，零模型推理）
for w in 2.0 3.0 4.0 6.0 10.0 20.0; do
  echo "=== weight=$w ==="
  cargo run -p locifind-evals --bin semantic_quality -- \
    --semantic-weight=$w --json
done | tee /tmp/sweep.log

# 步骤 2：人工读 sweep.log，选满足以下三条的 w*：
#   ① exact-name HYB_R = 1.000
#   ② OVERALL nDCG 最大（≥ 0.864 = 纯向量基准）
#   ③ crosslang nDCG 最大（≥ 0.700 目标）

# 步骤 3：bake w* 到 DEFAULT_SEMANTIC_WEIGHT
# 步骤 4：写新 baseline.json
cargo run -p locifind-evals --bin semantic_quality -- --write-baseline

# 步骤 5：跑回归门验证
cargo test -p locifind-evals --test semantic_quality_gate
```

**透明度承诺**：sweep 表（6 个 w 各自 6 桶 × 2 指标 = 72 数字）+ 选定理由 + 取舍写入 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md) 的「调优记录（2026-06-22）」节。

**异常分支**：
- 如果 **没有 w 能让 crosslang nDCG ≥ 0.700**：诚实说明天花板（可能需要路由 / 更大模型 / 评测集扩量），但仍 bake 一个相对最优 w（OVERALL 最高且 exact-name=1.0），把 crosslang 目标降为「显著优于 0.582」，并把「路由必要性」证据写报告归 15B-3 簇 A 路由子项。
- 如果 **任何 w 都让 exact-name 跌破 1.0**：abort——硬约束被破坏说明合成集设计或 fuse_rrf 实现需复查，回到 brainstorming。

## 6. 代码改动清单

| # | 层 | 文件 | 改动 | 估时 |
|---|---|---|---|---|
| 1 | 评测 binary | `packages/evals/src/bin/semantic_quality.rs` | 加 `--semantic-weight=<f64>` clap 参数，默认 = `DEFAULT_SEMANTIC_WEIGHT`，传入 `score_case` | 0.5h |
| 2 | 调优工作流 | 本机 sweep，**不入仓** | shell loop 跑 binary + 人工读表 + 选 w* | 0.5h |
| 3 | 生产默认 | `packages/result-normalizer/src/lib.rs:92` | bake `DEFAULT_SEMANTIC_WEIGHT = w*` + 更新 doc-comment 说明出处 | 0.1h |
| 4 | settings schema | `apps/desktop/src-tauri/src/settings.rs` | `AppSettings.semantic_weight: Option<f64>` + `resolve_semantic_weight() -> f64`（默认 + clamp[0.5, 50.0]） + 单测 | 0.5h |
| 5 | 调用 wiring | fanout / `SemanticIndexBackend` 调用 `fuse_rrf` 处 | 改成传 `resolve_semantic_weight()`（live-read 闭包） + 现有 floor 模式对照实现 | 1.0h |
| 6 | 设置页 UI | `apps/desktop/src/SettingsPage.tsx`（或 `App.tsx` 同等） | 数字输入「语义臂权重（融合：FTS vs 向量）· 默认 w*」，加入既有保存流 | 0.5h |
| 7 | 回归门 baseline | `packages/evals/fixtures/semantic-recall/baseline.json` | 用 `--write-baseline` 重写为 w* 实测水位 | 0.1h |
| 8 | 报告 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加「调优记录（2026-06-22）」节：sweep 表 + 选定 w* + 解读 + 与 BETA-26 对照 + 是否触发路由必要性 | 0.5h |

**总估**：~3.5h 代码 + 评测/手验

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- [ ] `cargo fmt --all -- --check` 净
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0
- [ ] `cargo test --workspace` 0 failed
- [ ] `cargo test -p locifind-evals --test semantic_quality_gate` pass（新 baseline）
- [ ] `cargo run -p locifind-evals --bin semantic_quality` 实测：
  - exact-name HYB_R = 1.000 ✅
  - OVERALL nDCG ≥ 0.864 ✅
  - crosslang nDCG ≥ 0.700（或诚实说明天花板）
- [ ] evals parser-only byte-equal：v0.5=473 / v0.9=877 不变（本刀不动 parser）
- [ ] desktop 前端 `tsc` + vite build 净
- [ ] settings.json round-trip 测试：写入 `semantic_weight=X.X` → 重启读出一致 → clamp 越界值（如 0.0 / 100.0）安全降级

## 8. 风险与已记忆教训

### 8.1 风险

1. **合成集偏「语义占优」，w* 可能在真实负载偏激进** ——缓解：① exact-name 桶满分守护；② baseline 是合成集水位、非真实负载最终值；③ 下 cycle 路由 + 真实数据校准锤纠偏；④ settings UI 让高级用户可调降。
2. **`SemanticIndexBackend` 现有架构是否支持双 provider（floor + weight）**——需查现状决定 wiring 复杂度。如果当前 floor_provider 设计已通用化，weight_provider 即对称加。
3. **真机用户搜索行为改变**——`DEFAULT_SEMANTIC_WEIGHT` 上调=语义结果排得更靠前，原靠 FTS 蹭进前 10 的同语言精确文档可能掉出 top 10。**缓解**：exact-name 桶守护是兜底；真机若反馈不适，用户可调降 weight。

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本刀不动 coverage，无 drift 风险。
- [[project-evals-reporter-nondeterministic]]：`semantic_quality` binary 输出已用 BTreeMap 有序，无 HashMap 序问题；但 baseline.json diff 须 by-key 比，不能裸 diff。
- [[feedback-per-task-verify-include-fmt]]：每 task 完成必跑 fmt-check + clippy + test 三件套，不只 clippy+test。
- [[project-stale-hybrid-fallback]]：本刀不动 fallback/hybrid 模型层，无 keyword/title 回声风险。

## 9. 链接

- 起点 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md)
- 前置 [BETA-15B-6 spec](2026-06-21-beta-15b-6-semantic-recall-quality-eval-design.md)
- 同簇前刀 [BETA-15B-3 A-1 floor + visible score spec](2026-06-18-beta-15b-3a1-tunable-floor-visible-score-design.md)
- 战略上下文 STATUS / ROADMAP（2026-06-22 收口两分支段）
