# 语义召回可信化（可解释 v1）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让语义召回结果可解释、可信——展示时按需算出 doc 里与 query 语义最相似的段落并高亮，标注召回来源，并在预览面板给出真实 cosine 置信档位 + 相似度下限说明。

**Architecture:** 三个特性均为**展示层加法**，不动 parser / 索引 / 融合算法。核心是新增「展示时按需算」的纯逻辑（切句 + 段落 embed + cosine 排序）放在 `locifind-semantic-index` crate 的新 `explain` 模块（可用确定性 fixture 单测），desktop 新增一个薄 Tauri 命令 `explain_semantic_hit` 复用现有预览正文读取，前端把返回的字符区间叠加到正文上高亮。来源标注与置信档位由前端从已有字段 + explain 真 cosine 派生。

**Tech Stack:** Rust（`locifind-semantic-index` / `apps/desktop` src-tauri）+ React/TypeScript（`apps/desktop/src`）+ Tauri 2。

---

## 关键设计事实（实现前必读）

- **段落 embed 走单条同步 API**：`TextEmbedder::embed(&str) -> Result<Vec<f32>, locifind_indexer::IndexError>`（`EmbeddingModelHandle` 实现它；返回 L2 归一化向量）。**每次 embed 新建 context、开销不小**，故段落数必须有上限（`MAX_PASSAGES=16`）——body 已被预览截断到 4000 字符，280×16≈4480 足够覆盖。Mac Metal 单篇百毫秒级；Windows CPU 单篇 1–3s × ≤16 段 = 数秒，**只在显式点选时发生 + 前端转圈**，可接受。批处理 / context 复用是 BETA-15B-4 的事，v1 不做。
- **query 向量无跨调用缓存**：检索期算的 query 向量不可在展示期复用 → explain 命令里重新 embed 一次 query（单条，便宜）。
- **字符偏移对齐**：explain 返回的 `start`/`end` 是 **Unicode scalar（char）偏移**，针对**与预览相同的截断后 body**（explain 内部复用 `get_preview_impl` 取 body，截断逻辑同源 → 偏移与前端渲染的 `preview.body` 完全对齐）。Rust `str.chars()` 与 JS `for..of`/`Array.from` 均按 code point 迭代，两端一致。
- **置信档位用真 cosine**：结果行的 `r.score` 在 fanout 融合后是 **RRF 累加分**（非 cosine 0–1），不可当相似度分档。真 cosine 只在 explain 返回的段落分里有 → **置信档位放预览面板**（取 top 段落 cosine），结果行徽标维持现状。
- **退化即等价**：feature `semantic-recall` 关 / 无模型时，`embed()` 返回 Err → `explain_passages` 返回空 → 前端无语义高亮、无置信档位。新命令不碰 parser/索引/融合 → evals 必须 byte-equal。
- **隐私**：explain 只读已索引正文（复用 `get_preview_impl`，不读原文件、不触发 OneDrive 水合），**不调 tracer**（按构造保证），段落向量内存即弃不落库。

### 涉及文件

| 文件 | 责任 | 动作 |
|---|---|---|
| `packages/search-backends/semantic-index/src/explain.rs` | 切句 + 排序 + 编排（纯逻辑，可单测） | 新建 |
| `packages/search-backends/semantic-index/src/lib.rs` | 挂载 `pub mod explain` | 改（1 行） |
| `apps/desktop/src-tauri/src/search/preview.rs` | `ExplainPayload` + `explain_semantic_hit_impl`（复用预览读 body） | 改 |
| `apps/desktop/src-tauri/src/search.rs` | `#[tauri::command] explain_semantic_hit` 薄包装 | 改 |
| `apps/desktop/src-tauri/src/main.rs` | `generate_handler!` 注册新命令 | 改（1 行） |
| `apps/desktop/src-tauri/src/search/tests.rs` | explain 退化路径测试 | 改 |
| `apps/desktop/src/SearchView.tsx` | explain 调用 + 段落高亮渲染 + 来源标注 + 置信/下限 | 改 |
| `apps/desktop/src/styles.css` | 语义高亮 mark 样式 | 改 |

---

## Task 1：explain 模块 —— `Passage` 类型 + `segment_passages` 切句

