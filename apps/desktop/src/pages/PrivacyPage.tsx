import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

// BETA-21：与 privacy.rs::DataLocation 对应。
interface DataLocation {
  label: string;
  path: string;
  exists: boolean;
  size_bytes: number;
}

// BETA-21：与 privacy.rs::PrivacyOverview 对应。
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
  // BETA-11D：用户同义词库组数。
  user_synonym_count: number;
  tracing_enabled: boolean;
}

// BETA-12：与 uninstall.rs::CleanupItem / CleanupReport 对应。
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

// 字节数转人类可读（B / KB / MB）。
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

const sectionTitle: React.CSSProperties = {
  fontSize: '18px',
  marginBottom: '12px',
  color: '#007aff',
};

export const PrivacyPage: React.FC = () => {
  const [overview, setOverview] = useState<PrivacyOverview | null>(null);
  // 清除态：'idle' | 'confirming' | 'working'，分别对应 audit / index 两个动作。
  const [clearMsg, setClearMsg] = useState('');
  const [confirmIndex, setConfirmIndex] = useState(false);
  const [working, setWorking] = useState(false);
  // BETA-12 卸载清理：二段确认 + 结果报告。
  const [confirmCleanup, setConfirmCleanup] = useState(false);
  const [cleanupReport, setCleanupReport] = useState<CleanupReport | null>(null);
  const [cleanupMsg, setCleanupMsg] = useState('');

  const loadOverview = () => {
    invoke<PrivacyOverview>('get_privacy_overview').then(setOverview).catch(console.error);
  };

  useEffect(() => {
    loadOverview();
    // 索引可能在后台进行，轻度轮询刷新统计。
    const timer = setInterval(loadOverview, 3000);
    return () => clearInterval(timer);
  }, []);

  const handleClearAudit = async () => {
    setWorking(true);
    setClearMsg('');
    try {
      await invoke('clear_audit_log');
      setClearMsg('操作审计日志已清除');
      loadOverview();
    } catch (err) {
      setClearMsg(`清除失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  const handleClearHistory = async () => {
    setWorking(true);
    setClearMsg('');
    try {
      await invoke('clear_search_history');
      setClearMsg('搜索历史已清除');
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
    setCleanupMsg('');
    setCleanupReport(null);
    setConfirmCleanup(false);
    try {
      const report = await invoke<CleanupReport>('uninstall_cleanup');
      setCleanupReport(report);
      setCleanupMsg(report.all_ok ? '清理完成，设置已保留。现在可以放心卸载 LociFind。' : '部分项目未能删除，详见下表。');
      loadOverview();
    } catch (err) {
      setCleanupMsg(`清理失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  const handleClearIndex = async () => {
    setWorking(true);
    setClearMsg('');
    setConfirmIndex(false);
    try {
      await invoke('clear_local_index');
      setClearMsg('本地索引已清空（下次索引会重建）');
      loadOverview();
    } catch (err) {
      setClearMsg(`清除失败: ${err}`);
    } finally {
      setWorking(false);
    }
  };

  return (
    <div style={{ padding: '24px', maxWidth: '640px', margin: '0 auto', color: '#333', lineHeight: 1.6 }}>
      <h1 style={{ fontSize: '24px', marginBottom: '8px' }}>隐私与数据安全</h1>
      <p style={{ fontSize: '13px', color: '#999', marginBottom: '24px' }}>
        所有数据都在<strong>这台电脑</strong>上。下面如实列出 LociFind 索引了什么、数据存在哪、以及随时一键清除的入口。
      </p>

      {/* 索引了什么 */}
      <section style={{ marginBottom: '32px' }}>
        <h2 style={sectionTitle}>索引了什么</h2>
        {!overview ? (
          <p style={{ fontSize: '14px', color: '#999' }}>读取中…</p>
        ) : !overview.index_available ? (
          <p style={{ fontSize: '14px', color: '#999' }}>
            尚未建立本地索引（可在「设置」页点「立即索引」，或等待启动后台索引完成）。
          </p>
        ) : (
          <>
            <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '8px' }}>
              <StatCard label="音乐" value={overview.music_count} />
              <StatCard label="文档" value={overview.document_count} />
              <StatCard label="图片 (OCR)" value={overview.image_count} />
            </div>
            <p style={{ fontSize: '13px', color: '#666' }}>
              {overview.indexing
                ? '⏳ 正在后台索引…'
                : overview.last_indexed
                  ? `上次索引：${new Date(overview.last_indexed).toLocaleString()}`
                  : ''}
            </p>
          </>
        )}
      </section>

      {/* 日志 / 数据在哪 */}
      <section style={{ marginBottom: '32px' }}>
        <h2 style={sectionTitle}>数据存在哪</h2>
        <p style={{ fontSize: '13px', color: '#666', marginBottom: '12px' }}>
          这些文件仅保存在本机数据目录，<strong>不会上传</strong>。
        </p>
        {overview && (
          <div style={{ border: '1px solid #eee', borderRadius: '8px', overflow: 'hidden' }}>
            <table style={{ width: '100%', fontSize: '13px', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ background: '#fafafa', textAlign: 'left' }}>
                  <th style={{ padding: '8px 12px' }}>类别</th>
                  <th style={{ padding: '8px 12px' }}>路径</th>
                  <th style={{ padding: '8px 12px', whiteSpace: 'nowrap' }}>大小</th>
                </tr>
              </thead>
              <tbody>
                {overview.locations.map((loc, i) => (
                  <tr key={i} style={{ borderTop: '1px solid #f0f0f0' }}>
                    <td style={{ padding: '8px 12px', whiteSpace: 'nowrap' }}>{loc.label}</td>
                    <td
                      style={{ padding: '8px 12px', wordBreak: 'break-all', color: '#666', fontFamily: 'monospace', fontSize: '12px' }}
                      title={loc.path}
                    >
                      {loc.path}
                    </td>
                    <td style={{ padding: '8px 12px', whiteSpace: 'nowrap', color: loc.exists ? '#666' : '#bbb' }}>
                      {loc.exists ? formatBytes(loc.size_bytes) : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {overview && (
          <p style={{ fontSize: '12px', color: '#999', marginTop: '8px' }}>
            数据目录：<code>{overview.data_root}</code>
            {overview.tracing_enabled && ' ・ 调试追踪已开启（日志仅本地）'}
          </p>
        )}
      </section>

      {/* 一键清除 */}
      <section style={{ marginBottom: '32px', backgroundColor: '#fdf6f6', padding: '16px', borderRadius: '8px', border: '1px solid #f3dada' }}>
        <h2 style={{ fontSize: '16px', marginBottom: '8px', color: '#d33' }}>一键清除</h2>
        <p style={{ fontSize: '13px', color: '#666', marginBottom: '16px' }}>
          可随时清除本机数据。清除后不可恢复，但本地索引可通过重新索引重建。
        </p>

        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px', flexWrap: 'wrap' }}>
          <button
            onClick={handleClearAudit}
            disabled={working || !overview || overview.audit_count === 0}
            style={clearBtnStyle(!overview || overview.audit_count === 0 || working)}
          >
            清除操作审计日志
          </button>
          <span style={{ fontSize: '13px', color: '#999' }}>
            {overview ? `${overview.audit_count} 条` : ''}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px', flexWrap: 'wrap' }}>
          <button
            onClick={handleClearHistory}
            disabled={working || !overview || overview.search_history_count === 0}
            style={clearBtnStyle(!overview || overview.search_history_count === 0 || working)}
          >
            清除搜索历史
          </button>
          <span style={{ fontSize: '13px', color: '#999' }}>
            {overview ? `${overview.search_history_count} 条` : ''}
          </span>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
          {!confirmIndex ? (
            <button
              onClick={() => setConfirmIndex(true)}
              disabled={working || !overview || !overview.index_available}
              style={clearBtnStyle(working || !overview || !overview.index_available)}
            >
              清空本地索引
            </button>
          ) : (
            <>
              <span style={{ fontSize: '13px', color: '#d33' }}>确定清空全部本地索引？</span>
              <button
                onClick={handleClearIndex}
                disabled={working}
                style={{ ...clearBtnStyle(working), background: '#d33', color: '#fff', borderColor: '#d33' }}
              >
                确认清空
              </button>
              <button
                onClick={() => setConfirmIndex(false)}
                disabled={working}
                style={clearBtnStyle(working)}
              >
                取消
              </button>
            </>
          )}
        </div>

        {clearMsg && (
          <p style={{ fontSize: '13px', color: clearMsg.includes('失败') ? '#d33' : '#34c759', marginTop: '12px' }}>
            {clearMsg}
          </p>
        )}
      </section>

      {/* BETA-12 卸载清理 */}
      <section style={{ marginBottom: '32px', backgroundColor: '#fdf6f6', padding: '16px', borderRadius: '8px', border: '1px solid #f3dada' }}>
        <h2 style={{ fontSize: '16px', marginBottom: '8px', color: '#d33' }}>卸载清理</h2>
        <p style={{ fontSize: '13px', color: '#666', marginBottom: '12px' }}>
          打算卸载 LociFind？一键删除本机全部派生数据——索引数据库、已下载的模型、运行日志、
          操作审计日志、搜索历史、用户同义词库；<strong>设置文件保留</strong>（重装后配置仍在）。
          Windows 安装版直接运行系统卸载程序即可，卸载时会自动完成同等清理（版本升级不受影响）。
        </p>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
          {!confirmCleanup ? (
            <button onClick={() => setConfirmCleanup(true)} disabled={working} style={clearBtnStyle(working)}>
              清理全部数据（保留设置）
            </button>
          ) : (
            <>
              <span style={{ fontSize: '13px', color: '#d33' }}>
                确定删除索引、模型、日志等全部数据？此操作不可恢复。
              </span>
              <button
                onClick={handleUninstallCleanup}
                disabled={working}
                style={{ ...clearBtnStyle(working), background: '#d33', color: '#fff', borderColor: '#d33' }}
              >
                确认清理
              </button>
              <button onClick={() => setConfirmCleanup(false)} disabled={working} style={clearBtnStyle(working)}>
                取消
              </button>
            </>
          )}
        </div>
        {cleanupMsg && (
          <p style={{ fontSize: '13px', color: cleanupMsg.includes('失败') || cleanupMsg.includes('未能') ? '#d33' : '#34c759', marginTop: '12px' }}>
            {cleanupMsg}
          </p>
        )}
        {cleanupReport && (
          <ul style={{ fontSize: '12px', color: '#666', marginTop: '8px', paddingLeft: '18px' }}>
            {cleanupReport.items.map((item, i) => (
              <li key={i} style={{ color: item.removed ? '#666' : '#d33' }}>
                {item.label}：{item.removed ? (item.existed ? '已删除' : '本来就不存在') : `删除失败（${item.detail ?? '未知原因'}）`}
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* 原有教育性说明（保留） */}
      <section style={{ marginBottom: '32px' }}>
        <h2 style={sectionTitle}>本地优先原则</h2>
        <p>
          LociFind 核心设计理念是<strong>本地优先</strong>。您的搜索索引、查询历史和配置文件均存储在您的本地计算机上，
          不会上传到任何云端服务器。
        </p>
        <ul style={{ paddingLeft: '20px', marginTop: '8px', fontSize: '14px' }}>
          <li>
            <strong>用户同义词库</strong>：你添加的同义词存于本机{' '}
            <code>user-synonyms.yaml</code>，不上传、不同步，可在「我的同义词」页查看 / 删除 / 导出。
            {overview != null && (
              <span style={{ color: '#999', marginLeft: '6px' }}>
                （当前 {overview.user_synonym_count} 组）
              </span>
            )}
          </li>
        </ul>
      </section>

      <section style={{ marginBottom: '32px' }}>
        <h2 style={sectionTitle}>三方服务说明</h2>
        <p>本应用仅在以下情况下可能产生网络请求：</p>
        <ul style={{ paddingLeft: '20px' }}>
          <li>当您手动启用“模型 Fallback”且本地模型无法运行时，可能会请求配置的云端 LLM 接口。</li>
          <li>应用检查更新（可选）。</li>
        </ul>
      </section>

      <p style={{ fontSize: '12px', color: '#999', textAlign: 'center', marginTop: '48px' }}>
        LociFind 开源项目 - 保护您的数字足迹
      </p>
    </div>
  );
};

const StatCard: React.FC<{ label: string; value: number }> = ({ label, value }) => (
  <div
    style={{
      flex: '1 1 120px',
      background: '#f5f7fa',
      borderRadius: '8px',
      padding: '12px 16px',
      textAlign: 'center',
    }}
  >
    <div style={{ fontSize: '22px', fontWeight: 600, color: '#007aff' }}>{value.toLocaleString()}</div>
    <div style={{ fontSize: '13px', color: '#666', marginTop: '2px' }}>{label}</div>
  </div>
);

function clearBtnStyle(disabled: boolean): React.CSSProperties {
  return {
    padding: '6px 16px',
    borderRadius: '6px',
    border: '1px solid #ccc',
    background: '#fff',
    cursor: disabled ? 'not-allowed' : 'pointer',
    fontSize: '14px',
    opacity: disabled ? 0.5 : 1,
  };
}

export default PrivacyPage;
