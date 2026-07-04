# BETA-11D 用户级持久化同义词库 — 设计文档

> 状态：设计定稿（2026-06-13，Claude Code）。经 brainstorming 六节逐节确认。
> 关联：ROADMAP §3.3 BETA-11D 卡片 + 验收点；依赖 BETA-15（共享 `SynonymExpander` trait，done）。
> 下一步：writing-plans 出实施计划。

## 1. 目标与范围

让用户能**教学**自己的同义词映射（如「友商竞争分析 → AWS / Azure / 产品分析 / 功能洞察」），
本地持久化、即时生效、参与搜索召回扩展。运行态 feedback 学习闭环。

**v1 范围（完整 A+B+C+D）**：

- **A. 持久化层**：用户词典落盘 `user-synonyms.yaml` + 加载，重启不丢，导出 / 导入。
- **B. 双层扩展器**：用户词典叠加在系统词典之上，**冲突时用户组完全替换系统组**，trace 带 `source`。
- **C. 管理 UI**：设置页「我的同义词」——查看 / 编辑 / 删除整组 / 导入导出，改动**即时生效不重启**。
- **D. 零命中自动触发**：搜索零结果时弹「扩展搜索?」→ 用户手输同义词 → 立即重查 → 「是否记住?」二次确认才沉淀。

**不做（YAGNI / 范围外）**：

- LLM 在线生成候选词（属 BETA-11B；本卡候选恒为空、用户手输）。
- 文件对话框导入导出（用 textarea 复制粘贴；文件对话框留后续，避免引入 Tauri dialog 插件依赖）。
- 卸载时清理用户词典文件（属 BETA-12；本卡仅在 ROADMAP BETA-12 卡片补 checklist）。
- 触发阈值可调（固定 0 = 仅零结果触发；只暴露「开 / 关」开关）。
- 隐私面板「一键清除用户词典」（清除走管理 UI；ROADMAP 未要求面板清除）。

## 2. 关键决策（brainstorming 确认）

| 决策 | 选择 | 理由 |
|---|---|---|
| v1 范围 | 完整 A+B+C+D | 用户要完整闭环 |
| 存储格式 | YAML 文件 `app_config_dir/user-synonyms.yaml` | 与系统词典同格式、复用 lint；导出物即文件；与 `search_history.json` 同目录同风格 |
| 冲突语义 | **替换**（用户组取代系统组） | 符合「覆盖」字面；用户心智可预测；实现最简 |
| 双层架构 | 新 `LayeredSynonymExpander` + 共享 `Arc<RwLock<UserIndex>>` | 系统层零改动、隔离最干净、满足即时生效 |

## 3. 数据模型与存储

**文件**：`app_config_dir()/user-synonyms.yaml`（与 `settings.json` / `search_history.json` 同目录）。

**Schema**（相比系统词典**故意放宽一处**）：

```yaml
version: 1
groups:
  - head: 友商竞争分析
    aliases: [AWS, Azure, 产品分析, 功能洞察]
```

**与系统词典的唯一差异 = 允许组内跨语言 alias**。系统词典禁止跨语言 alias；但 BETA-11D 目标 case
本身就是中文 head + 中英混合 aliases，扩展时 alias 只是 OR 检索词、语言不影响功能，故必须放宽。

**保留并复用的 lint 规则**（污染防护，全部沿用系统层原语，不重复造轮子）：

- `version == 1`；
- 每组 `aliases.len() ≤ 8`（同 `MAX_ALIASES_PER_GROUP`）；
- 组内去重；跨组去重（同一词不得在两组中重复出现）；
- head 非空；
- **拒绝「标识符样」head / alias**：含连字符 / 下划线 / 点的 code-like token（如 `synthetic-place`），对齐 BETA-11 §4.2 NoopExpand 规则；
- 同一 head 多次教学 → 合并去重，不无限增长。

**内存模型**：

```text
UserIndex {
    groups:   Vec<UserGroup>,            // 有序，序列化与 UI 列表的单一信源
    zh_index: HashMap<String, Arc<[String]>>,  // 派生查找表
    en_index: HashMap<String, Arc<[String]>>,  // 派生查找表
}
UserGroup { head: String, aliases: Vec<String> }
```

- `groups` 为权威态；`zh_index` / `en_index` 每次变更后从 `groups` 重建。
- 索引语义与系统层一致：**双向**——组内任一成员（head 或 alias）被搜到都扩展为整组；
  成员按各自语言（`classify`）入对应索引。

**损坏文件兜底**：解析失败 → 退到空词典 + 记日志，不崩溃、不阻塞搜索（仿 `history.rs`）。

**新增解析函数** `parse_user_dict_str`（harness）：复用 `has_cross_language_char` / `MAX_ALIASES_PER_GROUP` /
去重检查等原语；**不动现有 `parse_dict_str`**。

