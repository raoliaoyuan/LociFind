# MVP-27 性能基准报告

> 日期：2026-05-26  
> 运行机：macOS 26.5，unknown，unknown RAM  
> Spotlight 索引状态：未知（Spotlight server is disabled.）

## 1. 三档延迟分布

| 档位 | p50 | p95 | p99 | 样本量 | 非成功退出 | 出场阈值（§6.2） | 结果 |
|---|---:|---:|---:|---:|---:|---|---|
| parser-only | 0.038 ms | 0.050 ms | 0.053 ms | 1870 | 0 | < 500ms p95 | ✅ |
| translate | 0.041 ms | 0.054 ms | 0.058 ms | 1581 | 0 | - | - |
| cli-intent-only-cold | 4.963 ms | 7.785 ms | 11.508 ms | 279 | 0 | - | - |
| cli-intent-only-warm | 4.962 ms | 7.233 ms | 9.496 ms | 1581 | 0 | < 500ms p95（参考） | ✅ |
| cli-search-cold | 18.097 ms | 19.424 ms | 44.185 ms | 279 | 111 | < 1500ms p95（需 Spotlight 完整） | ⚠️ |
| cli-search-warm | 18.194 ms | 19.775 ms | 44.014 ms | 1581 | 629 | < 1500ms p95（需 Spotlight 完整） | ⚠️ |

## 2. 分桶分析

复杂度分桶为 MVP-27 临时口径：按 expected intent 顶层约束字段数量分为 simple / medium / complex。

### parser-only

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| Clarify | 0.030 ms | 68 |
| FileAction | 0.037 ms | 85 |
| FileSearch | 0.051 ms | 1054 |
| MediaSearch | 0.048 ms | 527 |
| Refine | 0.040 ms | 136 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 0.050 ms | 476 |
| mixed | 0.044 ms | 221 |
| zh | 0.050 ms | 1173 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 0.050 ms | 1615 |
| simple | 0.046 ms | 255 |

### translate

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| FileSearch | 0.055 ms | 1054 |
| MediaSearch | 0.052 ms | 527 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 0.053 ms | 442 |
| mixed | 0.045 ms | 221 |
| zh | 0.055 ms | 918 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 0.054 ms | 1445 |
| simple | 0.050 ms | 136 |

### cli-intent-only-cold

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| FileSearch | 6.737 ms | 186 |
| MediaSearch | 8.133 ms | 93 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 9.779 ms | 78 |
| mixed | 5.253 ms | 39 |
| zh | 7.191 ms | 162 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 7.108 ms | 255 |
| simple | 10.611 ms | 24 |

### cli-intent-only-warm

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| FileSearch | 6.634 ms | 1054 |
| MediaSearch | 7.839 ms | 527 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 8.342 ms | 442 |
| mixed | 5.223 ms | 221 |
| zh | 7.008 ms | 918 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 6.883 ms | 1445 |
| simple | 9.881 ms | 136 |

### cli-search-cold

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| FileSearch | 19.893 ms | 186 |
| MediaSearch | 19.240 ms | 93 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 19.351 ms | 78 |
| mixed | 19.522 ms | 39 |
| zh | 19.691 ms | 162 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 19.288 ms | 255 |
| simple | 55.960 ms | 24 |

### cli-search-warm

按 intent variant：

| bucket | p95 | 样本量 |
|---|---:|---:|
| FileSearch | 22.091 ms | 1054 |
| MediaSearch | 19.577 ms | 527 |

按 language：

| bucket | p95 | 样本量 |
|---|---:|---:|
| en | 19.533 ms | 442 |
| mixed | 20.012 ms | 221 |
| zh | 19.923 ms | 918 |

按 fixture complexity：

| bucket | p95 | 样本量 |
|---|---:|---:|
| medium | 19.493 ms | 1445 |
| simple | 50.953 ms | 136 |

## 3. 与 PROTO-09 对比

PROTO-09 出场报告记录 CLI release `--intent-only` 单条查询约 4ms（含 fork + binary 加载）。本次基准默认使用 debug binary，且 CLI 档覆盖多条 fixture 与真实 mdfind 进程调用，预计高一个量级。

## 4. 瓶颈定位

