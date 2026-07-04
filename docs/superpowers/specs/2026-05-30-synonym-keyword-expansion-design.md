# 同义词关键词扩展（手维护词典）—— 设计

> 阶段：B（Beta），BETA-11 提案
> 作者：Claude Code (Opus 4.7)
> 日期：2026-05-30
> 关联：[PROTO-05 SpotlightBackend](../../../packages/search-backends/spotlight/src/lib.rs)、[PROTO-06 IntentParser](../../../packages/intent-parser/src/lib.rs)、[MVP-19+ Slice B](./2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md)、[第 33 阶段 SearchDeps](../../../apps/desktop/src-tauri/src/search.rs)

## 1. 背景与问题

LociFind 当前的 keyword 匹配走 Spotlight 的 `CONTAINS[cd]` 谓词，本质上是**字面子串匹配**。`packages/search-backends/spotlight/src/lib.rs::keyword_predicate` 把单 keyword 翻成：

```
(kMDItemDisplayName CONTAINS[cd] "X"
 || kMDItemTextContent CONTAINS[cd] "X"
 || kMDItemFSName CONTAINS[cd] "X")
```

`CONTAINS[cd]` 不做同义词、不做向量。intent-parser 也只把自然语言转 `SearchIntent` 结构化字段，BETA-08 v1 LoRA adapter 学的是字段 patch，没有学近义词扩展。

**用户原 case**：电脑里有文件 `述职.ppt`，自然语言查询 `找一份工作汇报相关的 ppt`。当前架构必然漏——"工作汇报" 与 "述职" 无任何公共子串，谓词不命中文件名；仅当 ppt 内容文本恰好含 "工作汇报" 才偶发命中 `kMDItemTextContent`。

本设计给 BETA 早期补一条**轻量、可解释、可演示**的同义词扩展能力：词典手维护、harness 层中间件注入，跨后端共享接口。明确作为 BETA 中后期 LoRA / embedding 升级的低风险占位。

## 2. 范围（已与用户对齐）

| 维度 | 决策 |
|---|---|
| 词典来源 | **手维护小词典**（YAML），BETA-11 出场 ~100 组。开源词林 / LoRA 在线扩词留 §10 升级路径 |
| 注入位置 | **harness 层中间件 `SynonymExpander`**，在 IntentRouter 之后、SearchBackend 之前。跨 Spotlight / WindowsSearch / Everything 复用 |
| 扩展方向 | **仅同语言内扩词**（zh keyword 只扩中文同义词；en 只扩英文）。中英互扩留升级路径 |
| 验收门槛 | **手测 scenario 清单 + v0.5 evals 不回归**。不建新评测集；定量召回评测留升级路径 |
| 词典维护 | checked-in `resources/synonyms/{zh,en}.yaml`，PR + CI lint |

### 不做（YAGNI 闸口）

- 中英互扩（synthetic-place 等标识符误扩风险大）
- 多义词消歧
- 词典热重载（dev 改 yaml 需重启）
- 用户态 UI 编辑词典
- WindowsSearchBackend / EverythingBackend 接入 `search_expanded`（trait 默认实现 fallback，等真要做时再覆盖）
- LoRA 在线扩词 / embedding 语义召回
- 同义词召回定量评测集

## 3. 架构与数据流

**现状**：

```
NL query → IntentParser → SearchIntent → IntentRouter → SearchBackend → Results
                          keywords: ["工作汇报"]
```

**新增**（增量虚线段）：

```
NL query → IntentParser → SearchIntent ─────────────────────┐
                          keywords: ["工作汇报"]              │  ← evals fixture
                                                             │     仍按此判定
                          ↓                                  │     (字段精确匹配
                     SynonymExpander  ← synonyms/zh.yaml     │      不破)
                          ↓               synonyms/en.yaml   │
                  ExpandedSearchIntent
                  keyword_groups: [["工作汇报","述职",...]]
                          ↓
                    SearchBackend
                          ↓
                       Results
```

**关键性质**

| 性质 | 说明 |
|---|---|
| **SearchIntent 不变** | parser 输出形态零改动；v0.5 evals 472/26/2 字段精确匹配不破 |
| **新增 `ExpandedSearchIntent`** | `keywords: Vec<String>` → `keyword_groups: Vec<KeywordGroup>`。组内 OR、组间 AND |
| **无扩词时退化为恒等** | 词典查不到 / 关闭 / 加载失败 → 单元素组 → 后端行为与现在 byte-equal |
| **trace 可见** | 新增 `synonym_expand` event 写 JSONL，dev `LOCIFIND_TRACE=...` 可见 |
| **跨后端复用** | trait 加 default method，老后端零修改；BETA-11 仅 SpotlightBackend 覆盖 |

