// BETA-31 / BETA-33 cycle 3 v4：模型下载共用组件（embedding + generation）。
// 用于 Onboarding Step 2、PreferencesDialog NotFound 行下方（旧 SettingsPage 已随 cycle 9 删除）。
import React, { useEffect } from 'react';
import {
  useModelDownload,
  EMBEDDING_MODEL_FALLBACK_URL,
  GENERATION_MODEL_FALLBACK_URL,
  type ModelKind,
} from '../hooks/useModelDownload';

export interface ModelDownloadStepProps {
  onComplete: () => void;
  onSkip?: () => void;
  /// 紧凑模式：用于设置页 inline（无标题 / 无描述、仅按钮 + 进度条）。
  compact?: boolean;
  /// 模型种类。默认 embedding（保持 <=v0.9.3 调用点无参兼容）。
  kind?: ModelKind;
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

interface CopyByKind {
  title: string;
  description: string;
  fallbackUrl: string;
  skipLabel: string;
  buttonLabel: string;
}

function copyFor(kind: ModelKind): CopyByKind {
  if (kind === 'embedding') {
    return {
      title: '下载语义模型（313 MB）',
      description:
        'LociFind 用本地小模型把「按意思找到」做成现实：你输入中文，能召回英文文档；' +
        '记不清文件名，按主题描述也能命中。这一步把模型下载到本地，之后搜索全程不用网络。',
      fallbackUrl: EMBEDDING_MODEL_FALLBACK_URL,
      skipLabel: '稍后下载，先体验关键词搜索',
      buttonLabel: '下载模型',
    };
  }
  return {
    title: '下载生成模型 Qwen3-0.6B（~400 MB，可选）',
    description:
      '仅在解析复杂多条件自然语言查询（如「上周从张三收到的关于 Q3 报表的 PDF」）时才会触发；' +
      '日常关键词与语义召回不需要它。装了之后 parser 覆盖率从 88% 提升到 ~95%+。',
    fallbackUrl: GENERATION_MODEL_FALLBACK_URL,
    skipLabel: '暂不下载（当前搜索已可用）',
    buttonLabel: '下载 Qwen3-0.6B',
  };
}

export const ModelDownloadStep: React.FC<ModelDownloadStepProps> = ({
  onComplete,
  onSkip,
  compact = false,
  kind = 'embedding',
}) => {
  const { status, progress, error, start, cancel } = useModelDownload(kind);
  const copy = copyFor(kind);

  useEffect(() => {
    if (status === 'done') {
      const t = setTimeout(() => onComplete(), 500);
      return () => clearTimeout(t);
    }
  }, [status, onComplete]);

  const percentText =
    progress.percent !== null
      ? `${progress.percent.toFixed(1)}%`
      : progress.downloaded > 0
        ? formatBytes(progress.downloaded)
        : '准备中…';

  const containerStyle: React.CSSProperties = compact
    ? { padding: '12px 0' }
    : { padding: '20px', backgroundColor: '#f0f2f5', borderRadius: '12px', color: '#1d1d1f' };

  return (
    <div style={containerStyle}>
      {!compact && (
        <>
          <h2 style={{ fontSize: '18px', marginBottom: '8px' }}>{copy.title}</h2>
          <p style={{ color: '#555', marginBottom: '16px', lineHeight: 1.6 }}>
            {copy.description}
          </p>
        </>
      )}

      {status === 'idle' && (
        <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
          <button
            onClick={start}
            style={{
              backgroundColor: '#007aff',
              color: 'white',
              border: 'none',
              padding: '10px 24px',
              borderRadius: '8px',
              cursor: 'pointer',
              fontSize: '14px',
              fontWeight: 500,
            }}
          >
            {copy.buttonLabel}
          </button>
          {onSkip && (
            <button
              onClick={onSkip}
              style={{
                backgroundColor: 'transparent',
                color: '#666',
                border: '1px solid #ccc',
                padding: '10px 24px',
                borderRadius: '8px',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              {copy.skipLabel}
            </button>
          )}
        </div>
      )}

      {status === 'downloading' && (
        <div>
          <div style={{ marginBottom: '8px', fontSize: '14px', color: '#1d1d1f' }}>
            {percentText} · {formatBytes(progress.downloaded)}
            {progress.total ? ` / ${formatBytes(progress.total)}` : ''}
          </div>
          <div
            style={{
              height: '8px',
              backgroundColor: '#e0e0e0',
              borderRadius: '4px',
              overflow: 'hidden',
              marginBottom: '12px',
            }}
          >
            <div
              style={{
                height: '100%',
                width: progress.percent !== null ? `${progress.percent}%` : '5%',
                backgroundColor: '#007aff',
                transition: 'width 0.3s ease',
              }}
            />
          </div>
          <button
            onClick={cancel}
            style={{
              backgroundColor: 'transparent',
              color: '#d00',
              border: '1px solid #d00',
              padding: '6px 16px',
              borderRadius: '6px',
              cursor: 'pointer',
              fontSize: '13px',
            }}
          >
            取消
          </button>
        </div>
      )}

      {status === 'done' && (
        <div style={{ color: '#0a0', fontSize: '14px' }}>
          ✓ 模型已就绪。{!compact && '即将进入下一步。'}
        </div>
      )}

      {status === 'error' && (
        <div>
          <div style={{ color: '#d00', marginBottom: '12px', fontSize: '14px' }}>
            下载失败：{error || '未知错误'}
          </div>
          <p style={{ fontSize: '13px', color: '#555', lineHeight: 1.6 }}>
            网络问题？可手动下载 GGUF 文件并放到 app 数据目录的 <code>models/</code> 子目录：
          </p>
          <a
            href={copy.fallbackUrl}
            target="_blank"
            rel="noreferrer"
            style={{ color: '#007aff', fontSize: '13px', wordBreak: 'break-all' }}
          >
            {copy.fallbackUrl}
          </a>
          <div style={{ marginTop: '12px', display: 'flex', gap: '12px' }}>
            <button
              onClick={start}
              style={{
                backgroundColor: '#007aff',
                color: 'white',
                border: 'none',
                padding: '8px 20px',
                borderRadius: '6px',
                cursor: 'pointer',
                fontSize: '13px',
              }}
            >
              重试
            </button>
            {onSkip && (
              <button
                onClick={onSkip}
                style={{
                  backgroundColor: 'transparent',
                  color: '#666',
                  border: '1px solid #ccc',
                  padding: '8px 20px',
                  borderRadius: '6px',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                稍后下载
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
};

export default ModelDownloadStep;
