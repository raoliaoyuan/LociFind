# BETA-25 设计：model-fallback 动态库打包修复（静态链接路线）

> 状态：已批准（2026-06-13，用户认可设计三节）。spec → plan → 实施。
> 关联：BETA-23（模型 fallback 接入，done）真机手测暴露本问题；BETA-24（重训，done）。

## 1. 背景与根因

BETA-23 真机手测发现：`--features model-fallback-metal` 打出的 macOS `.app` **启动即崩**
（`Library not loaded: @rpath/libggml-base.0.dylib`），手测时靠手工
`cp dylibs → Contents/Frameworks` + `install_name_tool -add_rpath @executable_path/../Frameworks`
+ ad-hoc 重签才能跑。

本会话探明完整根因链：

1. `packages/model-runtime/Cargo.toml` 依赖 `llama-cpp-4 = { version = "0.3.0", optional = true }`，
   **继承其 default features**。
2. `llama-cpp-4` 0.3.0 的 `default = ["openmp", "mtmd", "dynamic-link"]` —— **`dynamic-link` 是默认 feature**。
3. `dynamic-link` 透传到 `llama-cpp-sys-4`，其 build.rs `build_shared_libs = cfg!(feature = "dynamic-link")`
   → `BUILD_SHARED_LIBS=ON` → 产出一组动态库（`libggml*` / `libllama*` / `libmtmd*`）。
4. 最终二进制把它们引用为 `@rpath/libggml-base.0.dylib` 等，但**二进制内 `LC_RPATH` 计数 = 0**
   （`otool -l target/release/locifind-desktop | grep -c LC_RPATH` 实测为 0）→ 运行期解析不到 → 崩。
5. tauri bundler 不收集这些 dylib，.app 内本来也没有。

关键发现：**`llama-cpp-sys-4` 原生支持静态链接**（不开 `dynamic-link` 即走静态）。问题源自
`llama-cpp-4` 把 `dynamic-link` 放进默认 feature、被我们继承。

## 2. 方案选型

### 路线 A — 静态链接（采用）

在 `model-runtime` 给 `llama-cpp-4` 加 `default-features = false`，去掉 `dynamic-link`，
让 llama 全部静态进二进制。结果：**没有 dylib 要打包、没有 rpath、没有重签名**，
tauri 直接打包单个胖二进制。**一处 Cargo 改动同时覆盖 macOS + Windows**（Windows 侧亦不再缺 DLL）。

- 优点：分发最干净；CI 打包逻辑不动；消除整类 rpath/签名/DLL 缺失问题。
- 风险：静态 + metal 能否编过、能否正常推理是唯一真风险 → 计划第一步 spike 证伪。
- 兜底：若静态不可行，退路线 B。

### 路线 B — 自动化打包动态库（兜底，不采用）

保留动态库，脚本/build.rs 自动拷 dylib 进 `Contents/Frameworks` + 注入
`@executable_path/../Frameworks` rpath + 重签名；Windows 侧塞 DLL 进 NSIS。
缺点：macOS/Windows 各一套、dylib 名带版本号、frameworks 路径需 config 期已知，脆弱。
仅作路线 A 证伪失败时的兜底。

## 3. 核心改动

`packages/model-runtime/Cargo.toml`：

```toml
# 改前（继承 default features，含 dynamic-link → 动态库）
llama-cpp-4 = { version = "0.3.0", optional = true }
# 改后（关默认、去 dynamic-link → 静态链接）
llama-cpp-4 = { version = "0.3.0", optional = true, default-features = false }
```

连带的默认 feature 取舍（spike 中以实测定夺）：

- **`dynamic-link`** → 去掉（病根，必须去）。
- **`mtmd`**（多模态 libmtmd）→ 倾向去掉：仅文本 intent 解析，spike 中确认 `llama.rs` 未用 mtmd API。
- **`openmp`** → 倾向去掉：openmp 牵出 `libgomp`/`libomp`，macOS 上可能又成 dylib 依赖把问题带回；
  llama.cpp 自带线程池，去掉只换线程实现、不影响正确性。

`metal` 不受影响：走 model-runtime 既有 `metal` feature 接线（`metal = ["llama-cpp-4?/metal", ...]`），
与 `default-features` 正交。

**验证不变量**：最终二进制 `otool -L` **零 `@rpath/*.dylib` 残留**（ggml/llama/mtmd/omp/gomp 全无）。

## 4. dev 窗口标题区分（顺带）

手测时安装版与 dev 构建同名同 bundle id，反复驱动错窗口。修复：`main.rs` setup 闭包内
`cfg!(debug_assertions)` 时把主窗口标题设为 `LociFind (dev)`，release 构建保持 `LociFind`。

## 5. 实施步骤（证伪优先）

1. **Spike（先证伪）**：临时改 Cargo → `cargo tauri build --features model-fallback-metal`
   → ① `otool -L` 确认零 dylib；② 启动**未经手工修补**的 .app；③ 跑问题 4 query 确认
   「模型补全」真出结果。任一不过则调 feature 组合（视情补回 mtmd/openmp）；
   若静态根本不可行，退路线 B（本 spec §2.B）。
2. 固化 Cargo 改动；三种 feature 形态（关 / `model-fallback` / `model-fallback-metal`）
   跑 fmt + clippy(`-D warnings`) + test。
3. **CI**：`release-windows.yml` 的 `args` 不变（仍 `--features model-fallback`），无需塞 DLL；
   更新构建说明注释（不再需要 DLL 打包）。Windows 静态编译能否过由 CI 兜，下个 Windows 会话装包实测。
4. dev 窗口标题（§4）。
5. 文档：STATUS / ROADMAP（BETA-25 → done、Windows 验证留待）/ manual-test-scenarios（BETA-25 节）；
   `third-party-licenses.md` 复核（静态链接 llama.cpp 无新增依赖、许可不变，确认即可）。

## 6. 验收门

- **代码层**：三 feature 形态 `clippy -D warnings` 0、全 workspace test 零回归；
  evals parser-only byte-equal 不动（本改动不碰 parser）。
- **真机（验收红线，macOS 本机）**：**未经任何手工修补**的 release `.app` 双击即开、
  问题 4 query 跑出模型补全结果、`otool -L` 二进制零 `@rpath` dylib。
- **Windows**：本会话不物理验证，仅确保 CI 能编出 NSIS 产物；实测留下个 Windows 会话。

## 7. 范围与非目标

- 本会话只物理验证 macOS；Windows 验证留下个 Windows 会话（路线 A 下同一份改动天然覆盖 Windows）。
- 不改 parser / 模型 / 搜索逻辑——纯构建/打包 + dev 标题。
- 路线 B（自动化打包动态库）仅作兜底文档，路线 A 通过则不实现。
