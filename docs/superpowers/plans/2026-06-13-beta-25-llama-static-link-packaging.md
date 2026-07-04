# BETA-25 model-fallback 静态链接打包修复 — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `--features model-fallback(-metal)` 打出的安装包无需手工修补即可启动并跑模型推理——通过把 llama 改为静态链接消除动态库打包/rpath/签名问题。

**Architecture:** 病根是 `llama-cpp-4` 把 `dynamic-link` 放进默认 feature、被 `model-runtime` 继承，导致产出一组 `@rpath/*.dylib` 而二进制无 `LC_RPATH`。改 `default-features = false` 关掉 `dynamic-link`，llama 全部静态进二进制；一处 Cargo 改动同时覆盖 macOS + Windows。先 spike 证伪（静态+metal 能否编过、能否推理），通过后固化。

**Tech Stack:** Rust / Cargo features / llama-cpp-4 / llama-cpp-sys-4（cmake 构建 llama.cpp）/ Tauri 2 bundler / `otool`（macOS dylib 检查）。

**关键路径/前置：**
- 已部署模型：`~/Library/Application Support/LociFind/models/qwen3-0.6b-q4_k_m.gguf`（= `dirs::data_dir()/LociFind/models/`，bundled .app 会从此处加载）。
- `packages/model-runtime/src/llama.rs` 仅用 `llama_cpp_4::{context, llama_backend, llama_batch, model, sampling, token}`，**未用 mtmd API**（已 grep 确认 → `mtmd` 可去）。
- 构建慢：改 llama feature 会触发 llama.cpp cmake 全量重编（数分钟）；若撞 `target/llama-cmake-cache` 旧配置缓存报错（如 `llama-common.X not found`），按 BETA-23 经验清缓存重建：`rm -rf apps/desktop/src-tauri/target/llama-cmake-cache target/llama-cmake-cache 2>/dev/null`（路径以实际 workspace target 为准）。

**API 速查（spike 测试要用）：**
```rust
// packages/model-runtime/src/lib.rs
LlamaLoader::new() -> Result<LlamaLoader, ModelError>            // llama.rs:47
ModelLoader::load(&self, &Path, &ModelLoadParams) -> Result<Box<dyn LlamaModelRuntime>, ModelError>
LlamaModelRuntime::generate(&self, &str, &GenerateParams) -> Result<String, ModelError>
ModelLoadParams { gpu_layers: u32, context_size: u32 }  // derive Default
GenerateParams { .. }                                   // derive Default
```

---

## Task 1: Spike — 静态链接 + metal，证伪「能编过 + 零 dylib + 能推理」

> 这是去风险闸门。本 task 同时落地真正的 Cargo 修复 + 一个永久的 `#[ignore]` 真机推理冒烟测试。
> 任一步骤失败且无法靠调整 feature 组合解决 → 停下，按 spec §2.B 退路线 B 并回报。

**Files:**
- Modify: `packages/model-runtime/Cargo.toml`（`llama-cpp-4` 依赖行）
- Create/Modify test: `packages/model-runtime/src/tests.rs`（追加 `#[ignore]` 真机推理测试）

- [ ] **Step 1: 改 Cargo —— 关默认 features（去 dynamic-link/mtmd/openmp）**

`packages/model-runtime/Cargo.toml`，把：
```toml
llama-cpp-4 = { version = "0.3.0", optional = true }
```
改为：
```toml
# BETA-25：default-features=false 去掉 llama-cpp-4 默认的 `dynamic-link`（病根：产出
# @rpath/*.dylib 而二进制无 LC_RPATH → 安装包启动即崩）。同去 `mtmd`(未用多模态)/`openmp`
# (避免牵出 libgomp/libomp 动态依赖)。metal 仍由 model-runtime 自身 `metal` feature 接线供给。
llama-cpp-4 = { version = "0.3.0", optional = true, default-features = false }
```

- [ ] **Step 2: 追加永久的真机推理冒烟测试（headless 验证静态 llama 能推理）**

