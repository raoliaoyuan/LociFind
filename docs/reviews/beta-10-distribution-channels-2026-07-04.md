# BETA-10/10A 开源分发渠道评估

> 评估人：Claude Code (Fable 5)
> 日期：2026-07-04
> 背景：2026-07-04 开源免费拍板后 BETA-10/10A re-scope——不签名不公证，分发 = GitHub Releases + 包管理器渠道。本文是渠道评估结论；用户侧安装步骤见 [docs/install.md](../install.md)。

## 现状盘点

- **Windows**：`release-windows.yml` 已就绪（v* tag → tauri-action → NSIS 包 → GitHub Release，prerelease）。Release body 已含 SmartScreen 提示与模型放置指引。
- **macOS**：**无发版 workflow**——DMG 产物 CI 是 BETA-10 唯一剩余工程项（tauri-action 加 macos-latest job 即可产未签名 DMG + ad-hoc 签名；需真机验证 Gatekeeper 放行路径）。
- 校验：GitHub Release 资产自带 SHA256 摘要展示，install.md 已给 `Get-FileHash` 对照法；无需自建 checksum 步骤。

## 渠道评估（按建议推进顺序）

### 1. Scoop（Windows）——成本最低，可先行

- **门槛**：对签名无要求；自建 bucket 仓库（如 `raoliaoyuan/scoop-locifind`）放一个 JSON manifest 即可，用户 `scoop bucket add locifind <repo-url> && scoop install locifind`。
- **适配**：Scoop 偏好 portable/zip，NSIS 安装包可在 manifest 用 `installer.script` 静默参数（NSIS `/S`）适配；或后续让 tauri 同时出 zip。
- **主 bucket（extras）**：有维护者审核，无硬性知名度指标，manifest 质量达标即可尝试。
- **建议**：**下次发版后即可做**（manifest 指向 Release 资产 + sha256，autoupdate 字段跟版本）。

### 2. winget（Windows）——覆盖最广

- **门槛**：接受未签名安装包（提交时过 Defender/SmartScreen 自动扫描）；要求稳定的 versioned 下载 URL（GitHub Releases 满足）与 per-version manifest PR（可用 `wingetcreate` / `komac` 半自动化）。
- **注意**：每次发版需向 `microsoft/winget-pkgs` 提 PR，节奏不稳时维护成本高；prerelease 版本不宜提交。
- **建议**：**Beta 出场（BETA-14）后、版本节奏进入稳定期再提交**首个 manifest。
- **联动**：winget 装的 Everything CLI 检测已有两段式兜底（onboarding），LociFind 自身进 winget 后形成同渠道闭环。

### 3. Homebrew（macOS）——依赖 DMG CI

- **自建 tap 先行**：`raoliaoyuan/homebrew-locifind` 放 cask 定义，用户 `brew tap` + `brew install --cask locifind`。cask 需要稳定 DMG URL + sha256 → **强依赖 BETA-10 DMG CI 落地**。
- **quarantine 注意**：brew 默认保留下载隔离属性，用户仍会遇 Gatekeeper（install.md 已覆盖）；cask 加 `caveats` 提示放行步骤。
- **官方 homebrew/cask**：有知名度门槛（星标/关注量指标），新项目大概率被拒——**待社区积累后再提交**，自建 tap 不受影响。

### 4. 不做的渠道

- Mac App Store / Microsoft Store：需开发者账号，已随 2026-07-04 开源免费拍板取消。
- Chocolatey：与 winget/Scoop 重叠，社区审核慢，暂不投入。

## 结论与剩余工程项

| 项 | 归属 | 时点 |
|---|---|---|
| 安装文档（SmartScreen / Gatekeeper / 校验 / 源码构建） | ✅ done（[install.md](../install.md)） | 2026-07-04 |
| 渠道评估 | ✅ done（本文） | 2026-07-04 |
| macOS DMG 产物 CI（未签名 + ad-hoc） | BETA-10 剩余 | 下次触碰 macOS 侧时 |
| 双平台真机「下载→放行→安装→可用」验证（§6.3 指标） | BETA-10/10A 剩余 | 随下次发版 / cycle 9 |
| Scoop 自建 bucket manifest | BETA-10A 后续 | 下次发版后 |
| winget 首个 manifest | BETA-10A 后续 | BETA-14 后稳定期 |
| Homebrew 自建 tap cask | BETA-10 后续 | DMG CI 后 |