### parser-only

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| v05-schema-6-006 | 0.088 ms | FileSearch | zh | 找文稿目录里 2025 年的 ppt | 规则分支 / 谓词字符串构造 |
| 8 | 0.070 ms | FileSearch | zh | 找过去一个月里大于 1GB 的视频 | 规则分支 / 谓词字符串构造 |
| 19 | 0.070 ms | FileSearch | en | find files over 100MB in downloads | 规则分支 / 谓词字符串构造 |
| 46a | 0.065 ms | FileSearch | zh | 找项目归档里的 budget pdf | 规则分支 / 谓词字符串构造 |
| v05-schema-24-024 | 0.059 ms | FileSearch | en | find videos larger than 1 GB | 规则分支 / 谓词字符串构造 |
| v05-schema-8-008 | 0.059 ms | FileSearch | zh | 找过去一个月里大于 1GB 的视频 | 规则分支 / 谓词字符串构造 |
| v05-schema-7-007 | 0.058 ms | FileSearch | zh | 找名字以「会议纪要」开头的文档 | 规则分支 / 谓词字符串构造 |
| 24 | 0.057 ms | FileSearch | en | find videos larger than 1 GB | 规则分支 / 谓词字符串构造 |
| 11 | 0.055 ms | FileSearch | zh | 找 2026 年 5 月 1 日之前修改的 zip | 规则分支 / 谓词字符串构造 |
| 22 | 0.053 ms | FileSearch | en | find Excel modified in the past 7 days | 规则分支 / 谓词字符串构造 |

### translate

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| 7 | 0.083 ms | FileSearch | zh | 找名字以「会议纪要」开头的文档 | 规则分支 / 谓词字符串构造 |
| v05-media-class1-sort-054 | 0.078 ms | MediaSearch | zh | 找最大的视频 | 规则分支 / 谓词字符串构造 |
| 30 | 0.071 ms | FileSearch | mixed | show me 上周的 PDF | 规则分支 / 谓词字符串构造 |
| v05-schema-44-045 | 0.066 ms | MediaSearch | en | find JPG and PNG screenshots from yesterday | 规则分支 / 谓词字符串构造 |
| 8 | 0.062 ms | FileSearch | zh | 找过去一个月里大于 1GB 的视频 | 规则分支 / 谓词字符串构造 |
| v05-schema-24-024 | 0.061 ms | FileSearch | en | find videos larger than 1 GB | 规则分支 / 谓词字符串构造 |
| 44 | 0.060 ms | MediaSearch | en | find JPG and PNG screenshots from yesterday | 规则分支 / 谓词字符串构造 |
| 11 | 0.059 ms | FileSearch | zh | 找 2026 年 5 月 1 日之前修改的 zip | 规则分支 / 谓词字符串构造 |
| v05-file-class1-size-075 | 0.059 ms | FileSearch | zh | 找下载目录里200 MB 以上的文件 | 规则分支 / 谓词字符串构造 |
| v05-schema-7-007 | 0.057 ms | FileSearch | zh | 找名字以「会议纪要」开头的文档 | 规则分支 / 谓词字符串构造 |

### cli-intent-only-cold

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| 1 | 327.752 ms | FileSearch | zh | 查找昨天编辑过的 ppt | binary 加载 / process spawn / 冷缓存 |
| v05-media-class1-sort-060 | 12.764 ms | MediaSearch | en | find the biggest video | binary 加载 / process spawn / 冷缓存 |
| v05-file-class1-sort-059 | 11.508 ms | FileSearch | en | find the biggest ppt in downloads | binary 加载 / process spawn / 冷缓存 |
| v05-media-class1-sort-058 | 10.611 ms | MediaSearch | zh | 找体积最大的视频 | binary 加载 / process spawn / 冷缓存 |
| v05-file-class1-sort-061 | 9.779 ms | FileSearch | en | find the largest ppt in downloads | binary 加载 / process spawn / 冷缓存 |
| v05-media-class1-sort-062 | 7.571 ms | MediaSearch | en | find the largest video | binary 加载 / process spawn / 冷缓存 |
| v05-schema-14-014 | 7.218 ms | MediaSearch | zh | 找上个月下载的周华健无损音乐 | binary 加载 / process spawn / 冷缓存 |
| v05-schema-12-012 | 7.204 ms | MediaSearch | zh | 找一首周华健的歌 | binary 加载 / process spawn / 冷缓存 |
| v05-schema-15-015 | 7.191 ms | MediaSearch | zh | 找我昨天截的付款二维码 | binary 加载 / process spawn / 冷缓存 |
| v05-schema-13-013 | 7.108 ms | MediaSearch | zh | 找周华健的朋友 | binary 加载 / process spawn / 冷缓存 |

### cli-intent-only-warm

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| v05-file-class1-week-071 | 14.037 ms | FileSearch | en | find ppt modified past 7 days | CLI process spawn |
| v05-media-class1-sort-060 | 12.414 ms | MediaSearch | en | find the biggest video | CLI process spawn |
| v05-media-class1-sort-058 | 11.640 ms | MediaSearch | zh | 找体积最大的视频 | CLI process spawn |
| v05-file-class1-sort-057 | 11.606 ms | FileSearch | zh | 找下载目录里体积最大的 ppt | CLI process spawn |
| v05-file-class1-sort-059 | 11.337 ms | FileSearch | en | find the biggest ppt in downloads | CLI process spawn |
| v05-media-class1-sort-052 | 10.430 ms | MediaSearch | zh | 找最大的的视频 | CLI process spawn |
| v05-file-class1-sort-061 | 8.790 ms | FileSearch | en | find the largest ppt in downloads | CLI process spawn |
| v05-media-class1-week-070 | 8.386 ms | MediaSearch | en | find videos modified this week | CLI process spawn |
| v05-media-class1-sort-056 | 8.288 ms | MediaSearch | zh | 找最重的视频 | CLI process spawn |
| v05-schema-14-014 | 7.624 ms | MediaSearch | zh | 找上个月下载的周华健无损音乐 | CLI process spawn |

