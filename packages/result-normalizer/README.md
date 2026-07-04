# locifind-result-normalizer

BETA-04 多源搜索结果归一化合并。把 fan-out 多后端（系统搜索 + 本地索引）返回的
`SearchResult` 按 canonical path 去重合并为 `MergedResult` 列表——同一文件被多个后端命中时
合成一条，保留全部来源与命中类型。

> **排序（BM25 / 打分）留 [BETA-05 Ranker](../../ROADMAP.md)**；本层只去重合并 + 保持首现序。
> 设计见 [spec](../../docs/superpowers/specs/2026-06-02-beta-04-result-normalizer-design.md)。

## API

```rust
use locifind_result_normalizer::{merge_results, MergedResult};

let merged: Vec<MergedResult> = merge_results(all_results_from_multiple_backends);
// MergedResult { result, sources: Vec<BackendKind>, match_types: Vec<MatchType> }
```

合并规则：
- 按 `result.path` 去重（**路径规范化由各 backend 负责**——产出 `SearchResult` 时
  `canonicalize`，本层纯函数无 IO，按 path 字节相等去重）；
- `sources` / `match_types` 取并集（稳定去重序）；
- 代表结果取 `metadata_richness`（非空元数据字段数）最高者；
- `score` 取所有同 path 结果的最大值；
- 保持首现顺序。

纯函数、零外部依赖（仅 `locifind-search-backend`），完全可单测（8 单测）。

## 关联

- 上游：[`locifind-local-index-backend`](../search-backends/local-index)（本地索引源）+ 系统
  搜索后端（Spotlight / WindowsSearch / Everything）；
- 调用方：`locifind-harness::run_fanout_merge`（fan-out 多源查询后调本层合并）；
- 下游：BETA-05 Ranker（对合并集打分排序）。
