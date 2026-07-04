# BETA-13-G follow-up：size between + 内容截图多关键词 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修两条 BETA-13-G8 遗留的 parser 近邻缺口——size 区间（`archives between 10 and 100 MB`）+ 内容截图多关键词变体（`截图里同时出现X和Y` / `screenshot with both X and Y`），全程 v0.5 byte-equal 守护。

**Architecture:** 纯增量规则扩展，改 `packages/intent-parser/src/parsers/{file_search.rs, media_search.rs}`。#1 给 `parse_size` 加区间正则→`SizeExpression::Between`；#2 给 `detect_content_clause` 加 `同时出现/with both` 引导词、在 file_search 关键词注入处对 both 标记按 `和`/`and` 拆多关键词。每条只命中 v0.5 不存在的新形态。

**Tech Stack:** Rust（regex crate + OnceLock），评测 `cargo run -p locifind-evals --bin evals`。

**关联 spec：** [docs/superpowers/specs/2026-06-19-beta-13-g-followup-size-between-content-screenshot-design.md](../specs/2026-06-19-beta-13-g-followup-size-between-content-screenshot-design.md)

---

## 基线与 byte-equal 闸门（每个 task 用）

```bash
# 首次存基线（若 /tmp/v05-baseline.json 不在或想重置）：
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-baseline.json
```
当前 v0.9 基线：**765 pass / 199 partial / 36 fail**；v0.5 = **473 pass**。

**byte-equal 规范化闸门**（reporter JSON 非确定，**禁用裸 diff**；脚本已存在 `/tmp/v05check.py`）：
```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-now.json
python3 /tmp/v05check.py   # 必须输出 V0.5 BYTE-EQUAL OK（473，0 差异）
```
若 `/tmp/v05check.py` 不存在，内容见 spec 引用——按 case.id 建表，比 `(result.type, sorted diff, actual_variant, sorted actual_json)`，忽略 elapsed_ms。

v0.9 计数：
```bash
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | python3 -c "import json,sys;from collections import Counter;print(dict(Counter(x['result']['type'] for x in json.load(sys.stdin))))"
```

每个 task 收尾：`cargo fmt --check`、`cargo clippy -p locifind-intent-parser -- -D warnings`、`cargo test -p locifind-intent-parser`。

---

## Task 1: Fix #1 — size 区间（between A and B）

**实测目标**：`archives between 10 and 100 MB` → `file_type=archive`（BETA-13-G8 已对）、`size=Between{min:10,max:100,unit:MB}`、keywords=null。当前 `parse_size` 对 `between…and…` 返回 None。

**Files:**
- Modify: `packages/intent-parser/src/parsers/file_search.rs`（`parse_size`，约 line 416-470，在返回 `None` 前加区间分支）
- Test: 同文件 `#[cfg(test)]` 模块

- [ ] **Step 1: 写失败测试**

```rust
#[test]
fn parse_size_between_range() {
    use locifind_search_backend::{SearchIntent, SizeExpression, SizeUnit};
    // 端到端：archives between 10 and 100 MB
    let SearchIntent::FileSearch(fs) = crate::parse("archives between 10 and 100 MB") else {
        panic!("应 file_search");
    };
    assert_eq!(
        fs.size,
        Some(SizeExpression::Between { min: 10.0, max: 100.0, unit: SizeUnit::Mb }),
        "size between"
    );
    // 纯函数：中文「到」
    assert_eq!(
        super::parse_size("10 到 100 mb"),
        Some(SizeExpression::Between { min: 10.0, max: 100.0, unit: SizeUnit::Mb })
    );
    // 顺序规范化：大数在前也应 min≤max
    assert_eq!(
        super::parse_size("between 100 and 10 gb"),
        Some(SizeExpression::Between { min: 10.0, max: 100.0, unit: SizeUnit::Gb })
    );
}
```
> 注：`parse_size` 是 `pub(crate)`，测试模块内用 `super::parse_size` 调纯函数；端到端用 `crate::parse`。`SizeUnit` 变体名核对 common/lib.rs（Mb/Gb 等）。

- [ ] **Step 2: 跑确认失败**

Run: `cargo test -p locifind-intent-parser parse_size_between_range`
Expected: FAIL（size 为 None）

- [ ] **Step 3: 实现区间识别**

