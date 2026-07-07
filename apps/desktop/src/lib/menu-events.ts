// BETA-33 cycle 2：菜单栏 → SearchView 内部 handler 的事件总线。
// 选事件总线（而非 Context 或 Ref）是因为 MenuBar 与 SearchView 都是大组件、
// 用全局自定义事件可零侵入、SearchView 内部不必改 props 签名。

export type MenuAction =
  | "new-search" // 文件→新建搜索 / Ctrl+N
  | "open-selected" // 文件→打开（需有选中项）
  | "locate-selected" // 文件→在资源管理器中显示
  | "copy-path" // 文件→复制路径 / Ctrl+Shift+C
  | "focus-search" // 编辑→查找 / Ctrl+F
  | "toggle-preview" // 视图→预览面板 / Ctrl+P
  | "reset-query" // 搜索→重置查询
  | "show-history" // 搜索→搜索历史
  | "clear-history" // 搜索→清空搜索历史
  | "save-search" // 书签→保存当前搜索 / Ctrl+D
  | "open-prefs" // 工具→选项 / Ctrl+, ——打开模态选项卡片（替代旧 /settings 路由跳转）
  | "open-prefs-indexing" // 快速入门第 5 步→打开选项对话框并跳到「索引」分类
  | "open-prefs-misc" // 工具→我的同义词→打开选项对话框并跳到「杂项」分类（2026-07-07 整页收编）
  | "open-prefs-privacy"; // 工具→隐私与数据→打开选项对话框并跳到「隐私与记录」分类（2026-07-07 整页收编）

const CHANNEL = "locifind:menu";

interface MenuEventDetail {
  action: MenuAction;
}

export function emitMenuAction(action: MenuAction): void {
  if (typeof window === "undefined") return;
  window.dispatchEvent(
    new CustomEvent<MenuEventDetail>(CHANNEL, { detail: { action } }),
  );
}

// 返回 unsubscribe 函数；调用方在 useEffect cleanup 中调即可。
export function onMenuAction(
  handler: (action: MenuAction) => void,
): () => void {
  if (typeof window === "undefined") return () => {};
  const listener = (e: Event) => {
    const ce = e as CustomEvent<MenuEventDetail>;
    if (ce.detail?.action) handler(ce.detail.action);
  };
  window.addEventListener(CHANNEL, listener);
  return () => window.removeEventListener(CHANNEL, listener);
}
