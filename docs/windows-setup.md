# Windows 开发 / 测试环境准备

> 目的：在一台干净的 Windows 机器上从 GitHub 同步 LociFind 代码后，最快路径跑起来——开发、测试、跑评测、（按需）跑桌面 app 与本地模型。
> 适用：Windows 10 / 11，x86-64。配合 Claude Code 使用。
> 维护：随工具链/前置变化更新；与 [PROJECT.md](../PROJECT.md) 技术决策、各 package README 对齐。

---

## 0. 一句话路径

- **只想跑 BETA-15A 召回评测 / parser / evals / harness / 后端单测** → 装 **Rust stable** 一项即可，`cargo test -p <crate>` 直接跑（最快，下面 §3）。
- **想跑桌面 app（Tauri）** → 额外装 **Node 18+ / MSVC C++ Build Tools / WebView2**（§4）。
- **想跑本地模型 fallback（GGUF 推理）** → 额外装 **CMake** + 手动拷贝模型文件（GGUF 被 gitignore，clone 不含，§5）。

按需要分层装，不必一次到位。

---

## 1. 同步代码

### 1.0 认证（clone 免认证；push 需登录）

`raoliaoyuan/LociFind` 已于 2026-07-04 转**公开仓库**（此前的私有认证步骤作废）：**clone / pull 无需任何认证**。若这台机器要 **push**，用 GitHub CLI 登录一次即可：

```powershell
winget install --id GitHub.cli
gh auth login        # 选 GitHub.com → HTTPS → Login with a web browser
```

> 仓库设置：公开 / 默认分支 `main` / 无分支保护（协作者可直接 push）。完整开发历史在私有归档仓库 `LociFind-archive`（公开仓库自单次「初始开源」commit 起步，详 [脱敏核查报告 §4](reviews/beta-00-repo-sanitization-2026-07-04.md)）。

### 1.1 clone / pull

```powershell
git clone https://github.com/raoliaoyuan/LociFind.git
cd LociFind
# 或已 clone 过：
git pull origin main
```

确认拿到最新：

```powershell
git log --oneline -1
# 与 GitHub 上 main 最新 commit 一致即可（公开仓库历史自 2026-07-04「初始开源发布」起步）
```

**行尾**：仓库用 `.gitattributes` 统一 LF。Windows 上 git 默认 `core.autocrlf=true` 可能改行尾——若 `git status` 显示大量"伪改动"，设 `git config core.autocrlf false` 后重新 checkout。

---

## 2. 会话开始（用 Claude Code 时）

LociFind 是 Claude Code / Codex / Gemini 三工具轮换协作的项目。**新会话开始必读四份共享文档**（[CLAUDE.md](../CLAUDE.md) 入口已写明）：

1. [PROJECT.md](../PROJECT.md) — 目标 / 架构
2. [STATUS.md](../STATUS.md) — 当前进度 / 当前 task / 下一步 / 会话日志（**单一信源**）
3. [ROADMAP.md](../ROADMAP.md) — 全程任务地图 / 出场标准
4. [CONVENTIONS.md](../CONVENTIONS.md) — 协作规则 / **收工流程** / 编码规范

收工时按 CONVENTIONS §3 更新 STATUS / ROADMAP，一次中文 commit，署名 `Claude Code`。

---

## 3. 最小环境：Rust（覆盖 BETA-15A / parser / evals / 后端单测）

### 装 Rust

