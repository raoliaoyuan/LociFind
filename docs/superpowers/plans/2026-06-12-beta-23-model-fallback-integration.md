# BETA-23 模型 fallback 接入桌面搜索默认流程 — 实现计划

> 执行完成（2026-06-13）：Task 1-12 全部 done；执行期偏差与验证发现见 ROADMAP BETA-23/BETA-24 卡片。

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把已就绪的 hybrid 模型 fallback（Qwen3-0.6B GGUF + llama-cpp）接进桌面搜索默认流程，并扩展触发器使「内容词遗漏」（真机问题 4）能触发模型补全。

**Architecture:** 方案 A——intent-parser 只加纯函数（内容词覆盖检测、apply_patch keywords 并集、prompt few-shot）；desktop 新模块 `search/model_fallback.rs` 做编排（懒加载状态机 + 同步等待 3s 超时 + 失败回落 parser，永不让搜索失败）；llama-cpp 经非默认 cargo feature `model-fallback` 隔离，日常 workspace 构建不编 C++。

**Tech Stack:** Rust（intent-parser / model-runtime / Tauri desktop）+ React/TS 前端 + evals 评测。

**Spec:** [docs/superpowers/specs/2026-06-12-beta-23-model-fallback-integration-design.md](../specs/2026-06-12-beta-23-model-fallback-integration-design.md)

**分支：** `feat-beta-23-model-fallback`（从 main 切出）

**每个 task 的统一验证门（缺一不可，按 memory 纪律含 fmt）：**
```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
（platform-macos 在 Windows 上的 2 个预存失败除外；本会话在 macOS 上应全绿。）

---

### Task 1: intent-parser — 暴露 `residual_content_segments`

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs`

背景：G1 的跨度剥离抽取器（`extract_zh_residual_keywords` / `extract_en_residual_keywords` / `merge_mixed_keywords`）是私有函数，且 `extract_filesearch_keywords` 的「文件名包含 X」等结构会**短路 return**（问题 4 丢词根因）。触发器需要一个**不走短路**的内容段抽取入口。

- [ ] **Step 1: 写失败测试**（加在 `file_search.rs` 的 `#[cfg(test)] mod tests` 内）

```rust
#[test]
fn residual_segments_cover_problem4_query() {
    // 问题 4：「文件名包含运维」短路丢掉「会议纪要」——残留段抽取必须能看到它
    let segs = residual_content_segments("2025年的会议纪要文件名包含运维");
    assert!(
        segs.iter().any(|s| s.contains("会议纪要")),
        "segs={segs:?}"
    );
}

#[test]
fn residual_segments_empty_for_pure_signal_query() {
    // 纯信号词查询（时间+类型）无内容残留段
    let segs = residual_content_segments("上周的pdf");
    assert!(
        segs.iter().all(|s| !s.chars().any(|c| is_cjk(c))),
        "segs={segs:?}"
    );
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-intent-parser residual_segments -- --nocapture
```
预期：编译失败（`residual_content_segments` 未定义）。

- [ ] **Step 3: 实现**（加在 `extract_filesearch_keywords` 附近）

```rust
/// BETA-23：内容残留段抽取——给 fallback 触发器做「内容词覆盖检测」。
///
/// 复用 G1 跨度剥离逻辑（zh / en / mixed 三形态），**刻意不走**
/// `extract_filesearch_keywords` 的「文件名包含 X」「」等短路结构——短路正是
/// 丢词根因，触发器需要看到 query 的全部内容段。
pub(crate) fn residual_content_segments(input: &str) -> Vec<String> {
    if !contains_chinese(input) {
        return extract_en_residual_keywords(input).unwrap_or_default();
    }
    if is_mixed_input(input) {
        if let Some(kws) = merge_mixed_keywords(input) {
            return kws;
        }
    }
    extract_zh_residual_keywords(input).unwrap_or_default()
}
```

同时把后续 task 需要的两个项提为 `pub(crate)`：
- `ZH_CONTAINER_NOUNS`（const）
- `is_cjk`（fn；若它实际定义在 `parsers/common.rs`，对实际位置做 `pub(crate)`，测试与 Task 2 引用处随之对齐）

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p locifind-intent-parser residual_segments
```
预期：2 passed。

- [ ] **Step 5: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p locifind-intent-parser
git add -A && git commit -m "feat(parser): 暴露 residual_content_segments 内容残留段抽取（BETA-23 触发器前置）"
```

---

### Task 2: intent-parser — 内容词覆盖检测 + `keywords` 结构性遗漏

**Files:**
- Modify: `packages/intent-parser/src/fallback.rs`
- Modify: `packages/intent-parser/src/hybrid.rs`（`IntentDraft::from_query` 适配 `ParseResult` 新字段）

- [ ] **Step 1: 写失败测试**（`fallback.rs` 测试模块；若无则新建 `#[cfg(test)] mod tests`）

```rust
#[test]
fn problem4_compound_query_triggers_keywords_omission() {
    let parsed = parse_with_signals("2025年的会议纪要文件名包含运维");
    let missing = analyze_structural_omissions(&parsed);
    assert!(missing.contains(&"keywords"), "missing={missing:?}");
    assert!(matches!(
        should_invoke_model(&parsed),
        FallbackDecision::InvokeModel(FallbackReason::StructuralOmission { .. })
    ));
}

#[test]
fn covered_queries_do_not_trigger_keywords_omission() {
    // parser 已全覆盖的查询不得误触发（反例集）
    for q in [
        "上周的pdf",
        "找最大的文件",
        "周杰伦的歌",
        "find the annual budget report",
        "找名字里有「预算」的文件",
    ] {
        let parsed = parse_with_signals(q);
        let missing = analyze_structural_omissions(&parsed);
        assert!(
            !missing.contains(&"keywords"),
            "query={q} missing={missing:?}"
        );
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-intent-parser keywords_omission
```
预期：`problem4_...` FAIL（missing 不含 "keywords"）。

- [ ] **Step 3: 实现**

3a. `ParseResult` 增加 query 字段（检测需要原始 query；`parse_with_signals` 有）：

```rust
pub struct ParseResult {
    /// 原始查询（BETA-23：内容词覆盖检测需要）。
    pub query: String,
    pub intent: SearchIntent,
    pub signals: CandidateSignals,
}

pub fn parse_with_signals(query: &str) -> ParseResult {
    let intent = crate::parse(query);
    let signals = scan(query);
    ParseResult {
        query: query.to_owned(),
        intent,
        signals,
    }
}
```

