# BETA-23：模型 fallback 接入桌面搜索默认流程（设计 spec）

> 日期：2026-06-12（macOS 会话）
> 状态：已与用户逐节确认（架构/组件/测试三节均批准）
> 起因：真机问题 4——复合查询「2025年的会议纪要文件名包含运维」被 parser 丢「会议纪要」。
> 任务编号：**BETA-23**（新登记；ROADMAP 原 BETA-15B 是 embedding/LoRA 扩词方向，与本任务区分，原卡片补指引）。

## 1. 背景与现状

### 1.1 已就绪的部件（MVP-17 / BETA-17 遗产）

- **触发器**：`intent-parser::fallback::should_invoke_model`——Clarify 或结构性遗漏（time/size/sort/location/action/media 六类信号 vs intent 字段）时返回 `InvokeModel`。
- **hybrid patch 模式**：`hybrid.rs`——parser 锁 variant + 已填字段，模型只输出待填字段 patch JSON；`apply_patch` 忽略 `intent`/`schema_version`。
- **模型与推理**：BETA-17 胜出模型 `models/qwen3-0.6b-q4_k_m.gguf`（LoRA 融合，378MB，evals with-fallback 96.0%，弱核显 p95 1.2s）。真实推理走 **llama-cpp feature**（llama-cpp-4 0.3.0，构建需 cmake；macOS 有 metal feature）。KV 前缀缓存（`generate_cached_prefix`）+ `stop_at_json` 提前停止两项优化已落地。
- **接入点现成**：desktop `search.rs` 已调 `resolve_intent(&query, None)`——fallback 参数传 `None`；`settings.rs` 已有 `enable_model_fallback: bool`（默认 true）但**无任何使用点**。
- **evals**：`--with-fallback --hybrid --model-path` 已支持，feature `model-fallback` 隔离 llama-cpp 编译。

### 1.2 两项修正（探索中实测确认）

1. **STATUS 中「candle GGUF 真实推理就绪」不准确**：candle 后端 `generate` 是占位 echo。真实推理只有 llama-cpp 后端。本设计基于 llama-cpp。
2. **问题 4 的查询当前不会触发 fallback**（probe 实测）：parser 对该 query 产出 `keywords:["运维"] + modified_time(2025) + sort`，六类信号全无遗漏 → `UseParser`。触发器**没有「内容词遗漏」检查**——只接入不扩展触发器修不了问题 4。

## 2. 范围决策（已与用户确认）

| 决策点 | 选择 |
|---|---|
| 触发器 | **扩展**：新增「内容词未被 keywords 覆盖」检查（修问题 4 的必要条件） |
| 模型分发 | **约定目录 + 可选路径覆盖**：默认扫 app 数据目录 `models/`，settings 加 `model_path` 覆盖；找不到则降级 parser-only |
| 加载策略 | **首次触发懒加载**：后台加载不阻塞当次查询（当次用 parser 结果），加载后常驻 |
| 等待策略 | **同步等待 + 超时兜底**：触发且就绪时同步等 patch（UI 发提示事件），3s 超时或失败回落 parser intent |
| 构建链 | desktop 加**非默认 feature** `model-fallback`（学 evals），日常 workspace 构建不编 llama-cpp；Release CI 打开 |
| 编排位置 | **方案 A**：desktop 新模块编排；intent-parser 只加纯函数（触发器扩展 + prompt 调整 + keywords 合并语义） |

## 3. 架构与数据流

### 3.1 搜索主流程改动

唯一改动点：`search.rs` 的 `resolve_intent(&query, None)` 替换为 `search::model_fallback::resolve(query, state)`：

```text
search_impl(query)
  ↓
model_fallback::resolve(query, state)
  ├─ parse_with_signals(query)                  [intent-parser，不变]
  ├─ should_invoke_model(&parsed)               [intent-parser，触发器已扩展]
  ├─ UseParser ──────────────────→ parser intent（绝大多数查询，零开销）
  └─ InvokeModel(reason)
      ├─ 开关关 / feature 关 ─────→ parser intent（source=ParserNoFallback）
      ├─ 模型未加载 ─────────────→ 发起一次后台加载，本次用 parser intent
      ├─ 加载失败 / 推理占用中 ───→ parser intent
      └─ 就绪 → 发 ModelThinking 事件 → spawn_blocking(invoke_hybrid) + 3s 超时
          ├─ Ok(patched intent) ─→ 模型补全 intent（source=Model）
          └─ Err / 超时 ─────────→ parser intent（记 trace，不报错给用户）
```

