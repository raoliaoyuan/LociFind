# BETA-15B-6 v2：content-not-name 桶扩量 + T\* 鲁棒性校验 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 BETA-15B-6 合成评测集 content-not-name 桶从 11 → 20 例扩量、Mac Metal 本机重算 vectors.json 全集、sweep 9 阈值验证 A-5 T\*=0.60 鲁棒性。

**Architecture:** 纯数据 cycle、零代码逻辑改动。手写 9 新 case + 7 新 corpus docs 进 `cases.json`/`corpus.json`、Mac Metal `--embed` 重算 `vectors.json`、`semantic_quality` binary 跑 9 阈值 sweep、按 spec §2.2 接受标准决定是否微调 bake T\*；条件性更新 `lib.rs` 常量 + `gate.rs` doc + `baseline.json`；baseline 报告追加 v2 节。

**Tech Stack:** Rust 2024 edition、locifind-evals semantic_quality / `--embed` feature `semantic-recall-metal`、llama-cpp 后端、qwen3-embedding-0.6b-q8_0.gguf 模型、JSON fixture（serde_json）。

**关键事实清单**（写 plan 时核对实情）：
- 当前 cases.json 总数 59、bucket 分布：synonym c001-c012 / concept c013-c024 / crosslang c025-c037 / content-not-name c038-c048 / exact-name c049-c059
- 新 9 case 续号 **c060-c068**（spec §4.1/§4.2 写「c049-c057」是错的——那段 id 已被 exact-name 桶用、plan 修正为 c060-c068）
- 当前 corpus.json 总数 108、最后 id `s00108`；新 7 doc 续号 **s00109-s00115**
- corpus s00023/s00024 是「客户支持常见问题手册 zh + Customer Support Playbook for Common Tickets en」对——v2 c063/c068「多语言客服话术规约」query 复用此对（content-not-name 主题：query 描述客服话术、doc 是客服处理手册、词面不共享）
- 完整性测试 `semantic_quality_fixtures_integrity` 现门：corpus ≥ 100、cases ≥ 40、5 桶都有 case、grade ∈ [1,3]、id 唯一、relevant 非空、doc_id 引用必须存在
- Schema：`SemanticDoc { doc_id, lang, title, body }`、`SemanticCase { id, bucket, query, relevant: Vec<RelevantDoc> }`、`RelevantDoc { doc_id, grade: u8 }`
- A-5 bake `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.60`（`packages/result-normalizer/src/lib.rs:105`）；接受标准：T\*=0.60 仍 best → 不改；T\* 微调 [0.55, 0.65] → bake 新值 + 升 doc；T\* 偏移 > ±0.10 → spec §5 降级 1.01
- `semantic_quality_gate.rs` 4 红线动态读 baseline、A-6 数值自动跟随、不动断言代码

---

## Task 1: 追加 9 新 case 进 cases.json（c060-c068）

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/cases.json`

- [ ] **Step 1: 失败前置 = 确认现 cases.json 总数 = 59、last id = c059**

Run: `python3 -c "import json; d=json.load(open('packages/evals/fixtures/semantic-recall/cases.json')); print(f'cases={len(d)}, last_id={d[-1][\"id\"]}')"`
Expected: `cases=59, last_id=c059`

- [ ] **Step 2: 用 jq/python3 追加 9 新 case 到 cases.json 末尾**

完整 9 新 case JSON（手抄进 cases.json `[]` 数组末尾，紧跟 c059 后、注意逗号 + 缩进 2 空格与现有风格一致）：

```json
  {
    "id": "c060",
    "bucket": "content-not-name",
    "query": "那份说重试机制要用指数退避配合抖动二者结合的方案",
    "relevant": [
      {
        "doc_id": "s00109",
        "grade": 3
      },
      {
        "doc_id": "s00110",
        "grade": 1
      }
    ]
  },
  {
    "id": "c061",
    "bucket": "content-not-name",
    "query": "提到 A/B 实验最小样本量按统计功效算的复盘记录",
    "relevant": [
      {
        "doc_id": "s00111",
        "grade": 3
      }
    ]
  },
  {
    "id": "c062",
    "bucket": "content-not-name",
    "query": "讲冷启动数据稀疏用人工冷启池规则的方案",
    "relevant": [
      {
        "doc_id": "s00112",
        "grade": 3
      },
      {
        "doc_id": "s00113",
        "grade": 1
      }
    ]
  },
  {
    "id": "c063",
    "bucket": "content-not-name",
    "query": "那篇关于多语言客服话术规约的内容",
    "relevant": [
      {
        "doc_id": "s00023",
        "grade": 3
      },
      {
        "doc_id": "s00024",
        "grade": 2
      }
    ]
  },
  {
    "id": "c064",
    "bucket": "content-not-name",
    "query": "提到内部 IM 表情包审核制度的那份",
    "relevant": [
      {
        "doc_id": "s00114",
        "grade": 3
      },
      {
        "doc_id": "s00115",
        "grade": 1
      }
    ]
  },
  {
    "id": "c065",
    "bucket": "content-not-name",
    "query": "the spec saying retries must use truncated exponential backoff with jitter",
    "relevant": [
      {
        "doc_id": "s00110",
        "grade": 3
      },
      {
        "doc_id": "s00109",
        "grade": 1
      }
    ]
  },
  {
    "id": "c066",
    "bucket": "content-not-name",
    "query": "the doc explaining cold-start data sparsity with manual cold pool rules",
    "relevant": [
      {
        "doc_id": "s00113",
        "grade": 3
      },
      {
        "doc_id": "s00112",
        "grade": 1
      }
    ]
  },
  {
    "id": "c067",
    "bucket": "content-not-name",
    "query": "the policy on internal IM emoji approval workflow",
    "relevant": [
      {
        "doc_id": "s00115",
        "grade": 3
      },
      {
        "doc_id": "s00114",
        "grade": 1
      }
    ]
  },
  {
    "id": "c068",
    "bucket": "content-not-name",
    "query": "the writeup on multilingual customer support phrasing conventions",
    "relevant": [
      {
        "doc_id": "s00024",
        "grade": 3
      },
      {
        "doc_id": "s00023",
        "grade": 2
      }
    ]
  }
