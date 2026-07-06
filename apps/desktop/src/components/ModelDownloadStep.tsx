// BETA-31 / BETA-33 cycle 3 v4：模型下载共用组件（embedding + generation）。
// 用于 Onboarding Step 2、PreferencesDialog NotFound 行下方（旧 SettingsPage 已随 cycle 9 删除）。
// 2026-07-06（cycle 9 真机反馈）：下载 UI 前先做本地发现——默认路径已有 → 直接就绪；
// 否则经 Everything 精确文件名全盘发现候选，「使用此文件」复制进默认目录（免重下 ~700MB）。
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  useModelDownload,
  EMBEDDING_MODEL_FALLBACK_URL,
  GENERATION_MODEL_FALLBACK_URL,
  type ModelKind,
} from '../hooks/useModelDownload';

interface LocalModelCandidate {
  path: string;
  size_bytes: number;
}

interface DiscoverResult {
  present: boolean;
  expected_path: string;
  candidates: LocalModelCandidate[];
  everything_available: boolean;
}

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

  // 本地发现：mount 时查默认路径 + Everything 候选。失败静默降级为原下载 UI。
  const [discover, setDiscover] = useState<DiscoverResult | null>(null);
  const [importing, setImporting] = useState<string | null>(null);
  const [importError, setImportError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    invoke<DiscoverResult>('discover_local_model', { kind })
      .then((r) => {
        if (alive) setDiscover(r);
      })
      .catch((e) => {
        console.error('[ModelDownloadStep] discover_local_model failed:', e);
      });
    return () => {
      alive = false;
    };
  }, [kind]);

  useEffect(() => {
    if (status === 'done') {
      const t = setTimeout(() => onComplete(), 500);
      return () => clearTimeout(t);
    }
  }, [status, onComplete]);

  // 默认路径已有完整模型：直接就绪、免下载（与 done 分支同款 500ms 进下一步）。
  useEffect(() => {
    if (discover?.present && status === 'idle') {
      const t = setTimeout(() => onComplete(), 500);
      return () => clearTimeout(t);
    }
  }, [discover, status, onComplete]);

  const importFrom = async (path: string) => {
    setImporting(path);
    setImportError(null);
    try {
      // 成功后 Rust 侧 emit 与下载一致的 done event → status 变 'done' → 既有流程收尾。
      await invoke<string>('import_local_model', { kind, source: path });
    } catch (e) {
      setImportError(String(e));
    } finally {
      setImporting(null);
    }
  };

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

      {status === 'idle' && discover?.present && (
        <div style={{ color: '#0a0', fontSize: '14px' }}>
          ✓ 已在本机检测到模型（{discover.expected_path}），无需下载。
          {!compact && '即将进入下一步。'}
        </div>
      )}

      {status === 'idle' && !discover?.present && (
        <div>
          {/* 本地发现候选：Everything 按精确文件名找到的同款模型，复制即用免重下。 */}
          {discover && discover.candidates.length > 0 && (
            <div
              style={{
                marginBottom: '14px',
                padding: '10px 12px',
                border: '1px solid #b7d8ff',
                backgroundColor: '#f2f8ff',
                borderRadius: '8px',
              }}
            >
              <div style={{ fontSize: '13.5px', fontWeight: 600, marginBottom: '6px' }}>
                在本机找到已有的模型文件，可直接使用（复制进数据目录，免下载）：
              </div>
              {discover.candidates.map((c) => (
                <div
                  key={c.path}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '4px 0',
                    fontSize: '12.5px',
                  }}
                >
                  <span style={{ flex: 1, wordBreak: 'break-all' }} title={c.path}>
                    📦 {c.path}{' '}
                    <span style={{ color: '#888' }}>({formatBytes(c.size_bytes)})</span>
                  </span>
                  <button
                    onClick={() => void importFrom(c.path)}
                    disabled={importing !== null}
                    style={{
                      backgroundColor: importing === c.path ? '#9cc4f5' : '#007aff',
                      color: 'white',
                      border: 'none',
                      padding: '5px 14px',
                      borderRadius: '6px',
                      cursor: importing !== null ? 'default' : 'pointer',
                      fontSize: '12.5px',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {importing === c.path ? '复制中…' : '使用此文件'}
                  </button>
                </div>
              ))}
              {importError && (
                <div style={{ color: '#d00', fontSize: '12.5px', marginTop: '4px' }}>
                  导入失败：{importError}
                </div>
              )}
            </div>
          )}
          {discover && !discover.everything_available && !compact && (
            <p style={{ fontSize: '12px', color: '#888', margin: '0 0 10px' }}>
              未检测到 Everything（es.exe），无法自动发现本机已有模型；如已有同款
              .gguf，可手动放到数据目录 <code>models/</code> 后回到本步。
            </p>
          )}
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