在 `packages/model-runtime/src/tests.rs` 末尾追加（仅 `llama-cpp` feature 下编译、默认 `#[ignore]`）：
```rust
// BETA-25：真机冒烟——验证静态链接的 llama 后端能加载已部署模型并产出非空生成。
// 默认 ignore（需真实 gguf + Metal）。运行：
//   cargo test -p locifind-model-runtime --features llama-cpp,metal beta25_static_llama_smoke -- --ignored --nocapture
#[cfg(feature = "llama-cpp")]
#[test]
#[ignore = "需真实 gguf 模型 + llama-cpp 后端；CI 无模型时跳过"]
fn beta25_static_llama_smoke() {
    use crate::{GenerateParams, LlamaLoader, ModelLoadParams, ModelLoader};
    use std::path::PathBuf;

    let model_path = dirs::data_dir()
        .expect("data_dir")
        .join("LociFind/models/qwen3-0.6b-q4_k_m.gguf");
    assert!(
        model_path.exists(),
        "模型不存在：{}（先部署 BETA-24 模型）",
        model_path.display()
    );

    let loader = LlamaLoader::new().expect("LlamaLoader::new");
    let model = loader
        .load(
            &model_path,
            &ModelLoadParams {
                gpu_layers: 99,
                context_size: 2048,
            },
        )
        .expect("load model");
    let out = model
        .generate("你好", &GenerateParams::default())
        .expect("generate");
    assert!(!out.trim().is_empty(), "生成结果为空");
}
```
> 注：`dirs` 是否为 model-runtime 依赖需确认；若不是，改用环境变量 `LOCIFIND_BETA25_MODEL` 或硬编码绝对路径常量（仅本机 ignore 测试用，不进默认构建）。先查 `grep -n dirs packages/model-runtime/Cargo.toml`，缺则用 `std::env::var("LOCIFIND_BETA25_MODEL")` 取路径并在缺失时 `eprintln!` 跳过断言。

- [ ] **Step 3: 编译 model-runtime（llama-cpp 后端）确认静态链接能编过**

Run:
```bash
cd /Users/alice/Work/LocalFind
cargo build -p locifind-model-runtime --features llama-cpp,metal 2>&1 | tail -20
```
Expected: 编译成功。若撞 cmake 缓存旧配置错误 → 清 `*/target/llama-cmake-cache` 重试。
若静态链接报符号/链接错误且补回 `mtmd`/`openmp` 仍不解 → 停，退路线 B 回报。

- [ ] **Step 4: headless 跑真机推理冒烟，确认静态 llama 真能推理**

Run:
```bash
cargo test -p locifind-model-runtime --features llama-cpp,metal beta25_static_llama_smoke -- --ignored --nocapture 2>&1 | tail -30
```
Expected: PASS（加载模型 + 生成非空）。FAIL → 诊断（feature 缺失 / 模型路径 / Metal）后再判 A/B。

- [ ] **Step 5: 打 metal release bundle，确认零 dylib 依赖**

Run（构建较慢，耐心等）：
```bash
cd /Users/alice/Work/LocalFind/apps/desktop
npm run tauri -- build --features model-fallback-metal 2>&1 | tail -20
```
然后检查 bundled 二进制的动态依赖：
```bash
cd /Users/alice/Work/LocalFind
BIN="$(find . -path '*/bundle/macos/LociFind.app/Contents/MacOS/locifind-desktop' | head -1)"
echo "binary: $BIN"
otool -L "$BIN" | grep -iE "ggml|llama|mtmd|gomp|omp\.dylib" && echo "!!! 仍有 dylib 残留 → 静态未生效" || echo "OK: 零 llama/ggml dylib 残留"
```
Expected: 打印 `OK: 零 llama/ggml dylib 残留`。若仍有残留 → 静态未生效，诊断 feature 透传。

- [ ] **Step 6: 启动未修补的 .app，确认不再启动即崩**

Run:
```bash
APP="$(find /Users/alice/Work/LocalFind -path '*/bundle/macos/LociFind.app' | head -1)"
open "$APP"
sleep 6
pgrep -f "LociFind.app/Contents/MacOS/locifind-desktop" >/dev/null && echo "OK: 进程存活，未启动即崩" || echo "!!! 进程已退出（可能仍崩）"
osascript -e 'tell application "LociFind" to quit' 2>/dev/null || pkill -f "LociFind.app/Contents/MacOS/locifind-desktop"
```
Expected: `OK: 进程存活，未启动即崩`。
> GUI 内「问题 4 query 跑出模型补全」的端到端验证留 Task 5 用户手测；本步只验证「无需手工修补即可启动」。

- [ ] **Step 7: Commit**

```bash
cd /Users/alice/Work/LocalFind
git add packages/model-runtime/Cargo.toml packages/model-runtime/src/tests.rs
git commit -m "fix(beta-25): llama-cpp-4 关默认 features 走静态链接（消除 @rpath dylib 打包缺口）+ 真机推理冒烟测试"
```

---

## Task 2: 固化 + 三 feature 形态门禁

> 确认改动在「关 feature / model-fallback / model-fallback-metal」三态都干净。

**Files:**
- 无新增；验证 Task 1 的改动不破坏默认构建。

