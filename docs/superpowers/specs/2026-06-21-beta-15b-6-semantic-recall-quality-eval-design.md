# BETA-15B-6：持久化语义召回质量评测集 + baseline 设计

> 状态：设计已与用户确认（2026-06-21），待用户复审本文档后转 writing-plans。
> 提议 task ID：**BETA-15B-6**（承接 BETA-15B 语义召回旗舰线，质量调优的前置评测设施）。
> 关联：BETA-26（throwaway 探针，真实数据 GO）、BETA-15A（同义词召回评测骨架）、BETA-15B-1/3-A1（语义召回 + 相似度下限）、BETA-15B-5（可解释 v1）。

## 1. 背景与动机

LociFind 的差异化主打是**本地跨语言语义召回**。BETA-26 探针已在真实数据上证明质的胜利（纯模糊子集 FTS5 2.1% → hybrid 88.4%，crosslang +100pp，exact-name 守护不破），用户拍板把它升格为旗舰能力（PROJECT.md 定位已改）。

要把召回质量「做到顶」（RRF 权重调优 / 相似度下限精校 / 更大 embedding 模型 / 原始 query 入 schema），前提是有一个**持久化、可度量、可复现**的 held-out 相关性评测集——否则调优只能凭感觉、无法判断"配置 X 是否优于 Y"。

但现状有两个缺口：

- **BETA-26 的评测集是 throwaway + 隐私剧毒**：68 条评测集 + 4952 篇语料是真机 `$HOME` 抽样，**评测 query 本身明文含真名、精确薪资（年 170 万）、雇主、量化交易记录**，全部 gitignored、在一次性 `spike-retrieval` crate 里（GO/NO-GO 后本可整包删）。
- **`packages/evals` 没有排名评测骨架**：现有 v0.5/v0.9 是 intent-parsing（variant 匹配）、BETA-15A 同义词召回是**集合级 Recall + 子串匹配模拟**（不跑模型、无 `Recall@k`/`nDCG`）。调 RRF 权重/下限改的是**排名位置**，集合级指标看不见。

本 spec 解决这两个缺口，为后续调优铺好可复现的度量地基。

**方向决策（2026-06-21 用户拍板，CONVENTIONS §9）**：
- 语料方案 = **混合**：合成语料入仓做可共享/CI 门控的主基准 + gitignored 真实集作本地"现实校准锤"。
- spec 范围 = **只建评测 + 出 baseline**；调优（权重/下限/模型）留下一 cycle（数据驱动，须先有评测数字）。

## 2. 目标与非目标

### 2.1 目标（in scope）

1. **合成多语言语料集**（入仓、无 PII）：~150–250 篇虚构似真个人文档（zh+en，含正文），覆盖 5 桶检索场景。
2. **分级相关性评测集**（入仓）：~50–70 条 graded 相关性 case（5 桶，grade 1–3）。
3. **排名评测跑法 + 指标**：`Recall@10` + `nDCG@10`，分桶 × 三臂（FTS5 / 向量 / hybrid），跑**生产融合代码**（`result-normalizer::fuse_rrf` + 相似度下限）。
4. **提交合成向量缓存**：合成 doc/query 向量（无 PII）入仓 → 评测**确定性、CI 可门控、不需模型**。
5. **回归门**：集成测试断言 hybrid 关键桶不跌破提交 baseline（跑缓存向量，确定性）。
6. **真实本地锤转正**：`spike-retrieval` 从 throwaway 转为**本地现实校准工具**（真实数据仍 gitignored 原地），文档化周期性本地跑、对照合成集是否同向。
7. **baseline 报告**：当前配置跑合成集的分桶 baseline，提交为回归门阈值 + 报告（`docs/reviews/`）。

### 2.2 非目标（out of scope，明确不做）

- **任何调优**——RRF 权重 / 相似度下限精校 / 更大模型 / 原始 query 入 schema：均留**下一 cycle**（依赖本评测出的数字）。
- **把真实 PII 语料搬进 `packages/evals`**——真实数据永远只留在 gitignored 的 `spike-retrieval`，**不渗进可提交树**。
- **把语义质量评测并入 byte-equal parser 闸门**——它是模型相关的质量基准，与 parser-only 的 v0.5/v0.9 byte-equal 闸门正交，独立跑。
- **改动语义召回的生产行为**——本 spec 只建评测、不调参，生产 evals（v0.5=473/v0.9=877）byte-equal 不变。

## 3. 跑法选型

