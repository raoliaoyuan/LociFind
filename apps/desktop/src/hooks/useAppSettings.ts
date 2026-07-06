// BETA-33 cycle 9：AppSettings 类型 + 加载/保存/未保存判定的单一信源 hook。
//
// 此前 `AppSettings` 接口与「get_settings 加载 → 本地编辑 → update_settings 保存 →
// 快照重置」整套流在 PreferencesDialog 与 SettingsPage 各复制一份（~120 行漂移面）。
// 本 cycle 删除旧 `/settings` 路由 + SettingsPage（PreferencesDialog 自 cycle 3 起是
// 唯一 UI 入口、旧路由已无任何导航入口），设置流收拢到本 hook——未来任何新表面
// （如 onboarding 步骤）需要读改 settings 一律从这里取，不再复制。
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** 单条 per-root 排除，patterns 是相对 root 的 path glob 列表（cycle 7-b）。 */
export interface RootExclude {
  root: string;
  patterns: string[];
}

/** 与后端 settings.rs `AppSettings` serde 对应（字段注释见各字段首次引入的 cycle）。 */
export interface AppSettings {
  global_shortcut: string;
  search_scope: string[];
  enable_model_fallback: boolean;
  enable_tracing: boolean;
  model_path: string | null;
  /** BETA-48：embedding 模型路径覆盖（null = 默认数据目录 models/）。
   *  此前接口缺该字段，`update_settings` 全量覆写会把用户手工写进
   *  settings.json 的值经 serde default 静默冲掉——必须透传。 */
  embedding_model_path: string | null;
  semantic_similarity_floor: number | null;
  semantic_weight: number | null;
  index_roots: string[];
  /** 是否纳入系统默认三夹（音乐/文档/图片）。默认 false。
   *  2026-07-06 起与 index_roots 空否解耦：不勾 + 无自定义 = 默认零索引。 */
  include_system_defaults: boolean;
  /** BETA-39：图片 OCR 文本参与语义索引 opt-in（默认 false，防乱码 OCR 污染召回）。 */
  enable_image_semantics: boolean;
  /** BETA-47：Everything 集成总开关（默认 true）。关闭停用搜索加速（需重启）、
   *  索引期音乐全盘发现与模型本地发现（live 生效）三处 es.exe 调用。 */
  enable_everything: boolean;
  exclude_globs: string[];
  /** cycle 7-b：per-root 子路径排除（相对 root 的 path glob）。 */
  root_excludes: RootExclude[];
}

/**
 * 设置的加载 / 编辑 / 保存流。
 *
 * - 挂载时 `get_settings` 一次，同步存 `initialSettings` 快照（识别 pending 改动 +
 *   未保存关闭前二次确认，cycle 7-a 语义保持不变）。
 * - `save()` 走 `update_settings`；成功后把当前 settings 快照进 initialSettings、
 *   置「设置已保存」3s 自清；失败置错误 message。返回是否成功（「确定」按钮用）。
 * - `message` / `setMessage` 一并暴露：调用方的其他轻提示（如 picker 反馈）复用同一条状态行。
 */
export function useAppSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [initialSettings, setInitialSettings] = useState<AppSettings | null>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  useEffect(() => {
    invoke<AppSettings>("get_settings")
      .then((s) => {
        setSettings(s);
        setInitialSettings(s);
      })
      .catch(console.error);
  }, []);

  // cycle 7-a：是否有未保存改动（sticky 提示 + 关闭前二次确认用）。
  const hasUnsavedChanges = useMemo(() => {
    if (!settings || !initialSettings) return false;
    return JSON.stringify(settings) !== JSON.stringify(initialSettings);
  }, [settings, initialSettings]);

  const save = async (): Promise<boolean> => {
    if (!settings) return false;
    setSaving(true);
    setMessage("");
    try {
      await invoke("update_settings", { settings });
      // cycle 7-a：应用成功后把当前 settings snapshot 到 initialSettings、清 pending/未保存状态。
      setInitialSettings(settings);
      setMessage("设置已保存");
      setTimeout(() => setMessage(""), 3000);
      return true;
    } catch (err) {
      setMessage(`保存失败: ${err}`);
      return false;
    } finally {
      setSaving(false);
    }
  };

  return {
    settings,
    setSettings,
    initialSettings,
    hasUnsavedChanges,
    save,
    saving,
    message,
    setMessage,
  };
}