- [ ] **Step 1: 默认构建（不开 model-fallback）clippy + test**

Run:
```bash
cd /Users/alice/Work/LocalFind
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -15
cargo test --workspace 2>&1 | tail -20
```
Expected: clippy 0 警告；test 零回归（platform-macos 既有 2 个 Windows 形态失败属预存，与本改动无关）。

- [ ] **Step 2: model-fallback（非 metal）形态 clippy**

Run:
```bash
cargo clippy -p locifind-model-runtime -p locifind-desktop --features locifind-desktop/model-fallback --all-targets -- -D warnings 2>&1 | tail -15
```
Expected: 0 警告。（注：feature 经 desktop crate 透传；若路径不便，退化为 `cd apps/desktop/src-tauri && cargo clippy --features model-fallback --all-targets -- -D warnings`。）

- [ ] **Step 3: model-fallback-metal 形态 clippy**

Run:
```bash
cd /Users/alice/Work/LocalFind/apps/desktop/src-tauri
cargo clippy --features model-fallback-metal --all-targets -- -D warnings 2>&1 | tail -15
```
Expected: 0 警告。

- [ ] **Step 4: fmt 检查**

Run:
```bash
cd /Users/alice/Work/LocalFind && cargo fmt --check 2>&1 | tail -5
```
Expected: 无输出（全部已格式化）。

- [ ] **Step 5: Commit（若上述步骤产生任何修复；纯验证无改动则跳过）**

```bash
git add -A && git commit -m "chore(beta-25): 三 feature 形态 fmt/clippy/test 门禁通过"
```

---

## Task 3: dev 窗口标题区分

> 手测时安装版与 dev 同名同 bundle id，反复驱动错窗口。debug 构建标题加 `(dev)`。

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`（`.setup(...)` 闭包内）

- [ ] **Step 1: 在 setup 闭包内、debug 构建时改主窗口标题**

在 `apps/desktop/src-tauri/src/main.rs` 的 `.setup(move |app| { ... })` 闭包体内、`app.manage(...)` 之前任意稳定处，插入：
```rust
// BETA-25：dev 构建窗口标题加后缀，避免与安装版（同名同 bundle id）在手测时混淆。
#[cfg(debug_assertions)]
if let Some(win) = app.get_webview_window("main") {
    let _ = win.set_title("LociFind (dev)");
}
```
> `get_webview_window` 已在 `shortcut.rs` 用过同款 API，确认 `tauri::Manager` trait 在 main.rs 已 `use`（否则补 `use tauri::Manager;`）。

- [ ] **Step 2: 编译确认（debug，快）**

Run:
```bash
cd /Users/alice/Work/LocalFind/apps/desktop/src-tauri && cargo build 2>&1 | tail -10
```
Expected: 编译成功。

- [ ] **Step 3: clippy + fmt**

Run:
```bash
cargo clippy --all-targets -- -D warnings 2>&1 | tail -8
cd /Users/alice/Work/LocalFind && cargo fmt --check 2>&1 | tail -3
```
Expected: 0 警告、无 fmt 输出。

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src-tauri/src/main.rs
git commit -m "feat(beta-25): dev 构建窗口标题加 (dev) 后缀，手测区分安装版"
```

---

## Task 4: CI 注释更新 + 第三方许可复核 + 手测场景登记

**Files:**
- Modify: `.github/workflows/release-windows.yml`（构建说明注释）
- Modify: `docs/third-party-licenses.md`（复核 llama.cpp 静态链接条目）
- Modify: `docs/manual-test-scenarios.md`（追加 BETA-25 节）

- [ ] **Step 1: 更新 release CI 注释（不再需要 DLL 打包）**

`.github/workflows/release-windows.yml` 的「构建说明（model-fallback feature）」注释块内，追加一行说明：
```yaml
      #   - BETA-25：llama-cpp-4 已关默认 dynamic-link 走静态链接，安装包不再依赖
      #     外部 llama DLL（修复此前 NSIS 缺 DLL 的隐患）；args 保持 --features model-fallback。
```
> `args: --features model-fallback` 一行**不改**。

- [ ] **Step 2: 复核第三方许可**

Run:
```bash
grep -niE "llama|ggml|qwen" /Users/alice/Work/LocalFind/docs/third-party-licenses.md | head
```
确认 llama.cpp/ggml 已登记。若已登记：在其条目补注「（BETA-25 起静态链接进二进制）」；静态链接不改变 MIT 许可义务，无需新增条目。若**未**登记 → 按文件既有格式补一条 llama.cpp（MIT）+ ggml（MIT）。

- [ ] **Step 3: 追加手测场景 BETA-25**

