# 英文召回 option 2：复合多词键 + minutes 时长词修复 — 设计

> 状态：draft（待用户 review）
> 关联：B3.5 搜索召回增强 / BETA-15A 召回评测集 / 2026-06-01 英文召回停词修复（上一会话）
> ID：沿用 B3.5「en 召回 option 2」backlog 条目（STATUS「下一步」Class B）

## 1. 背景

2026-06-01 英文召回停词修复后，BETA-15A 同义词召回评测集 en 桶从 46.7% 升到 80.0%（总 95.6% / 假阳 0.0%），残留 **3 个 FAIL case**：

| case | query | 期望命中 | 词典键 |
|---|---|---|---|
| `recall-en-doc-06` | find my cover letter for the Google position | `f-application-docx`（job_application_google.docx） | `cover letter` → `[application]` |
| `recall-en-office-05` | find the style guide for our brand assets | `f-branding-pdf`（company_branding_assets.pdf） | `style guide` → `[branding, guidelines]` |
| `recall-en-office-02` | where are the minutes from the October all-hands | `f-minutes-docx`（weekly_meeting_minutes_oct.docx） | `meeting notes` → `[minutes, meeting minutes]` |

评测纯离线确定性（真 `parse → YamlSynonymExpander::expand → 子串匹配模拟`，不跑 Spotlight / 模型）。

## 2. 根因（已实测确认）

逐 query 跑 `locifind-cli --intent-only`：

| case | parser 输出 | 根因 |
|---|---|---|
| en-doc-06 | `FileSearch keywords:["cover"]` | parser 抽**单 token** `cover`；词典键是**多词** `cover letter`。`expand` 的「有 keyword」分支只对每个 keyword 做 `expand_one` 精确查表，从不扫描 query 中更长的多词词典键。次生：`cover` 恰是另一组（album art）的 alias，`expand_one("cover")` 会错扩到无关组。 |
| en-office-05 | `FileSearch keywords:["style"]` | 同上：单 token `style` ≠ 多词键 `style guide`；`style` 不是任何词典键 → singleton，不命中 branding。 |
| en-office-02 | `MediaSearch media_type:audio` | `media_search::has_strong_media_signal` 用裸 `lower.contains("minutes")` 判定 → "minutes"（会议纪要）被当时长词触发 media 路径 → **variant 漂移**，keyword 丢失。即便走 gazetteer 兜底，`is_pure_content_term("minutes")` 重解析仍得 media → 守护跳过 → 漏召回。 |

对照：`recall-en-office-01`「find the meeting notes...」已通过——parser 抽 `meeting`，命中 `meeting notes` 组 → 含 `minutes` alias → 命中文件名。说明问题不在词典，在抽取层（多词键 + 时长词误判）。

两类**独立**问题：
- **A. 复合多词键**（en-doc-06 / en-office-05）：抽取/扩展层不识别多词词典键。
- **B. minutes variant 漂移**（en-office-02）：parser 时长词用裸子串匹配，无数字上下文也触发 media。

## 3. 设计

### Fix A — 多词键覆盖（`packages/harness/src/synonym/yaml.rs`）

在 `YamlSynonymExpander::expand` 的「parser 有 keyword」分支后，增加一遍**多词 gazetteer 覆盖**：

1. 收集所有**含空格的词典键**（zh 当前无多词键，主要作用于 en）。
2. 对每个多词键 `mk`：若 `query`（小写比较）包含 `mk`、且 `is_pure_content_term(mk)` 为真、且 `mk` **包含**当前某个 parser 单 token keyword（按词边界/子串），则该多词键命中。
3. 命中后：用 `expand_one(mk)` 的组**替换**被它包含的那个单 token keyword 组（去重保序，多词键组排在原位置）。未被任何多词键包含的 keyword 组保持不变。
4. 多个多词键命中取**最长**（字符数），并列取 query 中首现位置靠前者——与既有 `gazetteer_lookup` 选择策略一致。

**关键约束（低风险保证）**：只有当多词词典键**字面出现在 query 中**、且**包含** parser 已抽出的某个 keyword 时才改变行为。不引入新的召回来源，只是把「单 token 错配/漏配」升级为「多词键正确扩展」。

> 复用：抽出一个内部 helper `multiword_keys()`（遍历两 index 的键，过滤含空格者），`expand` 与未来 backend 共用。`is_pure_content_term` 守护直接复用。

### Fix B — 时长词需数字上下文（`packages/intent-parser/src/parsers/media_search.rs`）

`has_strong_media_signal` 当前把 `分钟/小时/minute/minutes/hour/hours` 与真正的强媒体词（歌/audio/screenshot…）混在一个裸 `contains` 列表里。

修改：**拆分**为
- 真·强媒体词（歌/音乐/audio/song/录音/录像/截图/截屏/screenshot(s)/截的/截了）——保持裸 `contains`；
- 时长词（分钟/小时/minute(s)/hour(s)）——**仅当前面有数字才算媒体信号**，复用与 `has_explicit_size_threshold` 同款的 `\d+\s*(?:...)` 正则（中文「分钟/小时」可不带空格，英文允许空格）。

修后 "where are the minutes from the October all-hands" 不再有数字 → 不触发 media → 退回 file_search。预期 parser 在 file_search 路径抽出 `minutes`（或经词典命中 `meeting notes` 组），命中 `weekly_meeting_minutes_oct.docx`。

> 实现期需验证：修 B 后 en-office-02 的 parser 输出确实产生可被 expand 命中的 keyword（若 parser 把 "minutes" 抽为 keyword → `expand_one("minutes")` 命中 `meeting notes` 组含 `minutes` alias，匹配文件名）。若抽不出，则在 Fix A 的多词键覆盖 / gazetteer 兜底路径补齐（"minutes" 是单词键 alias，gazetteer 守护此时应通过，因为不再 parse 成 media）。

## 4. 验收 / 验证门

1. **召回评测**：`cargo run -p locifind-evals --bin synonym_recall` → en 80.0% → **100.0%**（3 FAIL 清零）；总召回 95.6%→100%；**假阳保持 0.0%**。
2. **v0.5 evals 不回归（硬门）**：`cargo run -p locifind-evals --bin evals` parser-only **pass ≥ 472 / fail ≤ 2**（Fix B 动 parser，这是首要 guard；若回归则当场回退 Fix B 方案）。
3. **新增单测**：
   - harness `yaml.rs`：多词键覆盖（cover letter→application 组 / style guide→branding 组）；单 token keyword 不被无关多词键误覆盖；多词键不出现在 query 时不改变行为。
   - parser `media_search.rs`：「5 minutes」「longer than 10 minutes」仍触发媒体；裸「minutes / the minutes」不触发；中文「5分钟」触发、裸「分钟」不触发（按现有用例边界）。
4. **`bash scripts/ci.sh` 全套绿**（fmt + clippy -D warnings + build + test + synonym-recall 门）。

## 5. 非目标（YAGNI）

- 不重写 parser 的 keyword 抽取整体策略（仅时长词判定一处）。
- 不引入 embedding / LoRA 在线扩词（那是 BETA-15B）。
- 不扩词典（词典已含三组，问题在抽取层）。
- 不处理跨范畴多类型 / 多概念多 group（各自独立 backlog）。

## 6. 风险

- **Fix B parser 回归**：时长词数字上下文收窄可能影响某些 media duration 用例。缓解：v0.5 evals pass≥472 硬门 + 时长词单测；若回归当场回退。
- **Fix A 误覆盖**：多词键覆盖仅在字面出现 + 包含已抽 keyword 时触发，理论上不增召回来源；召回评测假阳率 ≤5% 兜底。