## 4. 组件设计

### 4.1 新类型：`ExpandedSearchIntent`

```rust
// packages/harness/src/synonym/expanded.rs（新模块）
pub struct ExpandedSearchIntent {
    pub base: SearchIntent,
    pub keyword_groups: Vec<KeywordGroup>,  // 对齐 base 中 *Search variant 的 keywords 顺序
}

pub struct KeywordGroup {
    pub head: String,           // parser 原始词（lookup key）
    pub synonyms: Vec<String>,  // 不含 head，与 head OR 拼起
}
```

`synonyms` 为空时 = 未扩词；后端实现需保证此时与 `SearchIntent::search(base)` byte-equal。

### 4.2 trait：`SynonymExpander`

```rust
pub trait SynonymExpander: Send + Sync {
    fn expand(&self, intent: SearchIntent) -> ExpandedSearchIntent;
}

pub struct NoopExpander;  // 测试 / 关闭 / 加载失败 fallback，恒等

pub struct YamlSynonymExpander {
    zh_index: HashMap<String, Arc<[String]>>,
    en_index: HashMap<String, Arc<[String]>>,
}

impl YamlSynonymExpander {
    pub fn from_paths(zh: &Path, en: &Path) -> Result<Self, ExpanderError>;
    pub fn from_str(zh: &str, en: &str) -> Result<Self, ExpanderError>;
}
```

**语言判定**（精确规则，按字符分类）：

| keyword 字符构成 | 判定 | 说明 |
|---|---|---|
| 全 ASCII 字母 / 数字 / 内部空格 | en index | 例 `slides`, `work report` |
| 全 CJK（无 ASCII 字母）| zh index | 例 `工作汇报`，可含全角标点 |
| 含 ASCII 字母 + 含其它（CJK / `-` / `_`）| NoopExpand 不扩 | 视为标识符，例 `synthetic-place`, `synthetic-place-笔记`，防误扩 |
| 纯数字 / 纯符号 | NoopExpand 不扩 | 无意义 |

### 4.3 词典 YAML 格式

`resources/synonyms/zh.yaml`：

```yaml
version: 1
language: zh

groups:
  - head: 工作汇报
    aliases: [述职, 年度总结, 季度汇报, 月度汇报]
    domain: office

  - head: 截图
    aliases: [截屏, 屏幕截图]
    domain: media
```

`resources/synonyms/en.yaml` 同结构。

加载期 lint（构造失败 → `ExpanderError`，CI 阻断）：

- 每组 `aliases.len() <= 8`
- alias 内**禁跨语言字符**（zh 表里出现 ASCII 字母 → error；en 表里出现 CJK → error）
- 组内 head + aliases 集合无重复
- head 不允许同时作 alias 出现在其它组（防传递性混乱）
- `version` 必须等于 1（schema breaking 时显式 bump）

### 4.4 双向扩展

YAML 写一个 head + 一组 alias。加载期展开为**无向同义图**：组内任一成员作为输入都能扩出其它所有成员。

例：词典写 `head: 截图, aliases: [截屏, 屏幕截图]`：

- query 含 `截图` → group = `["截图", "截屏", "屏幕截图"]`
- query 含 `截屏` → group = `["截屏", "截图", "屏幕截图"]`（head 位是命中词，其余按词典顺序，输出稳定）

### 4.5 SearchBackend trait 接口变化

最小破坏。trait 加 default method，老后端零修改：

```rust
pub trait SearchBackend: Send + Sync {
    fn search(&self, intent: &SearchIntent) -> Result<...>;

    /// 默认 fallback 到 search()，丢弃 group 信息。
    /// 支持同义词的后端覆盖此方法。
    fn search_expanded(
        &self,
        expanded: &ExpandedSearchIntent,
    ) -> Result<...> {
        self.search(&expanded.base)
    }
}
```

BETA-11 只 SpotlightBackend 覆盖 `search_expanded`：在 `keyword_predicate` 处把单 keyword 替换成 OR 谓词组：

```
(kMDItemDisplayName CONTAINS[cd] "工作汇报" || kMDItemDisplayName CONTAINS[cd] "述职" || ...
 || kMDItemTextContent CONTAINS[cd] "工作汇报" || kMDItemTextContent CONTAINS[cd] "述职" || ...
 || kMDItemFSName CONTAINS[cd] "工作汇报"     || kMDItemFSName CONTAINS[cd] "述职"     || ...)
```

