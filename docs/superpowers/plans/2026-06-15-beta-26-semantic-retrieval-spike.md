# BETA-26 本地语义检索质量探针 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在固定真实语料上，量化 hybrid（FTS5 + 向量）相对 FTS5-only 在"模糊查询"上的检索相关性提升够不够大，产出一个 go/no-go 数字 + 一份方法学。

**Architecture:** 一个**一次性丢弃**的独立 spike crate（`packages/spike-retrieval`），不碰生产管线。三步：① 从真实目录抽取文本冻结成 `corpus.jsonl`；② 用 Qwen3-Embedding-0.6B（GGUF / llama.cpp）把语料 embed 成向量；③ 加载手标评测集，跑 FTS5-only / 向量-only / hybrid-RRF 三组检索，算分桶 Recall@10 / nDCG@10 + 成本数字，对照预设 kill 标准出结论。向量检索用**内存暴力 cosine**（<5k 文档，spec §3.3 已认可不需 ANN），跳过 sqlite-vec 的扩展加载麻烦。

**Tech Stack:** Rust / rusqlite（FTS5，复用 indexer 已 bundled 的 SQLite）/ llama-cpp-4（embedding 模式，复用 model-runtime 后端初始化）/ Qwen3-Embedding-0.6B GGUF / serde_json。纯函数（cosine、RRF、Recall@10、nDCG@10、评测集完整性）走 TDD；embedding 与语料抽取走冒烟/证伪闸门；评测集由 Claude 读语料起草、用户复核。

**隐私硬约束（CONVENTIONS §7 + spec §3.2）：** `corpus.jsonl`、向量文件、评测集 `cases.json` 含真实个人数据，**一律不进 git**——Task 0 先把它们加进 `.gitignore`。代码可入库，数据不可。

**范围护栏（spec §2，YAGNI）：** 不做生产管线 / 增量索引 / UI / 跨平台（只 Mac 跑）/ 后台调度 / 分块调优 / 多模型横评。一个模型、一种融合（RRF）、一份冻结语料、一遍跑完。

---

## File Structure

```
packages/spike-retrieval/                 # 新 throwaway crate（GO/NO-GO 出结论后可整包删）
  Cargo.toml                              # deps: indexer(extract_document), model-runtime(embed), rusqlite, serde, serde_json, walkdir, anyhow
  src/lib.rs                              # 纯函数核心 + 类型：CorpusDoc / EvalCase / cosine / rrf_fuse / recall_at_k / ndcg_at_k
  src/bin/build_corpus.rs                 # 遍历真实目录 → extract_document → corpus.jsonl（冻结快照，gitignored）
  src/bin/embed_corpus.rs                 # 读 corpus.jsonl → embed 每篇 → vectors.bin + 成本数字（gitignored）
  src/bin/run_retrieval.rs               # 读 evalset + corpus + vectors → 跑 A/B/C 三组 → 分桶指标 + 成本 → stdout/JSON
  fixtures/                               # 全部 gitignored
    corpus.jsonl                          # 冻结语料快照
    vectors.bin                           # embedding 向量（f32）
    evalset/cases.json                    # 手标评测集（Claude 起草 + 用户复核）
packages/model-runtime/
  src/lib.rs                              # 加 trait 方法 embed()（gated by 现有 llama-cpp feature）
  src/llama.rs                            # 实现 embed()：embeddings=true context + mean-pooling + L2 normalize
docs/reviews/spike-semantic-retrieval.md  # 最终 go/no-go 备忘（产出物）
.gitignore                               # 加 spike fixtures 数据排除
```

任务顺序按依赖与"先动人是瓶颈的事"排：Task 0 脚手架 → Task 1 embedding 证伪闸门（最大技术风险，先打掉）→ Task 2 语料冻结 → Task 3 评测集起草 → Task 4 纯函数（cosine/RRF/指标，TDD）→ Task 5 embed 全语料 → Task 6 三组检索 + 指标 → Task 7 go/no-go 备忘。

---

### Task 0: Spike crate 脚手架 + 隐私 gitignore

**Files:**
- Create: `packages/spike-retrieval/Cargo.toml`
- Create: `packages/spike-retrieval/src/lib.rs`
- Modify: `Cargo.toml`（workspace 根，members 加 `packages/spike-retrieval`）
- Modify: `.gitignore`

- [ ] **Step 1: 在 workspace 根 `Cargo.toml` 的 `members` 数组加入新 crate**

找到 `[workspace]` 的 `members = [...]`，追加一行：
```toml
    "packages/spike-retrieval",
```

- [ ] **Step 2: 写 spike crate 的 Cargo.toml**

