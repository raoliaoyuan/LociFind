# BETA-27 设计：可配置本地索引目录 + 排除规则（通配符）

> 类型：**生产功能**。用户提出（参考 Everything 目录定义能力）。
> 起因：索引根目录写死 `dirs::document_dir()` / `audio_dir()` / `picture_dir()`，用户「文档」夹外的资料（桌面/下载/项目夹/D 盘）不进内容/语义索引；`search_scope` 设置是死配置（只展示不驱动）。真机实证：v0.4.0 语义索引仅 17 篇 = Documents 夹内文档数。
> 边界：本切片做「统一可配置索引目录 + basename glob 排除」。**不做**：include 目录通配符、完整路径 glob 排除、per-category 目录、搜索时范围限制（search_scope 真实现）——均 YAGNI / 后续。

## 1. 背景与目标

LociFind 的本地内容/语义索引只扫系统 Music/Documents/Pictures 三夹（`packages/indexer/src/scan.rs` 的 `default_*_roots()` hardcoded），用户无法指定自己的资料目录。`AppSettings.search_scope`（默认 `["~"]`）存在但只在隐私面板展示、不驱动索引（死配置）。

**唯一目标**：让用户在设置页**选择/增删要索引的文件夹**（统一列表，三臂共用）+ **排除目录名通配符**（剪掉 node_modules/.git/cache 等子树）；reindex/语义索引读此配置。无配置时行为与今天逐字节一致。

## 2. 范围护栏（YAGNI）

| 本切片做 | 不做 / 后续 |
|---|---|
| 统一 `index_roots`（具体文件夹列表，三臂共用） | per-category（音乐/文档/图片分开）目录 |
| basename glob `exclude_globs`（目录名匹配，filter_entry 短路） | 完整路径 glob（`**/build`、`D:\x\*\y`）排除 |
| 设置页目录选择器（Tauri dialog）+ 排除列表编辑 | include 目录的通配符（Everything Folders 也是具体路径） |
| 隐私面板显真实索引根 | `search_scope` 真实现为「搜索时范围限制」（保留弃用字段） |
| 默认保持现状覆盖（系统三夹）+ 默认噪声排除表 | 自动监听目录变更（无 watcher，沿用启动+手动触发） |

## 3. 架构与组件

### 3.1 数据模型 + 默认值（`apps/desktop/src-tauri/src/settings.rs`）

`AppSettings` 加两字段（`#[serde(default)]` 结构级已在 → 旧 settings.json 向后兼容）：

```rust
/// BETA-27：索引的具体文件夹列表（统一，三臂共用）。空 = 用系统默认（Music+Documents+Pictures）。
pub index_roots: Vec<String>,
/// BETA-27：排除的目录名 glob（basename 匹配，树中任何同名子目录被剪枝）。空 = 用默认噪声表。
pub exclude_globs: Vec<String>,
```

`Default` impl 给两字段 `Vec::new()`（空→走默认解析）。

解析助手（settings.rs，纯函数便于单测）：

```rust
/// 配置非空用配置，空回退系统 Music+Documents+Pictures 三夹并集（保持现状覆盖）。
pub(crate) fn resolve_index_roots(raw: &[String]) -> Vec<PathBuf> { ... }
/// 配置非空用配置，空回退默认噪声排除表。
pub(crate) fn resolve_exclude_globs(raw: &[String]) -> Vec<String> { ... }
```

默认噪声排除表常量 `DEFAULT_EXCLUDE_GLOBS`（取 BETA-26 build_corpus 验证表）：
`node_modules`、`.git`、`target`、`.cargo`、`.rustup`、`.venv`、`venv`、`__pycache__`、`dist`、`build`、`.next`、`Pods`、`.gradle`、`.Trash`、`vendor`、`.cache`、`DerivedData`、`Library`。

`search_scope`：保留字段（避免动 privacy.rs 测试），标注弃用；隐私面板改显 `index_roots`（§3.4）。

### 3.2 索引层排除 glob（`packages/indexer`）

- 加 `globset` crate（indexer dep）。新增薄封装 `ExcludeMatcher`（或直接用 `GlobSet`）：把 `exclude_globs` 编译成 basename `GlobSet`；非法 glob 跳过 + 记日志，不中断。
- `run_incremental_index(idx, roots, exts, extract_fn)` 加参 `exclude: &GlobSet`：`WalkDir::new(root).follow_links(false).into_iter().filter_entry(|e| !is_excluded_dir(e, exclude))`——`is_excluded_dir` = `e.file_type().is_dir() && e.file_name() basename 命中 exclude`。**整棵匹配子树不进遍历**（省扫描）。
- **空 GlobSet → 永不命中 → 与今天逐字节一致**（无排除时遍历行为不变）。
- `index_dirs` / `index_image_dirs` / 音乐侧 `index_dirs` 各加 `exclude` 透传到 `run_incremental_index`。

### 3.3 接线：reindex 读配置（`apps/desktop`）