之后流程（refine / 同义词扩展 / 路由 / fan-out）不动——模型补出的 keywords 自然流入 expand 与 gazetteer。**任何路径下搜索不因模型而失败**（硬约束）。

### 3.2 触发器扩展（intent-parser，纯函数）

`analyze_structural_omissions` 新增第七类检查 `"keywords"`——**内容词覆盖检测**：

1. 把 query 切成内容候选段（CJK 连续段 + 英文 content token），剥离时间/大小/排序/位置/类型词、扩展名、结构词（「文件名包含」「找」等前导动词）、停用词——**复用 BETA-13 G1 已有词表与边界逻辑**（的/了 边界、停用词表），不另起炉灶。
2. 检查剩余段是否被 intent 已有字段（keywords / extensions / artist / album / title / genre / location.hint）**子串覆盖**（双向：覆盖值为段的超串也算覆盖；替换按字符数降序防互为子串误伤）。
3. 存在未覆盖的 ≥2 字 CJK 段或 ≥3 字符英文词 → omissions 加 `"keywords"` → `fillable_fields` 含 keywords，hybrid prompt 引导模型补。

适用 intent：**仅 FileSearch**（实现期实测：MediaSearch 臂复用 FileSearch 剥离词表会带来 v0.9 媒体模板 +4.7% 误触发——songs by X / 时长 / 无损等媒体框架词未剥；媒体侧覆盖检测连同其噪声词表留后续 task，漏触发只回到现状。FileAction / Refine / Clarify 维持现状跳过）。

### 3.3 hybrid prompt 调整

- 固定前缀的「字段值速查」与 few-shot 补 **keywords 补全示例**（含问题 4 同型 case：「文件名包含X」+ 内容词）。
- 前缀仍跨 query 稳定（KV 缓存前提不破），`prefix_is_query_independent` 测试继续守。

### 3.4 keywords 合并语义（apply_patch 特例）

现状 `apply_patch` 整字段覆盖——模型补 keywords 时若丢掉 parser 已抽对的词（如「运维」）即倒退。改为：**patch 含 `keywords` 时与 draft 的 keywords 取并集（去重，draft 在前）**；其他字段维持覆盖语义。改在库内（hybrid.rs），evals 与 desktop 走同一路径。

### 3.5 两条硬底线

- `parse()` 本体**零改动** → v0.5(473) / v0.9(726) parser-only **byte-equal 不动**（机械验证）。
- 误触发率在 v0.9 集上量化（fire rate 报告），词表调到触发集中在真遗漏为止。

## 4. desktop 组件与构建链

### 4.1 Cargo / feature

- desktop 加直接依赖 `locifind-model-runtime`（无条件——它本就是 intent-parser 的传递依赖，零额外成本）+ 非默认 feature：
  - `model-fallback = ["locifind-model-runtime/llama-cpp"]`（feature 只控推理后端，不控依赖本身）
  - `model-fallback-metal = ["model-fallback", "locifind-model-runtime/metal"]`（macOS 真机）
- 日常 `cargo test --workspace` 不开 feature → 不编 llama-cpp C++（三工具开发体验不变）。
- `release-windows.yml` 构建加 `--features model-fallback`（GitHub runner 自带 cmake）。
- feature 关闭时 `model_fallback::resolve` 退化为现状（cfg 编译掉 daemon 分支），设置页显示「本构建不含模型支持」。

### 4.2 新模块 `search/model_fallback.rs`（编排状态机，约 200 行）

```rust
enum ModelState {
    NotLoaded,                  // 初始（feature 开）；首次触发探测文件，缺失不 latch（放好后下次触发即加载）
    Loading,                    // 后台线程 load_blocking 中（完成后预热 hybrid 前缀 KV 再转 Ready）
    Ready(Arc<ModelFallback>),  // 常驻
    Failed(String),             // 加载失败，不再重试，设置页可见原因
    Unavailable(String),        // feature 关（构造期即 latch）/ 显式禁用，带原因
}
```

（设置页快照在此之上细分 6 态：ready / loading / failed / unavailable / not_found（NotLoaded+文件缺失）/ not_loaded（NotLoaded+文件就位）。）

