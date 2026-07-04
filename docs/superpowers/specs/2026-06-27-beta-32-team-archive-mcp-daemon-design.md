# BETA-32 团队归档 MCP daemon —— 设计 spec

> 日期：2026-06-27
> 状态：design（待用户复核 → writing-plans）
> 任务编号：BETA-32（待登记 ROADMAP §3.3 B6 同级）
> 作者：Claude Code (Opus 4.7)

## 0. 一句话定位

把 LociFind 已有的 hybrid 检索能力，包装成一个 **headless MCP daemon**，让在职同事的 Claude Code / Codex 通过内网 MCP 接口，用自然语言查找离职同事归档目录里的文件。

## 1. 背景

LociFind 当前是「本地优先、单用户桌面 app」，用户通过桌面 UI 用自然语言搜本机文件。BETA-15B-11-v2 已 bake 跨语言 embeddinggemma-300m、BETA-31 把 Windows 模型分发 UX 做完。

新场景：一位离职同事留下大量文档放在共享目录，其他同事希望通过他们本地的 AI Coding 工具（Claude Code / Codex），用自然语言（如「去年汇报过的友商竞争分析」「去年 3 月架构定义汇报材料」）按意思查找这些文档。

LociFind 的 hybrid（FTS5 + embedding）检索能力天然贴合此场景，但桌面 app 单用户单机的形态不直接适用。需要一个 **集中部署、内网可访问、面向 AI 工具的接口**。

## 2. 范围与边界

### 2.1 在范围内

- 新增 `packages/locifind-server`（lib crate）+ `apps/daemon`（binary `locifindd`）
- MCP server over **streamable-HTTP** transport（与 Claude Code / Codex 当前 stable transport 一致）
- 复用 `packages/search-engine` / `packages/model-runtime` / `packages/indexer` 作 library，daemon 与桌面 app 共享底层 core
- 仅支持「单一固定集合」：管理员一台 daemon 对应一棵目录树
- 鉴权：static bearer token（启动时 CLI 或 TOML 注入）
- 索引：启动时全量扫一遍 + `POST /admin/reindex` 手动触发；**不监听 fs 事件**（归档场景天然快照）
- 三端 headless binary：macOS（arm64 + x86_64）/ Windows x86_64 / Linux x86_64

### 2.2 不在范围内（明确砍掉）

- ❌ 多文档集合 / namespace（首版不引入 `collection` 概念，未来可后向兼容追加）
- ❌ 多租户 / per-user ACL（单租户 + 单 token，归档场景内网信任）
- ❌ fs notify 持续监控（手动 reindex 足够）
- ❌ 全文 chunk / snippet 返回（仅返路径 + 元数据，AI 自决是否 `Read`）
- ❌ daemon 侧 LLM 调用（LLM 在 client 侧，daemon 纯检索）
- ❌ TLS / mTLS（V2 优化）
- ❌ Web UI / 桌面 app 嵌入开关（Approach B/C 拍掉）
- ❌ `get_metadata` MCP tool（与 `Read` 工具职责重叠，且引入额外 path traversal 暴露面）

### 2.3 与 PROJECT.md 核心原则一致性

| 原则 | 是否冲突 | 备注 |
|---|---|---|
| 本地优先 | ✅ 不冲突 | 内网 daemon 仍是本地、数据不出公司网络 |
| 不做云端 AI 搜索 | ✅ 不冲突 | daemon 自身不调 LLM；LLM 在 client 侧 |
| 单用户桌面 app | ⚠️ 部分张力 | daemon 是「团队多人 → 单一索引」，但**仍是单租户单 token**，与"单用户"叙事并列、不替代主线 |
| 跨平台一致 | ✅ 不冲突 | 三端 headless |
| 后端可插拔 | ✅ 不冲突 | 同款 SearchBackend trait |

**结论**：不需修改 PROJECT.md 「不做什么」节。

## 3. 架构