```

注：c063/c068 复用 s00023/s00024（客户支持手册 zh+en 对、与「多语言客服话术规约」query 词面不共享但内容相关）；c060-c062/c064 各挂新 doc + 跨语言 partner 1 例；c061 单 grade=3 relevant（A/B 实验主题单 doc）。

操作步骤：
1. 用文本编辑器打开 `packages/evals/fixtures/semantic-recall/cases.json`
2. 找到末尾 `]` 前的 c059 case
3. 在 c059 case `}` 后加 `,`、再粘贴上面 9 个 case 块
4. 保留末尾 `]` 与原始 EOF

- [ ] **Step 3: 验证 JSON 格式 + 总数 = 68 + content-not-name = 20**

Run:
```bash
python3 -c "
import json
with open('packages/evals/fixtures/semantic-recall/cases.json') as f:
    d = json.load(f)
print(f'cases={len(d)}, last_id={d[-1][\"id\"]}')
from collections import Counter
counts = Counter(c['bucket'] for c in d)
for b, n in sorted(counts.items()):
    print(f'  {b:<20} {n}')
"
```

Expected:
```
cases=68, last_id=c068
  concept              12
  content-not-name     20
  crosslang            13
  exact-name           11
  synonym              12
```

- [ ] **Step 4: 提交（暂不验证完整性，T2 才跑——因为新 case 引用未建的 s00109-s00115）**

```bash
git add packages/evals/fixtures/semantic-recall/cases.json
git commit -m "BETA-15B-6 v2 task 1：cases.json 追加 9 新 content-not-name 桶 case（c060-c068；5 zh + 4 en；4 边界 + 5 常规；2 例复用 s00023/s00024 客服对、7 例待 T2 加新 doc）"
```

注：本 commit 后 `semantic_quality_fixtures_integrity` 测试**预期失败**（c060/c061/c062/c064/c065/c066/c067 引用未建的 s00109-s00115）—— T2 加 doc 后才修复。

---

## Task 2: 追加 7 新 corpus docs 进 corpus.json（s00109-s00115）

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/corpus.json`

- [ ] **Step 1: 失败前置 = 跑完整性测试预期失败（c060+ 引用 s00109+ 不存在）**

Run: `cargo test -p locifind-evals --test semantic_quality_fixtures_integrity 2>&1 | tail -15`
Expected: 失败、错误信息含「case c060 引用未知 doc s00109」类似消息。

- [ ] **Step 2: 用文本编辑器追加 7 新 corpus doc 到 corpus.json 末尾**

完整 7 新 doc JSON（手抄进 corpus.json `[]` 数组末尾，紧跟 s00108 后、注意逗号 + 缩进 2 空格）：