编译器会指出所有手工构造 `ParseResult` 的调用点（本 crate 测试、evals、`hybrid.rs`）——逐一补 `query` 字段。

3b. 覆盖检测（`fallback.rs` 新增私有函数）：

```rust
/// BETA-23：命名结构噪声词——覆盖检测时从残留段中剥离（不算未覆盖内容）。
const COVERAGE_NOISE_WORDS: &[&str] = &["文件名", "名字", "名称"];

/// intent 中「已覆盖」的内容值：keywords / extensions / artist / album / title /
/// genre / location.hint。残留段中被这些值吃掉的部分不算遗漏。
fn covered_values(intent: &SearchIntent) -> Vec<String> {
    let mut vals: Vec<String> = Vec::new();
    let mut push_list = |list: &Option<Vec<String>>| {
        if let Some(items) = list {
            vals.extend(items.iter().cloned());
        }
    };
    match intent {
        SearchIntent::FileSearch(fs) => {
            push_list(&fs.keywords);
            push_list(&fs.extensions);
            if let Some(loc) = &fs.location {
                vals.push(loc.hint.clone());
            }
        }
        SearchIntent::MediaSearch(ms) => {
            push_list(&ms.keywords);
            push_list(&ms.extensions);
            for v in [&ms.artist, &ms.album, &ms.title, &ms.genre] {
                if let Some(s) = v {
                    vals.push(s.clone());
                }
            }
            if let Some(loc) = &ms.location {
                vals.push(loc.hint.clone());
            }
        }
        _ => {}
    }
    vals
}

/// 残留段去掉已覆盖值与噪声词后，是否仍含「实质内容」（≥2 连续 CJK 或 ≥3 字母英文词）。
fn has_uncovered_content(query: &str, intent: &SearchIntent) -> bool {
    let covered: Vec<String> = covered_values(intent)
        .into_iter()
        .map(|v| v.to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    for seg in crate::parsers::file_search::residual_content_segments(query) {
        let mut s = seg.to_lowercase();
        for c in &covered {
            s = s.replace(c.as_str(), " ");
        }
        for w in COVERAGE_NOISE_WORDS {
            s = s.replace(w, " ");
        }
        for w in crate::parsers::file_search::ZH_CONTAINER_NOUNS {
            s = s.replace(w, " ");
        }
        if has_content_run(&s) {
            return true;
        }
    }
    false
}

/// ≥2 连续 CJK 字符，或 ≥3 连续 ASCII 字母 → 视为实质内容。
fn has_content_run(s: &str) -> bool {
    let mut cjk_run = 0usize;
    let mut ascii_run = 0usize;
    for c in s.chars() {
        if crate::parsers::file_search::is_cjk(c) {
            cjk_run += 1;
            ascii_run = 0;
            if cjk_run >= 2 {
                return true;
            }
        } else if c.is_ascii_alphabetic() {
            ascii_run += 1;
            cjk_run = 0;
            if ascii_run >= 3 {
                return true;
            }
        } else {
            cjk_run = 0;
            ascii_run = 0;
        }
    }
    false
}
```

（`is_cjk` 的实际路径以 Task 1 的落点为准。）

3c. `analyze_structural_omissions` 的 FileSearch / MediaSearch 两臂**各**追加（放在该臂现有检查之后）：

```rust
if has_uncovered_content(&parsed.query, &parsed.intent) {
    missing.push("keywords");
}
```

3d. `hybrid.rs::IntentDraft::from_query` 因 `ParseResult` 新字段需要解构适配（保持行为不变）。

- [ ] **Step 4: 跑测试；反例失败时收紧词表而非放松断言**

```bash
cargo test -p locifind-intent-parser
```
预期：全过。**如果反例集中某条误触发**：把误报词加进 `COVERAGE_NOISE_WORDS`（或确认它属于 G1 strip 词表的缺口、补到对应 strip 表），**不许删反例**。

- [ ] **Step 5: parser-only byte-equal 硬门**

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9
```
预期：v0.5 = 473 pass / v0.9 = 726 pass，与 main 完全一致（`parse()` 零改动的机械验证）。

- [ ] **Step 6: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A && git commit -m "feat(parser): 触发器扩展——内容词覆盖检测，keywords 遗漏可触发模型 fallback（修问题4 前半）"
```

---

### Task 3: intent-parser — `apply_patch` keywords 并集语义

**Files:**
- Modify: `packages/intent-parser/src/hybrid.rs`

- [ ] **Step 1: 写失败测试**（hybrid.rs tests；`mk_bare_file_search` 已有 keywords `["ppt"]`）

```rust
#[test]
fn apply_patch_unions_keywords_with_draft() {
    // 模型补 keywords 不许丢 parser 已抽对的词（问题 4 的「运维」场景）
    let draft = IntentDraft {
        intent: mk_bare_file_search(),
        signals: CandidateSignals::default(),
        fillable_fields: vec!["keywords"],
    };
    let patch = json!({"keywords": ["会议纪要"]});
    let merged = apply_patch(&draft, &patch).unwrap();
    let SearchIntent::FileSearch(fs) = merged else {
        unreachable!()
    };
    assert_eq!(
        fs.keywords,
        Some(vec!["ppt".to_owned(), "会议纪要".to_owned()])
    );
}

#[test]
fn apply_patch_keywords_union_dedups() {
    let draft = IntentDraft {
        intent: mk_bare_file_search(),
        signals: CandidateSignals::default(),
        fillable_fields: vec!["keywords"],
    };
    let patch = json!({"keywords": ["ppt", "预算"]});
    let merged = apply_patch(&draft, &patch).unwrap();
    let SearchIntent::FileSearch(fs) = merged else {
        unreachable!()
    };
    assert_eq!(fs.keywords, Some(vec!["ppt".to_owned(), "预算".to_owned()]));
}

#[test]
fn apply_patch_keywords_null_keeps_overwrite_semantics() {
    // 显式 null（模型表示无值）维持原覆盖语义
    let draft = IntentDraft {
        intent: mk_bare_file_search(),
        signals: CandidateSignals::default(),
        fillable_fields: vec![],
    };
    let patch = json!({"keywords": null});
    let merged = apply_patch(&draft, &patch).unwrap();
    let SearchIntent::FileSearch(fs) = merged else {
        unreachable!()
    };
    assert_eq!(fs.keywords, None);
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-intent-parser apply_patch
```
预期：`apply_patch_unions_keywords_with_draft` FAIL（现为覆盖语义，得 `["会议纪要"]`）。

