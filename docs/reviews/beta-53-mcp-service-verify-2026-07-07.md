# BETA-53 本机 MCP 服务 —— 功能级 round-trip 验证（2026-07-07）

> 依据 [验证 playbook](./beta-53-mcp-service-manual-verify.md)。
> 方式：自动化 harness 驱动**真实的 `McpServiceState`**（生产对象，即 Tauri 命令背后的同一逻辑），
> 对着**真实桌面 index.db 的临时拷贝**（`%APPDATA%\LociFind\index.db`，86 篇真实文档；拷贝到临时目录、
> 不改动原库）跑完整 MCP round-trip；MCP 客户端用 reqwest 复刻 stateless streamable-HTTP 协议
> （等价于 Claude Code 的连接行为）。构建路径 A（无 `semantic-recall` feature → FTS-only）。
> harness 为用完即删（未入库；query / 文档路径经环境变量注入，仓库不留私有数据）。

## 结果

| playbook 分节 | 项 | 结果 | 证据 |
|---|---|---|---|
| §1 | `start()` 起服务 | ✅ | `running=true` · `addr=127.0.0.1:8766` · `doc_count=86` · `semantic=false`（FTS-only，路径 A 符合预期） |
| §2 | 只绑回环 | ✅ | 监听 `127.0.0.1:8766`（非 `0.0.0.0`） |
| §2 | `/health` 无鉴权 | ✅ | `200` |
| §2 | `/mcp` 无 token | ✅ | `401` |
| §2 | `/mcp` 错 token | ✅ | `401` |
| §3 | `tools/list` | ✅ | 含 `search` / `read_document` / `list_collections` 三工具 |
| §3 | `list_collections` | ✅ | 返回 `default` 集合 |
| §3 | `search`（真实 query） | ✅ | `200`、命中一篇已索引的英文 PDF（返回 `collection` / `name` / `path` / 出处），无 error |
| §3 | `read_document`（命中 path） | ✅ | `200`、`mode=snippets` 返回片段内容（1553 字节），无 `access denied` |
| §4 | `reset_token()` | ✅ | `running=false`（停服务）+ token 已轮换 |

**结论**：桌面本机 MCP 服务的**运行时功能全链路通过**——起停、只绑回环、token 鉴权（无/错 token 皆拒）、
三工具经 MCP 暴露、真实索引上的 `search` 命中与 `read_document` 读取、重置令牌停服务并轮换。
covered 了 playbook §2 / §3 / §4 的功能实质。

## 追加：真机 GUI 全流程验证（2026-07-07 同日，computer-use 驱动 dev app）

`npm run tauri dev` 起带 BETA-53 的 dev app（`semantic_recall_feature=false`＝路径 A），computer-use 实点验证。
遇到环境障碍：LociFind 主窗口默认隐藏、经全局快捷键唤起，而默认 `Ctrl+Space` 被搜狗输入法拦截（[已知坑](../../CLAUDE.md)），
临时把 dev 的 `global_shortcut` 改 `Ctrl+Alt+Space` 唤起窗口（**验后已复原为 `Ctrl+Space`**）。

| 项 | 结果 |
|---|---|
| 启动自启（`mcp_service_enabled=true` 持久化 → 启动即拉起） | ✅ 日志「本机 MCP 服务已按上次开关态自动启动」、起即监听 8766 |
| 工具菜单「本机 MCP 服务...」→ 选项对话框定位第八 tab | ✅ 路由正确 |
| 开关态 + 状态行 | ✅「✓ 运行中 · 监听 127.0.0.1:8766 · 已挂载 86 条索引 · 仅全文（未启用语义召回）」与后端一致 |
| token 展示 + 配置片段复制 | ✅ 剪贴板实读 = 合法片段（`url:http://127.0.0.1:8766/mcp` + `Bearer <token>`） |
| GUI 开关 **OFF** | ✅ 8766 停止监听 + `mcp_service_enabled=false` 持久化 |
| GUI 开关 **ON** | ✅ 8766 重新监听 + `mcp_service_enabled=true` 持久化 |
| 对**实跑 app** curl round-trip | ✅ /health 200 · 无 token 401 · tools/list 三工具 · search 命中真实 PDF · read_document 1548 字节片段（mode=snippets、无越权） |
| 旧设置迁移 | ✅ v0.9.19 写的 settings.json（无 `mcp_service_*` 字段）被新 dev app 正常读取（serde default） |

**结论**：真机 dev app 上 GUI 全流程 + 开关联动后端起停 + 复制 + 自启 + 旧设置迁移**全部通过**。

## 追加：路径 B 语义召回验证（2026-07-07 同日，`semantic-recall` feature 构建）

`cargo test -p locifind-desktop --bins --features semantic-recall`（llama 环境 = vcvars + 入仓
libclang + llcb 缓存复用）跑自足 headless harness：临时建 2 篇英文小文档 → embed pass 填
`document_vectors` → 真实 `McpServiceState` + 真 embedder（embeddinggemma-300m）起服务 → 经 MCP 查询。

| 项 | 结果 |
|---|---|
| 带 feature 后 embedder 就绪 | ✅ `is_ready()=true`；embed pass 2 篇成功（embedded=2/failed=0） |
| 语义臂激活 | ✅ `start()` 返回 **`semantic=Some(true)`**（路径 A 为 false，此处真 embedder → true） |
| **跨语言语义命中** | ✅ 中文 query「财务预算规划」经 MCP `search` 命中**英文**财务文档 `note-finance.txt`（score 0.16）——中文 query vs 英文正文无共享字符，FTS 结构不可达、纯语义召回 |

**结论**：带 `semantic-recall` 构建的桌面本机 MCP 服务，语义臂正确激活并经 MCP 提供**跨语言按意思召回**。

## 尚未覆盖（唯一剩项，可选）

- **真 Claude Code 进程客户端**：本轮全程以 reqwest/curl 充当 MCP 客户端（stateless streamable-HTTP
  协议同一、全通过）；用真实 Claude Code 进程连 `~/.claude/settings.json` 走一遍待用户可选复核
  （未擅改用户 Claude 配置）。协议层已验，属锦上添花。

综上，BETA-53 的**功能 / GUI / 语义**三维真机验证均已通过，仅余「真 Claude Code 进程实连」这一可选复核项，
ROADMAP BETA-53 可视作真机验证达成、转 done。