在 `docs/manual-test-scenarios.md` 追加一节（沿用文件既有标题层级与格式）：
```markdown
## BETA-25：静态链接打包真机验收（macOS）

前置：`--features model-fallback-metal` 打 release bundle；模型已部署
`~/Library/Application Support/LociFind/models/qwen3-0.6b-q4_k_m.gguf`。

1. **未修补启动**：直接双击 `LociFind.app`（不做任何 Frameworks/rpath/重签手工补丁），应正常打开，不报
   `Library not loaded: @rpath/libggml-base.0.dylib`。
2. **零 dylib 依赖**：`otool -L <app>/Contents/MacOS/locifind-desktop | grep -iE "ggml|llama|mtmd"` 应无输出。
3. **问题 4 端到端**：搜「2025年的会议纪要文件名包含运维」→ 触发模型 fallback → 结果出现「模型补全」徽标、
   补出「会议纪要」关键词（与 BETA-24 手测一致）。
4. **无模型降级**：临时移走模型文件 → 同一 query 不崩、静默降级 parser-only。

Windows（留下个 Windows 会话）：CI 打出的 NSIS 安装包安装后启动不缺 DLL、模型放置后问题 4 端到端通。
```

- [ ] **Step 4: Commit**

```bash
cd /Users/alice/Work/LocalFind
git add .github/workflows/release-windows.yml docs/third-party-licenses.md docs/manual-test-scenarios.md
git commit -m "docs(beta-25): CI 注释更新 + 许可复核 + 手测场景登记"
```

---

## Task 5: 真机验收（用户驱动 GUI）+ STATUS/ROADMAP 收工登记

> 代码层已验证；本 task 是用户手测验收红线 + 文档收尾。

**Files:**
- Modify: `STATUS.md`、`ROADMAP.md`（BETA-25 卡片状态）

- [ ] **Step 1: 引导用户做 BETA-25 真机验收**

向用户给出 `docs/manual-test-scenarios.md` BETA-25 节的 4 步，请用户用 Task 1 已打出的未修补 .app 验证问题 4 端到端 + 「模型补全」徽标。等待用户回报结果。
（Task 1 Step 5/6 已机器验证「零 dylib + 启动不崩」；本步补「GUI 内推理端到端」这一用户可见铁证。）

- [ ] **Step 2: 据验收结果登记 ROADMAP BETA-25 卡片**

`ROADMAP.md` 的 BETA-25 卡片 `状态` 改为 `done（2026-06-13，macOS 静态链接验证通过；Windows NSIS 留下个 Windows 会话装包实测）`，正文摘要根因+方案+验收（参 spec）。

- [ ] **Step 3: 更新 STATUS.md**

按 CONVENTIONS §3：当前 Task 区写 BETA-25 done；「下一步」区移除/更新 BETA-25 条，保留「Windows NSIS 装包实测」留待项；会话日志顶部追加本会话条目（署名 `Claude Code (Opus 4.8)`）。

- [ ] **Step 4: 收工 commit**

```bash
cd /Users/alice/Work/LocalFind
git add STATUS.md ROADMAP.md
git commit -m "收工(beta-25): 静态链接打包修复 done + STATUS/ROADMAP 登记 + 会话日志"
```

- [ ] **Step 5: 向用户确认提交内容**

列出本会话 commit 链，请用户确认。

---

## 自审记录（写计划后对照 spec）

- **Spec §3 核心改动** → Task 1 Step 1。
- **Spec §3 feature 取舍（mtmd/openmp）** → Task 1 Step 1 注释去掉、Step 3/4 实测兜底（补回逻辑）。
- **Spec §4 dev 标题** → Task 3。
- **Spec §5.1 spike 证伪** → Task 1 全程（含退 B 闸门）。
- **Spec §5.2 三 feature 形态门禁** → Task 2。
- **Spec §5.3 CI** → Task 4 Step 1。
- **Spec §5.5 文档/许可** → Task 4 Step 2/3 + Task 5。
- **Spec §6 验收门（代码层 + 真机 + Windows 留待）** → Task 2（代码）+ Task 1 Step 5/6 + Task 5 Step 1（真机）+ Task 4/5（Windows 留待登记）。
- **退路线 B 兜底** → Task 1 Step 3/5 失败分支显式回报。
- 占位符扫描：无 TBD/TODO；测试代码与命令均完整。
- 类型一致性：`LlamaLoader::new` / `ModelLoader::load` / `LlamaModelRuntime::generate` / `ModelLoadParams{gpu_layers,context_size}` / `GenerateParams::default` 与 lib.rs 实际签名一致。