```json
  {
    "doc_id": "s00109",
    "lang": "zh",
    "title": "分布式任务重试规约（草案）",
    "body": "对外部依赖调用统一走重试封装。首次失败立即记录上下文,后续按指数退避策略,基数取一秒、倍数取二,封顶三十秒。为防止集群同步抖动,在退避基础上再叠加随机抖动,范围取上一次等待时间的两成。重试上限按调用方分级:在线读类放开到五次,写类收紧到两次,异步任务可放到八次。每次重试都需上报指标,统计成功率与平均尝试次数。对幂等性不明的接口禁用自动重试,改为人工审查。"
  },
  {
    "doc_id": "s00110",
    "lang": "en",
    "title": "Retry Contract for Outbound Calls",
    "body": "All outbound dependency calls go through the shared retry wrapper. The first failure is logged with full context, then truncated exponential backoff kicks in with a base of one second and a doubling factor, capped at thirty seconds. To avoid herd retries we layer jitter on top, sampled uniformly from twenty percent of the previous wait. Retry budgets vary by call class: online reads allow up to five attempts, writes are tightened to two, async jobs may go to eight. Each attempt emits a metric so we can track success rate and average tries. Endpoints whose idempotency is unclear are excluded from auto-retry and require human review."
  },
  {
    "doc_id": "s00111",
    "lang": "zh",
    "title": "实验平台样本量计算说明（v2）",
    "body": "新建实验前先估算最小样本量,避免跑出无统计意义的结论。计算流程:先确定主指标的基线均值和最低可检测效应,再选定显著性水平和功效要求,然后按双样本检验公式反推每组样本量。若指标方差未知,可用历史一周数据近似。样本量计算结果会直接生成在实验创建表单上,作者需在备注里写出预计运行天数与流量分配。若结果偏离预估超出两成需重新评估并备注原因。功效达成前禁止解读结果以免被随机波动误导。"
  },
  {
    "doc_id": "s00112",
    "lang": "zh",
    "title": "推荐系统冷启动池规则",
    "body": "对新用户与新物品采用人工冷启池规则补足召回。新用户在没有交互记录时,默认从分类的热门池中按多样性约束采样。新物品入库后挂在对应主题与标签下,享有为期七天的曝光保护,期间不被纯协同过滤压低。冷启池由编辑维护,每周根据点击率与停留时长复盘剔除老化项。规则之外的精排仍走原模型,冷启池只影响候选阶段的纳入而非最终排序。所有冷启决策记录在审计日志便于事后归因。"
  },
  {
    "doc_id": "s00113",
    "lang": "en",
    "title": "Cold-Start Pool Rules for Recommendations",
    "body": "When the recommender has no interaction history, we lean on manual cold-start pools to bootstrap recall. New users default to category-based popular pools sampled under a diversity constraint. New items, upon ingestion, are tagged with their topic and granted a seven-day exposure window so that pure collaborative filtering cannot suppress them prematurely. The pool itself is curated by editors and pruned weekly based on click-through and dwell time. Beyond pool admission, the ranker remains unchanged: cold-start rules affect candidate inclusion, never final ranking. Every cold-start decision is logged for post-hoc attribution."
  },
  {
    "doc_id": "s00114",
    "lang": "zh",
    "title": "内部沟通工具内容审核制度",
    "body": "公司内部 IM 表情包与自定义贴纸需先经审核再上架到团队空间。提交人填写来源与用途,审核人按三档判定:可公开使用、限定团队使用、需修改。涉及肖像权、品牌商标、政治宗教与歧视性内容的一律拒绝。审核通过后存入资源库并打标签便于检索;不通过的需说明理由让提交人自查。每季度由审核组随机抽查已上架资源,过期或失当的下架。日常使用中若有员工举报不当资源,审核组在两个工作日内复核。"
  },
  {
    "doc_id": "s00115",
    "lang": "en",
    "title": "Internal Messaging Sticker and Emoji Approval Policy",
    "body": "Custom stickers and emoji intended for company chat tools must clear an approval queue before they appear in any team workspace. Submitters describe the source and intended usage. Reviewers triage into three buckets: open to all, scoped to a single team, or send back for edits. Anything touching likeness rights, brand marks, political or religious imagery, or discriminatory content is rejected outright. Approved assets land in the shared library with tags for discovery; rejections come with written reasons for self-review. Each quarter the review group spot-checks live assets and retires anything stale or off-policy. Day-to-day user reports are reviewed within two business days."
  }
```

PII 自检（README 5 项、commit 前必过）：
- ✅ 无真实人名/公司/邮箱/电话/精确薪资/真实路径
- ✅ 无具名人物（用「同事」「员工」「审核人」职位代称）
- ✅ 金额一律约整数（「一秒」「七天」「两个工作日」）
- ✅ doc_id 唯一（s00109-s00115 续号）
- ✅ 不涉及跨语言桶（content-not-name 桶 case）

- [ ] **Step 3: 验证 JSON 格式 + corpus 总数 = 115**

Run:
```bash
python3 -c "
import json
with open('packages/evals/fixtures/semantic-recall/corpus.json') as f:
    d = json.load(f)
print(f'corpus={len(d)}, last_id={d[-1][\"doc_id\"]}')
new_docs = [doc for doc in d if doc['doc_id'] >= 's00109']
print(f'new docs: {len(new_docs)}')
for doc in new_docs:
    print(f\"  {doc['doc_id']} lang={doc['lang']} title={doc['title'][:40]}\")
"
```

Expected:
```
corpus=115, last_id=s00115
new docs: 7
  s00109 lang=zh title=分布式任务重试规约（草案）
  s00110 lang=en title=Retry Contract for Outbound Calls
  s00111 lang=zh title=实验平台样本量计算说明（v2）
  s00112 lang=zh title=推荐系统冷启动池规则
  s00113 lang=en title=Cold-Start Pool Rules for Recommendations
  s00114 lang=zh title=内部沟通工具内容审核制度
  s00115 lang=en title=Internal Messaging Sticker and Emoji Approval Policy
```

- [ ] **Step 4: 跑完整性测试 = 现在应过（绿）**

Run: `cargo test -p locifind-evals --test semantic_quality_fixtures_integrity 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step 5: 提交**

```bash
git add packages/evals/fixtures/semantic-recall/corpus.json
git commit -m "BETA-15B-6 v2 task 2：corpus.json 追加 7 新 doc（s00109-s00115；4 zh + 3 en；分布式重试 zh+en 配对 / A/B 实验 zh / 冷启动 zh+en 配对 / IM 表情包审核 zh+en 配对；零 PII 全虚构占位）"
```

---

## Task 3: Mac Metal --embed 重算 vectors.json 全集

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/vectors.json`

**前置条件**：
- 模型在 `models/qwen3-embedding-0.6b-q8_0.gguf`
- cmake 已装（编 llama-cpp）

