import { AuditEntry } from "./shared";

/** 「隐私与记录」面板：本地操作记录（audit log）查看与清除。 */
export function PrivacyPane({
  auditLog,
  onReload,
  onClear,
  onNavigate,
}: {
  auditLog: AuditEntry[];
  onReload: () => void;
  onClear: () => void;
  /** BETA-47：跳转完整「隐私与数据」页（走关闭守卫，见 shell）。 */
  onNavigate: (path: string) => void;
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

      {/* BETA-47：完整隐私面板（索引了什么 / 数据在哪 / 一键清空索引）在独立页面。 */}
      <div className="prefs-field">
        <label className="prefs-label">隐私与数据</label>
        <p className="prefs-hint">
          查看索引了什么、数据存在哪、以及一键清空本地索引，在完整「隐私与数据」面板。
        </p>
        <button
          type="button"
          className="prefs-btn"
          onClick={() => onNavigate("/privacy")}
        >
          打开「隐私与数据」面板…
        </button>
      </div>
    </div>
  );
}
