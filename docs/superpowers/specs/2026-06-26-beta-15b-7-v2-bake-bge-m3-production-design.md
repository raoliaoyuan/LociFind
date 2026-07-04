# BETA-15B-7-v2：bake bge-m3 推到生产（桌面 wiring 切换最窄版）设计 spec

> 承接 BETA-15B-7 v4 cycle（embedding 模型跨族 + 同族最大档探针、PR #12 merged commit `094e7d0`）+ BETA-15B-8 model-runtime pooling type detection（PR #13 merged commit `5305ee1`、commit `1a86dc7`）+ BETA-15B-9 llama-cpp-4 升级 / qwen3-8b 全零 bug 排查（PR #14 merged commit `dc2a540`）三 cycle 的累积数据指证：① bge-m3 CLS pooling 真水位 OVERALL **0.869** ⭐ 双过 spec 字面 0.864 目标；② qwen3-embedding-8b 在 llama-cpp-4 0.3.2 binding 下 4-hypothesis ladder 全 FAIL、真水位仍未知、bake 决策不再等其反例。
>
> **范围哲学**：极窄、只动 desktop wiring。改 `apps/desktop/src-tauri/src/search/embedding_model.rs:11-13` 两个常量 + 同文件 doc 注释 + `apps/desktop/src-tauri/src/settings.rs` doc 注释；**不动** result-normalizer 三个 DEFAULT_* 常量 / packages/evals 整个目录 / packages/spike-retrieval / packages/model-runtime / packages/indexer。cosine 路由阈值（0.70）/ 相似度下限（0.30）/ 语义臂权重（10.0）保 qwen3-0.6b 调优值，bge-m3 上 OVERALL 仅 -0.005 vs sweep best（数据指证：v4-fixup 表 T=0.70 行 OVERALL 0.864 vs T=0.0/0.30/0.45 行 0.869）、可接受、留 follow-up cycle 视真机用户反馈再 sweep。
>
> **与 follow-up cycle 关系**：本 cycle 故意把以下三件事剥离作独立 follow-up：① cosine_threshold/floor 在 bge-m3 上重 sweep & bake；② 模型分发 UX（首启引导 / 自动下载 / Windows 真机性能验证 / 设置页 banner）；③ baseline.json rewrite + gate.rs 红线重锚到 bge-m3 数据（评测层切换、与桌面 wiring 解耦）。这三件事每件都可单独 cycle，本 cycle 不带。

## 1. 背景与动机

### 1.1 v4-fixup 数据指证 = bake 时机已到

BETA-15B-8 修复 model-runtime pooling type detection 后、bge-m3 CLS pooling 真水位 sweep（v3 数据集 78 cases / 124 docs / W=10.0、详 [baseline 报告 v4-fixup 节](../../reviews/semantic-recall-quality-baseline.md)）：

| 指标 | qwen3-0.6b T\*=0.70（生产锚）| bge-m3 best CLS（T\*=0.0/0.30/0.45）| bge-m3 @ T\*=0.70（保 qwen3 阈值）| Δ vs 0.6b（保 0.70）|
|---|---|---|---|---|
| **OVERALL** | 0.856 | **0.869** ⭐ | **0.864** | **+0.008** |
| crosslang | 0.717 | 0.685 | 0.662 | -0.055 |
| content-not-name | 0.870 | 0.871 | **0.875** | **+0.005** |
| exact-name | 1.000 | 1.000 | 1.000 | = |

**结论**：保 cosine_threshold = 0.70（qwen3 调优值）+ 切 bge-m3 = OVERALL +0.008 / content-not-name +0.005 / exact-name = / crosslang -0.055。OVERALL & content-not-name 净增、crosslang 是头号卖点反退（trade-off 须文档明示）。BETA-15B-9 排查后 qwen3-embedding-8b 在 llama-cpp-4 0.3.2 binding 下仍全零、4-hypothesis 全 FAIL、bake bge-m3 不再等其反例。

### 1.2 为什么用最窄 wiring 切换路径

