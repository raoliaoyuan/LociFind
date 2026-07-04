import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ModelDownloadStep } from "./ModelDownloadStep";
// BETA-33 cycle 9：EmbedStatus / ModelStatusJson 类型 + embedStatusLine 文案改从
// 单一信源引入（原与 SettingsPage / StatusIndicator 三处复制，详见 lib/model-status.ts）。
import {
  EmbedStatus,
  ModelStatusJson,
  embedStatusLine,
} from "../lib/model-status";
// BETA-33 cycle 9：AppSettings 类型 + 加载/保存/未保存判定流收拢到 useAppSettings
// （原与 SettingsPage 复制 ~120 行；旧 /settings 路由 + SettingsPage 已随本 cycle 删除）。
import { AppSettings, useAppSettings } from "../hooks/useAppSettings";

// BETA-33 cycle 3：参考 Everything「选项」对话框做的模态卡片版「选项」。
// 左侧分类树（4 类）+ 右侧表单 + 底部 取消 / 应用 / 确定。
// cycle 9 起本对话框是设置的唯一 UI 入口（旧 /settings 路由已删）。

interface AuditEntry {
  timestamp: string;
  operation: string;
  source_paths: string[];
  destination: string | null;
  new_name: string | null;
  result: string;
  error: string | null;
}

/** cycle 7-a：索引阶段（后端 IndexPhase enum snake_case serialize）。 */
type IndexPhase = "music_discovery" | "music_scan" | "doc" | "image";

interface IndexStatus {
  indexing: boolean;
  last_indexed: string | null;
  last_summary: string | null;
  /** cycle 6 v4：正在扫描的目录（bridge 更新为当前文件的父目录）；非索引中为 null。
   *  UI 文案叫「当前目录」（不是「索引根」，语义上是文件父目录、非配置 root）。 */
  current_root: string | null;
  /** cycle 6 v4：FTS 累计进度 [scanned, indexed]；非索引中为 null。 */
  fts_progress: [number, number] | null;
  /** cycle 7-a：当前索引阶段（UI phase chip 用）；非索引中为 null。 */
  current_phase: IndexPhase | null;
  semantic_indexing: boolean;
  semantic_progress: [number, number] | null;
  semantic_summary: string | null;
  /** cycle 9：全库索引总数 [音乐, 文档, 图片]（与「本地索引」行 last_summary 数字同源）。
   *  概貌是"当前生效目录内"口径、此为"全库"口径——两者可合法不一致（仅移除目录保留
   *  的记录 / 旧配置的记录仍在库），差值时概貌卡显式提示来源。 */
  db_totals: [number, number, number] | null;
}

/** cycle 7-a：把 IndexPhase 映射到中文文案 + emoji chip。 */
function phaseChipLabel(phase: IndexPhase): string {
  switch (phase) {
    case "music_discovery":
      return "🎵 扫描音乐（Everything 全盘发现，请稍候）";
    case "music_scan":
      return "🎵 扫描音乐目录";
    case "doc":
      return "📄 扫描文档";
    case "image":
      return "🖼 扫描图片";
  }
}

/** `reindex` / `reindex_root` 命令的返回统计（cycle 7-c 单目录重扫与全量共用）。 */
interface ReindexStats {
  music_added: number;
  music_updated: number;
  doc_added: number;
  doc_updated: number;
  image_added: number;
  image_updated: number;
}

function reindexDoneMsg(s: ReindexStats): string {
  return `完成：音乐 新增 ${s.music_added} / 更新 ${s.music_updated}，文档 新增 ${s.doc_added} / 更新 ${s.doc_updated}，图片 新增 ${s.image_added} / 更新 ${s.image_updated}`;
}

/** BETA-33 cycle 5：每个索引 root 的分类统计。后端 `get_index_overview` 返回。 */
interface RootIndexOverview {
  path: string;
  is_default: boolean;
  doc_count: number;
  image_count: number;
  music_count: number;
  last_indexed_time: string | null;
}

/** BETA-40：一条「未能索引的文件」留痕。后端 `get_extraction_failures` 返回（按时间倒序）。 */
interface ExtractionFailure {
  path: string;
  reason: string;
  failed_time: string | null;
}

/**
 * cycle 7-c：应用内二次确认 modal 的请求描述。
 *
 * **为什么不用 window.confirm**：wry/WebView2 生产装机版里 `window.confirm` 不显示
 * 任何对话框且守卫直接放行（v0.9.7 真机验证实锤、cycle 7-a 关闭守卫因此失效），
 * 二次确认类交互必须用 in-DOM modal。
 */
interface ConfirmOption {
  key: string;
  label: string;
  hint?: string;
}
interface ConfirmRequest {
  title: string;
  message: string;
  confirmLabel: string;
  /** 确认按钮用红色 danger 样式（默认蓝色 primary）。 */
  danger?: boolean;
  /** 非空 = 单选组；onConfirm 收到选中项 key。 */
  options?: ConfirmOption[];
  defaultOption?: string;
  onConfirm: (optionKey: string | null) => void;
}

