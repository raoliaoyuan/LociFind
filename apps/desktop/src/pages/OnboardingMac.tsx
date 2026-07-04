// macOS 版快速入门（5 步）：
// 1. 授予完全磁盘访问权限（FDA）
// 2. 下载嵌入模型（必需）
// 3. 下载生成模型 Qwen3-0.6B（可选）
// 4. 配置索引目录
// 5. 首次索引 + 试试搜索
//
// Mac 上不涉及 Everything，故比 Windows 少一步；其余步骤组件复用。
// 每步都提供 skipAction（"跳过此步"）保证用户不会被困在任何一步。
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from 'react-router-dom';
import { ModelDownloadStep } from '../components/ModelDownloadStep';
import OnboardingShell from '../components/onboarding/OnboardingShell';
import IndexRootsStep from '../components/onboarding/IndexRootsStep';
import FirstIndexStep from '../components/onboarding/FirstIndexStep';

type Step = 1 | 2 | 3 | 4 | 5;
const TOTAL_STEPS = 5;
type FdaStatus = 'Granted' | 'NotGranted' | 'Unknown' | 'Loading';

export const OnboardingMac: React.FC = () => {
  const [step, setStep] = useState<Step>(1);
  const [fdaStatus, setFdaStatus] = useState<FdaStatus>('Loading');
  const navigate = useNavigate();

  useEffect(() => {
    if (step !== 1) return;
    const check = async () => {
      try {
        const res = await invoke<'Granted' | 'NotGranted' | 'Unknown'>(
          'check_macos_full_disk_access',
        );
        setFdaStatus(res);
      } catch (err) {
        console.error(err);
        setFdaStatus('Unknown');
      }
    };
    void check();
    const timer = setInterval(check, 2000);
    return () => clearInterval(timer);
  }, [step]);

  const goTo = (next: Step) => setStep(next);

  const handleOpenFdaSettings = async () => {
    try {
      await invoke('open_macos_fda_settings');
    } catch (err) {
      alert(`无法打开设置: ${err}`);
    }
  };

  const markStep1Done = async () => {
    try {
      await invoke('complete_onboarding', { feature: 'macos_fda' });
    } catch (err) {
      console.error('[OnboardingMac] complete_onboarding macos_fda:', err);
    }
    goTo(2);
  };

  const markEmbeddingDone = async () => {
    try {
      await invoke('complete_onboarding', { feature: 'model_download' });
    } catch (err) {
      console.error('[OnboardingMac] complete_onboarding model_download:', err);
    }
    goTo(3);
  };

  const handlePickExample = (query: string) => {
    navigate(`/?q=${encodeURIComponent(query)}`);
  };

  const handleFinishOnboarding = () => {
    navigate('/');
  };

  return (
    <>
      {step === 1 && (
        <OnboardingShell
          totalSteps={TOTAL_STEPS}
          currentStep={1}
          title="第 1 步：授予完全磁盘访问权限"
          subtitle="必需 · Spotlight 才能索引到邮件、备份、系统等受保护目录"
          primaryAction={{
            label: fdaStatus === 'Granted' ? '已授权，下一步' : '我已设置好，下一步',
            onClick: markStep1Done,
          }}
          skipAction={{
            label: '跳过此步',
            onClick: markStep1Done,
          }}
          footerNote="LociFind 仅在本地运行，你的数据永远不会离开你的设备。"
        >
          {fdaStatus === 'Granted' && (
            <div
              style={{
                padding: '10px 14px',
                borderRadius: '10px',
                backgroundColor: '#e8f7ee',
                border: '1px solid #b7e4c7',
                marginBottom: '8px',
              }}
            >
              <div style={{ color: '#0a7a3b', fontSize: '14px', marginBottom: '2px' }}>
                ✅ 已获得完全磁盘访问权限
              </div>
              <div style={{ fontSize: '12.5px', color: '#446854' }}>
                LociFind 现在可以搜索您的所有文件了。
              </div>
            </div>
          )}

          {fdaStatus !== 'Granted' && (
            <>
              <p
                style={{
                  color: '#555',
                  margin: 0,
                  marginBottom: '10px',
                  lineHeight: 1.55,
                  fontSize: '13px',
                }}
              >
                为了让 LociFind 能通过 Spotlight 索引搜索到所有文档（邮件、备份、系统文件等），
                需要手动授予「完全磁盘访问权限」（FDA）。
              </p>

              <div
                style={{
                  backgroundColor: '#f5f5f7',
                  padding: '10px 14px',
                  borderRadius: '10px',
                  marginBottom: '8px',
                  color: '#1d1d1f',
                  fontSize: '12.5px',
                  lineHeight: 1.6,
                }}
              >
                <div style={{ fontWeight: 600, marginBottom: '4px' }}>操作步骤</div>
                <ol style={{ paddingLeft: '18px', margin: 0 }}>
                  <li>点下方「打开系统设置」</li>
                  <li>在「完全磁盘访问权限」面板里点 <strong>+</strong> 添加 LociFind</li>
                  <li>勾选 <strong>LociFind</strong> 旁的开关</li>
                  <li>提示需要退出 LociFind 时点「现在退出」，或稍后手动重启</li>
                </ol>
              </div>

              <div
                style={{
                  backgroundColor: '#fff7e6',
                  padding: '8px 12px',
                  borderRadius: '10px',
                  marginBottom: '10px',
                  color: '#5c3c00',
                  border: '1px solid #ffe3a3',
                  fontSize: '11.5px',
                  lineHeight: 1.55,
                }}
              >
                <strong>开发模式说明：</strong> dev 下跑的是{' '}
                <code>target/debug/locifind-desktop</code>——不是签名的
                .app，FDA 列表里默认看不到 LociFind。临时方案：给运行{' '}
                <code>npm run tauri dev</code> 的 Terminal.app 加 FDA 权限即可（子进程继承）。
              </div>

              <button
                onClick={handleOpenFdaSettings}
                style={{
                  backgroundColor: '#007aff',
                  color: 'white',
                  border: 'none',
                  padding: '7px 18px',
                  borderRadius: '7px',
                  cursor: 'pointer',
                  fontSize: '13px',
                  fontWeight: 500,
                }}
              >
                打开系统设置
              </button>
            </>
          )}
        </OnboardingShell>
      )}

      {step === 2 && (
        <OnboardingShell
          totalSteps={TOTAL_STEPS}
          currentStep={2}
          title="第 2 步：下载嵌入模型"
          subtitle="必需 · 让 LociFind 能按「意思」检索，而不仅是关键词匹配"
          skipAction={{
            label: '跳过此步（稍后下载）',
            onClick: markEmbeddingDone,
          }}
          secondaryAction={{
            label: '上一步',
            onClick: () => goTo(1),
          }}
          footerNote="嵌入模型只在本地运行；下载后全程无需联网。"
        >
          <ModelDownloadStep
            kind="embedding"
            onComplete={markEmbeddingDone}
          />
        </OnboardingShell>
      )}

      {step === 3 && (
        <OnboardingShell
          totalSteps={TOTAL_STEPS}
          currentStep={3}
          title="第 3 步：下载生成模型（可选）"
          subtitle="可选 · 用于解析复杂多条件的自然语言查询"
          primaryAction={{
            label: '下一步',
            onClick: () => goTo(4),
          }}
          skipAction={{
            label: '跳过此步（不下载）',
            onClick: () => goTo(4),
          }}
          secondaryAction={{
            label: '上一步',
            onClick: () => goTo(2),
          }}
          footerNote="不装也不影响关键词与语义搜索；装了后类似「上周从张三收到的 Q3 PDF」这类复杂 query 解析成功率更高。"
        >
          <ModelDownloadStep
            kind="generation"
            onComplete={() => goTo(4)}
          />
        </OnboardingShell>
      )}

      {step === 4 && (
        <OnboardingShell
          totalSteps={TOTAL_STEPS}
          currentStep={4}
          title="第 4 步：配置索引目录"
          subtitle="必需 · 决定 LociFind 会在哪些文件夹里搜索"
          primaryAction={{
            label: '下一步',
            onClick: () => goTo(5),
          }}
          skipAction={{
            label: '跳过此步（用默认）',
            onClick: () => goTo(5),
          }}
          secondaryAction={{
            label: '上一步',
            onClick: () => goTo(3),
          }}
        >
          <IndexRootsStep />
        </OnboardingShell>
      )}

      {step === 5 && (
        <OnboardingShell
          totalSteps={TOTAL_STEPS}
          currentStep={5}
          title="第 5 步：首次索引 + 试试搜索"
          subtitle="点「开始扫描并索引」启动首轮；索引跑起来后你可以随时进主界面"
          primaryAction={{
            label: '完成，进入主界面',
            onClick: handleFinishOnboarding,
          }}
          skipAction={{
            label: '跳过此步（不启动首轮）',
            onClick: handleFinishOnboarding,
          }}
          secondaryAction={{
            label: '上一步',
            onClick: () => goTo(4),
          }}
        >
          <FirstIndexStep
            onPickExample={handlePickExample}
            onFinish={handleFinishOnboarding}
          />
        </OnboardingShell>
      )}
    </>
  );
};

export default OnboardingMac;
