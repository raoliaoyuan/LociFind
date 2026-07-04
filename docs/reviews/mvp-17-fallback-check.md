# MVP-17 fallback 启动检查

日期：2026-05-26  
工具：Codex  
范围：独立 worktree 检查；不修改生产代码；不提交 commit。

## 结论

`locifind-model-runtime` 的 `llama-cpp` feature 当前在本机 **BLOCKED**，原因是系统找不到 `cmake` 命令。失败发生在 `llama-cpp-sys-4 v0.3.0` 的 build script 调 CMake 配置阶段，还没进入 llama.cpp C++ 编译。

MVP-17 fallback 的 Rust 接入形态已经具备：`packages/intent-parser/src/fallback.rs` 提供 `resolve_intent(query, Some(&fallback))`，并能区分 `IntentSource::Parser` / `ParserNoFallback` / `Model`。下一步最小可行验证不应先改桌面端，而应先在 `packages/evals` 加一个独立 `--with-fallback` 入口，跑 v0.5 中 20-30 条 Class D/MediaSearch fail 子集，量化模型救回率和 JSON 合法率。

## 已检查文件

- `PROJECT.md`
- `STATUS.md`
- `ROADMAP.md`
- `CONVENTIONS.md`
- `packages/model-runtime/README.md`
- `packages/model-runtime/Cargo.toml`
- `packages/model-runtime/src/lib.rs`
- `packages/model-runtime/src/llama.rs`
- `packages/model-runtime/src/daemon.rs`
- `packages/intent-parser/src/fallback.rs`
- `packages/evals/src/bin/evals.rs`
- `packages/evals/src/lib.rs`

## llama-cpp build 结果

执行命令：

```bash
cargo build -p locifind-model-runtime --features llama-cpp 2>&1 | tee /tmp/codex-llama-build.log
```

结果：失败。

关键日志摘要：

```text
Compiling llama-cpp-sys-4 v0.3.0
error: failed to run custom build command for `llama-cpp-sys-4 v0.3.0`

running: ... "cmake" ".../out/llama.cpp" "-B" ".../target/llama-cmake-cache/..."

thread 'main' panicked at .../cmake-0.1.58/src/lib.rs:1132:5:
failed to execute command: No such file or directory (os error 2)
is `cmake` not installed?
```

完整日志：`/tmp/codex-llama-build.log`

本机已能走到 bindgen/clang 头文件扫描和 `llama-cpp-sys-4` build script；日志里也已经尝试链接 Homebrew `libomp`：

```text
cargo:rustc-link-search=native=/opt/homebrew/opt/libomp/lib
cargo:rustc-link-lib=dylib=omp
```

所以当前第一阻塞点不是 `LIBCLANG_PATH`，而是 `cmake` 不存在。

建议修复步骤：

```bash
brew install cmake libomp
xcode-select --install  # 如本机尚未安装 Command Line Tools
```

修复后先复跑：

```bash
cargo build -p locifind-model-runtime --features llama-cpp
```

如下一步卡在 bindgen/libclang，再设置：

```bash
brew install llvm
export LIBCLANG_PATH=/opt/homebrew/opt/llvm/lib
cargo clean -p locifind-model-runtime
cargo build -p locifind-model-runtime --features llama-cpp
```

如要验证 Metal 后端，再单独跑：

```bash
cargo build -p locifind-model-runtime --no-default-features --features llama-cpp,metal
```

## GGUF 下载方案

推荐优先使用官方 Qwen GGUF 仓库：

- 仓库：`Qwen/Qwen2.5-1.5B-Instruct-GGUF`
- 文件：`qwen2.5-1.5b-instruct-q4_k_m.gguf`
- 来源：Hugging Face 官方 Qwen 模型页支持 `llama-server -hf Qwen/Qwen2.5-1.5B-Instruct-GGUF:Q4_K_M` / `llama-cli -hf ...:Q4_K_M` 方式直接拉取运行。
- 参考：<https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF>

备选源：`bartowski/Qwen2.5-1.5B-Instruct-GGUF`。该仓库明确列出 `Qwen2.5-1.5B-Instruct-Q4_K_M.gguf` 约 `0.99GB`，说明为 “Good quality, default size for must use cases, recommended”。  
参考：<https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF>

推荐放置目录：

```text
models/qwen2.5-1.5b-instruct-q4_k_m.gguf
```

理由：

