# Search Intent Schema / SearchBackend Trait 审阅

> 审阅人：Codex  
> 日期：2026-05-25  
> 对象：
> - [search-intent-schema.md](../search-intent-schema.md) v1.0 草稿
> - [search-backend-trait.md](../search-backend-trait.md) v0.1 草稿

## 总体结论

两份草稿方向正确，可以支撑“自然语言 → SearchIntent → SpotlightBackend → 结果”的技术原型。但 v1.0 发布前建议先修几处会影响实现一致性的契约问题：`refine.delta` 的列表合并语义、JSON Schema 与文字说明不一致、`file_action` 的批量 filter 选择器边界、Spotlight 查询翻译的可验证性、以及 stub backend 避免被误当成已实现后端。

同步阻塞 + `Vec<SearchResult>` 对原型期合理，不建议现在引入 async / streaming；但 trait 需要把“超时返回部分结果还是错误”说清楚。

## must-fix（v1.0 发布前必改）

### 1. 收紧机器 JSON Schema，避免“文档能表达、校验却放过错误”

引用：schema §5、§3、§4。

当前 JSON Schema 大量对象没有 `additionalProperties: false`，且 `FileSearch` / `FileAction` 等 defs 没有声明自身 `type: "object"`。这会导致拼错字段、错误 intent 附带无关字段、`destination` 缺失等情况被校验器放过，实际实现时 Harness 和 backend 会各自补防线。

建议 v1.0 前至少修：

- 所有 intent 分支和公共对象显式加 `type: "object"`。
- 除 `refine.delta` 的受控扩展点外，默认 `additionalProperties: false`。
- `file_action` 用条件校验约束：`copy` / `move` 必须有 `destination`，`rename` 必须有 `new_name`。
- `delete` 虽在枚举中保留，但 MVP 不开放；Schema 或 Policy 文档应明确运行时必须拒绝，而不是只靠说明文字。

### 2. 明确 `refine.delta` 对 list 字段的覆盖/追加/清空语义

引用：schema §3.4、§7.5 #31-#35。

文字只写“覆盖基准对应字段”，但多轮搜索中用户常说“再加上 pdf”“也包括 docx”“不要 png”。这些分别对应追加、覆盖、排除，不能全部靠模型猜成覆盖。

建议 v1.0 先采用保守规则：

- `delta.extensions` / `delta.keywords` 默认“覆盖”，与现文档一致。
- 追加语义暂不进入 v1.0；用户说“也包括”时 parser 生成完整覆盖后的列表。
- 清空语义必须有表达方式，否则无法处理“不要限制目录了”。可选做法是在 `delta` 中允许字段为 `null` 表示移除约束，或新增 `clear: ["location", "extensions"]`。
- `exclude_extensions` / `exclude_file_type` 只作为合并后 intent 的过滤条件，不应长期只存在于 refine 临时层；否则 backend 收到合并后的 intent 时无法表达排除条件。

### 3. `target_ref.selector.type=filter` 不宜作为 v1.0 写操作入口

引用：schema §4.6、§7.6 #39；STATUS 审阅重点 #6。

`filter` 对“把这些 pdf 复制到桌面”很自然，但对写操作过度灵活。它把“从上一轮结果中再筛选”与“执行批量文件操作”揉在一起，容易产生用户没看清目标集就批量复制/移动的风险。

建议 v1.0 改为：

- `file_action` 仅支持 `index` / `indices` / `all`。
- 用例 #39 改成 `clarify` 或二阶段流程：先 refine 出 pdf，再要求用户确认这些结果，随后用 `all` 执行 copy。
- `filter` 放到 v1.1，并限制 filter 字段白名单、最大命中数量、确认 UI 必须展示目标列表。

### 4. `clarify` 的触发边界需要写成可执行规则

引用：schema §3.5、§7.4 #27、§7.7 #41；STATUS 审阅重点 #4。

#27 “找最近的 budget pptx”不触发 clarify 是合理的，因为有关键词和类型约束，“最近的”可作为排序意图；#41 “找最近的”触发 clarify 也合理，因为缺少类型、关键词、位置等约束。

