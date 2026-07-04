# BETA-18 跨范畴多类型查询（file_type 多值）设计

> 状态：done（2026-06-02，Windows 真机验证）
> 作者：Claude Code (Opus 4.8)
> 关联：[ROADMAP §3.3 B3.5](../../../ROADMAP.md) BETA-18 · schema v1.0 §3.1/§3.2

## 1. 背景

`file_search` 解析器对「pdf和doc」同范畴多类型已支持（BETA-18 partial），但跨范畴
多类型（不同 `file_type`，如「图片和视频」「ppt和pdf」）受 **schema 单值 `file_type`**
限制，退回首范畴丢类型。本任务把 `file_type` 升级为多值，端到端支持跨范畴。

## 2. 决策（brainstorming）

1. **schema 表达 = 同名字段接受标量或数组**（用户选定）。`file_type` 内部类型改为
   `Option<Vec<FileType>>`，自定义 serde：JSON 同时接受标量 `"document"` 与数组
   `["image","video"]`，且**单元素序列化回标量**。→ 字段名不变、wire 向后兼容：旧
   fixtures / v1 LoRA 数据集 / evals 472/26/2 一律不变，**无需重训 LoRA**。否决「新增
   复数字段」（两字段表同一概念、需优先级规则）与「直接改数组」（破 fixtures + LoRA 数据集
   sha256 锁定 → 需重训 ~2 周）。
2. **范围 = 全栈 + Windows 真机验证**。schema + parser + 3 后端（spotlight 改类型但真机
   测试留 Mac；windows/everything Windows 真机）+ refine/context/desktop + 单测。

## 3. 实现

### 3.1 schema（`packages/search-backends/common`）
- `file_type_set` serde 模块：`ScalarOrVec` untagged 反序列化 + 单元素回写标量序列化 +
  空数组规整 `None`。
- `FileSearch.file_type` / `MediaSearch.file_type` / `RefineDelta.file_type`：
  `Option<FileType>` → `Option<Vec<FileType>>` + `#[serde(with = "file_type_set")]`。
- JSON schema `docs/schema/search-intent.v1.json`：加 `FileTypeOrSet`（`oneOf: [FileType, array]`），
  三处 `file_type` 引用改之。

### 3.2 parser（`packages/intent-parser`）
- `merge_extensions`：收集**全部**命中 alias 的 file_type（去重保命中序）+ 扩展名并集。
  同范畴 → 单元素（回写标量，byte-equal）；跨范畴 → 多元素。
- `refine.rs`：`delta.file_type = Some(vec![m.file_type])`。

### 3.3 后端（spotlight / windows-search / everything）
- `CommonConstraints.file_type`：`Option<FileType>` → `Option<&[FileType]>`；include 分支
  对多 file_type 取**扩展名并集**展开。
- media 路径：`media_derived_file_types` 返回 owned `Vec`，调用方 `let` 保活后 `as_deref()`。
- **everything bug 修复（真机暴露）**：`extension_filter` 原对每个扩展名 push 独立 `ext:`
  参数——es.exe 多个 `ext:` 是**空格 AND**（无文件同时是两扩展名 → 命中 0）。改为合并为
  单个分号列表 `ext:a;b;c`（OR）。此 bug 此前对任何多扩展名查询（含 file_type=Document
  展开、用户多扩展名）均致空，被 BETA-18 真机测试发现。windows-search（SQL ` OR `）/
  spotlight（glob `||`）本就正确。

### 3.4 harness / desktop
- `context.rs` refine 合并：`delta.file_type.clone()`（Vec 非 Copy）。
- 测试 fixture：`Some(FileType::X)` → `Some(vec![FileType::X])`。

## 4. 验证

- parser 新增 3 单测（ppt和pdf → [Presentation,Document] / 图片和视频 → [Video,Image] /
  单值序列化为标量 + 多值为数组）。
- common 新增 serde round-trip 单测（标量↔Vec / 空数组→None）。
- everything 新增确定性单测（多 file_type → 单个分号 `ext:` term）+ `#[ignore]` 真机集成
  测试 `cross_category_file_type_unions_extensions`（`.ppt` + `.pdf` 同时命中、`.png` 不命中）。
- **回归门**：evals v0.5 parser-only **472/26/2 byte-equal**（单值序列化保标量）；
  fmt/clippy(`-D warnings`)/全 workspace 测试零回归。

## 5. known limitation

- **MediaSearch.media_type 仍单值**：带媒体修饰的跨范畴媒体查询（如「最大的图片和视频」
  路由到 media_search）受 `media_type` 单值限制，仍只取一类——本任务只解 file_search 路径
  （裸「图片和视频」无修饰 → file_search，已覆盖）。
- file_type 顺序按词典命中序（确定性），非 query 出现序。
- spotlight 跨范畴真机测试留 Mac（类型已改、windows/everything 真机验证）。
