import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "../../hooks/useAppSettings";
import { EmbedStatus, ModelStatusJson, embedStatusLine } from "../../lib/model-status";
import { ModelDownloadStep } from "../ModelDownloadStep";

/**
 * 「语义召回」面板：语义参数（相似度下限 / 融合权重）+ 嵌入模型状态与下载
 * + **模型管理**小节（BETA-47 从「常规」迁入：生成模型 fallback 开关 / 状态 /
 * 下载 / 路径覆盖——模型下载与管理归位，一处管所有模型）。
 */
export function SemanticPane({
  settings,
  setSettings,
  embedStatus,
  onEmbedReload,
  modelStatus,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
  embedStatus: EmbedStatus | null;
  onEmbedReload: () => void;
  modelStatus: ModelStatusJson | null;
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

      {/* BETA-47：模型管理小节（原「常规」面板的生成模型部分整体迁入）。 */}
      <div className="prefs-field">
        <label className="prefs-label">模型管理 — 生成模型（Qwen3-0.6B，可选）</label>
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

      {/* BETA-48：embedding 路径覆盖 UI（与下方生成模型对称）。此前该字段只能手工改
          settings.json 且会被 UI 保存冲掉；接口透传修复 + 一并暴露。 */}
      <div className="prefs-field">
        <label className="prefs-label">语义模型路径覆盖（留空用默认）</label>
        <input
          type="text"
          className="prefs-input"
          value={settings.embedding_model_path ?? ""}
          onChange={(e) =>
            setSettings({
              ...settings,
              embedding_model_path: e.target.value || null,
            })
          }
          placeholder="默认：数据目录/models/embeddinggemma-300m-q8_0.gguf"
        />
        <p className="prefs-hint">
          修改路径后建议重启应用生效。留空时默认从上方路径查找。
        </p>
      </div>

      <div className="prefs-field">
        <label className="prefs-label">生成模型路径覆盖（留空用默认）</label>
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