但这个边界目前只靠两个用例暗示。建议 v1.0 写明：

- 当“最近”仅修饰排序，且 intent 至少有一个强约束（关键词、扩展名、file_type、location、media_type）时，不触发 clarify，使用 `sort: "modified_desc"`。
- 当“最近”是唯一有效约束，或用户明确问“最近几天/最近一段时间”的时间窗口时，触发 `clarify(reason: "ambiguous_time")`。
- `relative_recent` 不必进 v1.0；它会让 parser 和 backend 都多一个模糊时间分支，原型期收益不大。

### 5. Spotlight 翻译规则需要区分文件名、内容、扩展名与路径范围

引用：trait §4.1、schema §7.1 #4/#7、STATUS 审阅重点 #9。

`keywords: ["X"]` 直接作为 `mdfind` 裸关键词能跑通原型，但语义不够稳定：它可能匹配内容、文件名和部分 metadata；而用例 #4 写的是“名字里有”。同时扩展名用 `kMDItemFSName == '*.ppt'cd` 可用性需要真实验证，建议优先用更明确的文件名通配或 content type tree 组合。

建议 v1.0 前在 trait 文档补充：

- `keywords` v1.0 默认是“文件名 + 内容 + metadata 的宽匹配”，不要在用例里写成“名字里有”除非 schema 增加 `keyword_scope` 或 `keyword_match_mode`。
- 中文关键词匹配必须统一使用 `cd` 修饰符的谓词形式；裸关键词是否覆盖中文文件名要进入真实 mdfind 验证清单。
- 扩展名匹配建议生成 `kMDItemFSName == "*.ppt"cd` 这类谓词，并在 shell 调用中严格避免 shell 展开。
- `file_type` 可先按扩展名展开；`kMDItemContentTypeTree` 作为 should-have 验证项，避免不同应用生成的 UTI 与扩展名不一致。

### 6. Stub backend 必须不能进入正常 fallback 链

引用：trait §5、§6；STATUS 审阅重点 #10。

`is_available() -> false` 的 stub 方向对，但还不够防“假阴性/假阳性”。如果 Harness 只看 `BackendKind` 枚举或误配置顺序，stub 可能让测试误以为 Windows / Everything 路径已有覆盖。

建议：

- Stub 类型命名显式带 `Stub`，如 `WindowsSearchStubBackend`，不要与未来真实类型同名。
- `kind()` 可返回真实 `BackendKind`，但增加 `implementation_status()` 或仅在测试 feature 下编译 stub。
- 集成测试断言：生产 backend 列表不能包含 stub；stub 只允许在 harness 单元测试中使用。

## should-have（v1.0.x patch 修）

### 7. `keyword_match_mode` 值得提前设计，但不必现在落地

引用：schema §7.1 #7、§8.2；STATUS 审阅重点 #3。

#7 “名字以会议纪要开头”目前降级为 `keywords` 是可接受的，但文档应承认这是有损表达。`keyword_match_mode` 应进入 v1.1，不建议塞进 v1.0，因为它会连带需要 `keyword_scope`（filename/content/all）和每个 keyword 的作用域，否则只能半实现。

v1.0.x 可以先把用例 #7 的注释改得更硬：原型只保证包含匹配，不保证前缀匹配。

### 8. 时间表达建议补“最近 N 天”可扩展形态

引用：schema §4.1、§8.2；STATUS 审阅重点 #3。

`last_3_days` / `last_7_days` / `last_14_days` / `last_30_days` 能覆盖首批用例，但用户会说“最近 10 天”“过去 2 小时”。v1.0 可以不支持小时级，但建议在 v1.1 设计时优先考虑：

```json
{ "type": "relative_duration", "value": 10, "unit": "day" }
```

`recent` + `confidence` 不建议作为时间字段主线；它更像 parser 置信度，不是搜索约束。

### 9. location hint 映射表需要标注“系统默认目录名不等于显示名”

引用：schema §4.3；STATUS 审阅重点 #5。