- [ ] **Step 3: 实现** —— `apply_patch` 的 for 循环中、`intent`/`schema_version` 跳过之后加特例：

```rust
for (key, val) in patch_obj {
    if key == "intent" || key == "schema_version" {
        continue;
    }
    // BETA-23：keywords 取并集（draft 在前去重）——parser 已确定的词不许被模型推翻，
    // 与 variant 锁死同一哲学。其余字段维持覆盖语义。
    if key == "keywords" {
        let merged_kw = union_keywords(intent_obj.get("keywords"), val);
        intent_obj.insert(key.clone(), merged_kw);
        continue;
    }
    intent_obj.insert(key.clone(), val.clone());
}
```

```rust
/// keywords 并集：draft 现有值在前、patch 新值追加、字符串去重。
/// patch 不是字符串数组（如显式 null）时返回 patch 原值（维持覆盖语义）。
fn union_keywords(existing: Option<&Value>, patch: &Value) -> Value {
    fn collect(v: &Value, out: &mut Vec<String>) {
        if let Some(arr) = v.as_array() {
            for item in arr {
                if let Some(s) = item.as_str() {
                    if !out.iter().any(|e| e == s) {
                        out.push(s.to_owned());
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some(e) = existing {
        collect(e, &mut out);
    }
    collect(patch, &mut out);
    if out.is_empty() {
        return patch.clone();
    }
    Value::Array(out.into_iter().map(Value::String).collect())
}
```

- [ ] **Step 4: 跑测试确认通过**

```bash
cargo test -p locifind-intent-parser apply_patch
```
预期：全过（含既有 apply_patch 测试零回归）。

- [ ] **Step 5: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p locifind-intent-parser
git add -A && git commit -m "feat(parser): apply_patch keywords 并集语义——模型补词不丢 parser 已抽对的词"
```

---

### Task 4: intent-parser — hybrid prompt 补 keywords few-shot

**Files:**
- Modify: `packages/intent-parser/src/hybrid.rs`（`hybrid_prompt_prefix`）

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn prefix_teaches_keywords_completion() {
    let p = hybrid_prompt_prefix();
    assert!(p.contains("\"keywords\""));
    assert!(p.contains("会议纪要"));
    // 既有不变量：前缀仍以固定结尾收口（KV 缓存正确性前提）
    assert!(p.ends_with("# 现在请处理\n\n"));
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-intent-parser prefix_teaches
```
预期：FAIL（前缀目前无 keywords 示例）。

- [ ] **Step 3: 实现** —— 在 `hybrid_prompt_prefix` 的 raw string 里：

3a. 「字段值速查」段追加一行：

```text
- keywords: 字符串数组；只补 query 里出现、但 Draft.keywords 缺失的内容词，已有词不要重复
```

3b. 「# 示例」段、`找张学友的歌` 示例之后追加（保持 `# 现在请处理` 收尾不动）：

```text
Query: "2025年的会议纪要文件名包含运维"
当前 Draft: {"schema_version":"1.0","intent":"file_search","language":"zh","keywords":["运维"],"modified_time":{"type":"absolute","from":"2025-01-01","to":"2025-12-31"},"sort":"modified_desc"}
待填字段: keywords
Patch:
{"keywords":["会议纪要"]}
```

- [ ] **Step 4: 跑测试确认通过**（含既有 `prefix_is_query_independent` / `prefix_plus_suffix_equals_full_prompt` 零回归）

```bash
cargo test -p locifind-intent-parser hybrid
```

- [ ] **Step 5: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p locifind-intent-parser
git add -A && git commit -m "feat(parser): hybrid prompt 补 keywords 字段速查与 few-shot 示例"
```

---

### Task 5: model-runtime — `ModelDaemon::from_runtime` 构造器

**Files:**
- Modify: `packages/model-runtime/src/daemon.rs`

desktop 状态机单测需要注入「返回指定 patch JSON 的假 runtime」；现状 daemon 只能经 loader 构造。

- [ ] **Step 1: 写失败测试**（daemon.rs tests）

```rust
#[test]
fn test_from_runtime_is_ready() {
    struct Fixed;
    impl crate::LlamaModelRuntime for Fixed {
        fn generate(
            &self,
            _prompt: &str,
            _params: &GenerateParams,
        ) -> Result<String, crate::ModelError> {
            Ok("{\"sort\":\"size_desc\"}".to_owned())
        }
    }
    let daemon = ModelDaemon::from_runtime(Box::new(Fixed));
    assert_eq!(daemon.status(), DaemonStatus::Ready);
    let out = daemon.generate("p", &GenerateParams::default()).unwrap();
    assert!(out.contains("size_desc"));
}
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cargo test -p locifind-model-runtime from_runtime
```
预期：编译失败（方法不存在）。

- [ ] **Step 3: 实现**（daemon.rs，`load_blocking` 旁）

```rust
/// 从已构造的 runtime 直接组 daemon（测试注入 / 自定义后端用），状态即 Ready。
#[must_use]
pub fn from_runtime(runtime: Box<dyn LlamaModelRuntime>) -> Self {
    Self {
        runtime,
        status: DaemonStatus::Ready,
    }
}
```

- [ ] **Step 4: 跑测试确认通过 + 统一验证门 + commit**

```bash
cargo test -p locifind-model-runtime
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings
git add -A && git commit -m "feat(model-runtime): ModelDaemon::from_runtime 直构 daemon（测试注入用）"
```

---

### Task 6: desktop — cargo feature + `search/model_fallback.rs` 编排状态机

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/src/search/model_fallback.rs`
- Modify: `apps/desktop/src-tauri/src/search.rs`（mod 声明 + `SearchEvent::ModelThinking` 变体 + SearchDeps 字段）

- [ ] **Step 1: Cargo.toml 改动**

