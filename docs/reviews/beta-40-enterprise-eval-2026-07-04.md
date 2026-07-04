# 企业三场景 daemon 评测自动化（enterprise eval）落地报告

> 日期：2026-07-04
> 执行者：Claude Code
> 承接：STATUS 下一步 ①（[beta-40-ingest-semantic-gap-fix-2026-07-04.md](./beta-40-ingest-semantic-gap-fix-2026-07-04.md) 修复后的可重复回归缺口）。

## 1. 交付物

| 组件 | 位置 | 说明 |
|---|---|---|
| 评测纯逻辑模块 | `packages/evals/src/enterprise.rs` | queries.tsv 解析 / top-K 命中评分 / 越权评分 / daemon TOML config 生成 / 报告渲染；全部单测覆盖、CI 常跑 |
| 评测 CLI | `packages/evals/src/bin/enterprise_scenarios.rs` | 生成合规 config → 拉起真 `locifindd` → 逐 subject token 走 MCP `search` → Markdown + JSON 报告；`--require-all` / `--min-overall-pass` 闸门 |
| 回归门测试 | `packages/evals/tests/enterprise_scenarios_gate.rs` | 3 条 fixture 完整性（常跑 CI：TSV 合法、期望路径存在、期望内容落在 subject 授权集合内）+ 1 条环境变量门控端到端（`--require-all`） |
| daemon 权重旋钮 | `locifindd --semantic-weight <f>` | `ServerConfig.semantic_weight` 贯通 `SearchTool` RRF 融合；缺省镜像桌面 `DEFAULT_SEMANTIC_WEIGHT`，评测 A/B 用 |
| runner 扩展 | `runner_daemon::DaemonRunner::spawn_with_config` | collection 模式 spawn（`--config` TOML、per-subject token） |

评测语料 = [test-materials/enterprise-scenarios-raw](../../test-materials/enterprise-scenarios-raw/)（BETA-41 扩展材料），
query 期望集 = [expected/queries.tsv](../../test-materials/enterprise-scenarios-raw/expected/queries.tsv)（21 case：
律所 7 / 审计 6 / 离职 8，含 3 条 `ACCESS_DENIED` 信息墙负样本）。

## 2. 评测语义

- **正样本**：以该 subject 的 token 缺省 `search`（top-K=10），`expected_paths` 全部进 top-K 才 pass；报告记逐路径排名。
- **越权负样本**（双断言）：① 缺省检索的全部命中 collection 必须 ⊆ 该 subject 授权集（物理信息墙端到端验证）；② 对每个未授权 collection 显式指名 `search` 必须返回 tool error（`isError`）。
- config 由 runner 按 materials root **运行时生成**（绝对路径 roots + ≥32 字符 token）；仓库内示例
  [locifindd-enterprise-test.toml](../../test-materials/enterprise-scenarios-raw/configs/locifindd-enterprise-test.toml)
  原 token 全部 <32 字符、照抄会被 daemon `MIN_TOKEN_LEN` 校验拒启——本次已顺手补齐到合规长度。

## 3. 结果

环境：Windows 11 本机、真实模型 `embeddinggemma-300m-q8_0.gguf`（CPU）、locifindd dev build（`locifind-model-runtime/llama-cpp`）。

### 3.1 首轮（默认权重）：20/21，唯一失败暴露真实覆盖缺口

O-04（`交接时给的数据库账号权限清单` → `database-account-permissions.csv`）miss。
根因：**csv/tsv 不在 indexer `DOC_EXTS` 白名单**——文件从未入索引，任何检索臂都不可达（结构性 miss，非排名问题）。
企业归档场景权限清单 / 台账导出常为 csv，属实际覆盖缺口。

**修复**：`DOC_EXTS` 增补 `csv`/`tsv`、按纯文本提取（`doc_extract.rs` 路由到 `extract_txt`，`MAX_BODY_CHARS` 截断兜住大文件）；
新增单测 `extract_csv_tsv_as_plain_text`。桌面端共用同一 indexer、自动获得该覆盖。

### 3.2 复测（修复后）：21/21 全过

| scenario | passed | total |
|---|---|---|
| lawfirm | 7 | 7 |
| audit | 6 | 6 |
| offboarding | 8 | 8 |
| **OVERALL** | **21** | **21** |

- 18 条正样本中 15 条期望路径命中第 1 位；L-01 判决书 @3、O-04 csv @3、L-06/O-08 近重复对 @1+@2。
- 3 条越权负样本（L-07 / A-06 / O-05）双断言全过：缺省检索零跨集合泄漏 + 显式越权全部被拒。
- 全程 `degraded=false`。

### 3.3 语义权重 A/B：维持默认，不需要 daemon 侧独立调低

7-04 修复报告遗留问题 2（`DEFAULT_SEMANTIC_WEIGHT=10` 下 FTS 字面命中 score 明显低于语义臂，是否要 daemon 独立调低）。
用 `--semantic-weight 3` 对照跑：**21 case 逐条排名与权重 10 完全一致**（含 FTS 强项的 L-06/O-08 近重复对与 A-05 英文 query）。
在当前语料规模（7 集合 / 每集合 ≤15 文档）下融合权重不是可观测变量。**结论：维持默认权重；该问题降级为"待真实规模语料再看"，不再是待办**。