注入安全继续走现有 `escape_predicate_string`。

WindowsSearchBackend / EverythingBackend / NativeIndexBackend 在 BETA-11 不动，继续走 default fallback（不支持同义词），见 §10。

### 4.6 失控保护

| 风险 | 保护 |
|---|---|
| 词典写歪 | §4.3 lint 规则 + CI 阻断 |
| 词典加载失败 | 启动期 fallback 到 `NoopExpander` + `eprintln` warn，**不阻塞 desktop 启动** |
| 单 query 扩出过多 OR 项 | 单组 `aliases.len() <= 8`（结构期 lint）；运行期 cap：**扩展后 keyword 总数（`sum(group.size)`，含 head）≤ 32**，超额截断尾部 + warn trace。Spotlight 谓词在 3 个字段（DisplayName / TextContent / FSName）OR 拼，最终谓词最大 ~96 项 |
| 误扩到标识符（`synthetic-place`） | §4.2 混合字符 keyword 走 NoopExpand；§4.3 lint 禁跨语言 alias |
| 静默扩词难调试 | trace `synonym_expand` event：`{head, group, source: "zh.yaml"}` 写 JSONL，配 `LOCIFIND_TRACE` 开关 |

### 4.7 加载与接线

`SearchDeps`（第 33 阶段刚收拢）新增字段：

```rust
synonym_expander: Arc<dyn SynonymExpander>,
```

**词典路径解析**（dev 与 .app 打包两态）：

- **dev (`cargo run` / `npm run tauri dev`)**：从 workspace 根 `resources/synonyms/{zh,en}.yaml` 读
- **.app 打包态**：词典文件随 `tauri.conf.json` `bundle.resources` 一起打包到 `Contents/Resources/synonyms/`，运行期通过 `AppHandle::path().resource_dir()` 解析

封装在 `synonym::resource_paths(handle: &AppHandle) -> (PathBuf, PathBuf)`，两态由该函数桥接，调用方不感知。

`main.rs` 启动期（`setup` hook 内拿到 `AppHandle` 后）：

```rust
let (zh_path, en_path) = synonym::resource_paths(&app.handle());
let expander = YamlSynonymExpander::from_paths(&zh_path, &en_path)
    .map(|e| Arc::new(e) as Arc<dyn SynonymExpander>)
    .unwrap_or_else(|err| {
        eprintln!("synonym: 词典加载失败,退到 noop: {err}");
        Arc::new(NoopExpander)
    });
```

`search_impl` 在 IntentRouter 选好 backend、policy 通过后调 `expander.expand(intent)`，结果走 `backend.search_expanded(&expanded)`。

**不做热重载**。dev 改 yaml 需重启 desktop。BETA 中期可加。

## 5. 词典初始覆盖（BETA-11 出场目标）

~100 组，覆盖 5 大日常痛点桶：

| 桶 | 语言 | 示例组（head → aliases） | 目标组数 |
|---|---|---|---|
| office 汇报 | zh | 工作汇报→述职/年度总结/季度汇报/月度汇报；周报→周总结；简历→个人简历；总结→复盘 | ~12 |
| 文件类型 | zh | 幻灯片→演示文稿；表格→电子表格；文档→文稿 | ~8 |
| 文件类型 | en | slides→slideshow/presentation；spreadsheet→excel sheet；document→doc | ~8 |
| media | zh | 截图→截屏/屏幕截图；照片→相片/图片；视频→影片/录像 | ~8 |
| media | en | screenshot→screen capture/screencap；photo→picture/pic；video→movie/clip | ~8 |
| 文档管理 | zh+en | 合同→协议；发票→票据；contract→agreement；invoice→receipt | ~10 |
| 个人/家庭 | zh+en | 笔记→札记；设计稿→设计文件；note→memo；mockup→wireframe | ~8 |

合计目标 **~60 zh + ~40 en = ~100 组**。具体清单维护在 PR 中。

### 5.1 维护流程

- 词典文件 checked-in：`resources/synonyms/{zh,en}.yaml`
- 改动走 PR + CI lint（§4.3）
- 词典 PR 模板要求列出"加这组词解决了哪个真实 demo query"，防止收无意义条目
- schema 字段 `version: 1`，breaking 时 bump 并附 migration note

## 6. 测试

### 6.1 单元 / 集成测试