```toml
# [dependencies] 增加（model-runtime 本就是 intent-parser 的传递依赖，直依赖零额外成本）：
locifind-model-runtime = { path = "../../../packages/model-runtime" }

# tokio 既有行加 "time"（超时用）：
tokio = { version = "1", features = ["rt", "macros", "time"] }

# 文件尾新增：
[features]
# BETA-23：真实模型推理（llama-cpp 后端，构建需 cmake）。日常开发/CI 不开，
# Release 构建与真机验证开。关闭时模型 fallback 静默降级为 parser-only。
model-fallback = ["locifind-model-runtime/llama-cpp"]
# macOS 真机：metal 加速。
model-fallback-metal = ["model-fallback", "locifind-model-runtime/metal"]
```

- [ ] **Step 2: search.rs 加事件变体与 mod 声明**

`SearchEvent` enum 追加：

```rust
/// BETA-23：已触发模型 fallback，正在等待模型补全（约 1s）。UI 显示轻量提示。
ModelThinking,
```

mod 区追加 `pub(crate) mod model_fallback;`（不进 `pub(crate) use`——按 CLEAN-1 教训，跨模块以 `model_fallback::` 路径引用）。

`SearchDeps` 加字段与访问器（模式同 `audit`）：

```rust
/// BETA-23 模型 fallback 编排句柄。`new()` 默认 disabled（测试零行为变化），
/// main.rs 经 [`with_model`](Self::with_model) 注入真句柄。
model: Arc<model_fallback::ModelFallbackHandle>,
```

`new()` 初始化：`model: Arc::new(model_fallback::ModelFallbackHandle::disabled("未初始化"))`；builder 与访问器：

```rust
/// 注入模型 fallback 句柄（main.rs 用；测试可注入 stub 后断言）。
#[must_use]
pub fn with_model(mut self, model: Arc<model_fallback::ModelFallbackHandle>) -> Self {
    self.model = model;
    self
}

pub(crate) fn model(&self) -> &Arc<model_fallback::ModelFallbackHandle> {
    &self.model
}
```

- [ ] **Step 3: 写失败测试**（新建 `model_fallback.rs` 尾部 `#[cfg(test)] mod tests`；Channel 构造方式与 `search/tests.rs` 既有用法对齐）

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use locifind_intent_parser::fallback::IntentSource;
    use locifind_model_runtime::{GenerateParams, LlamaModelRuntime, ModelDaemon, ModelError};
    use std::time::Duration;

    struct FakeRuntime {
        output: String,
        delay: Duration,
    }
    impl LlamaModelRuntime for FakeRuntime {
        fn generate(&self, _p: &str, _params: &GenerateParams) -> Result<String, ModelError> {
            std::thread::sleep(self.delay);
            Ok(self.output.clone())
        }
    }

    fn ready_handle(output: &str, delay: Duration, timeout: Duration) -> Arc<ModelFallbackHandle> {
        let daemon = Arc::new(ModelDaemon::from_runtime(Box::new(FakeRuntime {
            output: output.to_owned(),
            delay,
        })));
        let fb = locifind_intent_parser::fallback::ModelFallback::new(daemon).with_hybrid_mode();
        ModelFallbackHandle::with_ready_for_test(fb, timeout)
    }

    fn drop_channel() -> tauri::ipc::Channel<crate::search::SearchEvent> {
        tauri::ipc::Channel::new(|_msg| Ok(()))
    }

    // 触发查询：问题 4（Task 2 后必触发 keywords 遗漏）
    const TRIGGER_QUERY: &str = "2025年的会议纪要文件名包含运维";

    #[test]
    fn use_parser_query_never_touches_model() {
        let handle = Arc::new(ModelFallbackHandle::disabled("test"));
        let r = tauri::async_runtime::block_on(resolve_with_model(
            "上周的pdf",
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::Parser));
    }

    #[test]
    fn disabled_handle_falls_back_to_parser() {
        let handle = Arc::new(ModelFallbackHandle::disabled("test"));
        let r = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::ParserNoFallback));
        // parser 自己的字段保留
        let locifind_search_backend::SearchIntent::FileSearch(fs) = &r.intent else {
            panic!("expected FileSearch");
        };
        assert_eq!(fs.keywords.as_deref(), Some(&["运维".to_owned()][..]));
    }

    #[test]
    fn ready_model_patches_keywords() {
        let handle = ready_handle(
            r#"{"keywords":["会议纪要"]}"#,
            Duration::ZERO,
            Duration::from_secs(3),
        );
        let r = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::Model));
        let locifind_search_backend::SearchIntent::FileSearch(fs) = &r.intent else {
            panic!("expected FileSearch");
        };
        // 并集：parser 的「运维」在前，模型补的「会议纪要」在后
        assert_eq!(
            fs.keywords,
            Some(vec!["运维".to_owned(), "会议纪要".to_owned()])
        );
    }

    #[test]
    fn garbage_model_output_falls_back_to_parser() {
        let handle = ready_handle("not json at all", Duration::ZERO, Duration::from_secs(3));
        let r = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::ParserNoFallback));
    }

    #[test]
    fn slow_model_times_out_to_parser() {
        let handle = ready_handle(
            r#"{"keywords":["会议纪要"]}"#,
            Duration::from_millis(500),
            Duration::from_millis(50),
        );
        let r = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::ParserNoFallback));
        // 超时后 busy 仍被被弃线程持有 → 立即再查询走单飞跳过
        let r2 = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r2.source, IntentSource::ParserNoFallback));
    }

    #[test]
    fn busy_guard_skips_model() {
        let handle = ready_handle(
            r#"{"keywords":["会议纪要"]}"#,
            Duration::ZERO,
            Duration::from_secs(3),
        );
        handle.busy.store(true, std::sync::atomic::Ordering::Release);
        let r = tauri::async_runtime::block_on(resolve_with_model(
            TRIGGER_QUERY,
            &handle,
            &drop_channel(),
        ));
        assert!(matches!(r.source, IntentSource::ParserNoFallback));
    }
}
```

- [ ] **Step 4: 跑测试确认失败**

```bash
cargo test -p locifind-desktop model_fallback
```
预期：编译失败（模块尚无实现）。

- [ ] **Step 5: 实现 `model_fallback.rs` 主体**

```rust
//! BETA-23：模型 fallback 编排——懒加载状态机 + 同步等待超时 + 失败回落 parser。
//!
//! 硬约束：任何路径下搜索不因模型而失败。模型只可能把 parser 的 intent 变得更全，
//! 不可能让查询出错（失败/超时/未加载/占用 → 一律回落 parser intent 继续）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use locifind_intent_parser::fallback::{
    parse_with_signals, should_invoke_model, FallbackDecision, IntentSource, ModelFallback,
    ResolvedIntent,
};
use tauri::ipc::Channel;

