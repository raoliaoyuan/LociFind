LociFind v0.9.14 —— **首个公开发版** 🎉

LociFind 自本版起开源免费（MIT OR Apache-2.0 双许可），代码与安装包任何人可自由使用。

## 安装

未做代码签名（开源免费分发）：SmartScreen 提示时选「更多信息 → 仍要运行」。放行步骤、SHA256 校验、升级/卸载说明详见 **[安装指南](https://github.com/raoliaoyuan/LociFind/blob/main/docs/install.md)**。升级安装不清除任何数据。

## 自 v0.9.13 以来的变更

### 新增

- **开源发布**：MIT OR Apache-2.0 双许可（LICENSE-MIT / LICENSE-APACHE）；[隐私说明 PRIVACY.md](https://github.com/raoliaoyuan/LociFind/blob/main/PRIVACY.md)（无遥测、唯一联网点 = 用户主动触发的模型下载）；[安装指南](https://github.com/raoliaoyuan/LociFind/blob/main/docs/install.md)与贡献指南。
- **卸载清理（BETA-12）**：卸载器自动清除索引 / 模型 / 日志 / 审计 / 搜索历史 / 用户同义词（settings.json 保留；升级安装不触发清理）；应用内「隐私 → 卸载清理」提供同款二段确认清理，覆盖手动清场景。**本版为 NSIS 卸载 hook 首次随包发布。**
- **意图草稿（BETA-29 v1+v2）**：结果页意图条新增「调整 ▾」草稿面板——以 chips 与下拉直接改关键词 / 类型 / 时间 / 排序后重跑；草稿可存入保存的搜索（带 ⚙ 标记）；搜索框 ⚙ 按钮 / Shift+Enter 可在执行前**预览**解析出的意图（零执行）。
- **检索出处（daemon，BETA-43）**：MCP 检索结果带 `snippet`/`pages` 出处、新增 `read_document` 工具（`allow_full_read` 门控）与审计报告导出 `/admin/audit/report`。

### 改进

- **解析质量大幅收敛**：1000 条评测集通过率 88.1% → **97.7%**（时间表达簇九形态绝对日期 / 日期区间 / 「这周·这个月·最近拍」等措辞、英文复数归一、关键词抽取多刀、`in the <kw>` 谓词族、「几百KB」尺寸启发等，全程对既有用例零回归）。
- 企业场景检索评测扩容 22 → 53 条查询，真实模型全过。

### 修复

- media 路径 `sorted by X` 短语泄漏进关键词、`bigger than` 尺寸条件丢失。
- Windows `\\?\` 长路径前缀在 daemon 出处/回读中的归一。

## 可选模型（推荐应用内「快速入门」一键下载）

- `embeddinggemma-300m-qat-Q8_0.gguf` —— 语义召回（「按意思找」/ 跨语言）；缺失降级纯关键词。
- `Qwen3-0.6B-Q4_K_M.gguf` —— 复杂查询 AI 解析 fallback；缺失降级规则解析。

两者均可不装（纯关键词搜索可用）；离线可手动放置到 `%APPDATA%\LociFind\models\`。