```toml
[package]
name = "spike-retrieval"
version = "0.0.0"
edition = "2021"
publish = false

# 一次性探针（BETA-26），GO/NO-GO 出结论后可整包删除。

[dependencies]
indexer = { path = "../indexer" }
model-runtime = { path = "../model-runtime", features = ["llama-cpp"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
walkdir = "2"
anyhow = "1"

[features]
metal = ["model-runtime/metal"]

[[bin]]
name = "build-corpus"
path = "src/bin/build_corpus.rs"

[[bin]]
name = "embed-corpus"
path = "src/bin/embed_corpus.rs"

[[bin]]
name = "run-retrieval"
path = "src/bin/run_retrieval.rs"
```
> 注：若 `walkdir` 未在其它 crate 用过，它是轻量纯 Rust 依赖，登记到 `docs/third-party-licenses.md`（Task 7 收尾时一并做）。`model-runtime` 的 `embed()` 在 Task 1 加。

- [ ] **Step 3: 写最小 lib.rs 占位（后续 Task 4 填纯函数）**

```rust
//! BETA-26 语义检索质量探针——一次性丢弃 crate。
//! 产出 go/no-go 数字 + 方法学，非生产代码。GO/NO-GO 后可删。

use serde::{Deserialize, Serialize};

/// 冻结语料里的一篇文档（chunk 粒度）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusDoc {
    pub id: String,
    pub path: String,
    pub text: String,
}

/// 一条模糊检索 case：query + 应命中的文件 id（按语义应然，独立于任何检索器实际返回）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalCase {
    pub id: String,
    /// 5 桶之一：synonym | concept | crosslang | ocr | content-not-name
    pub bucket: String,
    pub query: String,
    /// doc id -> 相关度分级 1..=3（用于 nDCG）
    pub relevant: Vec<RelevantDoc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevantDoc {
    pub doc_id: String,
    pub grade: u8,
}
```

- [ ] **Step 4: 把数据文件加进 `.gitignore`**

在 `.gitignore` 末尾追加：
```gitignore
# BETA-26 探针：真实个人数据语料/向量/评测集不进 git（CONVENTIONS §7 + spec §3.2）
/packages/spike-retrieval/fixtures/corpus.jsonl
/packages/spike-retrieval/fixtures/vectors.bin
/packages/spike-retrieval/fixtures/evalset/cases.json
```

- [ ] **Step 5: 验证编译 + gitignore 生效**

Run: `cargo build -p spike-retrieval`
Expected: 编译通过（暂无 bin，仅 lib）。
Run: `mkdir -p packages/spike-retrieval/fixtures/evalset && touch packages/spike-retrieval/fixtures/corpus.jsonl packages/spike-retrieval/fixtures/evalset/cases.json && git status --porcelain packages/spike-retrieval/fixtures/`
Expected: **空输出**（数据文件被 ignore，不出现在 status）。

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml .gitignore packages/spike-retrieval/Cargo.toml packages/spike-retrieval/src/lib.rs
git commit -m "BETA-26: spike-retrieval 脚手架 + 隐私数据 gitignore"
```

---

### Task 1: Embedding 证伪闸门（最大技术风险，先打掉）

> 这是 spike 的 kill-switch：若 llama-cpp-4 跑不出可用 embedding（或模型不存在/质量太差），到此为止，不必再投后续。

**Files:**
- Modify: `packages/model-runtime/src/lib.rs`（trait 加 `embed`）
- Modify: `packages/model-runtime/src/llama.rs`（实现 embed）
- Create: `packages/spike-retrieval/src/bin/embed_smoke.rs`（临时冒烟，验证后删，不入 `[[bin]]` 列表则用 `--example` 或直接临时加）

- [ ] **Step 1: 下载 Qwen3-Embedding-0.6B GGUF 到 `models/`**

Run:
```bash
curl -L -o models/qwen3-embedding-0.6b-q8_0.gguf \
  https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF/resolve/main/Qwen3-Embedding-0.6B-Q8_0.gguf
ls -la models/qwen3-embedding-0.6b-q8_0.gguf
```
Expected: 文件下载成功（约 600MB+）。若 URL 失效，去 HuggingFace `Qwen/Qwen3-Embedding-0.6B-GGUF` 仓库取实际文件名。embedding 模型对量化更敏感，用 Q8_0（探针不在乎体积）。

- [ ] **Step 2: 在 model-runtime trait 加 `embed` 方法（gated by llama-cpp feature）**

`packages/model-runtime/src/lib.rs` 的 `LlamaModelRuntime` trait 加：
```rust
    /// 生成单段文本的句向量（mean-pooled + L2 normalized）。
    /// 仅 embedding 模型有意义；BETA-26 探针用。
    fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