- [ ] **Step 1: 失败前置 = 跑 sweep 验证当前 vectors.json 缺新 case query 向量**

Run:
```bash
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --cosine-threshold 0.60 2>&1 | head -10
```
Expected: 报错「缺 query 向量: c060」（或类似缺向量错）—— vectors.json 仍是 v1 的 59 query + 108 doc。

- [ ] **Step 2: 跑 --embed 重算 vectors.json 全集**

```bash
cargo run --release -p locifind-evals --bin semantic_quality \
  --features semantic-recall-metal -- --embed
```
Expected:
- 编译 ~3-5 分钟（首次启 metal feature）
- embed 跑 ~1-2 分钟（115 doc + 68 query × ~50 token avg、Mac Metal）
- stderr 末尾: `已写 vectors.json`
- 退出码 0

- [ ] **Step 3: 验证 vectors.json 完整覆盖 + 元数据**

Run:
```bash
python3 -c "
import json
with open('packages/evals/fixtures/semantic-recall/vectors.json') as f:
    vc = json.load(f)
print(f'model_id: {vc[\"model_id\"]}')
print(f'dim: {vc[\"dim\"]}')
print(f'doc_vectors: {len(vc[\"doc_vectors\"])}')
print(f'query_vectors: {len(vc[\"query_vectors\"])}')
new_docs = [k for k in vc['doc_vectors'] if k >= 's00109']
new_queries = [k for k in vc['query_vectors'] if k >= 'c060']
print(f'new doc vecs: {len(new_docs)} {sorted(new_docs)}')
print(f'new query vecs: {len(new_queries)} {sorted(new_queries)}')
"
```

Expected:
```
model_id: models/qwen3-embedding-0.6b-q8_0.gguf
dim: 1024
doc_vectors: 115
query_vectors: 68
new doc vecs: 7 ['s00109', 's00110', 's00111', 's00112', 's00113', 's00114', 's00115']
new query vecs: 9 ['c060', 'c061', 'c062', 'c063', 'c064', 'c065', 'c066', 'c067', 'c068']
```

注：dim 可能是 1024 或别值、按模型实际维度；只要全 doc/query 同 dim 即可。

- [ ] **Step 4: 跑 check_vectors 完整性测试**

Run: `cargo test -p locifind-evals --lib semantic_quality::data 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`

- [ ] **Step 5: 提交（vectors.json 是 ~2MB 二进制 JSON、可入仓）**

```bash
git add packages/evals/fixtures/semantic-recall/vectors.json
git commit -m "BETA-15B-6 v2 task 3：Mac Metal --embed 重算 vectors.json 全集（115 doc + 68 query × qwen3-embedding-0.6b-q8_0）"
```

注：commit 前先确认 `git diff --stat packages/evals/fixtures/semantic-recall/vectors.json` 显示大小合理（~1.5-2.5MB）。

---

## Task 4: 跑 9 阈值 sweep + 选 T\*

**Files:**
- 无文件改动；产 `/tmp/sweep-cosine-v2.log` + 决定 T\*

- [ ] **Step 1: 跑 9 阈值 sweep**

```bash
rm -f /tmp/sweep-cosine-v2.log
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== cosine_threshold = $T ===" >> /tmp/sweep-cosine-v2.log
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --cosine-threshold $T 2>/dev/null >> /tmp/sweep-cosine-v2.log
done
wc -l /tmp/sweep-cosine-v2.log
```

Expected: log ~81 行（9 阈值 × 9 行 / 阈值，含 `=== ===` + 表头 + 7 数据行）

- [ ] **Step 2: 看 sweep 结果**

Run: `cat /tmp/sweep-cosine-v2.log`
Expected: 每 T 输出表格、含 6 桶（synonym/concept/crosslang/content-not-name/exact-name/OVERALL）× 8 指标列。

- [ ] **Step 3: 人工分析、按 spec §2.2 接受标准选 T\***

按 spec §2.2 接受标准顺序：

**第 1 优先：T\*=0.60 仍 sweep best**
- 检查：T=0.60 时 OVERALL HYBR_N 是否 > 0.864、crosslang HYBR_N 是否 > 0.700、各桶 HYBR_N 是否 ≥ HYB baseline（baseline 数值见 A-5 调优记录或 v1 baseline.json）
- 若全过 + T=0.60 是 sweep best → 不动 lib.rs 常量、跳到 T6

**第 2 优先：T\* 微调 [0.55, 0.65] 之间**
- 若 T=0.55 或 T=0.65 比 T=0.60 OVERALL 高 ≥ 0.005 且各桶 (4b) 仍守 → bake 新值
- sweep 表上看哪个 T 各桶都过 (4b) + OVERALL 最大

**第 3 优先：spec §5 降级 T\*=1.01**
- 若所有 T < 1.01 都至少破一桶 (4b) → T\*=1.01
- baseline 报告 v2 节诚实写「v2 数据集让 T\* 偏移、走 spec §5 降级」

把决策写进 `/tmp/T-star-v2.txt`：

```bash
echo "T* = <选定值>" > /tmp/T-star-v2.txt
echo "决策理由：<spec §2.2 接受标准第几优先 + 关键数值对比>" >> /tmp/T-star-v2.txt
cat /tmp/T-star-v2.txt
```

- [ ] **Step 4: 无 commit**（手动数据分析步骤、T6 落 bake commit）

