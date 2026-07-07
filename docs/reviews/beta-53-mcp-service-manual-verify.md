# BETA-53 桌面「本机 MCP 服务」真机验证 playbook

> 目标：验证桌面开关启用后，本机 Claude Code / Codex 能经 MCP 检索并读取桌面已索引的文件。
> 对应设计：[desktop-local-mcp-service-design.md](./desktop-local-mcp-service-design.md)（S1-S3 code-done）。
> 离线已验：server lib 93 / desktop 174 / clippy·fmt·tsc+vite 全绿 + `serve_bound` 真 socket 起停集成测试。
> 本 playbook 覆盖离线测不到的部分：GUI 开关流、真实端口绑定、Claude Code 实连 round-trip、语义命中。

数据目录（下称 `<DATA>`）：Windows = `%APPDATA%\LociFind\`（即 `dirs::data_dir()\LociFind`）；
macOS = `~/Library/Application Support/LociFind/`。索引库 `<DATA>\index.db`、审计 `<DATA>\audit.jsonl`、
模型 `<DATA>\models\`。

---

## 0. 前置

1. **已有索引**：先在「选项 → 索引」加至少一个目录并完成一次索引（`<DATA>\index.db` 有内容）。
   否则服务能起但检索为空。
2. **Claude Code 已装**，且能编辑 `~/.claude/settings.json`。
3. **选一条构建路径**（二选一）：

   | 路径 | 命令 | 验到什么 | 需要 llama 构建环境？ |
   |---|---|---|---|
   | **A（快速，先跑这条）** | 仓库根 `apps/desktop` 下 `npm install` 后 `npm run tauri dev` | 全部管道：开关 / 端口 / token / Claude Code 实连 / `search`(FTS 命中) / `read_document` / 重置 | **否**（无 feature = 语义臂降级 FTS-only，管道完整） |
   | **B（完整，验语义命中）** | `npm run tauri build -- --features model-fallback,semantic-recall -- --locked` | A 的全部 + **语义/跨语言命中** | **是** |

   - **路径 B 的构建环境**与 daemon 带 llama 构建同款（VS 2022 Build Tools + libclang）——见
     [apps/daemon/README §2.5](../../apps/daemon/README.md) 与 [scripts/build-locifindd-llama.bat](../../scripts/build-locifindd-llama.bat) 头注。
   - **路径 B 需放置 embedding 模型**：`embeddinggemma-300m-q8_0.gguf` 放 `<DATA>\models\`
     （或应用内「快速入门 / 选项 → 语义召回」一键下载）；「选项 → 语义召回」显示 Ready 即就绪。

> 建议：先跑**路径 A** 把管道走通（快、无重型环境），再按需用**路径 B** 补语义命中一项。

---

## 1. UI 开关与状态

| # | 操作 | 预期 |
|---|---|---|
| 1.1 | 菜单「工具 → 本机 MCP 服务...」 | 打开选项对话框并定位到第八 tab「本机 MCP 服务」 |
| 1.2 | 初始态 | 开关**未勾选**、状态「已停止」、无 token 区、无配置片段 |
| 1.3 | 勾选「启用本机 MCP 服务」 | 状态转绿「✓ 运行中 · 监听 127.0.0.1:8766 · 已挂载 N 条索引」；路径 A 尾带「· 仅全文（未启用语义召回）」，路径 B 带「· 含语义召回」 |
| 1.4 | token 区 | 出现 64 位十六进制 token + 「复制」按钮（点击变「已复制」） |
| 1.5 | 配置片段区 | 显示 `mcpServers.locifind-local` JSON（含 `url: http://127.0.0.1:8766/mcp` + `Bearer <token>`）+ 「复制」 |
| 1.6 | 取消勾选 | 状态转「已停止」、token/片段区消失 |
| 1.7 | 再次启用后**关闭并重开 app** | app 启动即自动拉起（第八 tab 状态仍「运行中」）——验证 enabled 持久化 + 自启 |

**通过判据**：1.1–1.7 全部符合预期。

---

## 2. 端口绑定与鉴权（安全红线）

服务运行中，命令行执行：