```text
┌────────────────────────────────────────────────────────────────┐
│  在职同事本机                                                   │
│  ┌──────────────────┐                                          │
│  │ Claude Code /    │ MCP over streamable-HTTP                 │
│  │ Codex            │─────────┐                                │
│  │ (settings 配 URL)│         │                                │
│  └──────────────────┘         │                                │
└─────────────────────────────  │ ───────────────────────────────┘
                                │  局域网 / 公司内网
                                │  Authorization: Bearer <token>
                                ▼
┌────────────────────────────────────────────────────────────────┐
│  集中机器（Mac mini / NAS / Linux box，长跑 headless）          │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                  locifindd（new binary）                  │  │
│  │                                                          │  │
│  │  ┌──────────────┐   ┌──────────────────────────────┐    │  │
│  │  │ MCP server   │   │ Admin HTTP（/health,         │    │  │
│  │  │ (rmcp)       │   │ /admin/reindex, /metrics）   │    │  │
│  │  └──────┬───────┘   └──────┬───────────────────────┘    │  │
│  │         │                  │                            │  │
│  │         ▼                  ▼                            │  │
│  │  ┌──────────────────────────────────────────────────┐  │  │
│  │  │     packages/locifind-server（new crate）          │  │  │
│  │  │  ToolRegistry: search / list_roots                │  │  │
│  │  └──────┬───────────────────────────────────────────┘  │  │
│  │         │ 复用 library 调用                            │  │
│  │         ▼                                              │  │
│  │  ┌─────────────────┬──────────────┬─────────────────┐ │  │
│  │  │ search-engine   │ model-runtime│ indexer         │ │  │
│  │  │（hybrid 检索）  │（embed 模型） │（扫盘+chunk）   │ │  │
│  │  └─────────────────┴──────────────┴─────────────────┘ │  │
│  │         ▼                                              │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  SQLite + FTS5 + vector blob                    │  │  │
│  │  │  (--data-dir 指定，独立于桌面 app)              │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  --root /shared/departed-colleague-docs   ← 索引根目录          │
└────────────────────────────────────────────────────────────────┘
```

**关键划分**：
- 新代码：`packages/locifind-server`（lib）+ `apps/daemon`（binary `locifindd`）
- 复用作 library：`packages/search-engine` / `packages/model-runtime` / `packages/indexer`
- daemon SQLite 与桌面 app SQLite 完全独立，二者甚至可装在同台机器互不干扰

## 4. 组件

### 4.1 `packages/locifind-server`（新 lib crate）

```rust
// MCP tools 暴露给 Claude/Codex 的契约
pub trait Tool {
    fn name(&self) -> &str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(&self, args: Value, ctx: &ServerCtx) -> Result<Value>;
}

pub struct SearchTool { /* 调 search-engine */ }      // 主入口
pub struct ListRootsTool { /* 返回索引根目录 */ }     // 让 AI 知道服务边界

pub struct ServerCtx {
    pub root: PathBuf,                 // 索引根
    pub search: Arc<SearchEngine>,     // 复用 packages/search-engine
    pub embedder: Arc<dyn Embedder>,   // 复用 packages/model-runtime
    pub indexer: Arc<Indexer>,         // 复用 packages/indexer
    pub config: ServerConfig,
}

pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub bearer_token: SecretString,    // 用 secrecy crate
    pub data_dir: PathBuf,
    pub root: PathBuf,
    pub log_level: LevelFilter,
}
```

**MCP tools schema**（首版两个，全部 read-only）：

| Tool | 入参 | 出参 |
|---|---|---|
| `search` | `query: string`（自然语言）, `limit?: number=20`（hard cap 50）| `results: [{path, name, size, mtime, score, why?}]`，可选 `degraded: bool` |
| `list_roots` | 无 | `{root: string, doc_count: number, indexed_at: timestamp}` |

`why?` 字段复用 BETA-15B-5 段落高亮聚合摘要（如「跨语言命中：'竞争分析' ↔ 'competitive analysis'」），不返原文 snippet。

### 4.2 `apps/daemon`（新 binary `locifindd`）

