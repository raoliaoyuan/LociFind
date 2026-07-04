# ROADMAP v0.1 审阅（Codex）

> 审阅人：Codex  
> 日期：2026-05-25  
> 对象：[ROADMAP.md](../../ROADMAP.md) v0.1  
> 关注范围：§3 任务分解、依赖、估时；§6 出场指标硬度；附录 A 给 Codex 的 6 条问题。

## 总体结论

ROADMAP v0.1 的阶段划分和主干任务顺序基本合理，P 阶段能表达“macOS Spotlight 闭环”的关键路径，MVP/Beta/1.0 的范围也与项目目标一致。但作为 v1.0 执行依据，还需要补几类工程任务和指标口径，否则后续会在“看似达成出场、实际不可复现”上返工。

最需要在 v1.0 前修的是：P 阶段补测试语料/fixture 与 macOS location resolver；统一 42/47 条 schema 用例口径；MVP 阶段补 SearchBackend async/streaming 迁移与 FileActionTool；§6 指标改成带数据集、统计口径、环境条件的硬指标。

## must-fix（ROADMAP v1.0 前必改）

### 1. P 阶段缺少可复现测试语料 / fixture 任务

引用：ROADMAP §3.1 PROTO-05 / PROTO-08 / PROTO-09，§6.1。

P 阶段要求 47 条用例 eval、30 条 Spotlight 翻译实测、端到端准确率 ≥ 80%，但没有任务负责创建可控文件语料。真实用户文件系统不可作为验收基础：文件是否存在、Spotlight 是否索引、内容是否可命中都不可复现，也不适合 commit。

建议新增 `PROTO-05A` 或 `PROTO-08A`：

- 模块：`tests/fixtures` 或 `packages/evals/fixtures`
- 内容：生成合成文件集，覆盖 ppt/pdf/doc/md/mp3/mp4/png/zip、中文文件名、大小、mtime/ctime、嵌套目录、截图命名、排除类型。
- 验收：fixture 生成脚本幂等；不包含真实用户路径/文件名；Spotlight 可索引目录下至少能命中 §7.1-§7.4 的核心查询；eval 可在干净机器复现。

没有这个 task，§6.1 的准确率和响应时间都缺少测量对象。

### 2. P 阶段需要提前做 macOS location resolver，不能等到 MVP-13

引用：ROADMAP §3.1 PROTO-05 / PROTO-07，§3.2 MVP-13，schema §4.3。

P 阶段用例已包含“下载目录”“桌面”“文稿目录”等 location hint。若 resolver 到 MVP-13 才做，PROTO-05/07 的端到端闭环只能靠 backend 临时解析 hint，后续会重构。

建议新增 P 阶段任务：

- `PROTO-04A macOS location resolver v0.1`
- 模块：`platform/macos` 或 `packages/search-backends/common` 中的跨平台接口 + macOS 实现
- 依赖：PROTO-02
- 估时：0.5d
- 验收：`下载/桌面/文稿/图片/影片/音乐/截屏` hint 可解析；截屏目录读取系统配置失败时 fallback；输出路径通过 `PathBuf` 和 home dir API 生成。

MVP-13 再扩为 Windows Known Folders + 跨平台完整 resolver。

### 3. Schema 用例数量口径不一致：42 vs 47

引用：ROADMAP §3.1 PROTO-02 / PROTO-06 / PROTO-08，trait §7，schema §7。

ROADMAP 多处写“47 条用例”，schema §7 标题仍写“42 条”，trait §7 也写“42 条 schema 用例”，但 schema 已新增 §7.8 #43-#47。这个口径不统一会直接影响 PROTO-02/06/08 的验收范围。

建议 ROADMAP v1.0 明确：

- P 阶段 eval 总集为 47 条。
- SpotlightBackend 翻译实测只覆盖 §7.1-§7.4 共 30 条搜索类用例。
- refine/action/clarify 用例不进入 SearchBackend 实测，但进入 serde/schema/parser/harness 契约测试。
- 同步修正 schema/trait 文档标题或 ROADMAP 引用，避免后续工具按不同口径执行。

### 4. MVP 缺少 SearchBackend v0.2 async / streaming 迁移任务

引用：ROADMAP §3.2 M1/M4，trait §3.1，§6.2。

Trait 文档明确“原型期同步，MVP 切 async + Stream”，ROADMAP 只有 `MVP-07 Streaming 抽象`，但没有任务负责把 `SearchBackend` trait、SpotlightBackend、WindowsSearchBackend、EverythingBackend 从同步 `Vec` 迁移到异步流式接口。

建议新增：

- `MVP-07A SearchBackend v0.2 async/streaming 迁移`
- 依赖：MVP-04、MVP-07、MVP-11、MVP-12 可分阶段
- 验收：三个真实 backend 暴露统一 stream 接口；支持取消；CLI/UI 都能消费；旧同步接口有兼容层或明确删除。

否则 `MVP-19 搜索框 UI + 流式结果列表` 对 backend 的真实依赖是隐藏的。