use super::SearchEvent;

/// 推理同步等待上限。超时回落 parser（被弃线程自然结束，busy 标志挡后续并发）。
const INFER_TIMEOUT: Duration = Duration::from_secs(3);

/// 默认模型文件名（BETA-17 胜出者）。
const DEFAULT_MODEL_FILE: &str = "qwen3-0.6b-q4_k_m.gguf";

/// 模型生命周期状态。
enum ModelState {
    /// 初始；首次触发时探测文件并转 Loading（文件缺失则保持 NotLoaded 下次再探测）。
    NotLoaded,
    /// 后台 load_blocking 进行中。
    Loading,
    /// 常驻就绪。
    Ready(Arc<ModelFallback>),
    /// 加载失败（文件损坏等），不再重试（设置页可见原因）。
    Failed(String),
    /// 模型不可用（feature 关 / 显式禁用），不参与触发。
    Unavailable(String),
}

/// 编排句柄：挂 SearchDeps，进程级单例。
pub struct ModelFallbackHandle {
    state: Mutex<ModelState>,
    /// 推理单飞守卫：true = 有 in-flight 推理（含已超时被弃但仍在跑的）。
    pub(crate) busy: AtomicBool,
    /// settings.json 路径（每次触发读 enable 开关；None = 默认开启）。
    settings_path: Option<PathBuf>,
    /// 默认模型路径（app 数据目录 models/）。
    default_model_path: PathBuf,
    /// 推理超时（测试可缩短）。
    infer_timeout: Duration,
}

impl std::fmt::Debug for ModelFallbackHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModelFallbackHandle").finish_non_exhaustive()
    }
}

impl ModelFallbackHandle {
    /// 真句柄（main.rs setup 注入）。`data_dir` 即 LociFind 数据目录（与 index.db 同级）。
    #[must_use]
    pub fn new(settings_path: Option<PathBuf>, data_dir: PathBuf) -> Self {
        Self {
            state: Mutex::new(ModelState::NotLoaded),
            busy: AtomicBool::new(false),
            settings_path,
            default_model_path: data_dir.join("models").join(DEFAULT_MODEL_FILE),
            infer_timeout: INFER_TIMEOUT,
        }
    }

    /// 固定不可用句柄（SearchDeps::new 默认 / 测试）。
    #[must_use]
    pub fn disabled(reason: &str) -> Self {
        Self {
            state: Mutex::new(ModelState::Unavailable(reason.to_owned())),
            busy: AtomicBool::new(false),
            settings_path: None,
            default_model_path: PathBuf::new(),
            infer_timeout: INFER_TIMEOUT,
        }
    }

    /// 测试用：直接 Ready + 自定义超时。
    #[cfg(test)]
    pub(crate) fn with_ready_for_test(fb: ModelFallback, infer_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(ModelState::Ready(Arc::new(fb))),
            busy: AtomicBool::new(false),
            settings_path: None,
            default_model_path: PathBuf::new(),
            infer_timeout,
        })
    }

    /// settings.json 的 enable_model_fallback（缺文件/损坏 → 默认 true，与 AppSettings::default 一致）。
    fn enabled_in_settings(&self) -> bool {
        let Some(path) = &self.settings_path else {
            return true;
        };
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str::<crate::settings::AppSettings>(&s)
                .map(|v| v.enable_model_fallback)
                .unwrap_or(true),
            Err(_) => true,
        }
    }

    /// 当前生效的模型路径（Task 8 起支持 settings.model_path 覆盖）。
    fn resolved_model_path(&self) -> PathBuf {
        self.default_model_path.clone()
    }

    /// 就绪则给 fallback；NotLoaded 且文件存在则踢一次后台加载（本次返回 None）。
    fn ready_or_kick_load(self: &Arc<Self>) -> Option<Arc<ModelFallback>> {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*st {
            ModelState::Ready(fb) => return Some(Arc::clone(fb)),
            ModelState::Loading | ModelState::Failed(_) | ModelState::Unavailable(_) => {
                return None;
            }
            ModelState::NotLoaded => {}
        }
        if !cfg!(feature = "model-fallback") {
            *st = ModelState::Unavailable(
                "本构建不含模型支持（feature model-fallback 未开启）".to_owned(),
            );
            return None;
        }
        let model_path = self.resolved_model_path();
        if !model_path.exists() {
            // 不锁死：用户放好文件后下次触发即可加载
            return None;
        }
        *st = ModelState::Loading;
        drop(st);
        let this = Arc::clone(self);
        tauri::async_runtime::spawn_blocking(move || {
            let params = locifind_model_runtime::ModelLoadParams {
                gpu_layers: 99,
                context_size: 4096,
            };
            let loaded = locifind_model_runtime::ModelDaemon::load_blocking(&model_path, params);
            let mut st = this.state.lock().unwrap_or_else(|e| e.into_inner());
            *st = match loaded {
                Ok(daemon) => ModelState::Ready(Arc::new(
                    ModelFallback::new(Arc::new(daemon)).with_hybrid_mode(),
                )),
                Err(err) => {
                    eprintln!("model fallback: 模型加载失败: {err}");
                    ModelState::Failed(err.to_string())
                }
            };
        });
        None
    }
}