```powershell
# 1) 只绑回环、绝不 0.0.0.0：应看到 127.0.0.1:8766 LISTENING（不是 0.0.0.0:8766）
netstat -ano | findstr 8766

# 2) /health 无鉴权 → 200
curl -si http://127.0.0.1:8766/health

# 3) /mcp 无 token → 401（鉴权确实生效）
curl -si -X POST http://127.0.0.1:8766/mcp -H "content-type: application/json" -d "{}"
```

**通过判据**：① 监听地址是 `127.0.0.1:8766`（非 `0.0.0.0`）；② `/health` 返 `200`；③ 无 token 的 `/mcp` 返 `401`。

---

## 3. Claude Code 实连 round-trip（核心）

1. 在 UI 点「复制」拿到配置片段，粘进 `~/.claude/settings.json` 的 `mcpServers`（已有该键则合并；片段里的
   server 名为 `locifind-local`）。
2. 重启 Claude Code（或重载 MCP）。执行 `/mcp` 或让其列出工具——应发现 `locifind-local` 的三个工具：
   `search`、`read_document`、`list_collections`。
3. **list_collections**：应返回一个集合（id=`default`、显示名「本机文件」、`roots` 为你配置的索引目录、
   `doc_count` 与 UI「已挂载 N 条」一致、`allow_full_read=true`）。
4. **search**：让 Claude「用自然语言在我电脑里找 <某个你知道内容的文件主题>」——应返回命中，每条带
   `snippet` 出处片段（扫描件另带命中页号）。
5. **read_document**：让 Claude 读某条命中的文档——应返回片段（或全文，个人变体默认放开）。
6. **语义命中（仅路径 B）**：用「换个说法 / 跨中英文」的 query（非文件名精确词），验证仍能命中——
   证明语义臂真参与（路径 A 此步会退化为纯关键词，可跳过）。

**通过判据**：工具列表出现三工具；`search` 返回真实命中含出处；`read_document` 能读到内容；
路径 B 下语义 query 也命中。

---

## 4. 令牌重置（踢连接）

1. 保持 Claude Code 已连、能 search。
2. UI 点「重置令牌」→ 状态应变为「已停止」、token 变为新值、开关回到未勾选。
3. 用**旧配置**的 Claude Code 再检索 → 应失败（401 / 连接不可用）。
4. UI 重新启用 → 复制**新**片段替换 `settings.json` → 重连 → 恢复可用。

**通过判据**：重置后旧 token 失效（踢掉旧连接）、新 token 重连成功。

---

## 5. 审计留痕（可选）

检索若干次后查 `<DATA>\audit.jsonl`——应有 `search` / `read` 动作记录（含时间、query、命中数、读取路径）。

---

## 结果记录模板

| 分节 | 结果 | 备注（版本 / 构建路径 A或B / 异常） |
|---|---|---|
| 1 UI 开关与状态（1.1–1.7） | ☐ 通过 / ☐ 失败 | |
| 2 端口绑定与鉴权 | ☐ 通过 / ☐ 失败 | |
| 3 Claude Code round-trip | ☐ 通过 / ☐ 失败 | |
| 4 令牌重置 | ☐ 通过 / ☐ 失败 | |
| 5 审计留痕（可选） | ☐ 通过 / ☐ N/A | |

- 平台 / 版本：
- 构建路径：A（FTS-only dev）/ B（semantic-recall）
- 结论：
- 沉淀：真机通过后在 STATUS 会话日志记一条、ROADMAP BETA-53 由 code-done 转 done。

---

## 常见问题

- **启用后状态一直「处理中」/ 报错端口被占用**：`netstat -ano | findstr 8766` 看是否已有进程占 8766
  （可能上一次实例未退或另一 app）。单实例锁通常已防同 app 二次启动。
- **Claude Code 连不上 / 401**：确认 `Authorization` 是 `Bearer <token>`（带前缀+空格）、token 与 UI 当前一致
  （重置过就要用新片段）、url 末尾是 `/mcp`。
- **search 返回空**：先确认「选项 → 索引」已索引目录且 `index.db` 有内容；UI 状态里「已挂载 N 条」应 > 0。
- **路径 A 语义 query 不命中**：预期——路径 A 无 `semantic-recall` feature，语义臂降级 FTS-only；语义命中需路径 B。
