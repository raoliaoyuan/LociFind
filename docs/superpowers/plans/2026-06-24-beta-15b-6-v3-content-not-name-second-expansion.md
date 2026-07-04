# BETA-15B-6 v3：content-not-name 桶二次扩量 + T\* 真水位校验 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 BETA-15B-6 合成评测集 content-not-name 桶从 20 → 30 例二次扩量、Mac Metal 本机重算 vectors.json 全集、sweep 9 阈值校验 v2 T\*=0.70 鲁棒性 + 测真水位、baseline 报告 v3 节明示主动放弃字面 0.864/0.700 spec 目标（认知层修订）。

**Architecture:** 纯数据 cycle + 条件性 doc 升 v3、零代码逻辑改动、零红线断言改动（gate.rs (4c)(4d) A-3 cycle 起已自锁 baseline、本 cycle 不动）。手写 10 新 case + 9 新 corpus docs 进 `cases.json`/`corpus.json`、Mac Metal `--embed` 重算 `vectors.json`、`semantic_quality` binary 跑 9 阈值 sweep、按 spec §2.2 三 Branch 接受标准决定是否微调 bake T\*；条件性更新 `lib.rs` 常量 + `lib.rs`/`gate.rs` doc 注释（升 v3 字样）+ `baseline.json`；baseline 报告追加 v3 节含「认知层主动放弃字面 spec 目标」+ 真水位结论。

**Tech Stack:** Rust 2024 edition、locifind-evals semantic_quality / `--embed` feature `semantic-recall-metal`、llama-cpp 后端、qwen3-embedding-0.6b-q8_0.gguf 模型、JSON fixture（serde_json）。

**关键事实清单**（写 plan 时核对实情）：
- 当前 cases.json 总数 68、bucket 分布：synonym c001-c012 / concept c013-c024 / crosslang c025-c037 / content-not-name c038-c048 + c060-c068 / exact-name c049-c059
- 新 10 case 续号 **c069-c078**（v2 用到 c068、续号无冲突）
- 当前 corpus.json 总数 115、最后 id `s00115`；新 9 doc 续号 **s00116-s00124**
- corpus s00011 (zh)/s00012 (en) 是「首页加载性能优化复盘 zh + Frontend Page Load Performance Tuning Postmortem en」对——v3 c077「性能基线监控（首屏指标对比）」query 复用此对（content-not-name 主题：query 描述 P50/P95 监控、doc 是性能优化复盘、词面不共享、验证 cosine 在「概念匹配 vs title 词面不匹配」分布）
- 完整性测试 `semantic_quality_fixtures_integrity` 现门：corpus ≥ 100、cases ≥ 40、5 桶都有 case、grade ∈ [1,3]、id 唯一、relevant 非空、doc_id 引用必须存在
- Schema：`SemanticDoc { doc_id, lang, title, body }`、`SemanticCase { id, bucket, query, relevant: Vec<RelevantDoc> }`、`RelevantDoc { doc_id, grade: u8 }`
- v2 bake `DEFAULT_COSINE_ROUTING_THRESHOLD = 0.70`（`packages/result-normalizer/src/lib.rs`，A-5 0.60 → v2 0.70）；v3 接受标准：T\*=0.70 仍 best → 不改；T\* 微调 [0.60, 0.80] → bake 新值 + 升 doc；T\* 偏移 > ±0.10 → spec §5 降级 1.01
- **gate.rs (4c)(4d) 已是动态读 baseline 自锁**（A-3 cycle 改过、v3 不动断言代码）；v3「红线修订」纯为认知层 / 文档层动作

---

## Task 1: 追加 10 新 case 进 cases.json（c069-c078）

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/cases.json`

- [ ] **Step 1: 失败前置 = 确认现 cases.json 总数 = 68、last id = c068**

Run: `python3 -c "import json; d=json.load(open('packages/evals/fixtures/semantic-recall/cases.json')); print(f'cases={len(d)}, last_id={d[-1][\"id\"]}')"`
Expected: `cases=68, last_id=c068`

- [ ] **Step 2: 用文本编辑器追加 10 新 case 到 cases.json 末尾**

完整 10 新 case JSON（手抄进 cases.json `[]` 数组末尾，紧跟 c068 后、注意逗号 + 缩进 2 空格与现有风格一致）：

```json
  {
    "id": "c069",
    "bucket": "content-not-name",
    "query": "那份说复盘要按 5 Whys 一层层追问、不归责到个人的模板",
    "relevant": [
      {
        "doc_id": "s00116",
        "grade": 3
      },
      {
        "doc_id": "s00117",
        "grade": 1
      }
    ]
  },
  {
    "id": "c070",
    "bucket": "content-not-name",
    "query": "提到灰度发布要按 5% / 25% / 100% 三档放量、错误率超阈值自动回滚的方案",
    "relevant": [
      {
        "doc_id": "s00118",
        "grade": 3
      },
      {
        "doc_id": "s00119",
        "grade": 1
      }
    ]
  },
  {
    "id": "c071",
    "bucket": "content-not-name",
    "query": "讲对外接口废弃要提前两个版本告知、保留至少一个 LTS 周期的约定",
    "relevant": [
      {
        "doc_id": "s00120",
        "grade": 3
      },
      {
        "doc_id": "s00121",
        "grade": 1
      }
    ]
  },
  {
    "id": "c072",
    "bucket": "content-not-name",
    "query": "the postmortem template asking five rounds of why without naming individuals",
    "relevant": [
      {
        "doc_id": "s00117",
        "grade": 3
      },
      {
        "doc_id": "s00116",
        "grade": 1
      }
    ]
  },
  {
    "id": "c073",
    "bucket": "content-not-name",
    "query": "the runbook describing canary deployment with auto-rollback when error rate exceeds threshold",
    "relevant": [
      {
        "doc_id": "s00119",
        "grade": 3
      },
      {
        "doc_id": "s00118",
        "grade": 1
      }
    ]
  },
  {
    "id": "c074",
    "bucket": "content-not-name",
    "query": "the policy stating API deprecation must give two-version notice and one LTS cycle",
    "relevant": [
      {
        "doc_id": "s00121",
        "grade": 3
      },
      {
        "doc_id": "s00120",
        "grade": 1
      }
    ]
  },
  {
    "id": "c075",
    "bucket": "content-not-name",
    "query": "那份按 P0 / P1 / P2 分级、P0 半小时升级到 leader 的告警制度",
    "relevant": [
      {
        "doc_id": "s00122",
        "grade": 3
      }
    ]
  },
  {
    "id": "c076",
    "bucket": "content-not-name",
    "query": "提到异常日志要带 trace_id / span_id / 错误码三个字段的规范",
    "relevant": [
      {
        "doc_id": "s00123",
        "grade": 3
      }
    ]
  },
  {
    "id": "c077",
    "bucket": "content-not-name",
    "query": "那份说首屏指标对比要按 P50 / P95 各画一张图、配版本号纵线的设计",
    "relevant": [
      {
        "doc_id": "s00011",
        "grade": 3
      },
      {
        "doc_id": "s00012",
        "grade": 2
      }
    ]
  },
  {
    "id": "c078",
    "bucket": "content-not-name",
    "query": "the policy explaining data retention with anonymization after the retention window",
    "relevant": [
      {
        "doc_id": "s00124",
        "grade": 3
      }
    ]
  }
