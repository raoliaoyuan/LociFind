# LociFind 安装指南

> LociFind 是开源免费软件（MIT OR Apache-2.0），**不购买代码签名证书、不注册 Apple Developer**——因此安装包未签名，系统会弹「未知发布者」类提示。这是开源分发的正常现象，本文告诉你每一步怎么过、以及如何校验你下载的文件确实来自官方 Release。

## Windows

### 下载安装

1. 到 [GitHub Releases](https://github.com/raoliaoyuan/LociFind/releases) 下载最新的 `LociFind_x.y.z_x64-setup.exe`（NSIS 安装包）。
2. 运行时若弹出 **SmartScreen「Windows 已保护你的电脑」**：点「**更多信息**」→「**仍要运行**」。
   - 为什么会弹：安装包未做代码签名（见顶部说明），SmartScreen 对无签名/低信誉文件一律提示，与是否安全无关。
3. 按向导完成安装。

### 校验下载（建议）

```powershell
Get-FileHash .\LociFind_x.y.z_x64-setup.exe -Algorithm SHA256
```

与 Release 页面对应资产显示的 SHA256 摘要比对一致即可。

### 升级 / 卸载

- **升级**：直接运行新版安装包覆盖安装，索引、模型、设置全部保留（安装器带升级守卫）。
- **卸载**：控制面板卸载即可，卸载器会自动清除索引、模型、日志、审计与搜索历史（`settings.json` 保留）；也可先在应用内「隐私 → 卸载清理」手动执行。

## macOS

> **当前状态**：macOS 安装包（DMG）的自动构建尚未上线（ROADMAP BETA-10 剩余项），目前请[从源码构建](#从源码构建)。以下 Gatekeeper 说明适用于未来发布的 DMG 下载件。

未签名、未公证的 app 首次打开会被 **Gatekeeper** 拦截（「无法打开，因为它来自身份不明的开发者」/「未能验证不包含恶意软件」）。任选其一放行：

- **macOS 14 及更早**：在访达中**右键（Control-点按）app → 打开** → 弹窗中再点「打开」。
- **macOS 15 (Sequoia) 及更新**：直接双击会被拒且右键不再提供旁路——先双击一次，然后到 **系统设置 → 隐私与安全性**，在页面底部找到 LociFind 条目点「**仍要打开**」。
- **命令行**（所有版本）：`xattr -dr com.apple.quarantine /Applications/LociFind.app`（移除下载隔离属性）。

以上任一操作只需做一次。app 二进制带 ad-hoc 签名（Apple Silicon 运行必需，Tauri 构建默认附带），Gatekeeper 提示只关乎「开发者身份未验证」，不代表文件被篡改——请配合 SHA256 校验下载完整性。

## 从源码构建

前置：Rust stable（版本见 [rust-toolchain.toml](../rust-toolchain.toml)）、Node 20+、[Tauri 2 前置依赖](https://tauri.app/start/prerequisites/)（Windows 另需 cmake，用于 llama.cpp）。

```bash
git clone https://github.com/raoliaoyuan/LociFind.git
cd LociFind/apps/desktop
npm install
npm run tauri build -- --features model-fallback,semantic-recall
```

产物在 `apps/desktop/src-tauri/target/release/bundle/`。源码构建的 app 不带下载隔离属性，无 Gatekeeper/SmartScreen 提示。

## 模型文件（可选）

安装后首次运行，「快速入门」提供两个本地模型的**一键下载**（来源 huggingface.co，这是应用唯一的联网行为，见 [PRIVACY.md](../PRIVACY.md)）：

- **embedding 模型** —— 启用「按意思找 / 跨语言」语义召回；缺失则降级纯关键词（FTS）搜索。
- **生成模型** —— 复杂查询的 AI 解析 fallback；缺失则降级规则解析。

两者都不装也能正常使用。离线环境可手动放置 GGUF 文件到数据目录 `models/`（Windows：`%APPDATA%\LociFind\models\`）。

## 包管理器渠道（规划中）

winget / Scoop / Homebrew 渠道在评估推进中（[渠道评估](reviews/beta-10-distribution-channels-2026-07-04.md)），上线后本文更新一键安装命令。
