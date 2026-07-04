# BETA-39 设计：图片语义索引 opt-in + 质量门槛（解除 BETA-33 cycle 4 一刀切）

> 2026-07-03 spec。关键决策四问已用户确认（全采推荐），见 §7。
> ROADMAP 卡片：BETA-39（packages/indexer + apps/desktop，依赖 BETA-33，估时 2-3d）。
> 验收原文：设置项 opt-in（默认关）；开启后图片走双层质量门槛（沿用 A 层 meaningful_ratio + 段级门槛）入语义索引；关闭时行为与 BETA-33 cycle 4 现状 byte-equal；已知污染 case（QQ 表情包乱码 OCR）仍被挡。

## 1. 背景与目标

BETA-33 cycle 4（2026-07-01）为止血 v0.9.4「搜作文命中 QQ 表情包」污染，在 B 层对图片 doc_type **一刀切跳过语义嵌入**（`embed_pending` 直跳 + `purge_short_body_vectors` 清图片向量 + `explain_semantic_hit_impl` 图片返空）。代价：聊天记录截图、扫描笔记照片等**真文字图片**也进不了语义召回（FTS 字面命中仍可用）。

目标：给愿意承担质量风险的用户一个 opt-in 开关，图片 OCR 文本经**更严的图片专属质量门槛**后进语义索引；默认关闭、关闭时与现状逐字节一致。

## 2. 核心矛盾与门槛设计（本次拍板：图片专属 ratio 0.75）

已知污染 case（QQ 表情包 OCR「动 @ 河的…」）meaningful_ratio ≈ 0.63，**通用 A 层门槛 0.6 挡不住**——这正是 cycle 4 加 B 层一刀切的原因。解除一刀切必须配更严的图片专属门槛，否则验收「已知 case 仍被挡」不成立。

| 门槛 | 非图片文档 | 图片（opt-in 开启后） |
| --- | --- | --- |
| 字数下限（trim 后） | 20（`MIN_EMBED_TEXT_CHARS`） | 20（沿用） |
| meaningful_ratio 下限 | 0.6（`MEANINGFUL_CHAR_RATIO_FLOOR`） | **0.75**（新 `IMAGE_MEANINGFUL_RATIO_FLOOR`） |

依据：真中文正文 ratio 通常 > 0.75、英文正文 > 0.90；已知乱码 case 实测 0.55-0.63。0.75 卡在乱码上界之上、真文字截图（通常 > 0.8）零误伤。备选 0.7 被否（乱码上界 0.63 附近有浮动、余量不足）；「字数下限提到 40」被否（短真文字截图不进语义索引代价大于收益，A 层 20 字 + 0.75 双条件已够）。

新 API：`embed.rs` 增 `is_image_embed_worthy(text) -> bool`（20 字 + ratio ≥ 0.75），与 `is_embed_worthy` 并列、单一信源供三处共用（embed_pending / purge / 段级 explain 的图片分支）。

## 3. 范围护栏（YAGNI）

- **默认关**：`enable_image_semantics: bool` 默认 false；关闭时 `embed_pending` / `purge_short_body_vectors` / `explain_semantic_hit_impl` 行为与现状 byte-equal。
- **不动 FTS 路径**：图片 OCR 文本进 FTS 字面检索的现状（BETA-03）与本卡无关、零改动。
- **不动 daemon**（apps/daemon / locifind-server 不调 embed_pending / purge，检索面无图片语义概念；企业场景需要时另开卡）。
- **不做每目录粒度开关**：全局一个布尔，够用为止。
- **不改 EXPLAIN_MIN_SCORE / EXPLAIN_TOP_N**：段级 cosine 下限 0.45 等既有常量不动。

## 4. 数据流改动

### 4.1 indexer（packages/indexer）

- `embed.rs`：新常量 `IMAGE_MEANINGFUL_RATIO_FLOOR = 0.75` + 新函数 `is_image_embed_worthy`。
- `doc_db.rs` `embed_pending(..., embed_images: bool)`：
  - `embed_images=false`（默认路径）：图片 doc_type 直跳（现状不变）；
  - `embed_images=true`：图片改走 `is_image_embed_worthy` 门槛，过了才入嵌；非图片文档仍走 `is_embed_worthy`（0.6，零回归）。
- `doc_db.rs` `purge_short_body_vectors(..., keep_worthy_images: bool)`（本次拍板：启动期 purge 依设置动态判）：
  - `false`（开关关）：清全部图片向量 + body 不合格向量（现状不变；开过再关，下次启动自动回收、恢复 byte-equal 态）；
  - `true`（开关开）：图片向量仅当 **不过 `is_image_embed_worthy`** 才清（历史乱码向量仍被回收），真文字图片向量保留。

### 4.2 desktop 后端（apps/desktop/src-tauri）