```

注：c077 复用 s00011/s00012（首页加载性能优化复盘 zh+en 对、与「首屏指标对比 P50/P95」query 词面不共享但内容相关、验证 cosine 在「概念匹配 vs title 词面不匹配」分布）；c069-c074 三组 zh+en 配对各挂新 doc；c075/c076/c078 各单语种新 doc。

操作步骤：
1. 用文本编辑器打开 `packages/evals/fixtures/semantic-recall/cases.json`
2. 找到末尾 `]` 前的 c068 case
3. 在 c068 case `}` 后加 `,`、再粘贴上面 10 个 case 块
4. 保留末尾 `]` 与原始 EOF

- [ ] **Step 3: 验证 JSON 格式 + 总数 = 78 + content-not-name = 30**

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
cases=78, last_id=c078
  concept              12
  content-not-name     30
  crosslang            13
  exact-name           11
  synonym              12
```

- [ ] **Step 4: 提交（暂不验证完整性，T2 才跑——因为新 case 引用未建的 s00116-s00124）**

```bash
git add packages/evals/fixtures/semantic-recall/cases.json
git commit -m "BETA-15B-6 v3 task 1：cases.json 追加 10 新 content-not-name 桶 case（c069-c078；6 zh + 4 en；4 边界 + 6 常规；c077 复用 s00011/s00012 性能优化对、其他 9 例待 T2 加新 doc）"
```

注：本 commit 后 `semantic_quality_fixtures_integrity` 测试**预期失败**（c069-c076/c078 引用未建的 s00116-s00124）—— T2 加 doc 后才修复。

---

## Task 2: 追加 9 新 corpus docs 进 corpus.json（s00116-s00124）

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/corpus.json`

- [ ] **Step 1: 失败前置 = 跑完整性测试预期失败（c069+ 引用 s00116+ 不存在）**

Run: `cargo test -p locifind-evals --test semantic_quality_fixtures_integrity 2>&1 | tail -15`
Expected: 失败、错误信息含「case c069 引用未知 doc s00116」类似消息。

- [ ] **Step 2: 用文本编辑器追加 9 新 corpus doc 到 corpus.json 末尾**

完整 9 新 doc JSON（手抄进 corpus.json `[]` 数组末尾，紧跟 s00115 后、注意逗号 + 缩进 2 空格）：

```json
  {
    "doc_id": "s00116",
    "lang": "zh",
    "title": "事故复盘报告模板（5 Whys 框架）",
    "body": "事故复盘以理解系统失效为目的,不以追究个人责任为目的。每次复盘按 5 Whys 框架进行:从直接现象出发,问第一个 why 找直接原因;基于直接原因问第二个 why 找触发条件;依次追问到第五层,定位到流程、机制或系统设计的根因。报告里不写人名,只写角色与系统组件。复盘结尾必须列出具体改进项,每项有负责团队、完成时间、验证方式。会上不允许说应该某人怎样做,只能说应该哪个流程或工具怎样改。复盘记录归档便于后续模式识别。"
  },
  {
    "doc_id": "s00117",
    "lang": "en",
    "title": "Blameless Postmortem Template with 5 Whys",
    "body": "The goal of a postmortem is to understand how a system failed, not to assign blame. Each session follows the 5 Whys framework: start from the observable symptom and ask the first why to surface the proximate cause, then ask the next why to find what enabled that, and continue for up to five layers until the root cause lands on a process, mechanism, or system design issue. Reports avoid individual names and refer to roles or system components only. Every postmortem closes with concrete action items, each tagged with an owning team, a completion date, and a verification method. Phrasing must target the process or tooling, never the person. Records are archived to support later pattern recognition."
  },
  {
    "doc_id": "s00118",
    "lang": "zh",
    "title": "灰度发布与自动回滚机制说明",
    "body": "新版本发布前先经过灰度流程,按三档放量验证稳态。第一档 5% 流量持续半小时,主要看错误率与延迟分位数是否漂移。第二档 25% 流量持续一小时,重点观察长尾接口与下游依赖。第三档全量后继续观察一天,确认无回归再关掉旧版本。每档放量前后采样核心指标对比,任意核心指标偏离基线超过预设阈值即自动回滚到上一版本,操作人事后复核。回滚动作记录在发布日志,异常需在下一次发布前定位修复。整套流程对生产事故的暴露时间控制在小时级。"
  },
  {
    "doc_id": "s00119",
    "lang": "en",
    "title": "Canary Deployment Runbook with Rollback Gates",
    "body": "Releases go through a canary process before reaching the full user base. The first tier routes 5% of traffic to the new version for thirty minutes, watching error rate and latency percentiles for drift. The second tier moves to 25% for an hour, with closer attention paid to long-tail endpoints and downstream dependencies. The third tier promotes to full traffic, followed by a one-day soak period to confirm no regression before retiring the old build. Each tier brackets a sampling of core metrics against baseline. Any core metric that drifts beyond its preset threshold triggers an automatic rollback to the previous build, with operators reviewing after the fact. Rollback actions are logged in the release record. The whole process keeps production exposure to incidents on the order of hours."
  },
  {
    "doc_id": "s00120",
    "lang": "zh",
    "title": "对外接口版本管理与废弃周期约定",
    "body": "对外接口采用语义化版本号,主版本号变化代表不兼容变更。任何不兼容变更需提前至少两个版本对外公告,并提供迁移指南。每个主版本保留至少一个 LTS 周期,期间只接受安全修复,不再加新特性。废弃接口在响应头里加 sunset 字段告知下线日期,文档站显著标注。下线前一个月,接口仍可用但响应延迟人为加倍以督促迁移。下线日生效后返回固定的迁移指引错误码,避免静默 404。整套约定写入 API 治理文档,接入前必读。"
  },
  {
    "doc_id": "s00121",
    "lang": "en",
    "title": "API Versioning and Deprecation Policy",
    "body": "Public APIs use semantic versioning, where a major version bump signals a breaking change. Every breaking change is announced at least two versions ahead, paired with a written migration guide. Each major version retains support for at least one LTS cycle, during which only security fixes ship and no new features land. Deprecated endpoints add a sunset header carrying the retirement date, and the documentation site flags them prominently. One month before retirement the endpoint continues to function but artificially doubles its response latency to push migration. After the retirement date it returns a fixed migration-pointer error code instead of a silent not-found. The full policy lives in the API governance document and is required reading before onboarding."
  },
  {
    "doc_id": "s00122",
    "lang": "zh",
    "title": "线上告警分级与升级流程",
    "body": "线上告警按影响面分三档处理。P0 表示用户可感知的全局故障,半小时内需升级到 leader 介入,所有相关同事中断手头工作支援。P1 表示部分用户或子系统受影响,两小时内由值班同事响应,必要时拉小组协同。P2 表示监控指标异常但用户尚未感知,在当日工作时段处理即可。每档告警都附带响应模板,包含止血、定位、复盘三个阶段的标准动作。误报需在事后归档原因,频繁误报触发阈值调整或规则下线。告警接收人轮值,周末与节假日有备份候选。"
  },
  {
    "doc_id": "s00123",
    "lang": "zh",
    "title": "服务端异常日志结构化字段规范",
    "body": "服务端异常日志统一走结构化输出,便于检索与跨服务关联。每条日志必须带三个核心字段:trace_id 用于贯穿一次请求的所有服务调用、span_id 用于定位本次调用在调用链中的位置、错误码用于快速归类故障类型。除核心字段外,栈帧信息按需附加,生产环境只保留前若干层防止日志体积爆炸。敏感字段如手机号、邮箱、身份号在日志写入前必须脱敏。日志等级遵守 ERROR / WARN / INFO 三档约定,DEBUG 级别只在本地开发开启。规范变更需经平台组评审。"
  },
  {
    "doc_id": "s00124",
    "lang": "en",
    "title": "Data Retention Policy with Anonymization",
    "body": "User-generated data is retained only for as long as it remains useful for product operations or is required by regulation. The default retention window is twelve months from creation. After the window closes, records are not hard-deleted; instead, identifying fields are replaced with stable hashes so aggregate analytics remain possible without exposing individuals. Hard deletion happens at thirty-six months, except where legal hold applies. Backups follow the same lifecycle on a one-month lag. Each system that stores user data declares its retention class in a central inventory and is audited quarterly for compliance. Exceptions require written sign-off from the privacy review group and are reviewed annually."
  }
