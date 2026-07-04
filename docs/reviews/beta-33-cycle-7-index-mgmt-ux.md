# BETA-33 cycle 7 — 索引目录管理 UX 升级 + 子路径排除通配符（design doc）

| 项 | 值 |
|---|---|
| 起草日期 | 2026-07-01 |
| 起草者 | Claude Code (Opus 4.7) |
| 待评审 | **Codex**（评审入口在文末 §9） |
| 承接 | [BETA-33 cycle 6 v4 (1781fd3)](https://github.com/raoliaoyuan/LociFind/commit/1781fd3) — `include_system_defaults` checkbox + `IndexStatus.current_root` + `fts_progress` bridge |
| 目标版本 | v0.9.7（三刀合一出货） |
| 估时 | 1-1.5 d（拆 7-a UX 2h + 7-b 子路径排除 0.5d + 7-c 老 follow-up 0.5d） |
| 目标平台 | Windows 优先（Sogou IME 场景已 covered）+ 后期 macOS 一致 |

---

## 1. 背景与动机

### 1.1 用户在 v0.9.6 装机后报告的两个 UX 缺口

来自 2026-07-01 会话截图 + 描述：

1. **「添加了目录后无法正常在上面的索引目录中显示」** —— 点「+ 添加目录」picker 选完后，UI 上"感觉"没有反应；用户不确定是否真加进去了。
2. **「本地索引进展过程中无法看到正在实施哪些索引」** —— 索引运行时只有一行文字「正在后台索引…」，缺乏"当前扫哪个目录、进度多少、什么类型"的可视化。
3. **诉求**：友好统一的索引目录管理界面——一眼看到目录数、每目录内容分布、当前正在索引的目录。
4. **新特性诉求**：**支持在索引目录下用通配符排除特定子目录**（例如 `Documents/临时/**`）。

### 1.2 v0.9.6 现状对照

| 用户诉求 | v0.9.6 代码状态 | 备注 |
|---|---|---|
| 添加目录后能立即看到 | ✅ **代码层已修** | `effectiveRoots` 跟 `settings.index_roots` useState 走（[PreferencesDialog.tsx:169-185](../../apps/desktop/src/components/PreferencesDialog.tsx#L169)）；picker 完成后 `setSettings` 触发 useEffect 重 fetch |
| 索引时看到当前目录 | ✅ **代码层已加** | `IndexStatus.current_root` + `fts_progress` bridge（[index_status.rs](../../apps/desktop/src-tauri/src/search/index_status.rs)）；UI 显示「⏳ 正在索引：📁 …　已扫描 N · 已入库 M」 |
| 每目录分类统计 | ✅ **已在 cycle 5** | `RootRow` 显示 📄 X · 🖼 Y · 🎵 Z + 上次索引 |
| 子路径通配符排除 | ❌ **缺** | 当前 `exclude_globs` 走 `is_excluded_dir`（[scan.rs:64](../../packages/indexer/src/scan.rs#L64)）只匹配 **basename**（`node_modules`、`*cache*`），**不能剪特定子路径** |

### 1.3 v0.9.6 首次真机验证的诊断分歧

STATUS cycle 6 v4 记录：**"接受标准（本机 cargo + tsc 全测过）… 真机验证留 v0.9.5 release 装机后"**——但 v0.9.5 → v0.9.6 直接跳，本次是**首次真机验证**。用户报告与代码期望不一致，起草时列了三种可能：

- **A. 真 bug**：picker 完成后 useEffect 未触发（或触发但 `effectiveRoots` fetch 失败静默）。
- **B. UX 不明显**：新目录确实显示在列表里，但没有"⏳ 待应用"badge 提示"未保存"，用户以为"没反应"直接关对话框，改动丢失。
- **C. 心智模型错位**：加自定义后系统默认三夹**消失**（覆盖语义）、Downloads 单独一行、顶部计数"3 目录 → 1 目录"变小——用户看到"目录数变少 + 熟悉的 Music/Documents/Pictures 都不见了"，误判"没加成功"或"丢了 3 个"。

### 1.4 【已诊断】2026-07-01 Claude Code 真机复现结论

**方法**：state 注入法——修改 settings.json 塞 `index_roots: ["C:\\Users\\Alice\\Downloads"]` + `include_system_defaults: false` 模拟"用户 picker 完成后未保存前"状态，重启 v0.9.6 NSIS 观察 UI 渲染（因 computer-use screenshot 对 msedgewebview2 mask 失效，改用 PowerShell 原生 System.Drawing.CopyFromScreen + Win32 SetCursorPos/mouse_event 驱动，DPI 125% 补偿）。

**观察到的 v0.9.6 索引 pane 实际渲染**（截图证据 `%TEMP%\locifind-shot.png`）：

- ✅ **索引概貌**：`1 目录 · 61 总条数 · 44 文档 · 15 图片 · 2 音乐 · N 分钟前`——数字正确、跟 Downloads 匹配
- ✅ **索引目录（当前 1 个自定义 + 0 系统默认）**——计数正确
- ✅ **Downloads 行显示**：`📁 C:\Users\Alice\Downloads    📄 44 · 🖼 15 · 🎵 2    N 分钟前    [移除]`
- ✅ **checkbox「同时索引系统默认目录（音乐 / 文档 / 图片）」显示**（cycle 6 v4 新加）
- ⚠️ **系统默认三夹（Music/Documents/Pictures）全部消失**——覆盖语义正确行为、但用户视角是"丢了 3 个"
- ⚠️ 「本地索引 · 上次索引」显示 `2026/7/1 16:33:35（音乐 34185 / 文档 200 / 图片 4687）` = **全库统计**，与顶部概貌 Downloads-only（音乐 2 / 文档 44 / 图片 15）**数据源口径不一致**

**分支归属**：**排除 A（无真 bug、代码路径工作）**；**C 主 + B 副**——用户"添加后不显示"= 系统默认消失让新目录被淹没，checkbox 灰色小字没引起注意；picker 后无 pending badge 让"改动可视化"也缺席。

**问题 2「本地索引进展中看不到实施哪些索引」真机复现**：点「立即索引」后 12 秒内截 12 张 shot，观察：

- ✅ **有** `⏳ 正在索引：已扫描 N · 已入库 M` 文案（cycle 6 v4 落地）
- ❌ **无 `current_root` 显示**——用户看不到"当前扫的是哪个目录/文件"
- ⚠️ **fts_progress 卡在 "0 · 0" 整个索引周期**——60 文件的 Downloads 秒级完成、2s 轮询完全错过中间态；大目录（Music 34185）里音乐 phase 走 Everything 全盘发现**完全无 progress**
- ⚠️ 索引完成后顶部概貌"上次索引"文案未刷新（仍显示旧的 "N 分钟前"）——`prevIndexing` useEffect 只重 fetch `indexOverview`，未强刷 `indexStatus`

### 1.5 修法确认（cycle 7-a 已锁定必做项，Codex §10 评审后修订）

基于 §1.4 诊断结论 + Codex §10 评审（APPROVED with suggestions）：

1. **C 修法 · 高优先级**（Codex APPROVED · SUGGEST 9 合入）：
   - 系统默认消失时**大字醒目提示条**：`ℹ️ 已隐藏系统默认（Music/Documents/Pictures）。勾选👆并列扫。` 直接放在 checkbox 上方大字体
   - 概貌"目录"格改「生效目录数」+ tooltip「设置里生效的目录数（含系统默认追加）」
   - checkbox 加更强 UI：绿色描边框 + 加粗、突出可 opt-in
   - **【新增 · Codex SUGGEST 9】** picker 成功后新行**立即滚入视野 + flash/highlight 1-2s**（浅蓝背景过渡）——比 pending badge 更直觉，视线自然被拉到列表

2. **B 修法 · 中优先级**：
   - Picker 完成后新增行加 `⏳ 待应用` 琥珀 badge（保留 · flash 是补充不是替代）
   - 底部 sticky 提示条 `⚠ 你有未保存的改动` + 关闭前二次确认

3. **数据源统一 · 高优先级**（Codex APPROVED 2 · 选 (a) 单一信源）：
   - 「本地索引 · 上次索引」文案改成引用 `indexOverview` 数据、跟顶部概貌一致
   - **不做**「全库 vs 生效目录」toggle（Codex：暴露内部差异会制造第二套心智模型）；若日后需要全库总量，留给隐私 / 数据管理页
   - reindex 完成后顶部概貌 "上次索引" 也强刷（新增 `setIndexStatus` 重 fetch 到 `prevIndexing` useEffect）

4. **进度可视化 · 高优先级**（Codex OBJECT 3 · SUGGEST 4/5 合入）：
   - **【修订 · Codex SUGGEST 5】** 新加 `IndexStatus.current_phase` 字段（`music_discovery` / `music_scan` / `doc` / `image`）+ `current_config_root`（配置 root，与文件父目录 `current_root` 区分）；前端文案改 「当前目录」（避免"根目录"误导）
   - **【修订 · Codex SUGGEST 4】** 音乐 Everything 全盘发现阶段 = **不追原生进度**（发现器接口成本高），只加 chip：`🎵 扫描音乐（Everything 全盘发现，请稍候）`；发现失败回退 walkdir 时用现有 progress
   - **【修订 · Codex OBJECT 3】** **不做** walkdir 预扫 count（大目录多一次磁盘 IO、正好打在用户最怕的路径）；进度条走 **indeterminate + `已扫描 N · 已入库 M`**；未来若要百分比，用上一轮 root 统计做弱估计但不阻塞本轮

（原 §3.1 三分支诊断保留在下方作为决策历史；§3.2 UX 打磨清单按上述 §1.5 修正细化；§10 Codex 完整评审保留在文末不动。）

---

## 2. cycle 7 目标与三刀拆分

### 2.1 总目标

- **让"添加了目录 → 看到目录 → 索引中 → 索引完成"的每一步都有清晰可见反馈**
- **支持在索引根目录下用相对路径 glob 排除子目录 / 子路径**
- 顺带清 cycle 5/6 遗留的两个老 follow-up（单目录重扫 + 打开目录）

### 2.2 三刀拆分

| Cycle | 主题 | 估时 | 前后端 | 风险 |
|---|---|---|---|---|
| **7-a** | 目录管理 UX 打磨 + 首次真机验证诊断 | ~2h | 纯前端 + 可能 1 小后端字段 | 低（若 §3.1 是 UX 层问题） |
| **7-b** | 子路径排除（相对 root path glob） | ~0.5d | AppSettings + scan.rs 双改 | 中（scan.rs 关键路径） |
| **7-c** | 单目录重扫 + 打开目录 + 移除时可选 purge | ~0.5d | 3 个 tauri command | 低 |

**三刀合一出货 v0.9.7**（用户 2026-07-01 决策）——一次装机验证全部覆盖，最集中改动，避免多次装机-测试-反馈往返。

---

## 3. cycle 7-a — UX 打磨（~2h、纯前端为主）

### 3.1 诊断完成，跳过（详 §1.4 结论）

原计划的"三分支 A/B/C 首次真机验证诊断"已由 Claude Code 2026-07-01 完成，结论 = **C 主 + B 副 + §1.4 次生问题**（数据源不一致 + 进度可视化断层）。以下 §3.2 UX 打磨清单按 §1.5 修法确认细化，不再依赖诊断结果分支。

### 3.2 UX 打磨清单（按 §1.5 修法确认）

**(1) Picker 后立即反馈**

新增 pending 状态：
```tsx
// 在 IndexingPane 里
const pendingRoots = settings.index_roots.filter(
  (p) => !originalIndexRoots.includes(p)  // originalIndexRoots = 初始 load 的快照
);

<RootRow
  path={path}
  isSystemDefault={!isCustom}
  isPending={pendingRoots.includes(path)}  // ← 新
  overview={overviewOf(path)}
  onRemove={...}
/>
```

`RootRow` 里 pending 时右侧加 `⏳ 待应用` chip（琥珀色 `#ff9500`）。

**(2) 未保存改动 sticky 提示**

对话框底部消息条改造：有未保存改动时（`hasUnsavedChanges = JSON.stringify(settings) !== JSON.stringify(initialSettings)`）显示：
```
⚠ 你有未保存的改动，点「应用」或「确定」生效
```

关闭前若 `hasUnsavedChanges` 弹二次确认「有未保存的改动，确认放弃？」。

**(3) 「本地索引」卡片当前索引可视化增强**（Codex §10 修订）

正在索引时：
- 卡片上部 **indeterminate 动画进度条**（不做百分比、Codex OBJECT 3）+ 纯文本 `已扫描 N · 已入库 M`
- 当前 phase chip：`🎵 扫描音乐（Everything 全盘发现，请稍候）` / `📄 扫描文档` / `🖼 扫描图片`——用 `current_phase` 字段驱动
- 当前目录高亮：在下方目录列表里，正在扫的 root 行整行加浅蓝背景 + 侧边条——用 `current_config_root` 匹配（Codex SUGGEST 5，避免用 `current_root` 文件父目录导致的漂移）
- 文本行区分 「当前目录：<current_root 父目录>」 vs 「正在扫描 root：<current_config_root>」

**(4) 顶部「索引概貌」标签澄清**

改「目录」→「已索引目录」，加 tooltip「设置里生效的目录数（含系统默认追加）」。新增第 7 单元格「待应用」（仅在有 pending 时显示）。

**(5) picker 成功后 flash/highlight 新行**（Codex SUGGEST 9 新增）

picker 关闭后：
- `RootRow` 新增 `flashUntil: number | null` prop；`onPick` 时给新加 root 一个 `Date.now() + 1500` 值
- CSS `@keyframes flash-in` 1.5s 从 `background: #e5f0ff` 淡到透明
- 同时 `scrollIntoView({ behavior: "smooth", block: "nearest" })`——把新行滚入视野

### 3.3 后端字段扩展（Codex §10 修订确定必做）

`IndexStatus` 加两个字段（[index_status.rs](../../apps/desktop/src-tauri/src/search/index_status.rs)）：

```rust
pub struct IndexStatus {
    // ... 现有字段
    /// BETA-33 cycle 7-a：当前索引阶段（Codex SUGGEST 5）。
    pub current_phase: Option<IndexPhase>,
    /// 当前正在扫的**配置 root**（与文件父目录 `current_root` 区分）。
    pub current_config_root: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexPhase {
    MusicDiscovery,   // Everything 全盘发现（无 per-file progress）
    MusicScan,        // Everything 失败 fallback walkdir
    Doc,              // 文档 phase
    Image,            // 图片 phase
}
```

`StatusProgressBridge` 在每 phase 开始时 `set_phase(...)`，`reindex_scoped_with_progress` 内部按顺序调用（音乐 discovery → 音乐 scan？→ doc → image）。前端 phase chip 直接 `switch` 展示。

### 3.4 验收

- (a) 打开选项 → 索引 pane，添加 Downloads → 立即看到新行 + `⏳ 待应用` badge + 底部 sticky「未保存」提示
- (b) 点「应用」→ badge 与 sticky 提示消失、settings.json 里 index_roots 含 Downloads
- (c) 触发 reindex → 卡片进度条动 + phase chip 切换 + 当前目录高亮
- (d) 索引完成后 → 概貌重刷、目录行 count 更新
- (e) 关对话框前有未保存改动 → 二次确认；不确认则不关

---

## 4. cycle 7-b — 子路径排除（相对 root path glob，~0.5d，Codex §10 修订）

### 4.1 数据模型（Codex OBJECT 1 · SUGGEST 1/2 合入）

**AppSettings 新增字段**：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    // ... 现有字段
    /// BETA-33 cycle 7-b：per-root 子路径排除。
    /// 每项 `{root: 索引根字符串（原样保留用户输入）, patterns: 相对 root 的 path glob 列表}`。
    /// 空列表 = 该 root 无 per-root 排除（仍走全局 exclude_globs basename 排除）。
    ///
    /// Codex OBJECT 1：**不用** `HashMap<String, Vec<String>>` —— JSON object key 用路径字符串
    /// 会让 Windows 盘符/反斜杠/大小写/尾部分隔符更脆，且未来加 enabled/comment/created_at 难扩展。
    pub root_excludes: Vec<RootExclude>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RootExclude {
    /// 与 index_roots 里的字符串对应（保留 display 形式，不做归一化）；
    /// 后端过滤前调用 `normalize_root_key` 归一化再比较，允许
    /// `C:\Users\Alice\Documents` 与 `C:/Users/Alice/Documents` 视为同一 root。
    pub root: String,
    pub patterns: Vec<String>,    // e.g. ["临时/**", "**/backup/**", "*.old/*"]
}

/// Codex SUGGEST 2：新加 helper，用于 root_excludes 查找 / 去重 / 删除孤儿条目 / 单测。
/// 语义：canonical-ish 归一化：
///   1) `std::fs::canonicalize` 成功 → 用 canonical 结果
///   2) 失败（路径不存在等）→ 保留原 PathBuf
///   3) trim 尾部 `/` `\`
///   4) Windows 上统一分隔符为 `\`（或全部转 `/`——见 §4.2）
///   5) 大小写：Windows 走 to_lowercase，Unix 保留
pub(crate) fn normalize_root_key(path: &str) -> String { ... }
```

**语义**（用户 2026-07-01 拍板 + Codex SUGGEST 3 边界补充）：
- **相对 root 的 path glob**（不是 basename glob）
- 用 `globset::Glob` 编译，但匹配对象是 `entry.path().strip_prefix(root).unwrap()`
- `**` = 任意深度、`*` = 单段、`?` = 单字符（同 gitignore 语义）
- **【Codex SUGGEST 3 · 关键 glob 边界】**：
  - `临时/**` 应同时匹配目录 `临时` 本身 + 其子树；否则 `walkdir::filter_entry` 遇到 dir entry `临时` 时未命中，剪枝失效
  - 实现：编译规则时对以 `/**` 结尾的 pattern **自动追加去掉 `/**` 的目录 pattern**（`临时/**` → 同时 build 一条 `临时`）
  - `**/backup` UI 上提示用户写 `**/backup/**`；实现层同样补目录本身兜底
- 例：root=`C:\Users\Alice\Documents`，pattern `临时/**` 匹配 `Documents\临时`、`Documents\临时\a.docx`、`Documents\临时\子\b.pdf`

### 4.2 scan.rs 改动（Codex OBJECT 2 · SUGGEST 1 修订）

当前 `is_excluded_dir` 只吃 basename `GlobSet`（[scan.rs:64](../../packages/indexer/src/scan.rs#L64)）：

```rust
fn is_excluded_dir(entry: &walkdir::DirEntry, exclude: &GlobSet) -> bool {
    entry.file_type().is_dir() && exclude.is_match(entry.file_name())
}
```

**Codex OBJECT 2 · 保留兼容层**：不换旧函数签名，新增并行 API，旧函数委托新函数：

```rust
/// 两层排除过滤：basename glob（全局，与 root 无关）+ path glob（限定 root）。
pub struct ExcludeFilter {
    pub basename: GlobSet,                                 // 全局 exclude_globs 编译
    pub per_root: Vec<(String, GlobSet)>,                  // key = normalize_root_key(root)、value = 相对路径 GlobSet
}

impl ExcludeFilter {
    /// 兼容构造：仅从 basename `GlobSet` 建 filter（per_root 为空）。
    /// 旧 API `index_dirs_excluding(..., &GlobSet)` 内部走 `ExcludeFilter::from_basename_set(gs)` 委托新 API。
    pub fn from_basename_set(gs: &GlobSet) -> Self {
        Self { basename: gs.clone(), per_root: Vec::new() }
    }

    /// 新构造：从 settings 直接建，含 normalize_root_key + glob 边界补充（临时/** → 补 临时 目录 pattern）。
    pub fn build(exclude_globs: &[String], root_excludes: &[RootExclude]) -> Self { ... }

    pub fn is_excluded_dir(&self, entry: &walkdir::DirEntry) -> bool {
        if !entry.file_type().is_dir() { return false; }
        // Layer 1：basename 全局
        if self.basename.is_match(entry.file_name()) { return true; }
        // Layer 2：per-root path glob
        for (root_key, gs) in &self.per_root {
            // 归一化 entry 路径 + 提取相对 root 的部分
            let entry_key = normalize_root_key(&entry.path().to_string_lossy());
            if let Some(rel) = entry_key.strip_prefix(root_key) {
                let rel = rel.trim_start_matches(['/', '\\']);
                if !rel.is_empty() {
                    // Codex SUGGEST 1：Windows 上匹配前统一 rel 分隔符为 `/`（globset 更稳定）
                    let rel_norm = rel.replace('\\', "/");
                    if gs.is_match(&rel_norm) { return true; }
                }
            }
        }
        false
    }
}
```

**Codex OBJECT 2 兼容层实现**（保 BETA-27 basename-only byte-for-byte）：

```rust
// scan.rs 旧 API 保留、内部委托新 API：
pub fn index_dirs_excluding(&self, roots: &[PathBuf], exclude: &GlobSet) -> Result<...> {
    self.index_dirs_with_filter(roots, &ExcludeFilter::from_basename_set(exclude))
}
// 新 API（前后向兼容）：
pub fn index_dirs_with_filter(&self, roots: &[PathBuf], filter: &ExcludeFilter) -> Result<...>
```

`index_dirs_excluding_with_progress` / `index_image_dirs_excluding` / `run_incremental_index` 都同款处理。

**walkdir 递归剪枝**：目录命中 = 剪整棵子树（现有 `.filter_entry` 行为不变，只是判定换 `ExcludeFilter::is_excluded_dir`）。

**性能**：per_root vec 短（真机预计 <10 root），每 entry 常数级比较；`normalize_root_key` 需 alloc、但只在目录 entry 上跑（walkdir 大头是文件、不进 `is_excluded_dir`）。

### 4.3 tauri 侧 wiring

`perform_reindex` 里当前 `exclude` 走 `resolve_exclude_globs(&settings.exclude_globs)` 编译成单一 GlobSet。改造为构建 `ExcludeFilter`：

```rust
let filter = ExcludeFilter {
    basename: build_exclude_set(&resolve_exclude_globs(&settings.exclude_globs)),
    per_root: settings.root_excludes.iter().map(|re| {
        (PathBuf::from(&re.root), build_exclude_set(&re.patterns))
    }).collect(),
};
```

`local-index::reindex_scoped_with_progress` API 签名改为接受 `ExcludeFilter` 而非 `&GlobSet`；一路传到 `scan.rs` 内部。

**向后兼容**：`AppSettings::default()` 里 `root_excludes: Vec::new()`；旧 settings.json 无字段 → serde default 走空 vec；行为退化为纯 basename 排除，零回归。

### 4.4 前端 UX

**每 `RootRow` 右侧加 `▸ 子路径排除 (N)` 折叠区**：

```tsx
<RootRow>
  {/* ... 现有元素 */}
  <button onClick={() => setExpanded(!expanded)}>
    ▸ 子路径排除 ({myPatterns.length})
  </button>
</RootRow>

{expanded && (
  <div className="prefs-root-excludes">
    {myPatterns.map((p, i) => (
      <div className="prefs-exclude-row">
        <code>{p}</code>
        <button onClick={() => removePatternAt(i)}>移除</button>
      </div>
    ))}
    <div>
      <input placeholder="如 临时/** 或 **/backup" />
      <button>添加</button>
    </div>
    <p className="prefs-hint">
      相对该目录的通配符：<code>**</code> = 任意层，<code>*</code> = 单段，<code>?</code> = 单字符
    </p>
  </div>
)}
```

**移除 root 时**：同时删掉对应的 root_excludes 条目（不留孤儿）。

### 4.5 单测（新增，Codex SUGGEST 10 合入）

`scan.rs`：
- `exclude_filter_basename_only_matches_all_roots`
- `exclude_filter_per_root_matches_relative_path`（`临时/**` 命中 `root/临时/a`）
- `exclude_filter_per_root_matches_dir_itself`（**Codex SUGGEST 3**：`临时/**` 命中 dir entry `临时` 本身，验证剪枝生效）
- `exclude_filter_per_root_does_not_leak_to_other_roots`
- `exclude_filter_ignores_pattern_when_entry_outside_root`
- `exclude_filter_walkdir_prunes_matching_subtree`（真跑 walkdir 验证剪枝）
- **【Codex SUGGEST 10 新增】** `exclude_filter_windows_separator_normalization`：pattern 用 `/`（`临时/**`）、真实 path 用 `\`（`C:\...\临时\a.docx`）应命中
- **【Codex SUGGEST 10 新增】** `exclude_filter_root_trailing_separator_equivalence`：root 带尾部 `\` 与不带尾部 `\` 归到同一 normalized key、行为等价

`settings.rs`：
- `old_settings_without_root_excludes_parses_ok`（向前兼容）
- `root_excludes_default_is_empty`
- `normalize_root_key_windows_paths_equivalence`（Codex SUGGEST 2：`C:\Users\Alice\Documents` == `C:/Users/Alice/Documents` == `c:\users\alice\documents\`）
- `normalize_root_key_unix_paths_preserve_case`（Unix 保留大小写）

### 4.6 验收

- (a) 加 root `C:\Users\Alice\Documents`，配 pattern `临时/**` → reindex 后 `临时\a.docx` 不入库
- (b) 顶部全局 `exclude_globs` 里的 `node_modules` 仍生效（basename 层未破）
- (c) 两个 root 各有 `backup/**` pattern → 互不影响
- (d) 空 `root_excludes` 行为与 v0.9.6 完全一致（零回归）
- (e) 删除 root → 对应 `root_excludes` 条目自动清

---

## 5. cycle 7-c — 老 follow-up 补齐（~0.5d，Codex §10 修订）

> **实施记录（2026-07-02、bump v0.9.8）**：本节三件套已全部落地，与下文方案的偏差有二：
> ① **§5.2 打开目录未用 tauri-plugin-shell**，改为复用既有 `open_path` tauri command（FileActionTool
> 策略引擎 + audit 口径一致、Windows `cmd /C start` 对目录即开 Explorer、零新依赖）；
> ② **§5.3 的二次确认对话框未用 window.confirm 而是新建 in-DOM `ConfirmModal` 组件**——
> v0.9.7 真机验证发现 wry/WebView2 生产装机版 `window.confirm` 不弹窗直接放行（cycle 7-a
> 的关闭前守卫因此完全失效），该组件同时替换了关闭守卫，两处共用（详记忆
> tauri-webview2-window-confirm-noop）。§5.4 验收 (d) 外键级联单测已加
> （`purge_under_root_cascades_document_vectors`）。

### 5.1 单目录重扫（Codex SUGGEST 6 修订）

**不能**直接 `perform_reindex(&app, vec![PathBuf::from(root)])`——会绕过 exclude / OCR / progress bridge 等配置。

**Codex SUGGEST 6**：抽 `perform_reindex_for_roots(status, db_path, settings_path, roots_override: Option<Vec<PathBuf>>)` 内部实现，`roots_override` = `Some(vec![单个 root])` 时替换 roots、过滤器 / OCR / progress 仍从 settings 读：

```rust
async fn perform_reindex_for_roots(
    status: &IndexStatus,
    db_path: &Path,
    settings_path: &Path,
    roots_override: Option<Vec<PathBuf>>,
) -> Result<ReindexSummary, String> {
    let settings = read_settings(settings_path)?;
    let roots = roots_override.unwrap_or_else(|| resolve_effective_roots(&settings));
    let filter = ExcludeFilter::build(&settings.exclude_globs, &settings.root_excludes);
    // 现有 fts_begin / bridge / phases 走同一份
    ...
}

#[tauri::command]
pub async fn reindex_root(app: AppHandle, root: String) -> Result<ReindexSummary, String> {
    perform_reindex_for_roots(..., Some(vec![PathBuf::from(root)])).await
}

// 现有全量 reindex command 也走这个内部函数：
#[tauri::command]
pub async fn reindex(app: AppHandle) -> Result<ReindexSummary, String> {
    perform_reindex_for_roots(..., None).await
}
```

前端 `RootRow` 加「立即重扫」按钮（磁盘 icon）。

### 5.2 打开目录

用 `tauri-plugin-shell` 的 `open` API（[tauri-shell open](https://tauri.app/plugin/shell/#open)），Windows explorer / macOS Finder 打开：

```tsx
import { open } from '@tauri-apps/plugin-shell';
<button onClick={() => open(path)}>打开</button>
```

需在 `apps/desktop/src-tauri/Cargo.toml` 加 `tauri-plugin-shell = "2"`，并在 `main.rs` `.plugin(tauri_plugin_shell::init())`。

### 5.3 移除目录时可选 purge（Codex SUGGEST 7/8 修订）

**移除 root 时弹二次确认对话框**（Codex SUGGEST 8 · 文案强调"不删磁盘文件"）：

```
移除「C:\Users\Alice\Downloads」

选项：
○ 仅从索引配置移除（保留数据库缓存，重新添加可复用旧记录）
○ 移除并清除索引记录（清除的是 LociFind 数据库缓存，不会删除原文件）

[取消]  [确定]
```

**Codex SUGGEST 7 · SQL 抽到 indexer 存储层**，不在 tauri command 里手写 DELETE：

```rust
// packages/indexer/src/doc_db.rs
impl DocumentIndex {
    /// 清除 root 下所有条目（含 document_vectors 外键级联）。
    /// 与 `stats_under_root` 共用前缀边界 helper `root_glob_predicate`。
    pub fn purge_under_root(&self, root: &str) -> Result<u64, IndexError> {
        // PRAGMA foreign_keys=ON 已在 open() 里设置，确保 document_vectors CASCADE
        let n = self.conn.execute(
            &format!("DELETE FROM documents WHERE {}", root_glob_predicate("path")),
            params![root, format!("{}/*", root), format!("{}\\*", root)],
        )?;
        Ok(n as u64)
    }
}

// packages/indexer/src/db.rs
impl MusicIndex {
    pub fn purge_under_root(&self, root: &str) -> Result<u64, IndexError> { ... }
}

/// 共享 helper，与 stats_under_root 一致的三 GLOB OR 边界。
pub(crate) fn root_glob_predicate(col: &str) -> String {
    format!("{col} = ?1 OR {col} GLOB ?2 OR {col} GLOB ?3")
}
```

tauri command 只做薄封装：

```rust
#[tauri::command]
pub fn purge_root_from_db(app: AppHandle, root: String) -> Result<PurgeSummary, String> {
    let db_path = local_index_db_path();
    let doc_deleted = DocumentIndex::open(&db_path)?.purge_under_root(&root)?;
    let music_deleted = MusicIndex::open(&db_path)?.purge_under_root(&root)?;
    Ok(PurgeSummary { doc_deleted, music_deleted })
}
```

### 5.4 验收（Codex SUGGEST 10 后半合入）

- (a) 单目录点「立即重扫」→ 只该目录进 reindex；status 显示对应 root、其他 root count 不变；exclude_globs / root_excludes 仍生效
- (b) 点「打开」→ Windows Explorer 打开该目录
- (c) 移除 + 选「移除并清除」→ DB 该 root 下 documents/music count 归零；不选则条目保留
- (d) **【Codex SUGGEST 10】** purge 后 `document_vectors` 表**外键级联**：删除 documents.id = X 后 `document_vectors.doc_id = X` 应被自动删除；单测断言 `SELECT COUNT(*) FROM document_vectors WHERE doc_id = ?` 归零（若 PRAGMA foreign_keys 未开会暴露此 bug）
- (e) 文案二次确认对话框明确说明"不删除磁盘文件"

---

## 6. 跨 cycle 的 CSS / 组件复用

- `.prefs-root-row` 从 flex 单行升级到 grid 3-column layout（path / stats+time / actions），列宽固定对齐
- 新 chip 组件 `.prefs-chip.pending / .prefs-chip.phase / .prefs-chip.warn` 复用
- 展开区 `.prefs-root-excludes` 参考现有 `.prefs-form` 缩进 padding-left

---

## 7. 验收标准（v0.9.7 出场）

### 7.1 本机自动化

```
✅ cargo test -p locifind-indexer --lib  (含 4 新 scan 单测)
✅ cargo test -p locifind-desktop settings::  (含 root_excludes 兼容单测)
✅ cargo test -p locifind-desktop search::index_status::  (含 phase 桥单测)
✅ cargo clippy --workspace -- -D warnings
✅ cargo fmt --all
✅ npx tsc --noEmit  (apps/desktop)
```

### 7.2 真机（computer-use 驱动 dev 窗口）

- **§3.4 (a)-(e)** cycle 7-a UX 验收
- **§4.6 (a)-(e)** cycle 7-b 子路径排除验收
- **§5.4 (a)-(c)** cycle 7-c 老 follow-up 验收
- **零回归**：现有搜索、索引、预览、历史、保存的搜索、include_system_defaults checkbox、模型下载、语义召回、隐私 pane 全部功能不动

### 7.3 CI Release

- push `v0.9.7` tag 触发 `.github/workflows/release-windows.yml`
- Release notes 中文 changelog 覆盖三刀改动

---

## 8. 决策记录（用户 2026-07-01 已拍板）

| # | 决策 | 值 | 理由 |
|---|---|---|---|
| D1 | cycle 7 出货节奏 | 三刀合一 v0.9.7 | 一次装机验证覆盖，避免多次真机-反馈往返 |
| D2 | 子路径排除语义 | 相对 root 的 path glob | 表达力强、心智模型清晰；`临时/**` 直觉可懂 |
| D3 | 交接方式 | 写 docs/reviews/beta-33-cycle-7.md 交 Codex 评审 | 与本仓 review 模式一致；Codex 可提反对意见或直接接手 |
| D4 | v0.9.6 用户报告归属 | 【已完成诊断 2026-07-01】= C 主 + B 副 + 数据源不一致 + 进度可视化断层 | 排除真 bug；核心是"系统默认消失"心智冲突（详 §1.4） |
| D5 | Codex §10 评审接受 | **APPROVED with suggestions**：3 OBJECT 全采纳 + 10 SUGGEST 全合入 §1.5/§3/§4/§5 | 2026-07-01 Codex 桌面版评审、1m26s、+44 -0；实施顺序按 Codex 建议 7-a → ExcludeFilter 兼容层 → 7-c |

---

## 9. Codex 评审 ask（本节留给 Codex 回复）

请 Codex 就以下几点给意见（可直接在本文件下面追加 §10 回复段）：

### 9.1 §1.5 修法确认（§3.1 诊断已完成、Codex 只需评审修法）

- §1.5 四类修法（C 主 / B 副 / 数据源统一 / 进度可视化）优先级排序合理吗？是否有遗漏？
- 「概貌统计口径与本地索引口径统一」建议怎么改？两个候选：(a) 本地索引区文案改成引用 indexOverview（Downloads-only 数字）; (b) 概貌上增加"全库 vs 生效目录"toggle。你倾向哪种？
- 音乐 Everything 全盘阶段的 progress 只能加 chip 提示"扫描中无进度"、还是有办法拿到 Everything 阶段的原生进度？

### 9.2 §4.1 数据模型

- `RootExclude { root, patterns }` vs 内联嵌套到 `AppSettings { root_excludes: HashMap<String, Vec<String>> }`——哪个 serde 兼容性更好？
- 跨平台路径分隔符归一：`root` 存 canonical 形式还是原样？（Windows `C:\Users` vs `C:/Users` 需要一致化）

### 9.3 §4.2 scan.rs 改动

- `ExcludeFilter` 传入 `is_excluded_dir` vs 保留 `&GlobSet` 签名 + 另加 per_root helper——哪个更小侵入？
- 是否需要在 `walkdir::WalkDir::new(root)` 时给 root 传 canonical 路径，防 `..` / symlink 让 `strip_prefix` 失败？

### 9.4 §5.3 purge_root 语义

- 用户误勾"一并清除"再撤销的可能性：要不要加 30 天软删除窗口而非硬 DELETE？（评估复杂度收益）
- 与 BETA-12 卸载流程有无重叠？（避免功能碎片）

### 9.5 §3.2 (3) 进度条 estimated_remaining

- walkdir 递归中拿不到"剩余多少"总数，只能事后统计。方案：
  - (i) 第一次扫时先跑一遍 `walkdir count`（多一遍磁盘 IO，大目录慢）
  - (ii) 用文档数/音乐数上一次的总量做估（首次索引没数、退到 indeterminate）
  - (iii) 干脆不做百分比，只显示 `已扫描 N` + spinner
- 建议？

### 9.6 兜底

- 有没有更简的方案能达成 §1.1 用户诉求？（例如：把「+ 添加目录」按钮做得更醒目、picker 后直接闪一下新行 → 也许比 pending badge 更直觉）

---

## 10. Codex 回复（待填）

（Codex 若接手评审，请在此节追加意见；若无异议，标注 "APPROVED" 并可直接接续实施。）

### Codex 评审意见（2026-07-01）

#### APPROVED

1. **§1.5 四类修法的总体排序合理**：C 主修法 + 数据源统一 + 进度可视化应并列高优先级，B 副修法中优先级合理。用户报告的核心不是 picker 失败，而是"覆盖语义导致系统默认消失"与"未保存态不可见"叠加；先把心智模型讲清，比继续追 picker bug 更有收益。

2. **数据源统一建议选 9.1 候选 (a)**：把「本地索引」区文案改成引用 `indexOverview` 的生效目录口径，不建议现在做"全库 vs 生效目录"toggle。toggle 会把一个诊断用内部差异暴露给用户，容易制造第二套心智模型；当前 UI 的主要目标是"我现在配置的目录里有什么"，因此应以 `indexOverview` 为单一信源。若仍需要全库总量，可留给隐私/数据管理或高级诊断页。

3. **§4 相对 root path glob 方向可行**：相对 root 的 glob 心智模型清晰，适合表达 `临时/**`、`**/backup/**` 这类真实需求；用 `walkdir::filter_entry` 做目录剪枝也符合当前 BETA-27 的性能路径。

4. **§5.3 默认硬删可接受**：在 v0.9.7 范围内，`purge_root_from_db(root)` 作为显式二次确认后的维护操作，用硬 DELETE 更符合 SQLite 本地索引的性质。索引库是可重建缓存，不是用户源文件；做 30 天软删会引入 tombstone/schema/恢复 UI/清理策略，复杂度明显超过本 cycle 收益。

#### OBJECT

1. **反对把 root_excludes 用 `HashMap<String, Vec<String>>` 存 settings**。JSON object 的 key 是路径字符串，Windows 盘符、反斜杠、大小写、尾部分隔符都会让迁移和人工编辑更脆；未来若要给每条规则加 enabled/comment/created_at，也会很难扩展。保留 `Vec<RootExclude>` 更稳。

2. **反对直接把所有 `&GlobSet` API 替换成 `ExcludeFilter` 且不留兼容层**。`scan.rs` 当前 `index_dirs_excluding(..., &GlobSet)` 已被多处测试和 local-index 调用覆盖，建议新增 `index_dirs_with_filter` / `run_incremental_index_with_filter` 或让 `ExcludeFilter::from_basename_set` 包一层，旧函数继续委托新函数。这样 blast radius 小，BETA-27 basename-only 行为也更容易 byte-for-byte 守住。

3. **不建议为百分比进度先跑一遍 walkdir count**。大目录会多一次磁盘 IO，正好打在用户最怕"索引慢/卡"的路径上。进度条应先做 indeterminate + `已扫描 N / 已入库 M`；如果以后要百分比，可以用上一轮 root 统计做弱估计，但不要阻塞本轮。

#### SUGGEST

1. **路径归一建议分两层**：settings.json 里尽量保留用户/系统返回的 display string；运行时构建 `RootExclude` filter 前做 canonical-ish 规范化。具体可用 `PathBuf::from(root)` 后尽量 `std::fs::canonicalize`，失败则 fallback 到原 PathBuf；同时 trim 尾部 `/`/`\`。Windows 上匹配前建议统一 `rel` 的分隔符为 `/` 再喂给 globset，或编译 pattern 时把 `/` 和 `\` 两种分隔符都覆盖；否则用户输入 `临时/**` 可能无法匹配 `临时\a.docx`。

2. **root_excludes 应按“归一化 root key”匹配，而不是按原字符串等值匹配**。前端删除 root 时可以按当前字符串删对应项，但后端过滤必须允许 `C:\Users\Alice\Documents` 与 `C:/Users/Alice/Documents` 归到同一 root。建议新增小 helper：`normalize_root_key(path) -> String`，用于去重、查找 root_excludes、删除孤儿条目和单测。

3. **glob 语义需要补两个边界**：`临时/**` 应匹配目录本身 `临时` 以及其子树，否则 `filter_entry` 看到目录 entry `临时` 时可能还没命中，剪枝失效；可在编译规则时对以 `/**` 结尾的 pattern 同时加入去掉 `/**` 的目录 pattern。另一个是 `**/backup` 是否只剪名为 backup 的目录，还是也剪其内容；建议 UI 提示用户写 `**/backup/**`，实现上可同样补目录本身。

4. **Everything 音乐阶段先加 phase/chip，不追原生进度**。当前代码里发现器成功时走 `discover_audio()` + `index_paths()`，没有 per-file progress；除非 Everything CLI/SDK 已返回可流式枚举，否则为了 v0.9.7 不值得改发现器接口。可以在进入 music discovery 前设置 `current_phase = "music_discovery"`，文案显示「扫描音乐（Everything 全盘发现，请稍候）」；发现失败回退 walkdir 时自然使用现有 progress。

5. **进度状态建议新增 phase，而不是只靠 current_root 推断**。`current_root` 现在实际是当前文件父目录，不一定是配置 root；对"当前扫哪个根目录"的高亮会不准。建议 `IndexStatus` 加 `current_phase` + `current_config_root`（或把 `current_root` 改口径为配置 root，另留 `current_dir` 显示父目录）。如果本 cycle 想小改，至少前端把现有 `current_root` 文案叫「当前目录」而不是「索引根」。

6. **单目录重扫 API 要复用同一套配置解析**。`reindex_root(root)` 不应绕过 `exclude_globs` / `root_excludes` / OCR / progress bridge；建议 `perform_reindex` 抽一个内部 `perform_reindex_for_roots(status, db_path, settings_path, roots_override)`，override 只替换 roots，过滤器仍从 settings 读。

7. **purge SQL 应抽到 indexer 存储层，复用前缀边界逻辑**。不要在 tauri command 里手写两份 DELETE；建议 `DocumentIndex::purge_under_root(root)` 和 `MusicIndex::purge_under_root(root)`，内部和 `stats_under_root` 共用 `path = root OR path GLOB root/* OR path GLOB root\*` 的边界 helper。这样 document_vectors 级联、FTS 删除和 busy_timeout 都留在存储层。

8. **移除 root 的确认文案要明确“不删除磁盘文件”**。建议按钮文案区分「仅移除目录」与「移除并清除索引记录」，说明“清除的是 LociFind 数据库缓存，不会删除原文件；以后重新添加可重建”。这比软删除更能降低误勾成本。

9. **更简 UX 兜底**：picker 成功后除 pending badge 外，建议立即把新行滚入视野并短暂 flash/highlight 1-2 秒；底部消息「已加入待保存列表」保留但不够，因为用户视线在目录列表。这个小动画可能比新增复杂说明更直接。

10. **验收补充**：§4.6 增加 Windows 分隔符单测：pattern 用 `/`，真实 path 用 `\`，应命中；再加 root 带尾部反斜杠与不带尾部反斜杠等价。§5.4 增加 purge 后语义向量不残留的断言（documents 删除后 `document_vectors` 应随外键级联，若连接未开 `PRAGMA foreign_keys=ON` 会暴露问题）。

**结论**：APPROVED with suggestions。可以按三刀合一继续做 v0.9.7；我建议实现时先落 7-a，再落 `ExcludeFilter` 兼容层和归一化单测，最后做 7-c 的 command/UI，避免 UX 改动和扫描关键路径同时失焦。