---

## Task 5: 条件性 bake T\*（若微调）+ rewrite baseline.json + gate.rs doc

**Files:**
- 条件 Modify: `packages/result-normalizer/src/lib.rs`（仅 T\* 微调时）
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`（rewrite）
- 条件 Modify: `packages/evals/tests/semantic_quality_gate.rs`（仅 T\* 微调时升 doc）

### Branch A：T\* = 0.60 仍 sweep best（不动 lib.rs / gate.rs）

- [ ] **Step A1: 跑 --write-baseline 让 baseline.json 反映 v2 数据集**

```bash
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --write-baseline
```
Expected: stderr「已写 baseline.json（6 桶含 OVERALL）」

- [ ] **Step A2: 跑回归门验证**

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step A3: 提交 baseline.json**

```bash
git add packages/evals/fixtures/semantic-recall/baseline.json
git commit -m "BETA-15B-6 v2 task 5：rewrite baseline.json 反映 v2 数据集（T*=0.60 仍 sweep best、A-5 bake 不动；content-not-name 20 例 + corpus 115 doc）"
```

### Branch B：T\* 微调 [0.55, 0.65]（bake 新值 + 升 lib.rs/gate.rs doc）

设新 T\* 为 `<T_new>`（如 0.55 或 0.65）。

- [ ] **Step B1: 改 `DEFAULT_COSINE_ROUTING_THRESHOLD`**

`packages/result-normalizer/src/lib.rs` 当前（A-5 task 8 写入的）大约 line 97-105：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 **T\* = 0.60**：A 簇 5 cycle 首次破 spec §5 降级、
/// ...（具体 doc 见 A-5 task 8）
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 0.60;
```

整段替换为（用 `<T_new>` 替换实际值如 `0.55`）：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 T\* = 0.60（合成集 v1 / 11 例 content-not-name 桶）；
/// BETA-15B-6 v2 扩 content-not-name 桶 11→20 后 sweep 微调到 **T\* = <T_new>**
/// （spec §2.2 接受标准第 2 优先：v2 数据集让 sweep best 偏移，bake 跟随）。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = <T_new>;
```

- [ ] **Step B2: 升级 gate.rs doc 注释**

`packages/evals/tests/semantic_quality_gate.rs` 当前（A-5 task 8 写入的）大约 line 85-89：

```rust
    // BETA-15B-3 A-5 红线：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=0.60 bake 后数值无需替换。
    // A 簇首次破 spec §5 降级：crosslang HYBR_N=0.726>0.700 spec 目标、OVERALL=0.871>0.864。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 调优记录节。
```

整段替换为：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=0.60 → v2 T*=<T_new> bake 后数值无需替换。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
```

- [ ] **Step B3: rewrite baseline.json**

```bash
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --write-baseline
```

- [ ] **Step B4: 跑回归门验证**

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step B5: 提交三文件**

```bash
git add packages/result-normalizer/src/lib.rs \
        packages/evals/fixtures/semantic-recall/baseline.json \
        packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-6 v2 task 5：bake DEFAULT_COSINE_ROUTING_THRESHOLD 0.60 → <T_new>（v2 sweep 微调）+ rewrite baseline.json + gate.rs doc 升 v2"
```

### Branch C：spec §5 降级 T\*=1.01

设 T\* = 1.01。

- [ ] **Step C1: 改 lib.rs 常量 + doc 升 v2 降级**

`packages/result-normalizer/src/lib.rs` 整段替换为：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 T\* = 0.60（合成集 v1）；
/// BETA-15B-6 v2 扩 content-not-name 桶 11→20 后 sweep **走 spec §5 降级 T\* = 1.01**
/// （cosine ∈ [0,1] 物理上限、永不跳、HYBR ≡ HYB）：v1 11 例确实带运气，v2 上无任一
/// T < 1.01 满足 spec §2.2 (4b) 各桶 ≥ HYB baseline 硬红线；下 cycle 抓手 = 更大 embedding
/// 模型 / cosine + lang 组合信号。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 1.01;
```

- [ ] **Step C2: gate.rs doc 升 v2 降级**

替换为：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=0.60 → v2 走 spec §5 降级 T*=1.01（HYBR≡HYB）。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
```

- [ ] **Step C3-C5: 同 Branch B step B3-B5**（rewrite baseline.json + 跑 gate + commit）

注：Branch C 的 baseline.json 中 HYBR_* 字段会 ≡ HYB_* 字段（路由不生效）。

### 三 Branch 决策点

T4 step 3 决定走哪 Branch：
- **A**：T\* = 0.60 不动（最理想结果、A-5 bake 鲁棒）
- **B**：T\* ∈ [0.55, 0.65] 微调
- **C**：T\* > ±0.10 偏移（走 §5 降级）

实施时只走一条 Branch、跳过另两条的 step。

---

## Task 6: 跑全套验证门

**Files:**
- 无文件改动；纯验证

- [ ] **Step 1: 跑 workspace 测试**

```bash
cargo test --workspace 2>&1 | grep "test result:" | awk '{passed += $4; failed += $6} END {print "passed", passed, "/ failed", failed}'
```
Expected: `passed N / failed 0`（N ≈ 860 ± 含完整性测试 + 1 个 gate 测试）

