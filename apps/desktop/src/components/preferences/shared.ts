// BETA-47：选项对话框共享类型与工具函数（原 PreferencesDialog.tsx 1579 行拆文件）。
// 各分类面板拆至同目录 *Pane.tsx；此处只放跨面板复用的类型 / 纯函数 / 分类表。

/** 分类 key（BETA-47 七 tab：常规 / 索引 / Everything / 语义召回 / Windows / 隐私与记录 / 杂项）。 */
export type Category =
  | "general"
  | "indexing"
  | "everything"
  | "semantic"
  | "windows"
  | "privacy"
  | "misc";

/** 当前是否 Windows（Everything / Windows 系统集成两个 tab 仅 Windows 显示）。 */
export const IS_WINDOWS =
  typeof navigator !== "undefined" && /Win/i.test(navigator.platform);

/** 分类表（按平台过滤后渲染左侧分类树）。 */
export const CATEGORIES: { key: Category; label: string }[] = [
  { key: "general", label: "常规" },
  { key: "indexing", label: "索引" },
  ...(IS_WINDOWS
    ? ([
        { key: "everything", label: "Everything" },
      ] as { key: Category; label: string }[])
    : []),
  { key: "semantic", label: "语义召回" },
  ...(IS_WINDOWS
    ? ([{ key: "windows", label: "Windows" }] as {
        key: Category;
        label: string;
      }[])
    : []),
  { key: "privacy", label: "隐私与记录" },
  { key: "misc", label: "杂项" },
];

export interface AuditEntry {
  timestamp: string;
  operation: string;
  source_paths: string[];
  destination: string | null;
  new_name: string | null;
  result: string;
  error: string | null;
}

/** cycle 7-a：索引阶段（后端 IndexPhase enum snake_case serialize）。 */
export type IndexPhase = "music_discovery" | "music_scan" | "doc" | "image";

export interface IndexStatus {
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
export function phaseChipLabel(phase: IndexPhase): string {
  switch (phase) {
    case "music_discovery":
      return "🎵 扫描音乐（Everything 快速发现，请稍候）";
    case "music_scan":
      return "🎵 扫描音乐目录";
    case "doc":
      return "📄 扫描文档";
    case "image":
      return "🖼 扫描图片";
  }
}

/** `reindex` / `reindex_root` 命令的返回统计（cycle 7-c 单目录重扫与全量共用）。 */
export interface ReindexStats {
  music_added: number;
  music_updated: number;
  doc_added: number;
  doc_updated: number;
  image_added: number;
  image_updated: number;
}

export function reindexDoneMsg(s: ReindexStats): string {
  return `完成：音乐 新增 ${s.music_added} / 更新 ${s.music_updated}，文档 新增 ${s.doc_added} / 更新 ${s.doc_updated}，图片 新增 ${s.image_added} / 更新 ${s.image_updated}`;
}

/** BETA-33 cycle 5：每个索引 root 的分类统计。后端 `get_index_overview` 返回。 */
export interface RootIndexOverview {
  path: string;
  is_default: boolean;
  doc_count: number;
  image_count: number;
  music_count: number;
  last_indexed_time: string | null;
}

/** BETA-40：一条「未能索引的文件」留痕。后端 `get_extraction_failures` 返回（按时间倒序）。 */
export interface ExtractionFailure {
  path: string;
  reason: string;
  failed_time: string | null;
}

/**
 * BETA-33 cycle 5：把 UTC rfc3339 时间转成本地口语（"5 分钟前" / "今天 15:32" / "2026-06-30"）。
 * 输入无效 → 空串。
 */
export function formatIndexTime(iso: string): string {
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