1. 装 [rustup](https://rustup.rs/)（会随之拉起 MSVC 链接器需求，见下）。
2. 仓库 `rust-toolchain.toml` pin 了 `channel = stable` + `rustfmt` + `clippy`，rustup 会自动按它装对应组件，无需手动指定版本（workspace `rust-version = 1.80`，stable 满足）。
3. rustup 在 Windows 默认用 **MSVC toolchain**，需要 **Visual Studio C++ Build Tools**（见 §4.1，链接器 `link.exe` 必需）。若暂时不想装 VS，可改用 GNU toolchain：`rustup default stable-x86_64-pc-windows-gnu`（但桌面 Tauri 仍建议 MSVC）。

### 跑 BETA-15A 同义词召回评测

```powershell
# 报告（按桶/按语言分桶 + 门槛退出码）
cargo run -p locifind-evals --bin synonym_recall
# 仅看未达标 case
cargo run -p locifind-evals --bin synonym_recall -- --only-failures
# JSON 报告
cargo run -p locifind-evals --bin synonym_recall -- --json
```

期望：总召回 88.2% / 假阳 0.0%（与 macOS 一致——纯离线 Rust，无平台差异），门槛通过退出码 0。

### 跑各 crate 单测 / 评测（不碰桌面与模型）

```powershell
cargo test -p locifind-evals          # 含 recall 单测 + 集成门槛测试
cargo test -p locifind-intent-parser  # parser
cargo test -p locifind-harness        # harness（含同义词 expander）
cargo test -p locifind-search-backend-everything   # Everything 后端
cargo test -p locifind-search-backend # common
cargo run  -p locifind-evals --bin evals -- --fixtures v0.5   # parser-only 评测（期望 472/26/2）
```

> **为什么用 `-p` 而不是 `--workspace`**：workspace 含 `apps/desktop/src-tauri`（需 Tauri 前置）与 `packages/model-runtime`（`llama-cpp` 特性需 CMake）。`cargo test --workspace` 会尝试编译它们——若还没装 §4/§5 的前置就会失败。**只做后端/评测/parser 开发时用 `-p` 精确指定 crate**，跑得最快、前置最少。

### ci.sh（bash 脚本）

`scripts/ci.sh`（fmt + clippy + build + test + synonym_recall）是 **bash 脚本**，Windows 原生 cmd/PowerShell 跑不了。两种方式：

- **Git Bash**（装 Git for Windows 自带）：`bash scripts/ci.sh`
- **直接敲命令**（推荐做局部开发时）：
  ```powershell
  cargo fmt --all -- --check
  cargo clippy -p <crate> --all-targets -- -D warnings
  cargo test -p <crate>
  ```
  注意 `scripts/ci.sh` 跑的是 `--workspace`，整套需要 §4/§5 前置齐全；局部开发用 `-p` 即可。

---

## 4. 桌面 app（Tauri 2）前置

仅当要跑/构建 `apps/desktop` 时需要。

### 4.1 MSVC C++ Build Tools（Rust 链接 + Tauri 都需要）

装 [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/downloads/)，勾选 **「使用 C++ 的桌面开发」**（含 MSVC v143 + Windows 11 SDK）。这是 Rust MSVC toolchain 链接器与 Tauri 编译的硬前置。

### 4.2 WebView2 Runtime

Windows 11 通常已内置；Windows 10 若缺，装 [WebView2 Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。Tauri 的 WebView 依赖它。

### 4.3 Node.js

装 **Node 18+**（前端 vite + Tauri CLI）。仓库 `apps/desktop/package.json` 用 `@tauri-apps/cli ^2`、`vite ^5`、`react 18`。

```powershell
cd apps\desktop
npm install
# 开发模式（热重载）
npm run tauri dev
# 构建安装包
npm run tauri build
```

> 桌面 app 的「后端状态指示」「模型 fallback」等功能在无 §5 模型时会降级显示，不影响搜索主路径（系统搜索后端）。

---

## 5. 本地模型（GGUF 推理 fallback）—— 文件被 gitignore，需手动获取

### 现状

`.gitignore` 排除了 `*.gguf` / `*.safetensors` / `training/mlx-lora/{adapters,checkpoints,data,fused}/`。**clean clone 不含任何训练好的模型**。纯 parser / 后端 / 评测开发**不需要**模型；只有跑「模型 fallback」（规则解析不足时调小模型补字段）才需要。

### 获取推荐 GGUF（BETA-17 winner = Qwen3-0.6B）

**当前推荐默认 = `beta17-qwen3-0.6b-q4_k_m.gguf`（378 MB）** —— BETA-17 选型实验（2026-06-01）实测：Qwen3-0.6B 与 v1 基线（Qwen2.5-1.5B）准确率**逐项相等**（hybrid pass 480/字段 96.0%/rescued 8/regressed 0），但体积小 60%、macOS Metal p95 fallback 快 34%。evals / fallback_probe 默认路径已指向 `models/qwen3-0.6b-q4_k_m.gguf`。从 macOS 训练机拷贝到 Windows，**sha256 校验**（务必核对，单点本地依赖）：

```
training/mlx-lora/fused/beta17-qwen3-0.6b-q4_k_m.gguf
sha256 = 898c98bcaa40489742cbd6586f31e768a5d8d238da70eb58cff25a5eb19117df
378 MB
```

PowerShell 校验：`Get-FileHash beta17-qwen3-0.6b-q4_k_m.gguf -Algorithm SHA256`。放到默认路径用 `models/qwen3-0.6b-q4_k_m.gguf`，或自定义后用 `LOCIFIND_MODEL_PATH` 指向。

> **✅ BETA-17 Windows 延迟复核已闭合（2026-06-02，Intel Iris Xe / Vulkan）**：winner GGUF 实测准确率与 Mac 逐项 0pp（pass 480/regressed 0）。延迟经两项推理优化（`stop_at_json` 首个 JSON 即停 + 固定前缀 KV 复用）后 **fallback p95 13764ms → 1197ms（快 11.5×），跨过 3000ms 交互门槛（余量 60%）**——弱核显也能交互式跑模型补全，能力感知降级从硬性必需降为可选。详 [docs/reviews/beta-17-base-model-bakeoff.md](../docs/reviews/beta-17-base-model-bakeoff.md) §6。

### 备选：v1 GGUF（Qwen2.5-1.5B，BETA-09a 已验）

需要 v1 基线对照时用 `main-v1-q4_k_m.gguf`（940 MB，sha256 `854125317fa478285eb939dc891e7844bb02cf4c11987d4340642e1698006b17`）。其它变体 q5_k_m (1.0 GB) / q6_k (1.2 GB) 与 adapter + fp16 GGUF 的 sha256 见 [training/mlx-lora/releases/v1.md](../training/mlx-lora/releases/v1.md) §3。

### 编译模型 fallback 特性（Windows 真机实测前置 — BETA-09(a) 2026-06-01 解锁）

> `packages/model-runtime` 的 `llama-cpp` feature 在 Windows 上编译，除 CMake 外还有几个 macOS 不暴露的隐藏前置（macOS 的 clang/Metal 工具链自带）。以下为 BETA-09(a) 真机一次性趟通的完整清单，缺一不可：

| 前置 | 装法 | 为什么需要 |
|---|---|---|
| **MSVC C++ Build Tools** | §4.1 | 链接器 + C++ 编译（vcvars64）|
| **CMake** | `winget install Kitware.CMake` | llama.cpp 构建 |
| **LLVM / libclang** | `winget install LLVM.LLVM`，设 `LIBCLANG_PATH=C:\Program Files\LLVM\bin` | `llama-cpp-sys-4` 经 `bindgen` 生成 FFI 绑定，需 `libclang.dll`（不装报 `Unable to find libclang`）|
| **Ninja 生成器**（VS 自带）| 设 `CMAKE_GENERATOR=Ninja` | 默认 VS 生成器下 `cmake` crate 把 `-j8` 传给 MSBuild 会报 `MSB1001 未知开关`；改用 Ninja 即可。**务必在 vcvars64 开发者环境里编**（cl.exe 在 PATH）|
| **Vulkan SDK**（GPU 加速，可选但强烈推荐）| `winget install KhronosGroup.VulkanSDK`，设 `VULKAN_SDK=C:\VulkanSDK\<ver>` | 纯 CPU 推理在弱机器上慢到不实用（单次 fallback 几十秒）。Vulkan 走核显/独显快很多。Windows 用 `vulkan` 特性（非 macOS 的 `metal`）|

**编译命令**（在 VS 开发者环境的 cmd 里，避免引号问题建议写 bat）：

```bat
call "...\VC\Auxiliary\Build\vcvars64.bat"
set "CMAKE_GENERATOR=Ninja"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
set "VULKAN_SDK=C:\VulkanSDK\1.4.350.0"
set "PATH=C:\Program Files\CMake\bin;...VS...\CMake\Ninja;%VULKAN_SDK%\Bin;%PATH%"
cargo clean -p llama-cpp-sys-4 --release   :: 切 CPU<->Vulkan 时必做，避免 CMakeCache 生成器冲突
cargo build -p locifind-evals --features model-fallback-vulkan --bin evals --release
```

> 切 GPU 后端时改 feature：`model-fallback`（纯 CPU）/ `model-fallback-vulkan`（Vulkan）/ `model-fallback-metal`（仅 macOS）。**注意 `cargo run` 时 feature 必须与编译一致，否则会按新 feature 重编。** 无法/不想装 CMake 时，model-runtime 有纯 Rust 的 `candle` 后端 fallback（见 [packages/model-runtime/README.md](../packages/model-runtime/README.md)）。

跑带模型的评测（CLI，在上述 bat 环境内）：

```bat
set "LOCIFIND_MODEL_PATH=...\training\mlx-lora\fused\beta17-qwen3-0.6b-q4_k_m.gguf"
cargo run -p locifind-evals --features model-fallback-vulkan --bin evals --release -- --fixtures v0.5 --with-fallback --hybrid
```

> **BETA-09(a) 实测结论（详 [docs/reviews/beta-09a-windows-parity.md](./reviews/beta-09a-windows-parity.md)）**：Intel Iris Xe Vulkan 跑完整 500 case 与 macOS/Metal **逐项 0pp 差异**（准确性跨平台完全一致）。但**延迟**：弱核显 p95 fallback ~22s（macOS Metal 1.6s），不达 3000ms 交互门槛——弱硬件上模型 fallback「准确但太慢」，产品侧应能力感知降级（默认纯 parser，检测到强 GPU 再启用模型）。

---

## 6. Windows 特定开发重点（这台机器才能推进的事）

STATUS / ROADMAP 里有几项一直**卡 Windows 真机**，正是这台机器解锁的价值：

1. **两个 Windows 后端执行层 ✅ 已在 Windows 11 真机实测（2026-05-31，MVP-11/12）**：
   - `packages/search-backends/windows-search/src/lib.rs`：`PlatformWindowsSearchExecutor` 经 `Search.CollatorDSO` OLE DB provider（固定 `PowerShell` + ADODB 脚本，SQL 经环境变量传入）执行；用 `System.ItemUrl` 还原真实路径（非本地化 `ItemPathDisplay`）；相对时间在执行器解析为绝对 ISO（provider 不支持 `DATEADD`/`GETDATE`）。真机集成测试 `tests/real_windows_search.rs`（`cargo test -p locifind-search-backend-windows-search -- --ignored`）。
   - `packages/search-backends/everything/src/lib.rs`：`EsCliExecutor` spawn `es.exe`（结构化参数、取消/超时）。需装 [Everything](https://www.voidtools.com/) + ES CLI（`winget install voidtools.Everything.Cli`；es.exe 落在 `%LOCALAPPDATA%\Microsoft\WinGet\Packages\voidtools.Everything.Cli_*\`，重启 shell 后入 PATH）。真机集成测试 `tests/real_everything.rs`（`-- --ignored`，需 es.exe 在 PATH）。修复：早期误加的 `-path` 会把搜索项当路径吞掉（真机实测 0 结果），已移除。
2. **MVP-26 跨平台一致性测试**：在 Windows 跑 v0.5 evals，与 macOS 对比，验证「双平台通过率差 < 5pp」（M→B 切换硬指标，至今从未实跑过）。
3. **BETA-09(a) 跨平台部署**：Windows 加载 v1 GGUF（§5）验证推理路径与 macOS 一致，对比 [release notes](../training/mlx-lora/releases/v1.md) §4 指标。
4. **MVP-24 Windows 索引位置引导**：当前 macOS stub，Windows 真检测待真机。

→ 下个会话开场，先看 STATUS「下一步」，从上述里选一条推。

### 6.1 图片 OCR（BETA-03）运行期前置

图片 OCR **无 cargo 依赖**，以外部进程运行，按平台需要可选前置：

- **Windows.Media.Ocr（首选，系统自带）**：需已装对应 **OCR 识别语言包**。检查：
  ```powershell
  [Windows.Media.Ocr.OcrEngine,Windows.Media.Ocr,ContentType=WindowsRuntime] | Out-Null
  [Windows.Media.Ocr.OcrEngine]::AvailableRecognizerLanguages | % DisplayName
  ```
  无中文识别器 → 设置 → 时间和语言 → 语言 → 添加「中文（简体）」语言包（含 OCR）。
- **Tesseract（跨平台兜底，可选）**：`winget install tesseract-ocr.tesseract` + 装 `chi_sim`/`eng`
  语言数据（PATH 上有 `tesseract` 即被 `default_ocr_engine` 选为兜底）。
- 两者皆无 → 图片索引**优雅跳过、不报错**（音乐/文档索引照常）。
- 真机集成测试：`cargo test -p locifind-indexer --test real_ocr -- --ignored`（需已装 OCR 语言）。

---

## 7. 常见坑速查

| 现象 | 原因 / 解法 |
|---|---|
| `link.exe not found` / 链接失败 | 没装 MSVC C++ Build Tools（§4.1），或没重启终端让 PATH 生效 |
| `cargo build --workspace` 在 model-runtime 报 cmake 错 | 没装 CMake（§5），或只想跑后端/评测——改用 `cargo test -p <crate>` |
| `cargo test --workspace` 在 desktop 报 Tauri/WebView2 错 | 桌面前置未装（§4）——开发后端时用 `-p` 跳过 desktop |
| `bash: scripts/ci.sh` 找不到 | 用 Git Bash 跑，或直接敲 cargo 命令（§3） |
| git status 一堆伪改动（行尾） | `git config core.autocrlf false` 再重新 checkout（§1） |
| 模型 fallback 不生效 / 找不到模型 | GGUF 被 gitignore，需手动拷 + 设 `LOCIFIND_MODEL_PATH`（§5）；sha256 务必核对 |
| Everything 后端 `BackendUnavailable` | 执行层 pending（§6.1），且需装 Everything + 启用 ES CLI |

---

## 8. 验证清单（环境装好后自检）

```powershell
# 最小（Rust）
cargo test -p locifind-evals                       # 召回单测 + 门槛集成测试全过
cargo run  -p locifind-evals --bin synonym_recall  # 88.2% / 0.0% 门槛通过
cargo run  -p locifind-evals --bin evals -- --fixtures v0.5   # 472/26/2

# 桌面（装了 §4）
cd apps\desktop && npm install && npm run tauri dev

# 模型（装了 §5）
$env:LOCIFIND_MODEL_PATH="C:\path\to\beta17-qwen3-0.6b-q4_k_m.gguf"
cargo run -p locifind-evals --features model-fallback --bin evals -- --fixtures v0.5 --with-fallback --hybrid
```

跑通最小那三条，就说明代码同步 + Rust 环境 OK，可以开始 Windows 侧开发了。