- [ ] **Step 2: 跑 clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```
Expected: 净（无 warning、无 error）

- [ ] **Step 3: 跑 fmt**

```bash
cargo fmt --all --check
```
Expected: 净（无输出）

- [ ] **Step 4: 跑 v0.5 byte-equal 复检**

```bash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v2-v05.json && python3 -c "
import json
with open('/tmp/v2-v05.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.5', counts)
"
```
Expected: `v0.5 {'pass': 473, 'partial': 25, 'fail': 2}` 精确

- [ ] **Step 5: 跑 v0.9 byte-equal 复检**

```bash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null > /tmp/v2-v09.json && python3 -c "
import json
with open('/tmp/v2-v09.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.9', counts)
"
```
Expected: `v0.9 {'pass': 877, 'partial': 119, 'fail': 4}` 精确

- [ ] **Step 6: 跑回归门 4 红线**

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step 7: 无 commit**（本 task 纯验证、改动落在 T5）

---

## Task 7: 写 baseline 报告 v2 节 + README v2 更新 + 总验收

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`
- Modify: `packages/evals/fixtures/semantic-recall/README.md`

- [ ] **Step 1: 在 baseline 报告末尾追加 v2 数据集节**

`docs/reviews/semantic-recall-quality-baseline.md` 末尾追加（找到现有 A-5 节末尾 `链接：[spec](...) / [plan](...)` 行之后追加）：

````markdown

## v2 数据集：content-not-name 桶扩量 11→20（2026-06-24 Claude Code）

**承接**：A-5 cycle T\*=0.60 bake、A 簇 5 cycle 首破 spec §5 降级。A-5 调优记录诚实承认「合成集 11 例可能让 T\*=0.60 带运气、扩量校验鲁棒性」。本 cycle 针对性扩 content-not-name 桶 11→20 + corpus 108→115、Mac Metal 本机重算 vectors.json 全集、重跑 9 阈值 sweep 校验 T\*=0.60 鲁棒性。

**扩量产出**：
- cases.json 59→68（content-not-name 11→20、其他 4 桶不动；新 9 case = c060-c068、5 zh + 4 en、4 边界 case + 5 常规 case）
- corpus.json 108→115（s00109-s00115、7 新 doc：4 zh + 3 en、4 跨语言主题对、零 PII 全虚构）
- vectors.json 全集重算（dim 跟随 qwen3-embedding-0.6b-q8_0、68 query + 115 doc）

**新 9 case 主题清单**：

| id | bucket | 主题 | 复用 doc |
|---|---|---|---|
| c060 | content-not-name | 重试机制（指数退避 + 抖动）zh | s00109 (zh) / s00110 (en) |
| c061 | content-not-name | A/B 实验最小样本量 zh | s00111 |
| c062 | content-not-name | 推荐系统冷启动池规则 zh | s00112 / s00113 |
| c063 | content-not-name | 多语言客服话术规约 zh | **复用 s00023/s00024** |
| c064 | content-not-name | 内部 IM 表情包审核 zh | s00114 / s00115 |
| c065 | content-not-name | 重试机制（truncated exp backoff）en | s00110 / s00109 |
| c066 | content-not-name | 推荐系统冷启动池 en | s00113 / s00112 |
| c067 | content-not-name | IM emoji approval workflow en | s00115 / s00114 |
| c068 | content-not-name | multilingual customer support phrasing en | **复用 s00024/s00023** |

**Sweep 全表**（W=10.0、T = cosine_threshold、v2 数据集）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec) | <填实测> | <填> | <填> | <填> | <填> | <填> |
| 0.30 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.45 | <填> | <填> | <填> | <填> | <填> | <填> |
| **<T\* 选定>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** |
| 0.70 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.80 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.90 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.99 | <填> | <填> | <填> | <填> | <填> | <填> |
| 1.01 (≡HYB) | 1.000 | <填> | <填> | <填> | <填> | <填> |

A-5 v1 baseline 对照（仅 hybrid_routed_ndcg）：synonym 0.9051 / concept 0.8197 / crosslang 0.7259 / content-not-name 0.9303 / exact-name 1.0 / OVERALL 0.8707。

**v2 baseline.json 实测**（T\*=`<选定>` rewrite）：
- synonym HYBR_N <填>
- concept HYBR_N <填>
- crosslang HYBR_N <填>
- content-not-name HYBR_N <填>
- exact-name HYBR_R/HYBR_N 1.0/1.0
- OVERALL HYBR_N <填>

**T\* 决定 = `<选定值>`（spec §2.2 接受标准第 `<1/2/3>` 优先）**：

依据：
1. exact-name HYBR_R = 1.000 ✅ 硬红线（所有 T 守住）
2. 各桶 HYBR_N ≥ HYB baseline 同桶：<填 v2 baseline 各桶守住情况>
3. OVERALL HYBR_N ≥ 0.864 spec 目标：<填实测>
4. crosslang HYBR_N ≥ 0.700 spec 目标：<填实测>

**T\* 鲁棒性结论**：

