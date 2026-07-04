# scoop-locifind

[LociFind](https://github.com/raoliaoyuan/LociFind)（Local search for humans——本地语义文件搜索）的 Scoop bucket。

## 安装

```powershell
scoop bucket add locifind https://github.com/raoliaoyuan/scoop-locifind
scoop install locifind
```

## 说明

- LociFind 经 NSIS 安装器安装到 `%LOCALAPPDATA%\LociFind`，升级/卸载由应用自身管理（非 Scoop 便携目录）。
- `scoop uninstall locifind` 会运行应用卸载器并清除索引/模型/日志等本地数据（`settings.json` 保留）。
- 安装包未签名（开源免费分发），SmartScreen/Defender 提示属正常，详见[安装指南](https://github.com/raoliaoyuan/LociFind/blob/main/docs/install.md)。

## License

manifest 与本仓库内容按 MIT OR Apache-2.0 提供（与主仓库一致）。