- 全局 `Mutex<ModelState>` 挂 `SearchDeps`（沿用 BETA-06 `with_audit` 注入模式，避免改全部 `new()` 调用点）。
- 推理**单飞**：同一时刻仅一个 in-flight 推理（`try_lock`），占用中的查询直接 parser——避免超时被弃的推理线程造成排队堆积。
- 超时 3s：`tokio::time::timeout` 包 `spawn_blocking`；超时丢弃结果，被弃线程自然结束（单飞守卫挡并发）。

### 4.3 模型发现与设置

- 默认路径：`app 数据目录/models/qwen3-0.6b-q4_k_m.gguf`（与 index.db 同级）；`settings.json` 加 `model_path: Option<String>` 覆盖。
- 启动零 IO，首次触发才探测文件。
- 设置页：补 `enable_model_fallback` 开关 UI；新增模型状态行（未找到→提示放置路径 / 加载中 / 已就绪+路径 / 失败+原因）；`model_path` 输入框。

### 4.4 前端事件

- 新增轻量事件 `ModelThinking`（invoke 前发）→ 搜索框下显示「正在理解查询…」。
- `Started` 沿用既有 `fallback_used` 字段（`source == Model` 时为 true），结果区可见「本次由模型补全」；触发原因记 stderr 开发日志（避免为 `Started` 加字段而牵动 fanout/balanced 三处构造点与 `ResolvedQuery`）。

### 4.5 隐私口径

推理 100% 本进程内；prompt（含 query 文本）不落盘不外发；失败/超时原因与耗时记 stderr 开发日志，**不新增 trace 事件**（隐私面最小化，prompt 全文与 patch 内容按构造不进任何持久记录）；audit 不涉及（非文件操作）。

## 5. 测试与验收

### 5.1 单元测试

- **触发器扩展**：问题 4 query → fillable 含 `keywords`；反例集（「上周的pdf」「周杰伦的歌」「最大的文件」等 parser 已全覆盖 query）不误触发；中英混合、纯英文覆盖。
- **apply_patch 并集语义**：模型丢 parser 已有词仍保留；去重；其他字段覆盖语义不变（防回归）。
- **desktop 状态机**（stub daemon，无需真模型）：NotLoaded→Loading→Ready、文件不存在→Unavailable、加载失败→Failed 不重试、单飞占用跳过、超时回落、开关关直通。

### 5.2 evals 三道门

1. **parser-only 硬门**：v0.5 473 / v0.9 726 **byte-equal 不动**。
2. **with-fallback 对比**：本机（metal）`--with-fallback --hybrid` v0.9 before/after，逐 case 报告 gains/regressions，**净回退 = 0**（模型把 parser 已对的改错属阻断项）。
3. **fire rate 报告**：v0.9 触发比例、按 reason 分桶；触发应集中在真遗漏 case，过高则收紧词表再测。

### 5.3 性能与资源（记录实测值）

- 触发查询 p95 ≤ 3s（含超时兜底；本机 metal 预期 <1s）。
- 未触发查询零额外延迟（UseParser 路径不碰模型代码，机械保证）。
- 常驻内存增量实测记录（预期 ~0.5GB，仅首次触发后）。
- 零新外部依赖（llama-cpp-4 已在 Cargo.toml，feature 隔离）。

### 5.4 真机验证（macOS 本机）

- 问题 4 query「2025年的会议纪要文件名包含运维」端到端命中会议纪要文件。
- 无模型文件时全功能降级（搜索照常、设置页提示）。
- 设置页状态/开关/路径覆盖各路径手测。

（2026-06-13 验证后记：问题 4 query 的触发/编排/回落链路端到端通过；keywords 补全因 LoRA 模型输出空 patch 未生效，登记 BETA-24 重训。）

### 5.5 常规质量门

fmt + clippy（`--workspace -D warnings`，**feature 开/关两种形态**）+ 全 workspace test 零回归。

## 6. 落库与登记

- feature 分支开发（命名如 `feat-beta-23-model-fallback`）。
- ROADMAP §3.3 登记 **BETA-23** 新卡片；原 BETA-15B 卡片补一句「问题 4 模型 fallback 接入已拆为 BETA-23」。
- STATUS「下一步」对应条目更新；candle「真实推理就绪」表述顺带修正。

## 7. 非目标（防范围蔓延）

- 不做 embedding 召回补强 / LoRA 在线扩词（留 BETA-15B）。
- 不做应用内模型下载（v1 手动放置；后续可升级）。
- 不做双阶段结果刷新（先 parser 后模型重跑）。
- 不做空闲卸载模型（常驻即可，v1 不引入卸载竞态）。
- 不动 candle 后端（占位现状保留，仅修文档表述）。