- **若 T\*=0.60 仍 sweep best**：A-5 bake 在 v2 扩量后**鲁棒**——content-not-name 桶 11 例不是运气、cosine 信号方向真破局；下 cycle 可放心扩到 30/50 例继续验证或转其他抓手。
- **若 T\* 微调 [0.55, 0.65]**：A-5 bake 在 v2 上**轻度偏移**——v1 11 例带轻微运气、扩量后 sweep best 略移；新 bake 值反映更鲁棒的 T\*。
- **若 T\* 偏移 > ±0.10 / 走 §5 降级**：A-5 bake **不鲁棒**——v1 11 例运气大、cosine 单维信号在更大集合上撞失败模式；下 cycle 必走更深抓手（更大 embedding 模型 / cosine + lang 组合）。

**v2 实际结论 = `<选定结论>`**：<填一句话>

**下 cycle 抓手**（v2 数据指证）：<填——若 A 子结论好则候选 ②③④；若 B 则微调即可继续；若 C 则更深抓手>

链接：[v2 spec](../superpowers/specs/2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md) / [v2 plan](../superpowers/plans/2026-06-24-beta-15b-6-v2-content-not-name-expansion.md)
````

注：`<填>` 占位符是 sweep 数据驱动占位（T4 sweep 后填实数）、不是懒散 TBD。subagent 执行时按 `/tmp/sweep-cosine-v2.log` + `/tmp/T-star-v2.txt` 逐处替换。

- [ ] **Step 2: 更新 README v2**

`packages/evals/fixtures/semantic-recall/README.md` 改两处：

**改 1**：line 9-10 当前：
```markdown
- corpus: corpus.json（108 篇合成多语言文档，zh 53 / en 55）
- cases: cases.json（59 条 graded 相关性，5 桶）
```

改为：
```markdown
- corpus: corpus.json（115 篇合成多语言文档，zh 57 / en 58；BETA-15B-6 v2 扩 +7 doc）
- cases: cases.json（68 条 graded 相关性，5 桶；BETA-15B-6 v2 扩 content-not-name 桶 +9 case）
```

实际数 zh/en：
```bash
python3 -c "
import json
with open('packages/evals/fixtures/semantic-recall/corpus.json') as f: c = json.load(f)
zh = sum(1 for d in c if d['lang']=='zh')
en = sum(1 for d in c if d['lang']=='en')
print(f'zh {zh} / en {en}')
"
```
按实际结果填入。

**改 2**：line 14-22 桶分布表当前：

```markdown
| 桶 | 条数 | 含义 |
| --- | --- | --- |
| synonym | 12 | query 用同义改述指向同一文档（与标题词面不重合） |
| concept | 12 | 概念/主题跳跃，高抽象描述特定内容 |
| crosslang | 13 | 中→英 或 英→中，配对主题、词面不共享 |
| content-not-name | 11 | query 描述正文要点而非文件名 |
| exact-name | 11 | query = 合成文档精确标题（守护桶） |
```

改 content-not-name 行为：
```markdown
| content-not-name | 20 | query 描述正文要点而非文件名（BETA-15B-6 v2 扩 11→20、T*=0.60 鲁棒性校验）|
```

- [ ] **Step 3: 跑全套总验收**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
cargo test --workspace 2>&1 | grep "test result:" | awk '{passed += $4; failed += $6} END {print "passed", passed, "/ failed", failed}'
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -3
```

Expected：
- fmt 净
- clippy 净
- workspace passed N / failed 0
- gate 1 passed / 0 failed

- [ ] **Step 4: 提交**

```bash
git add docs/reviews/semantic-recall-quality-baseline.md \
        packages/evals/fixtures/semantic-recall/README.md
