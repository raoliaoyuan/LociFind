# BETA-36 设计：daemon 检索权限模型 + 归档集合（collection）模型

> 2026-07-03 spec。关键决策四问已用户确认（全采推荐），见 §8。
> ROADMAP 卡片：BETA-36（packages/locifind-server + apps/daemon，依赖 BETA-32，估时 1.5-2.5w）。
> 验收原文：① bearer token 升级为 per-collection / per-root 权限模型（常数时间比较沿用）；② collection 概念落地：root 分组、归档主体（案件/员工/审计项目）边界、显示名、只读态、审计标签——否则 ACL 只能按路径打补丁；③ audit 留痕含 subject（谁查了什么）；④ 越权访问返 403 + audit 记录，e2e 覆盖。

## 1. 背景与目标

BETA-32 daemon 是单根单钥：一个 `--root`、一个全权 bearer token、无审计。三企业场景（律所卷宗 / 审计取证 / 离职归档）的准入门槛恰恰是它没有的三样：**信息墙**（律师 A 不能搜到案件 B）、**留痕**（谁在什么时候搜了什么）、**归档主体边界**（按案件/员工/项目组织，不是按路径打补丁）。本卡把这三样落进 daemon。

## 2. 范围护栏（YAGNI）

- **不做**用户系统 / OIDC / LDAP——token 即身份（token→subject 映射），企业接 SSO 留后续。
- **不做** per-document ACL——粒度到 collection 为止。
- **不做** audit 的 UI / 报表——JSONL + admin tail 端点，分析经 MCP+LLM 组合（BETA-40 可写示例）。
- **不做**桌面 app 侧任何改动——纯 daemon 子线。
- **不动** BETA-27 byte-equal 相关路径（indexer 不改）。

## 3. 配置模型（Q1：TOML 声明式）

```toml
# locifindd.toml（含 token 明文——README 明示 chmod 600 / 仅 daemon 运行账户可读）
[[collections]]
id = "case-2026-blueharbor"          # 唯一 slug（[a-z0-9-]+）
display_name = "蓝湾贸易合同纠纷案"
subject_kind = "case"                 # case | employee | audit_project | other
roots = ["/archive/cases/blueharbor"] # 支持多 root 归组
read_only = true                      # 只读态（冷冻归档）：admin reindex 拒绝
audit_tags = ["lawfirm", "litigation"]

[[tokens]]
token = "…≥32 chars…"
subject = "zhang.san"                 # audit 留痕主体，必填
collections = ["case-2026-blueharbor"]  # 或 ["*"] 全权
admin = false                         # admin=true 才能调 /admin/*

[audit]
log_query = true                      # false 时 audit 只记 query 长度
```

**legacy 兼容**：仍以 `--root` + `--token` 启动时，自动合成 `id="default"` collection（读写）+ subject=`"default"`、`collections=["*"]`、`admin=true` 的 token——现有部署零迁移。TOML `[[collections]]` 与 `--root` 互斥（同时给报错，防两套语义混用）。

## 4. 权限模型（验收 ①④）

- `AuthCtx` 升级为 `tokens: Vec<TokenEntry { token: SecretString, subject: String, collections: CollectionGrant, admin: bool }>`，`CollectionGrant = All | Some(HashSet<id>)`。
- `require_bearer` 逐条 `subtle::ConstantTimeEq`（先长度门、沿用现逻辑）；命中后把 `Arc<AuthedPrincipal { subject, grant, admin }>` 塞进 request extensions。
- **HTTP→MCP 穿透**：rmcp 1.8 streamable-HTTP 会把 `http::request::Parts`（含我们塞的 extension）注入 MCP request extensions（`tower.rs:1089/1158/1246`），tool 侧 `RequestContext.extensions.get::<http::request::Parts>()` → `parts.extensions.get::<Arc<AuthedPrincipal>>()`。`Tool::invoke` 签名加 `principal: Arc<AuthedPrincipal>` 参数。
- **越权**：MCP `search` 请求了未授权 collection → tool-level error `access denied: collection '<id>'`（不泄漏该 collection 是否存在之外的信息）+ audit denied 记录；admin REST 端点无 admin 标志 → **403** + audit 记录（验收 ④ 的 403 落在 REST 层；MCP 协议内错误按 MCP 惯例走 tool error）。

## 5. collection 运行时（Q2：per-collection 独立 index.db）

