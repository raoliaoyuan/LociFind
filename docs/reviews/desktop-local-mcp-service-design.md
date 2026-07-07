# 设计：桌面 App「本机 MCP 检索服务」开关（BETA-32 桌面变体）

> 状态：设计提案（2026-07-07，Claude Code）。待用户拍板范围后登 ROADMAP 实施。
> 触发：用户想让 Claude Code 这类 agent 方便地经 MCP 检索本机个人索引、查本机文件。

## 1. 目标与非目标

**用户故事**：我在桌面 App 工具菜单点一下「启用本机 MCP 服务」，Claude Code / Codex 等本机 agent 就能连上、用自然语言检索并读取我电脑里的文件——不用手动起 daemon、不用写 config。

**是**：BETA-32（headless 检索经 MCP 暴露）的**个人单机变体**——把桌面 App 已建的个人索引经 MCP 暴露给**本机** LLM 客户端。
**不是**：BETA-43。个人单库无多 collection / 多人 token / 信息墙，per-collection `allow_full_read` 闸门、跨主体审计报告等企业料在此用不到（read_document 片段/全文 + audit 留痕仍复用，见 §5）。

## 2. 现状与缺口

- **引擎已存在**：`locifindd`（legacy 单根模式 `--root/--token`）+ `packages/locifind-server`（axum router 工厂 + bearer auth + `search`/`read_document`/`list_collections` 三工具 + rmcp streamable-HTTP）。
- **缺口**：桌面 App（`apps/desktop`）对 daemon / MCP / server **零引用**；要手动起进程、写 config、填 token。

## 3. 关键决策：内嵌 server，而非 spawn daemon 子进程

| 方案 | 评价 |
|---|---|
| A. 桌面 spawn `locifindd.exe` 子进程 | 需随包带第二个 binary、管子进程生命周期、daemon 自建 + 重索引一份（与桌面 index.db 重复/冲突） |
| **B. 桌面进程内嵌 `packages/locifind-server` 的 axum router（推荐）** | 桌面本就是 Tauri（Rust + tokio），直接在 app 内起一个 axum server、**复用桌面已加载的 indexer + embedder + index.db**——零重索引、零第二 binary、**语义召回白送**（桌面 embedder 已加载，语义灯绿即证），实时与桌面索引一致 |

**推荐 B**。`packages/locifind-server` 是为复用而拆的 lib crate；主要集成工作 = 用桌面现成的 indexer/embedder 句柄构造 `ServerCtx`、把 router 挂到一个 `127.0.0.1` 的 tokio listener。

**索引复用（只读）**：daemon 场景 server 会"首次全量索引"；桌面变体**不索引**——索引仍由桌面后台调度负责，内嵌 server 只做**读**（search + read_document 都取自索引 db）。避免两处写同一 `index.db` 的 SQLite 并发写冲突。需确认 server 层能"只读复用现有 ServerCtx / index.db 而不触发自建索引"（主要实现改点）。

## 4. UI（工具菜单 / 选项）

- 选项对话框新增（或并入「杂项/高级」）一节「本机 MCP 服务」：
  - **开关**：启用 / 停用（默认**关**）。
  - **监听地址**：`127.0.0.1:<port>`（端口默认固定、可改；见 §5 绝不 `0.0.0.0`）。
  - **访问令牌**：随机生成、展示 + 一键复制 + 「重置令牌」。
  - **Claude Code 接入片段**：一键复制 `~/.claude/settings.json` 的 `mcpServers` 块（含 url + Bearer token），照 [daemon README §4](../../apps/daemon/README.md)。
  - **状态**：运行中 / 已停 / 端口占用；可选连接计数。
- 工具菜单加入口项（`open-prefs-...` 定位到该节，沿用 BETA-51 收编模式）。

## 5. 安全红线（不可省）

1. **只绑 `127.0.0.1`**——默认绝不 `0.0.0.0`/局域网。局域网暴露若要做，另开 opt-in + 强警告 + 强制非空 token（留 v2）。
2. **随机 token 必填**——本机任何持 token 的进程才能搜/读；token 可重置（重置即踢掉旧连接）。
3. **暴露面知情**：启用即"任何拿到 token 的本机进程可搜索并读取被索引文件内容"，UI 要一句人话讲清。
4. **read_document 全文策略**：个人单库场景默认可给 `allow_full_read=true`（自己的文件、自己的 agent），但要用户知情、可关（关→只回片段）。
5. **audit 留痕复用**：agent 的 search / read_document 落 `audit.jsonl`（谁的 agent、何时、搜了什么、读了哪个文件）——个人可追溯 agent 行为，价值正当；沿用 `[audit] log_query` 口径。

## 6. 分阶段实现建议

- **v1（MVP）**：内嵌 server + 工具菜单开关 + `127.0.0.1:port` + 随机 token + 配置片段一键复制 + 只读复用桌面索引（FTS + 桌面已加载的语义）。默认关。
- **v2**：`allow_full_read` UI 开关 + audit 查看/导出入口 + 端口自定义 + （强警告下）局域网暴露 opt-in + 连接状态细化。

## 7. 待用户/后续拍板

- **端口默认值**（如 `8765` 还是另选避开 daemon 惯用口）。
- **v1 是否带 audit 查看**，还是先只留后台写入。
- **只读复用 index.db 的具体改点**：server 层是否已支持"给定现有 ctx 不自建索引"，或需要新增一个"attach 只读 ctx"入口——实现前需在 `packages/locifind-server` 落一次确认。
- 登 ROADMAP 卡片 ID（建议归 B 阶段 BETA-32 后续或 V 阶段，取决于是否纳入本 beta 出场范围）。
