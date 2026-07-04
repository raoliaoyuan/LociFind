; BETA-12 卸载流程（NSIS 卸载器 hook，经 tauri.conf.json > bundle.windows.nsis.installerHooks 挂载）。
;
; 卸载时删除本机派生数据：索引 / 模型 / 日志 / 审计（$APPDATA\${PRODUCTNAME} 整目录、
; 与桌面端 dirs::data_dir().join("LociFind") 同一位置），并清除配置目录里的用户同义词库
; 与搜索历史（查询词属敏感数据）。**保留配置**：settings.json 与 onboarding 状态所在的
; $APPDATA\${BUNDLEID} 目录不整删（Tauri 自带的「删除应用数据」勾选框负责它，属用户显式选择）。
;
; $UpdateMode 守卫（不可去掉）：版本升级时安装器带 /UPDATE 调起旧卸载器，
; 此时绝不清数据——否则每次升级都会误删用户索引与已下载模型（数百 MB 重下）。
;
; 对应的应用内清理入口见 src/uninstall.rs（macOS / 便携版走那条路径）；
; 两处清理范围保持一致，改动时同步。仓内闸门：uninstall.rs 测试
; nsis_uninstall_hook_is_wired_and_guarded 校验本文件在位 + 守卫在位。
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    RMDir /r "$APPDATA\${PRODUCTNAME}"
    Delete "$APPDATA\${BUNDLEID}\user-synonyms.yaml"
    Delete "$APPDATA\${BUNDLEID}\search_history.json"
  ${EndIf}
!macroend
