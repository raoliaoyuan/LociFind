# 企业三场景 semantic daemon 测试报告

> 日期：2026-07-03  
> 执行者：Codex  
> 范围：律师事务所、内部审计、离职员工三类企业归档场景  
> 目标：验证真实模型 + semantic daemon + MCP 检索主链路是否可用，并记录真实格式材料索引现状。

## 1. 测试对象

- 仓库：`D:\Git\Locifind`
- daemon：`target/debug/locifindd.exe`
- 测试材料：`test-materials/enterprise-scenarios-raw/`
- 真实格式材料：`test-materials/enterprise-scenarios-raw/real-formats/`
- collection 配置：`.tmp/enterprise-smoke/locifindd-smoke.toml`
- 真实模型：`C:\Users\Alice\AppData\Roaming\LociFind\models\embeddinggemma-300m-q8_0.gguf`
- LLVM/Clang：`.tmp/LLVM-20.1.8-extracted/bin`

## 2. 环境修复记录

### 2.1 LLVM/Clang 19+ 安装

语义后端 `llama-cpp` 构建需要 Clang 19+ 的 `libclang.dll`。

已完成：

- 下载并补全 `LLVM-20.1.8-win64.exe`。
- 使用 7-Zip 解包到 `.tmp/LLVM-20.1.8-extracted`。
- 验证 `clang.exe --version` 为 `20.1.8`。
- 验证 `libclang.dll` 位于 `.tmp/LLVM-20.1.8-extracted/bin/libclang.dll`。

此前失败原因：

- 第一次下载超时，安装包只有约 176 MiB，7-Zip 报 `Unexpected end of archive`。
- 本机 Anaconda 自带 `libclang.dll` 版本过旧，MSVC 14.44 STL 要求 Clang 19+。
- CMake/Ninja 已随 Visual Studio BuildTools 安装，但不在 PATH，测试时临时加入 PATH。

### 2.2 代码侧修正

为了跑通 Windows semantic daemon，做了两处修正：

- `packages/model-runtime/Cargo.toml`
  - `llama-cpp` feature 增加 `llama-cpp-4/mtmd`。
  - 原因：`llama-cpp-4` 默认去掉 default-features 后，`LLAMA_BUILD_COMMON=OFF`，Windows 链接会缺 `common_*` 符号。

- `packages/evals/src/mcp_client.rs`
  - evals daemon-mode helper 改为适配当前 daemon 的 stateless MCP JSON framing。
  - 原因：daemon e2e 已按 BETA-36 改为无需 `initialize` / `mcp-session-id`，但 evals helper 仍按旧 stateful 协议，导致 `MCP initialize 响应缺 mcp-session-id header`。

## 3. 自动化测试结果

### 3.1 semantic daemon 构建

命令：

```powershell
cargo build -p locifindd --features locifind-model-runtime/llama-cpp
```

结果：通过。

说明：

- 使用真实 `llama-cpp` 后端构建。
- 已加载 LLVM/Clang 20.1.8、VS CMake、VS Ninja。

### 3.2 daemon-mode semantic smoke

命令：

```powershell
cargo test -p locifind-evals --features semantic-recall --test daemon_mode_smoke -- --nocapture
```

结果：通过，`3 passed; 0 failed`。

覆盖：

- daemon-mode 参数校验。
- CLI help flags。
- 使用真实模型路径启动 semantic daemon。
- 等待 `/health`。
- 通过 MCP `search` 跑一条端到端查询。
- shutdown daemon 子进程。

### 3.3 locifindd e2e

命令：

```powershell
cargo test -p locifindd --test e2e -- --nocapture
```

结果：通过，`9 passed; 0 failed`。

覆盖：

- `/health`
- bearer token 鉴权 401
- `list_collections`
- MCP `search`
- 信息墙授权范围
- 越权 denied
- audit 留痕
- read-only collection reindex 409
- 增量 reindex 发现新增文件

## 4. 企业三场景手工 smoke

### 4.1 daemon 启动与索引

由于 D 盘一度无剩余空间，临时 data-dir 放到：

```text
E:\Locifind-smoke\semantic-daemon\data
```

启动结果：

- 真实模型加载成功。
- `/health` 返回 `{"status":"ok","version":"0.1.0"}`。
- 7 个 collection 全部完成首次索引。

索引日志要点：

| collection | document_scanned | document_added |
|---|---:|---:|
| `case-2026-blueharbor` | 15 | 14 |
| `case-2026-northfield` | 1 | 1 |
| `audit-2026-procurement` | 15 | 15 |
| `audit-2025-facilities` | 1 | 1 |
| `offboarding-lishili-tech` | 14 | 14 |
| `offboarding-lishili-hr` | 2 | 2 |
| `offboarding-other-tech` | 1 | 1 |

### 4.2 MCP 查询命中

使用 admin token 通过 `/mcp` 调 `tools/call search`。

| 场景 | 查询 | 结果 |
|---|---|---|
| 律师事务所 | `违约金 条款` | 命中 5 条，包括 EML、扫描 PDF、PPTX、DOCX |
| 内部审计 | `收款账户 不一致` | 命中 `payment-account-mismatch.eml` |
| 离职员工 - 技术交接 | `Lighthouse` | 命中 5 条，包括 EML、XLSX、PPTX、MD |
| 离职员工 - 技术交接 | `双层鉴权` | 命中 2 条，包括 DOCX、MD |
| 离职员工 - HR | `保密协议` | 命中 `nda-scan-source.txt` |

说明：

- 三个目标场景均有可检索命中。
- collection 权限和 list_collections 在 e2e 中已覆盖。
- 手工 smoke 验证了真实业务关键词能从测试材料中召回。

## 5. 已发现问题

### 5.1 PDF/JPG/PNG/OCR 仍需专项验收

当前真实格式材料中，扫描 PDF 被识别并进入 OCR 路径，日志出现：

```text
扫描版 PDF 检出，走 rasterize + OCR 管线
```

但此前 SQLite 检查显示 PDF/JPG/PNG 落库仍不稳定，需要后续单独排查：

- 文本层 PDF 是否稳定落入 `documents`。
- 扫描 PDF OCR 后是否稳定落入 `documents`。
- JPG/PNG OCR 是否进入索引。
- OCR 依赖是否在最终用户环境中可用。

### 5.2 泛词查询可能 degraded

已观察到部分泛词或短查询返回 degraded：

- `handover`
- `项目交接`

但更具体的业务查询可命中：

- `Lighthouse`
- `双层鉴权`
- `违约金 条款`
- `收款账户 不一致`

后续需要从 intent parser、短词策略、中文 trigram、候选链 fallback 角度继续看。

## 6. 结论

本轮测试结论：

- LLVM/Clang 19+ 环境问题已解决。
- semantic daemon 可在本机成功构建。
- 真实 GGUF 模型可加载。
- daemon semantic 自动化 smoke 已通过。
- locifindd e2e 已通过。
- 三个企业场景的核心检索链路已通过手工 smoke 验证。

不能宣称完成的部分：

- PDF/JPG/PNG/OCR 真实格式全链路尚未完成验收。
- 泛词 degraded 需要继续定位。

建议下一步：

1. 做 PDF/JPG/PNG/OCR 专项测试和修复。
2. 把三场景 `expected/queries.tsv` 接入自动化 eval，形成可重复的场景回归测试。
3. 清理或正式化 `.tmp` 下 LLVM/CMake PATH 配置，避免每次手工拼环境变量。