### cli-search-cold

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| 4 | 56.416 ms | FileSearch | zh | 找名字里有「预算」的文件 | binary 加载 / process spawn / 冷缓存 |
| v05-schema-4-004 | 55.960 ms | FileSearch | zh | 找名字里有「预算」的文件 | binary 加载 / process spawn / 冷缓存 |
| 2 | 34.547 ms | FileSearch | zh | 找最近三天修改的 Excel | binary 加载 / process spawn / 冷缓存 |
| 18 | 20.070 ms | FileSearch | en | find pdf modified last week | binary 加载 / process spawn / 冷缓存 |
| 26 | 19.893 ms | FileSearch | mixed | 找 downloads 里的 mp4 | binary 加载 / process spawn / 冷缓存 |
| v05-schema-2-002 | 19.691 ms | FileSearch | zh | 找最近三天修改的 Excel | binary 加载 / process spawn / 冷缓存 |
| v05-schema-28-028 | 19.522 ms | MediaSearch | mixed | find 周华健 的歌 | binary 加载 / process spawn / 冷缓存 |
| 19 | 19.446 ms | FileSearch | en | find files over 100MB in downloads | binary 加载 / process spawn / 冷缓存 |
| 23 | 19.424 ms | MediaSearch | en | find audio files by Eric Clapton | binary 加载 / process spawn / 冷缓存 |
| v05-schema-10-010 | 19.413 ms | FileSearch | zh | 找最近一周访问过的 markdown | binary 加载 / process spawn / 冷缓存 |

### cli-search-warm

| case | max | variant | language | query | 初步归因 |
|---|---:|---|---|---|---|
| 4 | 55.881 ms | FileSearch | zh | 找名字里有「预算」的文件 | mdfind process spawn / Spotlight 索引命中 |
| v05-schema-4-004 | 55.176 ms | FileSearch | zh | 找名字里有「预算」的文件 | mdfind process spawn / Spotlight 索引命中 |
| 18 | 38.312 ms | FileSearch | en | find pdf modified last week | mdfind process spawn / Spotlight 索引命中 |
| 21 | 35.262 ms | FileSearch | en | find images on desktop | mdfind process spawn / Spotlight 索引命中 |
| v05-schema-25-025 | 34.789 ms | FileSearch | mixed | 找我 yesterday 改过的 ppt | mdfind process spawn / Spotlight 索引命中 |
| v05-media-class1-sort-056 | 34.651 ms | MediaSearch | zh | 找最重的视频 | mdfind process spawn / Spotlight 索引命中 |
| 25 | 30.515 ms | FileSearch | mixed | 找我 yesterday 改过的 ppt | mdfind process spawn / Spotlight 索引命中 |
| 10 | 30.057 ms | FileSearch | zh | 找最近一周访问过的 markdown | mdfind process spawn / Spotlight 索引命中 |
| 26 | 29.762 ms | FileSearch | mixed | 找 downloads 里的 mp4 | mdfind process spawn / Spotlight 索引命中 |
| v05-schema-9-009 | 29.089 ms | FileSearch | zh | 找上周收到的 pdf | mdfind process spawn / Spotlight 索引命中 |

## 5. 出场建议

- parser-only 对照 §6.2「简单查询响应（规则解析路径）p95 < 500ms」。
- CLI 完整搜索 warm 对照 §6.2「简单查询响应」的交互体感阈值 p95 < 1500ms；cold 样本仅用于观察进程启动与索引预热影响。
- translate 档暂无 §6.2 硬阈值，作为定位 parser 与 backend process spawn 之间的纯 CPU 基线。
- 本机 `mdutil` 显示 Spotlight server disabled；CLI 完整搜索存在非成功退出，当前 CLI 搜索数字只能作为本机观测，不作为正式出场通过证据。
- 若 CLI warm 超阈值，下一步应落到 Spotlight process 启动/索引预热与 CLI release 构建基准复测；若 parser-only 超阈值，才进入 parser 规则路径优化。

## 附录：运行参数

- runs_per_case：20
- warmup_per_case：3
- fixture_dir：`/tmp/locifind-evals-perf-fixtures`
- rustc：`rustc 1.95.0 (59807616e 2026-04-14)`