/** cycle 7-c：通用 in-DOM 确认弹窗（关闭守卫 + 移除目录二次确认共用）。 */
function ConfirmModal({
  req,
  onCancel,
}: {
  req: ConfirmRequest;
  onCancel: () => void;
}) {
  const [selected, setSelected] = useState<string | null>(
    req.defaultOption ?? req.options?.[0]?.key ?? null,
  );
  return (
    <div
      className="prefs-confirm-backdrop"
      onClick={(e) => {
        // 阻断冒泡：不让点击穿透到外层 prefs-backdrop 触发关闭守卫。
        e.stopPropagation();
        onCancel();
      }}
    >
      <div
        className="prefs-confirm-card"
        role="alertdialog"
        aria-modal="true"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="prefs-confirm-title">{req.title}</h3>
        <p className="prefs-confirm-msg">{req.message}</p>
        {req.options?.map((o) => (
          <label key={o.key} className="prefs-confirm-option">
            <input
              type="radio"
              name="prefs-confirm-opt"
              checked={selected === o.key}
              onChange={() => setSelected(o.key)}
            />
            <span>
              {o.label}
              {o.hint && <em className="prefs-confirm-hint">{o.hint}</em>}
            </span>
          </label>
        ))}
        <div className="prefs-confirm-actions">
          <button type="button" className="prefs-btn" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className={`prefs-btn ${req.danger ? "danger" : "primary"}`}
            onClick={() => req.onConfirm(selected)}
          >
            {req.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

type Category = "general" | "semantic" | "indexing" | "privacy";

const CATEGORIES: { key: Category; label: string }[] = [
  { key: "general", label: "常规" },
  { key: "semantic", label: "语义召回" },
  { key: "indexing", label: "索引" },
  { key: "privacy", label: "隐私与记录" },
];

interface Props {
  onClose: () => void;
  /** 打开时默认选中的分类。快速入门第 5 步用（跳直接到「索引」）。缺省为 "general"。 */
  initialCategory?: Category;
}

export default function PreferencesDialog({
  onClose,
  initialCategory,
}: Props) {
  const [active, setActive] = useState<Category>(initialCategory ?? "general");
  // cycle 9：settings 加载/编辑/保存流（含 initialSettings 快照 + 未保存判定）收拢进 hook。
  const {
    settings,
    setSettings,
    initialSettings,
    hasUnsavedChanges,
    save,
    saving,
    message,
    setMessage,
  } = useAppSettings();
  const [reindexing, setReindexing] = useState(false);
  const [reindexMsg, setReindexMsg] = useState("");
  const [auditLog, setAuditLog] = useState<AuditEntry[]>([]);
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [modelStatus, setModelStatus] = useState<ModelStatusJson | null>(null);
  const [embedStatus, setEmbedStatus] = useState<EmbedStatus | null>(null);
  const [effectiveRoots, setEffectiveRoots] = useState<string[] | null>(null);
  // BETA-33 cycle 5：每 root 的分类统计（doc / image / music + 上次索引）。
  const [indexOverview, setIndexOverview] = useState<RootIndexOverview[] | null>(
    null,
  );
  // BETA-40：文件级提取失败留痕（「未能索引的文件」清单）。
  const [extractionFailures, setExtractionFailures] = useState<
    ExtractionFailure[] | null
  >(null);
  // cycle 7-a：picker 后 flash 高亮的路径（1.5s 后清空、CSS animation 触发）。
  const [flashPath, setFlashPath] = useState<string | null>(null);
  const flashTimerRef = useRef<number | null>(null);
  // cycle 7-c：应用内二次确认弹窗（关闭守卫 + 移除目录共用；window.confirm 在
  // wry/WebView2 生产环境是 no-op、不可用）。
  const [confirmReq, setConfirmReq] = useState<ConfirmRequest | null>(null);

  // 初始加载（settings 的加载由 useAppSettings 承担）。
  useEffect(() => {
    invoke<AuditEntry[]>("get_audit_log").then(setAuditLog).catch(console.error);
    invoke<IndexStatus>("get_index_status")
      .then(setIndexStatus)
      .catch(console.error);
    invoke<ExtractionFailure[]>("get_extraction_failures")
      .then(setExtractionFailures)
      .catch(console.error);
  }, []);

  // 索引状态轮询（2s）
  useEffect(() => {
    const t = setInterval(() => {
      invoke<IndexStatus>("get_index_status")
        .then(setIndexStatus)
        .catch(() => {});
    }, 2000);
    return () => clearInterval(t);
  }, []);

  // 模型状态轮询（3s）
  useEffect(() => {
    let alive = true;
    const poll = () =>
      invoke<ModelStatusJson>("get_model_status")
        .then((s) => {
          if (alive) setModelStatus(s);
        })
        .catch(() => {});
    poll();
    const t = setInterval(poll, 3000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);

  // embedding 状态轮询（3s）
  useEffect(() => {
    let alive = true;
    const poll = () =>
      invoke<EmbedStatus>("embedding_model_status")
        .then((s) => {
          if (alive) setEmbedStatus(s);
        })
        .catch(() => {});
    poll();
    const t = setInterval(poll, 3000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);

  // 跟随 settings.index_roots / include_system_defaults 重 fetch effectiveRoots + indexOverview
  useEffect(() => {
    if (!settings) return;
    invoke<string[]>("get_effective_index_roots", {
      indexRoots: settings.index_roots,
      includeSystemDefaults: settings.include_system_defaults,
    })
      .then(setEffectiveRoots)
      .catch(console.error);
    // cycle 5：拉每 root 的分类统计。reindex 完成后（indexStatus.indexing false）
    // 触发也刷一次——见下 useEffect。
    invoke<RootIndexOverview[]>("get_index_overview", {
      indexRoots: settings.index_roots,
      includeSystemDefaults: settings.include_system_defaults,
    })
      .then(setIndexOverview)
      .catch(console.error);
  }, [settings?.index_roots, settings?.include_system_defaults]);

  // BETA-33 cycle 5：reindex 从 indexing=true 转 false 时（一次全量刚跑完）重 fetch overview。
  // cycle 7-a：Codex §10 数据源统一——强刷 indexStatus 让顶部概貌"上次索引"文案同步更新，
  // 避免出现"reindex 完成后 UI 仍显示 N 分钟前"的口径不一致。
  const prevIndexing = useRef(false);
  useEffect(() => {
    const now = indexStatus?.indexing ?? false;
    if (prevIndexing.current && !now && settings) {
      invoke<RootIndexOverview[]>("get_index_overview", {
        indexRoots: settings.index_roots,
        includeSystemDefaults: settings.include_system_defaults,
      })
        .then(setIndexOverview)
        .catch(console.error);
      // 强刷 indexStatus：last_indexed 应该刚被后端 apply_reindex_result 填、拉过来给 UI
      invoke<IndexStatus>("get_index_status")
        .then(setIndexStatus)
        .catch(console.error);
      // BETA-40：失败留痕随本轮 reindex 增删（成功重扫 / 磁盘删除自动清除），同步刷新。
      invoke<ExtractionFailure[]>("get_extraction_failures")
        .then(setExtractionFailures)
        .catch(console.error);
    }
    prevIndexing.current = now;
  }, [
    indexStatus?.indexing,
    settings?.index_roots,
    settings?.include_system_defaults,
  ]);

  // cycle 7-a：clean up flash timer on unmount
  useEffect(() => {
    return () => {
      if (flashTimerRef.current !== null) {
        window.clearTimeout(flashTimerRef.current);
      }
    };
  }, []);

  // cycle 7-a：包装 onClose——未保存改动时弹二次确认（hasUnsavedChanges 由 hook 提供）。
  // cycle 7-c：改用 in-DOM ConfirmModal——window.confirm 在 wry/WebView2 生产环境
  // 不弹窗直接放行（v0.9.7 真机验证发现守卫完全失效），不可用。
  const handleCloseWithGuard = () => {
    if (hasUnsavedChanges) {
      setConfirmReq({
        title: "放弃未保存的改动？",
        message: "你有未保存的改动，关闭对话框将放弃这些改动。",
        confirmLabel: "放弃改动并关闭",
        danger: true,
        onConfirm: () => {
          setConfirmReq(null);
          onClose();
        },
      });
      return;
    }
    onClose();
  };

  // Esc 关闭：走 React onKeyDown 事件冒泡 + dialog root 自动获焦。
  // BETA-33 cycle 3 v2 hotfix：window/document addEventListener 在 Tauri WebView2 里 Esc 未触发
  //（cycle 3 v1 试 useRef + window + capture 仍不响应）；换成 React 合成事件挂在 dialog root、
  // 挂载后 focus() 让键盘事件必有 target 且冒泡到 root、绕开 native 事件层的疑难。
  const dialogRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    dialogRef.current?.focus();
  }, []);

  // cycle 9：保存核心流在 hook（快照重置 + message 3s 自清）；此处只叠加本组件
  // 特有的收尾（清 picker flash 高亮）。
  const handleApply = async (): Promise<boolean> => {
    const ok = await save();
    if (ok) setFlashPath(null);
    return ok;
  };

  const handleOk = async () => {
    const ok = await handleApply();
    if (ok) onClose();
  };

  const handleReindex = async () => {
    setReindexing(true);
    setReindexMsg("正在索引音乐、文档与图片目录…");
    try {
      const s = await invoke<ReindexStats>("reindex");
      setReindexMsg(reindexDoneMsg(s));
    } catch (err) {
      setReindexMsg(`索引失败: ${err}`);
    } finally {
      setReindexing(false);
    }
  };

  // cycle 7-c：单目录重扫（RootRow「重扫」按钮）。exclude / root_excludes 仍从已保存
  // settings 生效（后端 perform_reindex_for_roots 只 override roots）。
  const handleReindexRoot = async (root: string) => {
    setReindexing(true);
    setReindexMsg(`正在重扫 ${root} …`);
    try {
      const s = await invoke<ReindexStats>("reindex_root", { root });
      setReindexMsg(reindexDoneMsg(s));
    } catch (err) {
      setReindexMsg(`重扫失败: ${err}`);
    } finally {
      setReindexing(false);
    }
  };

  // cycle 7-c：在系统文件管理器中打开目录。复用既有 open_path 命令
  // （FileActionTool 策略 + audit 口径一致，Windows 走 cmd start / macOS 走 open）。
  const handleOpenRoot = async (path: string) => {
    try {
      await invoke("open_path", { path });
    } catch (err) {
      setMessage(`打开目录失败: ${err}`);
      setTimeout(() => setMessage(""), 5000);
    }
  };

  // cycle 7-c：从未保存 settings 中移除 root（同步清该 root 的 root_excludes、不留孤儿）。
  const removeRootFromSettings = (rootPath: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      index_roots: settings.index_roots.filter((p) => p !== rootPath),
      root_excludes: settings.root_excludes.filter(
        (re) => re.root !== rootPath,
      ),
    });
  };

  // cycle 7-c（Codex SUGGEST 8）：移除目录二次确认——区分「仅移除配置」与「移除并清除
  // 索引记录」，文案明确**不删除磁盘文件**。清除走 purge_root_from_db（存储层 SQL、
  // FTS 同步删、向量外键级联）；清除失败时不移除配置，避免"以为清了其实没清"。
  const requestRemoveRoot = (path: string) => {
    setConfirmReq({
      title: "移除索引目录",
      message: `${path}\n以下操作只影响 LociFind 的数据库缓存，不会删除磁盘上的任何原文件。`,
      confirmLabel: "确定",
      options: [
        {
          key: "keep",
          label: "仅从索引配置移除",
          hint: "保留数据库缓存，重新添加可复用旧记录",
        },
        {
          key: "purge",
          label: "移除并清除索引记录",
          hint: "同时删除该目录下的文档 / 图片 / 音乐索引缓存（不删原文件）",
        },
      ],
      defaultOption: "keep",
      onConfirm: (opt) => {
        setConfirmReq(null);
        void (async () => {
          if (opt === "purge") {
            try {
              const s = await invoke<{
                doc_deleted: number;
                music_deleted: number;
              }>("purge_root_from_db", { root: path });
              setMessage(
                `已清除索引记录：文档/图片 ${s.doc_deleted} 条 · 音乐 ${s.music_deleted} 条（磁盘文件未动）`,
              );
              setTimeout(() => setMessage(""), 5000);
            } catch (err) {
              setMessage(`清除索引记录失败: ${err}`);
              setTimeout(() => setMessage(""), 5000);
              return;
            }
          }
          removeRootFromSettings(path);
        })();
      },
    });
  };

  const handleClearAuditLog = async () => {
    try {
      await invoke("clear_audit_log");
      setAuditLog([]);
    } catch (err) {
      console.error(err);
    }
  };

  const indexStatusLine = useMemo(() => {
    if (!indexStatus) return "状态加载中…";
    if (indexStatus.indexing) {
      // cycle 7-a：先显示 phase chip（帮 Everything 全盘发现"卡在 0·0"的场景解释状态）、
      // 再显示当前目录（文件父目录、非配置 root）+ 累计进度。
      const parts: string[] = [];
      if (indexStatus.current_phase) {
        parts.push(phaseChipLabel(indexStatus.current_phase));
      }
      if (indexStatus.current_root) {
        parts.push(`📁 当前目录：${indexStatus.current_root}`);
      }
      if (indexStatus.fts_progress) {
        const [scanned, indexed] = indexStatus.fts_progress;
        parts.push(
          `已扫描 ${scanned.toLocaleString()} · 已入库 ${indexed.toLocaleString()}`,
        );
      }
      const detail = parts.length > 0 ? parts.join("　") : "扫描准备中…";
      return `⏳ 正在索引：${detail}`;
    }
    if (indexStatus.last_indexed) {
      return `上次索引: ${new Date(indexStatus.last_indexed).toLocaleString()}${indexStatus.last_summary ? `（${indexStatus.last_summary}）` : ""}`;
    }
    if (indexStatus.last_summary) {
      return `当前索引: ${indexStatus.last_summary}`;
    }
    return "尚未索引";
  }, [indexStatus]);

  return (
    <div
      className="prefs-backdrop"
      onClick={handleCloseWithGuard}
      onKeyDownCapture={(e) => {
        if (e.key === "Escape" || e.key === "Esc") {
          e.preventDefault();
          e.stopPropagation();
          // cycle 7-c：确认弹窗打开时 Esc 只关弹窗（= 取消），不触发外层关闭守卫。
          if (confirmReq) {
            setConfirmReq(null);
            return;
          }
          handleCloseWithGuard();
        }
      }}
    >
      <div
        className="prefs-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="prefs-title"
        ref={dialogRef}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="prefs-header">
          <h2 id="prefs-title" className="prefs-title">
            选项
          </h2>
          <button
            type="button"
            className="prefs-close-x"
            onClick={handleCloseWithGuard}
            aria-label="关闭"
            title="关闭"
          >
            ×
          </button>
        </div>

        <div className="prefs-body">
          <nav className="prefs-categories" aria-label="分类">
            <ul>
              {CATEGORIES.map((c) => (
                <li
                  key={c.key}
                  className={`prefs-category${active === c.key ? " active" : ""}`}
                  onClick={() => setActive(c.key)}
                  role="tab"
                  aria-selected={active === c.key}
                >
                  {c.label}
                </li>
              ))}
            </ul>
          </nav>

          <div className="prefs-pane" role="tabpanel">
            {!settings ? (
              <p style={{ color: "#888" }}>加载中…</p>
            ) : active === "general" ? (
              <GeneralPane
                settings={settings}
                setSettings={setSettings}
                modelStatus={modelStatus}
              />
            ) : active === "semantic" ? (
              <SemanticPane
                settings={settings}
                setSettings={setSettings}
                embedStatus={embedStatus}
                onEmbedReload={() =>
                  invoke<EmbedStatus>("embedding_model_status")
                    .then(setEmbedStatus)
                    .catch(console.error)
                }
              />
            ) : active === "indexing" ? (
              <IndexingPane
                settings={settings}
                setSettings={setSettings}
                initialIndexRoots={initialSettings?.index_roots ?? []}
                effectiveRoots={effectiveRoots}
                indexOverview={indexOverview}
                indexStatus={indexStatus}
                indexStatusLine={indexStatusLine}
                extractionFailures={extractionFailures}
                semanticLine={
                  indexStatus &&
                  (indexStatus.semantic_indexing ||
                    indexStatus.semantic_summary)
                    ? indexStatus.semantic_indexing
                      ? `🧠 语义索引中${indexStatus.semantic_progress ? ` ${indexStatus.semantic_progress[0]}/${indexStatus.semantic_progress[1]}` : "…"}`
                      : `🧠 ${indexStatus.semantic_summary ?? ""}`
                    : null
                }
                reindexing={reindexing}
                reindexMsg={reindexMsg}
                onReindex={handleReindex}
                onReindexRoot={handleReindexRoot}
                onOpenRoot={handleOpenRoot}
                onRequestRemoveRoot={requestRemoveRoot}
                onPickMessage={(m) => {
                  setMessage(m);
                  setTimeout(() => setMessage(""), 5000);
                }}
                flashPath={flashPath}
                onFlash={(path) => {
                  if (flashTimerRef.current !== null) {
                    window.clearTimeout(flashTimerRef.current);
                  }
                  setFlashPath(path);
                  flashTimerRef.current = window.setTimeout(() => {
                    setFlashPath(null);
                    flashTimerRef.current = null;
                  }, 1600);
                }}
              />
            ) : (
              <PrivacyPane
                auditLog={auditLog}
                onReload={() =>
                  invoke<AuditEntry[]>("get_audit_log")
                    .then(setAuditLog)
                    .catch(console.error)
                }
                onClear={handleClearAuditLog}
              />
            )}
          </div>
        </div>

        <div className="prefs-footer">
          <span
            className={`prefs-msg${message.includes("失败") ? " err" : message ? " ok" : hasUnsavedChanges ? " warn" : ""}`}
          >
            {message
              ? message
              : hasUnsavedChanges
                ? "⚠ 你有未保存的改动，点「应用」或「确定」生效"
                : ""}
          </span>
          <div className="prefs-actions">
            <button
              type="button"
              className="prefs-btn"
              onClick={handleCloseWithGuard}
              disabled={saving}
            >
              取消
            </button>
            <button
              type="button"
              className="prefs-btn"
              onClick={handleApply}
              disabled={saving || !settings || !hasUnsavedChanges}
              title={hasUnsavedChanges ? "" : "没有未保存改动"}
            >
              {saving ? "保存中…" : "应用"}
            </button>
            <button
              type="button"
              className="prefs-btn primary"
              onClick={handleOk}
              disabled={saving || !settings}
            >
              确定
            </button>
          </div>
        </div>

        {/* cycle 7-c：应用内二次确认弹窗（关闭守卫 / 移除目录）。渲染在 dialog 内部
            （dialog onClick 已 stopPropagation），backdrop 点击只关弹窗不穿透。 */}
        {confirmReq && (
          <ConfirmModal req={confirmReq} onCancel={() => setConfirmReq(null)} />
        )}
      </div>
    </div>
  );
}

// ---------- 分类面板 ----------

function GeneralPane({
  settings,
  setSettings,
  modelStatus,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  modelStatus: ModelStatusJson | null;
}) {
  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">全局唤起快捷键</label>
        <input
          type="text"
          className="prefs-input"
          value={settings.global_shortcut}
          onChange={(e) =>
            setSettings({ ...settings, global_shortcut: e.target.value })
          }
          disabled
        />
        <p className="prefs-hint">当前版本暂不支持修改快捷键。</p>
      </div>

      <div className="prefs-field">
        <label className="prefs-checkbox">
          <input
            type="checkbox"
            checked={settings.enable_model_fallback}
            onChange={(e) =>
              setSettings({
                ...settings,
                enable_model_fallback: e.target.checked,
              })
            }
          />
          启用模型 Fallback（可选，仅在解析复杂查询时使用）
        </label>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">生成模型（Qwen3-0.6B，可选）</label>
        <p className="prefs-hint">
          仅在 parser 解不出复杂多条件自然语言（如「上周从张三收到的关于 Q3 报表的 PDF」）时才会触发；
          日常关键词与语义召回不需要它。装了 parser 覆盖率从 88% → ~95%+。
        </p>
        {modelStatus && (
          <p
            className="prefs-status"
            style={{
              color: modelStatus.state === "ready" ? "#0a0" : undefined,
            }}
          >
            状态：{modelStatus.detail}
          </p>
        )}
        {/* BETA-33 cycle 3 v4：not_found 时嵌入一键下载（与 embedding 同款） */}
        {modelStatus?.state === "not_found" && (
          <ModelDownloadStep
            compact
            kind="generation"
            onComplete={() => {
              // 触发一次 get_model_status 重读、让状态从 not_found 变 ready。
              // 3s 轮询也会自动更新，但立即触发一次响应更快。
              void invoke<ModelStatusJson>("get_model_status");
            }}
          />
        )}
      </div>

      <div className="prefs-field">
        <label className="prefs-label">模型路径覆盖（留空用默认）</label>
        <input
          type="text"
          className="prefs-input"
          value={settings.model_path ?? ""}
          onChange={(e) =>
            setSettings({ ...settings, model_path: e.target.value || null })
          }
          placeholder="默认：数据目录/models/qwen3-0.6b-q4_k_m.gguf"
        />
        <p className="prefs-hint">
          修改路径后需重启应用生效。留空时默认从上方路径查找。
        </p>
      </div>
    </div>
  );
}

function SemanticPane({
  settings,
  setSettings,
  embedStatus,
  onEmbedReload,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  embedStatus: EmbedStatus | null;
  onEmbedReload: () => void;
}) {
  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">语义召回（按意思找到）</label>
        {embedStatus ? (
          (() => {
            const { text, color } = embedStatusLine(embedStatus);
            return (
              <p style={{ fontSize: "13px", color, margin: 0 }}>{text}</p>
            );
          })()
        ) : (
          <p className="prefs-status">语义召回：状态获取中…</p>
        )}
        <p className="prefs-hint">
          语义召回让「找放假相关的通知」等按含义命中文件，支持跨语言模糊匹配。
        </p>
        {embedStatus?.state === "not_found" && (
          <ModelDownloadStep compact onComplete={onEmbedReload} />
        )}
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          语义相似度下限（0–1，越高越严，默认 0.30）
        </label>
        <input
          type="number"
          min={0}
          max={1}
          step={0.05}
          className="prefs-input prefs-input-small"
          value={settings.semantic_similarity_floor ?? 0.3}
          onChange={(e) =>
            setSettings({
              ...settings,
              semantic_similarity_floor:
                e.target.value === "" ? null : parseFloat(e.target.value),
            })
          }
        />
        <p className="prefs-hint">
          语义结果低于此 cosine 分数将被过滤；改后重新搜索即生效。
        </p>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          语义臂权重（融合 FTS vs 向量，越高越偏向量，默认 10.0）
        </label>
        <input
          type="number"
          min={0.5}
          max={50}
          step={0.5}
          className="prefs-input prefs-input-small"
          value={settings.semantic_weight ?? 10.0}
          onChange={(e) =>
            setSettings({
              ...settings,
              semantic_weight:
                e.target.value === "" ? null : parseFloat(e.target.value),
            })
          }
        />
        <p className="prefs-hint">
          融合时语义臂（按意思）相对 FTS 臂（关键词）的权重。0.5–50；改后重新搜索即生效。
        </p>
      </div>
    </div>
  );
}

/**
 * BETA-33 cycle 5：把 UTC rfc3339 时间转成本地口语（"5 分钟前" / "今天 15:32" / "2026-06-30"）。
 * 输入无效 → 空串。
 */
function formatIndexTime(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const diffMs = now.getTime() - d.getTime();
  const diffMin = Math.round(diffMs / 60_000);
  if (diffMin < 1) return "刚刚";
  if (diffMin < 60) return `${diffMin} 分钟前`;
  const diffH = Math.round(diffMin / 60);
  if (diffH < 24 && d.getDate() === now.getDate()) {
    return `今天 ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }
  if (diffH < 48) return "昨天";
  const diffD = Math.round(diffH / 24);
  if (diffD < 7) return `${diffD} 天前`;
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/**
 * BETA-33 cycle 5：单个索引 root 行。左路径 + 系统默认 tag + 中间分类统计 + 右移除按钮。
 * `overview` = null 时统计显示"…"（尚未加载）。`onRemove` = null 时不显示移除按钮
 * （系统默认目录用户不能"移除"、只能通过"+ 添加目录"覆盖）。
 *
 * cycle 7-a：
 * - `isPending`：picker 加入但未保存的自定义 root，显示 `⏳ 待应用` 琥珀 badge。
 * - `flash`：picker 后 1.5s CSS flash 高亮 + scrollIntoView（消除"选了没反应"错觉）。
 */
function RootRow({
  path,
  isSystemDefault,
  overview,
  onRemove,
  isPending,
  flash,
  excludePatterns,
  onUpdateExcludes,
  onOpenDir,
  onRescan,
  rescanDisabled,
}: {
  path: string;
  isSystemDefault: boolean;
  overview: RootIndexOverview | null;
  onRemove: (() => void) | null;
  isPending?: boolean;
  flash?: boolean;
  /** cycle 7-b：该 root 的 per-root 子路径 exclude patterns（默认空）。 */
  excludePatterns?: string[];
  /** cycle 7-b：更新 patterns 回调；null = 只读（例如 fallback、无 root_excludes wiring）。 */
  onUpdateExcludes?: ((patterns: string[]) => void) | null;
  /** cycle 7-c：在系统文件管理器中打开该目录。 */
  onOpenDir?: () => void;
  /** cycle 7-c：单目录重扫；null = 不显示（如待应用的 pending root，排除配置尚未保存）。 */
  onRescan?: (() => void) | null;
  /** cycle 7-c：重扫按钮禁用（全局索引中）。 */
  rescanDisabled?: boolean;
}) {
  const stats = overview
    ? [
        `文档 ${overview.doc_count.toLocaleString()}`,
        `图片 ${overview.image_count.toLocaleString()}`,
        `音乐 ${overview.music_count.toLocaleString()}`,
      ].join(" · ")
    : "…";
  const lastIndexed = overview?.last_indexed_time
    ? formatIndexTime(overview.last_indexed_time)
    : null;
  const rowRef = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);
  const [patternDraft, setPatternDraft] = useState("");
  useEffect(() => {
    if (flash) {
      rowRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [flash]);
  const cls = [
    "prefs-root-row",
    isPending ? "pending" : "",
    flash ? "flash" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const patterns = excludePatterns ?? [];
  const excludeEditable = onUpdateExcludes != null;
  const addPattern = () => {
    const t = patternDraft.trim();
    if (!t || !onUpdateExcludes) return;
    if (!patterns.includes(t)) {
      onUpdateExcludes([...patterns, t]);
    }
    setPatternDraft("");
  };
  return (
    <>
      <div className={cls} ref={rowRef}>
        <span className={`prefs-root-path${isSystemDefault ? " sys" : ""}`}>
          📂 {path}
        </span>
        {isSystemDefault && <span className="prefs-root-tag">系统默认</span>}
        {isPending && (
          <span className="prefs-root-tag pending" title="picker 加入但未保存">
            ⏳ 待应用
          </span>
        )}
        <span
          className="prefs-root-stats"
          title="该目录下索引条数（文档 · 图片 · 音乐）"
        >
          {stats}
        </span>
        {lastIndexed && (
          <span
            className="prefs-root-time"
            title={`上次索引：${overview?.last_indexed_time ?? ""}`}
          >
            {lastIndexed}
          </span>
        )}
        {excludeEditable && (
          <button
            type="button"
            className={`prefs-btn small${patterns.length > 0 ? " has-excludes" : ""}`}
            onClick={() => setExpanded(!expanded)}
            title="配置该目录下的子路径排除（通配符）"
          >
            {expanded ? "▾" : "▸"} 子路径排除
            {patterns.length > 0 ? ` (${patterns.length})` : ""}
          </button>
        )}
        {onOpenDir && (
          <button
            type="button"
            className="prefs-btn small"
            onClick={onOpenDir}
            title="在系统文件管理器中打开该目录"
          >
            打开
          </button>
        )}
        {onRescan && (
          <button
            type="button"
            className="prefs-btn small"
            onClick={onRescan}
            disabled={rescanDisabled}
            title="只重扫该目录（排除规则仍生效，不影响其他目录）"
          >
            重扫
          </button>
        )}
        {onRemove && (
          <button type="button" className="prefs-btn small" onClick={onRemove}>
            移除
          </button>
        )}
      </div>
      {excludeEditable && expanded && (
        <div className="prefs-root-excludes">
          <p className="prefs-hint">
            相对该目录的通配符：<code>**</code>=任意层，<code>*</code>=单段，
            <code>?</code>=单字符。示例：<code>临时/**</code>、
            <code>**/backup/**</code>、<code>*.old/*</code>。
          </p>
          {patterns.map((p, i) => (
            <div key={i} className="prefs-exclude-row">
              <code>{p}</code>
              <button
                type="button"
                className="prefs-btn small"
                onClick={() => {
                  if (!onUpdateExcludes) return;
                  onUpdateExcludes(patterns.filter((_, j) => j !== i));
                }}
              >
                移除
              </button>
            </div>
          ))}
          <div className="prefs-exclude-add-row">
            <input
              type="text"
              className="prefs-input"
              value={patternDraft}
              onChange={(e) => setPatternDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") addPattern();
              }}
              placeholder="如 临时/** 或 **/backup/**"
            />
            <button
              type="button"
              className="prefs-btn"
              onClick={addPattern}
              disabled={!patternDraft.trim()}
            >
              添加
            </button>
          </div>
        </div>
      )}
    </>
  );
}

function IndexingPane({
  settings,
  setSettings,
  initialIndexRoots,
  effectiveRoots,
  indexOverview,
  indexStatus,
  indexStatusLine,
  extractionFailures,
  semanticLine,
  reindexing,
  reindexMsg,
  onReindex,
  onReindexRoot,
  onOpenRoot,
  onRequestRemoveRoot,
  onPickMessage,
  flashPath,
  onFlash,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  initialIndexRoots: string[];
  effectiveRoots: string[] | null;
  indexOverview: RootIndexOverview[] | null;
  indexStatus: IndexStatus | null;
  indexStatusLine: string;
  /** BETA-40：文件级提取失败留痕（null = 加载中）。 */
  extractionFailures: ExtractionFailure[] | null;
  semanticLine: string | null;
  reindexing: boolean;
  reindexMsg: string;
  onReindex: () => void;
  /** cycle 7-c：单目录重扫。 */
  onReindexRoot: (path: string) => void;
  /** cycle 7-c：文件管理器打开目录。 */
  onOpenRoot: (path: string) => void;
  /** cycle 7-c：移除目录（父组件弹二次确认、可选 purge）。 */
  onRequestRemoveRoot: (path: string) => void;
  onPickMessage: (m: string) => void;
  flashPath: string | null;
  onFlash: (path: string) => void;
}) {
  const [excludeDraft, setExcludeDraft] = useState("");
  // BETA-40：「未能索引的文件」清单折叠态（默认收起，仅显示条数）。
  const [failuresExpanded, setFailuresExpanded] = useState(false);

  const addExclude = () => {
    const t = excludeDraft.trim();
    if (!t) return;
    if (!settings.exclude_globs.includes(t)) {
      setSettings({
        ...settings,
        exclude_globs: [...settings.exclude_globs, t],
      });
    }
    setExcludeDraft("");
  };

  // 按 path 找对应统计（overview 里的顺序 = effectiveRoots 顺序、但按 path 匹配更稳）。
  const overviewOf = (path: string): RootIndexOverview | null =>
    indexOverview?.find((o) => o.path === path) ?? null;

  // 顶部总览合计（跨所有 root）。
  const totalDocs = indexOverview?.reduce((s, o) => s + o.doc_count, 0) ?? 0;
  const totalImages =
    indexOverview?.reduce((s, o) => s + o.image_count, 0) ?? 0;
  const totalMusic = indexOverview?.reduce((s, o) => s + o.music_count, 0) ?? 0;
  const grandTotal = totalDocs + totalImages + totalMusic;
  // cycle 7-a：数据源统一（Codex APPROVED 2 · 选 a）——概貌"上次索引"用 indexOverview.max()、
  // 与「本地索引」区文案一致；避免出现"顶部 Downloads-only 数字 vs 底部全库数字"两套口径。
  const latestTime = indexOverview
    ?.map((o) => o.last_indexed_time)
    .filter((t): t is string => !!t)
    .sort()
    .pop();

  // cycle 9：口径统一明示——概貌是「当前生效目录内」口径、「本地索引」行 last_summary 是
  // 「全库」口径，两者可合法不一致（「仅移除」目录保留的记录 / override 前旧默认目录的
  // 记录仍在库且仍可被搜索命中）。全库 > 概貌合计时显式提示差值来源，不放任两个数字
  // 各说各话。反向（概貌 > 全库，生效目录相互嵌套导致重复计数）不提示、属已知统计特性。
  const dbGrand = indexStatus?.db_totals
    ? indexStatus.db_totals[0] + indexStatus.db_totals[1] + indexStatus.db_totals[2]
    : null;
  const outsideRootsCount =
    dbGrand !== null && indexOverview !== null && dbGrand > grandTotal
      ? dbGrand - grandTotal
      : 0;

  // cycle 7-a：C 修法核心——识别"覆盖语义导致系统默认三夹消失"场景，显示大字醒目提示。
  // 条件：用户加了自定义目录（index_roots 非空）+ 没勾 include_system_defaults + 系统本来有默认目录（effectiveRoots 里没有非自定义项 = 说明被覆盖了）。
  const hasCustomRoots = settings.index_roots.length > 0;
  const showSystemDefaultsOverriddenWarning =
    hasCustomRoots && !settings.include_system_defaults;

  // cycle 7-a：pending 集合——settings.index_roots 里但不在 initialIndexRoots 里 = picker 加入未保存。
  const pendingSet = new Set(
    settings.index_roots.filter((p) => !initialIndexRoots.includes(p)),
  );

  // cycle 7-b：查某 root 对应的 excludePatterns。后端按 normalize_root_key 归一化匹配、
  // 但前端保留 display 形式（跟 settings.index_roots 字符串一致）；简单按等值匹配。
  const excludesFor = (rootPath: string): string[] => {
    return (
      settings.root_excludes.find((re) => re.root === rootPath)?.patterns ?? []
    );
  };
  const updateExcludesFor = (rootPath: string, patterns: string[]) => {
    const others = settings.root_excludes.filter((re) => re.root !== rootPath);
    if (patterns.length === 0) {
      // 空 patterns → 从 root_excludes 里删（避免存空条目）
      setSettings({ ...settings, root_excludes: others });
    } else {
      setSettings({
        ...settings,
        root_excludes: [...others, { root: rootPath, patterns }],
      });
    }
  };
  // cycle 7-c：移除 root 走父组件的二次确认弹窗（onRequestRemoveRoot），
  // 确认后由父组件同步删 root_excludes 条目（不留孤儿）+ 可选 purge 索引记录。

  return (
    <div className="prefs-form">
      {/* BETA-33 cycle 5：顶部概貌卡片——总目录 / 分类分总 / 上次索引 */}
      <div className="prefs-overview-card">
        <div className="prefs-overview-title">索引概貌</div>
        {indexOverview === null ? (
          <p className="prefs-hint">加载中…</p>
        ) : indexOverview.length === 0 ? (
          <p className="prefs-hint err">
            ⚠️ 无生效索引目录（未添加 + 系统未检测到默认音乐/文档/图片目录）。
          </p>
        ) : (
          <div className="prefs-overview-stats">
            <div
              className="prefs-overview-cell"
              title="设置里生效的目录数（含系统默认追加）"
            >
              <div className="prefs-overview-num">{indexOverview.length}</div>
              <div className="prefs-overview-label">生效目录</div>
            </div>
            <div
              className="prefs-overview-cell"
              title="当前生效目录内的条数合计（全库口径见下方「本地索引」行）"
            >
              <div className="prefs-overview-num">{grandTotal.toLocaleString()}</div>
              <div className="prefs-overview-label">总条数</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalDocs.toLocaleString()}</div>
              <div className="prefs-overview-label">文档</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalImages.toLocaleString()}</div>
              <div className="prefs-overview-label">图片</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num">{totalMusic.toLocaleString()}</div>
              <div className="prefs-overview-label">音乐</div>
            </div>
            <div className="prefs-overview-cell">
              <div className="prefs-overview-num prefs-overview-time">
                {latestTime ? formatIndexTime(latestTime) : "尚未"}
              </div>
              <div className="prefs-overview-label">上次索引</div>
            </div>
          </div>
        )}
        {/* cycle 9：全库 vs 概貌口径差显式提示（差值来源 + 清理路径），替代两个数字各说各话。 */}
        {outsideRootsCount > 0 && (
          <p className="prefs-hint" style={{ marginTop: "8px" }}>
            ℹ️ 库内另有 <strong>{outsideRootsCount.toLocaleString()}</strong>{" "}
            条记录在当前生效目录之外（来自已移除的目录或旧配置），搜索仍会命中它们。
            如需清理：移除目录时选「移除并清除索引记录」，或在隐私页清空索引后重建。
          </p>
        )}
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          索引目录（生效 {effectiveRoots?.length ?? 0} 个 = 自定义{" "}
          {settings.index_roots.length} +{" "}
          {effectiveRoots
            ? Math.max(0, effectiveRoots.length - settings.index_roots.length)
            : 0}{" "}
          系统默认）
        </label>
        {/* cycle 7-a：C 修法核心——覆盖语义时大字醒目提示条，说明系统默认被隐藏 + 引导勾选 checkbox。
            这是用户在 v0.9.6 报「添加后不显示」的心智模型冲突真凶：Music/Documents/Pictures 消失让新目录被淹没。 */}
        {showSystemDefaultsOverriddenWarning && (
          <div className="prefs-warn-banner">
            <strong>ℹ️ 已隐藏系统默认目录（音乐 / 文档 / 图片）</strong>
            <p>
              加了自定义目录后，默认使用「覆盖」模式（只扫自定义目录）。
              勾选下方 ✅ 后可**同时索引**系统默认三夹。
            </p>
          </div>
        )}
        {/* cycle 6 v4：include_system_defaults checkbox——只在有自定义目录时才有意义暴露
            （空 index_roots 时本就走系统默认、参数无效果）。cycle 7-a：加强 UI（绿描边）便于发现。 */}
        {hasCustomRoots && (
          <label className="prefs-checkbox prefs-checkbox-strong">
            <input
              type="checkbox"
              checked={settings.include_system_defaults}
              onChange={(e) =>
                setSettings({
                  ...settings,
                  include_system_defaults: e.target.checked,
                })
              }
            />
            <strong>同时索引系统默认目录（音乐 / 文档 / 图片）</strong>
          </label>
        )}
        {!hasCustomRoots && (
          <p className="prefs-hint">
            未添加自定义目录，当前使用系统默认（音乐 / 文档 / 图片）：
          </p>
        )}
        {/* cycle 6 v4：统一按 effectiveRoots 渲染，自定义项显示「移除」、系统默认项显示 tag。
            cycle 7-a：pending 集合传 RootRow 显示琥珀 badge；flashPath 命中的行加 CSS flash 高亮。 */}
        {effectiveRoots?.map((path, i) => {
          const isCustom = settings.index_roots.includes(path);
          const isPending = pendingSet.has(path);
          return (
            <RootRow
              key={`${isCustom ? "usr" : "sys"}-${i}`}
              path={path}
              isSystemDefault={!isCustom}
              overview={overviewOf(path)}
              isPending={isPending}
              flash={flashPath === path}
              excludePatterns={excludesFor(path)}
              onUpdateExcludes={(patterns) => updateExcludesFor(path, patterns)}
              onOpenDir={() => onOpenRoot(path)}
              // pending root 的排除配置尚未保存、重扫口径会与预期不符 → 不给重扫入口。
              onRescan={isPending ? null : () => onReindexRoot(path)}
              rescanDisabled={reindexing || (indexStatus?.indexing ?? false)}
              onRemove={isCustom ? () => onRequestRemoveRoot(path) : null}
            />
          );
        })}
        {effectiveRoots && effectiveRoots.length === 0 && (
          <p className="prefs-hint err">
            ⚠️ 系统未检测到默认音乐 / 文档 / 图片目录，请手动「+ 添加目录」。
          </p>
        )}
        <button
          type="button"
          className="prefs-btn"
          onClick={async () => {
            const { open } = await import("@tauri-apps/plugin-dialog");
            const picked = await open({ directory: true, multiple: false });
            if (typeof picked === "string") {
              if (settings.index_roots.includes(picked)) {
                // cycle 7-a：已在列表也 flash 一下让用户知道"没重复添加、但确实是这条"
                onFlash(picked);
                onPickMessage("该目录已在列表中");
              } else {
                setSettings({
                  ...settings,
                  index_roots: [...settings.index_roots, picked],
                });
                onFlash(picked);
                onPickMessage(
                  "已加入下方列表 · 未保存 —— 点「应用」或「确定」生效",
                );
              }
            }
          }}
        >
          + 添加目录
        </button>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">
          排除目录名（通配符，留空 = 默认排除 node_modules/.git 等）
        </label>
        {settings.exclude_globs.map((g, i) => (
          <div key={i} className="prefs-root-row">
            <span className="prefs-root-path">{g}</span>
            <button
              type="button"
              className="prefs-btn small"
              onClick={() =>
                setSettings({
                  ...settings,
                  exclude_globs: settings.exclude_globs.filter(
                    (_, j) => j !== i,
                  ),
                })
              }
            >
              移除
            </button>
          </div>
        ))}
        <div style={{ display: "flex", gap: "8px" }}>
          <input
            type="text"
            className="prefs-input"
            value={excludeDraft}
            onChange={(e) => setExcludeDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") addExclude();
            }}
            placeholder="如 node_modules 或 *cache*"
          />
          <button type="button" className="prefs-btn" onClick={addExclude}>
            添加
          </button>
        </div>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">本地索引</label>
        <p className="prefs-hint">
          建立音乐 metadata 与文档内容的本地索引；应用启动时会在后台自动索引。
        </p>
        {/* BETA-39：图片语义索引 opt-in。默认关（防乱码 OCR 污染语义召回）；
            开启后图片文字走更严的质量门槛（0.75）入语义索引，需重新索引生效。 */}
        <label className="prefs-checkbox">
          <input
            type="checkbox"
            checked={settings.enable_image_semantics}
            onChange={(e) =>
              setSettings({
                ...settings,
                enable_image_semantics: e.target.checked,
              })
            }
          />
          <span>
            <strong>让图片文字参与语义搜索（实验性）</strong>
            <br />
            <span className="prefs-hint">
              默认关闭：图片 OCR 文字仅支持字面（关键词）匹配。开启后，通过更严格质量门槛的图片文字（如聊天截图、扫描笔记）也能被「按意思」搜到；乱码 OCR 会被自动挡下。
              <strong>需重新索引后生效。</strong>
            </span>
          </span>
        </label>
        {/* cycle 7-a：正在索引时显示 indeterminate 进度条（Codex OBJECT 3 · 不做百分比）
            + 阶段 chip + 当前目录 + 累计计数。文本行由 indexStatusLine 生成。 */}
        {indexStatus?.indexing && (
          <div className="prefs-progress-indeterminate" aria-hidden="true">
            <div className="prefs-progress-bar" />
          </div>
        )}
        <p className="prefs-status">{indexStatusLine}</p>
        {semanticLine && <p className="prefs-status">{semanticLine}</p>}
        <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
          <button
            type="button"
            className="prefs-btn primary"
            onClick={onReindex}
            disabled={reindexing}
          >
            {reindexing ? "索引中…" : "立即索引"}
          </button>
          {reindexMsg && <span className="prefs-status">{reindexMsg}</span>}
        </div>
      </div>

      {/* BETA-40：文件级提取失败留痕——哪些文件没能进索引、为什么。成功重扫 /
          文件从磁盘删除后自动从清单消失。无失败时不渲染整节（不制造焦虑）。 */}
      {extractionFailures !== null && extractionFailures.length > 0 && (
        <div className="prefs-field">
          <label className="prefs-label">未能索引的文件</label>
          <p className="prefs-hint">
            以下文件在索引时提取失败（损坏 / 加密 / 缺依赖等），搜索不到它们的内容。
            修复原因后「立即索引」会自动重试；成功或文件删除后自动从此清单消失。
          </p>
          <button
            type="button"
            className="prefs-btn small"
            onClick={() => setFailuresExpanded((v) => !v)}
          >
            {failuresExpanded ? "▾" : "▸"} 共 {extractionFailures.length} 个文件
          </button>
          {failuresExpanded && (
            <div
              style={{
                maxHeight: "220px",
                overflowY: "auto",
                marginTop: "8px",
              }}
            >
              {extractionFailures.map((f, i) => (
                <div key={i} className="prefs-root-row" title={f.path}>
                  <span className="prefs-root-path">
                    {f.path.split(/[\\/]/).pop() ?? f.path}
                    <span className="prefs-hint">
                      {" — "}
                      {f.reason}
                      {f.failed_time
                        ? `（${formatIndexTime(f.failed_time)}）`
                        : ""}
                    </span>
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function PrivacyPane({
  auditLog,
  onReload,
  onClear,
}: {
  auditLog: AuditEntry[];
  onReload: () => void;
  onClear: () => void;
}) {
  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">操作记录</label>
        <p className="prefs-hint">
          LociFind 对文件执行的操作（打开 / 定位 / 复制 / 移动 / 重命名）记录在本地，便于查看与追溯。
          <strong>仅保存在本机、不上传</strong>，可随时一键清除。
        </p>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "10px",
            marginBottom: "8px",
          }}
        >
          <button type="button" className="prefs-btn" onClick={onReload}>
            刷新
          </button>
          <button
            type="button"
            className="prefs-btn danger"
            onClick={onClear}
            disabled={auditLog.length === 0}
          >
            清除记录
          </button>
          <span className="prefs-status">{auditLog.length} 条</span>
        </div>
        {auditLog.length === 0 ? (
          <p className="prefs-status">暂无操作记录</p>
        ) : (
          <div className="prefs-audit-wrap">
            <table className="prefs-audit-table">
              <thead>
                <tr>
                  <th>时间</th>
                  <th>操作</th>
                  <th>文件</th>
                  <th>结果</th>
                </tr>
              </thead>
              <tbody>
                {auditLog.slice(0, 200).map((e, i) => (
                  <tr key={i}>
                    <td className="ts">
                      {new Date(e.timestamp).toLocaleString()}
                    </td>
                    <td>{e.operation}</td>
                    <td className="files">
                      {e.source_paths.join(", ")}
                      {e.destination ? ` → ${e.destination}` : ""}
                      {e.new_name ? ` → ${e.new_name}` : ""}
                    </td>
                    <td className={e.result === "failed" ? "err" : "ok"}>
                      {e.result === "failed"
                        ? `失败${e.error ? `(${e.error})` : ""}`
                        : "已执行"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