- `packages/model-runtime/README.md` 示例已经使用 `models/qwen2.5-1.5b-instruct-q4_k_m.gguf`。
- 仓库 `.gitignore` 已忽略 `*.gguf`，模型不会误提交。
- `LOCIFIND_MODEL_PATH` 可继续作为运行时覆盖项。

下载命令：

```bash
mkdir -p models
pipx run huggingface_hub download \
  Qwen/Qwen2.5-1.5B-Instruct-GGUF \
  qwen2.5-1.5b-instruct-q4_k_m.gguf \
  --local-dir models \
  --local-dir-use-symlinks False
```

如果官方仓库文件名或 CLI 查询出现大小写差异，可用官方 `:Q4_K_M` alias 先验证：

```bash
brew install llama.cpp
llama-cli -hf Qwen/Qwen2.5-1.5B-Instruct-GGUF:Q4_K_M -p "hello" -n 16
```

校验方式：

```bash
shasum -a 256 models/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  > models/qwen2.5-1.5b-instruct-q4_k_m.gguf.sha256

shasum -a 256 -c models/qwen2.5-1.5b-instruct-q4_k_m.gguf.sha256
```

更严格的来源校验建议：

```bash
huggingface-cli scan-cache
huggingface-cli download Qwen/Qwen2.5-1.5B-Instruct-GGUF \
  qwen2.5-1.5b-instruct-q4_k_m.gguf \
  --local-dir models \
  --local-dir-use-symlinks False
```

下载后记录模型仓库 commit hash、文件名、SHA-256 到 `docs/third-party-licenses.md` 或后续模型台账；不要把 `.gguf` 或 `.sha256` 提交到仓库。

## evals 当前基线

当前 v0.5 parser-only：

```text
variant 命中率:      427 / 500  (85.4%)
字段级精确匹配率:  237 / 500  (47.4%)
pass:              237 / 500  (47.4%)
partial:           190 / 500  (38.0%)
fail:               73 / 500  (14.6%)

MediaSearch: pass 2, partial 43, fail 55
```

`docs/reviews/mvp-25-lexicon-gaps.md` 中的 Class D 定义仍成立：parser 产出结构化 intent 但关键字段为空，属于 MVP-17 signals/model fallback 应接管的结构性遗漏。parser v0.3 后，剩余最值得跑 fallback 的最小子集是：

- MediaSearch fail：`expected MediaSearch` 但 parser 走 `FileSearch`，如 `找最大的视频`、`find videos modified this week`。
- MediaSearch partial：artist/location/time/sort 字段空或错，模型可能补全。
- Clarify fail：`find recent`、`delete 全部` 这类 parser 未进入 clarify 的边界。
- 少量 FileAction partial：`move the 4 result to documents`、`rename the 5 result to synthetic-final`，模型可能补全 destination/new_name。

## `--with-fallback` 最小入口草稿

不建议先改现有 `evaluate_case(case)` 的签名，可以先加一个轻量上下文对象，让 parser-only 路径保持默认。

`packages/evals/Cargo.toml` 草稿：

```toml
[dependencies]
locifind-model-runtime = { path = "../model-runtime", default-features = false, features = ["llama-cpp"] }
```

`packages/evals/src/bin/evals.rs` 参数草稿：

```rust
#[derive(Parser, Debug)]
struct Args {
    #[arg(short, long)]
    case: Option<String>,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    only_failures: bool,
    #[arg(long, default_value = "v0.1")]
    fixtures: String,

    /// 用 MVP-17 ModelFallback 跑解析，而不是 parser-only。
    #[arg(long)]
    with_fallback: bool,

    /// GGUF 模型路径；未提供时读 LOCIFIND_MODEL_PATH 或 models/qwen2.5-1.5b-instruct-q4_k_m.gguf。
    #[arg(long)]
    model_path: Option<std::path::PathBuf>,

    /// 只跑 fallback 候选子集，避免 500 条全量慢跑。
    #[arg(long)]
    fallback_subset: bool,
}
```

`packages/evals/src/lib.rs` 草稿：