```text
locifindd [OPTIONS]
  --root <PATH>          索引根目录（必填）
  --bind <ADDR>          监听地址（默认 0.0.0.0:8765）
  --token <STRING>       bearer token（必填，或 LOCIFINDD_TOKEN 环境变量）
  --data-dir <PATH>      索引 DB 目录（默认 ~/.locifindd/<root-hash>/）
  --model-path <PATH>    embedder GGUF 文件路径（必填，或 LOCIFINDD_MODEL_PATH 环境变量；
                         约定文件名 = 桌面 app 同款 DEFAULT_EMBEDDING_MODEL_FILENAME）
  --config <PATH>        TOML 配置文件（CLI 参数覆盖）
  --log-format <json|text>
  --log-level <error|warn|info|debug|trace>
```

启动流程 = 启 axum HTTP server + 跑 `Indexer::full_scan(root)` → 写 SQLite → 接受 MCP 请求。

### 4.3 现有 crate 改动

| crate | 当前问题 | 改动 |
|---|---|---|
| `packages/search-engine` | 部分 API 隐式假设桌面 app context | 抽 `SearchEngine::new(config: SearchConfig)` 构造器，让 daemon 也能用 |
| `packages/model-runtime` | 已 trait 化、改动小 | 确认 `Embedder` trait 暴露足够；daemon 走同款 embeddinggemma-300m |
| `packages/indexer` | 当前耦合 Tauri event emit | 抽 `IndexProgress` trait，桌面 app 走 Tauri event、daemon 走 tracing log |

**估改动量**：locifind-server ~1200 行 / apps/daemon ~400 行 / 现有 crate trait 抽离 ~400 行。总 ~2000 行。

## 5. 数据流

### 5.1 启动 + 索引

```text
$ locifindd --root /shared/departed-docs --token $TOKEN

[1] 解析 config（CLI > env > TOML 文件）
[2] 打开 SQLite at --data-dir/index.db
       └─ schema 版本不匹配 → 提示并退出（不自动 migrate）
[3] 启 indexer：
       ├─ 扫 root 目录树（同款 walker，忽略 .DS_Store / .git / node_modules 等）
       ├─ 按 mime 分流：text / pdf / office / image / audio
       ├─ 每文件 chunk → embed → 写 FTS5 行 + vector blob
       └─ tracing log 输出进度
[4] 写 indexed_at 时间戳到 SQLite metadata 表
[5] 启 axum HTTP server，绑定 --bind
       ├─ /mcp                ← MCP streamable-HTTP endpoint
       ├─ /health             ← 健康检查（无鉴权）
       ├─ /admin/reindex      ← 触发全量重建（bearer 鉴权）
       └─ /metrics            ← Prometheus 文本格式（可选 feature flag）
```

### 5.2 单次查询

```text
Claude Code 同事的电脑
  └─ tool_call: search(query="去年汇报过的友商竞争分析", limit=20)
       │
       ▼  POST /mcp  Authorization: Bearer <token>
locifindd 集中机器
  └─ MCP server 收 tool_call
       └─ 路由到 SearchTool::invoke
            │
            ▼
            packages/search-engine::SearchEngine::query
              ├─ planner: 自然语言 → SearchIntent JSON
              ├─ FTS5 召回（关键词 / 同义词族）
              ├─ embedding 召回（query embed → cosine top-k）
              ├─ hybrid 融合（RRF 权重 = 现行 baseline）
              ├─ result-normalizer 归一
              └─ ranker 排序（cosine_threshold = 0.70 桌面 app 现行值）
       └─ 输出 SearchResult vec
            │
            ▼  jsonify: 只取 path/name/size/mtime/score/why
            └─ MCP server 返 tool_result
  └─ Claude Code 收到 results
       └─ 决定是否 Read 某条命中文件继续推理
```

**关键决策**：daemon 检索路径完全复用桌面 app 现有 search-engine，行为一致。

### 5.3 索引重建