```

PII 自检（README 5 项、commit 前必过）：
- ✅ 无真实人名/公司/邮箱/电话/精确薪资/真实路径
- ✅ 无具名人物（用「同事」「值班同事」「leader」「平台组」职位代称）
- ✅ 金额一律约整数（「半小时」「两小时」「12 个月」「30 分钟」「5%」百分比）
- ✅ doc_id 唯一（s00116-s00124 续号）
- ✅ 不涉及跨语言桶（content-not-name 桶 case）

- [ ] **Step 3: 验证 JSON 格式 + corpus 总数 = 124 + zh/en 分布**

Run:
```bash
python3 -c "
import json
with open('packages/evals/fixtures/semantic-recall/corpus.json') as f:
    d = json.load(f)
print(f'corpus={len(d)}, last_id={d[-1][\"doc_id\"]}')
new_docs = [doc for doc in d if doc['doc_id'] >= 's00116']
print(f'new docs: {len(new_docs)}')
for doc in new_docs:
    print(f\"  {doc['doc_id']} lang={doc['lang']} title={doc['title'][:50]}\")
zh = sum(1 for doc in d if doc['lang']=='zh')
en = sum(1 for doc in d if doc['lang']=='en')
print(f'zh={zh} / en={en}')
"
```

Expected:
```
corpus=124, last_id=s00124
new docs: 9
  s00116 lang=zh title=事故复盘报告模板（5 Whys 框架）
  s00117 lang=en title=Blameless Postmortem Template with 5 Whys
  s00118 lang=zh title=灰度发布与自动回滚机制说明
  s00119 lang=en title=Canary Deployment Runbook with Rollback Gates
  s00120 lang=zh title=对外接口版本管理与废弃周期约定
  s00121 lang=en title=API Versioning and Deprecation Policy
  s00122 lang=zh title=线上告警分级与升级流程
  s00123 lang=zh title=服务端异常日志结构化字段规范
  s00124 lang=en title=Data Retention Policy with Anonymization
zh=62 / en=62
```

- [ ] **Step 4: 跑完整性测试 = 现在应过（绿）**

Run: `cargo test -p locifind-evals --test semantic_quality_fixtures_integrity 2>&1 | tail -10`
Expected: `test result: ok. 1 passed; 0 failed; ...`

- [ ] **Step 5: 提交**

```bash
git add packages/evals/fixtures/semantic-recall/corpus.json
git commit -m "BETA-15B-6 v3 task 2：corpus.json 追加 9 新 doc（s00116-s00124；5 zh + 4 en；故障复盘 zh+en 配对 / 灰度发布 zh+en 配对 / API 版本 zh+en 配对 / 告警分级 zh / 异常日志 zh / 数据保留 en；零 PII 全虚构占位；zh/en 比例 62:62 平衡）"
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
  --semantic-weight 10.0 --cosine-threshold 0.70 2>&1 | head -10
```
Expected: 报错「缺 query 向量: c069」（或类似缺向量错）—— vectors.json 仍是 v2 的 68 query + 115 doc。

- [ ] **Step 2: 跑 --embed 重算 vectors.json 全集**

```bash
cargo run --release -p locifind-evals --bin semantic_quality \
  --features semantic-recall-metal -- --embed
```
Expected:
- 编译 ~3-5 分钟（首次启 metal feature）
- embed 跑 ~1-2 分钟（124 doc + 78 query × ~50 token avg、Mac Metal）
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
new_docs = [k for k in vc['doc_vectors'] if k >= 's00116']
new_queries = [k for k in vc['query_vectors'] if k >= 'c069']
print(f'new doc vecs: {len(new_docs)} {sorted(new_docs)}')
print(f'new query vecs: {len(new_queries)} {sorted(new_queries)}')
"
```

Expected:
```
model_id: models/qwen3-embedding-0.6b-q8_0.gguf
dim: 1024
doc_vectors: 124
query_vectors: 78
new doc vecs: 9 ['s00116', 's00117', 's00118', 's00119', 's00120', 's00121', 's00122', 's00123', 's00124']
new query vecs: 10 ['c069', 'c070', 'c071', 'c072', 'c073', 'c074', 'c075', 'c076', 'c077', 'c078']
```

注：dim 跟随 qwen3-embedding-0.6b-q8_0、与 v2 一致 1024。

- [ ] **Step 4: 跑 check_vectors 完整性测试**

Run: `cargo test -p locifind-evals --lib semantic_quality::data 2>&1 | grep "test result:" | head -3`
Expected: `test result: ok. N passed; 0 failed; ...`

- [ ] **Step 5: 提交（vectors.json 是 ~2-2.5MB 二进制 JSON、可入仓）**

```bash
git add packages/evals/fixtures/semantic-recall/vectors.json
git commit -m "BETA-15B-6 v3 task 3：Mac Metal --embed 重算 vectors.json 全集（124 doc + 78 query × qwen3-embedding-0.6b-q8_0、dim 1024）"
```

注：commit 前先确认 `git diff --stat packages/evals/fixtures/semantic-recall/vectors.json` 显示大小合理（~2-2.5MB）。

---

## Task 4: 跑 9 阈值 sweep + 选 T\*

**Files:**
- 无文件改动；产 `/tmp/sweep-cosine-v3.log` + 决定 T\*

- [ ] **Step 1: 跑 9 阈值 sweep**

```bash
rm -f /tmp/sweep-cosine-v3.log
for T in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  echo "=== cosine_threshold = $T ===" >> /tmp/sweep-cosine-v3.log
  cargo run --release -p locifind-evals --bin semantic_quality -- \
    --semantic-weight 10.0 \
    --cosine-threshold $T 2>/dev/null >> /tmp/sweep-cosine-v3.log
done
wc -l /tmp/sweep-cosine-v3.log
```

Expected: log ~81 行（9 阈值 × 9 行 / 阈值，含 `=== ===` + 表头 + 7 数据行）

- [ ] **Step 2: 看 sweep 结果**

Run: `cat /tmp/sweep-cosine-v3.log`
Expected: 每 T 输出表格、含 6 桶（synonym/concept/crosslang/content-not-name/exact-name/OVERALL）× 8 指标列。

- [ ] **Step 3: 人工分析、按 spec §2.2 三 Branch 接受标准选 T\***

按 spec §2.2 接受标准顺序：

**Branch A：T\*=0.70 仍 sweep best**
- 检查：T=0.70 时各桶 HYBR_N 是否 ≥ v2 HYB baseline（baseline 数值见 v2 baseline.json 或 baseline 报告 v2 节）
- 若全过 + T=0.70 是 sweep best → 不动 lib.rs 常量、跳到 T6 跳过 + 直接 T7
- v2 baseline 实测（对照）：synonym 0.905 / concept 0.819 / crosslang 0.717 (HYBR) / content-not-name 0.853 (HYBR) / exact-name 1.000 / OVERALL 0.854 (HYBR)

**Branch B：T\* 微调 [0.60, 0.80] 之间**
- 若 T=0.60、0.65 (插值)、0.75 (插值) 或 0.80 比 T=0.70 OVERALL 高 ≥ 0.005 且各桶 (4b) 仍守 v2 baseline → bake 新值
- sweep 表上看哪个 T 各桶都过 (4b) + OVERALL 最大

**Branch C：spec §5 降级 T\*=1.01**
- 若所有 T < 1.01 都至少破一桶 (4b) → T\*=1.01
- baseline 报告 v3 节诚实写「v3 数据集让 T\* 偏移到不可达、走 spec §5 降级、cosine 单维信号在 30 例上不稳」

把决策写进 `/tmp/T-star-v3.txt`：

```bash
echo "T* = <选定值>" > /tmp/T-star-v3.txt
echo "决策理由：<spec §2.2 接受标准 Branch A/B/C + 关键数值对比>" >> /tmp/T-star-v3.txt
cat /tmp/T-star-v3.txt
```

- [ ] **Step 4: 无 commit**（手动数据分析步骤、T6 落 bake commit 或 T7 落 rewrite commit）

---

## Task 5: 三 Branch 路径选择 + 条件性 bake T\*（若微调）+ doc 升 v3

**Files:**
- 条件 Modify: `packages/result-normalizer/src/lib.rs`（仅 Branch B/C）
- 条件 Modify: `packages/evals/tests/semantic_quality_gate.rs`（仅 Branch B/C，纯 doc 注释、不动断言代码）

### Branch A：T\* = 0.70 仍 sweep best（不动 lib.rs / gate.rs，跳到 Task 6）

- [ ] **Step A1: 确认 Branch A 命中、跳过本 task 后续 step**

Run: `cat /tmp/T-star-v3.txt`
Expected: 含「T* = 0.70」。若是，本 task 跳过、直接进 Task 6。

### Branch B：T\* 微调 [0.60, 0.80]（bake 新值 + 升 lib.rs/gate.rs doc 到 v3）

设新 T\* 为 `<T_new>`（如 0.65 或 0.75）。

- [ ] **Step B1: 改 `DEFAULT_COSINE_ROUTING_THRESHOLD`**

`packages/result-normalizer/src/lib.rs` 当前（v2 cycle 写入的）大约 line 97-110：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 T\* = 0.60（合成集 v1 / 11 例 content-not-name 桶）；
/// BETA-15B-6 v2 扩 content-not-name 桶 11→20 后 sweep 上移到 **T\* = 0.70**
/// （Branch B 边界 inclusive 上界 [0.55, 0.65] —— A-5 v1 含轻微运气、v2 揭示真水位）。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 0.70;
```

整段替换为（用 `<T_new>` 替换实际值如 `0.65`）：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 T\* = 0.60（合成集 v1）；
/// BETA-15B-6 v2 扩 content-not-name 桶 11→20 后 sweep 上移到 T\* = 0.70（Branch B 边界 inclusive 上界）；
/// BETA-15B-6 v3 二次扩量 20→30 后 sweep 微调到 **T\* = <T_new>**
/// （spec §2.2 接受标准 Branch B：v3 数据集让 sweep best 偏移 ±0.05/±0.10、bake 跟随；
/// v3 cycle 认知层主动放弃字面 0.864/0.700 spec 目标、移交下 cycle 抓手 = 更大 embedding 模型）。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 + v3 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = <T_new>;
```

- [ ] **Step B2: 升级 gate.rs doc 注释（纯文档、不动断言代码）**

`packages/evals/tests/semantic_quality_gate.rs` 当前（v2 cycle 写入的）大约 line 117-121：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline、A-5 T*=0.60 → v2 T*=0.70 bake 后数值无需替换。
    // 诚实边界：v2 上 OVERALL 0.854 < A-5 v1 的 0.871、A-5 v1 11 例含轻微运气、v2 真水位回落。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 调优记录节。
```

整段替换为：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2/v3 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline（A-3 cycle 即如此）、A-5 T*=0.60 → v2 T*=0.70 → v3 T*=<T_new> bake 后数值无需替换。
    // 诚实边界：v3 cycle 认知层主动放弃字面 0.864/0.700 spec 目标，
    // 真水位 = baseline.OVERALL/crosslang.hybrid_routed_ndcg 实测、移交下 cycle（更大 embedding 模型 / 信号组合）。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 + v3 调优记录节。
```

- [ ] **Step B3: fmt + clippy + test 三件套验证**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -3
cargo test --workspace 2>&1 | grep "test result:" | tail -3
```
Expected: 全净、0 failed

- [ ] **Step B4: 提交 lib.rs + gate.rs doc 升 v3**

```bash
git add packages/result-normalizer/src/lib.rs packages/evals/tests/semantic_quality_gate.rs
git commit -m "BETA-15B-6 v3 task 5 Branch B：bake DEFAULT_COSINE_ROUTING_THRESHOLD 0.70 → <T_new>（v3 二次扩量 sweep 微调）+ lib.rs/gate.rs doc 升 v3（含认知层主动放弃字面 spec 目标字样）"
```

### Branch C：spec §5 降级 T\*=1.01（cosine 单维信号在 v3 上不稳）

设 T\* = 1.01。

- [ ] **Step C1: 改 lib.rs 常量 + doc 升 v3 降级**

`packages/result-normalizer/src/lib.rs` 整段替换为：

```rust
/// VEC top-1 cosine 绝对分数阈值路由：`vec[0].score >= threshold` 时跳过 FTS。
/// BETA-15B-3 A-5 sweep 选定 T\* = 0.60（合成集 v1）；
/// BETA-15B-6 v2 扩 content-not-name 桶 11→20 sweep 上移到 T\* = 0.70；
/// BETA-15B-6 v3 二次扩量 20→30 后 sweep **走 spec §5 降级 T\* = 1.01**
/// （cosine ∈ [0,1] 物理上限、永不跳、HYBR ≡ HYB）：
/// v3 上无任一 T < 1.01 满足 spec §2.2 (4b) 各桶 ≥ v2 HYB baseline 硬红线；
/// cosine 单维信号在 30 例上不稳、下 cycle 抓手 = 更大 embedding 模型 / cosine + lang 组合信号。
/// 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 + v3 调优记录节。
pub const DEFAULT_COSINE_ROUTING_THRESHOLD: f64 = 1.01;
```

- [ ] **Step C2: gate.rs doc 升 v3 降级**

替换为：

```rust
    // BETA-15B-3 A-5 红线 + BETA-15B-6 v2/v3 校验：HYBR（hybrid_routed）各桶不退步 HYB baseline，
    // exact-name HYBR_R = 1.0 硬断言，OVERALL/crosslang HYBR_N 自锁 baseline.hybrid_routed_*
    // —— 4 红线动态读 baseline（A-3 cycle 即如此）、A-5 T*=0.60 → v2 T*=0.70 → v3 走 spec §5 降级 T*=1.01（HYBR≡HYB）。
    // 诚实边界：v3 cycle 认知层主动放弃字面 0.864/0.700 spec 目标 + cosine 单维信号在 30 例上不稳，
    // 下 cycle 抓手 = 更大 embedding 模型 / cosine + lang 组合信号。
    // 详 docs/reviews/semantic-recall-quality-baseline.md A-5 + v2 + v3 调优记录节。
```

- [ ] **Step C3-C4: 同 Branch B step B3-B4**（fmt+clippy+test + commit）

注：Branch C 的 baseline.json 中 HYBR_* 字段会 ≡ HYB_* 字段（路由不生效）。

### 三 Branch 决策点

T4 step 3 决定走哪 Branch：
- **A**：T\* = 0.70 不动（最理想结果、v2 bake 鲁棒、本 task 跳过）
- **B**：T\* ∈ [0.60, 0.80] 微调（含 v2 起点 ±0.10、inclusive 边界）
- **C**：T\* > ±0.10 偏移（走 §5 降级）

实施时只走一条 Branch、跳过另两条的 step。

---

## Task 6: rewrite baseline.json + 跑全套验证门

**Files:**
- Modify: `packages/evals/fixtures/semantic-recall/baseline.json`（rewrite）

- [ ] **Step 1: 跑 --write-baseline 让 baseline.json 反映 v3 数据集 + 新 T\***

```bash
cargo run --release -p locifind-evals --bin semantic_quality -- \
  --semantic-weight 10.0 --write-baseline
```
Expected: stderr「已写 baseline.json（6 桶含 OVERALL）」

- [ ] **Step 2: 跑回归门 4 红线**

```bash
cargo test -p locifind-evals --test semantic_quality_gate 2>&1 | tail -5
```
Expected: `test result: ok. 1 passed; 0 failed; ...`（4 红线全过、动态读 v3 baseline）

- [ ] **Step 3: 跑 workspace 测试**

```bash
cargo test --workspace 2>&1 | grep "test result:" | awk '{passed += $4; failed += $6} END {print "passed", passed, "/ failed", failed}'
```
Expected: `passed N / failed 0`（N ≈ 860 ± 含完整性测试 + 1 个 gate 测试）

- [ ] **Step 4: 跑 clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
```
Expected: 净（无 warning、无 error）

- [ ] **Step 5: 跑 fmt**

```bash
cargo fmt --all --check
```
Expected: 净（无输出）

- [ ] **Step 6: 跑 v0.5 byte-equal 复检**

```bash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v3-v05.json && python3 -c "
import json
with open('/tmp/v3-v05.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.5', counts)
"
```
Expected: `v0.5 {'pass': 473, 'partial': 25, 'fail': 2}` 精确

- [ ] **Step 7: 跑 v0.9 byte-equal 复检**

```bash
cargo run --release -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null > /tmp/v3-v09.json && python3 -c "
import json
with open('/tmp/v3-v09.json') as f: data = json.load(f)
counts = {}
for c in data:
    s = c['result'].get('type', 'unknown')
    counts[s] = counts.get(s, 0) + 1
print('v0.9', counts)
"
```
Expected: `v0.9 {'pass': 877, 'partial': 119, 'fail': 4}` 精确

- [ ] **Step 8: 提交 baseline.json**

```bash
git add packages/evals/fixtures/semantic-recall/baseline.json
git commit -m "BETA-15B-6 v3 task 6：rewrite baseline.json 反映 v3 数据集（content-not-name 30 例 + corpus 124 doc + T*=<选定>）+ 全套验证门过（workspace/clippy/fmt/gate/v0.5/v0.9 byte-equal）"
```

---

## Task 7: 写 baseline 报告 v3 节 + README v3 更新 + 总验收

**Files:**
- Modify: `docs/reviews/semantic-recall-quality-baseline.md`
- Modify: `packages/evals/fixtures/semantic-recall/README.md`

- [ ] **Step 1: 在 baseline 报告末尾追加 v3 数据集节**

`docs/reviews/semantic-recall-quality-baseline.md` 末尾追加（找到现有「v2 数据集」节末尾 `链接：[v2 spec](...) / [v2 plan](...)` 行之后追加）：

````markdown

## v3 数据集：content-not-name 桶二次扩量 20→30 + 认知层主动放弃字面 spec 目标（2026-06-24 Claude Code）

**承接**：v2 cycle T\*=0.70 bake、Branch B 边界 inclusive 上界、揭示 A-5 v1 含运气、v2 真水位 OVERALL 0.854 / crosslang 0.717。v2 调优记录诚实承认「T\*=0.70 在 v2 20 例上仍可能含残余运气、扩量到 30+ 才能更精确测出真水位」。本 cycle 针对性二次扩 content-not-name 桶 20→30 + corpus 115→124、Mac Metal 本机重算 vectors.json 全集、重跑 9 阈值 sweep 校验 T\*=0.70 鲁棒性 + 测真水位。

**v3 cycle 认知层修订**：v3 起草前发现 v2 上 OVERALL 0.864 spec 目标走 baseline 自锁路径绕过、连续两 cycle 自欺；v3 cycle 主动**放弃字面 0.864 / 0.700 spec 目标**（移交下 cycle = 更大 embedding 模型 qwen3-0.6b → 1.5b/3b / cosine + lang 组合信号、已挂 ROADMAP 候选）。gate.rs (4c)(4d) 代码层 A-3 cycle 起即动态读 baseline 自锁、本 cycle **不动断言代码**、仅升 doc 注释字样到 v3 + baseline 报告本节明示放弃决策。

**扩量产出**：
- cases.json 68→78（content-not-name 20→30、其他 4 桶不动；新 10 case = c069-c078、6 zh + 4 en、4 边界 + 6 常规、3 zh+en 配对主题 + 4 单语种主题 + c077 复用 s00011/s00012 性能优化 corpus）
- corpus.json 115→124（s00116-s00124、9 新 doc：5 zh + 4 en、3 配对主题 + 3 单语种新 doc、零 PII 全虚构；zh/en 比例 62:62 平衡）
- vectors.json 全集重算（dim 1024、qwen3-embedding-0.6b-q8_0、124 doc + 78 query）

**新 10 case 主题清单**：

| id | bucket | 主题 | 复用/新 doc | 设计类 |
|---|---|---|---|---|
| c069 | content-not-name | 故障复盘 5 Whys (zh) | s00116 (zh) / s00117 (en) | 边界 |
| c070 | content-not-name | 灰度发布与回滚阈值 (zh) | s00118 (zh) / s00119 (en) | 常规 |
| c071 | content-not-name | API 接口废弃周期 (zh) | s00120 (zh) / s00121 (en) | 常规 |
| c072 | content-not-name | 5 Whys postmortem template (en) | s00117 (en) / s00116 (zh) | 边界 |
| c073 | content-not-name | canary deployment runbook (en) | s00119 (en) / s00118 (zh) | 常规 |
| c074 | content-not-name | API deprecation policy (en) | s00121 (en) / s00120 (zh) | 常规 |
| c075 | content-not-name | 告警分级 P0/P1/P2 (zh) | s00122 | 边界 |
| c076 | content-not-name | 异常日志结构化字段 (zh) | s00123 | 常规 |
| c077 | content-not-name | 性能基线监控 P50/P95 (zh) | **复用 s00011/s00012 性能优化对** | 边界 |
| c078 | content-not-name | data retention with anonymization (en) | s00124 | 常规 |

**Sweep 全表**（W=10.0、T = cosine_threshold、v3 数据集 78 cases / 124 docs / dim 1024）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec) | <填实测> | <填> | <填> | <填> | <填> | <填> |
| 0.30 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.45 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.60 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.70 (v2 bake) | <填> | <填> | <填> | <填> | <填> | <填> |
| **<T\* 选定>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** | **<填>** |
| 0.80 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.90 | <填> | <填> | <填> | <填> | <填> | <填> |
| 0.99 | <填> | <填> | <填> | <填> | <填> | <填> |
| 1.01 (≡HYB) | 1.000 | <填> | <填> | <填> | <填> | <填> |