**Files:**
- Create: `packages/search-backends/semantic-index/src/explain.rs`
- Test: 同文件内 `#[cfg(test)] mod tests`

- [ ] **Step 1: 写失败测试**

新建 `packages/search-backends/semantic-index/src/explain.rs`，写入：

```rust
//! BETA-15B-5：语义命中段落高亮的纯逻辑（切句 + 排序 + 编排）。
//! 展示时按需算：不落库、不动索引。desktop 命令 `explain_semantic_hit` 调用。

use locifind_indexer::vectors::cosine;
use locifind_indexer::TextEmbedder;

/// 单个候选段落。`start`/`end` 为 body 的**字符**偏移（Unicode scalar，`end` 不含）。
#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// 每段目标字符数：到达即在下个句界/硬界收口（控制 embed 次数）。
const PASSAGE_TARGET_CHARS: usize = 280;
/// 段落数上限（body 已截断到 4000 字符，280×16≈4480 足够覆盖；兜底防极端长正文）。
const MAX_PASSAGES: usize = 16;
/// 高亮取相似度前 N 段。
const EXPLAIN_TOP_N: usize = 2;
/// 段落相似度下限：低于此不高亮（避免"硬凑"高亮误导）。
const EXPLAIN_MIN_SCORE: f32 = 0.30;

fn is_sentence_end(c: char) -> bool {
    // 只取 CJK 句末标点 + 换行 + 英文感叹/问号；英文句号易撞小数/缩写，
    // 不作句界，靠 `PASSAGE_TARGET_CHARS` 硬界兜住长英文段。
    matches!(c, '。' | '！' | '？' | '!' | '?' | '\n')
}

fn push_passage(out: &mut Vec<Passage>, start: usize, end: usize, buf: &str) {
    if !buf.trim().is_empty() {
        out.push(Passage {
            start,
            end,
            text: buf.to_owned(),
        });
    }
}

/// 把 body 切成有序、字符偏移连续的段落；跳过纯空白段；段数封顶 `MAX_PASSAGES`。
#[must_use]
pub fn segment_passages(body: &str) -> Vec<Passage> {
    let mut passages = Vec::new();
    let mut start = 0usize;
    let mut buf = String::new();
    let mut cur_len = 0usize;
    let mut idx = 0usize;
    for c in body.chars() {
        buf.push(c);
        idx += 1;
        cur_len += 1;
        if is_sentence_end(c) || cur_len >= PASSAGE_TARGET_CHARS {
            push_passage(&mut passages, start, idx, &buf);
            if passages.len() >= MAX_PASSAGES {
                return passages;
            }
            start = idx;
            buf.clear();
            cur_len = 0;
        }
    }
    push_passage(&mut passages, start, idx, &buf);
    passages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_on_sentence_ends_with_char_offsets() {
        let p = segment_passages("猫在叫。狗在跑！");
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].start, p[0].end), (0, 4)); // 猫在叫。
        assert_eq!(p[0].text, "猫在叫。");
        assert_eq!((p[1].start, p[1].end), (4, 8)); // 狗在跑！
    }

    #[test]
    fn empty_body_yields_no_passages() {
        assert!(segment_passages("").is_empty());
        assert!(segment_passages("   \n  ").is_empty());
    }

    #[test]
    fn long_unpunctuated_text_is_hard_split_by_target_len() {
        let body: String = "a".repeat(600);
        let p = segment_passages(&body);
        assert_eq!(p.len(), 3); // 280 + 280 + 40
        assert_eq!((p[0].start, p[0].end), (0, 280));
        assert_eq!((p[1].start, p[1].end), (280, 560));
        assert_eq!((p[2].start, p[2].end), (560, 600));
    }
}
```

- [ ] **Step 2: 运行测试确认失败（模块未挂载，编译错）**

Run: `cargo test -p locifind-semantic-index explain::tests::segments_on_sentence_ends_with_char_offsets 2>&1 | tail -5`
Expected: 编译失败 `module \`explain\` not found` 或测试不可见（模块尚未在 lib.rs 声明）。

- [ ] **Step 3: 在 lib.rs 挂载模块**

修改 `packages/search-backends/semantic-index/src/lib.rs`，在文件顶部（现有 `use`/`mod` 区域）加一行：

