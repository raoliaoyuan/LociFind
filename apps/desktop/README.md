# locifind-desktop

LociFind 桌面端应用（基于 Tauri 2 + React）。

## 目录结构

- `src/`: React / TypeScript 前端代码。
- `src-tauri/`: Rust 后端代码（Tauri Command 与集成）。
- `dist/`: 前端构建产物（被忽略）。

## 模块集成指南

本应用采用模块化设计，新功能模块需在 `main.rs` 和 `App.tsx` 中注册。

### Rust 端 (src-tauri/src/main.rs)

1. **导入模块**:
   ```rust
   mod shortcut;
   mod status;
   mod settings;
   ```

2. **注册全局快捷键**:
   在 `tauri::Builder::default()` 的 `.setup` 中调用：
   ```rust
   .setup(|app| {
       shortcut::register_global_shortcut(app.handle())?;
       Ok(())
   })
   ```

3. **注册命令**:
   在 `.invoke_handler` 中添加：
   ```rust
   .invoke_handler(tauri::generate_handler![
       status::get_backend_status,
       settings::get_settings,
       settings::update_settings
   ])
   ```

4. **初始化插件**:
   在 `Builder` 链中添加插件：
   ```rust
   .plugin(tauri_plugin_global_shortcut::Builder::new().build())
   ```

### 前端 (src/App.tsx)

1. **导入组件**:
   ```tsx
   import { ShortcutBanner } from './components/ShortcutBanner';
   import { StatusIndicator } from './components/StatusIndicator';
   import { SettingsPage } from './pages/SettingsPage';
   import { PrivacyPage } from './pages/PrivacyPage';
   ```

2. **挂载组件**:
   - `ShortcutBanner` 建议放在根部，它会自动消失。
   - `StatusIndicator` 建议放在顶栏或状态栏。
   - `SettingsPage` 和 `PrivacyPage` 建议配合 `react-router-dom` 使用。

## 开发与构建

### 依赖准备

确保本地已安装 Node.js 与 Rust 环境。

```bash
# 安装前端依赖
cd apps/desktop
npm install
```

### 开发模式

```bash
# 启动 Tauri 开发环境（含前端热更新）
cd apps/desktop
npm run tauri dev
```

### 构建应用

```bash
# 构建生产包
cd apps/desktop
npm run tauri build
```

## 当前进度 (MVP-18)

- [x] Tauri 2 + React + Vite 骨架搭建。
- [x] Rust 后端 Tauri Command 闭环（MVP-18 的 `echo` demo 已被正式 `search` 等命令取代并移除）。
- [x] 前端搜索框与回显交互。
- [x] 成功跑通 `npm run build` 与 `cargo build`。
- [x] MVP-22: 应用设置页（快捷键/Fallback开关）。
- [x] MVP-23/24: macOS FDA 与 Windows 索引引导页。

## 模块集成指南

本应用采用模块化设计，新功能模块需在 `main.rs` 和 `App.tsx` 中注册。

### Rust 端 (src-tauri/src/main.rs)

1. **导入模块**:
   ```rust
   mod shortcut;
   mod status;
   mod settings;
   mod permissions; // MVP-23/24
   ```

2. **注册命令**:
   在 `.invoke_handler` 中添加：
   ```rust
   .invoke_handler(tauri::generate_handler![
       status::get_backend_status,
       settings::get_settings,
       settings::update_settings,
       // MVP-23/24 Permissions & Onboarding
       permissions::check_macos_full_disk_access,
       permissions::open_macos_fda_settings,
       permissions::check_windows_search_indexed,
       permissions::open_windows_indexing_options,
       permissions::get_onboarding_state,
       permissions::complete_onboarding,
   ])
   ```

### 前端 (src/App.tsx)

1. **路由集成**:
   在 `Routes` 中添加：
   ```tsx
   <Route path="/onboarding/mac" element={<OnboardingMac />} />
   <Route path="/onboarding/win" element={<OnboardingWin />} />
   ```

2. **自动引导逻辑**:
   建议在 `App.tsx` 中使用 `useShouldShowOnboarding` hook：
   ```tsx
   const onboarding = useShouldShowOnboarding();
   useEffect(() => {
     if (onboarding === 'macos') navigate('/onboarding/mac');
     if (onboarding === 'windows') navigate('/onboarding/win');
   }, [onboarding]);
   ```