```
若 trait 的非-llama 实现（candle/占位）也需满足，给它们返回 `anyhow::bail!("embed not supported by this backend")`。

- [ ] **Step 3: 在 llama.rs 实现 embed**

关键点：embedding 需要 `LlamaContextParams` 开 `with_embeddings(true)`，且 batch decode 后取 `ctx.embeddings_seq_ith` / `ctx.embeddings_ith`（按 llama-cpp-4 实际 API）。实现 mean-pooling over tokens + L2 normalize。参考现有 `llama.rs` 里 model/backend 初始化与 tokenize 代码复用：
```rust
fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
    use llama_cpp_4::context::params::LlamaContextParams;
    use llama_cpp_4::llama_batch::LlamaBatch;

    let ctx_params = LlamaContextParams::default()
        .with_embeddings(true)
        .with_n_ctx(std::num::NonZeroU32::new(2048));
    let mut ctx = self
        .model
        .new_context(&self.backend, ctx_params)?;

    let tokens = self.model.str_to_token(text, llama_cpp_4::model::AddBos::Always)?;
    let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
    let last = tokens.len().saturating_sub(1);
    for (i, tok) in tokens.iter().enumerate() {
        batch.add(*tok, i as i32, &[0], i == last)?;
    }
    ctx.decode(&mut batch)?;

    // mean-pool：llama-cpp-4 在 embeddings 模式下按 seq 给池化向量；
    // 若 API 给的是每 token 向量，则手动平均。下面取 seq 级向量：
    let emb = ctx.embeddings_seq_ith(0)?; // &[f32]
    let mut v = emb.to_vec();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    Ok(v)
}
```
> 注：`embeddings_seq_ith` 的确切名字/签名以 llama-cpp-4 当前版本为准——本步第一要务是"跑通拿到非空向量"，API 细节在编译期对照 crate docs 调整。若只能拿每 token 向量，就 mean-pool 后再 normalize。

- [ ] **Step 4: 写 embedding 冒烟（证伪闸门）**

`packages/spike-retrieval/src/bin/embed_smoke.rs`（临时，验证后删除）：
```rust
use anyhow::Result;
use model_runtime::{llama::LlamaRuntime, LlamaModelRuntime}; // 按实际导出路径调整

fn main() -> Result<()> {
    let model_path = std::env::var("EMBED_MODEL")
        .unwrap_or_else(|_| "models/qwen3-embedding-0.6b-q8_0.gguf".into());
    let rt = LlamaRuntime::load(&model_path)?; // 按实际构造函数调整

    let a = rt.embed("如何处理客户的退款申请")?;
    let b = rt.embed("退货与退款流程说明")?;          // 语义相近
    let c = rt.embed("今天天气适合去爬山")?;          // 语义无关

    let cos = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
    let sim_close = cos(&a, &b);
    let sim_far = cos(&a, &c);
    println!("dim={} sim_close={:.4} sim_far={:.4}", a.len(), sim_close, sim_far);
    assert!(!a.is_empty(), "embedding 为空——闸门失败");
    assert!(sim_close > sim_far, "相近句相似度应高于无关句——模型/实现可疑");
    println!("✅ embedding 证伪闸门通过");
    Ok(())
}
```

- [ ] **Step 5: 跑闸门**

Run（Mac，带 metal 加速）: `cargo run -p spike-retrieval --features metal --bin embed_smoke`
Expected: 打印非空 `dim`（Qwen3-Embedding-0.6B 维度 1024）、`sim_close > sim_far`、最后 `✅ ... 通过`。
**若失败**：embedding 路径不通是整个 spike 的硬阻塞——停下来与用户同步，不要继续 Task 2+。

- [ ] **Step 6: 删除临时冒烟 bin，保留 embed 实现，Commit**

```bash
rm packages/spike-retrieval/src/bin/embed_smoke.rs
cargo build -p model-runtime --features llama-cpp
git add packages/model-runtime/src/lib.rs packages/model-runtime/src/llama.rs
git commit -m "BETA-26: model-runtime 加 embed() 路径（llama.cpp embedding 模式）+ 证伪闸门通过"
```
> `embed()` 是唯一可能在 GO 后存活进 BETA-15B 的代码；其余 spike-retrieval 是丢弃件。

---

### Task 2: 冻结语料（真实目录 → corpus.jsonl）

**Files:**
- Create: `packages/spike-retrieval/src/bin/build_corpus.rs`

- [ ] **Step 1: 写 build_corpus.rs**

```rust
use anyhow::Result;
use indexer::extract_document;
use spike_retrieval::CorpusDoc;
use std::io::Write;
use std::path::Path;
use walkdir::WalkDir;