### 5. MVP 缺少 FileActionTool 任务

引用：ROADMAP §3.2 M1/M4/M5，schema §3.3 / §7.6，PROJECT.md“不做删除/批量修改自动执行”。

Schema v1.0 已覆盖 `file_action`，§6.2 又要求“文件操作权限策略 100% 通过安全 evals”，但 ROADMAP 没有落地 `open/locate/copy/move/rename` 的工具实现。只有 Policy Engine 不足以支撑 action intent。

建议新增 `MVP-10A FileActionTool`：

- 依赖：MVP-03 Policy Engine、MVP-06 Context Memory。
- 模块：`packages/harness` + `platform/{macos,windows}`。
- 验收：`open` / `locate` 可用；`copy` / `move` / `rename` 经过确认；`delete` 明确禁用；target_ref 越界、批量阈值、路径冲突有测试。

### 6. §6 指标需要定义统计口径，否则不够硬

引用：ROADMAP §6.1-§6.4。

目前很多指标是方向正确但口径不足，例如“简单查询响应 < 500ms”“16GB 机器流畅运行”“SearchBackend 工具调用成功率 > 95%”。这些不能直接判定通过/失败。

建议 v1.0 为每个性能/准确率指标补四要素：

- 数据集：哪份 eval、多少条、是否固定 seed。
- 统计方式：平均值 / p50 / p95 / p99，是否允许重试。
- 运行环境：macOS/Windows 版本、硬件档位、Spotlight/Windows Search 索引状态。
- 排除项：冷启动、模型首次加载、Spotlight 首次索引、网络下载等是否计入。

例如 P 阶段可改为：“在固定 47 条 eval + 合成 fixture 上，规则解析路径 p95 < 500ms，不含 mdfind 执行；CLI 端到端简单查询 p95 < 1500ms，Spotlight 索引已完成。”

## should-have（v1.0.x 或启动对应阶段前修）

### 7. PROTO-03 “与 schema md §5 内容字节一致”不现实

引用：ROADMAP §3.1 PROTO-03。

Markdown 中嵌入的 JSON Schema 与独立 `.json` 文件很难保持“字节一致”，因为围栏、缩进、注释说明都会干扰。更合理的验收是语义一致与测试一致。

建议改为：

- `docs/schema/search-intent.v1.json` 通过 JSON Schema meta-schema 校验。
- 从 schema md 的 §7 用例抽样/全量验证。
- Rust serde 类型与 JSON Schema 对同一批正/反例结论一致。

### 8. PROTO-02 依赖 PROTO-03 的方向可考虑反转或并行

引用：ROADMAP §3.1 PROTO-02 / PROTO-03。

当前 PROTO-03 依赖 PROTO-02，意味着先写 Rust serde，再落独立 JSON Schema。若 schema 是外部契约，通常应先落 JSON Schema，再用它驱动 serde 测试。但考虑 schema md 已存在，当前顺序也能跑。

建议不是硬改，而是在 ROADMAP 里标注 PROTO-02/03 可并行：一边落 JSON Schema，一边实现 serde；最终用交叉测试合流。

### 9. PROTO-05 估时 2d 偏乐观

引用：ROADMAP §3.1 PROTO-05，trait §4.2。

SpotlightBackend 不只是拼谓词，还包括 `mdfind` 子进程管理、超时 kill、metadata 补全、中文匹配、`-onlyin`、fixture、注入防护、macOS 版本差异实测。2d 对“可演示”可行，对“通过 trait §4.2 实测清单”偏紧。

建议估时改为 3d，或拆成：

- PROTO-05a 查询翻译 + 单元测试：1d
- PROTO-05b mdfind 执行 + metadata + 超时：1d
- PROTO-05c 实机验证清单 + 修正：1d

### 10. PROTO-06 估时 3d 风险中等，需标注“规则解析优先、模型不进 P”

引用：ROADMAP §3.1 PROTO-06，PROJECT.md 架构。

47 条自然语言到 SearchIntent，包含中英混合、refine、action、clarify。3d 可以接受，但前提是 P 阶段只做规则/启发式 parser，不接本地模型。

建议在任务标题或验收中明确：“规则解析器 v0.1（不含模型 fallback）”。模型 fallback 已在 MVP-17，避免后续实现时把 PROTO-06 扩成模型集成任务。

### 11. MVP 并行机会有，但 M1/M2/M4 存在隐藏依赖

引用：ROADMAP §3.2 M1-M5。

M2 Windows backend 可与 M1 并行启动，但 M4 UI 对 M1 的依赖不止 MVP-18/MVP-07：后端状态、错误降级、权限确认、Context Memory 都会影响 UI 行为。

建议在 §3.2 增加“并行约束”：

- MVP-18 桌面骨架可早启。
- MVP-19 流式结果列表依赖 MVP-07 + 至少一个 stream backend。
- MVP-21 后端状态依赖 MVP-09/MVP-10。
- 文件操作 UI 依赖新增 FileActionTool + MVP-03。
- 设置/隐私页可与 backend 并行，但 Full Disk Access 引导依赖平台权限检测。

