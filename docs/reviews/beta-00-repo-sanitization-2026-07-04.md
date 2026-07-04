# BETA-00 公开仓库脱敏核查报告

> 评估人：Claude Code (Fable 5)
> 日期：2026-07-04
> 范围：BETA-00 开源发布审查最后一项——公开仓库前的敏感信息核查与清理。

## 结论

**工作区（HEAD 树）已清理完毕，可公开**；唯一遗留决策 = **git 历史是否随仓库公开**（见 §4，不阻塞其余工作）。

## 1. 核查范围与方法

| 面 | 方法 | 结果 |
|---|---|---|
| BETA-44 企业语料（66 文件） | 逐目录清点 + 内容抽查 + README 来源核对 | ✅ 全合成（example.com / 李示例 / 虚构公司 BlueHarbor·Orion·lishili 等；README 明示「均为虚构或占位」）；`real-formats/` 指真实**格式**非真实内容 |
| 语料图片（5 张 png/jpg） | 文件头检查（EXIF）+ 体积 | ✅ 程序生成（33-48KB、JFIF 头无 EXIF APP1 段，无相机/GPS 元数据） |
| 密钥/凭证（工作区） | 正则扫描（sk- / ghp_ / AKIA / PRIVATE KEY / xox / AIzaSy 等） | ✅ 零命中 |
| 密钥/凭证（**git 全历史** 924 commits） | `git log --all -p` 同一组正则 | ✅ 零命中 |
| 内网 IP / 主机名 | 私网段正则 + DESKTOP-* 模式 | ✅ 仅文档示例 `192.168.1.50`（通用示例，保留） |
| 个人 OS 用户名 | `Users[/\\](Roger\|roger\|raoli)` + 独立词边界 + 中文本地化路径 `用户\raoli` | ⚠️ 148 处 / 37 文件 → **本次全部清理**（见 §2） |
| training/ 数据集（21 文件） | 全库扫描覆盖 + 来源核对（CONVENTIONS「严格用合成数据」） | ✅ 无个人信息 |
| artifacts/ / resources/ | git ls-files 清点 | ✅ 仅同义词 yaml，无敏感内容 |

## 2. 清理动作（本次 commit）

- 全库 **等长替换**：`Roger→Alice`、`roger→alice`、`raoli→alice`（路径上下文 + 中文本地化路径 `C:\用户\raoli\下载`），覆盖 37 文件 148 处——代码测试字符串（settings.rs / tracing.rs / windows-search / model-runtime / preview 均为自洽字面量）、CLAUDE.md、docs 历史文档与归档。
- CLAUDE.md npm 路径改通用写法 `%APPDATA%\npm`。
- **验证**：locifind-harness 188 / windows-search 14+13 / model-runtime 4+1 / desktop settings 21 全绿；indexer examples `cargo check` 通过；终检 `Roger|roger|raoli`（词边界，排除 raoliaoyuan）全库零命中。

## 3. 有意保留（非泄漏）

- **`raoliaoyuan`**（GitHub 账号 / LICENSE 版权署名 / repo URL / commit author）：开源即以此身份发布，属公开身份。
- **`192.168.1.50`**：daemon 文档示例 IP，通用私网段示例。
- **`LocalFind-gemini` 等 worktree 名**：历史项目代号，无个人信息。
- **虚构语料全部**：README 已声明虚构；后续替换为设计伙伴真实语料时**不得入仓**（README 红线已写明）。

## 4. 遗留决策：git 历史（公开仓库前拍板，不阻塞其他工作）

历史 diff 中残留 ~192 处用户名路径（含本次清理的删除行自身）。无密钥、无真实语料，敏感度=OS 用户名级别。三个选项：

1. **推荐：orphan/squash 首发**——公开仓库以单次「初始开源」commit 起步（或 squash 全史），私有仓库保留完整历史作内部档案。干净且零风险。
2. **接受残留直接公开**——用户名级信息公开风险低，如不介意 `Roger/raoli` 出现在历史中可直接转公开。
3. **git filter-repo 重写**——保历史又除残留，但重写破坏所有既有 clone/commit 引用，工程成本最高，不建议。

## 5. BETA-00 收口状态

LICENSE 双许可 ✅ / third-party 台账差异 0 ✅ / Everything 条款核查 ✅ / PRIVACY.md ✅ / 商标使用规范（不暗示背书）复核 ✅ / **工作区脱敏 ✅（本报告）**。BETA-00 全部验收项完成；唯一开放项 = §4 历史决策，归属「转公开」操作时点。