```rust
pub mod explain;
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p locifind-semantic-index explain:: 2>&1 | tail -8`
Expected: 3 个 explain 测试 PASS。

- [ ] **Step 5: 提交**

```bash
git add packages/search-backends/semantic-index/src/explain.rs packages/search-backends/semantic-index/src/lib.rs
git commit -m "BETA-15B-5 步1：explain 模块 segment_passages 切句（含字符偏移）"
```

---

## Task 2：`rank_passages` —— 过滤 + 排序 + 截断

**Files:**
- Modify: `packages/search-backends/semantic-index/src/explain.rs`

- [ ] **Step 1: 写失败测试**

在 `explain.rs` 的 `mod tests` 里追加：

```rust
    #[test]
    fn rank_filters_below_floor_sorts_desc_truncates() {
        let passages = vec![
            Passage { start: 0, end: 4, text: "a".into() },
            Passage { start: 4, end: 8, text: "b".into() },
            Passage { start: 8, end: 12, text: "c".into() },
        ];
        // (idx, cosine)：0=0.9 强、1=0.1 低于下限被滤、2=0.5 中
        let scored = vec![(0usize, 0.9f32), (1, 0.1), (2, 0.5)];
        let out = rank_passages(&passages, scored, 2, 0.30);
        assert_eq!(out, vec![(0, 4, 0.9), (8, 12, 0.5)]); // 降序 + 截断到 2 + 滤掉 0.1
    }

    #[test]
    fn rank_all_below_floor_is_empty() {
        let passages = vec![Passage { start: 0, end: 4, text: "a".into() }];
        let out = rank_passages(&passages, vec![(0usize, 0.05f32)], 2, 0.30);
        assert!(out.is_empty());
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p locifind-semantic-index explain::tests::rank_filters 2>&1 | tail -5`
Expected: FAIL —— `cannot find function \`rank_passages\``。

- [ ] **Step 3: 实现 `rank_passages`**

在 `explain.rs` 内（`segment_passages` 之后、`mod tests` 之前）加：

```rust
/// 过滤掉 < `min_score` 的段、按相似度降序、截断到 `top_n`，
/// 返回 `(start, end, score)`（字符偏移 + 真 cosine）。
fn rank_passages(
    passages: &[Passage],
    mut scored: Vec<(usize, f32)>,
    top_n: usize,
    min_score: f32,
) -> Vec<(usize, usize, f32)> {
    scored.retain(|(_, s)| *s >= min_score);
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(top_n);
    scored
        .into_iter()
        .map(|(i, s)| (passages[i].start, passages[i].end, s))
        .collect()
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p locifind-semantic-index explain:: 2>&1 | tail -8`
Expected: 全部 explain 测试 PASS（5 个）。

- [ ] **Step 5: 提交**

```bash
git add packages/search-backends/semantic-index/src/explain.rs
git commit -m "BETA-15B-5 步2：explain rank_passages 过滤+排序+截断"
```

---

## Task 3：`explain_passages` —— 编排（切句 → embed → cosine → 排序）

**Files:**
- Modify: `packages/search-backends/semantic-index/src/explain.rs`

- [ ] **Step 1: 写失败测试**

在 `mod tests` 顶部加一个确定性 embedder fixture（轴向量法，复用 lib.rs 同款思路）+ 两个测试：