## 4. 双层扩展集成

### 4.1 核心重构（零行为变更，evals byte-equal 守门）

把 `yaml.rs` 中 `expand()` 的算法（gazetteer 扫描 → `merge_or_group` → 多词覆盖 → 裸内容词兜底 → cap）
**参数化到一个「词典视图」抽象**。视图只暴露：

- `lookup(lang, keyword) -> Option<&[members]>`（组全体成员，含 head）；
- 可迭代的索引键集合（gazetteer 扫描用）+ 多词键集合。

随后：

- `YamlSynonymExpander` 实现该视图（基于自身 `zh_index` / `en_index`）——走同一份算法，
  **输出与现状逐字节相同**（由 v0.5 / v0.9 byte-equal 门验证）。
- `LayeredSynonymExpander` 实现该视图为「**用户层覆盖系统层**」组合：
  - `lookup` 先查 user 再查 system → 替换语义自然落地；
  - gazetteer 键集 = user 键 ∪ system 键，冲突键经 `lookup` 解析到 user 组。

整套扩展逻辑**只有一份**；双层只是换数据视图，不复制算法，不动 `NoopExpander`。

### 4.2 运行时可变

```text
LayeredSynonymExpander {
    system: YamlSynonymExpander,        // 现有只读层，不动
    user:   Arc<RwLock<UserIndex>>,     // 可变用户层，与管理层共享同一份
}
```

- `expand(&self, ...)` 取**读锁**完成一次扩展（持锁时长 = 一次查询）；
- 管理命令取**写锁**更新；
- 锁粒度 = 单次查询 / 单次编辑，无并发问题。

### 4.3 trace source

某 keyword 组来自用户层 → 该组 `SynonymExpandEvent.source = "user"`；系统层维持 `"zh.yaml"` / `"en.yaml"`。
`SynonymExpandEvent.source` 字段已存在，无需改结构。满足「冲突可观测 source」+ 隐私边界
（tracer 默认 noop，仅 `LOCIFIND_TRACE` 开启才装真 hook，用户词条默认不进 trace）。

## 5. 管理层（Tauri 命令）+「我的同义词」UI

### 5.1 共享状态接线

启动时创建一份 `Arc<RwLock<UserIndex>>`：

1. 传给 `LayeredSynonymExpander`（搜索路径，进 `SearchDeps` 的 `Arc<dyn SynonymExpander>`）；
2. 由独立 `tauri::State`（用户词典管理器，持同一 Arc + 文件路径）持有，供管理命令使用。

两者引用**同一把锁** → 管理命令改完，搜索路径立即可见，零重启。

### 5.2 新 desktop 模块 `user_synonyms.rs`

仿 `history.rs`：纯逻辑（校验 / 合并 / 序列化）与文件 IO 分离，便于单测。命令：

| 命令 | 行为 |
|---|---|
| `get_user_synonyms() -> Vec<UserGroup>` | 列出全部组（UI 渲染） |
| `add_user_synonym(head, aliases)` | 校验（复用 lint）→ 同 head 已存在则合并去重 → 写锁更新 + 持久化；lint 失败返回错误串 |
| `update_user_synonym(head, aliases)` | 编辑某组 aliases（head 作 key；改 head = 删 + 加） |
| `delete_user_synonym(head)` | 删整组，持久化，即时生效 |
| `import_user_synonyms(yaml_text) -> Result<summary>` | 校验整份 YAML，**任一组 lint 失败则整份拒绝并报哪组** |
| `export_user_synonyms() -> String` | 返回当前 YAML 文本 |

**持久化纪律**：每个写命令先**完整校验候选** → 写文件 → 成功才更新内存索引。
文件写失败则报错、内存不变（内存 == 最后一次成功落盘状态，单一信源）。

### 5.3 前端「我的同义词」页（设置区新增）

- 组列表：head + aliases chips；每组「编辑 / 删除」；顶部「添加」（head 输入 + aliases 输入）。
- 导入 / 导出走 **textarea 复制粘贴**（不引入 file-dialog 插件；对齐 ROADMAP「半手动跨设备」+
  BETA-22「内联输入避开 WKWebView `window.prompt` 限制」既有做法）。
- lint 错误**内联提示**（如「别名超过 8 个」「不允许标识符 `synthetic-place`」）。删除即时生效。

## 6. 零命中自动触发 UX（D）

**触发条件**：一次真实搜索返回**结果数 ≤ 阈值（默认 0 = 零结果）**，且 query 经 parser 判定为搜索 variant
（非空、非 clarify / file-action）。在**前端 `SearchView` post-search** 判定。设置项「搜索无结果时提示添加同义词」
（默认开，可关）；阈值固定 0，不暴露成可调项。