在 `parse_size` 内、`// "几个 G"` 启发式分支**之前**（在三条 GT/LT 之后）加：
```rust
    // BETA-13-G follow-up：区间 size —— "between A and B unit" / "A 到 B 单位" / "A-B 单位"。
    // 单位取末位、两数共用；min/max 规范化。
    static RE_BETWEEN: OnceLock<Regex> = OnceLock::new();
    let re_between = RE_BETWEEN.get_or_init(|| {
        Regex::new(r"(?:between\s+)?(\d+(?:\.\d+)?)\s*(?:and|到|至|-|~)\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)")
            .expect("regex valid")
    });
    if let Some(cap) = re_between.captures(lower) {
        let a: f64 = cap[1].parse().ok()?;
        let b: f64 = cap[2].parse().ok()?;
        let unit = parse_size_unit(&cap[3])?;
        let (min, max) = if a <= b { (a, b) } else { (b, a) };
        return Some(SizeExpression::Between { min, max, unit });
    }
```
> 放置顺序很关键：必须在 GT/LT 正则之后（避免 `larger than 1 GB` 等被误吞），在「几个 G」「大文件」启发式之前。`between` 前缀可选——使 `10 到 100 mb` 也命中；但裸 `A-B` 形态因 `-` 可能与负数/范围歧义，已用单位锚定收窄。确保 `use locifind_search_backend::SizeExpression;` 等已在作用域（parse_size 已用 SizeExpression，无需新增 import）。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser parse_size_between_range`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（v0.5 无 between/到 size 形态，已 grep 确认 0 冲突）
Run v0.9 → fail 应 −1（archives between 转 pass）、无新增 fail

- [ ] **Step 5: fmt/clippy/test + commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G follow-up Fix1：parse_size 支持区间 between A and B"
```

---

## Task 2: Fix #2 — 内容截图多关键词（同时出现 / with both）

**实测目标**：
- `截图里同时出现订单号和金额的` → `file_type=screenshot, keywords=["订单号","金额"]`
- `screenshot with both order id and tracking number` → `file_type=screenshot, keywords=["order id","tracking number"]`

当前 `detect_content_clause` 引导词不含 `同时出现/with both`，这两条仍误路由 media_search。且需按 `和`/`and` 拆多关键词。

**关键边界**：**仅 both/同时 标记才拆多关键词**；常规内容子句（`里面有提到甲方乙方的协议`→`["甲方乙方"]`）保持单关键词不拆。

**Files:**
- Modify: `packages/intent-parser/src/parsers/media_search.rs`（`detect_content_clause` 加 `同时出现/同时包含/with both` 引导词；新增 pub(crate) `content_clause_is_multi`）
- Modify: `packages/intent-parser/src/parsers/file_search.rs`（`extract_filesearch_keywords` 内容子句短路处，约 line 523-527：both 标记则拆多关键词）
- Test: 两文件 `#[cfg(test)]` 模块

- [ ] **Step 1: 写失败测试**（media_search.rs 测试模块）

```rust
#[test]
fn content_screenshot_both_multi_keyword() {
    use locifind_search_backend::{FileType, SearchIntent};
    let cases = [
        ("截图里同时出现订单号和金额的", vec!["订单号", "金额"]),
        ("screenshot with both order id and tracking number", vec!["order id", "tracking number"]),
    ];
    for (q, kws) in cases {
        let SearchIntent::FileSearch(fs) = crate::parse(q) else {
            panic!("{q} 应路由 file_search");
        };
        assert_eq!(fs.file_type, Some(vec![FileType::Screenshot]), "{q} file_type");
        assert_eq!(
            fs.keywords,
            Some(kws.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
            "{q} keywords"
        );
    }
}

#[test]
fn content_clause_single_not_split_on_he() {
    // 边界：常规内容子句（无 both/同时 标记）即便含「和」也不拆
    use locifind_search_backend::SearchIntent;
    if let SearchIntent::FileSearch(fs) = crate::parse("截图里写着甲方和乙方的") {
        // 单关键词保留（含「和」整体），不因「和」拆碎
        assert_eq!(fs.keywords, Some(vec!["甲方和乙方".to_owned()]), "无 both 标记不拆");
    } else {
        panic!("应 file_search");
    }
}
```

- [ ] **Step 2: 跑确认失败**

Run: `cargo test -p locifind-intent-parser content_screenshot_both_multi_keyword`
Expected: FAIL（`同时出现` 未被识别 → 仍 media_search）

- [ ] **Step 3a: detect_content_clause 加 both 引导词**