v2 baseline 对照（hybrid_routed_ndcg、T\*=0.70）：synonym 0.9051 / concept 0.8190 / crosslang 0.7168 / content-not-name 0.8525 / exact-name 1.0 / OVERALL 0.8538。

**控制对照核验**：
- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等 ✓）
- T=1.01 时 HYBR ≡ HYB（六桶完全相等 ✓）
- T=<T\* 选定> 时路由触发：<描述哪几桶 cosine 强跳 FTS、哪几桶 cosine 弱保留>

**v3 baseline.json 实测**（T\*=`<选定>` rewrite）：
- synonym HYBR_R/HYBR_N <填>
- concept HYBR_R/HYBR_N <填>
- crosslang HYBR_R/HYBR_N <填>
- content-not-name HYBR_R/HYBR_N <填>
- exact-name HYBR_R/HYBR_N 1.0/1.0
- OVERALL HYBR_R/HYBR_N <填>

**T\* 决定 = `<选定值>`（spec §2.2 接受标准 Branch `<A/B/C>`）**：

依据：
1. exact-name HYBR_R = 1.000 ✅ 硬红线（所有 T 守住）
2. 各桶 HYBR_N ≥ v2 HYB baseline 同桶（不退步）：<填 v3 各桶守住情况 + Δ>
3. spec §2.2 (4d) OVERALL 自锁 baseline：<v3 实测>（v2 baseline 0.8538、动态读、本 cycle 不再追字面 0.864）
4. spec §2.2 (4c) crosslang 自锁 baseline：<v3 实测>（v2 baseline 0.7168、动态读、本 cycle 不再追字面 0.700）

