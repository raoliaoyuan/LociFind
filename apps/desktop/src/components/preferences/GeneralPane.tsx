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
      <div className="prefs-field">
        <label className="prefs-label">多条件检索匹配方式</label>
        <select
          className="prefs-input"
          value={settings.search_match_all_conditions ? "all" : "any"}
          onChange={(e) =>
            setSettings({
              ...settings,
              search_match_all_conditions: e.target.value === "all",
            })
          }
        >
          <option value="all">全部条件都命中（推荐，更精确）</option>
          <option value="any">任一条件命中即可（更宽泛，结果更多）</option>
        </select>
        <p className="prefs-hint">
          搜索词被拆成多个条件（如同义词组）时，「全部命中」要求每个条件都满足，避免只符合部分条件的结果混入；
          「任一命中」放宽为只要满足一个条件就返回，召回更广但可能包含较多不相关结果。
        </p>
      </div>
    </div>
  );
}