在 `detect_content_clause`（media_search.rs）ZH 正则的引导词组加 `同时出现|同时包含`：
```rust
        Regex::new(r"(?:里写着|里写了|写着|写了|里提到|提到|里面有|里面提到|同时出现|同时包含)\s*([^，。,]+?)(?:的那张|的那个|的报错|的提示|的错误提示|的)?$")
            .expect("content clause zh regex")
```
EN 正则加 `with both` 分支（与 `that says/...` 并列；`with both` 后直接接内容，无需 that 锚）：
```rust
        Regex::new(r"(?i)(?:that\s+(?:says|mentions?|shows?)|with both)\s+(.+?)\s*$")
            .expect("content clause en regex")
```

- [ ] **Step 3b: 新增 both 标记判定**

在 media_search.rs 加 pub(crate) 函数：
```rust
/// BETA-13-G follow-up：内容子句是否含「both/同时」语义（多对象并列）。
/// 仅此类才把内容按 和/and 拆多关键词；常规内容子句保持单关键词。
pub(crate) fn content_clause_is_multi(input: &str) -> bool {
    let lower = input.to_lowercase();
    input.contains("同时出现") || input.contains("同时包含") || lower.contains("with both")
}
```

- [ ] **Step 3c: file_search 注入处按 both 拆分**

在 `extract_filesearch_keywords`（file_search.rs，当前约 line 523-527）的内容子句短路改为：
```rust
    if super::media_search::has_screenshot_word(lower) {
        if let Some(phrase) = super::media_search::detect_content_clause(input) {
            if super::media_search::content_clause_is_multi(input) {
                // both/同时 → 按 和 / and 拆多关键词
                let parts: Vec<String> = phrase
                    .split(|c| c == '和')
                    .flat_map(|p| p.split(" and "))
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect();
                if !parts.is_empty() {
                    return Some(parts);
                }
            }
            return Some(vec![phrase]);
        }
    }
```
> 拆分对中文 `和`（char）与英文 ` and `（含空格，避免切断 "brand"）双切；trim + 过滤空串。`phrase` 已由 detect_content_clause 剥掉尾部「的」。

- [ ] **Step 4: 跑测试 + byte-equal**

Run: `cargo test -p locifind-intent-parser content_screenshot_both_multi_keyword content_clause_single_not_split_on_he`
Expected: PASS
Run byte-equal 闸门 → `V0.5 BYTE-EQUAL OK`（v0.5 无 `同时出现/with both`，已 grep 确认 0 冲突）
Run v0.9 → fail 应 −2、无新增 fail；**特别核对** BETA-13-G8 已通过的内容截图单关键词 case 未被拆碎（如 `截图里写着已支付的`→`["已支付"]` 仍单元素）

- [ ] **Step 5: fmt/clippy/test + commit**

```bash
cargo fmt && cargo clippy -p locifind-intent-parser -- -D warnings && cargo test -p locifind-intent-parser
git add packages/intent-parser/src/parsers/media_search.rs packages/intent-parser/src/parsers/file_search.rs
git commit -m "BETA-13-G follow-up Fix2：内容截图 同时出现/with both 多关键词"
```

---

## Task 3: 整体回归门

- [ ] **Step 1: 全量验证**

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
cargo test --workspace
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.5 --json 2>/dev/null > /tmp/v05-now.json && python3 /tmp/v05check.py
cargo run -q -p locifind-evals --bin evals -- --fixtures v0.9 --json 2>/dev/null | python3 -c "import json,sys;from collections import Counter;print(dict(Counter(x['result']['type'] for x in json.load(sys.stdin))))"
```
Expected: fmt 净 / clippy 0 / test 全绿 / **V0.5 BYTE-EQUAL OK** / v0.9 pass ≥ 768、fail ≤ 33（无新增）。

- [ ] **Step 2: 如实记录实测增益**（v0.9 765 → 实测，不凑指标）。

> 收工（STATUS/ROADMAP/合并/最终 commit）由用户说「收工」时执行，不在本计划内。

---

## Self-Review 检查

- **Spec 覆盖**：Fix #1（Task 1）/ Fix #2（Task 2）/ 验证（Task 3）全覆盖。
- **byte-equal**：两 task 均含 `/tmp/v05check.py` 闸门；v0.5 对 between/到/同时出现/with both 已 grep 确认 0 冲突。
- **类型一致**：`SizeExpression::Between{min,max,unit}`（common/lib.rs:786）、`content_clause_is_multi`（media_search，pub(crate)）被 file_search 调用、`has_screenshot_word`/`detect_content_clause` 沿用 BETA-13-G8 既有 pub(crate) 接口；测试用 `crate::parse` 端到端。
- **无占位符**：每步给了具体正则/函数/真实 query 与断言。
- **边界明确**：多关键词拆分仅 both 标记触发，含 `content_clause_single_not_split_on_he` 负样本守护。