```rust
    /// 含「猫」→ x 轴 [1,0]；含「狗」→ y 轴 [0,1]；否则 [0,0]。query 与段落同法，cosine 可预测。
    #[derive(Debug)]
    struct AxisEmbedder;
    impl TextEmbedder for AxisEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, locifind_indexer::IndexError> {
            Ok(vec![
                if text.contains('猫') { 1.0 } else { 0.0 },
                if text.contains('狗') { 1.0 } else { 0.0 },
            ])
        }
        fn model_id(&self) -> &'static str {
            "axis"
        }
    }

    /// query embed 失败 → 整体退化为空。
    #[derive(Debug)]
    struct FailEmbedder;
    impl TextEmbedder for FailEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, locifind_indexer::IndexError> {
            Err(locifind_indexer::IndexError::Tag {
                path: String::new(),
                detail: "no model".into(),
            })
        }
        fn model_id(&self) -> &'static str {
            "fail"
        }
    }

    #[test]
    fn explain_highlights_semantically_matching_passage() {
        // 段0「我有一只猫。」含猫→[1,0]，与 query「猫」[1,0] cosine=1.0；
        // 段1「外面有一条狗。」含狗→[0,1]，cosine=0.0 被下限滤掉。
        let body = "我有一只猫。外面有一条狗。";
        let out = explain_passages(body, "猫", &AxisEmbedder);
        assert_eq!(out.len(), 1);
        let (start, end, score) = out[0];
        assert_eq!((start, end), (0, 6)); // 「我有一只猫。」
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn explain_returns_empty_when_embedder_fails() {
        let out = explain_passages("我有一只猫。", "猫", &FailEmbedder);
        assert!(out.is_empty());
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p locifind-semantic-index explain::tests::explain_highlights 2>&1 | tail -5`
Expected: FAIL —— `cannot find function \`explain_passages\``。

- [ ] **Step 3: 实现 `explain_passages`**

在 `explain.rs` 内（`rank_passages` 之后）加：

```rust
/// 展示时按需算：切句 → 各段 embed → 与 query 向量 cosine → 取前 N 高于下限的段。
/// 返回 `(start, end, score)`（字符偏移 + 真 cosine）。无段落 / embed 失败 → 空。
/// **不落库、不读原文件**：`body` 由调用方从已索引正文取得。
#[must_use]
pub fn explain_passages(
    body: &str,
    query: &str,
    embedder: &dyn TextEmbedder,
) -> Vec<(usize, usize, f32)> {
    let passages = segment_passages(body);
    if passages.is_empty() {
        return Vec::new();
    }
    let Ok(query_vec) = embedder.embed(query) else {
        return Vec::new();
    };
    let mut scored = Vec::with_capacity(passages.len());
    for (i, p) in passages.iter().enumerate() {
        let Ok(v) = embedder.embed(&p.text) else {
            return Vec::new(); // embedder 中途坏掉 → 不给半成品高亮
        };
        scored.push((i, cosine(&query_vec, &v)));
    }
    rank_passages(&passages, scored, EXPLAIN_TOP_N, EXPLAIN_MIN_SCORE)
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p locifind-semantic-index explain:: 2>&1 | tail -10`
Expected: 7 个 explain 测试全 PASS。

- [ ] **Step 5: clippy + fmt + 提交**

Run: `cargo clippy -p locifind-semantic-index --all-targets -- -D warnings && cargo fmt -p locifind-semantic-index --check`
Expected: 0 警告、fmt 干净。

```bash
git add packages/search-backends/semantic-index/src/explain.rs
git commit -m "BETA-15B-5 步3：explain_passages 编排（embed+cosine+排序）"
```

---

## Task 4：desktop `ExplainPayload` + `explain_semantic_hit_impl`

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/preview.rs`
- Test: `apps/desktop/src-tauri/src/search/tests.rs`

- [ ] **Step 1: 写失败测试**

先确认 `tests.rs` 里构造带 embedding 的 `SearchDeps` 的现成写法（参考 `search_deps_with_embedding_round_trips`，约 438-463 行：`EmbeddingModelHandle::new(None, PathBuf)` + `deps.with_embedding(handle)`）。在 `tests.rs` 追加：

```rust
    #[test]
    fn explain_semantic_hit_unindexed_path_is_empty() {
        // 不存在的路径 → get_preview_impl 返 Unindexed → explain 空（无需真模型）。
        let registry = build_test_registry(FakeOkBackend(0), vec![SupportedIntent::FileSearch]);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _calls) = build_tracer_with_mock();
        let deps = SearchDeps::new(
            registry,
            policy,
            tracer,
            empty_context(),
            build_file_action_tool().0,
            empty_pending(),
            Arc::new(NoopExpander) as Arc<dyn SynonymExpander>,
        )
        .with_embedding(Arc::new(embedding_model::EmbeddingModelHandle::new(
            None,
            std::path::PathBuf::from("/tmp/locifind-explain-test"),
        )));

        let payload = explain_semantic_hit_impl("/no/such/file.txt", "猫", &deps);
        assert!(payload.passages.is_empty());
    }
