import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "../../hooks/useAppSettings";

const WINGET_CMD =
  "winget install voidtools.Everything voidtools.Everything.Cli";
const VOIDTOOLS_URL = "https://www.voidtools.com/downloads/";

/**
 * BETA-47：「Everything」面板（仅 Windows tab 树中出现）。
 * 检测（es.exe 可用性，与开关独立——关了也要能告知「装没装」）+ 集成总开关 +
 * 未安装时的安装引导（与快速入门 EverythingCheckStep 同口径的精简版）。
 */
export function EverythingPane({
  settings,
  setSettings,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
}) {
  // null = 首次检测中；true = es.exe 可用；false = 未装 / 未运行。
  const [esAvailable, setEsAvailable] = useState<boolean | null>(null);
  const [checking, setChecking] = useState(false);
  const [copied, setCopied] = useState(false);

  const check = useCallback(async () => {
    setChecking(true);
    try {
      setEsAvailable(await invoke<boolean>("check_everything_available"));
    } catch (err) {
      console.error("[EverythingPane] check_everything_available failed:", err);
      setEsAvailable(false);
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    void check();
    // 用户可能切窗去装 Everything，3s 一轮询自动感知（与快速入门步骤同款）。
    const t = setInterval(() => void check(), 3000);
    return () => clearInterval(t);
  }, [check]);

  const copyCmd = async () => {
    try {
      await navigator.clipboard.writeText(WINGET_CMD);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (err) {
      console.error("[EverythingPane] clipboard write failed:", err);
    }
  };

  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">检测</label>
        {esAvailable === null ? (
          <p className="prefs-status">正在检测 Everything（es.exe）…</p>
        ) : esAvailable ? (
          <p className="prefs-status" style={{ color: "#0a7a3b" }}>
            ✓ 已检测到 Everything（es.exe CLI 可用）
          </p>
        ) : (
          <p className="prefs-status" style={{ color: "#8a5a00" }}>
            ⚠ 未检测到 Everything（es.exe）——未装或服务未运行
          </p>
        )}
        <div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
          <button
            type="button"
            className="prefs-btn small"
            onClick={() => void check()}
            disabled={checking}
          >
            {checking ? "检测中…" : "重新检测"}
          </button>
          <span className="prefs-hint">每 3 秒自动检测一次</span>
        </div>
      </div>

      <div className="prefs-field">
        <label className="prefs-checkbox prefs-checkbox-strong">
          <input
            type="checkbox"
            checked={settings.enable_everything}
            onChange={(e) =>
              setSettings({ ...settings, enable_everything: e.target.checked })
            }
          />
          <strong>使用 Everything 加速（推荐，装了就自动用）</strong>
        </label>
        <p className="prefs-hint">
          开启时 LociFind 在三处调用 Everything（es.exe）：① 按文件名搜索加速与
          Windows 索引盲区兜底（如 <code>%TEMP%</code>、外接盘）；② 建索引时的音乐
          快速发现（结果仅限所选索引目录）；③ 模型下载前的本机已有模型发现。
          未安装 Everything 时自动降级、不影响使用。
        </p>
        <p className="prefs-hint">
          关闭后 LociFind 完全不调用 es.exe：搜索加速部分<strong>需重启应用生效</strong>，
          音乐发现与模型发现保存后即生效（音乐改为只扫描索引目录）。
        </p>
      </div>

      {esAvailable === false && (
        <div className="prefs-field">
          <label className="prefs-label">安装 Everything（可选）</label>
          <p className="prefs-hint">
            方式 A：在 PowerShell 或 CMD 里执行 winget 命令（同时装主程序 +
            es.exe，两者都需要）：
          </p>
          <div style={{ display: "flex", gap: "8px", alignItems: "center" }}>
            <code
              style={{
                flex: 1,
                padding: "6px 10px",
                borderRadius: "5px",
                backgroundColor: "#1e1e1e",
                color: "#e6e6e6",
                fontSize: "12.5px",
                wordBreak: "break-all",
              }}
            >
              {WINGET_CMD}
            </code>
            <button type="button" className="prefs-btn small" onClick={copyCmd}>
              {copied ? "已复制" : "复制"}
            </button>
          </div>
          <p className="prefs-hint">
            方式 B：官网下载{" "}
            <a href={VOIDTOOLS_URL} target="_blank" rel="noreferrer">
              {VOIDTOOLS_URL}
            </a>
            （除主程序外务必勾选 <strong>Command-line Interface</strong>）。
          </p>
        </div>
      )}
    </div>
  );
}