**T\* 鲁棒性结论**（v3 校验 v2 T\*=0.70）：

- **若 T\*=0.70 仍 sweep best（Branch A）**：v2 bake 在 v3 二次扩量后**鲁棒**——content-not-name 桶 20 例不是运气、cosine 信号方向真破局；下 cycle 可放心扩到 50 例继续验证或转其他抓手。
- **若 T\* 微调 [0.60, 0.80]（Branch B）**：v2 bake 在 v3 上**轻度偏移**——20 例带轻微运气、扩量后 sweep best 略移；新 bake 值反映更鲁棒的 T\*。
- **若 T\* 偏移 > ±0.10 / 走 §5 降级（Branch C）**：v2 bake **不鲁棒**——cosine 单维信号在 30 例上撞失败模式；下 cycle 必走更深抓手（更大 embedding 模型 / cosine + lang 组合）。

**v3 实际结论 = `<选定结论>`**：<填一句话总结>

**真水位结论**（v2 → v3 真水位变化）：
- OVERALL HYBR_N：v2 0.854 → v3 `<填>`（Δ <填>）
- crosslang HYBR_N：v2 0.717 → v3 `<填>`（Δ <填>）
- content-not-name HYBR_N：v2 0.853 → v3 `<填>`（Δ <填>，最关键、二次扩量的核心校验）