| 路线 | 机制 | 取舍 | 结论 |
|---|---|---|---|
| **1：合成集 + 提交合成向量缓存** | 合成语料无 PII → doc/query 向量一并提交；评测读缓存向量 + 真 `fuse_rrf` 跑，`--embed` 子模式才调活模型重算 | 确定性、CI 可门控、不需模型即可跑下限/权重评测；排名敏感指标 | ✅ **采纳** |
| 2：每次跑活模型 | 像 BETA-26 run-retrieval 每次 embed | 必须有模型、非确定、慢、CI 不可门控 | ❌ |
| 3：复用 BETA-15A 集合级 Recall | 子串匹配、集合级 | 看不见排名位置——调 RRF 权重/下限度量不到 | ❌ 达不到目标 |

**采纳路线 1 的核心理由**：缓存合成向量让"下限/权重调优"评测纯靠缓存 + 生产融合代码跑——确定性、可复现、CI 门控、零模型依赖；换更大模型时才走 `--embed` 重算。这是唯一同时满足"确定性 CI 门控 + 排名敏感指标"的路线。

## 4. 详细设计

### 4.1 合成语料集（committable）

落点：`packages/evals/fixtures/semantic-recall/corpus.json`

```jsonc
// 数组，每篇一条
{
  "doc_id": "s00042",
  "lang": "en",                 // zh | en
  "title": "Annual Leave & Remote Work Policy",   // 文件名/标题
  "body": "All full-time employees accrue 15 days of paid annual leave ..."  // 正文（语义召回对内容）
}
```

- 规模 ~150–250 篇。需足量**干扰文档**（distractor）使 top-10 命中非平凡。
- **跨语言桶必须有配对虚构主题**：同一假主题既有 zh 文档又有 en 文档（如"年假政策"的中文版 + 英文版），让"中文 query→英文 doc"能真实命中、且 FTS5 trigram 结构性打不中（验证语义臂唯一可赢场景）。
- **生成**：LLM 一次性生成虚构似真文档 → **隐私+质量复核**（无任何真实人名/公司/财务/路径）→ 提交静态 JSON。**全程合成、零 PII**（CONVENTIONS §7）。
- **元数据清单** `packages/evals/fixtures/semantic-recall/README.md`：记 `dataset_name / version / generation_method / privacy_review_status / created_at / reviewer / model_id`（风险清单 §5.2）。

### 4.2 分级相关性评测集（committable）

落点：`packages/evals/fixtures/semantic-recall/cases.json`

```jsonc
{
  "id": "c012",
  "bucket": "crosslang",        // synonym | concept | crosslang | content-not-name | exact-name
  "query": "年假和远程办公的规定",
  "relevant": [
    { "doc_id": "s00042", "grade": 3 },   // 3=完全相关 2=部分相关 1=弱相关
    { "doc_id": "s00043", "grade": 2 }
  ]
}
```

- ~50–70 条，5 桶大致均衡（复刻 BETA-26 桶定义）。
- **exact-name 守护桶**：query 是合成文档的精确标题/文件名 → 验证语义臂不拖垮精确查询（hybrid 不劣于 FTS5）。
- grade 语义同 BETA-26（3/2/1），喂 nDCG 增益。

### 4.3 合成向量缓存（committable）

落点：`packages/evals/fixtures/semantic-recall/vectors.json`

```jsonc
{
  "model_id": "qwen3-embedding-0.6b-q8_0",   // 标明哪个模型产的，换模型即失配须重算
  "dim": 1024,
  "doc_vectors": { "s00042": [0.01, -0.03, ...], ... },     // 每篇 doc 的 embedding
  "query_vectors": { "c012": [0.02, ...], ... }              // 每条 case query 的 embedding
}
```

- 向量是**合成文本**的 embedding → 无 PII，可提交。
- 评测默认读它跑（向量臂 + hybrid 无需模型）。`model_id` 失配时报错提示需 `--embed` 重算。

### 4.4 跑法 + 指标

- **复用 + 扩展 `recall.rs` 骨架**：现有 `load_*`/完整性/分桶报告可复用；**新增排名指标模块**（纯函数）：
  - `recall_at_k(ranked: &[String], relevant: &[(String, u8)], k) -> f64`：top-k 命中的相关文档数 / 相关文档总数。
  - `ndcg_at_k(ranked, relevant, k) -> f64`：`DCG = Σ (2^grade − 1)/log2(rank+1)`，`IDCG` = 理想排序 DCG，`nDCG = DCG/IDCG`。
- **新 binary `semantic_quality`**（`packages/evals/src/bin/`）：
  - 读 corpus + cases + vectors（缓存）。
  - **三臂排名**：
    - **FTS5 臂**：内存 SQLite FTS5 trigram 索引合成正文 → query trigram-OR → BM25 排名 top-k（参考 `spike-retrieval` run-retrieval 做法）。
    - **向量臂**：query 向量 vs 全 doc 向量 cosine（复用 `locifind_indexer::vectors::cosine`）→ top-k。
    - **hybrid 臂**：调**生产** `result-normalizer::fuse_rrf`（默认权重）+ 相似度下限过滤 → top-k。
  - **输出**：分桶 × 三臂 `Recall@10`/`nDCG@10` 表 + 总体 + 与 baseline 对比；`--json` 机读。
  - **`--embed` 子模式**：调活模型（`model-runtime::embed`，feature `semantic-recall`）重算 doc+query 向量、写回 `vectors.json`（换模型时用）。默认构建不编模型、读缓存即可跑。