/// search_impl 的 intent 解析入口：parser 优先，必要且可行时模型 hybrid 补全。**永不失败**。
pub(crate) async fn resolve_with_model(
    query: &str,
    handle: &Arc<ModelFallbackHandle>,
    on_event: &Channel<SearchEvent>,
) -> ResolvedIntent {
    let parsed = parse_with_signals(query);
    let decision = should_invoke_model(&parsed);

    if matches!(decision, FallbackDecision::UseParser) {
        return ResolvedIntent {
            intent: parsed.intent,
            source: IntentSource::Parser,
            decision,
            signals: parsed.signals,
        };
    }

    let parser_fallback = |reason: &str| {
        if !reason.is_empty() {
            eprintln!("model fallback: {reason}，回落 parser");
        }
        ResolvedIntent {
            intent: parsed.intent.clone(),
            source: IntentSource::ParserNoFallback,
            decision: decision.clone(),
            signals: parsed.signals.clone(),
        }
    };

    if !handle.enabled_in_settings() {
        return parser_fallback("");
    }
    let Some(fb) = handle.ready_or_kick_load() else {
        return parser_fallback("");
    };
    if handle
        .busy
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return parser_fallback("推理占用中");
    }

    let _ = on_event.send(SearchEvent::ModelThinking);
    let q = query.to_owned();
    let h = Arc::clone(handle);
    let join = tauri::async_runtime::spawn_blocking(move || {
        let out = fb.invoke(&q);
        h.busy.store(false, Ordering::Release);
        out
    });
    match tokio::time::timeout(handle.infer_timeout, join).await {
        Ok(Ok(Ok(intent))) => ResolvedIntent {
            intent,
            source: IntentSource::Model,
            decision,
            signals: parsed.signals,
        },
        Ok(Ok(Err(err))) => parser_fallback(&format!("推理失败: {err}")),
        Ok(Err(join_err)) => parser_fallback(&format!("推理任务异常: {join_err}")),
        Err(_) => parser_fallback(&format!("推理超时({:?})", handle.infer_timeout)),
    }
}
```

注意：`parser_fallback` 闭包借用 `parsed`/`decision`，而成功臂消费它们——若 borrow checker 冲突，把闭包改为接收引用的私有 fn 或在成功臂前先 `let signals = parsed.signals.clone()`，以编译为准、语义不变。

- [ ] **Step 6: 跑测试确认通过**

```bash
cargo test -p locifind-desktop model_fallback
```
预期：6 passed（默认 feature 关、走 stub 类型即可，全部不需要真模型）。

- [ ] **Step 7: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A && git commit -m "feat(desktop): model_fallback 编排状态机——懒加载/单飞/超时/永不失败（feature model-fallback 隔离 llama-cpp）"
```

---

### Task 7: desktop — search_impl 接线 + 前端 ModelThinking

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs:195-211`（替换 step 1 块）
- Modify: `apps/desktop/src-tauri/src/main.rs`（setup 注入 with_model）
- Modify: `apps/desktop/src/SearchView.tsx`（事件类型 + 提示 UI）

- [ ] **Step 1: search.rs 替换 intent 解析块**

删除 `use locifind_intent_parser::fallback::{resolve_intent, IntentSource};` 中的 `resolve_intent`（保留 `IntentSource`），把 search_impl 的 step 1（原 `let resolved = match resolve_intent(&query, None) {...}` 整块）替换为：

```rust
// 1) NL → intent（BETA-23：parser 优先；结构性遗漏且模型可用时 hybrid 补全；永不失败）
let resolved = model_fallback::resolve_with_model(&query, deps.model(), &on_event).await;
let locifind_intent_parser::fallback::ResolvedIntent {
    intent,
    source,
    signals,
    ..
} = resolved;
```

（原 Err 分支整体删除——新入口不返回 Result。）

- [ ] **Step 2: main.rs setup 注入真句柄**（`with_audit(audit)` 之后链式追加）

```rust
.with_model(std::sync::Arc::new(
    search::model_fallback::ModelFallbackHandle::new(
        settings::settings_file_path(&app.handle().clone()),
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("LociFind"),
    ),
))
```

- [ ] **Step 3: 前端 SearchView.tsx**

3a. `SearchEvent` 联合类型追加：

```ts
| { event: "model_thinking" }
```

3b. 组件内加状态（与 `switchNotes` 同区）：

```ts
const [modelThinking, setModelThinking] = useState(false);
```

`runSearch` 重置区（`setSwitchNotes([])` 旁）加 `setModelThinking(false);`。

3c. `onmessage` switch 追加 case；`started` / `complete` / `error` 三个 case 里各加 `setModelThinking(false);`：

```ts
case "model_thinking": {
  setModelThinking(true);
  break;
}
```

3d. 渲染（switchNotes 提示条同位置）：

```tsx
{modelThinking && <div className="switch-note">正在理解查询…</div>}
```

（className 与 switchNotes 现用样式一致即可，无新 CSS。）

- [ ] **Step 4: 验证（后端零回归 + 前端构建）**

```bash
cargo test -p locifind-desktop
cd apps/desktop && npx tsc --noEmit && npm run build && cd ../..
```
预期：desktop 单测全过（SearchDeps::new 默认 disabled 句柄 → 既有 search_impl 测试行为不变）；tsc/vite 零错误。

- [ ] **Step 5: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A && git commit -m "feat(desktop): search_impl 接入模型 fallback + 前端「正在理解查询」提示（修问题4 全链路）"
```

---

### Task 8: desktop — settings `model_path` + `get_model_status` 命令 + 设置页

**Files:**
- Modify: `apps/desktop/src-tauri/src/settings.rs`
- Modify: `apps/desktop/src-tauri/src/search/model_fallback.rs`（路径覆盖 + 状态快照）
- Modify: `apps/desktop/src-tauri/src/search.rs`（命令薄包装，`#[tauri::command]` 必须在 search.rs 根——CLEAN-1 教训）
- Modify: `apps/desktop/src-tauri/src/main.rs`（注册命令）
- Modify: `apps/desktop/src/pages/SettingsPage.tsx`

- [ ] **Step 1: settings.rs 加字段**（向后兼容必须 `#[serde(default)]`）

```rust
/// BETA-23：模型文件路径覆盖（None = 默认 app 数据目录 models/qwen3-0.6b-q4_k_m.gguf）。
#[serde(default)]
pub model_path: Option<String>,
```

`Default` impl 加 `model_path: None`。

- [ ] **Step 2: 写失败测试**（model_fallback.rs tests）

```rust
#[test]
fn model_path_override_from_settings() {
    let dir = std::env::temp_dir().join(format!("locifind-mf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let settings_file = dir.join("settings.json");
    std::fs::write(
        &settings_file,
        r#"{"global_shortcut":"x","search_scope":["~"],"enable_model_fallback":true,"enable_tracing":false,"model_path":"/tmp/custom.gguf"}"#,
    )
    .unwrap();
    let handle = ModelFallbackHandle::new(Some(settings_file), dir.clone());
    assert_eq!(
        handle.resolved_model_path(),
        std::path::PathBuf::from("/tmp/custom.gguf")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn status_snapshot_reports_state() {
    let handle = ModelFallbackHandle::disabled("测试禁用");
    let s = handle.status_snapshot();
    assert_eq!(s.state, "unavailable");
    assert!(s.detail.contains("测试禁用"));
}
```