```text
管理员（一般也是部署 daemon 的人）
  └─ POST /admin/reindex Authorization: Bearer <token>
       └─ daemon 后台 spawn full_scan
            ├─ 不阻塞 MCP server（查询继续走旧索引）
            ├─ scan 写入临时 db 文件 <data-dir>/index.db.rebuild
            ├─ 完成后原子 swap：
            │     ① fs::rename(index.db, index.db.old)
            │     ② fs::rename(index.db.rebuild, index.db)
            │     ③ 新连接池打开 index.db；旧连接池 drain 后关闭
            │     ④ fs::remove_file(index.db.old)
            └─ 返回 {"status": "completed", "doc_count": N, "duration_ms": …}
```

**swap 中断恢复**：启动时若发现 `index.db.rebuild` 或 `index.db.old` 残留，提示并退出（要求 `--allow-rebuild-schema` 显式清理）。


### 5.4 部署形态示例

```text
管理员的 Mac mini（一直长跑，launchd）：

  $ launchctl load ~/Library/LaunchAgents/com.locifind.daemon.plist

  com.locifind.daemon.plist 关键字段：
    ProgramArguments = ["/usr/local/bin/locifindd",
                        "--config", "/etc/locifindd.toml"]
    RunAtLoad = true
    KeepAlive = true

  /etc/locifindd.toml:
    root      = "/Volumes/Shared/departed-colleague-docs"
    bind      = "192.168.1.50:8765"
    token     = "<from secrets>"
    data_dir  = "/var/lib/locifindd/index.db"

在职同事的 Claude Code settings.json：
  "mcpServers": {
    "locifind-archive": {
      "type": "http",
      "url": "http://192.168.1.50:8765/mcp",
      "headers": {
        "Authorization": "Bearer <token>"
      }
    }
  }
```

## 6. 错误处理

### 6.1 错误分层

| 层 | 典型错误 | 处理策略 |
|---|---|---|
| MCP transport | bearer token 缺失/错、SSE 断连、JSON-RPC 协议错 | HTTP 401 / 400；MCP error code 标准映射；不暴露内部栈 |
| Tool 入参校验 | query 空、limit 越界 | 返 MCP `InvalidParams` + 中文 message |
| 检索内部 | embedder 加载失败、SQLite IO 错、planner 解析失败 | 内部 `tracing::error!` + MCP `InternalError`；不抛细节给 client |
| 索引 | root 不存在、SQLite schema 不匹配、磁盘满 | 启动 fail-fast 退出（exit code 非 0、stderr 中文）；运行期 reindex 返 HTTP 5xx + JSON body |
| 管理接口 | token 错、并发 reindex 重叠 | 401 / 409 |

### 6.2 边界硬规则

- **path traversal 防御**：本版无 path 入参 tool；未来添加 path 入参的 tool 时必须 canonicalize + `starts_with(root)` 校验。
- **bearer token 常量时间比较**：用 `subtle::ConstantTimeEq`。
- **不在 log 里打 query 原文**（默认）：log 默认只打 `query_len=N tools=search results=M elapsed_ms=…`；要原文调试需 `--log-level=debug` 显式开。
- **`/admin/reindex` 重入保护**：BETA-31 同款 `AtomicBool IN_FLIGHT` guard，重复请求返 409。
- **不缓存 query → results**：避免 cache 跨用户串味。

### 6.3 启动 fail-fast 清单

启动按顺序 check，任何一条 fail 退出 + 中文错误说明：

1. `--root` 存在且可读
2. `--data-dir` 父目录可写
3. `--token` 非空且长度 ≥ 32
4. `--bind` 端口可绑
5. embedder model 文件存在（`--model-path` 或 `LOCIFINDD_MODEL_PATH` 指向同款 embeddinggemma-300m GGUF）
6. SQLite schema 版本与 daemon 编译时版本一致（不一致 → 提示 `--allow-rebuild-schema` 显式重建）

### 6.4 运行期降级

- **embedder 临时挂掉**（很少见）：search 退化为 FTS5-only，结果里加 `degraded: true` 字段；不直接 5xx
- **SQLite 临时锁**（reindex 中冲突）：retry 3 次每次 100ms backoff；仍失败返 503 + Retry-After

## 7. 测试

### 7.1 三层金字塔