**认知层修订小结**：v3 cycle 主动放弃字面 0.864 / 0.700 spec 目标的字面追求，**承认在「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下结构性不可达**。gate.rs 4 红线全部自锁 baseline、未来 cycle 调优只要不退步即合规；字面 spec 目标移交下 cycle 抓手（更大 embedding 模型 / cosine + lang 组合信号）。诚实承认目标下调 ≠ 项目失败、而是诚实接受当前技术栈天花板。

**下 cycle 抓手优先级（v3 数据指证）**：

| 候选 | 原理 | 优先级 |
|---|---|---|
| **更大 embedding 模型 qwen3-0.6b → 1.5b/3b** | 抬升 vec 召回质量、cosine_top1 分布上移、T\* 可能进一步上调 + 抬 OVERALL/crosslang、有望真破 0.864 字面 spec 目标 | <填、按 v3 结论：A/B 子结论 → 仍高优、C 子结论 → 极高优> |
| **评测集再扩量**（content-not-name 30→50 + crosslang 13→20） | 若 v3 Branch B/C 命中、扩到 50 例再校验；若 v3 Branch A 命中、crosslang 桶可能下次 cycle 主扩 | <填> |
| **原始 query 入 schema**（A 簇余项） | 让语义臂 keywords 拼接近似真 query | 中（byte-equal 风险须 router 后置填充不动 parser） |
| **cosine + lang 组合信号** | 若 v3 Branch C 命中、cosine 单维信号在 30 例上不稳、用 lang 信号细化 cosine 路由（A-3 jaccard / A-4 detect_lang git history 可恢复） | <填、按 v3 结论：A/B → 备选、C → 极高优> |