| crate | 新增测试 | 关键 case |
|---|---|---|
| harness | `YamlSynonymExpander::from_str` | 合法 yaml / lint 触发（>8 alias / 跨语言 alias / 重复 head / version != 1）/ 中文 keyword 扩 / 英文 keyword 扩 / 混合 keyword 不扩 / 总 OR cap 截断 + warn |
| harness | `NoopExpander` 恒等 | 任意 intent in == out（单元素组）|
| harness | `ExpandedSearchIntent` 构造 | `keyword_groups` 与 `base.keywords` 对齐顺序 |
| spotlight | `keyword_predicate_expanded` | 单元素组 byte-equal 原版 / 多元素组 OR 形态 / `escape_predicate_string` 注入安全沿用 |
| spotlight | fixture 端到端 | 合成 `述职.ppt` + 测试 zh.yaml + intent `keywords:["工作汇报"]` → 命中 |
| desktop | `SearchDeps` 接线 | yaml 缺失 → `NoopExpander` fallback / `search_impl` 链路喂 `ExpandedSearchIntent` |
| desktop | trace `synonym_expand` event | `LOCIFIND_TRACE=...` 时 JSONL 写一行；`NoopExpander` 不写 |

### 6.2 回归 guard

- `bash scripts/ci.sh` 全过
- `cargo run -p locifind-evals` parser-only baseline **pass 472 / partial 26 / fail 2 byte-equal**（evals 喂 parser-only，不走 expander，必须不变）
- `cargo run -p locifind-evals --with-fallback --hybrid` **pass 480 不掉**（hybrid 流不经 expander）

### 6.3 不引入回归的明示口

reviewer 检查项，下述文件应零改动：

- `packages/intent-parser/**`
- `packages/evals/**` 测试数据集与生成器
- `packages/search-backends/spotlight/src/lib.rs` 中除 `keyword_predicate` 与 `SearchBackend` impl 之外的所有函数

## 7. 手测 scenario（落 `docs/manual-test-scenarios.md` BETA-11 节）

| # | 自然语言 query | 准备 fixture | 期望 | 验证什么 |
|---|---|---|---|---|
| 1 | `找一份工作汇报相关的ppt` | `述职.ppt` | 命中 | **用户原 case**：zh office 桶 |
| 2 | `找最近的截图` | `屏幕截图 2024-...png` | 命中 | zh media 桶 |
| 3 | `find a slideshow` | `foo.pptx` | 命中 | en file_type 桶 |
| 4 | `找合同` | `购房协议.pdf` | 命中 | zh document 桶 |
| 5 | `find a photo` | `bar.jpg` | 命中（picture 文件名）| en media 桶 |
| 6 | `找 synthetic-place 的笔记` | `synthetic-place-note.md` | 命中（精确）| hyphenated 标识符**不被误扩** |
| 7 | `LOCIFIND_TRACE=/tmp/a.jsonl` + #1 | — | trace 含 `synonym_expand` 一行 | 可解释 |
| 8 | mv 走 yaml + 重启 + 跑 #1 | — | 命中"述职.ppt" 退化为不命中 | `NoopExpander` degrade 路径 |

## 8. 安全 / 隐私

- 词典文件 checked-in 仓库，不在用户机本地生成；不读取用户数据生成词典
- 扩出来的 alias 仍走现有 `escape_predicate_string` 防止 mdfind 谓词注入
- trace 落 JSONL 沿用既有 `LOCIFIND_TRACE` 开关；NoopExpander 路径不写 trace（默认场景零侧信道）

## 9. 不做（YAGNI 闸口，对应 §2）

见 §2"不做"小节。

## 10. 升级路径（建议进 ROADMAP）

| ID | 阶段 | 内容 |
|---|---|---|
| BETA-11A | BETA 中后期 | 同义词召回评测集（独立 30~50 case fixture + query + 期望命中）。定量衡量召回 / 假阳 |
| BETA-11B | BETA 中后期 | 词典从 yaml 升级为 **embedding 索引**（本地 sentence-transformers + ANN）或 **LoRA 在线扩词**。二选一时再评估 |
| BETA-11C | BETA 中后期 | WindowsSearchBackend / EverythingBackend 覆盖 `search_expanded` |

升级时点判定：手维护词典 > 200 组、或出现可重复的"用户原 case 找不到"反馈时进 BETA-11B。

## 11. 修订记录

| 日期 | 修订 |
|---|---|
| 2026-05-30 | v0.1：初稿（Claude Code Opus 4.7，brainstorming 流程） |