- 布局：named collection → `data_dir/collections/<id>/index.db`；legacy default → 沿用 `data_dir/index.db`（零迁移）。
- `ServerCtx` 重构：`collections: HashMap<id, CollectionRuntime { meta, db_path, search_candidates }>`（每 collection 一份 `LocalIndexBackend` 候选链缓存，沿用 BETA-32 启动时构造一次的节奏）；`RuntimeState` 改 per-collection（indexed_at / doc_count / reindex_in_flight 按 collection 记）。
- **reindex**：`POST /admin/reindex?collection=<id>`（省略 = 全部非只读 collection 顺序跑）；`read_only=true` 的 collection 显式指名 reindex → **409**（冻结语义冲突，非鉴权问题）；并发同 collection → 409（沿用现互斥）。
- **物理信息墙**：搜索永远只 open 授权 collection 的 db 文件，越权没有可泄漏的查询面。

## 6. MCP 工具面（Q4：list_roots → list_collections）

- **search** 入参加 `collections?: string[]`（缺省 = token 授权的全部）；对每个目标 collection 跑 fallback chain → `merge_results` → `rank` → 截断 limit；hit 加 `collection: <id>` 字段。多库结果直接合并进同一 ranker（score 同源可比：同一套 FTS/rank 逻辑）。
- **list_collections**（替换 list_roots，pre-1.0 直接 breaking）：仅列当前 token 授权的 collection：`{ id, display_name, subject_kind, read_only, roots, doc_count, indexed_at, audit_tags }`。
- `get_info` instructions 同步更新（提示按 collection 检索与授权语义）。

## 7. audit 留痕（Q3：query 明文，可配置关；验收 ③）

- 新 `audit.rs`：`AuditSink` 追加写 `data_dir/audit.jsonl`（与桌面 BETA-06 audit.jsonl 同形态；`Mutex<File>` 串行 append + 每条 flush）。
- 每条：`{ ts, subject, action: search|list_collections|reindex|denied, collections: [...], query?: string, results?: n, denied_reason?: string }`。
- `[audit] log_query=false` 时 `query` 字段替换为 `query_len`。
- **两套规则各守其职**（README 明示）：ops tracing log 永不记 query（BETA-32 §6.2 不变）；audit.jsonl 是给取证/合规的专用留痕，默认记明文、属被检索数据同级的敏感资产。
- `GET /admin/audit?tail=N`（admin token）：返最近 N 条（默认 100，cap 1000），取证导出入口。

## 8. 关键决策(2026-07-03 用户确认，全采推荐）

| # | 问题 | 拍板 |
| --- | --- | --- |
| Q1 | 配置形态 | TOML `[[collections]]` + `[[tokens]]`；`--root/--token` 合成 default 全权，零迁移 |
| Q2 | 索引隔离 | per-collection 独立 index.db（物理信息墙）；legacy 沿用 data_dir/index.db |
| Q3 | audit 内容 | query 明文入 audit.jsonl，`[audit] log_query=false` 可降级；ops tracing 仍不记 query |
| Q4 | 工具面 | list_roots 改造为 list_collections（breaking OK）；search 加 collections 参数 |

## 9. cycle 划分

1. **cycle 1**：配置模型（TOML 解析 + 校验 + legacy 合成 + 互斥检查）+ `AuthedPrincipal` + auth.rs 多 token 匹配与 extension 注入 + 单测。
2. **cycle 2**：`ServerCtx` per-collection 重构（CollectionRuntime + per-collection RuntimeState + db 布局）+ reindex per collection + read_only/并发 409 + admin 标志门 /admin/*。
3. **cycle 3**：HTTP→MCP principal 穿透 + `Tool::invoke` 签名升级 + search 多 collection/越权 tool error/hit.collection + list_collections。
4. **cycle 4**：audit.rs（AuditSink JSONL + log_query 配置）+ search/denied/reindex/403 全埋点 + `GET /admin/audit`。
5. **cycle 5**：e2e 扩展（双 collection 双 token：信息墙隔离 / 越权 403+audit / denied tool error+audit / read_only 409）+ apps/daemon README + third-party-licenses（如新增依赖）+ 收工。

## 10. 验收对照

| 验收 | 落点 |
| --- | --- |
| ① per-collection token 权限、常数时间 | §4 AuthCtx 多 token + ct_eq 沿用 |
| ② collection 概念（分组/主体/显示名/只读/审计标签） | §3 配置模型 + §5 运行时 |
| ③ audit 含 subject | §7 每条记 subject（token→subject 映射） |
| ④ 越权 403 + audit + e2e | §4 越权双路径 + §7 denied 埋点 + cycle 5 e2e |