### 4.5 回归门

落点：`packages/evals/tests/semantic_quality_gate.rs`

- 跑缓存向量（确定性）→ 断言 hybrid 在**关键桶**不跌破提交 baseline，至少：
  - `crosslang` hybrid `Recall@10` ≥ baseline（语义唯一可赢场景，护核心卖点）。
  - `exact-name` hybrid `Recall@10` ≥ baseline（守护：语义不拖垮精确查询）。
- 阈值取 §4.6 baseline 实测值（留小容差防浮点抖动）。随 `cargo test --workspace` 门控。
- corpus/cases/vectors 均 checked-in → 门**始终可跑**（不像 BETA-26 gitignored 缺数据则跳过）。

### 4.6 baseline 产出

- 用当前生产配置（`DEFAULT_SEMANTIC_WEIGHT=2.0` / `DEFAULT_SIMILARITY_FLOOR=0.30` / 当前 embedding 模型）跑合成集 → 记录分桶 × 三臂 `Recall@10`/`nDCG@10`。
- 提交为 §4.5 回归门阈值 + 一份 baseline 报告 `docs/reviews/semantic-recall-quality-baseline.md`（含配置、桶定义、数字、与 BETA-26 真实集的定性对照）。

### 4.7 真实本地锤转正（gitignored，保真校准）

- `spike-retrieval` crate **从 throwaway 转为本地现实校准工具**：
  - 改 `Cargo.toml` 注释（去掉"GO/NO-GO 后可整包删除"），改为"本地现实校准锤，真实数据 gitignored"。
  - 加/更新 `packages/spike-retrieval/README.md`：说明它是**周期性本地跑、对照合成集数字是否同向**的现实锚点（合成集若与真实集背离则警示合成集失真）。
  - 真实 corpus/vectors/evalset **仍 gitignored 留原地**，PII 隔离在此、不渗进 `packages/evals`。
  - **顺带修 BETA-15B-2 记的遗留缺陷**：`spike-retrieval` 无条件 llama-cpp 致 workspace feature 统一拉真 loader（3 个 stub-loader 测试曾崩、加 let-else 止血、根因仍在）——把 `spike-retrieval` 的 llama-cpp 改为可选 feature（与转正"长期保留"一致，根因清除）。

### 4.8 测试与隐私

- **纯函数单测**：`recall_at_k` / `ndcg_at_k` 给定排名+分级 → 正确值（含边界：无相关、全相关、k 大于结果数、grade 影响 nDCG 排序）。
- **完整性测试**：corpus/cases/vectors 三者引用一致（case.relevant 的 doc_id ∈ corpus、vectors 覆盖全 doc+全 case、grade∈1–3、id 唯一），复用 recall.rs 风格。
- **隐私自查**：corpus/cases/vectors 全 checked-in、**断言无真实 PII**——生成时复核 + 一条 grep 式自查（无真名/邮箱/精确金额模式）写进 README 校验清单。
- **生产 byte-equal 不变**：本 spec 不碰 parser/索引/融合生产路径 → v0.5=473 / v0.9=877 parser-only byte-equal 不动（验证步骤纳入 plan）。
- **隐私红线**：可提交树零 PII（CONVENTIONS §7）；真实数据隔离在 gitignored spike-retrieval；评测不外发。

## 5. 待用户复审 / 可微调项

1. **路线 1（缓存合成向量）**：已确认。
2. **合成语料规模**：~150–250 篇 / ~50–70 cases——实现时若区分度不足可上调，会在 baseline 报告说明。
3. **生成方式**：LLM 生成 + 复核 + 提交（用户已接受）。
4. **回归门关键桶**：crosslang + exact-name 守护；是否再加 concept/content-not-name 桶门留实现时据 baseline 稳定性定。
5. **嵌入模型**：缓存向量绑当前 `qwen3-embedding-0.6b`；换模型走 `--embed` 重算（下一 cycle 的事）。

## 6. 收工时需同步（提醒，非本文档内容）

- ROADMAP §3.3：登记 BETA-15B-6（状态/模块/依赖/估时）；`spike-retrieval` 由 throwaway 改标"本地校准工具"。
- STATUS：当前 task 指向 BETA-15B-6；「待用户决策 Class B」的评测语料隐私决策标记为已拍板（混合方案）。
- `docs/third-party-licenses.md`：若 FTS5 内存索引引入新依赖（如 rusqlite 已在则不动）则登记。
