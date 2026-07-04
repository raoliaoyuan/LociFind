# BETA-15B-11-v2：bake EmbeddingGemma-300M 推到生产（桌面 wiring 切换最窄版）设计 spec

> 承接 BETA-15B-11 cycle（EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验、[PR #18](https://github.com/raoliaoyuan/LociFind/pull/18) merged commit `49b5f4a`）的数据指证：① v6 数据集 no-prefix mode T=0.70 OVERALL **0.874** / crosslang **0.716** ⭐ 双过 spec 字面 0.864 + 0.700 目标；② no-prefix mode T=0.60 sweep best OVERALL 0.882 / crosslang 0.716（Δ vs T=0.70 OVERALL +0.008 / crosslang =）；③ 推理稳定（dim=768、L2 mean=1.0、全零 0/0、无 BETA-15B-9 式全零 bug）；④ prefix mode +0.013 ~ +0.026 各桶加成但**非 GO 必要条件**、不需要 model-runtime 层 prefix API。
>
> **范围哲学**：极窄、只动 desktop wiring。改 [`apps/desktop/src-tauri/src/search/embedding_model.rs:4-24`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L4) 顶部 mod doc + `DEFAULT_EMBED_MODEL_FILE` + `EMBED_MODEL_ID` 三处；**不动** result-normalizer 三个 DEFAULT_* 常量 / packages/evals 整个目录 / packages/spike-retrieval / packages/model-runtime / packages/indexer。cosine 路由阈值（0.70）/ 相似度下限（0.30）/ 语义臂权重（10.0）保 v5 bge-m3 调优值不动；no-prefix mode T=0.70 OVERALL 0.874 仅 -0.008 vs sweep best T=0.60 0.882（数据指证：v6 表 T=0.70 行）、可接受、留 follow-up cycle 视真机用户反馈再 sweep。
>
> **与 follow-up cycle 关系**：本 cycle 故意把以下三件事剥离作独立 follow-up：① cosine_threshold/floor 在 embeddinggemma 上重 sweep & bake；② 模型分发 UX（首启引导 / 自动下载 / Windows 真机性能验证 / 设置页 banner）；③ baseline.json rewrite + gate.rs 红线重锚到 embeddinggemma 数据（评测层切换、与桌面 wiring 解耦）；④ BETA-15B-11-v3 prefix API 接 model-runtime + 桌面索引应用 standard prefix（+0.013~+0.026 加成、可选）。这四件事每件都可单独 cycle，本 cycle 不带。

## 1. 背景与动机

### 1.1 v6 数据指证 = bake 时机已到

BETA-15B-11 cycle Mac Metal embed + 9 阈值 × 2 prefix mode = 18 次 sweep（v5 数据集 81 cases / 127 docs / W=10.0、详 [baseline 报告 v6 节](../../reviews/semantic-recall-quality-baseline.md#v6-数据集节--beta-15b-11-embeddinggemma-300m-跨厂探针--prefix-契约对照实验-done-)）：

| 指标 | v5 bge-m3 T=0.70（生产锚）| embeddinggemma no-prefix T=0.60（sweep best）| embeddinggemma no-prefix T=0.70（保 v5 阈值）| Δ vs bge-m3（保 0.70）|
|---|---|---|---|---|
| **OVERALL** | 0.864 | **0.882** | **0.874** | **+0.010** ⭐ |
| **crosslang** | 0.686 | 0.716 | **0.716** | **+0.030** ⭐⭐ |
| content-not-name | 0.869 | 0.903 | 0.895 | **+0.026** |
| exact-name | 1.000 | 1.000 | 1.000 | = |

**结论**：保 cosine_threshold = 0.70（v5 调优值）+ 切 embeddinggemma-300m + no-prefix mode = **OVERALL +0.010 / crosslang +0.030 / content-not-name +0.026 / exact-name =**。**全方面提升、无 trade-off**（vs BETA-15B-7-v2 时 bge-m3 vs qwen3 crosslang -0.055 反退、本 cycle 数据更顺）。

**对比 BETA-15B-7-v2 关键区别**：BETA-15B-7-v2 切 bge-m3 是 "OVERALL +0.008 但 crosslang -0.055 反退"（trade-off 需文档明示）；BETA-15B-11-v2 切 embeddinggemma 是 **"OVERALL +0.010 + crosslang +0.030 全方面提升"**（无 trade-off）。**bake 数据底气比 BETA-15B-7-v2 强一截**。

### 1.2 为什么用最窄 wiring 切换路径

- **分发增量成本不变**：embeddinggemma-300m q8_0 = **313 MB**（**比 bge-m3 q8_0 605 MB 还小一半**、~292 MB 净降）、Beta 出场分发预算反而下降
- **diff < 20 行**：改三处常量 + doc 注释、回滚成本 = 改回三处
- **依赖 BETA-15B-1 已建立的失效 + reindex 机制**（`packages/indexer/src/doc_db.rs:367` `vector_is_current(model_id)`、`document_vectors.embed_model` 列、`document_vectors.dim` 列）、旧用户切换零代码新增（bge-m3 dim=1024 → embeddinggemma dim=768 的 dim 不一致由 `vector_is_current` 守住）
- **评测层完全解耦**：gate 仍守 v5 bge-m3 baseline（OVERALL 0.864 等）、desktop 跑 embeddinggemma-300m、两条独立路径互不阻塞；follow-up cycle 重写 baseline 时再对齐
- **YAGNI**：分发 UX / Windows 性能 / cosine_threshold 重 sweep / prefix API 都是独立题、混进来 = 多变量耦合、本 cycle 不带

### 1.3 BETA-15B-11 cycle 已知边界与本 cycle 的关系

BETA-15B-11 cycle 收尾时诚实承认的边界：

| 边界 | 来源 | 本 cycle 处理 |
|---|---|---|
| crosslang +0.039 在 14 例桶上可能含运气 | BETA-15B-11 收工边界 | **接受**：v5 baseline 守、即使 follow-up 评测扩量后 crosslang 真水位降到 0.700-0.716 范围、仍 > spec 字面 0.700、bake 决策不动 |
| prefix mode T=0/0.30/0.45 三连冠 plateau | sweep 表 | **接受**：本 cycle 不 bake T、保 0.70 与 v5 同；不动 prefix mode（no-prefix mode 单独已双过字面） |
| 本 cycle 范围 = 评测探针 only / 桌面行为零变化 | BETA-15B-11 spec | **本 cycle 就是兑现**：把 v6 数据搬到桌面 wiring、用户真正能用上 |

## 2. 目标与验收

### 2.1 目标

- 改 [`apps/desktop/src-tauri/src/search/embedding_model.rs:20`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L20) `DEFAULT_EMBED_MODEL_FILE` 从 `"bge-m3-q8_0.gguf"` 切到 `"embeddinggemma-300m-q8_0.gguf"`
- 改 [`apps/desktop/src-tauri/src/search/embedding_model.rs:24`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L24) `EMBED_MODEL_ID` 从 `"bge-m3"` 切到 `"embeddinggemma-300m"`
- 改 [`apps/desktop/src-tauri/src/search/embedding_model.rs:1-9`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L1) 顶部 mod doc + [`embedding_model.rs:17-23`](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L17) 常量 doc 注释升级到位（注明 BETA-15B-11 v6 真水位 + 无 trade-off 全方面提升 + 评测层守 v5 bge-m3 baseline 不动 + cosine_threshold 保 0.70 待 follow-up sweep）
- 收工时 doc-sync：[STATUS.md](../../../STATUS.md)、[ROADMAP.md](../../../ROADMAP.md)、[`docs/reviews/semantic-recall-quality-baseline.md`](../../reviews/semantic-recall-quality-baseline.md)（追加 v6-prod 节简短记录 bake 完成 + 桌面切换路径 + follow-up 候选）

### 2.2 验收红线（不可回归）

1. `cargo test --workspace`：0 failed（**预期 evals 层零变化**：vectors.json / vectors-bge-m3.json / vectors-qwen3-0.6b.json / vectors-embeddinggemma-300m-*.json / baseline.json SHA256 不变；gate 仍验 v5 bge-m3 数据全过；`embedding_model_path_override_from_settings` / `prewarm_feature_off_*` / `feature_off_*` 全过；`model_id_is_stable` 单测自动跟着编译期常量、无需改断言代码）
2. `cargo clippy --workspace --all-targets -- -D warnings`：0 warning
3. `cargo fmt --check`：净
4. `npm run -w apps/desktop build`（含 tsc + vite）：净（desktop 改动是 Rust 端、TS 不受影响）
5. evals byte-equal：`cargo run -p locifind-evals --bin evals -- --fixtures v0.5 / v0.9 --json` 与 main byte-equal（jq -S 规范化后 0 diff）= 500 / 1000 case 数（纯 desktop wiring 改动、parser 无关联）
6. fixture SHA256：parser-rs / v0.5 / v0.9 fixture + semantic-recall 既有所有 vectors-*.json + baseline.json + corpus.json + cases.json 全部与 main 入栈状态完全等价（本 cycle 不重 embed 任何评测 fixtures）
7. `semantic_quality_gate` 单测 1 passed（baseline 不动 + 数据不动）
8. **Mac 真机最小化手测**（不写入正式 manual-test-scenarios.md、本 cycle 只在 cycle 末会话日志记录通过即可）：
   - 起 app → 设置页 EmbedStatus 显示 NotFound + expected_path 文本明确含 `embeddinggemma-300m-q8_0.gguf`
   - 手动 `cp <downloaded>/embeddinggemma-300m-q8_0.gguf` 到 app data dir 的 `models/` → 重启 app → status 变 Ready
   - 跑一次跨语言查询（如「年假和休假规定」期望召回英文 leave policy 文档）→ 命中 + 「按意思找到」徽标显示 + cosine 分数可见

### 2.3 判定矩阵

| 分支 | 触发条件 | 行动 |
|---|---|---|
| **GO**（默认）| §2.2 红线 1-7 全过 + §2.2.8 Mac 真机手测三项全过 | 落库 / doc-sync / PR / 合 main |
| **GO with documented gap** | §2.2 红线 1-7 全过 + Mac 真机手测 cycle 内来不及做（与 BETA-15B-7-v2 / BETA-15B-10 同款）| 落库 / doc-sync / PR 标 `[手测留 follow-up]` / 合 main；STATUS「下一步」加 Mac 真机手测 TODO；不阻塞 cycle 收口 |
| **NO GO**（异常分支）| §2.2 红线 1-7 任意一条不过 | 不发布、回滚 desktop 改动、问题录入 STATUS、cycle 标 done-with-rollback |

## 3. 改动清单

### 3.1 精确到 file:line 的改动

| 文件 | 行 | 改动 |
|---|---|---|
| [embedding_model.rs:1-9](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L1) | 顶部 mod doc | 在 BETA-15B-7-v2 段之后追加 BETA-15B-11-v2 段（2026-06-27）：默认模型从 bge-m3 切到 embeddinggemma-300m、落实 BETA-15B-11 v6 真水位 OVERALL=0.874 / crosslang=0.716 ⭐⭐（vs v5 bge-m3 baseline +0.010 / +0.030、无 trade-off、评测层 baseline 仍守 v5 bge-m3 不动、follow-up cycle 视真机反馈再切换）；cosine 路由阈值 0.70 / 相似度下限 0.30 / 语义臂权重 10.0 保 v5 调优值不动；prefix mode +0.013~+0.026 加分项留 BETA-15B-11-v3 follow-up |
| [embedding_model.rs:17-20](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L17) | 常量 doc + 字符串 | 注释从「BETA-15B-8 v4-fixup CLS pooling 真水位 OVERALL=0.869」改为「BETA-15B-11 v6 EmbeddingGemma-300M no-prefix mode T=0.70 真水位 OVERALL=0.874 + crosslang=0.716 ⭐⭐ 双过 spec 字面、BETA-15B-11-v2 bake 切换」；字面值 `"bge-m3-q8_0.gguf"` → `"embeddinggemma-300m-q8_0.gguf"` |
| [embedding_model.rs:21-24](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L21) | 常量 doc + 字符串 | 注释从「BETA-15B-7-v2：从 "qwen3-embedding-0.6b" 切到 "bge-m3"」改为「BETA-15B-11-v2：从 "bge-m3" 切到 "embeddinggemma-300m"」；字面值 `"bge-m3"` → `"embeddinggemma-300m"` |

**注 1**：BETA-15B-7-v2 spec 中验证过 [embedding_model.rs:222-224](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L222) `model_id_is_stable` 单测代码 `assert_eq!(h.model_id(), EMBED_MODEL_ID);` 用编译期常量、不写字面字符串、**无需手动改单测断言**。本 cycle 沿用此性质。

**注 2**：[settings.rs:17](../../../apps/desktop/src-tauri/src/settings.rs#L17) doc 注释只写「BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。」、**不含具体文件名**、无需改动（BETA-15B-7-v2 spec 起手已确认）。

### 3.2 不动清单（防止越界）

为防 cycle 内越界改动其他层、明确列出本 cycle **不改**的范围：

| 包/文件 | 不动理由 |
|---|---|
| [`packages/result-normalizer/src/lib.rs`](../../../packages/result-normalizer/src/lib.rs) | `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.70` / `DEFAULT_SEMANTIC_WEIGHT = 10.0` / `DEFAULT_RRF_K` / `DEFAULT_SIMILARITY_FLOOR = 0.30` 保 v5 bge-m3 调优值、embeddinggemma 重 sweep 留 follow-up |
| `packages/evals/**` | binary 默认、baseline.json、gate.rs、vectors-*.json（含本 cycle 已入仓的 vectors-embeddinggemma-300m-*.json reference snapshot）、cases/corpus 全部不动；gate 仍守 v5 bge-m3、零变化 |
| `packages/spike-retrieval/**` | embed_corpus / run_retrieval 默认 `models/qwen3-embedding-0.6b-q8_0.gguf` 保留作 BETA-26 探针历史锚 |
| `packages/model-runtime/**` | pooling detection 已在 BETA-15B-8 + BETA-15B-11 落地（含 gemma-embedding 白名单）；本 cycle 不动 llama-cpp-4 版本 / pooling.rs / llama.rs |
| `packages/indexer/**` | embed trait / `vector_is_current(model_id)` / doc_db schema 全不动；旧 vectors 失效与 reindex 走现有机制（dim=1024 → 768 也由 vector_is_current 守住） |
| [apps/desktop/src-tauri/src/search/model_fallback.rs](../../../apps/desktop/src-tauri/src/search/model_fallback.rs) | fallback chat 模型与 embedding 模型解耦、BETA-23 落地的 qwen3-0.6b chat 模型不在本 cycle 范围 |
| `apps/desktop/src/**`（TS / TSX） | UI 完全不动；状态显示 / 设置页 banner / 进度行复用 BETA-15B-2 现有实现 |
| [`docs/manual-test-scenarios.md`](../../manual-test-scenarios.md) | 不写新手测剧本；§2.2.8 真机最小化手测仅在 cycle 末会话日志记录、不入正式剧本 |

## 4. 数据流：旧用户向 EmbeddingGemma-300M 切换

无需新代码、完全依赖 BETA-15B-1 + BETA-15B-2 已建立的失效 + reindex 机制（与 BETA-15B-7-v2 同款路径、仅常量改动）。

```
用户从 v0.7.x（含 BETA-15B-7-v2 bge-m3）升级到 v0.8.0（含本 cycle 改动）
    │
    ▼
v0.8.0 启动
    │  EMBED_MODEL_ID = "embeddinggemma-300m"
    │  DEFAULT_EMBED_MODEL_FILE = "embeddinggemma-300m-q8_0.gguf"
    │
    ▼
EmbeddingModelHandle::is_active() 检查：
  cfg!(feature = "semantic-recall") = true
  resolved_model_path = <data_dir>/models/embeddinggemma-300m-q8_0.gguf
  .exists() = false（旧用户没 embeddinggemma 文件）
    │
    ▼
返回 false → spawn_semantic_index 不启动 prewarm
EmbedStatus::NotFound{expected_path: "…/models/embeddinggemma-300m-q8_0.gguf"}
    │
    ▼
设置页显示「未找到模型文件，期望放置路径：…/models/embeddinggemma-300m-q8_0.gguf」
    │
    ▼  （用户手动 cp embeddinggemma-300m-q8_0.gguf 到 models/）
    │
    ▼
下次 app 启动（或用户手动触发 reindex）
    │
    ▼
EmbedStatus::Ready
spawn_semantic_index 后台 worker：
  prewarm() 加载 embeddinggemma 模型（pooling=Mean 自动检测自 GGUF metadata）+ 暖机
  embed_pending(roots, embedder, progress_cb)：
    indexer 扫 document_vectors 表：
      WHERE doc_id = ? AND embed_model = ? AND source_hash = ?
      旧行 embed_model = "bge-m3" ≠ "embeddinggemma-300m" → vector_is_current = false
      旧行 dim = 1024 ≠ 768 → 即使 model_id 巧合相等也会失效
      → 需 re-embed → upsert_vector with embed_model = "embeddinggemma-300m" + dim = 768
    │
    ▼
设置页「🧠 语义索引中 X/Y」进度行
    │
    ▼  （N 分钟 ~ N 小时，取决于语料大小；embeddinggemma 313 MB 比 bge-m3 605 MB 加载更快）
    │
    ▼
全部 re-embed done、查询走 embeddinggemma 向量
旧 bge-m3 q8_0 文件 `models/bge-m3-q8_0.gguf` 留在磁盘
（下游无消费者、占用 ~605 MB、用户自行清理）
```

**关键不变量**（与 BETA-15B-7-v2 完全同款）：
- 切换全程不阻塞查询（FTS-only 降级仍工作、`EmbedStatus::NotFound` 时 `ready()` 返回 None、`semantic-index` arm 自然空、`fuse_rrf_with_fts_routing` empty-arm early-return guard 兜底）
- 切换不擦数据（document_vectors 旧行保留、`upsert_vector` UPSERT 语义就地替换、不丢文档元数据）
- 切换无误删（旧 bge-m3 模型文件用户自管、本 cycle 不动磁盘）

## 5. 异常分支 / 边界（与 BETA-15B-7-v2 完全同款）

| 场景 | 当前 v0.7 行为 | 切换后 v0.8 行为 | 是否需改 |
|---|---|---|---|
| 模型文件不存在 | NotFound{expected_path = …/bge-m3-…} | NotFound{expected_path = …/embeddinggemma-…} | 否（行为天然正确、文本自动更新） |
| feature `semantic-recall` 关 | Unavailable | 同上、无变化 | 否 |
| 模型加载失败（文件损坏等） | Failed{reason} | 同上、reason 由 llama.cpp 返回 | 否 |
| 旧 vectors blob dim 不匹配 | embed_model 列先过滤、不会读到旧 blob | 同上、`vector_is_current` 守住；本 cycle dim 也变（1024→768）、双重防御 | 否 |
| `settings.embedding_model_path` 用户已设自定义路径 | 优先用 custom 路径 | 不变、尊重用户显式覆盖 | 否（即使用户指定的还是 bge-m3 文件、行为定义良好：用户拿到的是 bge-m3 模型 + EMBED_MODEL_ID="embeddinggemma-300m" 元数据 → 嵌入向量被标 "embeddinggemma-300m" 但其实是 bge-m3 模型生成、**这是用户责任**、本 cycle 不防御）|
| 用户磁盘里同时有 bge-m3 + embeddinggemma 两个模型 | 只用默认路径模型 | 同上、embeddinggemma 默认；bge-m3 文件冗余、用户自清 | 否 |

**特殊边界：settings.embedding_model_path 覆盖**：上表第 5 行的「用户自定义路径仍指向 bge-m3 文件」场景属技术上可能但产品上罕见、与 BETA-15B-7-v2 同款不防御策略。

## 6. 非范围（YAGNI）

明确不在本 cycle 范围、留给独立 follow-up cycle 的题：

1. **cosine_threshold 在 embeddinggemma 上重 sweep & bake**——v6 表数据齐全（no-prefix T*=0.60 sweep best OVERALL 0.882、可再拿回 +0.008）；独立 follow-up cycle = 1d 工作量
2. **模型分发 UX 增强**——首次启动引导对话框 / 自动下载 / 内置打包；与 Beta 出场 BETA-09(a) 同步规划、独立 cycle
3. **Windows 真机性能验证**——v0.7 Windows 实测 bge-m3 性能未明、embeddinggemma 313 MB 比 bge-m3 605 MB 加载快、推理性能未知；独立 cycle 含 model-runtime embed context 复用 + Vulkan/CUDA GPU 加速
4. **evals 层 baseline rewrite**——把 baseline.json / gate.rs 红线从 v5 bge-m3 数据切到 embeddinggemma 数据；与本 cycle 解耦、可视真机用户反馈再做、独立 cycle
5. **BETA-15B-11-v3 prefix API 接 model-runtime**——`embed_query` / `embed_doc` 双 API + 桌面索引应用 standard prefix（+0.013~+0.026 加成）；独立 cycle、~1w
6. **评测集扩量**（v6 81 cases → 100+ cases）——crosslang 桶 14 例校验 prefix +0.039 加成是否含运气；低优、~1d
7. **manual-test-scenarios.md 加 embeddinggemma 切换章节**——本 cycle 真机手测只在会话日志记录、独立 doc cycle 做（与 BETA-09(a) doc work 一起）

## 7. 测试 / 验证门

按 §2.2 红线 1-8 执行、cycle 末 PR 前必须全过。

### 7.1 自动化（CI / 本地）

```bash
# §2.2 红线 1
cargo test --workspace

# §2.2 红线 2
cargo clippy --workspace --all-targets -- -D warnings

# §2.2 红线 3
cargo fmt --all --check

# §2.2 红线 4
npm run -w apps/desktop build

# §2.2 红线 5（parser-only byte-equal、与 BETA-15B-11 同款脚本）
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/v05-now.json
git stash; cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json | \
    jq -S 'map(del(.elapsed_ms)) | sort_by(.id // .case_id // .case // "")' > /tmp/v05-main.json; git stash pop
diff /tmp/v05-main.json /tmp/v05-now.json  # 期望 0 lines
# v0.9 同款

# §2.2 红线 6
find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/sha-now.txt
git stash; find packages/evals/fixtures/v0.5 packages/evals/fixtures/v0.9 -name "*.json" -type f | \
    sort | xargs sha256sum > /tmp/sha-main.txt; git stash pop
diff /tmp/sha-main.txt /tmp/sha-now.txt  # 期望 0 lines

# §2.2 红线 7
cargo test -p locifind-evals --test semantic_quality_gate  # 期望 1 passed
```

### 7.2 手动（Mac 真机）

详 §2.2.8。三步走、~5 min：起 app → cp 模型 → 跨语言查询。

## 8. 时间估算

- T1 改 embedding_model.rs（3 处常量 + doc）：~30 min
- T2 全套验证门 1-7：~30 min
- T3 Mac 真机手测三项（或标 DEFERRED）：~5 min 或 0
- T4 baseline 报告 v6-prod 节：~30 min
- T5 STATUS / ROADMAP doc-sync + commit + PR + 合 main：~30 min

**总计**：~2-3h（GO 路径）或 ~1-1.5h（GO with documented gap 路径）。

## 9. 链接

- [BETA-15B-11 spec](./2026-06-26-beta-15b-11-embeddinggemma-prefix-probe-design.md)（前置 cycle）
- [BETA-15B-11 plan](../plans/2026-06-26-beta-15b-11-embeddinggemma-prefix-probe.md)
- [BETA-15B-7-v2 spec](./2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md)（节奏模板）
- [baseline 报告 v6 数据集节](../reviews/semantic-recall-quality-baseline.md#v6-数据集节--beta-15b-11-embeddinggemma-300m-跨厂探针--prefix-契约对照实验-done-)（数据指证）
- [embedding_model.rs](../../apps/desktop/src-tauri/src/search/embedding_model.rs)（唯一改动文件）
- [EmbeddingGemma HF 卡（google）](https://huggingface.co/google/embeddinggemma-300m)
- [EmbeddingGemma GGUF（ggml-org）](https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf)