- `perform_reindex` 读 settings.json → `resolve_index_roots` + `resolve_exclude_globs` → 编译 GlobSet → 传 `LocalIndexBackend::reindex(roots, exclude)`（统一 roots 喂三臂 + exclude）。
- `LocalIndexBackend::reindex` 签名从 `(music_roots, doc_roots, image_roots)` 改为 `(roots, exclude)`（统一）——内部三臂 `index_dirs(roots, exclude)`。
- `spawn_semantic_index` 用配置 roots（替换 `default_document_roots()`）+ exclude。
- reindex 入口（启动后台任务 + reindex 命令）确保把 settings_path 透传到 perform_reindex（或 perform_reindex 内部读 settings.json，mirror `read_similarity_floor`）。
- 大目录首索引慢 → 复用 BETA-15B-2 后台调度（语义 worker 渐进），不阻塞 UI。

### 3.4 前端 + 插件（`apps/desktop`）

- **Tauri dialog 插件**：`package.json` 加 `@tauri-apps/plugin-dialog`；`Cargo.toml` 加 `tauri-plugin-dialog`；main.rs `.plugin(tauri_plugin_dialog::init())`。
- **设置页**（`SettingsPage.tsx`，`AppSettings` TS 接口加 `index_roots: string[]` + `exclude_globs: string[]`）：
  - **索引目录块**：列表渲染各目录 + 每行「移除」；「+ 添加目录」→ `open({ directory: true })` 弹系统文件夹选择 → 追加。空列表提示「将使用系统默认（音乐/文档/图片）」。
  - **排除规则块**：glob 字符串列表 + 文本框「添加」+ 每行「移除」；首次预填 `DEFAULT_EXCLUDE_GLOBS`（或空+占位说明默认生效）。
  - 保存沿用现有 `update_settings`；改后点「立即索引」生效。
- **隐私面板**（`privacy.rs`）：`PrivacyOverview` 展示来源从 `search_scope` 改为 `resolve_index_roots`（真实索引根）。

## 4. 数据流

- **配置**：设置页选目录/写排除 → `update_settings` 写 settings.json。
- **索引**：点「立即索引」/ 启动 → `perform_reindex` 读 settings → roots + GlobSet → `reindex(roots, exclude)` → 三臂 `index_dirs` walkdir `filter_entry` 短路排除 → FTS；`spawn_semantic_index` 同 roots/exclude 补向量。
- **展示**：隐私面板显真实 `index_roots`。

## 5. 错误处理

- `index_roots` 含不存在/无权限目录 → walkdir 跳过该 root（best-effort，不中断其它 root）。
- 非法 glob 模式 → 编译跳过该条 + 记日志，其余照常。
- 空 `index_roots` → 系统三夹默认；空 `exclude_globs` → 默认表（注：默认表非空，故默认会排 node_modules 等——这是期望行为）。
- 旧 settings.json 无新字段 → `#[serde(default)]` → 空 → 走默认（向后兼容）。
- dialog 取消/失败 → 不追加，不报错。

## 6. 测试

- **indexer**：
  - `ExcludeMatcher`/GlobSet 编译 + 匹配单测（`node_modules` 命中、`*cache*` 命中 `mycache`、`.git` 命中、空集永不命中、非法 glob 跳过不 panic）。
  - `run_incremental_index` 带排除端到端：临时树含 `node_modules/` 子目录放文件 → 索引后该文件不在库、`node_modules` 外文件在库。
  - **空排除集 = 今天行为**（回归守护）。
- **settings**：`resolve_index_roots`（空→系统三夹、非空→配置 PathBuf）、`resolve_exclude_globs`（空→默认表、非空→配置）、旧 json 向后兼容解析。
- **回归（硬门）**：evals v0.5=473/v0.9=726 byte-equal（不碰 parser）；`cargo test --workspace` / clippy `-D warnings` / fmt / tsc 全绿；**无配置时索引行为与今天一致**（默认 roots = 旧 default_*_roots 并集、默认排除表是新增的「更干净」行为——需确认这不破坏现有 backend 测试，若破坏则现有测试用空排除集跑）。
- **手测登记** → manual-test-scenarios BETA-27 节：加目录（桌面/D 盘）→ 立即索引 → 语义索引篇数增加；排除规则加 `node_modules` → 其内文件不被索引；隐私面板显真实根；空配置 → 默认三夹。

## 7. 平台

跨平台（macOS + Windows）。路径分隔符用 `PathBuf`；Windows 大小写不敏感（glob basename 匹配默认大小写敏感，Windows 上可能需 case-insensitive——实现时按平台决定，默认表都是小写/固定名，影响小，登记观察）。

## 8. 验收标准

1. 设置页可增删索引目录（系统文件夹选择框）+ 排除 glob 规则；保存持久化。
2. reindex/语义索引读配置：加目录后该目录内容被索引（语义篇数增加）；排除规则剪掉匹配子树。
3. 无配置 → 系统三夹 + 默认噪声排除；旧 settings.json 向后兼容。
4. 隐私面板显真实索引根。
5. evals byte-equal；全 workspace test / clippy / fmt / tsc 全绿；无配置时索引行为与今天一致。
6. 真机手测登记。

## 9. 未尽 / 后续

- include 目录通配符 / 完整路径 glob 排除 / per-category 目录 / 大小写不敏感匹配调优 / `search_scope` 真实现（搜索时范围限制）——按需后续。
- 大目录索引性能（首次全量）观察；必要时接 15B-2 调度增强 / 并行。