| 层 | 范围 | 工具 | 目标 |
|---|---|---|---|
| 单元 | locifind-server crate 内部 | `cargo test` | 工具行为契约 / 错误码 / token 比较 |
| 集成 | daemon binary + 真索引 SQLite + **stub embedder** | `cargo test --test integration` + httptest fixture | 端到端 MCP 请求/响应、reindex 并发、auth 边界 |
| 评测 | daemon 接口在 v0.9 evals 上跑分 | 复用 `packages/evaluations` harness | 与桌面 app **top-K 集合等价** |

**关键决策**：集成测试用 stub embedder（CI 快）+ 评测层用真 embedder（确保行为对齐）。

### 7.2 单元测试

| 测试 | 断言 |
|---|---|
| `search_tool_basic_query` | 给 query "竞争分析"，返非空 results，path 全在 root 内 |
| `search_tool_limit_cap` | limit=1000 自动截到 50 |
| `list_roots_returns_indexed_at` | 返 indexed_at 与 SQLite metadata 表一致 |
| `auth_bearer_correct` | 正确 token → 200 |
| `auth_bearer_wrong` | 错 token → 401，**不泄露 token 长度差异**（`subtle::ConstantTimeEq`）|
| `auth_bearer_missing` | 无 Authorization header → 401 |
| `reindex_concurrent_409` | 第二次 reindex 在前一次未结束时 → 409 |
| `schema_version_mismatch_exits` | 启动 schema 不匹配 → exit code 非 0 + stderr 含中文 |
| `log_redacts_query` | info level 下 query 原文不出现在 log buffer |

### 7.3 集成测试（端到端）

新增 `apps/daemon/tests/e2e.rs`：

```rust
#[tokio::test]
async fn e2e_search_returns_results() {
    let fixture_dir = tempdir().unwrap();
    fixtures::copy_synthetic_corpus(&fixture_dir);

    let daemon = DaemonHandle::spawn(DaemonConfig {
        root: fixture_dir.path().to_path_buf(),
        bind: "127.0.0.1:0".parse().unwrap(),
        token: "test-token-32-chars-minimum-len".into(),
        data_dir: tempdir().unwrap().path().to_path_buf(),
    }).await;

    daemon.wait_indexed().await;

    let client = McpClient::connect(daemon.addr(), "test-token-32-chars-minimum-len").await;
    let results = client.call("search", json!({"query": "竞争分析"})).await.unwrap();

    assert!(!results["results"].as_array().unwrap().is_empty());
    for r in results["results"].as_array().unwrap() {
        assert!(r["path"].as_str().unwrap().starts_with(fixture_dir.path().to_str().unwrap()));
    }
}
```

主集成路径：
1. `e2e_search_returns_results`
2. `e2e_auth_401_wrong_token`
3. `e2e_reindex_409_when_in_flight`
4. `e2e_health_no_auth_required`
5. `e2e_list_roots_after_indexing`

### 7.4 评测层一致性闸门

daemon 跑 v0.9 evals harness 应当与桌面 app **top-K 集合等价**（同 root / 同模型 / 同 cosine_threshold）。

新增 evals 模式（evals harness 自起 daemon 进程、跑完关停，与现行桌面 app 模式同款一次性子进程节奏）：

```bash
cargo run -p evaluations -- run \
  --mode daemon \
  --daemon-binary target/release/locifindd \
  --root <evals-fixture-dir> \
  --model-path <embeddinggemma-gguf>
```

红线：v0.5 / v0.9 每条 case 的 top-20 path 集合等价桌面 app 模式；若不一致 → 闸门 fail。允许 ≤2 条偏差作为可接受 noise（评测固有 tie-breaker），详 §9 风险表。

**说明**：不要求严格 byte-equal（顺序可有 tie-breaker 微差），只要 top-20 path 集合一致。

### 7.5 不做的测试（明确边界）

- ❌ 100 并发压测：MVP 不针对高并发场景
- ❌ fuzz：MCP transport 自身已有 reference impl 覆盖
- ❌ 跨网络分区测试：内网部署单机器，不引入分布式失败模型
- ❌ 不测桌面 app 的现有能力（hybrid 召回 / parser / OCR）：BETA cycle 已锁

