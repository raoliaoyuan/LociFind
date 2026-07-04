// 快速入门第 1 步：Windows 搜索服务（WSearch）状态条。
// BETA-33 cycle 9：`check_windows_search_indexed` 真做后的首个真实消费点——
// 此前该命令恒返 Unknown 且无 UI 消费，用户无从得知系统搜索臂是否可用。
// 探测语义 = 「Windows 搜索**服务**是否运行」（服务停了 SystemIndex 必不可查）；
// 目录级索引范围仍由「打开索引选项…」交系统 UI 确认。
import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type WinIndexStatus = "Indexed" | "NotIndexed" | "Unknown" | "NotApplicable";

export const WindowsSearchCheckStep: React.FC = () => {
  // null = 首次检测中。
  const [status, setStatus] = useState<WinIndexStatus | null>(null);

  const check = useCallback(async () => {
    try {
      const s = await invoke<WinIndexStatus>("check_windows_search_indexed");
      setStatus(s);
    } catch (err) {
      console.error("[WindowsSearchCheckStep] check failed:", err);
      setStatus("Unknown");
    }
  }, []);

  useEffect(() => {
    void check();
    // 用户可能在这一步切窗去「服务」里启用 Windows Search。3s 一轮询自动感知。
    const t = setInterval(() => void check(), 3000);
    return () => clearInterval(t);
  }, [check]);

  if (status === null || status === "NotApplicable") {
    return null; // 检测中 / 非 Windows：不占版面。
  }

  if (status === "Indexed") {
    return (
      <div
        style={{
          padding: "8px 14px",
          borderRadius: "10px",
          backgroundColor: "#e8f7ee",
          border: "1px solid #b7e4c7",
          marginBottom: "10px",
          fontSize: "12.5px",
          color: "#0a7a3b",
        }}
      >
        ✓ Windows 搜索服务运行中——系统索引可用，确认目录范围即可。
      </div>
    );
  }

  if (status === "NotIndexed") {
    return (
      <div
        style={{
          padding: "8px 14px",
          borderRadius: "10px",
          backgroundColor: "#fff4e5",
          border: "1px solid #f5cd8f",
          marginBottom: "10px",
          fontSize: "12.5px",
          color: "#8a5a00",
          lineHeight: 1.5,
        }}
      >
        ⚠ Windows 搜索服务未运行——LociFind 仍可用本地索引 / Everything
        搜索，但系统搜索臂不可用。建议在「服务」中启动 Windows Search
        （服务名 WSearch）后再做本步。
      </div>
    );
  }

  // Unknown：探测失败，不拦流程。
  return (
    <div
      style={{
        padding: "8px 14px",
        borderRadius: "10px",
        backgroundColor: "#f0f2f5",
        border: "1px solid #d9dce1",
        marginBottom: "10px",
        fontSize: "12.5px",
        color: "#555",
      }}
    >
      无法检测 Windows 搜索服务状态（不影响使用，可继续）。
    </div>
  );
};

export default WindowsSearchCheckStep;
