# Spike 报告：全盘音频索引（Everything 发现层）

> 类型：研究性原型（spike），非正式功能
> 日期：2026-06-02（Windows 11 真机，主仓库 `C:\Users\alice\dev\LociFind`）
> 分支：`spike-disk-wide-audio`（原型代码，未合并）
> 关联：ROADMAP §3.3 B1 [BETA-01A](../../ROADMAP.md)；承接 [BETA-01 音乐索引](../superpowers/specs/2026-06-02-beta-01-music-metadata-index-design.md)

## 1. 动机

现状 `reindex` 命令写死 `default_music_roots()` = `dirs::audio_dir()`（仅 `C:\Users\<user>\Music`）。
用户实际音频散落在 OneDrive 学习资料、下载目录等处，**默认 Music 目录为空**，固定目录索引扫到 0 条。
需求：索引**电脑内任意位置**的音频，实现跨目录搜索。

## 2. 方案（发现 / 提取 / 存储 三层拆分）

不自建全盘扫描器，复用项目现成基础设施：

| 层 | 用什么 | 平台 |
|---|---|---|
| **发现**（枚举全盘音频路径） | Everything `es.exe ext:mp3;flac;...`（MVP-12 已集成） | Win；macOS 对应 Spotlight `mdfind 'kMDItemContentTypeTree==public.audio'` |
| **提取**（读 ID3 标签） | lofty `extract_metadata`（BETA-01 现成） | 跨平台 |
| **存储**（标签入库 + FTS5） | `MusicIndex`（BETA-01 现成） | 跨平台 |

**原型代码**（spike 分支）：
- `packages/indexer/src/scan.rs`：给 `MusicIndex` 加 `index_paths(&[PathBuf])`（索引显式路径列表，不递归不回收；mtime 跳过；提取失败计 `failed`）。
- `packages/indexer/examples/discover_audio.rs`：es.exe 发现（`-export-txt -utf8-bom` 规避 GBK stdout 破坏 CJK 路径）→ `index_paths` → 诊断 + 搜索 demo。

## 3. 实测结果（真实 1249 个文件）

| 阶段 | 结果 |
|---|---|
| **发现**（Everything 全盘） | 1249 条音频路径，**307ms**（瞬时，符合预期） |
| **提取 + 入库**（lofty 单线程） | 947 added / **302 failed** / **耗时 304.9s（~5 分钟，244ms/文件）** |
| **标签覆盖** | 947 入库中 artist 273 / title 279 / album 266（**~71% 无 artist 标签**） |
| **跨目录搜索** | `artist="@露珠英语工作室"` → 5 条跨目录命中 ✅ |

## 4. 关键发现（只有真跑才暴露）

### 4.1 OneDrive 占位符是真问题（302/1249 = 24% 失败）

```
读取标签失败 ...新概念（第3册）...20－Pioneer Pilots.mp3:
   已拒绝访问云文件。 (os error 395)   ← ERROR_CLOUD_FILE_ACCESS_DENIED
```

- "仅在线"（未下载）的 OneDrive 文件，lofty 一读即被拒。
- 成功的 947 个慢到 **244ms/文件**（本地裸盘 lofty 通常个位数毫秒），疑似 OneDrive 过滤驱动开销 / 触发水合下载。
- **风险**：全盘索引 OneDrive 内容既慢、又可能悄悄下载大量内容、还有 1/4 读不到。

### 4.2 标签质量差（覆盖率仅 ~21%）

入库 947 中仅 273 有 artist 标签（教学 mp3、游戏音效 `save.mp3`、`未知艺术家.wav` 占多数）。
**按标签搜只覆盖约 1/5**；其余只能靠文件名搜——而文件名跨目录搜系统后端（Windows Search / Everything）本就能做。

## 5. 结论与设计要点（喂给 BETA-01A）

架构（发现/提取/存储三层）**验证可行，搜索真能跨目录命中**。做成正式功能需补三处：

1. **跳过"仅在线"文件**：读标签前查 Windows 文件属性 `FILE_ATTRIBUTE_OFFLINE` / `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`，是占位符则跳过 → 既避开 24% 失败，**又避免触发 OneDrive 下载**。这些文件可只存文件名（不读标签），仍按名可搜。
2. **并行提取**：IO 密集，单线程 244ms×上千 = 5 分钟；rayon 线程池可砍到几分之一。
3. **文件名进 FTS**：标签稀疏，把 `file_name` 加进全文索引（现 FTS 仅 artist/title/album），按文件名搜也命中本地索引。

> 同样的「发现层全盘枚举 + 占位符跳过 + 并行 + 文件名 FTS」思路也适用于 BETA-02 文档索引，可在 BETA-01A 定型后推广。

## 6. 估时建议

约 3-4d（占位符属性检测 platform/windows 侧 + macos 侧；rayon 并行；file_name 入 FTS schema 迁移；Everything/Spotlight 发现 provider 接进 `LocalIndexBackend.reindex`；测试）。