macOS 截屏默认在 `~/Desktop`，但用户可通过系统设置改位置；Windows 中文系统中资源管理器显示“桌面/下载/文档”，实际路径通常仍是 `%USERPROFILE%\Desktop` / `Downloads` / `Documents`。当前表基本方向正确，但应补充：

- Windows 不应硬编码中文目录名，应通过 Known Folders API 解析。
- macOS 截屏目录不能只靠 `~/Desktop`；可读取系统截图位置配置，失败再 fallback 到 `~/Desktop` 和 `~/Pictures/Screenshots`。
- `include` 一律由 resolver 生成，模型不直接生成绝对路径，除非用户明确输入路径。

### 10. 错误码最小集可以保留，但原型期建议增加 `UnsupportedIntent`

引用：trait §2.3；STATUS 审阅重点 #8。

5 条错误码作为 v0.1 足够简洁，但 `InvalidIntent` 同时承载“schema 无效”和“schema 有效但后端暂不支持”，会让 Harness 难以判断是 parser bug 还是 backend 能力不足。

建议增加：

- `UnsupportedIntent { detail: string }`：合法 intent 但当前 backend 不支持，例如 Spotlight 原型暂不支持部分媒体 metadata。

`FullDiskAccessRequired`、`EverythingNotRunning`、`SpotlightDirectoryExcluded` 可以等到 MVP，但需要在 `BackendUnavailable.reason` / `PermissionDenied.path` 中保留可诊断文本。

### 11. 明确同步 trait 的超时返回策略

引用：trait §3.1、§8；STATUS 审阅重点 #7。

同步阻塞 + `Vec<SearchResult>` 对技术原型合理，不建议现在上 async / streaming。原因是原型目标是验证 intent 和 mdfind 翻译，不是 UI 流式体验。

但现在文档同时写“尽力在时限内返回部分结果”和 `Result<Vec<_>, Timeout>`，语义冲突。建议 v0.1 选一种：

- 简单方案：超时即 `Err(SearchError::Timeout)`，不返回部分结果。
- 如需部分结果：引入 `SearchOutcome { results, partial, warnings }`，但这会扩大原型范围。

建议 v0.1 采用简单方案。

### 12. `SearchResult.id` 不应只写“路径 hash”

引用：trait §2.2。

路径会随 rename / move 变化，hash 也会变。原型可用 canonical path hash，但文档应明确稳定性级别：

- v0.1：`id` 是本次查询内稳定标识，可用规范化绝对路径 hash。
- MVP：macOS 可考虑 file system id / bookmark，Windows 可考虑 file reference number 或 Search index id。

## nice-to-have（v1.1 再说）

### 13. 增加 `keyword_scope` / `content_match`

引用：schema §8.2。

`content_match` 不必进 v1.0。Spotlight 原型可以宽匹配，但 Beta 做 Office/PDF 内容和 OCR 后，应区分：

- `filename`
- `content`
- `metadata`
- `ocr`
- `any`

这比单独加 `content_match: true` 更可扩展。

### 14. `person` 字段暂缓

引用：schema §8.2。

`person` 适合企业联系人、邮件、聊天记录等更高层语义，不适合当前“本地文件自然语言搜索”原型。建议等到数据源超出文件系统 metadata 后再设计。

### 15. 媒体质量 `quality` 需要后端能力发现后再严格化

引用：schema §3.2、§7.2 #14。

`lossless` 可先由扩展名启发式表达（flac/wav/aiff/ape 等），但不要承诺真实音频质量识别。MVP 后可由媒体 metadata reader 或索引器提供。

## out-of-scope（建议不做的理由）

### 16. v1.0 不做 async / streaming trait

引用：trait §3.1、§8。

现在引入 async 会提前绑定 tokio / async-trait / stream 类型，增加 workspace 和测试复杂度。原型期 `mdfind` 子进程 + `Vec` 足够验证核心风险。等 Tauri UI 和多后端并行查询出现后，再切 async stream 更自然。

### 17. v1.0 不做完整 Capability Discovery

引用：trait §1、§6。

