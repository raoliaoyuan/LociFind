; BETA-12 卸载流程（NSIS 卸载器 hook，经 tauri.conf.json > bundle.windows.nsis.installerHooks 挂载）。
;
; 卸载时删除本机派生数据：日志 / 审计（$APPDATA\${PRODUCTNAME} 目录、与桌面端
; dirs::data_dir().join("LociFind") 同一位置），并清除配置目录里的用户同义词库与搜索历史
; （查询词属敏感数据）。**保留配置**：settings.json 与 onboarding 状态所在的
; $APPDATA\${BUNDLEID} 目录不整删（Tauri 自带的「删除应用数据」勾选框负责它，属用户显式选择）。
;
; **模型 + 索引默认保留**：
; - 模型（2026-07-06 cycle 9 真机反馈拍板）：公开权重、非用户敏感数据，整删导致重装
;   ~700MB 重下。
; - 索引 index.db（+ -wal/-shm）（用户反馈拍板）：重建是小时级本地重新提取 + 嵌入的
;   计算成本（非下载），「先卸载旧版再装新版」这类手动两步操作不该让用户平白丢掉这份
;   成本、逼一次全量重扫——与覆盖安装走 `$UpdateMode` 分支保留数据同等待遇。
; 卸载时 MessageBox 询问是否一并删除模型与索引，默认「否」（保留）；静默卸载（/S）走
; /SD IDNO = 保留。保留实现：models 子目录 + index.db 系列文件同卷 Rename 暂存 → 整目录
; RMDir /r（敏感派生数据零遗漏、未来新增子项自动纳入删除面）→ 移回。
; **索引仍含从用户文档抽出的正文 / PII 实体标记（BETA-59）**——选「是」彻底删除时，索引
; 与模型一样整删，不会遗留任何文档正文片段。
;
; $UpdateMode 守卫（不可去掉）：版本升级时安装器带 /UPDATE 调起旧卸载器，
; 此时绝不清数据——否则每次升级都会误删用户索引与已下载模型（数百 MB 重下 + 小时级重扫）。
;
; 对应的应用内清理入口见 src/uninstall.rs（macOS / 便携版走那条路径；应用内清理仍是
; 全删含模型与索引——§6.3「一键删除索引/日志/模型/配置」指标由它承担，是用户主动发起的
; 「清空我的数据」动作，与「卸载程序想留着索引下次重装接着用」语义不同，不做同等保留）。
; 仓内闸门：uninstall.rs 测试 nsis_uninstall_hook_is_wired_and_guarded 校验本文件在位 +
; 守卫在位 + 模型/索引保留路径在位。
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    ; 问一次「模型 + 索引」去留（默认/静默 = 都保留）。
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
      "是否同时删除已下载的 AI 模型文件（约 700 MB）和搜索索引数据库？$\r$\n选「否」两者都保留，重装后模型无需重新下载、索引自动增量续跑（不用整库重扫）。" \
      /SD IDNO IDYES locifind_delete_all
    ; 保留模型 + 索引：同卷暂存 → 整删 → 移回（目标不存在时各步自然 no-op）。
    Rename "$APPDATA\${PRODUCTNAME}\models" "$APPDATA\${PRODUCTNAME}-models-keep"
    Rename "$APPDATA\${PRODUCTNAME}\index.db" "$APPDATA\${PRODUCTNAME}-index.db-keep"
    Rename "$APPDATA\${PRODUCTNAME}\index.db-wal" "$APPDATA\${PRODUCTNAME}-index.db-wal-keep"
    Rename "$APPDATA\${PRODUCTNAME}\index.db-shm" "$APPDATA\${PRODUCTNAME}-index.db-shm-keep"
    RMDir /r "$APPDATA\${PRODUCTNAME}"
    IfFileExists "$APPDATA\${PRODUCTNAME}-models-keep\*.*" 0 locifind_restore_index
    CreateDirectory "$APPDATA\${PRODUCTNAME}"
    Rename "$APPDATA\${PRODUCTNAME}-models-keep" "$APPDATA\${PRODUCTNAME}\models"
locifind_restore_index:
    IfFileExists "$APPDATA\${PRODUCTNAME}-index.db-keep" 0 locifind_cleanup_config
    CreateDirectory "$APPDATA\${PRODUCTNAME}"
    Rename "$APPDATA\${PRODUCTNAME}-index.db-keep" "$APPDATA\${PRODUCTNAME}\index.db"
    IfFileExists "$APPDATA\${PRODUCTNAME}-index.db-wal-keep" 0 locifind_restore_shm
    Rename "$APPDATA\${PRODUCTNAME}-index.db-wal-keep" "$APPDATA\${PRODUCTNAME}\index.db-wal"
locifind_restore_shm:
    IfFileExists "$APPDATA\${PRODUCTNAME}-index.db-shm-keep" 0 locifind_cleanup_config
    Rename "$APPDATA\${PRODUCTNAME}-index.db-shm-keep" "$APPDATA\${PRODUCTNAME}\index.db-shm"
    Goto locifind_cleanup_config
locifind_delete_all:
    RMDir /r "$APPDATA\${PRODUCTNAME}"
locifind_cleanup_config:
    Delete "$APPDATA\${BUNDLEID}\user-synonyms.yaml"
    Delete "$APPDATA\${BUNDLEID}\search_history.json"
  ${EndIf}
!macroend