```rust
use locifind_intent_parser::fallback::{resolve_intent, IntentSource, ModelFallback};

pub struct EvalContext<'a> {
    pub fallback: Option<&'a ModelFallback>,
}

impl<'a> EvalContext<'a> {
    pub const fn parser_only() -> Self {
        Self { fallback: None }
    }
}

#[must_use]
pub fn evaluate_case_with_context(case: &Case, ctx: &EvalContext<'_>) -> CaseReport {
    let actual_intent = match ctx.fallback {
        Some(fallback) => resolve_intent(&case.query, Some(fallback))
            .map(|resolved| resolved.intent)
            .unwrap_or_else(|_| parse(&case.query)),
        None => parse(&case.query),
    };

    evaluate_actual(case, actual_intent)
}

fn evaluate_actual(case: &Case, actual_intent: SearchIntent) -> CaseReport {
    let actual_json = serde_json::to_value(&actual_intent).unwrap_or(Value::Null);
    let expected_json = &case.expected_intent;
    let actual_variant = variant_name(&actual_intent);

    let result = if actual_variant == case.variant {
        let diff = compare_json(expected_json, &actual_json);
        if diff.is_empty() {
            EvalResult::Pass
        } else {
            EvalResult::Partial { diff }
        }
    } else {
        EvalResult::Fail {
            actual_variant: actual_variant.to_owned(),
        }
    };

    CaseReport {
        case: case.clone(),
        nl_input: case.query.clone(),
        result,
        actual_json,
    }
}
```

`main()` 初始化 fallback 草稿：

```rust
let fallback_holder;
let eval_ctx = if args.with_fallback {
    use locifind_intent_parser::fallback::ModelFallback;
    use locifind_model_runtime::{ModelDaemon, ModelLoadParams};
    use std::sync::Arc;

    let model_path = args
        .model_path
        .or_else(|| std::env::var_os("LOCIFIND_MODEL_PATH").map(Into::into))
        .unwrap_or_else(|| "models/qwen2.5-1.5b-instruct-q4_k_m.gguf".into());

    let daemon = Arc::new(ModelDaemon::load_blocking(
        &model_path,
        ModelLoadParams {
            gpu_layers: 999,
            context_size: 2048,
        },
    )?);
    fallback_holder = ModelFallback::new(daemon);
    EvalContext {
        fallback: Some(&fallback_holder),
    }
} else {
    EvalContext::parser_only()
};

let reports: Vec<CaseReport> = filtered_cases
    .iter()
    .filter(|case| !args.fallback_subset || is_fallback_candidate(case))
    .map(|case| evaluate_case_with_context(case, &eval_ctx))
    .collect();
```

fallback 候选过滤草稿：

```rust
fn is_fallback_candidate(case: &Case) -> bool {
    matches!(case.variant.as_str(), "MediaSearch" | "Clarify" | "FileAction")
        || case.query.contains("video")
        || case.query.contains("视频")
        || case.query.contains("截图")
        || case.query.contains("screenshots")
        || case.query.contains("recent")
        || case.query.contains("rename")
        || case.query.contains("move")
}
```

首轮建议只跑 20-30 条固定 case，避免模型慢跑掩盖问题。推荐从当前 v0.5 失败中抽样：

```text
v05-media-class1-sort-052
v05-media-class1-sort-054
v05-media-class1-sort-060
v05-media-class1-week-064
v05-media-class1-week-070
v05-media-class1-size-074
v05-media-class1-size-078
v05-media-template-250
v05-media-template-254
v05-media-template-258
v05-media-template-266
v05-media-template-274
v05-media-template-282
v05-media-template-286
v05-media-template-290
v05-media-template-298
v05-media-template-300
v05-media-template-304
v05-action-template-352
v05-action-template-353
v05-action-template-381
v05-action-template-382
v05-clarify-template-481
v05-clarify-template-485
v05-clarify-template-489
v05-clarify-template-497
```

建议新增输出指标：

```text
fallback candidate total
fallback invoked
model source count
model valid JSON count
model invalid JSON count
rescued_to_pass
rescued_to_partial
regressed
p50/p95 latency
```

“救回率”建议定义为：

```text
rescued_to_pass_or_partial / parser_only_fail_count
```

同时单独报告 `rescued_to_pass / parser_only_fail_count`，避免 partial 掩盖字段质量问题。

## 风险与建议

- 真实模型跑 evals 前必须先解决 `cmake`，否则 `llama-cpp` 后端无法 build。
- `ModelFallback::invoke` 当前只做 serde 反序列化，没有接 harness `SchemaValidator`；evals 中可以先接受，但桌面端接入前应补 schema validator 或错误降级。
- 当前 prompt 的 few-shot 会在 `build_full_prompt()` 中重复一次 `few_shots()`，因为 `PromptBuilder::user_prompt()` 内部也会追加示例；fallback evals 若延迟高或上下文超限，应先去重 prompt。
- evals dependency 不应默认打开 `llama-cpp`，否则 CI 会受 CMake 和本地模型影响。建议通过 feature gate，例如 `locifind-evals --features model-fallback`，或保留草稿中的代码到后续专门 PR。