**流程**（严格按 ROADMAP §252 / §257 顺序）：

1. 零结果 → 内联提示：「没找到结果。为「**<head>**」添加同义词扩展搜索?」
   - head 默认 = parser 抽出的 keyword，无则用剥离动词后的 query（用户可改）。
2. **候选为空**（BETA-11B 未做）→ 用户手敲 aliases（内联输入，复用 BETA-22 范式）。
3. 用户确认 → **立即重查但先不沉淀**：调新命令 `search_with_adhoc_synonyms(query, head, aliases)`——
   直接注入一次性 OR 组构造 `ExpandedSearchIntent` 跑搜索路径，**不写用户词典**（避免未确认映射污染词典），
   返回结果展示。
4. 重查出结果后 → UI 再问「**是否记住此映射?**」二次确认：
   - 「记住」→ 调 `add_user_synonym(head, aliases)` 沉淀（下次零延迟直接命中，闭合目标 case）；
   - 「不记住」→ 不写盘，本次仅临时扩展。

**关键正确性**：临时扩展（步骤 3）与持久化（步骤 4）分离——一次性搜索不碰磁盘词典，
仅二次确认才落盘。教学路径的 `add_user_synonym` 走第 5 节同一套 lint，不绕过污染防护。

## 7. 隐私 / 污染防护 / 卸载

**隐私**（守 PROJECT「本地优先」+ ROADMAP §255）：

- `user-synonyms.yaml` 只存自身 app_config_dir、**不上传、不进任何 telemetry**；默认 trace 关 →
  用户词条不进 trace（仅 `LOCIFIND_TRACE` dev 可见）。
- **接入 BETA-21 隐私面板**（仿 BETA-22 把 search_history 接进去）：数据位置表新增「用户同义词库」一行
  （路径 + 组数，只读展示）。清除走「我的同义词」页（删除即时生效）；面板一键清除留后续（不做）。
- Privacy 页教育性文案 + 隐私政策 doc 新增「用户同义词库」一条。

**污染防护**：全部收敛到第 5.2 节那一条 lint 路径，**add / import / teach 三入口共用、无旁路**：
aliases ≤ 8 / 禁标识符样 head·alias / 组内去重 / 跨组去重 / 同 head 合并去重 / head 非空。

**卸载**：BETA-12（未做）需清 `user-synonyms.yaml`——本卡不写代码，仅在 ROADMAP BETA-12 卡片补一条 checklist。

## 8. 测试策略

- **harness**：
  - `parse_user_dict_str` lint（≤8 / 拒标识符 / **允许跨语言 alias** / 组内·跨组去重 / 损坏文件退空）；
  - `LayeredSynonymExpander`（用户覆盖系统 / 用户层 gazetteer 命中 / 替换语义 /
    **用户层空时输出 == 系统层 byte-equal**）；
  - 重构后 `YamlSynonymExpander` 经视图抽象**既有测试全过**。
- **desktop**：`user_synonyms.rs`（add / 合并 / update / delete / import 拒绝 / export roundtrip /
  持久化 + 重载 / 损坏兜底）；`search_with_adhoc_synonyms` 一次性扩展不落盘；privacy 计数断言。
- **evals 硬门**：v0.5 / v0.9 **parser-only byte-equal 全程不动**（重构守门）；with-fallback 不受影响。
- **前端**：tsc + vite build。
- **手测场景登记** `docs/manual-test-scenarios.md`：目标 case 端到端——搜「友商竞争分析」零结果 →
  教学 → 重查命中 → 记住 → 重启零延迟直接命中。
- **每 task 验证门**：`fmt --check` + `clippy --workspace -D warnings` + `cargo test --workspace` 零回归（含 fmt）。

## 9. 验收（对齐 ROADMAP §BETA-11D）

- 持久化：用户词典落盘、重启不丢、不上传不同步、支持导出 / 导入 YAML。
- 双层叠加：先查用户后查系统；冲突用户**替换**；trace `source` 含 `"user"`。
- 触发 UX：零结果弹提示 → 手输 aliases → 立即重查 → 二次确认才沉淀。
- 撤销 / 编辑 / 查看：设置页「我的同义词」查看 / 编辑 / 删除 / 导入导出，删除即时生效。
- 隐私：用户词典不进 trace（默认）；隐私文案新增一条；不混入 telemetry。
- 反向污染防护：aliases ≤ 8；禁标识符样 head / alias；同 head 合并去重。
- 目标 case：搜「友商竞争分析」零命中 → 教学 `[AWS, Azure, 产品分析, 功能洞察]` + 记住 →
  命中合成 fixture；重启后再搜同 query → 零延迟直接命中。
- 卸载集成：登记 BETA-12 checklist（本卡不实现）。
- evals v0.5 / v0.9 parser-only byte-equal 不回归。