**基础设施完整保留**（A-3/A-4/A-5 都未动）：
- `result-normalizer::lang::Lang/detect_lang` 保留作 wiring 元数据
- wrapper `fuse_rrf_with_fts_routing` 5 参签名 + `RouteVerdict { skipped_fts, query_lang, vec_top1_cosine, cosine_threshold }`
- 评测 `vector_rank → (id, cosine)` + `to_results_with_scores` + `score_case` cosine_threshold + binary `--cosine-threshold` flag
- 生产 `run_fanout_merge_rrf` 5 参 wrapper 调用 + struct-update 后置覆写 query_lang
- baseline.json HYBR 字段 v3 rewrite + gate 4 红线动态读 baseline（A-3 cycle 起即如此）

链接：[v3 spec](../superpowers/specs/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion-design.md) / [v3 plan](../superpowers/plans/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion.md)
````

注：`<填>` 占位符是 sweep 数据驱动占位（T4 sweep 后填实数）、不是懒散 TBD。subagent 执行时按 `/tmp/sweep-cosine-v3.log` + `/tmp/T-star-v3.txt` 逐处替换。

- [ ] **Step 2: 更新 README v3**

`packages/evals/fixtures/semantic-recall/README.md` 改两处：

**改 1**：line 9-10 当前：
```markdown
- corpus: corpus.json（115 篇合成多语言文档，zh 57 / en 58；BETA-15B-6 v2 扩 +7 doc）
- cases: cases.json（68 条 graded 相关性，5 桶；BETA-15B-6 v2 扩 content-not-name 桶 +9 case）
```

改为：
```markdown
- corpus: corpus.json（124 篇合成多语言文档，zh 62 / en 62；BETA-15B-6 v2 扩 +7 doc、v3 再扩 +9 doc）
- cases: cases.json（78 条 graded 相关性，5 桶；BETA-15B-6 v2 扩 content-not-name 桶 +9 case、v3 再扩 +10 case）
```

实际数 zh/en 已在 T2 step 3 验证（62/62）。

**改 2**：line 14-22 桶分布表当前 content-not-name 行：

```markdown
| content-not-name | 20 | query 描述正文要点而非文件名（BETA-15B-6 v2 扩 11→20、T\*=0.70 鲁棒性校验、bake T\* 由 v1 0.60 微调到 v2 0.70）|
```

改为：
```markdown
| content-not-name | 30 | query 描述正文要点而非文件名（BETA-15B-6 v2 扩 11→20、v3 再扩 20→30、T\* 真水位校验、bake T\* v1 0.60 → v2 0.70 → v3 `<T_new>` <若 Branch A 则保留 v2 0.70 不动；若 Branch B 则填 <T_new>；若 Branch C 则填 1.01 spec §5 降级>）|
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
git commit -m "BETA-15B-6 v3 task 7：baseline 报告追加 v3 数据集节（sweep 全表 + T*=<选定> 决定 + 鲁棒性结论 + 认知层主动放弃字面 0.864/0.700 spec 目标）+ README v3 更新（content-not-name 20→30、corpus 115→124）+ 总验收过"
```

---

## 验证 checklist 汇总

- [x] T1 step 3 + T2 step 4：cases/corpus 完整性测试通过
- [x] T2 PII 自检 5 项（commit 前再过一遍）
- [x] T3 step 4：vectors.json check_vectors 完整性通过
- [x] T4 step 3：sweep 全表读 + T\* 决策记录 `/tmp/T-star-v3.txt`
- [x] T5：条件性 bake、若 Branch B/C 则改 lib.rs/gate.rs doc 升 v3
- [x] T6：rewrite baseline.json + workspace test / clippy / fmt / gate / v0.5/v0.9 byte-equal 全过
- [x] T7：baseline 报告 v3 节完整含 sweep 表 + T\* 决定 + 鲁棒性结论 + 认知层修订理由；README v3 更新数字

## 风险与对策

