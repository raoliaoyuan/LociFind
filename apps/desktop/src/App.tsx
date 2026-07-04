import { useEffect, useRef, useState } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import SearchView from "./SearchView";
import MenuBar from "./components/MenuBar";
import PreferencesDialog from "./components/PreferencesDialog";
import ShortcutBanner from "./components/ShortcutBanner";
import StatusIndicator from "./components/StatusIndicator";
import { useShouldShowOnboarding } from "./hooks/useShouldShowOnboarding";
import { onMenuAction } from "./lib/menu-events";
import OnboardingMac from "./pages/OnboardingMac";
import OnboardingWin from "./pages/OnboardingWin";
import PrivacyPage from "./pages/PrivacyPage";
import UserSynonymsPage from "./pages/UserSynonymsPage";

// MVP-19/20/21/22/23/24 集成：
// - / → 搜索主视图
// - /privacy → 隐私（设置走模态 PreferencesDialog；旧 /settings 路由已随
//   BETA-33 cycle 9 删除——cycle 3 起无任何导航入口，仅剩路由注册本身）
// - /onboarding/mac /onboarding/win → 首次启动权限引导
// - 启动时根据 OS 自动跳转到对应 onboarding（已完成的不再跳）
function App() {
  const navigate = useNavigate();
  const location = useLocation();
  const onboarding = useShouldShowOnboarding();
  const [showPrefs, setShowPrefs] = useState(false);
  // 快速入门第 5 步「配置索引目录」直接打开选项对话框时用；null = 用默认「常规」。
  const [prefsInitialCategory, setPrefsInitialCategory] = useState<
    "general" | "semantic" | "indexing" | "privacy" | null
  >(null);

  // BETA-33 cycle 3：监听菜单事件 `open-prefs`、打开模态选项对话框（替代旧 navigate("/settings")）。
  // 快速入门追加：`open-prefs-indexing` 打开对话框并直接切到「索引」分类。
  useEffect(
    () =>
      onMenuAction((a) => {
        if (a === "open-prefs") {
          setPrefsInitialCategory(null);
          setShowPrefs(true);
        } else if (a === "open-prefs-indexing") {
          setPrefsInitialCategory("indexing");
          setShowPrefs(true);
        }
      }),
    [],
  );

  // 仅在启动后自动跳转 onboarding **一次**：跳转过之后（含用户在 onboarding 点「进入应用」、
  // navigate('/') 回到搜索）不再自动把用户拉回。否则 useShouldShowOnboarding 的状态在本次会话内
  // 不刷新（其 useEffect 依赖为空、仅 mount 检测一次），完成索引设置后 shouldShow 仍是 'windows'，
  // 会把用户反复弹回 onboarding，需重启才能进搜索界面。
  const hasAutoRedirected = useRef(false);
  useEffect(() => {
    // 已在 onboarding 路由上不重复跳
    if (location.pathname.startsWith("/onboarding")) {
      return;
    }
    if (hasAutoRedirected.current) {
      return;
    }
    if (onboarding === "macos") {
      hasAutoRedirected.current = true;
      navigate("/onboarding/mac");
    } else if (onboarding === "windows") {
      hasAutoRedirected.current = true;
      navigate("/onboarding/win");
    }
  }, [onboarding, navigate, location.pathname]);

  return (
    <div className="container">
      <header className="app-header">
        <MenuBar />
        <StatusIndicator />
      </header>
      <ShortcutBanner />
      <main className="app-main">
        <Routes>
          <Route path="/" element={<SearchView />} />
          <Route path="/privacy" element={<PrivacyPage />} />
          <Route path="/synonyms" element={<UserSynonymsPage />} />
          <Route path="/onboarding/mac" element={<OnboardingMac />} />
          <Route path="/onboarding/win" element={<OnboardingWin />} />
        </Routes>
      </main>
      {showPrefs && (
        <PreferencesDialog
          onClose={() => setShowPrefs(false)}
          initialCategory={prefsInitialCategory ?? undefined}
        />
      )}
    </div>
  );
}

export default App;
