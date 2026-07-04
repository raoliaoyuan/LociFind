# BETA-13-G15 `documents`/`pictures` 类型义 vs 位置义消歧 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让英文歧义名词 `documents`/`pictures` 仅在带显式位置标记（`in`/`里`）时才作 location，否则作类型义（→ `file_type`），把 §C1 的 ~10-11 条 v0.9 partial 翻成 pass，且不破 v0.5 byte-equal。

**Architecture:** 单一消歧谓词 `en_ambiguous_noun_is_location`（common.rs）。(a) 在共享的 `parse_location_with_language` 里用该谓词门控 `documents`/`pictures` 命中，类型义时跳过不产 location（file_search + media_search 自动受益）。(b) 仅在 `parse_file_search` 里，对类型义的 `documents` 按 query 语序注入 `FileType::Document`（`pictures`/`images` 已在 EXTENSION_ALIASES，无需注入）。

**Tech Stack:** Rust（crate `locifind-intent-parser`）；评测 crate `locifind-evals`；byte-equal 闸门用 `/tmp/v05check.py`（规范化逐 case 比对，reporter JSON 非确定）。

**前置基线（实现前先记录，供回归对照）：** v0.9 parser-only = 863 pass / 161 partial / 4 fail（§6 86.3%）；v0.5 = 473 pass（byte-equal 基线）。

**关键命令速查：**
- 跑 v0.9 摘要：`cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9`
- 跑 v0.5 摘要：`cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5`
- 取 JSON（byte-equal 用）：`cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/FILE.json`
- byte-equal 比对：`python3 /tmp/v05check.py /tmp/cur.json /tmp/base.json`（期望末行 `diffs=0`）
- 单测：`cargo test -p locifind-intent-parser <test_name>`

---

## Task 0: 记录基线（实现前）

**Files:** 无（只产临时文件）

- [ ] **Step 1: 确认工作树干净并记录 v0.5 baseline JSON**

```bash
git status --short        # 期望空
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/v05_base.json
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 > /tmp/v09_base.txt
tail -5 /tmp/v09_base.txt   # 记下 pass/partial/fail（期望 863/161/4 量级）
```

Expected: 命令成功，`/tmp/v05_base.json` 生成。这是后续每刀 byte-equal 的对照基线。

---

## Task 1: 消歧谓词 `en_ambiguous_noun_is_location`（纯函数）

**Files:**
- Modify: `packages/intent-parser/src/parsers/common.rs`（在 `screenshot_dir_is_location` 之后、`#[cfg(test)]` 之前新增函数；测试加进 line 322 起的 `mod tests`）

- [ ] **Step 1: 写失败单测**

在 `packages/intent-parser/src/parsers/common.rs` 的 `mod tests`（约 line 323）里追加：

```rust
    #[test]
    fn g15_en_ambiguous_noun_location_marker() {
        use super::en_ambiguous_noun_is_location as is_loc;
        // 位置义：前置 in / 后置 里
        assert!(is_loc("find ppt over 100mb in documents", "documents"));
        assert!(is_loc("in documents", "documents"));
        assert!(is_loc("find documents 里的 ppt", "documents"));
        assert!(is_loc("find documents里的 ppt", "documents"));
        assert!(is_loc("find photos in pictures", "pictures"));
        // 类型义：裸 / 句首 / 并列 / 内容子句 / 尾置
        assert!(!is_loc("documents and images", "documents"));
        assert!(!is_loc("documents that mention quarterly revenue", "documents"));
        assert!(!is_loc("code files and documents", "documents"));
        assert!(!is_loc("我昨天 opened 的 documents", "documents"));
        assert!(!is_loc("png and jpg pictures", "pictures"));
        // 不被 "within" 误命中（"within documents" 不含独立 " in documents"）
        assert!(!is_loc("files within documents folder", "documents"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser g15_en_ambiguous_noun_location_marker`
Expected: 编译失败 / FAIL —— `cannot find function en_ambiguous_noun_is_location`。

- [ ] **Step 3: 实现谓词**

在 `common.rs` 的 `screenshot_dir_is_location` 函数（结束于约 line 119）之后插入：