fn main() -> Result<()> {
    let dir = std::env::args().nth(1).expect("用法: build-corpus <真实目录>");
    let out = "packages/spike-retrieval/fixtures/corpus.jsonl";
    let max_docs: usize = std::env::var("MAX_DOCS").ok()
        .and_then(|s| s.parse().ok()).unwrap_or(5000);

    let exts = ["docx", "pptx", "xlsx", "xls", "pdf", "html", "md", "txt"];
    let mut f = std::io::BufWriter::new(std::fs::File::create(out)?);
    let mut n = 0usize;
    let mut skipped = 0usize;

    for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
        if n >= max_docs { break; }
        let p = entry.path();
        if !p.is_file() { continue; }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !exts.contains(&ext.as_str()) { continue; }

        let mtime = p.metadata().and_then(|m| m.modified()).ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64).unwrap_or(0);

        match extract_document(p, mtime) {
            Ok((_entry, text)) if !text.trim().is_empty() => {
                let doc = CorpusDoc {
                    id: format!("d{n:05}"),
                    path: p.to_string_lossy().to_string(),
                    text,
                };
                writeln!(f, "{}", serde_json::to_string(&doc)?)?;
                n += 1;
            }
            _ => { skipped += 1; }
        }
    }
    f.flush()?;
    eprintln!("✅ 冻结 {n} 篇文档 → {out}（跳过 {skipped} 篇空/失败）");
    Ok(())
}
```
> 说明：本探针先按"整文档一向量"做（chunk = 整篇）。若 D3 发现长文档拉低召回，再在此切 chunk——spec §2 明确不做分块调优马拉松，先最简。

- [ ] **Step 2: 跑语料冻结（路径由用户提供）**

Run: `cargo run -p spike-retrieval --bin build-corpus -- "<用户提供的真实目录>"`
Expected: stderr 打印 `✅ 冻结 N 篇文档`，N 落在 2000~5000（不足则换更大目录或放宽 `exts`；过多用 `MAX_DOCS=5000` 截断）。
Run: `wc -l packages/spike-retrieval/fixtures/corpus.jsonl`
Expected: 行数 == N。

- [ ] **Step 3: 抽查语料质量（确认含噪声硬货）**

Run: `head -c 2000 packages/spike-retrieval/fixtures/corpus.jsonl`
人工确认：含 zh/en 混排、Office/PDF 抽取文本、非纯空白。spec §3.2 要求语料够杂才有意义。

- [ ] **Step 4: Commit（仅代码，数据已 gitignore）**

```bash
git add packages/spike-retrieval/src/bin/build_corpus.rs
git commit -m "BETA-26: build-corpus 从真实目录抽取冻结 corpus.jsonl"
```

---

### Task 3: 手标评测集（Claude 读语料起草 + 用户复核）

> 这是 spec 说的"人是瓶颈"。由 Claude 读 `corpus.jsonl` 起草，用户复核修正。**这是一个协作步骤，不是纯代码步骤。**

**Files:**
- Create: `packages/spike-retrieval/fixtures/evalset/cases.json`（gitignored）
- Create: `packages/spike-retrieval/tests/evalset_integrity.rs`

- [ ] **Step 1: Claude 通读 corpus.jsonl，按 5 桶起草 50~100 条 case**

构造铁律（spec §3.1）：**query 里不能含目标文件名/正文的精确关键词**，否则 FTS5 早就赢了、测不出语义价值。
- 反例（无效）：query「客服SOP」命中 `客服SOP-v3.docx`。
- 正例（有效）：query「那份讲我们怎么处理退款的文档」，目标正文写「退货流程」——词面不重叠。

5 桶各 ~15 条，压在风险点上：
1. `synonym`——同义/改述
2. `concept`——概念/主题
3. `crosslang`——中文 query → 英文文档（小模型最易跌）
4. `ocr`——query 描述图片里的字（测 OCR 噪声；若语料 OCR 文本不足，此桶按实减少并在备忘注明）
5. `content-not-name`——描述正文内容而非文件名

每条 `{id, bucket, query, relevant:[{doc_id, grade(1-3)}]}`。标"按语义**应该**命中什么"，独立于任何检索器当前返回（复用 BETA-13 "标应然不标实然"纪律）。

- [ ] **Step 2: 用户复核修正**

把起草的 cases 交用户过一遍：query 是否真"词面不重叠"、目标文件是否标全、分级是否合理。修正后落 `cases.json`。
> 此步必须有用户确认才进 Task 6——评测集质量直接决定 go/no-go 数字可信度。

- [ ] **Step 3: 写评测集完整性测试（TDD：先红）**

`packages/spike-retrieval/tests/evalset_integrity.rs`：
```rust
use spike_retrieval::{CorpusDoc, EvalCase};
use std::collections::HashSet;

fn load_corpus_ids() -> HashSet<String> {
    let txt = std::fs::read_to_string("fixtures/corpus.jsonl").expect("corpus.jsonl 缺失（先跑 build-corpus）");
    txt.lines().filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str::<CorpusDoc>(l).unwrap().id)
        .collect()
}

fn load_cases() -> Vec<EvalCase> {
    let txt = std::fs::read_to_string("fixtures/evalset/cases.json").expect("cases.json 缺失");
    serde_json::from_str(&txt).expect("cases.json 解析失败")
}