## 4. 复现方式

```text
# 1) 编 daemon（需 llama-cpp feature；Windows 本机配方见 docs/windows-setup.md §模型侧）
cargo build -p locifindd --features locifind-model-runtime/llama-cpp

# 2) 跑评测（严格闸门）
cargo run -p locifind-evals --bin enterprise_scenarios -- \
    --daemon-binary target/debug/locifindd.exe \
    --model-path <embedder GGUF> \
    --require-all [--semantic-weight <f>] [--json report.json]

# 3) 或走测试入口（无环境变量时自动 skip，有则端到端 + 严格闸门）
LOCIFIND_DAEMON_BIN=... LOCIFIND_MODEL_PATH=... \
    cargo test -p locifind-evals --test enterprise_scenarios_gate
```

单轮耗时 ≈3-4 分钟（CPU 嵌入 7 集合 + 21 case × [1 缺省 + 负样本 6 探针] 查询）。

## 5. 2 字 CJK 泛词现状评估（STATUS 下一步 ③）

代码链路核实：FTS 臂的**两条**查询路径都受 trigram 结构性限制——
① `fts_match_from_groups`（组词 MATCH）主动剔除 <3 字纯 CJK 词项（BETA-42 修的正是"不拖垮 AND"）；
② 兜底 `build_doc_query` 把 text 包成单 phrase MATCH，2 字 CJK phrase 同样生成不出可匹配 trigram token。
即：**纯 2 字 CJK 查询在 FTS 臂结构性 0 命中，语义臂是唯一兜底**。

本评测集 21 条 query 未暴露该问题（query 都长于 2 字、语义兜底有效）。真正不可达的组合是
**「图片 OCR 内容 + 纯 2 字词」**（图片默认不入语义索引，BETA-39 opt-in）。两个候选修法待拍板：

- (a) 企业场景（daemon）默认开图片语义（BETA-39 门槛护栏已就位，风险=OCR 乱码污染已有双层门槛挡）；
- (b) 2 字纯 CJK 词 FTS 走 `LIKE '%词%'` 兜底（新 DocumentQuery 分支，冷归档规模全表扫可接受，需 ESCAPE 处理）。

建议：daemon 侧先做 (a)（一行配置级改动、护栏现成）；(b) 作为桌面通用能力另立卡评估。

## 6. 遗留

1. BETA-40 验收第二条严格口径（用户真实内网/归档目录证据）仍待用户环境——本报告为合成材料证据，不替代。
2. 评测闸门已启用 `--require-all`；fixture 扩容（`.msg` 样本、更多负样本）后 baseline 随 TSV 自然生长。
3. 桌面 UI 消费 `extraction_failures()`（7-04 遗留 4）未动。
4. csv/tsv 入 `DOC_EXTS` 同时作用于桌面端（用户文档目录的 csv 将进索引）；体量由 `MAX_BODY_CHARS` 截断兜底，如真机反馈索引膨胀再评估按大小跳过。

## 7. 追加（同日）：daemon 默认开图片语义 + 语义臂 MediaSearch 空洞修复

§5 建议 (a) 用户拍板执行。落地三件事：

1. **daemon 图片语义默认开**：`ServerConfig.embed_images`（daemon 默认 true、桌面维持 BETA-39
   opt-in 默认关，两侧策略独立）；`locifindd --disable-image-semantics` 逃生舱；首次索引
   embed 前按开关跑 `purge_short_body_vectors`——镜像桌面启动期语义（关 → 清全部图片向量
   回到一刀切态）；reindex 路径同步透传。BETA-39 双层质量门槛原样生效。
2. **评测集扩到 22 case**：新增 O-09（`鲲鹏值班看板的截图` → offboarding attachments 下的
   PNG，OCR 文本全为 2 字 CJK 词——FTS 结构性不可达、语义唯一兜底的靶子）。
3. **O-09 首跑 miss 暴露语义臂真空洞**：诊断（真 daemon 手工 MCP 查询）确认图片已入索引且
   `鲲鹏`（纯 2 字词）语义顶位命中，但完整问法返回**零结果**——`截图/照片` 类词被 parser 路由成
   `MediaSearch(Image|Screenshot)`，而 `SemanticIndexBackend` 只接 `FileSearch`：图片语义索引
   对最自然的图片问法反而失效。**修复**：语义臂 `query_spec` 扩展接受图片类 MediaSearch，
   候选按代表 path 扩展名 ∈ `IMAGE_EXTS` 过滤（尊重 intent 类型语义）；音频/视频类维持不接。
   桌面共用该后端——opt-in 关闭时图片向量为空、行为零变化。

**复测：22/22 全过（`--require-all`）**，O-09 顶位命中，其余 21 case 排名与 §3.2 逐条一致（零回归）。
新增单测：`media_image_intent_served_and_filtered_to_images` / `query_spec_and_image_path_helpers`
（semantic-index，stub OCR + stub embedder，CI 常跑）。

**2 字 CJK 泛词（§5）就此收尾**：daemon 侧语义兜底对文本与图片内容均已生效；LIKE 兜底
（候选修法 (b)，桌面通用能力）不再紧迫，需要时另立卡。
