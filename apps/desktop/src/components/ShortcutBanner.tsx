import React, { useEffect, useState } from 'react';

/**
 * ShortcutBanner 组件
 * 
 * 在应用启动时显示 5 秒钟，告知用户全局唤起快捷键。
 */
export const ShortcutBanner: React.FC = () => {
  const [visible, setVisible] = useState(true);
  const [isMac] = useState(() => navigator.userAgent.includes('Mac'));

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(false);
    }, 5000);

    return () => clearTimeout(timer);
  }, []);

  if (!visible) return null;

  const shortcut = isMac ? '⌥ Space' : 'Ctrl + Space';

  return (
    <div style={{
      position: 'fixed',
      top: '20px',
      left: '50%',
      transform: 'translateX(-50%)',
      backgroundColor: '#ffffff',
      color: '#1d1d1f',
      padding: '10px 20px',
      borderRadius: '8px',
      zIndex: 1000,
      fontSize: '14px',
      boxShadow: '0 4px 14px rgba(0,0,0,0.18)',
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      border: '1px solid #e2e2e2',
      animation: 'fadeIn 0.3s ease-out'
    }}>
      <span style={{ fontWeight: 'bold' }}>LociFind 已就绪</span>
      <span style={{ opacity: 0.4 }}>|</span>
      <span>使用 <kbd style={{
        backgroundColor: '#f3f3f3',
        color: '#333',
        border: '1px solid #d2d2d7',
        padding: '2px 6px',
        borderRadius: '4px',
        fontFamily: 'monospace'
      }}>{shortcut}</kbd> 随时唤起</span>
      <button
        onClick={() => setVisible(false)}
        style={{
          background: 'none',
          border: 'none',
          color: '#888',
          cursor: 'pointer',
          padding: '4px',
          marginLeft: '10px',
          fontSize: '16px',
          lineHeight: 1
        }}
      >
        ×
      </button>

      <style>{`
        @keyframes fadeIn {
          from { opacity: 0; transform: translate(-50%, -10px); }
          to { opacity: 1; transform: translate(-50%, 0); }
        }
      `}</style>
    </div>
  );
};

export default ShortcutBanner;