- [ ] **Step 3: 跑测试确认失败**

```bash
cargo test -p locifind-desktop model_path_override status_snapshot
```
预期：编译失败（方法不存在）。

- [ ] **Step 4: 实现**

4a. `resolved_model_path` 改为读 settings 覆盖：

```rust
/// 当前生效的模型路径：settings.model_path 覆盖 → 默认 app 数据目录 models/。
fn resolved_model_path(&self) -> PathBuf {
    if let Some(path) = &self.settings_path {
        if let Ok(s) = std::fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<crate::settings::AppSettings>(&s) {
                if let Some(custom) = v.model_path.filter(|p| !p.trim().is_empty()) {
                    return PathBuf::from(custom);
                }
            }
        }
    }
    self.default_model_path.clone()
}
```

4b. 状态快照（model_fallback.rs）：

```rust
/// 设置页用的状态快照。
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelStatusJson {
    /// "ready" | "loading" | "failed" | "unavailable" | "not_found" | "not_loaded"
    pub state: String,
    /// 人话详情（路径 / 失败原因 / 放置提示）。
    pub detail: String,
}

impl ModelFallbackHandle {
    pub(crate) fn status_snapshot(&self) -> ModelStatusJson {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (state, detail) = match &*st {
            ModelState::Ready(_) => (
                "ready",
                format!("已就绪：{}", self.resolved_model_path().display()),
            ),
            ModelState::Loading => ("loading", "模型加载中…".to_owned()),
            ModelState::Failed(e) => ("failed", format!("加载失败：{e}")),
            ModelState::Unavailable(r) => ("unavailable", r.clone()),
            ModelState::NotLoaded => {
                let p = self.resolved_model_path();
                if p.exists() {
                    ("not_loaded", "首次触发时自动加载".to_owned())
                } else {
                    ("not_found", format!("未找到模型文件，请放置到：{}", p.display()))
                }
            }
        };
        ModelStatusJson {
            state: state.to_owned(),
            detail,
        }
    }
}
```

4c. search.rs 根加命令薄包装 + main.rs `generate_handler!` 列表加 `search::get_model_status`：

```rust
/// BETA-23：设置页查询模型状态。
#[tauri::command]
pub fn get_model_status(
    deps: tauri::State<'_, SearchDeps>,
) -> model_fallback::ModelStatusJson {
    deps.model().status_snapshot()
}
```

4d. SettingsPage.tsx：`AppSettings` interface 加 `model_path: string | null`；「模型 fallback」开关区下方加状态行与路径输入（样式沿用页内既有 form 元素）：

```tsx
// 状态轮询（组件顶部 state 区）
const [modelStatus, setModelStatus] = useState<{ state: string; detail: string } | null>(null);
useEffect(() => {
  let alive = true;
  const poll = () =>
    invoke<{ state: string; detail: string }>('get_model_status')
      .then(s => { if (alive) setModelStatus(s); })
      .catch(() => {});
  poll();
  const t = setInterval(poll, 3000);
  return () => { alive = false; clearInterval(t); };
}, []);
```

```tsx
{/* enable_model_fallback 开关行下方 */}
{modelStatus && (
  <p className="setting-hint">模型状态：{modelStatus.detail}</p>
)}
<label>
  模型路径覆盖（留空用默认）
  <input
    type="text"
    value={settings.model_path ?? ''}
    onChange={e =>
      setSettings({ ...settings, model_path: e.target.value || null })
    }
    placeholder="默认：数据目录/models/qwen3-0.6b-q4_k_m.gguf"
  />
</label>
```

（`setting-hint` 等 className 以 SettingsPage 现有样式类为准对齐。）

- [ ] **Step 5: 跑测试 + 前端构建确认通过**

```bash
cargo test -p locifind-desktop
cd apps/desktop && npx tsc --noEmit && npm run build && cd ../..
```

- [ ] **Step 6: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A && git commit -m "feat(desktop): settings model_path 覆盖 + get_model_status 命令 + 设置页模型状态行"
```

---

### Task 9: CI — Release 构建开启 model-fallback

**Files:**
- Modify: `.github/workflows/release-windows.yml`

- [ ] **Step 1:** `tauri-apps/tauri-action@v0` 步骤的 `with:` 增加：

```yaml
args: --features model-fallback
```

并在 workflow 注释中写明：模型文件不打包，Release 说明须包含「模型手动放置到 `%APPDATA%/../Local（dirs::data_dir）/LociFind/models/qwen3-0.6b-q4_k_m.gguf`，不放则自动降级 parser-only」（CONVENTIONS §8 changelog 要求）。

- [ ] **Step 2: 本地 YAML 校验 + commit**

```bash
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release-windows.yml'))" && echo OK
git add -A && git commit -m "ci(release): Windows Release 构建开启 model-fallback feature"
```

（真实 CI 验证在下次推 tag 时；本任务不发版。）

---

### Task 10: evals — `--fire-rate` 触发率报告（parser-only，无需模型）

**Files:**
- Modify: `packages/evals/src/bin/evals.rs`

- [ ] **Step 1:** `Args` 加 flag（字段名/类型对齐既有 clap derive 风格）：

```rust
/// BETA-23：只报告 fallback 触发率（按 reason 分桶），不评测。纯 parser，无需模型。
#[arg(long)]
fire_rate: bool,
```

- [ ] **Step 2:** main 中 cases 加载完成后、评测循环之前插入：

```rust
if args.fire_rate {
    report_fire_rate(&cases);
    return Ok(());
}
```

实现（case 的 query 字段名以 evals.rs 现有 `EvalCase` 定义为准）：

```rust
/// BETA-23：统计 should_invoke_model 在数据集上的触发率与 reason 分布。
fn report_fire_rate(cases: &[EvalCase]) {
    use locifind_intent_parser::fallback::{
        parse_with_signals, should_invoke_model, FallbackDecision, FallbackReason,
    };
    let mut clarified = 0usize;
    let mut omission_fields: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut triggered_ids: Vec<&str> = Vec::new();
    for case in cases {
        let parsed = parse_with_signals(&case.query);
        match should_invoke_model(&parsed) {
            FallbackDecision::UseParser => {}
            FallbackDecision::InvokeModel(reason) => {
                triggered_ids.push(&case.id);
                match reason {
                    FallbackReason::ParserClarified => clarified += 1,
                    FallbackReason::StructuralOmission { fields } => {
                        for f in fields {
                            *omission_fields.entry(f).or_insert(0) += 1;
                        }
                    }
                }
            }
        }
    }
    let total = cases.len();
    let fired = triggered_ids.len();
    #[allow(clippy::cast_precision_loss)]
    let rate = if total == 0 { 0.0 } else { fired as f64 * 100.0 / total as f64 };
    println!("fire-rate: {fired}/{total} ({rate:.1}%)");
    println!("  clarified: {clarified}");
    for (field, n) in &omission_fields {
        println!("  omission.{field}: {n}");
    }
    println!("triggered ids: {}", triggered_ids.join(", "));
}
```

- [ ] **Step 3: 跑两套 fixtures 记录基线**

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5 --fire-rate
cargo run -p locifind-evals --bin evals -- --fixtures v0.9 --fire-rate
```
预期：正常输出比例与分桶。把两组数字记下来（写进 Task 12 的 ROADMAP 卡片）。**判读**：v0.5（模板 query、parser 全对）fire rate 应当很低；v0.9 coverage（自然语言、参考 keywords gap 183 例）偏高是预期——那正是模型要补的人群。若 v0.5 触发率 > ~5%，回 Task 2 收紧 `COVERAGE_NOISE_WORDS`/strip 词表后重测。