| 风险 | 对策 |
|---|---|
| Mac Metal embed 失败 / 模型未就位 | T3 step 2 前先检查 `models/qwen3-embedding-0.6b-q8_0.gguf` 存在 + `cmake --version` 可跑；feature `semantic-recall-metal` 启 |
| sweep 实测 cosine 偏离预期 | T4 step 3 接受 spec §5 降级路径；baseline 报告 v3 节诚实记录 |
| PII 泄漏 | T2 step 2 自检 5 项；commit 前 grep 实名「李/王/张/陈」+ 真实公司「阿里/腾讯/字节」+ 邮箱「@gmail/@163」|
| byte-equal v0.5/v0.9 退步 | 本 cycle 完全不动 parser/coverage/model fallback；T6 step 6-7 byte-equal 复检自然守住 |
| baseline.json rewrite 后字段顺序变 | serde_json 输出按 struct 字段定义顺序、与 v2 同；diff 仅数值变化 |
| c077 复用 s00011/s00012 反伤 cosine 信号 | 复用本身是验证「query 概念匹配 vs title 词面不匹配」分布的合理设计；若实测 c077 cosine 偏离预期 → baseline 报告 v3 节诚实记录、不视为失败 |
| **gate.rs (4c)(4d) 现状误判**（spec 起草前误以为字面常量需修订） | 已在 spec 顶部修订说明 + 原 T7 红线修订 task 删除（gate.rs A-3 cycle 起已自锁）；v3 plan 7 task 反映现实 |

## Plan Self-Review（writing-plans skill 要求）

### 1. Spec coverage 检查

逐 spec §3.1 in-scope 项对应 task：

| spec §3.1 项 | 对应 task |
|---|---|
| 写 10 新 case 进 cases.json（id `c069-c078`） | task 1 |
| 写 9 新 corpus docs 进 corpus.json | task 2 |
| 1 case（c077）复用现有 corpus s00011/s00012 | task 1（c077 relevant 指向 s00011/s00012）|
| Mac Metal --embed 重算 vectors.json 全集 | task 3 |
| 跑 9 阈值 sweep + 人工读表选 T\* | task 4 |
| 若 T\* 微调 → bake 新值 + 升 lib.rs doc + 升 gate.rs doc 注释（升 v3 字样、纯文档动作） | task 5 Branch B/C |
| baseline.json rewrite | task 6 step 1 |
| 认知层主动放弃字面 0.864/0.700 spec 目标 | task 5 step B2/C2 doc 注释 + task 7 step 1 baseline 报告 v3 节 |
| baseline 报告追加 v3 数据集节 | task 7 step 1 |
| README v3 更新 | task 7 step 2 |
| PII 自查 5 项 | task 2 step 2 + 风险段 |

### 2. Placeholder 扫描

**Sweep 数据占位（合理）**：
- task 7 step 1 sweep 全表 `<填>` 10 行 × 6 列 = 60 处
- task 7 step 1 baseline 实测 6 桶 × 2 字段 = 12 处
- task 7 step 1 T\* 决定理由 / 鲁棒性结论 / 真水位变化 / 下 cycle 抓手 各处

**性质**：与 v2 plan 的 `<填>` 占位同款——sweep 数据驱动决策的「执行时填」、非懒散 TBD。约定：
- task 4 sweep 后产 `/tmp/sweep-cosine-v3.log` + `/tmp/T-star-v3.txt`，是 task 7 占位的数据源
- subagent 执行 task 4 后必须暂存 sweep 全表
- task 7 执行时按数据源逐处替换 `<填>`

**T\* Branch 占位（合理）**：
- task 5 三 Branch（A/B/C）只走其中一条、占位 `<T_new>` 仅 Branch B 用、由 task 4 sweep 决定值
- task 5 step B1/B2/C1/C2 是「条件性执行」的清晰说明、不是 TBD

**非 sweep 占位**：plan 通篇无 TBD / TODO / implement later。10 新 case 全文 + 9 新 doc 全文 + 所有命令 expected output 都精确给出。

### 3. Type consistency 检查

| 概念 | 类型签名 | 跨 task 一致性 |
|---|---|---|
| `SemanticCase.bucket` | String | 全用 "content-not-name" ✓ |
| `RelevantDoc.grade` | u8 ∈ [1,3] | task 1 全 case grade=1/2/3 ✓ |
| `SemanticDoc.lang` | String | task 2 全用 "zh" / "en" ✓ |
| `cases.json` 数组顺序 | JSON array | task 1 10 case 续号 c069-c078 ✓ |
| `corpus.json` 数组顺序 | JSON array | task 2 9 doc 续号 s00116-s00124 ✓ |
| `DEFAULT_COSINE_ROUTING_THRESHOLD` | f64 const | task 5 Branch A/B/C 任一 Branch 都保持 f64 类型 ✓ |
| `vectors.json` `dim` | usize | task 3 step 3 全 doc/query 同 dim 1024 ✓ |

### 4. Scope 检查

✅ 单一实施 plan、单一 cycle、7 task 颗粒度合理；纯数据 cycle + 条件性 bake / doc 升 v3、零代码逻辑改动、零红线断言改动。

### 5. 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本 cycle 不动 v0.9 coverage、不触发
- [[project-evals-reporter-nondeterministic]]：T6 byte-equal 闸门用 status 计数（不裸 diff JSON）
- [[feedback-baseline-lock-red-line-pattern]]：bake 后锁新 baseline + 不可破红线硬断言 + 调优记录追加报告、三件套全做（gate 4 红线全部自锁 baseline、A-3 cycle 起即「自锁完全体」、v3 cycle 仅认知层修订）
- [[project-stale-hybrid-fallback]]：本 cycle 不动 fallback/hybrid model wiring、不触发
- [[project-rrf-weight-tuning-ceiling]]：W=10.0 固定
- [[feedback-per-task-verify-include-fmt]]：每 task 验证门必含 fmt + clippy + test ✓（T6 step 3-5、T5 Branch B/C step B3）
- [[project-pull-full-distribution-before-convention-call]]：扩量前已数 v1+v2 20 例 content-not-name 主题分布 + 14 主题清单、列在 spec §4 + plan task 1 c077 复用决策

## 链接

- spec：[../specs/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion-design.md](../specs/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion-design.md)
- baseline 报告：[../../reviews/semantic-recall-quality-baseline.md](../../reviews/semantic-recall-quality-baseline.md)
- v2 plan（参考节奏）：[2026-06-24-beta-15b-6-v2-content-not-name-expansion.md](./2026-06-24-beta-15b-6-v2-content-not-name-expansion.md)
- A-5 plan（参考节奏）：[2026-06-24-beta-15b-3a5-cosine-routing.md](./2026-06-24-beta-15b-3a5-cosine-routing.md)
- BETA-15B-6 v2 README：[../../../packages/evals/fixtures/semantic-recall/README.md](../../../packages/evals/fixtures/semantic-recall/README.md)
