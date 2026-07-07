import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import AboutDialog from "./AboutDialog";
import { emitMenuAction, type MenuAction } from "../lib/menu-events";

// BETA-33 cycle 1 + 2：参考 Everything 菜单栏的纯前端实现。
// cycle 1 = 7 个下拉骨架 + 路由收编 + 关于对话框。
// cycle 2 = 全局快捷键（Ctrl+N/F/P/D/Shift+C/,） + Alt+首字母访问键 + 菜单事件总线接通 SearchView。
// 仍待 cycle 3：列控制 / 排序 / 范围 / 跨语言 / 重建索引 / 模型 / 后端 / 打开日志/数据目录 / 用户手册 / 反馈（plugin-shell）。

type MenuItem =
  | {
      type: "item";
      label: string;
      shortcut?: string;
      disabled?: boolean;
      onClick?: () => void;
    }
  | { type: "separator" };

interface Menu {
  title: string;
  accessKey: string; // Alt+ 首字母，由 keydown 监听绑定
  items: MenuItem[];
}

export default function MenuBar() {
  const navigate = useNavigate();
  const [openIndex, setOpenIndex] = useState<number | null>(null);
  const [showAbout, setShowAbout] = useState(false);
  const barRef = useRef<HTMLDivElement>(null);

  const close = useCallback(() => setOpenIndex(null), []);

  // 点击菜单条外部 / Esc 关闭下拉
  useEffect(() => {
    if (openIndex === null) return;
    const onDoc = (e: MouseEvent) => {
      if (!barRef.current?.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [openIndex, close]);

  const handleExit = useCallback(async () => {
    try {
      await getCurrentWindow().close();
    } catch (err) {
      console.error("[MenuBar] close window failed:", err);
    }
  }, []);

  const go = useCallback(
    (path: string) => {
      navigate(path);
      close();
    },
    [navigate, close],
  );

  const openAbout = useCallback(() => {
    setShowAbout(true);
    close();
  }, [close]);

  const fire = useCallback(
    (action: MenuAction) => {
      emitMenuAction(action);
      close();
    },
    [close],
  );

  const onboardingPath =
    typeof navigator !== "undefined" && /Win/i.test(navigator.platform)
      ? "/onboarding/win"
      : "/onboarding/mac";

  const menus: Menu[] = [
    {
      title: "文件",
      accessKey: "F",
      items: [
        {
          type: "item",
          label: "新建搜索",
          shortcut: "Ctrl+N",
          onClick: () => fire("new-search"),
        },
        {
          type: "item",
          label: "打开",
          shortcut: "Enter",
          onClick: () => fire("open-selected"),
        },
        {
          type: "item",
          label: "在资源管理器中显示",
          shortcut: "Ctrl+Enter",
          onClick: () => fire("locate-selected"),
        },
        {
          type: "item",
          label: "复制路径",
          shortcut: "Ctrl+Shift+C",
          onClick: () => fire("copy-path"),
        },
        { type: "separator" },
        { type: "item", label: "导出结果...", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "退出",
          shortcut: "Alt+F4",
          onClick: handleExit,
        },
      ],
    },
    {
      title: "编辑",
      accessKey: "E",
      items: [
        { type: "item", label: "撤销", shortcut: "Ctrl+Z", disabled: true },
        { type: "item", label: "重做", shortcut: "Ctrl+Y", disabled: true },
        { type: "separator" },
        { type: "item", label: "剪切", shortcut: "Ctrl+X", disabled: true },
        { type: "item", label: "复制", shortcut: "Ctrl+C", disabled: true },
        { type: "item", label: "粘贴", shortcut: "Ctrl+V", disabled: true },
        { type: "item", label: "全选", shortcut: "Ctrl+A", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "查找",
          shortcut: "Ctrl+F",
          onClick: () => fire("focus-search"),
        },
      ],
    },
    {
      title: "视图",
      accessKey: "V",
      items: [
        { type: "item", label: "列...", disabled: true },
        { type: "item", label: "排序方式 ▸", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "预览面板",
          shortcut: "Ctrl+P",
          onClick: () => fire("toggle-preview"),
        },
        { type: "item", label: "状态指示", disabled: true },
        { type: "separator" },
        { type: "item", label: "快捷键提示横条", disabled: true },
      ],
    },
    {
      title: "搜索",
      accessKey: "S",
      items: [
        {
          type: "item",
          label: "重置查询",
          shortcut: "Esc",
          onClick: () => fire("reset-query"),
        },
        { type: "item", label: "搜索范围 ▸", disabled: true },
        { type: "item", label: "跨语言匹配", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "搜索历史...",
          onClick: () => fire("show-history"),
        },
        {
          type: "item",
          label: "清空搜索历史",
          onClick: () => fire("clear-history"),
        },
        { type: "separator" },
        { type: "item", label: "高级语法帮助...", disabled: true },
      ],
    },
    {
      title: "书签",
      accessKey: "B",
      items: [
        {
          type: "item",
          label: "保存当前搜索...",
          shortcut: "Ctrl+D",
          onClick: () => fire("save-search"),
        },
        { type: "item", label: "管理保存的搜索...", disabled: true },
      ],
    },
    {
      title: "工具",
      accessKey: "T",
      items: [
        { type: "item", label: "重建索引", disabled: true },
        { type: "item", label: "索引状态...", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "我的同义词...",
          onClick: () => fire("open-prefs-misc"),
        },
        {
          type: "item",
          label: "隐私与数据...",
          onClick: () => fire("open-prefs-privacy"),
        },
        {
          type: "item",
          label: "本机 MCP 服务...",
          onClick: () => fire("open-prefs-mcp"),
        },
        { type: "separator" },
        { type: "item", label: "模型 ▸", disabled: true },
        { type: "item", label: "搜索后端 ▸", disabled: true },
        { type: "separator" },
        { type: "item", label: "打开日志目录", disabled: true },
        { type: "item", label: "打开数据目录", disabled: true },
        { type: "separator" },
        {
          type: "item",
          label: "选项...",
          // BETA-33 cycle 3 v4：从 Ctrl+, 换到 Ctrl+; 绕开 Sogou 中文 IME 拦截。
          // Sogou 拿 `,` 做标点/切换键、`;` 不拿。VSCode 惯例也用 Ctrl+; 系。
          shortcut: "Ctrl+;",
          onClick: () => fire("open-prefs"),
        },
      ],
    },
    {
      title: "帮助",
      accessKey: "H",
      items: [
        {
          type: "item",
          label: "快速入门...",
          onClick: () => go(onboardingPath),
        },
        { type: "item", label: "键盘快捷键...", disabled: true },
        { type: "item", label: "用户手册", disabled: true },
        { type: "separator" },
        { type: "item", label: "反馈与报告 bug", disabled: true },
        { type: "separator" },
        { type: "item", label: "关于 LociFind...", onClick: openAbout },
      ],
    },
  ];

  // BETA-33 cycle 2：全局快捷键 + Alt+首字母访问键。
  // 单一 useEffect 监听 window keydown，分发到对应 MenuAction 或菜单展开。
  // 注意：Alt+F4 由系统接管、Alt+空格是系统窗口菜单；这里只处理 Alt+字母 中
  // 与菜单条 accessKey 匹配的组合，其余 Alt 组合穿透给系统/webview。
  useEffect(() => {
    const accessKeyToIndex = new Map<string, number>(
      menus.map((m, i) => [m.accessKey, i]),
    );

    const onKeyDown = (e: KeyboardEvent) => {
      // Alt + 首字母：展开/折叠对应下拉
      if (e.altKey && !e.ctrlKey && !e.metaKey && !e.shiftKey) {
        const key = e.key.toUpperCase();
        const idx = accessKeyToIndex.get(key);
        if (idx !== undefined) {
          e.preventDefault();
          setOpenIndex((prev) => (prev === idx ? null : idx));
        }
        return;
      }

      // Ctrl/Cmd + 组合键
      const cmdLike = e.ctrlKey || e.metaKey;
      if (cmdLike && !e.altKey) {
        const k = e.key.toLowerCase();
        // Ctrl+Shift+C：复制路径
        if (e.shiftKey && k === "c") {
          e.preventDefault();
          emitMenuAction("copy-path");
          return;
        }
        if (e.shiftKey) return; // 其余 Ctrl+Shift+X 不拦截
        switch (k) {
          case "n":
            e.preventDefault();
            emitMenuAction("new-search");
            return;
          case "f":
            e.preventDefault();
            emitMenuAction("focus-search");
            return;
          case "p":
            e.preventDefault();
            emitMenuAction("toggle-preview");
            return;
          case "d":
            e.preventDefault();
            emitMenuAction("save-search");
            return;
          // BETA-33 cycle 3 v4：Ctrl+, 换 Ctrl+; 绕 Sogou IME。老 Ctrl+, 保留作
          // fallback（英文 IME / 装机器无中文 IME 时仍能用），双键同触发 open-prefs。
          case ";":
          case ",":
            e.preventDefault();
            emitMenuAction("open-prefs");
            return;
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
    // accessKeyToIndex 来自静态 menus 结构，accessKey 字符串稳定；
    // navigate 是 useNavigate 稳定 ref。其他依赖（如 fire/go/openAbout）
    // 此处不需要——快捷键直接 emit / navigate，不经过菜单 onClick 闭包。
  }, [navigate]);

  const onTitleClick = (idx: number) => {
    setOpenIndex((prev) => (prev === idx ? null : idx));
  };

  const onTitleHover = (idx: number) => {
    if (openIndex !== null && openIndex !== idx) setOpenIndex(idx);
  };

  return (
    <>
      <div className="menu-bar" ref={barRef}>
        {menus.map((menu, idx) => {
          const open = openIndex === idx;
          return (
            <div className="menu-bar-item" key={menu.title}>
              <button
                type="button"
                className={`menu-bar-title${open ? " open" : ""}`}
                onClick={() => onTitleClick(idx)}
                onMouseEnter={() => onTitleHover(idx)}
                aria-haspopup="menu"
                aria-expanded={open}
              >
                {menu.title}
                <span className="menu-access-key">({menu.accessKey})</span>
              </button>
              {open && (
                <ul className="menu-dropdown" role="menu">
                  {menu.items.map((item, i) => {
                    if (item.type === "separator") {
                      return (
                        <li key={`sep-${i}`} className="menu-separator" />
                      );
                    }
                    return (
                      <li
                        key={`item-${i}`}
                        className={`menu-item${item.disabled ? " disabled" : ""}`}
                        role="menuitem"
                        aria-disabled={item.disabled || undefined}
                        onClick={() => {
                          if (item.disabled) return;
                          item.onClick?.();
                        }}
                      >
                        <span className="menu-item-label">{item.label}</span>
                        {item.shortcut && (
                          <span className="menu-item-shortcut">
                            {item.shortcut}
                          </span>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          );
        })}
      </div>
      {showAbout && <AboutDialog onClose={() => setShowAbout(false)} />}
    </>
  );
}
