# BETA-31-v2 设计：Windows GPU 推理优化（vulkan）

> **类型**：Backend feature + Workflow + Distribution UX cycle（含 model-runtime feature 扩、apps Cargo features 扩、release-windows.yml 切 GPU build、运行时 fallback 路径、文档）
> **承接**：BETA-31 收尾（[PR #20](https://github.com/raoliaoyuan/LociFind/pull/20) merged commit `1f04f51`）+ STATUS 2026-06-29 v0.8.0 Windows 真机暴露 5 个 UX bug 其中 #4「embedding 冷启动 ~17s 撞 15s 搜索 timeout」
> **目标**：让 Windows 用户 v0.9.0 默认走 GPU 推理（vulkan 后端）、首次冷启动 < 5s / 稳态单次 query embedding < 100ms；GPU 不存在 / Vulkan 缺失时运行时自动回退 CPU（不崩、不需用户配置）
> **范围**：vulkan feature 接入 model-runtime + apps/desktop + apps/daemon、release-windows.yml 默认 GPU build、运行时 fallback CPU 守门、文档同步
> **不涉及**：cuda 路径、CPU + GPU 双轨 binary、macOS metal 路径调整、Linux GPU、benchmark suite 入 CI、AB test、`gpu_layers` 字面值调整（保持 BETA-25 简化版 99）

## §1 背景与动机

### §1.1 v0.8.0 Windows 真机痛点（动机起点）

STATUS 2026-06-29 真机会话日志记录的 v0.8.0 Windows 真机暴露 UX bug #4：

> embedding 模型首次冷启动 ~17s 直接撞 15s 搜索 timeout——`prewarm` 由 `spawn_semantic_index` 在 FTS reindex 之后才触发、期间用户搜任何 query 都必然超时；用户实测「错误：search timeout after 15011 ms」

根因 = CPU 跑 embeddinggemma-300m 加载 + warmup 在 ~17s 量级、撞 15s 搜索 timeout。**抬 timeout 是治标、上 GPU 才治本**——抬到 30s 用户仍要等 17s 看到第一个语义结果、UX 不可接受。

### §1.2 现状 — 基建已就位 90%

经 packages/model-runtime + apps 调研：

| 层 | 现状 | 本 cycle 待办 |
|---|---|---|
| `packages/model-runtime/Cargo.toml` | `vulkan = ["llama-cpp-4?/vulkan"]` **已预留** | 不动 |
| `packages/model-runtime/src/llama.rs` | `with_n_gpu_layers(gpu_layers)` 已透传、`gpu_layers > 0` 守门已就位 | 加运行时 fallback CPU 守门 |
| 桌面 / daemon 调用方 | embedding_model.rs / model_fallback.rs / daemon main.rs 全部已传 `gpu_layers: 99` | 不动 |
| `apps/desktop/src-tauri/Cargo.toml` | `model-fallback-metal` / `semantic-recall-metal` 已加 | 加 `-vulkan` 配套 |
| `apps/daemon/Cargo.toml` | 无 features section | 加 `vulkan` feature |
| `.github/workflows/release-windows.yml` | `--features model-fallback,semantic-recall`（CPU-only） | 切 `-vulkan` 变体 + install Vulkan SDK |
| 历史佐证 | `docs/reviews/beta-09a-windows-parity.md`：`gpu_layers=999（默认）全量卸载到 Vulkan0`、`模型加载 ~1s` ⭐ | 复用结论 |

### §1.3 vulkan vs cuda 决策（brainstorming 收敛）

选 **vulkan**：

| 维度 | vulkan | cuda |
|---|---|---|
| NVIDIA 性能 | 80-90% cuda | 顶配 |
| 无 GPU / AMD / Intel 用户 binary | **自动 fallback CPU 不崩** | 大概率 cudart64_*.dll 找不到启动崩 |
| binary 体积 / runtime 依赖 | vulkan-1.dll Windows 10/11 自带、无需 bundle | 需 bundle CUDA runtime ~500MB 或要求用户装 CUDA toolkit |
| CI install | Vulkan SDK ~30s | Jimver/cuda-toolkit Action ~3-5min |
| 项目佐证 | BETA-09a 已跑过 | 从未跑过 |
| 与「运行时回退 CPU」契合度 | **天然契合** | 矛盾（cuda binary 在无 cuda 机崩） |

cuda 唯一优势 = NVIDIA 顶配 5-15% 性能加成、不抵以上维度成本。本 cycle 红线（冷启动 < 5s / query < 100ms）vulkan 大概率能达成（llama.cpp vulkan + NVIDIA 上 300M 模型一般 token/s 50-150 / encode 路径单 doc < 50ms）。

## §2 接受标准与红线

### §2.1 验证门

| # | 红线 | 验证命令 | 目标 |
|---|---|---|---|
| 1 | rustfmt | `cargo fmt --all --check` | 净 |
| 2 | clippy（GPU feature 也跑）| `cargo clippy --workspace --all-targets --features semantic-recall-vulkan,model-fallback-vulkan -- -D warnings` | 0 warning |
| 3 | workspace test（无 feature 默认）| `cargo test --workspace` | 全过、不引入回归 |
| 4 | semantic_quality_gate（CPU 路径不动）| `cargo test -p locifind-evals --test semantic_quality_gate` | 1 passed、baseline.json 不动 |
| 5 | parser-only byte-equal | v0.5 / v0.9 evals 与 main byte-equal | 0 diff（不动 parser） |
| 6 | fixture SHA256 | 既有 fixture 与 main byte-equal | 0 diff |
| 7 | desktop build with vulkan feature | `cargo check -p locifind-desktop --features semantic-recall-vulkan,model-fallback-vulkan` | 编译通过、无 link 错 |
| 8 | daemon build with vulkan feature | `cargo build -p locifindd --features vulkan` | 编译通过、无 link 错 |
| 9 | 真机性能基准（本机 NVIDIA 独显）| 见 §2.2.1 | 冷启动 < 5s / 稳态 query < 100ms |
| 10 | 运行时 fallback CPU（mock no-GPU）| 见 §2.2.2 | 无 panic、降级时延符合预期 |

### §2.2 真机性能基准（必做、cycle 内）

#### §2.2.1 GPU 加速验证（必做）

本机环境：Windows 11 + NVIDIA 独显（型号 / VRAM cycle 末记录到 STATUS）+ LunarG Vulkan SDK 装好 + VS Build Tools C++ workload 装好。

步骤：

1. **净启 dev mode**：`npm run tauri dev --features semantic-recall-vulkan`
2. **看启动 log**：识别 `ggml_vulkan: Found 1 Vulkan devices` / `Device 0: <NVIDIA GPU 型号>`
3. **冷启动计时**：app 启动 → 第一次 semantic search 返回结果（含 prewarm）秒表
4. **稳态 query 时延**：连续相同输入 query × 10 次 → 均时延（含 tokenize + embedding + cosine + rank）
5. **任务管理器观察**：GPU 利用率（"GPU - 3D" 或 "GPU - Compute_0" 列）/ VRAM 占用
6. **对比 CPU baseline**：`--features semantic-recall`（不带 vulkan）跑同流程、记录差距

#### §2.2.2 运行时 fallback 测试（mock no-GPU）

- 临时 env `LOCIFIND_FORCE_CPU=1` → model load 路径强制 `gpu_layers=0` → embedding 仍工作、app 不崩、log 显示 fallback 信息
- 同时验：`--features semantic-recall-vulkan` 编译出的 binary 在 mock 无 Vulkan device 时（env 强制）也不崩

### §2.3 GO 判定

| Branch | 条件 | 行动 |
|---|---|---|
| **GO**（默认） | 红线 1-10 全过 | 落库 / doc-sync / PR / 合 main / 用户后续 bump v0.9.0 + Windows tag 触发 GPU release |
| **GO with documented gap** | 红线 1-8 全过 + §2.2.1 性能基准未完全达字面（如稳态 query 120ms vs 红线 100ms）但显著优于 CPU | 落库 / PR 标 [perf follow-up] / 合 main / 留 follow-up cycle 调 gpu_layers / KV cache |
| **NO GO** | 红线 1-8 任一不过、或 vulkan 加载 crash 无法绕过、或 §2.2.2 fallback 失败致 app 崩 | 不合并、回滚、cycle 标 done-with-rollback |

## §3 改动清单（YAGNI）

### §3.1 做什么

| # | 文件 | 改动 | 体量 |
|---|---|---|---|
| 1 | `apps/desktop/src-tauri/Cargo.toml` | 加 `model-fallback-vulkan = ["model-fallback", "locifind-model-runtime/vulkan"]` + `semantic-recall-vulkan = ["semantic-recall", "locifind-model-runtime/vulkan"]` | +4 行 |
| 2 | `apps/daemon/Cargo.toml` | 加 `[features]` section + `default = []` + `vulkan = ["locifind-model-runtime/vulkan", "locifind-model-runtime/llama-cpp"]` + `semantic-recall = ["locifind-model-runtime/llama-cpp"]`（daemon 当前默认 stub、CI binary 缺真模型已是 BETA-32 follow-up bug #1） | +6-8 行 |
| 3 | `packages/model-runtime/src/llama.rs` `worker_main` | 加 `LOCIFIND_FORCE_CPU` env 检测（入口）+ GPU 加载失败时 fallback CPU 重试（model load Err 路径）、加 tracing warn log；首次 fallback 后 worker 继续运行、不 panic | +25-40 行 |
| 4 | `packages/model-runtime/src/llama.rs` 模块注释 | BETA-31-v2 段：说明 fallback 行为 / env / 与 BETA-15B-9 cycle hypothesis 2 临时 gpu_layers=0 改动区别（彼为 hypothesis 探针、此为生产路径） | +10-15 行注释 |
| 5 | `.github/workflows/release-windows.yml` | (a) `--features` 加 `-vulkan` 变体；(b) 加 `Install Vulkan SDK` step（humbletim/install-vulkan-sdk@v1 或 LunarG direct download）；(c) Release body 更新 GPU 说明（默认 vulkan + 自动 fallback CPU + NVIDIA / AMD / Intel 通吃） | +15-20 行 |
| 6 | `docs/third-party-licenses.md` | 加 vulkan runtime / LunarG Vulkan SDK 说明（vulkan-1.dll Windows 系统组件 / LunarG SDK 各组件 license 由 install action 落地审查） | +5-8 行 |
| 7 | `apps/daemon/README.md` | §2 部署样板加 vulkan feature 说明（Linux / Windows 可选 GPU、env / build flag） | +8-12 行 |
| 8 | `apps/desktop/src-tauri/src/search/embedding_model.rs` | 仅加 BETA-31-v2 mod 顶部注释段、不改 `gpu_layers=99` 字面值 | +6-10 行注释 |

**总改动量预估**：~75-115 行（含注释）；纯逻辑 ~40-60 行。

### §3.2 不做什么

- ❌ **cuda 路径**（NVIDIA-only 维护成本不抵性能加成）
- ❌ **双轨 CPU + GPU binary**（vulkan + 运行时 fallback 已覆盖）
- ❌ **`gpu_layers` 字面值调整**（保持 99 = 全量卸载、与 BETA-25 简化版本一致）
- ❌ **`ModelLoadParams` 结构改动**（保持 BETA-25 简化版）
- ❌ **macOS metal / Linux GPU 路径**（macOS 已 work、Linux 不在 1.0 范围）
- ❌ **benchmark suite 入 CI**（一次性 cycle 内本机跑、不入 CI；CI runner 无 GPU）
- ❌ **AB test / feature flag 切换**（GO 后所有 Windows 用户默认走 GPU、由运行时 fallback 兜底）
- ❌ **vulkan-1.dll bundle 进 NSIS**（Windows 10/11 系统自带）
- ❌ **GPU 状态 UI 内嵌**（保留为 follow-up、本 cycle 只做 log + env、不动 EmbedStatus 渲染）

## §4 运行时 fallback CPU 设计（关键）

### §4.1 触发条件 + 检测点

| 触发 | 检测点 | 行为 |
|---|---|---|
| `LOCIFIND_FORCE_CPU=1` env | `LlamaModelImpl::spawn` 入口 | 直接传 `gpu_layers = 0` 走 worker、不尝试 GPU |
| `LlamaBackend::init()` 失败（系统无 Vulkan runtime） | `LlamaLoader::new` | 当前 fn 已返 Err、上游 EmbeddingModelHandle::load 已守门 `Failed` 状态、UX 显示 |
| `LlamaModel::load_from_file` 失败且 `gpu_layers > 0` | `worker_main` | tracing warn + 重试 `with_n_gpu_layers(0)` 一次、成功则继续、再失败则 ready_tx send Err |

### §4.2 实现伪代码

```rust
// worker_main 内：
let mut effective_gpu_layers = gpu_layers;
if std::env::var("LOCIFIND_FORCE_CPU").is_ok() {
    tracing::info!("LOCIFIND_FORCE_CPU=1 set, forcing CPU mode");
    effective_gpu_layers = 0;
}

let mut model_params = LlamaModelParams::default();
if effective_gpu_layers > 0 {
    model_params = model_params.with_n_gpu_layers(effective_gpu_layers);
}

let model = match LlamaModel::load_from_file(backend, path, &model_params) {
    Ok(m) => m,
    Err(e) if effective_gpu_layers > 0 => {
        // BETA-31-v2: GPU 加载失败时尝试 CPU 回退一次
        tracing::warn!("GPU model load failed ({}), retrying with CPU", e);
        let cpu_params = LlamaModelParams::default(); // gpu_layers = 0
        match LlamaModel::load_from_file(backend, path, &cpu_params) {
            Ok(m) => {
                tracing::warn!("Successfully loaded model on CPU after GPU fallback");
                m
            }
            Err(e2) => {
                let _ = ready_tx.send(Err(ModelError::LoadError(format!(
                    "Model load failed on both GPU and CPU: gpu={e}, cpu={e2}"
                ))));
                return;
            }
        }
    }
    Err(e) => {
        let _ = ready_tx.send(Err(ModelError::LoadError(format!(
            "Failed to load model: {e}"
        ))));
        return;
    }
};
```

注：`LlamaBackend::init()`（在 `LlamaLoader::new`）失败属另一条路径——系统级 Vulkan runtime 完全缺失时 backend init 直接挂、走 `EmbedState::Failed` 显示。本 cycle scope 不修这条路径（已是 BETA-23 既有行为、Settings 页可见原因）。

### §4.3 用户感知

- log（tracing INFO / WARN）显示 fallback 路径走过、用户可在设置页「日志」面板查看
- 顶栏 EmbedStatus 灯：GPU fallback CPU 仍显示 Ready 绿（embedding 工作正常）；只有 backend init 完全失败才显示 Failed
- **本 cycle 不在 UI 加 GPU/CPU 区分显示**（留 BETA-31-v3「设置页 GPU 状态行」 follow-up）

### §4.4 Build-time vs Run-time 责任

- **Build-time**：`--features vulkan` 把 vulkan backend code 编进 binary。无 vulkan feature 的 binary 即使有 GPU 也走 CPU。
- **Run-time**：llama.cpp Vulkan backend 在 device 列表为空时上游已支持静默 fallback CPU；但 model load 阶段（VRAM 不足等）可能仍 Err、需 §4.2 守门兜底。

## §5 真机基准对比预期（量化期望）

| 指标 | 当前 CPU baseline | vulkan + NVIDIA 期望 | 红线 |
|---|---|---|---|
| 模型加载时间（model load 完成 → ready） | ~12-15s | < 3s | — |
| 首次 query embedding（含 warmup） | ~17s | < 5s | ✅ 红线 9 |
| 稳态单次 query embedding | ~300-500ms | < 100ms | ✅ 红线 9 |
| VRAM 占用 | 0 | < 1GB（embeddinggemma-300m Q8_0 313MB + KV cache ~200MB） | 软目标 |
| GPU 利用率（query 时） | 0% | 30-80% | 软目标 |
| CPU 利用率（query 时） | 100% × 1-2 core | < 20% × 1 core | 软目标 |

参考：llama.cpp Vulkan 在 NVIDIA RTX 系列上对 300M 模型一般 token/s 50-150、encode 路径（BERT 系 arch）单 doc < 50ms 量级。

## §6 失败模式 + cycle 内决策

| 失败模式 | 处理 |
|---|---|
| Vulkan SDK 找不到 / `VULKAN_SDK` 环境变量未设 | Tier 0 fix（装 SDK、env 加 PATH）、cycle 内一次性 |
| build 出 binary 但 dev mode app 启动 crash | 紧急 fallback：临时 commit `LlamaModelParams::default()` 不 with_n_gpu_layers、cycle 标 NO GO + 回滚 |
| Vulkan device 探测到但模型加载 Err（VRAM 不足等） | §4 fallback 路径必接、cycle 内验 |
| 性能不达红线（稳态 query > 100ms） | 调 gpu_layers / KV cache size / batch；必要时标 GO with documented gap |
| 模型输出与 CPU baseline 数值漂移 | 接受漂移（vulkan vs CPU kernel 实现差异 / L2-norm 抵消大部分 / cosine sim 量级一致即可、不动 baseline.json） |
| cargo workspace 整体 build 受影响（其他 crate） | 红线 3 fail → cycle 拉回前一 commit |
| `LlamaBackend::init` 失败 | log Failed、不在 cycle scope 修（与 BETA-23 既有行为一致） |

## §7 真机手测剧本（cycle 收尾用）

到 `docs/manual-test-scenarios.md` 加 BETA-31-v2 节，含上述 §2.2.1 + §2.2.2 步骤、加 GPU 型号 / VRAM / 时延记录占位、cycle 收口后留用户填回真机数据。

## §8 与其他 cycle 的关系

- **BETA-31**：本 cycle 直接 follow-up。BETA-31 收尾的 follow-up 列表第 ① 项「BETA-31-v2 Windows GPU 推理优化（vulkan/cuda、~1-2w、需 Windows 真机）」就是本 cycle。
- **BETA-32**：daemon CI binary 当前 FTS-only（follow-up ⑥）；本 cycle 不解决该 bug、但加 daemon vulkan feature 为下次 release-daemon.yml 切到 hybrid 时一并启 vulkan 留接口。
- **BETA-15B-9**：曾在 hypothesis 2 / 3 临时改 `gpu_layers=0` 探针、cycle 末已还原。本 cycle 的 fallback 路径是**生产路径**、与 BETA-15B-9 的探针改动无关（worker_main 中加新分支、不是覆盖既有 gpu_layers）。
- **BETA-09(a)**：Windows 跨平台一致性出场报告记录过 vulkan 路径跑通（`gpu_layers=999 全量卸载到 Vulkan0`、模型加载 ~1s）；本 cycle 兑现该路径生产化。
- **v0.8.0 真机 5 个 UX bug**：本 cycle 解决 bug #4（embedding 冷启动 17s 撞 timeout）；其余 4 bug 不动、留 BETA-31-v3「v0.8.0 真机 UX gap 修复集」cycle。

---

**spec 由 Claude Code (Opus 4.7) 起草于 2026-06-29**。