```

> 注：若 `SearchDeps::new` 的实参列表与上面不完全一致，照 `tests.rs` 现有 `search_deps_*` 测试的同款构造写法对齐即可（本测试只关心退化为空）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p locifind-desktop explain_semantic_hit_unindexed 2>&1 | tail -6`
Expected: FAIL —— `cannot find function \`explain_semantic_hit_impl\`` / `ExplainPayload`。

- [ ] **Step 3: 实现 `ExplainPayload` + impl**

修改 `apps/desktop/src-tauri/src/search/preview.rs`，在 `PreviewPayload` 定义之后加类型，在 `get_preview_impl` 之后加函数：

```rust
/// BETA-15B-5：语义命中的高亮段落区间（字符偏移，针对截断后 body）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExplainPayload {
    pub passages: Vec<ExplainPassage>,
}

/// 单个高亮段：`start`/`end` 为 body 的字符偏移（`end` 不含），`score` 为真 cosine。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExplainPassage {
    pub start: usize,
    pub end: usize,
    pub score: f32,
}

/// 对选中的语义命中结果，算出 body 中与 query 语义最相似的段落区间。
/// **只读已索引正文**（复用 [`get_preview_impl`]，不读原文件、不触发水合）、**不调 tracer**。
/// 非文档 / 无模型 / feature 关 → 空 payload（前端无高亮，逐字节等价于现状）。
pub(crate) fn explain_semantic_hit_impl(
    path: &str,
    query: &str,
    deps: &SearchDeps,
) -> ExplainPayload {
    // 取与预览相同的（已截断）body，保证字符偏移与前端渲染对齐。
    let PreviewPayload::Document { body, .. } = get_preview_impl(path, None, deps) else {
        return ExplainPayload { passages: Vec::new() };
    };
    let embedder = deps.embedding();
    let ranges =
        locifind_semantic_index::explain::explain_passages(&body, query, embedder.as_ref());
    ExplainPayload {
        passages: ranges
            .into_iter()
            .map(|(start, end, score)| ExplainPassage { start, end, score })
            .collect(),
    }
}
```

> 确认 `preview.rs` 顶部已 `use` 到 `SearchDeps` / `Serialize` / `get_preview_impl` 同模块可见（`get_preview_impl` 同文件、`SearchDeps` 经 `super::` 或现有 `use`）。`deps.embedding()` 返回 `&Arc<EmbeddingModelHandle>`，`embedder.as_ref()` 得 `&EmbeddingModelHandle`，自动 coerce 为 `&dyn TextEmbedder`。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p locifind-desktop explain_semantic_hit_unindexed 2>&1 | tail -6`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/search/preview.rs apps/desktop/src-tauri/src/search/tests.rs
git commit -m "BETA-15B-5 步4：desktop explain_semantic_hit_impl + ExplainPayload"
```

---

## Task 5：Tauri 命令 `explain_semantic_hit` + 注册

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: 加命令薄包装**

修改 `apps/desktop/src-tauri/src/search.rs`，在 `get_preview` 命令（约 668-675 行）之后加：

```rust
/// BETA-15B-5：取选中语义结果的「命中段落」高亮区间（字符偏移 + 真 cosine）。
/// **只读本地索引、不读原文件、不进 trace**。无模型 / feature 关 → 空。
#[tauri::command]
pub async fn explain_semantic_hit(
    path: String,
    query: String,
    deps: tauri::State<'_, SearchDeps>,
) -> Result<ExplainPayload, String> {
    Ok(explain_semantic_hit_impl(&path, &query, deps.inner()))
}
```

> `ExplainPayload` / `explain_semantic_hit_impl` 经 `search.rs` 顶部已有的 `pub(crate) use preview::*;` 自动可见，无需额外 import。

- [ ] **Step 2: 注册到 `generate_handler!`**

修改 `apps/desktop/src-tauri/src/main.rs`，在 `generate_handler!` 列表里 `search::get_preview,`（约 380 行）之后加一行：

```rust
            search::explain_semantic_hit,
```

- [ ] **Step 3: 编译验证**

Run: `cargo build -p locifind-desktop 2>&1 | tail -6`
Expected: 编译通过（命令签名被 Tauri 宏接受、已注册）。

- [ ] **Step 4: 全 crate 测试 + clippy + fmt**