- `settings.rs`：`AppSettings` 增 `enable_image_semantics: bool`（`#[serde(default)]` 向前兼容，Default false）+ live-read helper `read_enable_image_semantics(settings_path)`（读/解析失败 → false，安全侧）。
- `main.rs` 启动期 purge：传 `keep_worthy_images = read_enable_image_semantics(...)`。
- `index_status.rs` `spawn_semantic_index` → `semantic_index_pass`：worker 启动时 live-read 设置传给 `embed_pending`（与 roots live-read 同节奏；改设置后下一轮语义 pass 生效，无需重启——但需重扫触发，UI 文案说明）。
- `preview.rs` `explain_semantic_hit_impl`（本次拍板：段级 explain 同步放开）：
  - 开关关 → 图片返空（现状）；
  - 开关开 → 图片走段落级 explain，段级门槛用**图片专属 ratio 0.75**（`explain_passages` 增门槛参数，防 v0.9.4「作文」段级 0.62 虚高复现）；非图片调用路径参数取旧值、byte-equal。
- 设置读取通道：`SearchDeps` 已有 settings_path 类 provider 先例（floor / weight）；explain 侧同款 live-read。

### 4.3 semantic-index（packages/search-backends/semantic-index）

- `explain.rs`：`explain_passages` 抽出带段级 ratio 参数的变体（如 `explain_passages_with_ratio`），旧签名保留转发默认 0.6（所有既有调用零改动）。

### 4.4 前端（apps/desktop/src）

- `PreferencesDialog.tsx`「索引」pane（本次拍板）：checkbox「让图片文字参与语义搜索（实验性）」+ 副文案（更严质量门槛 0.75 / 需重新索引生效 / 默认关闭防乱码 OCR 污染召回）。
- `PreferencesDialog.tsx` + `SettingsPage.tsx` 的 `AppSettings` interface **必须**同步加字段——`update_settings` 收前端整只对象，interface 缺字段会在保存时被 serde default 冲回 false。

## 5. 已知边界与 trade-off

- **开启后不自动补嵌**：`embed_pending` 由语义 pass 驱动（目前默认还挂在 `LOCIFIND_ENABLE_EMBED=1` 门后，BETA-31-v2 真修前不变）；开启开关后需触发重扫才见效。UI 副文案说明。
- **段级门槛对图片段落一视同仁用 0.75**：图片 OCR 段落里混入的低质段直接 skip，可能减少高亮段数量——符合「宁缺毋滥」的 cycle 4 精神。
- **`is_image_embed_worthy` 挡不住「高 ratio 但语义空」的极端 case**（理论存在）：一期接受，真机踩到再调门槛或加信号（登记 STATUS 观察项即可，不预做）。

## 6. 验收对照

| ROADMAP 验收 | 落点 |
| --- | --- |
| 设置项 opt-in（默认关） | `AppSettings.enable_image_semantics` default false + 「索引」pane checkbox |
| 开启后图片走双层质量门槛入语义索引 | A 层字数 20 + 图片专属 ratio 0.75（文档级）+ 段级 0.75（explain） |
| 关闭时与现状 byte-equal | 三处调用点默认参数路径零改动 + 单测断言 |
| 已知污染 case 仍被挡 | 单测：QQ 表情包风格文本（ratio ≈ 0.63）`is_image_embed_worthy` = false、`embed_pending(embed_images=true)` 不嵌、purge(keep=true) 仍清 |

## 7. 关键决策（2026-07-03 用户确认，全采推荐）

1. **图片专属门槛档位** → ratio 0.75（字数下限沿用 20）。备选 0.7 / 「0.75+字数 40」被否。
2. **开关关闭时旧图片向量处理** → 启动期 purge 依设置动态判：关（默认）清全部图片向量（现状 + 自动恢复）；开只清不过图片门槛的。备选「关闭后保留已嵌向量」被否（悖 byte-equal 验收）。
3. **段落级 explain** → 同步放开 + 图片专属段级门槛 0.75。备选「一期只做文档级」被否（体验不一致）。
4. **UI 位置** → 「选项 → 索引」pane checkbox + 副文案。

## 8. Cycle 计划

1. **cycle 1（indexer）**：`is_image_embed_worthy` + `embed_pending` / `purge_short_body_vectors` 参数化 + 单测（含 QQ case 仍被挡、默认路径 byte-equal）。
2. **cycle 2（semantic-index + desktop 后端）**：`explain_passages` ratio 参数化；settings 字段 + live-read；main.rs / index_status.rs / preview.rs 三调用点接线 + 单测。
3. **cycle 3（前端）**：两处 interface 字段 + 「索引」pane checkbox + 文案。
4. **cycle 4（收尾）**：全 workspace test + clippy/fmt；ROADMAP / STATUS / 相关 README 同步。