- **零分发增量成本**：bge-m3 q8_0 = 605 MB ≈ qwen3-0.6b q4_k_m ≈ 610 MB（实测、同尺寸）、Beta 出场分发预算无变化
- **diff < 20 行**：改两常量 + doc 注释、回滚成本 = 改回两行
- **依赖 BETA-15B-1 已建立的失效 + reindex 机制**（`packages/indexer/src/doc_db.rs:367` `vector_is_current(model_id)`、`document_vectors.embed_model` 列）、旧用户切换零代码新增
- **评测层完全解耦**：gate 仍守 qwen3-0.6b baseline（OVERALL 0.856 等）、desktop 跑 bge-m3、两条独立路径互不阻塞；follow-up cycle 重写 baseline 时再对齐
- **YAGNI**：分发 UX / Windows 性能 / cosine_threshold 重 sweep 都是独立题、混进来=多变量耦合、本 cycle 不带

### 1.3 已知 trade-off（必须文档明示）

**crosslang 退步 -0.055**：LociFind 头号卖点是「按意思 / 跨语言模糊召回」（[PROJECT.md](../../../PROJECT.md) 一句话定位）、crosslang 桶 nDCG 反退是真实 regression。但：

- crosslang 桶 v3 评测集合成、13 例偏小、本身真水位标定就不稳（v3 cycle 主动放弃字面 0.700）
- 真机用户体验里 crosslang query 跑得动（vec_recall 仍 1.0、跨语言文档能找到、只是排序 nDCG 略降）
- OVERALL & content-not-name 双增 + exact-name 守住 = 总体正 ROI
- follow-up cycle 重 sweep cosine_threshold 可拿回 +0.023（bge-m3 T\*=0.0/0.30/0.45 vs T\*=0.70）

## 2. 目标与验收

### 2.1 目标

- 改 `apps/desktop/src-tauri/src/search/embedding_model.rs`：`DEFAULT_EMBED_MODEL_FILE` 从 `"qwen3-embedding-0.6b-q4_k_m.gguf"` 切到 `"bge-m3-q8_0.gguf"`、`EMBED_MODEL_ID` 从 `"qwen3-embedding-0.6b"` 切到 `"bge-m3"`、顶部 doc 注释更新到位（注明 BETA-15B-8 真水位 + crosslang trade-off）
- 改 `apps/desktop/src-tauri/src/search/embedding_model.rs:223` 单测 `model_id_is_stable` expected 字符串 `"qwen3-embedding-0.6b"` → `"bge-m3"`
- ~~改 `apps/desktop/src-tauri/src/settings.rs:17` doc 注释~~（**spec 修订**：plan 起手实测 line 17 doc 不含具体文件名、无需改动；详 §3.1 行 4 修订说明）
- 收工时 doc-sync：[PROJECT.md](../../../PROJECT.md)（如有 embedding 模型名引用）、[STATUS.md](../../../STATUS.md)、[ROADMAP.md](../../../ROADMAP.md)、[docs/reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)（追加 v4-fixup3 节简短记录 bake 完成 + trade-off + follow-up 候选）

### 2.2 验收红线（不可回归）

1. `cargo test --workspace`：0 failed（**预期 evals 层零变化**：vectors.json/vectors-qwen3-0.6b.json/baseline.json SHA256 不变；gate 仍验 qwen3-0.6b 数据全过；`embedding_model_path_override_from_settings` / `prewarm_feature_off_*` / `feature_off_*` 全过；`model_id_is_stable` 期望值更新到 `"bge-m3"` 后过）
2. `cargo clippy --workspace --all-targets -- -D warnings`：0 warning
3. `cargo fmt --check`：净
4. `tsc` / `vite build`：净（desktop 改动是 Rust 端、TS 不受影响）
5. evals byte-equal：`cargo run -p locifind-evals --bin evals -- ...` v0.5=473 / v0.9=877 parser-only 精确不变（纯 desktop wiring 改动、parser 无关联）
6. `vectors-bge-m3.json` / `vectors-qwen3-0.6b.json` / `vectors.json` SHA256 与 main 入栈状态完全等价（本 cycle 不重 embed 任何评测 fixtures）
7. `semantic_quality_gate` 单测 1 passed（baseline 不动 + 数据不动）
8. **Mac 真机最小化手测**（不写入正式 manual-test-scenarios.md，本 cycle 只在 cycle 末会话日志记录通过即可）：
   - 起 app → 设置页 EmbedStatus 显示 NotFound + expected_path 文本明确含 `bge-m3-q8_0.gguf`
   - 手动 `cp <downloaded>/bge-m3-q8_0.gguf` 到 app data dir 的 `models/` → 重启 app → status 变 Ready
   - 跑一次跨语言查询（如「年假和休假规定」期望召回英文 leave policy 文档）→ 命中 + 「按意思找到」徽标显示 + cosine 分数可见

