import React, { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AuditEntry } from "./shared";

// 与 privacy.rs::DataLocation 对应（BETA-21）。
interface DataLocation {
  label: string;
  path: string;
  exists: boolean;
  size_bytes: number;
}

// 与 privacy.rs::PrivacyOverview 对应（BETA-21）。
interface PrivacyOverview {
  music_count: number;
  document_count: number;
  image_count: number;
  index_available: boolean;
  last_indexed: string | null;
  indexing: boolean;
  data_root: string;
  locations: DataLocation[];
  search_scope: string[];
  audit_count: number;
  search_history_count: number;
  user_synonym_count: number;
  tracing_enabled: boolean;
}

// 与 uninstall.rs::CleanupItem / CleanupReport 对应（BETA-12）。
interface CleanupItem {
  label: string;
  path: string;
  existed: boolean;
  removed: boolean;
  detail: string | null;
}

interface CleanupReport {
  items: CleanupItem[];
  all_ok: boolean;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * 「隐私与记录」面板：操作记录（audit）+ 完整「隐私与数据」内容。
 *
 * 2026-07-07：原「隐私与数据」是独立整页 `/privacy`、进入后无返回入口，现整体
 * 折叠进本 tab（索引概览 / 数据存放位置 / 一键清除 / 卸载清理）——设置内容统一
 * 收进选项对话框、不再跳出整页。操作记录部分仍由对话框壳提供数据（props）。
 */
export function PrivacyPane({
  auditLog,
  onReload,
  onClear,
}: {
  auditLog: AuditEntry[];
  onReload: () => void;
  onClear: () => void;
}) {
  const [overview, setOverview] = useState<PrivacyOverview | null>(null);
  const [clearMsg, setClearMsg] = useState("");
  const [confirmIndex, setConfirmIndex] = useState(false);
  const [working, setWorking] = useState(false);
  const [confirmCleanup, setConfirmCleanup] = useState(false);
  const [cleanupReport, setCleanupReport] = useState<CleanupReport | null>(null);
  const [cleanupMsg, setCleanupMsg] = useState("");

  const loadOverview = () => {
    invoke<PrivacyOverview>("get_privacy_overview")
      .then(setOverview)
      .catch(console.error);
  };

  useEffect(() => {
    loadOverview();
    // 索引可能在后台进行，轻度轮询刷新统计。
    const timer = setInterval(loadOverview, 3000);
    return () => clearInterval(timer);
  }, []);

  const handleClearHistory = async () => {
    setWorking(true);
    setClearMsg("");
    try {
      await invoke("clear_search_history");
      setClearMsg("搜索历史已清除");
      loadOverview();
    } catch (err) {
      setClearMsg(`清除失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  const handleClearIndex = async () => {
    setWorking(true);
    setClearMsg("");
    setConfirmIndex(false);
    try {
      await invoke("clear_local_index");
      setClearMsg("本地索引已清空（下次索引会重建）");
      loadOverview();
    } catch (err) {
      setClearMsg(`清除失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  // BETA-12：卸载清理（删索引/模型/日志/审计/搜索历史/用户同义词库，保留设置）。
  const handleUninstallCleanup = async () => {
    setWorking(true);
    setCleanupMsg("");
    setCleanupReport(null);
    setConfirmCleanup(false);
    try {
      const report = await invoke<CleanupReport>("uninstall_cleanup");
      setCleanupReport(report);
      setCleanupMsg(
        report.all_ok
          ? "清理完成，设置已保留。现在可以放心卸载 LociFind。"
          : "部分项目未能删除，详见下表。",
      );
      loadOverview();
    } catch (err) {
      setCleanupMsg(`清理失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  return (
    <div className="prefs-form">
      {/* 操作记录（audit log，数据来自对话框壳） */}
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

      {/* 索引了什么 */}
      <div className="prefs-field">
        <label className="prefs-label">索引了什么</label>
        {!overview ? (
          <p className="prefs-status">读取中…</p>
        ) : !overview.index_available ? (
          <p className="prefs-status">
            尚未建立本地索引（可在「索引」tab 点「立即索引」，或等待启动后台索引完成）。
          </p>
        ) : (
          <>
            <div
              style={{
                display: "flex",
                gap: "12px",
                flexWrap: "wrap",
                marginBottom: "8px",
              }}
            >
              <StatCard label="音乐" value={overview.music_count} />
              <StatCard label="文档" value={overview.document_count} />
              <StatCard label="图片 (OCR)" value={overview.image_count} />
            </div>
            <p className="prefs-status">
              {overview.indexing
                ? "⏳ 正在后台索引…"
                : overview.last_indexed
                  ? `上次索引：${new Date(overview.last_indexed).toLocaleString()}`
                  : ""}
            </p>
          </>
        )}
      </div>

      {/* 数据存在哪 */}
      <div className="prefs-field">
        <label className="prefs-label">数据存在哪</label>
        <p className="prefs-hint">
          这些文件仅保存在本机数据目录，<strong>不会上传</strong>。
        </p>
        {overview && (
          <div
            style={{
              border: "1px solid #eee",
              borderRadius: "8px",
              overflow: "hidden",
            }}
          >
            <table
              style={{
                width: "100%",
                fontSize: "13px",
                borderCollapse: "collapse",
              }}
            >
              <thead>
                <tr style={{ background: "#fafafa", textAlign: "left" }}>
                  <th style={{ padding: "8px 12px" }}>类别</th>
                  <th style={{ padding: "8px 12px" }}>路径</th>
                  <th style={{ padding: "8px 12px", whiteSpace: "nowrap" }}>
                    大小
                  </th>
                </tr>
              </thead>
              <tbody>
                {overview.locations.map((loc, i) => (
                  <tr key={i} style={{ borderTop: "1px solid #f0f0f0" }}>
                    <td style={{ padding: "8px 12px", whiteSpace: "nowrap" }}>
                      {loc.label}
                    </td>
                    <td
                      style={{
                        padding: "8px 12px",
                        wordBreak: "break-all",
                        color: "#666",
                        fontFamily: "monospace",
                        fontSize: "12px",
                      }}
                      title={loc.path}
                    >
                      {loc.path}
                    </td>
                    <td
                      style={{
                        padding: "8px 12px",
                        whiteSpace: "nowrap",
                        color: loc.exists ? "#666" : "#bbb",
                      }}
                    >
                      {loc.exists ? formatBytes(loc.size_bytes) : "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {overview && (
          <p
            style={{ fontSize: "12px", color: "#999", marginTop: "8px" }}
          >
            数据目录：<code>{overview.data_root}</code>
            {overview.tracing_enabled && " ・ 调试追踪已开启（日志仅本地）"}
            {overview.user_synonym_count > 0 &&
              ` ・ 用户同义词 ${overview.user_synonym_count} 组`}
          </p>
        )}
      </div>

      {/* 一键清除 */}
      <div
        className="prefs-field"
        style={{
          backgroundColor: "#fdf6f6",
          padding: "16px",
          borderRadius: "8px",
          border: "1px solid #f3dada",
        }}
      >
        <label className="prefs-label" style={{ color: "#d33" }}>
          一键清除
        </label>
        <p className="prefs-hint">
          可随时清除本机数据。清除后不可恢复，但本地索引可通过重新索引重建。
        </p>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            marginBottom: "12px",
            flexWrap: "wrap",
          }}
        >
          <button
            type="button"
            className="prefs-btn"
            onClick={handleClearHistory}
            disabled={
              working ||
              !overview ||
              overview.search_history_count === 0
            }
          >
            清除搜索历史
          </button>
          <span className="prefs-status">
            {overview ? `${overview.search_history_count} 条` : ""}
          </span>
        </div>

        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            flexWrap: "wrap",
          }}
        >
          {!confirmIndex ? (
            <button
              type="button"
              className="prefs-btn"
              onClick={() => setConfirmIndex(true)}
              disabled={working || !overview || !overview.index_available}
            >
              清空本地索引
            </button>
          ) : (
            <>
              <span style={{ fontSize: "13px", color: "#d33" }}>
                确定清空全部本地索引？
              </span>
              <button
                type="button"
                className="prefs-btn danger"
                onClick={handleClearIndex}
                disabled={working}
              >
                确认清空
              </button>
              <button
                type="button"
                className="prefs-btn"
                onClick={() => setConfirmIndex(false)}
                disabled={working}
              >
                取消
              </button>
            </>
          )}
        </div>

        {clearMsg && (
          <p
            style={{
              fontSize: "13px",
              color: clearMsg.includes("失败") ? "#d33" : "#34c759",
              marginTop: "12px",
            }}
          >
            {clearMsg}
          </p>
        )}
      </div>

      {/* BETA-12 卸载清理 */}
      <div
        className="prefs-field"
        style={{
          backgroundColor: "#fdf6f6",
          padding: "16px",
          borderRadius: "8px",
          border: "1px solid #f3dada",
        }}
      >
        <label className="prefs-label" style={{ color: "#d33" }}>
          卸载清理
        </label>
        <p className="prefs-hint">
          打算卸载 LociFind？一键删除本机全部派生数据——索引数据库、已下载的模型、运行日志、
          操作审计日志、搜索历史、用户同义词库；<strong>设置文件保留</strong>（重装后配置仍在）。
          Windows 安装版直接运行系统卸载程序即可，卸载时会自动完成同等清理（版本升级不受影响）。
        </p>
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: "12px",
            flexWrap: "wrap",
          }}
        >
          {!confirmCleanup ? (
            <button
              type="button"
              className="prefs-btn"
              onClick={() => setConfirmCleanup(true)}
              disabled={working}
            >
              清理全部数据（保留设置）
            </button>
          ) : (
            <>
              <span style={{ fontSize: "13px", color: "#d33" }}>
                确定删除索引、模型、日志等全部数据？此操作不可恢复。
              </span>
              <button
                type="button"
                className="prefs-btn danger"
                onClick={handleUninstallCleanup}
                disabled={working}
              >
                确认清理
              </button>
              <button
                type="button"
                className="prefs-btn"
                onClick={() => setConfirmCleanup(false)}
                disabled={working}
              >
                取消
              </button>
            </>
          )}
        </div>
        {cleanupMsg && (
          <p
            style={{
              fontSize: "13px",
              color:
                cleanupMsg.includes("失败") || cleanupMsg.includes("未能")
                  ? "#d33"
                  : "#34c759",
              marginTop: "12px",
            }}
          >
            {cleanupMsg}
          </p>
        )}
        {cleanupReport && (
          <ul
            style={{
              fontSize: "12px",
              color: "#666",
              marginTop: "8px",
              paddingLeft: "18px",
            }}
          >
            {cleanupReport.items.map((item, i) => (
              <li key={i} style={{ color: item.removed ? "#666" : "#d33" }}>
                {item.label}：
                {item.removed
                  ? item.existed
                    ? "已删除"
                    : "本来就不存在"
                  : `删除失败（${item.detail ?? "未知原因"}）`}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

const StatCard: React.FC<{ label: string; value: number }> = ({
  label,
  value,
}) => (
  <div
    style={{
      flex: "1 1 120px",
      background: "#f5f7fa",
      borderRadius: "8px",
      padding: "12px 16px",
      textAlign: "center",
    }}
  >
    <div style={{ fontSize: "22px", fontWeight: 600, color: "#007aff" }}>
      {value.toLocaleString()}
    </div>
    <div style={{ fontSize: "13px", color: "#666", marginTop: "2px" }}>
      {label}
    </div>
  </div>
);