```rust
/// BETA-13-G15：英文歧义名词 `documents`/`pictures` 是否为「位置义」（带显式位置标记）。
///
/// 严格——仅认 v0.5 实证的两种形态：
/// - 前置介词 `in`：句首 `in <kw>` 或 ` in <kw>`（要求 in 前有空格/句首，排除
///   `within documents` 这类子串误命中）；
/// - 后置「里」：`<kw>` 后（允许中间空格）紧跟 `里`（如 `Documents 里` / `documents里`）。
///
/// 其余位置（裸 / 句首 / 并列枚举 / 内容子句 / 尾置名词）= 类型义 → false。
/// 仅对 ascii 关键词 `documents` / `pictures` 调用；中文 alias 不经此门控。
pub(crate) fn en_ambiguous_noun_is_location(lower: &str, kw: &str) -> bool {
    let in_kw = format!("in {kw}");
    if lower.starts_with(&in_kw) || lower.contains(&format!(" {in_kw}")) {
        return true;
    }
    let mut start = 0;
    while let Some(pos) = lower[start..].find(kw) {
        let abs = start + pos;
        if lower[abs + kw.len()..].trim_start().starts_with('里') {
            return true;
        }
        start = abs + kw.len();
    }
    false
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-intent-parser g15_en_ambiguous_noun_location_marker`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add packages/intent-parser/src/parsers/common.rs
git commit -m "BETA-13-G15 步1：documents/pictures 位置义消歧谓词"
```

---

## Task 2: (a) 在 `parse_location_with_language` 里抑制类型义位置

**Files:**
- Modify: `packages/intent-parser/src/parsers/common.rs:17-42`（`parse_location_with_language` 循环体）
- Test: `packages/intent-parser/src/parsers/file_search.rs` 的 `mod tests`（约 line 1599 区）

- [ ] **Step 1: 写失败单测**

在 `file_search.rs` 的 `mod tests` 末尾（最后一个 `}` 之前）追加：

```rust
    #[test]
    fn g15a_bare_english_documents_pictures_not_location() {
        use super::parse_location;
        // 裸 / 句首 / 并列 / 内容子句 → 不作 location
        assert_eq!(parse_location("documents and images"), None);
        assert_eq!(parse_location("documents that mention quarterly revenue"), None);
        assert_eq!(parse_location("documents modified today"), None);
        assert_eq!(parse_location("png and jpg pictures"), None);
        assert_eq!(parse_location("music, videos and pictures"), None);
        // 带 in/里 标记 → 仍作 location（保 v0.5）
        assert!(parse_location("find ppt over 100mb in documents").is_some());
        assert!(parse_location("find documents 里的 ppt").is_some());
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser g15a_bare_english_documents_pictures_not_location`
Expected: FAIL —— 前 5 条 `assert_eq!(..., None)` 失败（当前 parser 给 `Some(location=documents/图片)`）。

- [ ] **Step 3: 实现（门控循环命中）**

在 `common.rs` 的 `parse_location_with_language`（line 18-40）中，把命中分支改为先做歧义门控。
找到现有代码：

```rust
    for a in lexicon::LOCATION_ALIASES {
        for k in a.keywords {
            if word_present(lower, k) && !cjk_location_shadowed(lower, k) {
                let hint = match language {
```

改为：

```rust
    for a in lexicon::LOCATION_ALIASES {
        for k in a.keywords {
            if word_present(lower, k) && !cjk_location_shadowed(lower, k) {
                // BETA-13-G15：英文歧义名词 documents/pictures 仅在带位置标记（in/里）时才作
                // location；否则是类型/复数名词（→ file_type，见 file_search 注入），跳过此命中。
                // 中文 alias（文稿/文档目录/图片目录…）无歧义，不受此门控影响。
                if (*k == "documents" || *k == "pictures")
                    && !en_ambiguous_noun_is_location(lower, k)
                {
                    continue;
                }
                let hint = match language {
```

（其余循环体不变。`en_ambiguous_noun_is_location` 与本函数同模块，直接可见。）

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-intent-parser g15a_bare_english_documents_pictures_not_location`
Expected: PASS。

- [ ] **Step 5: v0.5 byte-equal 闸门**

```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/v05_cur.json
python3 /tmp/v05check.py /tmp/v05_cur.json /tmp/v05_base.json | tail -3
```

Expected: 末行 `diffs=0`（v0.5 全部 `in documents`/`里` 锚点带标记 → location 保留 → 逐字节不变）。
**若 diffs>0：停止**，逐条看 DIFF——预期不应有，若有说明谓词漏判某 v0.5 形态，回 Task 1 修谓词。

- [ ] **Step 6: 提交**

```bash
git add packages/intent-parser/src/parsers/common.rs packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G15 步2(a)：裸 documents/pictures 不作 location"
```

---

## Task 3: (b) 类型义 `documents` → 注入 `FileType::Document`

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs:11-14`（`use super::common::{...}` 加 `en_ambiguous_noun_is_location`）
- Modify: `packages/intent-parser/src/parsers/file_search.rs:34`（`let (extensions, mut file_type)` → `let (mut extensions, mut file_type)`），并在其后调用注入
- Create: `packages/intent-parser/src/parsers/file_search.rs` 新增函数 `inject_type_meaning_document`（放在 `merge_extensions` 之后、约 line 530 区）
- Test: `file_search.rs` 的 `mod tests`

- [ ] **Step 1: 写失败单测**

在 `file_search.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn g15b_type_meaning_documents_sets_file_type() {
        use locifind_search_backend::{FileType, SearchIntent};
        let ft = |q: &str| -> Option<Vec<FileType>> {
            let SearchIntent::FileSearch(fs) = crate::parse(q) else {
                panic!("not file_search: {q}")
            };
            assert_eq!(fs.location, None, "{q} location 应为 None");
            fs.file_type
        };
        // 句首 documents（head fallback 已能给，本测兼验 location 被消除）
        assert_eq!(
            ft("documents that mention quarterly revenue"),
            Some(vec![FileType::Document])
        );
        assert_eq!(ft("documents modified today"), Some(vec![FileType::Document]));
        // 并列：document 需按语序注入到正确位置
        assert_eq!(
            ft("show me documents and images"),
            Some(vec![FileType::Document, FileType::Image])
        );
        assert_eq!(
            ft("code files and documents"),
            Some(vec![FileType::Code, FileType::Document])
        );
        assert_eq!(
            ft("documents, spreadsheets and presentations"),
            Some(vec![
                FileType::Document,
                FileType::Spreadsheet,
                FileType::Presentation
            ])
        );
        // 尾置 documents（mixed）
        assert_eq!(
            ft("我昨天 opened 的 documents"),
            Some(vec![FileType::Document])
        );
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-intent-parser g15b_type_meaning_documents_sets_file_type`
Expected: FAIL —— `show me documents and images` 等给 `Some([Image])`（缺 Document），`我昨天 opened 的 documents` 给 `None`。

- [ ] **Step 3: 实现注入函数**

3a. 在 `file_search.rs` 的 `use super::common::{...}`（line 11-14）里追加 `en_ambiguous_noun_is_location`：

```rust
use super::common::{
    en_ambiguous_noun_is_location, is_cjk, parse_location_with_language, parse_time_fields,
    picture_dir_is_location, screenshot_dir_is_location, word_present,
};
```

3b. 把 line 34 改为 `extensions` 可变，并在其后调用注入：

```rust
    let (mut extensions, mut file_type) = merge_extensions(pos_lower, &all_ext_matches);

    // BETA-13-G15 (b)：类型义的英文 documents → 注入 FileType::Document（按 query 语序、去重）。
    inject_type_meaning_document(pos_lower, &all_ext_matches, &mut file_type, &mut extensions);
```

3c. 在 `merge_extensions` 函数之后（约 line 529 之后）新增：

```rust
/// BETA-13-G15 (b)：类型义的英文 `documents`（或单数 `document`）→ `FileType::Document`，
/// 按 query 语序插入既有 file_type 列表（去重）。`pictures`/`images` 已在 EXTENSION_ALIASES，
/// 无需此处注入。
///
/// 仅当 `documents` 为**类型义**（无 `in`/`里` 位置标记）时生效；位置义由
/// [`parse_location_with_language`] 保留为 location，此处早退、不动 file_type / extensions。
/// 注入使 file_type 达到 ≥2 类时，按 [`merge_extensions`] 同规则把 extensions 置 None
/// （跨范畴多类型不列部分扩展名）。v0.5 无类型义裸 documents（全带 in/里）→ byte-equal 安全。
fn inject_type_meaning_document(
    pos_lower: &str,
    all_matches: &[&'static lexicon::ExtensionAlias],
    file_type: &mut Option<Vec<FileType>>,
    extensions: &mut Option<Vec<String>>,
) {
    let kw = if word_present(pos_lower, "documents") {
        "documents"
    } else if word_present(pos_lower, "document") {
        "document"
    } else {
        return;
    };
    if en_ambiguous_noun_is_location(pos_lower, kw) {
        return; // 位置义，交回 location
    }
    if file_type
        .as_deref()
        .is_some_and(|v| v.contains(&FileType::Document))
    {
        return; // 已含 Document（如「word ... documents」），去重无需注入
    }
    let doc_pos = pos_lower.find(kw).unwrap_or(usize::MAX);
    let mut typed: Vec<(usize, FileType)> = Vec::new();
    if let Some(v) = file_type.as_deref() {
        for ft in v {
            let pos = all_matches
                .iter()
                .filter(|a| a.file_type == *ft)
                .flat_map(|a| a.keywords.iter())
                .filter_map(|k| keyword_position(pos_lower, k))
                .min()
                .unwrap_or(usize::MAX);
            typed.push((pos, *ft));
        }
    }
    typed.push((doc_pos, FileType::Document));
    typed.sort_by_key(|(p, _)| *p);
    let new_types: Vec<FileType> = typed.into_iter().map(|(_, ft)| ft).collect();
    if new_types.len() >= 2 {
        *extensions = None;
    }
    *file_type = Some(new_types);
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-intent-parser g15b_type_meaning_documents_sets_file_type`
Expected: PASS。若某条仍失败（如 `我昨天 opened 的 documents` 的 file_type 或路由），按 systematic-debugging 定位（先确认 `crate::parse` 路由到 FileSearch、再看 `pos_lower`/语言检测）。

- [ ] **Step 5: v0.5 byte-equal 闸门**

```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json > /tmp/v05_cur.json
python3 /tmp/v05check.py /tmp/v05_cur.json /tmp/v05_base.json | tail -3
```

Expected: `diffs=0`。**若 diffs>0：停止**，看 DIFF——若是 `… in documents` 类锚点被注入了 document，说明 `en_ambiguous_noun_is_location` 在 `pos_lower` 上漏判（注意 `pos_lower` 可能截断），回查谓词调用。

- [ ] **Step 6: 提交**

```bash
git add packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G15 步3(b)：类型义 documents 注入 file_type=document"
```

---

## Task 4: 全量回归 + workspace 闸门 + 文档同步

**Files:**
- Modify: `STATUS.md`（当前 Task / 下一步 / 会话日志顶部）
- Modify: `ROADMAP.md`（BETA-13-G15 卡片状态 done + §6 百分比；BETA-14 行 §6 数据）
- Modify: `docs/reviews/beta-13-rebaseline-decisions.md`（C1 标注落地 + d2-en-005/016 follow-up 登记）

- [ ] **Step 1: 跑 v0.9 全量回归**

```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 | tail -8
```

Expected: pass 较基线 863 **只增不减**（预估 +10~11 → ~873~874）；partial 相应下降；fail 不增（仍 4）。
记录精确数字（写进会话日志）。**若 pass 反降或 fail 增：停止排查**（不得用改 coverage 凑指标）。

- [ ] **Step 2: workspace 全量测试**

```bash
cargo test --workspace 2>&1 | tail -15
```

Expected: 全部 passed / 0 failed（含本次 3 个新单测）。

- [ ] **Step 3: clippy + fmt 闸门**

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -5
cargo fmt --check 2>&1 | tail -5
```

Expected: clippy 0 warning；fmt 无 diff（无输出）。**若 fmt 有 diff：`cargo fmt` 后重测再提交。**

- [ ] **Step 4: 同步文档**

- `ROADMAP.md` BETA-13-G15 卡片：`not_started` → `done`（写实际 pass 增量 + §6 新百分比 + d2-en-005/016 follow-up 说明）；BETA-14 行 §6 数据更新。
- `docs/reviews/beta-13-rebaseline-decisions.md` §C1：标注「(b) 上下文消歧已落地」+ 记录 d2-en-005/016 因多类型 ext 约定（C2/coverage）留 partial。
- `STATUS.md`：当前 Task 更新（G15 done 或下一步）、会话日志顶部追加（按 CONVENTIONS §3 格式，署名 `Claude Code (Opus 4.8)`，含 v0.9 数字 / v0.5 byte-equal=0 / 未尽事宜）。

- [ ] **Step 5: 收工提交（待用户确认）**

```bash
git add -A
git commit -m "BETA-13-G15：documents/pictures 类型义vs位置义消歧（v0.9 863→XXX，§6 86.3%→XX%）"
git log --oneline -5
```

提交后向用户汇报：v0.9 实际增量、v0.5 byte-equal=0、workspace/clippy/fmt 全绿、d2-en-005/016 留 follow-up 的诚实边界。**push 等用户授权。**

---

## Self-Review（已核对）

**Spec 覆盖：**
- §3.1 谓词 → Task 1 ✓
- §3.2 (a) 位置抑制 → Task 2 ✓
- §3.3 (b) file_type 注入 → Task 3 ✓
- §2 byte-equal 硬约束 → Task 2/3 Step 5 闸门 + Task 0 基线 ✓
- §4 诚实预估（d2-en-005/016 留 partial）→ Task 4 Step 1/4 登记 follow-up，不凑指标 ✓
- §5 测试策略（TDD + byte-equal + 回归 + workspace）→ 各 Task 落实 ✓
- §6 范围边界（不动中文 alias / 不改 coverage / 严格标记）→ 谓词只门控 ascii documents/pictures、无 coverage 改动 ✓

**占位符扫描：** 无 TBD/TODO；所有代码步给出完整代码与命令。

**类型一致性：** 谓词名 `en_ambiguous_noun_is_location` 在 Task 1 定义、Task 2/3 引用一致；注入函数 `inject_type_meaning_document` 签名（`pos_lower`, `all_matches: &[&'static ExtensionAlias]`, `&mut Option<Vec<FileType>>`, `&mut Option<Vec<String>>`）与调用处一致；复用既有 `keyword_position`（file_search.rs:533）、`word_present`、`en_ambiguous_noun_is_location`。