### 12. Beta 阶段任务缺少显式依赖列

引用：ROADMAP §3.3。

BETA 表只有 ID/标题/状态/模块/估时，没有依赖列，和 §1 Task 卡片字段约定不一致。Beta 虽较远，但 v1.0 ROADMAP 应至少列粗粒度依赖，避免后续阶段切换时重新梳理。

最低限度建议：

- BETA-04 依赖 MVP-10 / BETA-01-03。
- BETA-05 依赖 BETA-04。
- BETA-07 依赖 BETA-01-03。
- BETA-10/11 依赖 MVP-18/19/22 和 §5 账号/证书。
- BETA-14 依赖 BETA-10/11/13。

### 13. 风险地图建议补“系统搜索不可控导致 eval 不稳定”

引用：ROADMAP §7，§6.1/§6.2。

已有 Spotlight 排除目录、Windows Search 禁用风险，但缺少“系统搜索索引延迟/版本差异导致 eval flake”的工程风险。这个风险会直接影响 CI 与出场评测可信度。

建议新增：

- 风险：系统搜索索引延迟或排序差异导致 eval 不稳定。
- 概率：高；影响：中。
- 缓解：fixture 预热、等待索引完成、backend 测试分层（翻译单元测试 vs 实机 smoke）、Top-K 容忍、记录 OS/索引状态。

## nice-to-have（可后续优化）

### 14. 增加每阶段“不可回归指标”

引用：ROADMAP §6。

例如 P 结束后，MVP 阶段不应让 47 条 P eval 通过率下降；MVP 结束后，Beta 的新增索引不应破坏系统搜索基础路径。可以在 §6 写成：

- 每阶段出场必须重跑所有前序阶段 eval。
- 前序 eval 通过率不得低于上一阶段出场报告，或下降需有明确豁免记录。

### 15. P 阶段可增加 `clippy` / `rustfmt` / lint gate

引用：ROADMAP §3.1 PROTO-01，CONVENTIONS §6。

PROTO-01 只写 `cargo build/test`，建议顺手把 `cargo fmt --check`、`cargo clippy --workspace --deny warnings` 放进本地 CI 脚本。不是必须，但早设 gate 可减少后续多工具协作格式漂移。

### 16. `docs/reviews/proto-exit.md` 可模板化

引用：ROADMAP §3.1 PROTO-09，§8。

建议在 PROTO-09 验收中要求报告包含固定章节：环境、数据集版本、准确率、性能、失败用例、豁免、下一阶段风险。这样 MVP/Beta 出场报告可以复用同一格式。

## out-of-scope（建议不在 ROADMAP v1.0 处理）

### 17. 不建议把本地模型提前到 P 阶段

引用：ROADMAP §3.1、§3.2 M3。

P 阶段的核心风险是 schema、规则解析、Spotlight 翻译和本地搜索闭环。模型引入会增加 llama.cpp、模型文件、prompt、JSON 修复等变量，降低原型验证效率。保持 M3 再接模型是合理的。

### 18. 不建议在 P 阶段做 Tauri UI

引用：ROADMAP §3.1、§3.2 M4。

CLI 足以验证闭环和 eval，UI 会引入前端构建、权限弹窗、跨平台样式等非核心变量。P 阶段不做 UI 是合理边界。

## 对附录 A 给 Codex 6 个问题的直接答复

1. **P 阶段任务分解**：主干合理，但遗漏两个必须任务：合成 fixture/eval 语料、macOS location resolver v0.1。若不补，P 出场指标不可复现。
2. **P 阶段依赖图与关键路径**：依赖图主线基本正确，但 PROTO-03 可与 PROTO-02 并行，PROTO-05 需要 resolver/fixture 隐含依赖。关键路径 6 天偏乐观，补齐 fixture/resolver 后更接近 7-8 天；总工期 7-10 天仍合理。
3. **估时可信度**：最不可信的是 PROTO-05（2d 偏紧）、PROTO-06（3d 取决于是否只做规则）、MVP-11（WindowsSearchBackend 3d 偏乐观，OLE DB/SystemIndex 环境差异多）、BETA-02/03（内容索引和 OCR 估时风险高）。
4. **§6 指标硬度**：方向正确但口径不够硬。准确率要绑定 eval 版本和计算方式；性能要用 p95/p99、固定环境、区分冷/热路径；“流畅运行”“成功率”要拆成可测指标。
5. **MVP 并行机会与隐藏依赖**：M1/M2/M3 可并行，但 M4 的流式 UI 隐含依赖 backend async/streaming 迁移；文件操作 UI 隐含依赖 FileActionTool；后端状态 UI 隐含依赖 Capability Discovery/Fallback Chain。
6. **工程风险遗漏**：建议补系统搜索索引延迟/排序差异导致 eval flake、Windows Search OLE DB 权限/企业策略差异、Tauri 权限/签名与本地文件访问联动、模型文件体积和首次下载/随包分发策略。
