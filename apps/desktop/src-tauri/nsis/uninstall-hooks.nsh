; BETA-12 卸载流程（NSIS 卸载器 hook，经 tauri.conf.json > bundle.windows.nsis.installerHooks 挂载）。
;
; 卸载时删除本机派生数据：索引 / 日志 / 审计（$APPDATA\${PRODUCTNAME} 目录、与桌面端
; dirs::data_dir().join("LociFind") 同一位置），并清除配置目录里的用户同义词库与搜索历史
; （查询词属敏感数据）。**保留配置**：settings.json 与 onboarding 状态所在的
; $APPDATA\${BUNDLEID} 目录不整删（Tauri 自带的「删除应用数据」勾选框负责它，属用户显式选择）。
;
; **模型默认保留（2026-07-06 cycle 9 真机反馈拍板）**：模型是公开权重、非用户敏感数据，
; 整删导致重装 ~700MB 重下。卸载时 MessageBox 询问是否一并删除模型，默认「否」（保留）；
; 静默卸载（/S）走 /SD IDNO = 保留。保留实现：models 子目录同卷 Rename 暂存 → 整目录
; RMDir /r（敏感派生数据零遗漏、未来新增子项自动纳入删除面）→ 移回。
;
; $UpdateMode 守卫（不可去掉）：版本升级时安装器带 /UPDATE 调起旧卸载器，
; 此时绝不清数据——否则每次升级都会误删用户索引与已下载模型（数百 MB 重下）。
;
; 对应的应用内清理入口见 src/uninstall.rs（macOS / 便携版走那条路径；应用内清理仍是
; 全删含模型——§6.3「一键删除索引/日志/模型/配置」指标由它承担）。
; 仓内闸门：uninstall.rs 测试 nsis_uninstall_hook_is_wired_and_guarded 校验本文件在位 +
; 守卫在位 + 模型保留路径在位。
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; 问一次模型去留（默认/静默 = 保留）。
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
      "是否同时删除已下载的 AI 模型文件（约 700 MB）？$\r$\n选「否」保留模型，重装后无需重新下载。" \
      /SD IDNO IDYES locifind_delete_all
    ; 保留模型：models 同卷暂存 → 整删 → 移回（数据目录不存在 / 无 models 时各步自然 no-op）。
    Rename "$APPDATA\${PRODUCTNAME}\models" "$APPDATA\${PRODUCTNAME}-models-keep"
    RMDir /r "$APPDATA\${PRODUCTNAME}"
    IfFileExists "$APPDATA\${PRODUCTNAME}-models-keep\*.*" 0 locifind_cleanup_config
    CreateDirectory "$APPDATA\${PRODUCTNAME}"
    Rename "$APPDATA\${PRODUCTNAME}-models-keep" "$APPDATA\${PRODUCTNAME}\models"
    Goto locifind_cleanup_config
locifind_delete_all:
    RMDir /r "$APPDATA\${PRODUCTNAME}"
locifind_cleanup_config:
    Delete "$APPDATA\${BUNDLEID}\user-synonyms.yaml"
    Delete "$APPDATA\${BUNDLEID}\search_history.json"
  ${EndIf}
!macroend