#[test]
fn evalset_is_well_formed() {
    let corpus = load_corpus_ids();
    let cases = load_cases();
    assert!(cases.len() >= 50, "评测集至少 50 条，实际 {}", cases.len());

    let mut ids = HashSet::new();
    let valid_buckets = ["synonym", "concept", "crosslang", "ocr", "content-not-name"];
    for c in &cases {
        assert!(ids.insert(c.id.clone()), "case id 重复: {}", c.id);
        assert!(valid_buckets.contains(&c.bucket.as_str()), "非法 bucket: {}", c.bucket);
        assert!(!c.relevant.is_empty(), "case {} 无相关文件", c.id);
        for r in &c.relevant {
            assert!(corpus.contains(&r.doc_id), "case {} 引用了语料外的 doc_id {}", c.id, r.doc_id);
            assert!((1..=3).contains(&r.grade), "case {} grade 越界: {}", c.id, r.grade);
        }
    }
}
```

- [ ] **Step 4: 跑完整性测试**

Run: `cargo test -p spike-retrieval --test evalset_integrity`
Expected: PASS（cases ≥50、id 唯一、bucket 合法、relevant 全在语料内、grade∈1..3）。失败则回 Step 2 修 cases。

- [ ] **Step 5: Commit（仅代码与测试，cases.json 已 gitignore）**

```bash
git add packages/spike-retrieval/tests/evalset_integrity.rs
git commit -m "BETA-26: 手标评测集完整性测试（cases 由 Claude 起草+用户复核，数据不入库）"
```

---

### Task 4: 纯函数核心——cosine / RRF / Recall@10 / nDCG@10（TDD）

**Files:**
- Modify: `packages/spike-retrieval/src/lib.rs`

- [ ] **Step 1: 写失败测试（先红）**

在 `lib.rs` 底部加：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_basic() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn rrf_fuses_two_rankings() {
        // doc "a" 在两榜都靠前 → 融合后应居首
        let fts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vec = vec!["a".to_string(), "c".to_string(), "d".to_string()];
        let fused = rrf_fuse(&[&fts, &vec], 60);
        assert_eq!(fused[0], "a");
        assert!(fused.contains(&"d".to_string())); // 只在一个榜上的也进结果
    }

    #[test]
    fn recall_at_k_counts_hits() {
        let ranked = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        let relevant: std::collections::HashSet<String> =
            ["y".to_string(), "w".to_string()].into_iter().collect();
        // 命中 y（在前 3），w 没召回 → 2 个相关里命中 1 个
        assert!((recall_at_k(&ranked, &relevant, 3) - 0.5).abs() < 1e-6);
        assert!((recall_at_k(&ranked, &relevant, 1) - 0.0).abs() < 1e-6); // top-1 是 x
    }

    #[test]
    fn ndcg_at_k_rewards_high_rank() {
        // grade: a=3, b=2；理想序 [a,b]
        let grades: std::collections::HashMap<String, u8> =
            [("a".to_string(), 3u8), ("b".to_string(), 2u8)].into_iter().collect();
        let perfect = vec!["a".to_string(), "b".to_string()];
        let swapped = vec!["b".to_string(), "a".to_string()];
        let n_perfect = ndcg_at_k(&perfect, &grades, 10);
        let n_swapped = ndcg_at_k(&swapped, &grades, 10);
        assert!((n_perfect - 1.0).abs() < 1e-6, "理想序 nDCG 应为 1.0");
        assert!(n_swapped < n_perfect, "次优序 nDCG 应更低");
    }
}
```

- [ ] **Step 2: 跑测试确认全红**

Run: `cargo test -p spike-retrieval --lib`
Expected: FAIL（`cosine` / `rrf_fuse` / `recall_at_k` / `ndcg_at_k` 未定义）。

- [ ] **Step 3: 实现四个纯函数**

在 `lib.rs`（测试 mod 之前）加：
```rust
use std::collections::{HashMap, HashSet};

/// 两向量 cosine（输入未必归一化，这里不假设）。
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

/// Reciprocal Rank Fusion：每个榜单贡献 1/(k + rank)，按总分降序。
/// k 常用 60。返回融合后的 doc id 序（去重）。
pub fn rrf_fuse(rankings: &[&Vec<String>], k: usize) -> Vec<String> {
    let mut score: HashMap<String, f64> = HashMap::new();
    for ranking in rankings {
        for (rank, id) in ranking.iter().enumerate() {
            *score.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + (rank as f64 + 1.0));
        }
    }
    let mut ids: Vec<String> = score.keys().cloned().collect();
    ids.sort_by(|a, b| {
        score[b].partial_cmp(&score[a]).unwrap()
            .then_with(|| a.cmp(b)) // 稳定 tie-break
    });
    ids
}

/// Recall@k：前 k 命中的相关文件数 / 相关文件总数。
pub fn recall_at_k(ranked: &[String], relevant: &HashSet<String>, k: usize) -> f64 {
    if relevant.is_empty() { return 0.0; }
    let hits = ranked.iter().take(k).filter(|id| relevant.contains(*id)).count();
    hits as f64 / relevant.len() as f64
}

/// nDCG@k：grade 作增益 (2^g - 1)，折扣 1/log2(rank+1)，对理想 DCG 归一化。
pub fn ndcg_at_k(ranked: &[String], grades: &HashMap<String, u8>, k: usize) -> f64 {
    let dcg = |ids: &[String]| -> f64 {
        ids.iter().take(k).enumerate().map(|(i, id)| {
            let g = *grades.get(id).unwrap_or(&0) as f64;
            ((2f64.powf(g)) - 1.0) / (i as f64 + 2.0).log2()
        }).sum()
    };
    let actual = dcg(ranked);
    let mut ideal_ids: Vec<String> = grades.keys().cloned().collect();
    ideal_ids.sort_by(|a, b| grades[b].cmp(&grades[a]));
    let ideal = dcg(&ideal_ids);
    if ideal == 0.0 { 0.0 } else { actual / ideal }
}
```

