# MediaSearch 100 Case 人工分桶分析报告 (v0.4)

本文档对 MediaSearch 变体中 98 条失败用例（含 Partial/Fail）进行了分类分桶，旨在识别 parser v0.3 后的主要缺口。

## 1. 统计概览

| Bucket | 类别 | Count | 占比 |
|:---|:---|:---|:---|
| **Bucket C** | media_type 误判 / 缺失 | 54 | 55.1% |
| **Bucket E** | artist 漏识别 / 多识别 | 18 | 18.4% |
| **Bucket F** | 时间字段 misroute (created vs modified) | 10 | 10.2% |
| **Bucket D** | 英文 stop words 污染 keywords | 8 | 8.2% |
| **Bucket A** | 多余 extensions (预期无，实际有) | 4 | 4.1% |
| **Bucket G** | 其他 (Location 缺失等) | 4 | 4.1% |
| **Bucket B** | title vs quality 边界 | 0 | 0.0% |
| **Total** | | **98** | **100%** |

---

## 2. 各桶代表用例

### Bucket C: media_type 误判 / 缺失 (54)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-schema-15 | 找昨天截的付款二维码 | media: screenshot | media: audio | "二维码" 触发了音频误判 |
| v05-media-sort-052 | 找最大的的视频 | media: video | media: null | "最大的" 干扰了视频类型识别 |
| v05-media-week-064 | 找一周内修改的视频 | media: video | media: null | "修改的" 干扰了视频类型识别 |
| v05-media-week-066 | 找本周修改的视频 | media: video | media: null | 时间词干扰了类型识别 |
| v05-media-sort-060 | find the biggest video | media: video | media: null | 英文 size 信号导致 media_type 丢失 |

### Bucket E: artist 漏识别 / 多识别 (18)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-media-temp-245 | 找 synthetic-artist 的歌 | artist: synthetic-artist | artist: null | 无法识别合成/陌生艺人名 |
| v05-media-temp-249 | 找 synthetic-artist 的歌 | artist: synthetic-artist | artist: null | 规则未覆盖 "X 的歌" 结构 |
| v05-media-temp-253 | 找 synthetic-artist 的歌 | artist: synthetic-artist | artist: null | 同上，艺人名被漏掉 |
| v05-media-temp-257 | 找 synthetic-artist 的歌 | artist: synthetic-artist | artist: null | 同上 |
| v05-media-temp-261 | 找 synthetic-artist 的歌 | artist: synthetic-artist | artist: null | 同上 |

### Bucket F: 时间字段 misroute (10)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-media-temp-244 | 找上周截的 ... 截图 | modified: last_week | created: last_week | 截图默认映射到 created_time |
| v05-media-temp-248 | 找本周截的 ... 截图 | modified: last_week | created: last_week | 截图时间属性路由不一致 |
| v05-media-temp-252 | 找昨天截的 ... 截图 | modified: yesterday | created: yesterday | 截图同上 |
| v05-media-temp-256 | 找最近一周截的 ... | modified: last_7_days | created: last_7_days | 截图同上 |
| v05-media-temp-260 | 找上个月截的 ... | modified: last_month | created: last_month | 截图同上 |

### Bucket D: 英文 stop words 污染 keywords (8)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-schema-20 | screenshots from last month | keywords: [] | keywords: ['from', 'last', 'month'] | 停用词未过滤干净 |
| v05-schema-44 | find JPG and PNG screenshots | keywords: [] | keywords: ['JPG', 'and', 'PNG', ...] | 连词和后缀进入关键字 |
| v05-media-week-070 | find videos modified this week | keywords: [] | keywords: ['this'] | 指示词泄露 |
| v05-media-temp-284 | screenshots from last week in downloads | keywords: [] | keywords: ['from', 'last', 'week', ...] | 介词泄露 |
| v05-media-temp-296 | screenshots from past 7 days ... | keywords: [] | keywords: ['from', 'past', 'days', ...] | 英文描述词泄露 |

### Bucket A: 多余 extensions (4)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-schema-13 | 找周华健的朋友 | ext: null | ext: ['mp3', 'flac', ...] | 音频类自动补全了后缀 |
| v05-schema-14 | 找 ... 无损音乐 | ext: null | ext: ['mp3', 'flac', ...] | 音频类自动补全了后缀 |
| v05-schema-23 | find audio files by Eric Clapton | ext: null | ext: ['mp3', 'flac', ...] | 明确指定 audio 时不应出 ext |
| v05-schema-28 | find 周华健 的歌 | ext: null | ext: ['mp3', 'flac', ...] | 音频类自动补全了后缀 |

### Bucket G: 其他 (4)
| ID | Query | Expected | Actual | 归因 |
|:---|:---|:---|:---|:---|
| v05-media-temp-302 | find 上周 的截图 | location: downloads | location: null | 模板用例中的隐式路径解析失败 |
| v05-media-temp-306 | find 本周 的截图 | location: documents | location: null | 模板用例中的隐式路径解析失败 |
| v05-media-temp-310 | find 上周 的截图 | location: downloads | location: null | 模板用例中的隐式路径解析失败 |
| v05-media-temp-314 | find 本周 的截图 | location: 桌面 | location: null | 模板用例中的隐式路径解析失败 |