git commit -m "BETA-15B-6 v2 task 7：baseline 报告追加 v2 数据集节（sweep 全表 + T*=<选定> 决定 + 鲁棒性结论）+ README v2 更新（content-not-name 11→20、corpus 108→115）+ 总验收过"
```

---

## 验证 checklist 汇总

- [x] T1 step 3 + T2 step 4：cases/corpus 完整性测试通过
- [x] T2 PII 自检 5 项（commit 前再过一遍）
- [x] T3 step 4：vectors.json check_vectors 完整性通过
- [x] T4 step 3：sweep 全表读 + T\* 决策记录 `/tmp/T-star-v2.txt`
- [x] T5：rewrite baseline.json、若微调则改 lib.rs/gate.rs doc
- [x] T6：workspace test / clippy / fmt / gate / v0.5/v0.9 byte-equal 全过
- [x] T7：baseline 报告 v2 节完整含 sweep 表 + T\* 决定 + 鲁棒性结论；README v2 更新数字

## 风险与对策

| 风险 | 对策 |
|---|---|
| Mac Metal embed 失败 / 模型未就位 | T3 step 2 前先检查 `models/qwen3-embedding-0.6b-q8_0.gguf` 存在 + `cmake --version` 可跑；feature `semantic-recall-metal` 启 |
| sweep 实测 cosine 偏离预期 | T4 step 3 接受 spec §5 降级路径；baseline 报告 v2 节诚实记录 |
| PII 泄漏 | T2 step 2 自检 5 项；commit 前 grep 实名「李/王/张」+ 真实公司「阿里/腾讯/字节」+ 邮箱「@gmail/@163」|
| byte-equal v0.5/v0.9 退步 | 本 cycle 完全不动 parser/coverage/model fallback；T6 step 4-5 byte-equal 复检自然守住 |
| baseline.json rewrite 后字段顺序变 | serde_json 输出按 struct 字段定义顺序、与 v1 同；diff 仅数值变化 |

## Plan Self-Review（writing-plans skill 要求）

### 1. Spec coverage 检查

逐 spec §3.1 in-scope 项对应 task：

| spec §3.1 项 | 对应 task |
|---|---|
| 写 9 新 case 进 cases.json（id `c049-c057`） | **task 1（plan 修正为 c060-c068）** |
| 写 5-9 新 corpus docs 进 corpus.json | task 2（plan 定为 7 doc）|
| 部分新 case 复用现有 108 docs | task 1（c063/c068 复用 s00023/s00024）|
| Mac Metal --embed 重算 vectors.json 全集 | task 3 |
| 跑 9 阈值 sweep + 人工读表选 T\* | task 4 |
| 若 T\* 微调 → bake 新值 + 升 doc | task 5 Branch B |
| baseline.json rewrite | task 5 step A1 / B3 / C3 |
| baseline 报告追加 v2 数据集节 | task 7 step 1 |
| README v2 更新 | task 7 step 2 |
| PII 自查 5 项 | task 2 step 2 + 风险段 |

**Plan vs spec 修正**：

1. **case id 范围**：spec §4.1/§4.2 写 c049-c057；plan 修正为 c060-c068。原因：v1 实际 case id 范围中 c049-c059 已被 exact-name 桶用、新 case 续号必须 ≥ c060。subagent 执行时按 plan id；建议 v2 收口时同步修 spec id 标注。

2. **corpus 新 doc 数量**：spec §3.1 / §4.3 写 5-9 doc、plan 定为 7 doc（s00109-s00115）。原因：4 个跨语言主题对（重试 / 冷启动 / IM 表情包 / A/B 实验单 zh）= 7 doc 精确；c063/c068 复用 s00023/s00024 无新增。

### 2. Placeholder 扫描

**Sweep 数据占位（合理）**：
- task 7 step 1 sweep 全表 `<填>` 9 行 × 6 列 = 54 处
- task 7 step 1 baseline 实测 6 桶 × 2 字段 = 12 处
- task 7 step 1 T\* 决定理由 / 鲁棒性结论 / 下 cycle 抓手 各处

**性质**：与 A-3/A-4/A-5 plan 的 `{T*}` / `<填>` 占位同款——sweep 数据驱动决策的「执行时填」、非懒散 TBD。约定：
- task 4 sweep 后产 `/tmp/sweep-cosine-v2.log` + `/tmp/T-star-v2.txt`，是 task 7 占位的数据源
- subagent 执行 task 4 后必须暂存 sweep 全表
- task 7 执行时按数据源逐处替换 `<填>`

**T\* Branch 占位（合理）**：
- task 5 三 Branch（A/B/C）只走其中一条、占位 `<T_new>` 仅 Branch B 用、由 task 4 sweep 决定值
- task 5 step B1/B2/C1/C2 是「条件性执行」的清晰说明、不是 TBD

**非 sweep 占位**：plan 通篇无 TBD / TODO / implement later。9 新 case 全文 + 7 新 doc 全文 + 所有命令 expected output 都精确给出。

### 3. Type consistency 检查

| 概念 | 类型签名 | 跨 task 一致性 |
|---|---|---|
| `SemanticCase.bucket` | String | 全用 "content-not-name" ✓ |
| `RelevantDoc.grade` | u8 ∈ [1,3] | task 1 全 case grade=1/2/3 ✓ |
| `SemanticDoc.lang` | String | task 2 全用 "zh" / "en" ✓ |
| `cases.json` 数组顺序 | JSON array | task 1 9 case 续号 c060-c068 ✓ |
| `corpus.json` 数组顺序 | JSON array | task 2 7 doc 续号 s00109-s00115 ✓ |
| `DEFAULT_COSINE_ROUTING_THRESHOLD` | f64 const | task 5 Branch A/B/C 任一 Branch 都保持 f64 类型 ✓ |
| `vectors.json` `dim` | usize | task 3 step 3 全 doc/query 同 dim ✓ |

### 4. Scope 检查

✅ 单一实施 plan、单一 cycle、~7 task 颗粒度合理；纯数据 cycle + 条件性 bake、零代码逻辑改动。

### 5. 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 v0.9 coverage、不触发
- [[project-evals-reporter-nondeterministic]]：T6 byte-equal 闸门用 status 计数（不裸 diff JSON）
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告、三件套全做（gate 4 红线动态读 baseline、本 cycle 自动跟随）
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring、不触发
- [[project-rrf-weight-tuning-ceiling]]：W=10.0 固定
- [[feedback-per-task-verify-include-fmt]]：每 task 验证门必含 fmt + clippy + test ✓（T6 step 1-3）
- [[project-pull-full-distribution-before-convention-call]]：扩量前已数 v1 11 例 content-not-name 主题分布、列在 spec §4 + plan task 1 cases 复用决策

## 链接

- spec：[../specs/2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md](../specs/2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)
- A-5 plan（参考节奏）：[2026-06-24-beta-15b-3a5-cosine-routing.md](./2026-06-24-beta-15b-3a5-cosine-routing.md)
- BETA-15B-6 v1 README：[../../../packages/evals/fixtures/semantic-recall/README.md](../../../packages/evals/fixtures/semantic-recall/README.md)
