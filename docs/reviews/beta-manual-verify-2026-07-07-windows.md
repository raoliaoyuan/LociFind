# v0.9.18/19 Windows 真机验证（2026-07-07）

> 工具：Claude Code (Opus 4.8) 经 computer-use 驱动装机版桌面 App，+ 用户手动补验卸载/升级。
> 环境：用户真机（Windows 11），四状态灯全绿（Everything / 本地索引 / 语义召回 / Windows Search）。
> 索引现状：2 音乐 / 67 文档 / 17 图片(OCR)，上次索引 2026-07-07 09:49，数据全在本机路径（不上传）。

## 通过项

| 项 | 验证方式 | 证据 |
|---|---|---|
| 基础搜索回归 | computer-use | `找 pdf` → 50 条 / 229ms、intent=file_search、via search.everything；列齐（含 BETA-29「调整」+ 相似度列 + 同义词提示） |
| BETA-47 选项页七 tab | computer-use | 常规 / 索引 / Everything / 语义召回 / Windows / 隐私与记录 / 杂项；Windows 平台 tab 已显示 |
| BETA-51 设置统一 | computer-use | 「我的同义词」→ 选项对话框定位「杂项」tab 内联；「隐私与数据」→ 折叠进「隐私与记录」tab（索引概览 + 数据位置表）；取消/应用/确定 + × 返回路径完整，旧无返回整页已消除 |
| BETA-52 模型管理 | computer-use | ① 状态行「当前模型：embeddinggemma-300m-q8_0.gguf」；② 「检测」→「✓ 可用 · 313.3 MB」（只探不加载）；③ 「扫描本机 gguf」全盘扫出 3 个模型（跨 C:/D:，含 dev artifacts 份）+ 每项「设为语义/生成」按钮 |
| BETA-50 OCR 数字校正 | computer-use | 搜 `150138` 命中 `…-推考证.png`（类型=图片 OCR、匹配方式=内容）；预览命中片段 `150138` 高亮，OCR 文本底部有【OCR数字校正】追加行——原文保留 + 校正变体行使 6 位数字子串可搜 |
| BETA-12 卸载 / 覆盖升级 | 用户手动 | 用户实测「跑完了都没问题」（含发版阻断项「升级零数据损失」） |

## 第二轮（同日续验，computer-use 驱动）

| 项 | 验证方式 | 证据 |
|---|---|---|
| BETA-29 意图草稿 v1 | computer-use | 搜 `找上周的 pdf`（file_search + time）→「调整▾」面板字段与执行一致（扩展名 `.pdf`、类型 文档、修改时间 上周、排序 最近修改）；移除 `.pdf` chip →「按此条件重跑」→ 结果由 PDF 变 50 条 txt/html（无扩展名约束）、**修改时间「上周」保留**、命中全为上周日期（时间窗真生效）；重跑走 `search_with_intent` 跳过 parser |
| BETA-29 搜索前预览 v2 | computer-use | 输入 `找桌面上的 png` + Shift+Enter → 出现「预览 · file_search · **尚未执行搜索**」意图条 + 草稿面板（扩展名 `.png`、类型 图片），结果区不刷新；「按此条件搜索」→ 真执行（桌面无 png → 干净「未找到结果」空态，非报错） |
| BETA-33 cycle 9 单实例锁 | computer-use + tasklist | 最小化后从命令行起第二实例 → 既有窗口**取消最小化并置前、保持原状态**、无新窗口；`tasklist` 确认 `locifind-desktop.exe` 仅 1 个进程（PID 唯一），第二实例检测锁后自退 |
| BETA-33 cycle 9 设置流 | computer-use | Ctrl+; 打开选项、无改动时「应用」置灰（脏态跟踪）；改一项后点 ✕ → 弹「放弃未保存的改动？」二次确认 + 底部「⚠ 未保存改动」提示；点「放弃改动并关闭」→ 重开确认该项恢复原值（放弃真丢弃、用户配置零改动） |

## 本轮未覆盖（留后续 / macOS）

- **BETA-49** 音乐发现不越界（依赖用户目录配置）、**BETA-43** MCP 出处·审计（需 daemon + 外部 LLM 客户端）、**BETA-33 cycle 9** WSearch 服务状态条 / 全库-概貌口径差提示（需停系统服务 / 造索引口径差，不宜自动化）。
- **macOS 侧真机验证整体待跑**——§6 出场线 Class A 仅剩双平台 evals 真机（macOS 需完整 Spotlight 索引）。