- [ ] **Step 4: 跑测试确认全绿**

Run: `cargo test -p spike-retrieval --lib`
Expected: PASS（4 个测试全过）。

- [ ] **Step 5: Commit**

```bash
git add packages/spike-retrieval/src/lib.rs
git commit -m "BETA-26: 纯函数核心 cosine/RRF/Recall@10/nDCG@10（TDD 全绿）"
```

---

### Task 5: Embed 全语料 → vectors.bin

**Files:**
- Create: `packages/spike-retrieval/src/bin/embed_corpus.rs`

- [ ] **Step 1: 写 embed_corpus.rs（含成本数字）**

向量按 `id \t f32,f32,...` 行式存（或定长二进制；这里用简单 JSONL-ish 易调试）：
```rust
use anyhow::Result;
use model_runtime::{llama::LlamaRuntime, LlamaModelRuntime}; // 按实际导出调整
use spike_retrieval::CorpusDoc;
use std::io::{BufRead, Write};
use std::time::Instant;

fn main() -> Result<()> {
    let corpus = "packages/spike-retrieval/fixtures/corpus.jsonl";
    let out = "packages/spike-retrieval/fixtures/vectors.bin";
    let model = std::env::var("EMBED_MODEL")
        .unwrap_or_else(|_| "models/qwen3-embedding-0.6b-q8_0.gguf".into());
    let rt = LlamaRuntime::load(&model)?;

    let f = std::io::BufReader::new(std::fs::File::open(corpus)?);
    let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
    let mut n = 0usize;
    let t0 = Instant::now();
    for line in f.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        let doc: CorpusDoc = serde_json::from_str(&line)?;
        // 截断超长文本避免爆 ctx（探针先简单截断；spec §2 不做分块马拉松）
        let text: String = doc.text.chars().take(4000).collect();
        let v = rt.embed(&text)?;
        let csv = v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        writeln!(w, "{}\t{}", doc.id, csv)?;
        n += 1;
        if n % 200 == 0 { eprintln!("  embedded {n}..."); }
    }
    w.flush()?;
    let secs = t0.elapsed().as_secs_f64();
    let bytes = std::fs::metadata(out)?.len();
    // 成本数字（spec §3.4，白测验证门槛②③④）
    eprintln!("✅ embed {n} docs in {secs:.1}s ({:.1} docs/s) | vectors.bin {:.1}MB",
        n as f64 / secs, bytes as f64 / 1e6);
    Ok(())
}
```

- [ ] **Step 2: 跑全语料 embedding（Mac + metal）**

Run: `cargo run -p spike-retrieval --features metal --release --bin embed-corpus`
Expected: stderr 进度 + 末行成本数字（吞吐 docs/s、总耗时、vectors.bin 大小）。**记下这些数字**——它们是 kill 标准里的"硬 UX kill"（门槛④）判据。
Run: `wc -l packages/spike-retrieval/fixtures/vectors.bin`
Expected: 行数 == 语料文档数。

- [ ] **Step 3: Commit（仅代码）**

```bash
git add packages/spike-retrieval/src/bin/embed_corpus.rs
git commit -m "BETA-26: embed-corpus 全语料 embedding + 成本数字"
```

---

### Task 6: 三组检索 + 分桶指标（探针主输出）

**Files:**
- Create: `packages/spike-retrieval/src/bin/run_retrieval.rs`

- [ ] **Step 1: 写 run_retrieval.rs**

三条路径：
- **A. FTS5-only**：把 corpus 灌进内存 SQLite 的 FTS5 表，对每个 query 跑 `MATCH ... ORDER BY bm25(...)` 取 top-N doc id。
- **B. 向量-only**：query embed → 与 vectors.bin 全量 cosine → 降序 top-N。
- **C. Hybrid**：A、B 各取 top-N 列表 → `rrf_fuse` → top-N。