Run: `cargo test -p locifind-desktop 2>&1 | tail -6 && cargo clippy -p locifind-desktop --all-targets -- -D warnings && cargo fmt -p locifind-desktop --check`
Expected: 测试零失败、clippy 0 警告、fmt 干净。

- [ ] **Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/main.rs
git commit -m "BETA-15B-5 步5：explain_semantic_hit Tauri 命令 + 注册"
```

---

## Task 6：前端段落高亮（explain 调用 + 渲染 + 样式）

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`
- Modify: `apps/desktop/src/styles.css`

> 前端无单测框架（仅 `tsc && vite build`）。每步以编译通过为门，真机视觉留用户。

- [ ] **Step 1: 加 TS 类型**

在 `SearchView.tsx` 顶部 `SearchResultJson` interface（约 4-16 行）附近加：

```tsx
interface ExplainPayload {
  passages: { start: number; end: number; score: number }[];
}
```

- [ ] **Step 2: 加渲染 helper**

在 `renderHighlighted`（约 1345 行）附近加（按 code point 迭代，与 Rust char 偏移一致）：

```tsx
/// 把语义命中段落区间（字符偏移）叠加到正文上高亮。区间不重叠、已按 start 排序处理。
function renderWithSemanticRanges(
  body: string,
  passages: { start: number; end: number; score: number }[],
): React.ReactNode[] {
  const ranges = [...passages].sort((a, b) => a.start - b.start);
  const parts: React.ReactNode[] = [];
  let buf = "";
  let key = 0;
  let i = 0; // 当前 code point 序号
  let ri = 0; // 当前 range 指针
  let inMark = false;
  let curScore: number | undefined;
  const flush = () => {
    if (!buf) return;
    parts.push(
      inMark ? (
        <mark
          key={key++}
          className="semantic-highlight"
          title={curScore !== undefined ? `语义相似度 ${curScore.toFixed(2)}` : undefined}
        >
          {buf}
        </mark>
      ) : (
        <span key={key++}>{buf}</span>
      ),
    );
    buf = "";
  };
  for (const ch of body) {
    const inRange = ri < ranges.length && i >= ranges[ri].start && i < ranges[ri].end;
    if (inRange !== inMark) {
      flush();
      inMark = inRange;
      curScore = inRange ? ranges[ri].score : undefined;
    }
    buf += ch;
    i += 1;
    if (ri < ranges.length && i >= ranges[ri].end) {
      flush();
      inMark = false;
      curScore = undefined;
      ri += 1;
    }
  }
  flush();
  return parts;
}
```

- [ ] **Step 3: 加 explain 状态 + 调用 effect**

在 SearchView 组件内、`preview` 相关 state 附近加：

```tsx
const [explain, setExplain] = useState<ExplainPayload | null>(null);
```

在预览 `useEffect`（约 839-865 行）之后加一个独立 effect：

```tsx
// BETA-15B-5：选中语义命中结果时，按需算命中段落高亮区间。
useEffect(() => {
  setExplain(null);
  if (!selectedResult || !executedQuery) return;
  const isSemantic =
    selectedResult.match_type === "semantic" ||
    (selectedResult.sources?.includes("semanticindex") ?? false);
  if (!isSemantic) return;
  let cancelled = false;
  invoke<ExplainPayload>("explain_semantic_hit", {
    path: selectedResult.path,
    query: executedQuery,
  })
    .then((e) => {
      if (!cancelled) setExplain(e);
    })
    .catch(() => {
      if (!cancelled) setExplain(null);
    });
  return () => {
    cancelled = true;
  };
}, [selectedResult, executedQuery]);
```

> `selectedResult` / `executedQuery` 为现有 state（预览 effect 已用）。

- [ ] **Step 4: 传 prop 并渲染**

把 `explain` 传进 `PreviewPanel`（找到 `<PreviewPanel ... />` 调用处，加 `explain={explain}`）。在 `PreviewPanel` 函数签名（约 1396 行）加形参 `explain: ExplainPayload | null;`。把正文渲染（约 1480-1483 行）改为：

