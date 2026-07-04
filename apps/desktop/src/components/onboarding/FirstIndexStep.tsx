// 快速入门共用步骤：触发首次索引 + 展示示例查询。
// 用户可以随时点「完成」进主界面，索引在后台继续跑（不阻塞）。
import React, { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
// 首次索引这步空间紧张、复用 ExampleQueries 会太占位；用行内更小的例句列表。
const EXAMPLES: { query: string; hint: string }[] = [
  { query: "年假和休假规定", hint: "中文找英文文档" },
  { query: "leave policy", hint: "英文同款" },
  { query: "公司发票模板", hint: "找 Excel / Word" },
  { query: "meeting notes Q3", hint: "英文项目笔记" },
];

type IndexPhase = "music_discovery" | "music_scan" | "doc" | "image";

interface IndexStatus {
  indexing: boolean;
  last_indexed: string | null;
  last_summary: string | null;
  current_root: string | null;
  fts_progress: [number, number] | null;
  current_phase: IndexPhase | null;
  semantic_indexing: boolean;
  semantic_progress: [number, number] | null;
  semantic_summary: string | null;
}

interface ReindexStats {
  music_added: number;
  music_updated: number;
  doc_added: number;
  doc_updated: number;
  image_added: number;
  image_updated: number;
}

function phaseLabel(phase: IndexPhase | null): string {
  switch (phase) {
    case "music_discovery":
      return "🎵 全盘发现音乐";
    case "music_scan":
      return "🎵 扫描音乐目录";
    case "doc":
      return "📄 扫描文档（关键词 + 语义）";
    case "image":
      return "🖼 扫描图片（OCR + 语义）";
    default:
      return "准备中…";
  }
}

export interface FirstIndexStepProps {
  onPickExample: (query: string) => void;
  onFinish: () => void;
}

export const FirstIndexStep: React.FC<FirstIndexStepProps> = ({
  onPickExample,
  // 目前"跳过示例"由 shell 底部的 skipAction 提供，本组件内无独立入口；保留 prop 以便未来扩展。
  onFinish: _onFinish,
}) => {
  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [triggered, setTriggered] = useState(false);
  const [lastStats, setLastStats] = useState<ReindexStats | null>(null);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      const s = await invoke<IndexStatus>("get_index_status");
      setStatus(s);
    } catch (err) {
      console.error("[FirstIndexStep] get_index_status failed:", err);
    }
  }, []);

  useEffect(() => {
    void loadStatus();
    const t = setInterval(() => void loadStatus(), 1500);
    return () => clearInterval(t);
  }, [loadStatus]);

  const startIndexing = async () => {
    setTriggered(true);
    setErrorMsg(null);
    try {
      const stats = await invoke<ReindexStats>("reindex");
      setLastStats(stats);
    } catch (err) {
      // "正在索引中，请稍候" 不算错误——就是并发状态；轮询会拿到 indexing=true
      const msg = String(err);
      if (msg.includes("正在索引")) return;
      setErrorMsg(msg);
    }
  };

  const isIndexing = status?.indexing === true;
  const hasEverIndexed = status?.last_indexed !== null;

  const [ftsScanned, ftsIndexed] = status?.fts_progress ?? [0, 0];
  const ftsPct =
    ftsScanned > 0 ? Math.min(100, (ftsIndexed / ftsScanned) * 100) : null;

  const [semDone, semTotal] = status?.semantic_progress ?? [0, 0];
  const semPct =
    semTotal > 0 ? Math.min(100, (semDone / semTotal) * 100) : null;

  return (
    <>
      <p
        style={{
          color: "#555",
          margin: 0,
          marginBottom: "10px",
          lineHeight: 1.55,
          fontSize: "13px",
        }}
      >
        点下面按钮启动<strong>首轮索引</strong>：扫描目录 · 抽取文本 / 音乐元数据 / 图片
        OCR · 生成语义向量。几分钟到几十分钟不等；
        <strong>你随时可以点「完成」进主界面，索引后台继续跑。</strong>
      </p>

      <div
        style={{
          padding: "10px 12px",
          borderRadius: "10px",
          backgroundColor: "#f0f2f5",
          marginBottom: "10px",
        }}
      >
        {!isIndexing && !triggered && !hasEverIndexed && (
          <button
            onClick={() => void startIndexing()}
            style={{
              backgroundColor: "#007aff",
              color: "white",
              border: "none",
              padding: "7px 18px",
              borderRadius: "7px",
              cursor: "pointer",
              fontSize: "13px",
              fontWeight: 500,
            }}
          >
            开始扫描并索引
          </button>
        )}

        {!isIndexing && hasEverIndexed && (
          <div>
            <div
              style={{
                color: "#0a7a3b",
                fontSize: "13px",
                marginBottom: "3px",
              }}
            >
              ✓ 首轮索引已完成
            </div>
            {status?.last_summary && (
              <div
                style={{
                  fontSize: "12.5px",
                  color: "#446854",
                  marginBottom: "4px",
                }}
              >
                {status.last_summary}
              </div>
            )}
            {lastStats && (
              <div style={{ fontSize: "11.5px", color: "#666" }}>
                本次：音乐 +{lastStats.music_added}/~{lastStats.music_updated}，
                文档 +{lastStats.doc_added}/~{lastStats.doc_updated}，
                图片 +{lastStats.image_added}/~{lastStats.image_updated}
              </div>
            )}
            <button
              onClick={() => void startIndexing()}
              style={{
                marginTop: "6px",
                backgroundColor: "transparent",
                color: "#007aff",
                border: "1px solid #007aff",
                padding: "3px 12px",
                borderRadius: "5px",
                cursor: "pointer",
                fontSize: "11.5px",
              }}
            >
              重新索引
            </button>
          </div>
        )}

        {isIndexing && (
          <div>
            <div
              style={{
                fontSize: "12.5px",
                fontWeight: 500,
                color: "#1d1d1f",
                marginBottom: "4px",
              }}
            >
              {phaseLabel(status?.current_phase ?? null)}
            </div>
            {status?.current_root && (
              <div
                style={{
                  fontSize: "11.5px",
                  color: "#666",
                  marginBottom: "6px",
                  wordBreak: "break-all",
                }}
              >
                当前目录：{status.current_root}
              </div>
            )}

            <div style={{ marginBottom: "6px" }}>
              <div
                style={{
                  fontSize: "11.5px",
                  color: "#333",
                  marginBottom: "2px",
                }}
              >
                关键词索引（FTS）：{ftsIndexed} / {ftsScanned}
                {ftsPct !== null && `（${ftsPct.toFixed(1)}%）`}
              </div>
              <div
                style={{
                  height: "5px",
                  backgroundColor: "#e0e0e0",
                  borderRadius: "3px",
                  overflow: "hidden",
                }}
              >
                <div
                  style={{
                    height: "100%",
                    width: ftsPct !== null ? `${ftsPct}%` : "5%",
                    backgroundColor: "#007aff",
                    transition: "width 0.3s ease",
                  }}
                />
              </div>
            </div>

            {status?.semantic_indexing && (
              <div>
                <div
                  style={{
                    fontSize: "11.5px",
                    color: "#333",
                    marginBottom: "2px",
                  }}
                >
                  语义索引（embedding）：{semDone} / {semTotal}
                  {semPct !== null && `（${semPct.toFixed(1)}%）`}
                </div>
                <div
                  style={{
                    height: "5px",
                    backgroundColor: "#e0e0e0",
                    borderRadius: "3px",
                    overflow: "hidden",
                  }}
                >
                  <div
                    style={{
                      height: "100%",
                      width: semPct !== null ? `${semPct}%` : "5%",
                      backgroundColor: "#34c759",
                      transition: "width 0.3s ease",
                    }}
                  />
                </div>
              </div>
            )}
          </div>
        )}

        {!isIndexing && triggered && !hasEverIndexed && !errorMsg && (
          <div style={{ fontSize: "12.5px", color: "#666" }}>正在启动…</div>
        )}

        {errorMsg && (
          <div style={{ color: "#d00", fontSize: "12.5px" }}>
            索引启动失败：{errorMsg}
          </div>
        )}
      </div>

      <div style={{ fontSize: "12.5px", color: "#555", marginBottom: "6px" }}>
        试试这些搜索（点一下会带你去主界面）：
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: "6px",
        }}
      >
        {EXAMPLES.map((ex) => (
          <button
            key={ex.query}
            onClick={() => onPickExample(ex.query)}
            style={{
              textAlign: "left",
              backgroundColor: "white",
              border: "1px solid #ddd",
              borderRadius: "7px",
              padding: "7px 10px",
              cursor: "pointer",
            }}
          >
            <div
              style={{
                fontSize: "12.5px",
                fontWeight: 500,
                color: "#1d1d1f",
              }}
            >
              {ex.query}
            </div>
            <div style={{ fontSize: "11px", color: "#888", marginTop: "1px" }}>
              {ex.hint}
            </div>
          </button>
        ))}
      </div>
    </>
  );
};

export default FirstIndexStep;