原型只需静态后端顺序 + `is_available()`。真正的 capability matrix 会涉及平台权限、索引状态、内容搜索能力、媒体 metadata 能力、OCR 能力，适合 MVP 阶段统一设计。

## 对 STATUS.md 11 个审阅问题的直接答复

1. 字段覆盖常见查询模式基本充分；明显缺口是 keyword scope/match mode、清空 refine 约束、后端不支持能力表达。
2. `refine.delta` list 字段当前不够清晰；v1.0 应明确默认覆盖，追加由 parser 生成完整列表，清空需要新增表达。
3. `keyword_match_mode` 和 `relative_recent` 都不建议现在纳入 v1.0；前者等同 `keyword_scope` 一起做，后者建议改成未来的 `relative_duration` 或 clarify 规则。
4. #41 与 #27 的边界合理，但需写成规则：有强约束时“最近”作为排序；无强约束时 clarify。
5. location 映射表大方向正确；macOS 截屏目录要读系统配置并 fallback，Windows 要用 Known Folders API 而不是中文显示名。
6. `target_ref.selector.type=filter` 对 v1.0 写操作过度灵活，建议限制到 v1.1。
7. 原型期同步阻塞 + `Vec<SearchResult>` 合理；不用现在上 async / streaming。
8. 错误码略简，但只建议 v0.1 立即增 `UnsupportedIntent`；其他平台细分错误可等 MVP。
9. mdfind 规则需补中文 `cd` 验证、裸关键词语义、扩展名谓词、`kMDItemContentTypeTree` 的验证项。
10. Stub backend 有误判风险；应显式命名为 Stub、限制编译/注册范围，并加测试防止进入生产 fallback。
11. 追加 corner cases 见下节。

## 建议补充的 corner case 用例

### A. 清空上一轮位置约束（需要扩展 schema）

前置：上一轮 `location.hint = "下载"`。

用户：`不限制下载目录了`

当前 v1.0 无法明确表达“移除 location 约束”。建议扩展：

```json
{ "schema_version":"1.0","intent":"refine","language":"zh",
  "base_ref":"last_intent",
  "delta":{ "location": null } }
```

或：

```json
{ "schema_version":"1.0","intent":"refine","language":"zh",
  "base_ref":"last_intent",
  "clear":["location"] }
```

### B. 英文复数扩展名 + 大小写混合（v1.0 可表达）

用户：`find JPG and PNG screenshots from yesterday`

```json
{ "schema_version":"1.0","intent":"media_search","language":"en",
  "media_type":"screenshot",
  "extensions":["jpg","png"],
  "created_time":{"type":"relative","value":"yesterday"},
  "sort":"created_desc" }
```

### C. 排除某类文件（v1.0 refine 可表达，但合并后 intent 需支持）

前置：上一轮查找下载目录大文件。

用户：`排除压缩包`

```json
{ "schema_version":"1.0","intent":"refine","language":"zh",
  "base_ref":"last_intent",
  "delta":{ "exclude_file_type":["archive"] } }
```

### D. 路径 hint 未识别（v1.0 可表达，但 resolver 需 clarify 或降级）

用户：`找项目归档里的 budget pdf`

```json
{ "schema_version":"1.0","intent":"file_search","language":"zh",
  "keywords":["budget"],
  "extensions":["pdf"],
  "file_type":"document",
  "location":{"hint":"项目归档"},
  "sort":"modified_desc" }
```

实现建议：resolver 找不到明确目录时，不要静默把 `项目归档` 当路径；可作为关键词参与搜索，或触发 `ambiguous_location` clarify。

### E. 高风险批量操作（v1.0 应 clarify）

用户：`把这些都移动到桌面`

```json
{ "schema_version":"1.0","intent":"clarify","language":"zh",
  "reason":"ambiguous_action",
  "question":"要移动上一轮的全部结果吗？请先确认目标文件列表。",
  "options":["确认全部","只选择部分","取消"] }
```

理由：`move` 是 L4 写操作，即使 schema 能表达 `selector.type = "all"`，parser 也不应在未展示目标列表前直接进入 `file_action`。