```tsx
              <pre className="preview-content-text">
                {explain && explain.passages.length > 0
                  ? renderWithSemanticRanges(preview.body, explain.passages)
                  : preview.body || "（无可预览的文本内容）"}
                {preview.body_truncated && "\n…（已截断）"}
              </pre>
```

- [ ] **Step 5: 加 CSS**

在 `apps/desktop/src/styles.css` 的 `.preview-snippet-text mark`（约 825 行）附近加（蓝色区别于 FTS 黄色）：

```css
.preview-content-text mark.semantic-highlight {
  background-color: rgba(0, 103, 192, 0.18);
  color: inherit;
  border-radius: 2px;
}
```

- [ ] **Step 6: 编译验证 + 提交**

Run: `cd apps/desktop && npm run build 2>&1 | tail -8`
Expected: tsc 无类型错、vite build 成功。

```bash
git add apps/desktop/src/SearchView.tsx apps/desktop/src/styles.css
git commit -m "BETA-15B-5 步6：前端语义命中段落高亮"
```

---

## Task 7：来源标注 + 置信档位 + 下限说明

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`

- [ ] **Step 1: 来源标注 helper + 应用到匹配列**

在 `matchTypeLabel`（约 154 行）附近加：

```tsx
/// 语义召回来源标注：纯语义命中 / 关键词+语义双中 / 非语义（null）。
function semanticSourceLabel(r: SearchResultJson): string | null {
  const srcs = r.sources ?? [];
  const hasSem = srcs.includes("semanticindex") || r.match_type === "semantic";
  if (!hasSem) return null;
  const hasKeyword = srcs.some((s) => s !== "semanticindex");
  return hasKeyword ? "关键词+语义双中" : "纯语义命中";
}
```

把"匹配方式"列 render（约 244-257 行）改为用该标注（保留现有 RRF 分显示，不当作 cosine）：

```tsx
    render: (r) => {
      const semLabel = semanticSourceLabel(r);
      return semLabel ? (
        <span className="badge-semantic" title={`${semLabel}（按语义/跨语言召回）`}>
          {semLabel}
          {typeof r.score === "number" && (
            <span style={{ color: "#999", marginLeft: "6px", fontWeight: 400 }}>
              · {r.score.toFixed(2)}
            </span>
          )}
        </span>
      ) : (
        matchTypeLabel(r.match_type)
      );
    },
```

- [ ] **Step 2: 取相似度下限（复用 get_settings）**

在 SearchView 组件内加 state + 挂载读取：

```tsx
const [semanticFloor, setSemanticFloor] = useState(0.3);
```

```tsx
// BETA-15B-5：读相似度下限用于「弱相关已隐藏」说明（get_settings 已返回该字段）。
useEffect(() => {
  invoke<{ semantic_similarity_floor: number | null }>("get_settings")
    .then((s) => setSemanticFloor(s.semantic_similarity_floor ?? 0.3))
    .catch(() => {});
}, []);
```

- [ ] **Step 3: 置信档位 helper + 预览面板展示**

在 `confidenceBand` 处加（用真 cosine，来自 explain 段落分）：

```tsx
/// 真 cosine → 置信档位。
function confidenceBand(score: number): string {
  if (score >= 0.5) return "强相关";
  if (score >= 0.3) return "中相关";
  return "弱相关";
}
```

在 `PreviewPanel` 签名加 `semanticFloor: number;`，并在文档正文区块（约 1474 行 `preview-content` 之前）插入语义置信行（仅当有 explain 段落时）：

```tsx
            {explain && explain.passages.length > 0 && (
              <div className="preview-semantic-note">
                <div className="preview-section-label">语义命中</div>
                <p>
                  最相似段落 · 语义相似度 {explain.passages[0].score.toFixed(2)}（
                  {confidenceBand(explain.passages[0].score)}）
                  <span style={{ color: "#999", marginLeft: "8px" }}>
                    低于 {semanticFloor.toFixed(2)} 的弱相关结果已隐藏
                  </span>
                </p>
              </div>
            )}
```

在 `<PreviewPanel ... />` 调用处加 `semanticFloor={semanticFloor}`。

- [ ] **Step 4: 编译验证 + 提交**

Run: `cd apps/desktop && npm run build 2>&1 | tail -8`
Expected: tsc 无类型错、vite build 成功。

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "BETA-15B-5 步7：召回来源标注 + 置信档位 + 下限说明"
```

