import { AppSettings } from "../../hooks/useAppSettings";

/**
 * 「常规」面板：全局唤起快捷键。
 * BETA-47：生成模型 fallback / 模型路径覆盖迁往「语义召回 → 模型管理」小节
 * （模型下载与管理归位，一处管所有模型）。
 */
export function GeneralPane({
  settings,
  setSettings,
}: {
  settings: AppSettings;
  setSettings: (s: AppSettings) => void;
}) {
  return (
    <div className="prefs-form">
      <div className="prefs-field">
        <label className="prefs-label">全局唤起快捷键</label>
        <input
          type="text"
          className="prefs-input"
          value={settings.global_shortcut}
          onChange={(e) =>
            setSettings({ ...settings, global_shortcut: e.target.value })
          }
          disabled
        />
        <p className="prefs-hint">当前版本暂不支持修改快捷键。</p>
      </div>
    </div>
  );
}