### 2.3 判定矩阵

| 分支 | 触发条件 | 行动 |
|---|---|---|
| **GO**（默认）| §2.2 红线 1-7 全过 + §2.2.8 Mac 真机手测三项全过 | 落库 / doc-sync / PR / 合 main |
| **GO with documented gap** | §2.2 红线 1-7 全过 + Mac 真机手测 cycle 内来不及做（如用户当前不在 Mac 真机会话）| 落库 / doc-sync / PR 标 `[手测留 follow-up]` / 合 main；STATUS「下一步」加 Mac 真机手测 TODO；不阻塞 cycle 收口 |
| **NO GO**（异常分支）| §2.2 红线 1-7 任意一条不过 | 不发布、回滚 desktop 改动、问题录入 STATUS、cycle 标 done-with-rollback |

## 3. 改动清单

### 3.1 精确到 file:line 的改动

| 文件 | 行 | 改动 |
|---|---|---|
| [apps/desktop/src-tauri/src/search/embedding_model.rs:1](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L1) | 顶部 mod doc | 注释扩一行说明 BETA-15B-7-v2 bake bge-m3、真水位 OVERALL 0.869、crosslang -0.032 已知 trade-off、cosine_threshold 保 0.70 待 follow-up sweep |
| [apps/desktop/src-tauri/src/search/embedding_model.rs:10-11](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L10) | 10 注释 + 11 字符串 | 注释从「BETA-26 探针胜出者」改为「BETA-15B-8 v4-fixup CLS pooling 真水位 + BETA-15B-7-v2 bake」；字面值 `"qwen3-embedding-0.6b-q4_k_m.gguf"` → `"bge-m3-q8_0.gguf"` |
| [apps/desktop/src-tauri/src/search/embedding_model.rs:12-13](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L12) | 12 注释 + 13 字符串 | 字面值 `"qwen3-embedding-0.6b"` → `"bge-m3"`；注释保持「换模型→旧向量陈旧」语义 |
| [apps/desktop/src-tauri/src/search/embedding_model.rs:222-224](../../../apps/desktop/src-tauri/src/search/embedding_model.rs#L222) | 单测 expected 值 | `model_id_is_stable` 中 `EMBED_MODEL_ID` 引用不变（编译期常量、单测自动跟着）；**确认**单测断言代码无需改（用 `EMBED_MODEL_ID` 而非字面字符串） |
| ~~apps/desktop/src-tauri/src/settings.rs:17~~ | ~~doc 注释~~ | ~~默认 embedding 路径示例文件名 `qwen3-embedding-0.6b-q4_k_m.gguf` → `bge-m3-q8_0.gguf`~~（**spec 修订**：plan 起手实测发现 [settings.rs:17](../../../apps/desktop/src-tauri/src/settings.rs#L17) doc 注释只写「BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。」、**不含具体文件名**、无需改动；行 15 model_path doc 含 `qwen3-0.6b-q4_k_m.gguf` 但属 fallback chat 模型、本 cycle 不动）|

**注 1**：起草本 spec 时实测 `model_id_is_stable` 单测代码（line 222-224）`assert_eq!(h.model_id(), EMBED_MODEL_ID);` 用编译期常量、不写字面字符串、**无需手动改单测断言**。但若未来重构改字面值断言，需手动同步。

**注 2**：spec 修订（settings.rs:17 行划掉）已在 plan 起手前发现、本 spec 文件已就地更新；plan Task 1.0 commit 中包含此修订。本 cycle 唯一改 desktop 源文件 = embedding_model.rs。

### 3.2 不动清单（防止越界）

为防 cycle 内越界改动其他层、明确列出本 cycle **不改**的范围：

| 包/文件 | 不动理由 |
|---|---|
| `packages/result-normalizer/src/lib.rs` | `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.70` / `DEFAULT_SEMANTIC_WEIGHT = 10.0` / `DEFAULT_RRF_K` / `DEFAULT_SIMILARITY_FLOOR` 保 qwen3-0.6b 调优值、bge-m3 重 sweep 留 follow-up |
| `packages/evals/**` | binary 默认、baseline.json、gate.rs、vectors-*.json、cases/corpus 全部不动；gate 仍守 qwen3-0.6b、零变化 |
| `packages/spike-retrieval/**` | embed_corpus / run_retrieval 默认 `models/qwen3-embedding-0.6b-q8_0.gguf` 保留作 BETA-26 探针历史锚 |
| `packages/model-runtime/**` | pooling detection 已在 BETA-15B-8 落地；本 cycle 不动 llama-cpp-4 版本 / pooling.rs / llama.rs |
| `packages/indexer/**` | embed trait / `vector_is_current(model_id)` / doc_db schema 全不动；旧 vectors 失效与 reindex 走现有机制 |
| `apps/desktop/src-tauri/src/search/model_fallback.rs` | fallback 模型与 embedding 模型解耦、BETA-23 落地的 qwen3-0.6b chat 模型不在本 cycle 范围 |
| `apps/desktop/src/**`（TS / TSX） | UI 完全不动；状态显示 / 设置页 banner / 进度行复用 BETA-15B-2 现有实现 |
| `docs/manual-test-scenarios.md` | 不写新手测剧本；§2.2.8 真机最小化手测仅在 cycle 末会话日志记录、不入正式剧本 |

## 4. 数据流：旧用户向 bge-m3 切换

无需新代码、完全依赖 BETA-15B-1 + BETA-15B-2 已建立的失效 + reindex 机制。

```
用户从 v0.6.x 升级到 v0.7.0（含本 cycle 改动）
    │
    ▼
v0.7.0 启动
    │  EMBED_MODEL_ID = "bge-m3"
    │  DEFAULT_EMBED_MODEL_FILE = "bge-m3-q8_0.gguf"
    │
    ▼
EmbeddingModelHandle::is_active() 检查：
  cfg!(feature = "semantic-recall") = true
  resolved_model_path = <data_dir>/models/bge-m3-q8_0.gguf
  .exists() = false（旧用户没 bge-m3 文件）
    │
    ▼
返回 false → spawn_semantic_index 不启动 prewarm
EmbedStatus::NotFound{expected_path: "…/models/bge-m3-q8_0.gguf"}
    │
    ▼
设置页显示「未找到模型文件，期望放置路径：…/models/bge-m3-q8_0.gguf」
    │
    ▼  （用户手动 cp bge-m3-q8_0.gguf 到 models/）
    │
    ▼
下次 app 启动（或用户手动触发 reindex）
    │
    ▼
EmbedStatus::Ready
spawn_semantic_index 后台 worker：
  prewarm() 加载 bge-m3 模型 + 暖机
  embed_pending(roots, embedder, progress_cb)：
    indexer 扫 document_vectors 表：
      WHERE doc_id = ? AND embed_model = ? AND source_hash = ?
      旧行 embed_model = "qwen3-embedding-0.6b" ≠ "bge-m3" → vector_is_current = false
      → 需 re-embed → upsert_vector with embed_model = "bge-m3"
    │
    ▼
设置页「🧠 语义索引中 X/Y」进度行
    │
    ▼  （N 分钟 ~ N 小时，取决于语料大小）
    │
    ▼
全部 re-embed done、查询走 bge-m3 向量
旧 qwen3 q4_k_m 文件 `models/qwen3-embedding-0.6b-q4_k_m.gguf` 留在磁盘
（下游无消费者、占用 ~610MB、用户自行清理）
```

**关键不变量**：
- 切换全程不阻塞查询（FTS-only 降级仍工作、`EmbedStatus::NotFound` 时 `ready()` 返回 None、`semantic-index` arm 自然空、`fuse_rrf_with_fts_routing` empty-arm early-return guard 兜底）
- 切换不擦数据（document_vectors 旧行保留、`upsert_vector` UPSERT 语义就地替换、不丢文档元数据）
- 切换无误删（旧 qwen3 模型文件用户自管、本 cycle 不动磁盘）

## 5. 异常分支 / 边界

| 场景 | 当前 v0.6 行为 | 切换后 v0.7 行为 | 是否需改 |
|---|---|---|---|
| 模型文件不存在 | NotFound{expected_path = …/qwen3-…} | NotFound{expected_path = …/bge-m3-…} | 否（行为天然正确、文本自动更新） |
| feature `semantic-recall` 关 | Unavailable | 同上、无变化 | 否 |
| 模型加载失败（文件损坏等） | Failed{reason} | 同上、reason 由 llama.cpp 返回 | 否 |
| 旧 vectors blob dim 不匹配 | embed_model 列先过滤、不会读到旧 blob | 同上、`vector_is_current` 守住 | 否 |
| `settings.embedding_model_path` 用户已设自定义路径 | 优先用 custom 路径 | 不变、尊重用户显式覆盖 | 否（即使用户指定的还是 qwen3 文件、行为定义良好：用户拿到的是 qwen3 模型 + EMBED_MODEL_ID="bge-m3" 元数据 → 嵌入向量被标 "bge-m3" 但其实是 qwen3 模型生成、**这是用户责任**、本 cycle 不防御）|
| 用户磁盘里同时有 qwen3 + bge-m3 两个模型 | 只用默认路径模型 | 同上、bge-m3 默认；qwen3 文件冗余、用户自清 | 否 |

**特殊边界：settings.embedding_model_path 覆盖**：上表第 5 行的「用户自定义路径仍指向 qwen3 文件」场景属技术上可能但产品上罕见——设置页这个字段几乎只用于「想把模型放到非默认目录」、用户大概率不会显式指向特定模型文件名。本 cycle 不加防御逻辑（如「比对文件名与 EMBED_MODEL_ID 不一致时拒绝」）、视用户为知情用户、出现问题用户能从「按意思找到」徽标缺失 + 设置页 status 观察到。

## 6. 非范围（YAGNI）

明确不在本 cycle 范围、留给独立 follow-up cycle 的题：

1. **cosine_threshold 在 bge-m3 上重 sweep & bake**——v4-fixup 表数据齐全（T\*=0.0/0.30/0.45 OVERALL 0.869、可再拿回 +0.005）；独立 follow-up cycle = 1d 工作量
2. **模型分发 UX 增强**——首次启动引导对话框 / 自动下载 / 内置打包；与 Beta 出场 BETA-09(a) 同步规划、独立 cycle
3. **Windows 真机性能验证**——v0.6 Windows 实测 qwen3-0.6b 温查询 2.8s、bge-m3 BERT 架构推理路径不同、Windows CPU 性能未知；独立 cycle 含 model-runtime embed context 复用 + Vulkan/CUDA GPU 加速
4. **evals 层 baseline rewrite**——把 baseline.json / gate.rs 红线从 qwen3-0.6b 数据切到 bge-m3 数据；与本 cycle 解耦、可视真机用户反馈再做、独立 cycle
5. **跨厂替代候选探针**（EmbeddingGemma / jina-v3 / bge-multilingual-gemma2 9B）——bge-m3 已部分破局、若想冲 crosslang ≥0.700 spec 字面目标再考虑、低优
6. **评测集扩量**（v3 78 cases → v4 100+ cases）——bge-m3 真水位在 v3 上已稳、扩量验鲁棒留低优
7. **manual-test-scenarios.md 加 bge-m3 切换章节**——本 cycle 真机手测只在会话日志记录、独立 doc cycle 做（与 BETA-09(a) doc work 一起）

## 7. 测试 / 验证门

按 §2.2 红线 1-8 执行、cycle 末 PR 前必须全过。

### 7.1 自动化（CI / 本地）

```bash
# §2.2 红线 1
cargo test --workspace --all-features

# §2.2 红线 2
cargo clippy --workspace --all-targets --all-features -- -D warnings

# §2.2 红线 3
cargo fmt --check

# §2.2 红线 4（desktop UI）
cd apps/desktop && pnpm tsc --noEmit && pnpm vite build

# §2.2 红线 5 / 6 / 7
cargo run -p locifind-evals --bin evals -- --version v0.5 --output /tmp/evals-v05.json
cargo run -p locifind-evals --bin evals -- --version v0.9 --output /tmp/evals-v09.json
shasum /tmp/evals-v05.json /tmp/evals-v09.json  # 与 main 入栈完全一致
shasum packages/evals/fixtures/semantic-recall/vectors*.json packages/evals/fixtures/semantic-recall/baseline.json  # 与 main 完全一致
cargo test -p locifind-evals --test semantic_quality_gate -- --include-ignored  # 1 passed
```

### 7.2 真机手测（§2.2.8 Mac 最小化）

| # | 步骤 | 预期 |
|---|---|---|
| 1 | 起 app（feature semantic-recall 开、bge-m3 文件不在 models/）| 设置页 EmbedStatus 显示 `NotFound`、expected_path 含 `bge-m3-q8_0.gguf` |
| 2 | `cp ~/Downloads/bge-m3-q8_0.gguf <app-data-dir>/models/` + 重启 app | EmbedStatus 变 `Ready` |
| 3 | 跑跨语言查询「年假和休假规定」（语料含英文 leave policy）| 命中、第 1-3 名有英文文档、「按意思找到」徽标、cosine 分数显示 0.6+ |

未在 Mac 真机会话时按 §2.3 GO with documented gap 路径处理、不阻塞 cycle 收口。

## 8. 出场标准

cycle 标 done 进 ROADMAP BETA-15B-7 task 卡片的判据：

- §2.2 红线 1-7 全过（自动化必要、本机可跑）
- §2.2.8 Mac 真机手测三步全过（或按 §2.3 GO with documented gap 路径标 TODO）
- spec / plan 落库 + PR 合 main
- STATUS / ROADMAP doc-sync：STATUS「当前 task」+ 会话日志 + 本机可立即上手列表（移除 BETA-15B-7-v2、加入 follow-up 候选）；ROADMAP BETA-15B-7 卡片追加 v2 已合 + follow-up cycle 候选
- baseline 报告追加 **v4-fixup3 节**（精简、3-5 行）：bake bge-m3 完成 commit hash / cosine_threshold 保 0.70 trade-off 记录 / follow-up 候选清单（cosine sweep + 分发 UX + Windows 性能 + baseline rewrite）

## 9. 风险与缓解

| 风险 | 概率 | 缓解 |
|---|---|---|
| 用户升级后 reindex 卡很久（v3 数据集 ~124 docs 在 Mac Metal ≈ 10s、真机大语料未知）| 中 | BETA-15B-2 进度行已就绪、`embed_pending` 是 streaming + progress_cb 透传、用户可见进度；本 cycle 不动调度逻辑 |
| Windows 用户切 bge-m3 后 BERT 架构推理性能差于 qwen3 | 高（未验）| 独立 follow-up cycle、本 cycle 不阻塞 Mac 发布；release notes 标注「Windows 用户性能基准 follow-up」|
| crosslang 真机用户体验退步可感知 | 低-中 | nDCG -0.055 是排序略降、不是 recall 失败；vec_recall 仍 1.0；真机查询「中文 query → 英文文档」仍能找到；release notes / 设置页可见 cosine 分数让用户能感知到差异 |
| 模型文件分发依赖用户手动放（与 v0.5 qwen3 流程一致）| 接受 | 这是本 cycle 故意承接的现状、follow-up cycle 做分发 UX |
| `model_id_is_stable` 单测期望值需手改 | 低 | spec §3.1 注：单测用 `EMBED_MODEL_ID` 编译期常量、不写字面字符串、**无需手改**；起 cycle 时 implementer 实测确认即可 |
| spec 起草期错估 cosine_threshold 不动的 ROI 损失 | 低 | v4-fixup 表 T=0.70 OVERALL = 0.864 vs sweep best 0.869 = -0.005、量化已固化、无 surprise |

---

> **文档归档约定**：本 spec 起草于 2026-06-26、对应 cycle ID = BETA-15B-7-v2、ROADMAP 中归属 BETA-15B-7 task 卡片（v4 cycle 的 v2 follow-up）。完整 superpowers 流程：brainstorming（Q1 范围 = 仅桌面 wiring 切换 / Q2 baseline 不动 + gate 仍守 qwen3 / Q3 旧用户依赖 model_id 自动失效 + 后台 reindex） → 本 spec → writing-plans skill → subagent-driven 执行 → integration review → PR → 合 main → doc-sync。