- [ ] **Step 4: 统一验证门 + commit**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p locifind-evals
git add -A && git commit -m "feat(evals): --fire-rate 触发率报告（BETA-23 误触发量化）"
```

---

### Task 11: 综合验证 — 双形态质量门 + with-fallback 真实模型对比（本机 metal）

无新文件；本任务产出**实测数据**（记入 Task 12 文档）。

- [ ] **Step 1: 前置检查**

```bash
cmake --version || brew install cmake
ls -lh models/qwen3-0.6b-q4_k_m.gguf
```

- [ ] **Step 2: feature 开形态的编译与 clippy 门**（首次编 llama-cpp 较慢属预期）

```bash
cargo clippy -p locifind-desktop --features model-fallback --all-targets -- -D warnings
cargo clippy -p locifind-desktop --features model-fallback-metal --all-targets -- -D warnings
```
预期：0 告警。

- [ ] **Step 3: parser-only byte-equal 硬门（再次机械确认）**

```bash
cargo run -p locifind-evals --bin evals -- --fixtures v0.5
cargo run -p locifind-evals --bin evals -- --fixtures v0.9
```
预期：473 / 726，与 main 一致。

- [ ] **Step 4: with-fallback before/after 对比（真实模型推理）**

```bash
# baseline（parser-only）与 with-fallback 各跑一次，输出 JSON 逐 case 比对
cargo run -p locifind-evals --bin evals -- --fixtures v0.9 --json > /tmp/beta23-parser-only.json
cargo run -p locifind-evals --features model-fallback --bin evals -- \
  --fixtures v0.9 --with-fallback --hybrid \
  --model-path models/qwen3-0.6b-q4_k_m.gguf --json > /tmp/beta23-with-fallback.json
```
（`--json` 的确切形态以 evals.rs 现有参数为准——BETA-13 会话曾用它做逐 case before/after。）

逐 case 比对，统计 gains（fail/partial→pass）与 regressions（pass→非 pass）。**验收：regressions = 0**；若有，逐条定位（多半是模型 patch 推翻 parser 正确字段 → 回 Task 3/4 调并集语义或 prompt），修复后重跑。

- [ ] **Step 5: 记录性能数据**

从 with-fallback 输出/日志记录：触发 case 的延迟 p50/p95（evals 与 BETA-17 同款口径）、模型加载耗时。**验收：触发查询 p95 ≤ 3s（本机 metal 预期 <1s）**。

- [ ] **Step 6: 全量回归 + commit（若本任务有代码修正）**

```bash
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git add -A && git commit -m "test(beta-23): with-fallback v0.9 对比与性能实测（净回退 0）" --allow-empty
```

---

### Task 12: 文档登记 — ROADMAP / STATUS / 真机手测指引

**Files:**
- Modify: `ROADMAP.md`（§3.3 B 阶段表）
- Modify: `STATUS.md`（candle 表述修正 + 下一步条目更新）
- Modify: `docs/manual-test-scenarios.md`（追加 BETA-23 场景）

- [ ] **Step 1: ROADMAP §3.3 新增 BETA-23 卡片**（状态 done，含：触发器扩展 / keywords 并集 / 编排状态机 / feature 隔离 / fire-rate 与 with-fallback 实测数据 / 模型放置约定）；原 BETA-15B 行补一句：「问题 4 的模型 fallback 接入已拆为 BETA-23（done）；本卡保留 embedding/LoRA 扩词方向」。

- [ ] **Step 2: STATUS.md**：「下一步」中问题 4 条目改为指向 BETA-23 done；修正「candle GGUF 真实推理就绪」表述（candle 是占位 echo，真实推理 = llama-cpp feature）。

- [ ] **Step 3: docs/manual-test-scenarios.md 追加 BETA-23 场景**：

```markdown
## BETA-23 模型 fallback（需 --features model-fallback-metal 构建 + 模型文件就位）

前置：cp models/qwen3-0.6b-q4_k_m.gguf "~/Library/Application Support/LociFind/models/"

1. 搜「2025年的会议纪要文件名包含运维」→ 首次出现「正在理解查询…」提示，
   本次结果为 parser（后台加载中）；稍候再搜同句 → 结果含「会议纪要」关键词命中，
   Started 摘要 fallback_used=true。
2. 设置页：模型状态显示「已就绪 + 路径」；关掉「模型 fallback」开关再搜 → 不再触发。
3. 把模型文件移走重启 → 设置页显示「未找到模型文件 + 放置路径」，搜索全功能正常（降级）。
```

- [ ] **Step 4: commit**

```bash
git add -A && git commit -m "docs: 登记 BETA-23 done（ROADMAP/STATUS/手测场景）+ 修正 candle 推理表述"
```

---

## 收尾

全部 task 完成后：用 superpowers:requesting-code-review 走 review，然后 superpowers:finishing-a-development-branch 决定合并方式（预期：合回 main）。真机 GUI 手测（manual-test BETA-23）留用户驱动，数据层验证（evals + 单测）已由 Task 11 覆盖。