```rust
use anyhow::Result;
use model_runtime::{llama::LlamaRuntime, LlamaModelRuntime};
use rusqlite::Connection;
use spike_retrieval::*;
use std::collections::{HashMap, HashSet};
use std::io::BufRead;

const TOPK: usize = 10;
const POOL: usize = 50; // 各路径取前 POOL 再融合

fn load_corpus() -> Result<Vec<CorpusDoc>> {
    let f = std::io::BufReader::new(std::fs::File::open(
        "packages/spike-retrieval/fixtures/corpus.jsonl")?);
    let mut v = Vec::new();
    for l in f.lines() { let l = l?; if !l.trim().is_empty() { v.push(serde_json::from_str(&l)?); } }
    Ok(v)
}

fn load_vectors() -> Result<HashMap<String, Vec<f32>>> {
    let f = std::io::BufReader::new(std::fs::File::open(
        "packages/spike-retrieval/fixtures/vectors.bin")?);
    let mut m = HashMap::new();
    for l in f.lines() {
        let l = l?;
        let (id, csv) = l.split_once('\t').unwrap();
        let v: Vec<f32> = csv.split(',').map(|x| x.parse().unwrap()).collect();
        m.insert(id.to_string(), v);
    }
    Ok(m)
}

fn fts_search(conn: &Connection, query: &str) -> Result<Vec<String>> {
    // FTS5 MATCH 语法对特殊字符敏感——用引号包裹每个 term
    let safe: String = query.split_whitespace()
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>().join(" OR ");
    if safe.is_empty() { return Ok(vec![]); }
    let mut stmt = conn.prepare(
        "SELECT id FROM corpus_fts WHERE corpus_fts MATCH ?1 ORDER BY bm25(corpus_fts) LIMIT ?2")?;
    let rows = stmt.query_map(rusqlite::params![safe, POOL as i64], |r| r.get::<_, String>(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn vec_search(qv: &[f32], vectors: &HashMap<String, Vec<f32>>) -> Vec<String> {
    let mut scored: Vec<(String, f32)> = vectors.iter()
        .map(|(id, v)| (id.clone(), cosine(qv, v))).collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored.into_iter().take(POOL).map(|(id, _)| id).collect()
}

fn main() -> Result<()> {
    let corpus = load_corpus()?;
    let vectors = load_vectors()?;
    let cases: Vec<EvalCase> = serde_json::from_str(
        &std::fs::read_to_string("packages/spike-retrieval/fixtures/evalset/cases.json")?)?;
    let model = std::env::var("EMBED_MODEL")
        .unwrap_or_else(|_| "models/qwen3-embedding-0.6b-q8_0.gguf".into());
    let rt = LlamaRuntime::load(&model)?;

    // 建内存 FTS5
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE corpus_fts USING fts5(id UNINDEXED, text, tokenize='trigram');")?;
    {
        let mut ins = conn.prepare("INSERT INTO corpus_fts(id, text) VALUES (?1, ?2)")?;
        for d in &corpus { ins.execute(rusqlite::params![d.id, d.text])?; }
    }

    // 分桶累加 [A,B,C] 的 recall/ndcg
    let mut agg: HashMap<String, [(f64, f64, usize); 3]> = HashMap::new();
    let mut overall = [(0.0f64, 0.0f64, 0usize); 3];

    for c in &cases {
        let relevant: HashSet<String> = c.relevant.iter().map(|r| r.doc_id.clone()).collect();
        let grades: HashMap<String, u8> = c.relevant.iter().map(|r| (r.doc_id.clone(), r.grade)).collect();
        let qv = rt.embed(&c.query)?;

        let a = fts_search(&conn, &c.query)?;
        let b = vec_search(&qv, &vectors);
        let cc = rrf_fuse(&[&a, &b], 60);

        for (i, ranked) in [&a, &b, &cc].iter().enumerate() {
            let rec = recall_at_k(ranked, &relevant, TOPK);
            let ndcg = ndcg_at_k(ranked, &grades, TOPK);
            let e = agg.entry(c.bucket.clone()).or_insert([(0.0,0.0,0);3]);
            e[i].0 += rec; e[i].1 += ndcg; e[i].2 += 1;
            overall[i].0 += rec; overall[i].1 += ndcg; overall[i].2 += 1;
        }
    }

    let names = ["FTS5-only ", "vector    ", "hybrid-RRF"];
    println!("\n=== 分桶 Recall@{TOPK} / nDCG@{TOPK} ===");
    let mut buckets: Vec<&String> = agg.keys().collect(); buckets.sort();
    for bk in buckets {
        println!("\n[{bk}]");
        let e = &agg[bk];
        for i in 0..3 {
            let n = e[i].2.max(1) as f64;
            println!("  {}  R@{TOPK}={:.3}  nDCG@{TOPK}={:.3}", names[i], e[i].0/n, e[i].1/n);
        }
    }
    println!("\n=== 总体 ===");
    for i in 0..3 {
        let n = overall[i].2.max(1) as f64;
        println!("  {}  R@{TOPK}={:.3}  nDCG@{TOPK}={:.3}", names[i], overall[i].0/n, overall[i].1/n);
    }
    // kill 标准关注：hybrid 总体 Recall@10 − FTS5-only ≥ +15pp(GO) / <8pp(NO-GO)
    let delta = (overall[2].0 - overall[0].0) / overall[0].2.max(1) as f64;
    println!("\nhybrid − FTS5 ΔRecall@{TOPK} = {:+.1}pp", delta * 100.0);
    Ok(())
}
```

- [ ] **Step 2: 跑三组检索**

Run: `cargo run -p spike-retrieval --features metal --release --bin run-retrieval`
Expected: 打印分桶 + 总体 Recall@10 / nDCG@10 三组对比 + `hybrid − FTS5 ΔRecall`。