---

## Task 8：全栈验证 + evals byte-equal 闸门

**Files:** 无（仅验证）

- [ ] **Step 1: 全 workspace 测试**

Run: `cargo test --workspace 2>&1 | tail -15`
Expected: 0 failed（platform-macos 既有 2 个预存失败除外，见 STATUS 惯例）。

- [ ] **Step 2: clippy + fmt 全量**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5 && cargo fmt --check 2>&1 | tail -5`
Expected: 0 警告、fmt 干净。

- [ ] **Step 3: evals byte-equal 硬门**

本特性是纯展示层加法，不动 parser / 索引 / 融合 → parser-only evals 必须 0 变化。按 STATUS 记载方法跑 v0.5 / v0.9（规范化逐 case 比对，HashMap 序 + elapsed_ms 非确定）：

Run（参考 `packages/evals` 的 ci 跑法 + STATUS「byte-equal 闸门方法」段）：
```bash
# 跑 v0.5 / v0.9 parser-only，与改动前快照规范化逐 case 比对 actual_json
cargo run -p locifind-evals -- --dataset v0.5 --json 2>/dev/null | <规范化比对脚本>
```
Expected: **v0.5 = 473、v0.9 = 877，diffs = 0**（与改动前完全一致）。
> 若仓库已有现成 evals 跑脚本（见 `packages/evals/README` / `ci.sh`），用现成的；关键是确认 pass/partial/fail 计数与 §6 百分比相对改动前**零变化**。

- [ ] **Step 4: 前端构建**

Run: `cd apps/desktop && npm run build 2>&1 | tail -6`
Expected: tsc + vite build 成功。

- [ ] **Step 5: 退化形态自查（无 feature）**

确认默认构建（不含 `semantic-recall`）下 `explain_semantic_hit` 返回空、前端无语义高亮——即 Step 1 的 desktop 测试已覆盖（`embed()` Unavailable → 空）。无需额外动作，确认即可。

> **真机手测（留用户，双平台）**：① 跨语言 case（中文 query 召回英文 doc）点开预览 → 高亮段是否真命中语义对应句；② Windows 验「点选后转圈→高亮出现」延迟可接受；③ 来源标注（纯语义/双中）与置信档位与直觉一致。手测脚本追加到 [docs/manual-test-scenarios.md](../../manual-test-scenarios.md) BETA-15B-5 节。

---

## Self-Review（计划对照 spec）

- **spec §2.1 特性1 段落高亮** → Task 1-3（纯逻辑）+ Task 4-5（后端命令）+ Task 6（前端渲染）✅
- **spec §2.1 特性2 来源标注** → Task 7 Step 1（前端从 `sources` 派生）✅
- **spec §2.1 特性3 置信分级 + 下限解释** → Task 7 Step 2-3（预览面板用真 cosine + get_settings 下限）✅。**对 spec 的一处诚实修正**：spec 原设想档位可挂结果行；实现核查发现结果行 `score` 是 RRF 累加分非 cosine，故档位改放预览面板、用 explain 段落真 cosine——更准确，已在「关键设计事实」记录。
- **spec §3 路线 A（展示时按需算）** → Task 4 `explain_semantic_hit_impl` 复用预览读 body、内存即弃 ✅
- **spec §5 隐私** → 只读索引正文、不调 tracer、向量内存即弃（Task 4 注释 + 构造保证）✅
- **spec §6 退化等价** → Task 8 Step 5 + Task 4 测试（embed Err → 空）✅
- **spec §7 评测语料隐私决策** → 不在本 plan（spec 明确 v1 不实现，仅登记决策）；收工时登记到 STATUS「阻塞/待决策」✅
- **spec §8 测试** → Task 1-3 纯函数单测、Task 4 退化测试、Task 8 evals byte-equal ✅
- **占位扫描**：无 TBD/TODO；evals 比对脚本指向仓库现成 ci 跑法（Step 3 已注明回退）。
- **类型一致**：`Passage{start,end,text}`、`explain_passages -> Vec<(usize,usize,f32)>`、`ExplainPayload{passages:Vec<ExplainPassage{start,end,score}>}`、TS `ExplainPayload` 全程一致 ✅
```