## 8. 项目登记与出场标准

### 8.1 ROADMAP 登记

**位置**：新增 **BETA-32 团队归档 daemon** 卡片，挂在 ROADMAP §3.3 B 阶段 B6 之后，与 BETA-29/30/31 同级。

**标注**：「与主线 BETA→1.0 并行的衍生子线、不阻塞 1.0 出场标准」。

### 8.2 出场红线（接受标准）

参照 BETA cycle 同款节奏：

1. **红线 1-9 全过**：
   - fmt 净 / clippy 0 warning
   - workspace test 0 failed
   - locifind-server 单元测试全过
   - apps/daemon 集成测试全过
   - 评测层 top-K 等价桌面 app
   - tsc 不涉及 / desktop build 不涉及 / fixture SHA 不涉及（这条 daemon 自身没有 fixture）
   - manual install 烟雾测：管理员一台机器起 daemon、健康检查通
2. **三平台 binary 产物**：Mac arm64 / Mac x86_64 / Windows x86_64 / Linux x86_64，由 GitHub Actions 产
3. **真机部署一次**（DEFERRED 用户自验、BETA-31 同款节奏）：管理员 Mac mini / Windows / Linux box 起 daemon、Claude Code 在另一台机器接、跑通 5 个 example query
4. **doc-sync**：
   - 新增 `apps/daemon/README.md`：部署 / 配置 / 故障排查
   - ROADMAP 加 BETA-32 卡片
   - STATUS 会话日志追加
   - `docs/third-party-licenses.md` 加新增依赖（rmcp / axum / secrecy / subtle / clap-based CLI 等）
   - **不动** PROJECT.md / CONVENTIONS.md（核心原则不变）

### 8.3 估时

| 阶段 | 估时 |
|---|---|
| C1 现有 crate trait 抽离（search-engine / indexer 解耦 Tauri 依赖）| ~2-3d |
| C2 locifind-server crate（tools + ctx + schema）| ~3-4d |
| C3 apps/daemon binary（CLI / config / lifecycle / axum 接线）| ~2-3d |
| C4 单元 + 集成测试 | ~2d |
| C5 评测层 daemon 模式 + 等价闸门 | ~1-2d |
| C6 三平台 binary CI workflow（.github/workflows/release-daemon.yml）| ~1d |
| C7 doc-sync + PR | ~0.5d |
| **总计** | **~12-16d（~2-3w）** |

## 9. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| MCP streamable-HTTP transport 规范变动 | 中 | 中 | 用 rmcp / official SDK；锁版本；预留 transport 抽象 |
| 现有 search-engine / indexer 解耦 Tauri 依赖比预期难 | 中 | 中 | C1 提前 spike；超出 1w 重评估范围 |
| 真机部署失败（管理员配 launchd / Windows service 出错）| 中 | 低 | README 详尽样板 + 5 个常见故障排查 |
| 评测 top-K 等价闸门一直差几条 | 中 | 低 | 允许 ≤2 条偏差作为可接受 noise（评测固有 tie-breaker）|
| 大目录（10w+ 文件）首次全量索引很慢 | 高 | 低 | 接受、tracing log 透传进度；MVP 不优化 |
| 用户拿这个去做团队多人 ACL（超出范围）| 低 | 中 | README 明确"单租户内网信任、不做 per-user ACL"，超出场景请用主线桌面 app |

## 10. 未来扩展（不在本 cycle）

- **V2 mTLS**：跨网段部署时升级 TLS
- **V2 多 collection**：每个离职同事 / 项目独立 namespace（接口加 `collection?` 参数后向兼容）
- **V2 fs notify**：归档目录如果会变（罕见），加 notify watcher
- **V2 全文 chunk 接口**：如果 AI 不想 Read 大文件想要 daemon 侧 chunk 后投递
- **V3 daemon 内嵌 LLM 重写 query / rerank**：纯检索不够好时再加

---

> 写完待用户复核 → writing-plans