- [ ] **Step 3: 抽查正确性（人工 sanity check）**

确认数字合理：FTS5-only 在"模糊集"上应该偏低（query 词面不重叠）；若 FTS5 也很高，说明评测集没守住"词面不重叠"铁律 → 回 Task 3 修 cases。向量/hybrid 应在 crosslang/content-not-name 桶明显更高。

- [ ] **Step 4: Commit**

```bash
git add packages/spike-retrieval/src/bin/run_retrieval.rs
git commit -m "BETA-26: run-retrieval 三组检索 + 分桶 Recall@10/nDCG@10 + ΔRecall"
```

---

### Task 7: go/no-go 备忘 + 收尾

**Files:**
- Create: `docs/reviews/spike-semantic-retrieval.md`
- Modify: `docs/third-party-licenses.md`（若新增 walkdir）
- Modify: `ROADMAP.md`（BETA-26 状态）、`STATUS.md`（会话日志 + 下一步）

- [ ] **Step 1: 写 go/no-go 备忘**

`docs/reviews/spike-semantic-retrieval.md`，对照 spec §3.5 kill 标准（**跑前定死的**）：

| 结果 | 判定 |
|---|---|
| GO | hybrid 模糊集 Recall@10 ≥ FTS5-only **+15pp** 且精确名子集不低于 FTS5 |
| NO-GO | 提升 < **~8pp**，或 hybrid 拖垮精确名查询 |
| 灰区 8–15pp | "有戏但小模型是天花板"→ 再议换大模型/调分块，不直接进 BETA-15B |
| 硬 UX kill | 16GB 机首索引慢到几小时压不下 → 不论质量 NO-GO |

备忘须含：① 三组分桶指标表（贴 Task 6 输出）；② 成本数字（Task 5：吞吐/耗时/vectors 大小 + 检索 p95）；③ 对照表给结论；④ 标注"保守档/进取档"方向选择**留用户**（spec §5，不预设）；⑤ 注意点：对原返回空的模糊 query，Recall 从 0 拉起才是"10x 手感"，别只盯平均分。

- [ ] **Step 2: 登记新依赖许可（若 walkdir 是新引入）**

Run: `grep -c walkdir docs/third-party-licenses.md`
若为 0，按现有格式追加 walkdir（MIT/Apache-2.0）条目。

- [ ] **Step 3: 全量验证门（CONVENTIONS + 用户 feedback：fmt + clippy + test）**

Run:
```bash
cargo fmt --check
cargo clippy -p spike-retrieval -p model-runtime --features llama-cpp -- -D warnings
cargo test -p spike-retrieval
```
Expected: fmt 干净、clippy 0 warning、test 全绿。

- [ ] **Step 4: 收工（用户说"收工"时按 CONVENTIONS §3）**

更新 STATUS.md（会话日志顶部 + 下一步：备忘结论 + 方向待用户拍板）、ROADMAP.md（BETA-26 状态 done / 结论），中文 commit，向用户确认。
> 注意：探针 GO/NO-GO 出结论后，`packages/spike-retrieval` 是丢弃件——是否删除/保留为耐久评测集（评测集本身是 spec §3.1 说的耐久资产），收工时与用户确认。

---

## Self-Review

**Spec 覆盖：**
- §3.1 评测集 → Task 3（5 桶、词面不重叠铁律、应然标注、完整性测试）✅
- §3.2 真实含噪语料 + 不进 git → Task 2 + Task 0 gitignore ✅
- §3.3 三组检索（FTS5/向量/hybrid-RRF）+ 暴力 cosine → Task 6 ✅
- §3.4 Recall@10/nDCG@10 分桶 + 成本数字 → Task 4(指标) + Task 5(成本) + Task 6(分桶) ✅
- §3.5 kill 标准跑前定死 → Task 7 备忘对照表 ✅
- §5 方向边界留用户 → Task 7 Step 1 ④ ✅
- §7 三项产出（评测集/分桶对比/备忘）→ Task 3 / Task 6 / Task 7 ✅
- §2 YAGNI（无 sqlite-vec/无分块马拉松/单模型）→ 暴力 cosine + 整篇一向量 + 单模型 ✅

**类型一致性：** `CorpusDoc{id,path,text}` / `EvalCase{id,bucket,query,relevant}` / `RelevantDoc{doc_id,grade}` 在 Task 0 定义，Task 2/3/4/6 一致使用；`embed/cosine/rrf_fuse/recall_at_k/ndcg_at_k` 签名 Task 4 定义、Task 1/6 一致调用。✅

**已知不确定（实施时对照 crate 确认，非 placeholder）：** ① llama-cpp-4 embedding API 的确切方法名（`embeddings_seq_ith` 等）以当前版本为准——Task 1 Step 3 已注明；② `LlamaRuntime::load` 构造函数与模块导出路径以 model-runtime 实际为准——Task 1 Step 4 已注明"按实际调整"。这两处是"对照现有代码确认"，不是待填空白。
