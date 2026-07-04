# 跨范畴多类型查询均衡展示（Cross-Category Balanced Display）

- 日期：2026-06-03
- 作者：Claude Code (Opus 4.8)
- 状态：spec
- 关联：承接 BETA-18（跨范畴多 `file_type`）真机手测发现的 backlog（STATUS Class B）

## 1. 问题

「图片和视频」「ppt 和 pdf」这类**跨范畴多 `file_type`** 查询，当一类数量碾压另一类
（真机实测：图片 10 万+ vs 视频 162）时，少数派类型在结果里**完全不可见**。

根因（比 backlog 原记录深一层）：

- BETA-18 后 parser 对「图片和视频」同时填 `extensions=Some(并集)` + `file_type=Some([Video,Image])`，
  后端 `extensions` 优先 → 实际发出**一个扩展名并集查询**。
- 无 keyword 的纯类型查询 `expanded_needs_content==false` → `route_search_fanout` 返回**单个**后端
  （被 `.filter(len>=2)` 滤掉，不走 `run_fanout_search`）→ 落 **fallback chain，由单后端服务**。
- 该单后端一次并集查询 + 默认 `modified_desc` + `limit=50` → top-50 几乎全是多数派类型，
  少数派**在后端就被截断**，根本进不了结果集。

结论：「视频在并集中、只需 ranker 交错」的原判断不成立——**少数派在到达 ranker 前已被 limit 截掉**。
只在 ranker 层重排救不回没返回的结果。

## 2. 决策（brainstorming，2026-06-03）

1. **修复层 = 源头按类型分别查询**（否决「仅 ranker 交错」=治标不治本；否决「提高 limit + 交错」=无保证、拉大数据量）。
   intent 含 N 个 `file_type` 时拆成 N 个单类型子查询，各类型各跑一遍自己的路由+执行（各得一份配额），
   再合并。保证少数派一定被召回。代价：N×（通常 2-3）后端查询，略慢。
2. **展示 = 类型间交错（round-robin）**（否决分组=少数派仍靠后；否决等配额截断后按时间混排=少数派可能仍靠后）。
   图片、视频、图片、视频…轮流取，各类型在前 N 条都可见；每类内部仍按原排序（modified_desc/相关性）。

## 3. 设计

### 3.1 触发条件（最小侵入）

仅当 `expanded.base` 是 `FileSearch` 且 `file_type` 去重后 `len() >= 2` 时走**均衡分支**；
否则**完全沿用现有路径**（单类型查询 byte-for-byte 不变）。`MediaSearch.media_type` 仍单值（独立 backlog），不在范围。

### 3.2 单类型子查询构造 `single_type_expanded`

`expanded.clone()` → 改 base（FileSearch）：

- `file_type = Some(vec![t])`；
- `extensions`：把原并集**按类型切回该类型子集**（`原 extensions ∩ extensions_for_file_type(t)`），
  保留用户显式收窄（如「png 和 mp4」只查 png/mp4 而非全图全视频）；交集空 → `None`，让后端按 `file_type` 派生。

`keyword_groups` 原样保留（同义词与类型无关）。

### 3.3 每类型执行（复用现有 fan-out 机制）

对每个类型 `t` 的子查询 `sub`：

```
backends = router.route_search_fanout(&sub)         // 内容查询→内容后端；纯类型→单后端
fallback = router.route_filename_fallback(&sub)      // Everything 文件名兜底
bucket   = run_fanout_merge_with_fallback(backends, fallback, &sub, …, on_result=收进桶)
ranked   = ranker::rank(bucket, RankContext::from_expanded(&sub))   // 桶内各自排序
```

→ 得 N 个已排序的桶。

### 3.4 交错合并 `ranker::interleave`

round-robin：轮流从每个桶取下一条，按 canonical `path` 去重（跨桶按扩展名天然不重，去重防御性）。
显式 `limit=L` → 交错后截断至 L（保「总数 ≤ L」语义，且 L 个名额按类型均衡分配）；
`limit=None` → 不截断（与 `run_fanout_search` 全发哲学一致）。

### 3.5 事件 / 上下文

`Started`（原 effective intent）+ 同义词事件各发一次；逐类型 `on_tool_call`/`on_tool_result`；
`Result` 逐条发交错后结果；`context.record(expanded.base, interleaved)` 记原多类型 intent 供 refine；
全桶空 → `Error{"未找到结果"}`。

## 4. 改动面

- **common (`locifind-search-backend`)**：抽出 `pub fn extensions_for_file_type(FileType)`（三后端原各持一份**完全相同**的副本 → 收拢单一信源），3 后端私有 `file_type_extensions` 改为委托。
- **ranker**：新增 `pub fn interleave(Vec<Vec<MergedResult>>) -> Vec<MergedResult>`（round-robin + path 去重）。
- **desktop `search.rs`**：`multi_file_types` 判定 + `single_type_expanded` 构造 + `run_balanced_multitype_search` 编排 + 在扩展后、fan-out 分流前插入均衡分支。

## 5. 回归边界

- 单类型查询（`file_type.len() < 2` 或非 FileSearch）**零行为变化**。
- evals 为 parser-only（472/26/2），不经 desktop 路由 → **不受影响**。
- 三后端 `file_type_extensions` 收拢前后表内容不变 → 后端既有测试守护。

## 6. 测试

- ranker：`interleave` round-robin 顺序 / 不等长桶 / 跨桶 path 去重 / 空桶。
- common：`extensions_for_file_type` 各类型扩展名（迁移等价）。
- desktop：`multi_file_types` 判定（多类型 Some / 单类型 None / 非 FileSearch None / 重复类型去重）；
  `single_type_expanded` 切分（file_type 单值 + extensions 交集收窄 + 交集空回 None）；
  均衡搜索端到端（mock 两后端，少数派类型在前若干条可见）。
- 真机（`#[ignore]`）：「图片和视频」少数派（视频）在结果前列可见。

## 7. 已知限制

- N×后端查询（N=类型数，通常 2-3）略慢于单查询；纯类型查询本就快（Everything ext 查询亚秒级）。
- `MediaSearch.media_type` 多值仍未做（带媒体修饰的跨范畴媒体查询，独立 backlog）。
- 交错为均匀 round-robin，不按各类型总量加权（如想「多数派多给名额」需后续）。
