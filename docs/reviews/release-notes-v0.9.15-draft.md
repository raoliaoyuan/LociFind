LociFind v0.9.15

本版首次由 CI 产出 **macOS DMG 安装包**（Apple Silicon），Windows 与 macOS 同 tag 并行发布；并带来追问（clarify）体验的中英文本地化与一批自然语言解析修正。

## 安装

- **Windows**：未做代码签名（开源免费分发），SmartScreen 提示时选「更多信息 → 仍要运行」。
- **macOS**（Apple Silicon）：未做签名 / 公证，首次运行 Gatekeeper 会拦截——右键 App →「打开」，或终端执行 `xattr -dr com.apple.quarantine /Applications/LociFind.app`。Intel Mac 请从源码构建。

放行步骤、SHA256 校验、升级/卸载、从源码构建详见 **[安装指南](https://github.com/raoliaoyuan/LociFind/blob/main/docs/install.md)**。升级安装不清除任何数据。

## 自 v0.9.14 以来的变更

### 新增

- **macOS DMG 安装包**：本版起 macOS（Apple Silicon / aarch64）安装包由 GitHub Actions 自动构建、随 Release 发布（此前仅 Windows）。**本版为 macOS DMG CI 首次真实构建。**

### 改进

- **追问（clarify）本地化**：当查询过于模糊需要追问时，英文查询现返回**英文**追问文案与选项（此前固定中文）；追问按歧义维度（类型 / 时间 / 位置 / 动作 / 危险操作）提供一键收窄的选项。
- **自然语言解析修正（评测 97.7% → 99.4%）**：`songs by <艺人>` 识别连字符艺人名、含「和」的专有词（如「碳中和目标」）不再被错误切分、`no <扩展名>` 排除、混排目录名（如「music 目录」）位置识别、抽象大小（「几个 G 的视频」）排序等一批修正，对既有用例零回归。

## 可选模型（推荐应用内「快速入门」一键下载）

- `embeddinggemma-300m-q8_0.gguf` —— 语义召回（「按意思找」/ 跨语言）；缺失降级纯关键词。
- `qwen3-0.6b-q4_k_m.gguf` —— 复杂查询 AI 解析 fallback；缺失降级规则解析。

两者均可不装（纯关键词搜索可用）；离线可手动放置到模型目录（Windows `%APPDATA%\LociFind\models\` / macOS `~/Library/Application Support/LociFind/models/`）。
