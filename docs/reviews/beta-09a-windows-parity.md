# BETA-09(a) — Windows 跨平台部署与一致性验证报告（**通过：双平台 0pp**）

| 项 | 值 |
|---|---|
| 日期 | 2026-06-01 |
| 操作者 | Claude Code (Opus 4.8) |
| 设备 | Windows 11 / Intel Core (8 逻辑核) / 16 GB RAM / Intel Iris Xe Graphics（核显，无独显）|
| 仓库 | `C:\Users\alice\dev\LociFind`（主仓库）|
| 模型 | v1 GGUF `main-v1-q4_k_m.gguf`（940 MB，sha256 `854125317fa478285eb939dc891e7844bb02cf4c11987d4340642e1698006b17`，与 macOS 训练产物一致）|
| 推理后端 | llama-cpp-4 0.3.0 + **Vulkan**（Intel Iris Xe，对照 macOS 的 Metal）|
| 前置 | [BETA-09 量化 baseline](../../training/mlx-lora/releases/v1.md)、[v1 出场报告](./beta-08-lora-v1.md) |
| 结论 | **BETA-09(a) 通过** — 准确性双平台 **0pp 差异**；延迟在弱核显上不达交互门槛（硬件等级问题，非正确性问题）|

## 1. 验证目标

验证「同一份 macOS 训练的 v1 GGUF 在 Windows 真机上的推理结果与 macOS 一致」，并解除 M→B 切换硬门「双平台 evals 通过率差 < 5pp」（此前从未实测）。

## 2. 核心结果：完整 500-case 双平台逐项对标

同一 GGUF（sha256 校验一致），Windows/Vulkan vs macOS/Metal（v1 基准）：

| 指标 | macOS/Metal (v1 基准) | Windows/Vulkan | 差异 |
|---|---|---|---|
| pass | 480 (96.0%) | **480 (96.0%)** | **0pp** |
| partial | 18 | **18** | 0 |
| fail | 2 | **2** | 0 |
| variant 命中率 | 99.6% | **99.6%** | 0 |
| 字段级精确匹配率 | 96.0% | **96.0%** | 0 |
| fallback 触发数 | 86 / 500 | **86 / 500** | 0 |
| rescued_to_pass | 8 | **8** | 0 |
| regressed | 0 | **0** | 0 |

**每一项数字逐一相同，差异 = 0pp**，远在 < 5pp 硬门内。残留 18 partial 为同款 bucket（artist / new_name / language 检测 / location 中英 hint），与 macOS v1 报告 §6 完全对应。variant 分桶（Clarify 39/1/0、FileAction 76/4/0、FileSearch 192/7/1、MediaSearch 94/6/0、Refine 79/0/1）亦与 macOS 一致。

**先期代表性子集对标**（5 个 rescued_to_pass case，覆盖 duration/location/size/keyword+sort 全部字段类型）：5/5 全部 pass 且 rescued_to_pass=1，与 macOS 逐一对上。

## 3. 延迟（关键发现：硬件等级差距）

| 仅 fallback 触发的 case | macOS/Metal (M5 Pro) | Windows/Vulkan (Iris Xe) | 门槛 |
|---|---|---|---|
| p50 | 1565 ms | **19597 ms** | — |
| p95 | 1586 ms | **21858 ms** | < 3000 ms ❌ |

- **准确性一致，但延迟差约 13×**——M5 Pro 高端 GPU vs 低端核显的硬件差距，非正确性问题。
- 纯 CPU 后端更差：单次 fallback 不可预测的几十秒+，500 全量 > 1 小时未完（本次先试 CPU、后切 Vulkan）。Vulkan 把延迟降到稳定 ~20s，但仍远超 3000ms 交互门槛。
- 模型 fallback 在低端核显上**准确但太慢，不适合交互式实时使用**。

## 4. Vulkan GPU 确认

```
ggml_vulkan: Found 1 Vulkan devices:
  0 = Intel(R) Iris(R) Xe Graphics | uma:1 | fp16:1 | 7431 MiB free
llama_prepare_model_devices: using device Vulkan0 (Intel Iris Xe)
load_tensors: layer 0..N assigned to device Vulkan0   ← 模型层卸载到 GPU
```

`gpu_layers=999`（默认）全量卸载到 Vulkan0。模型加载 ~1s。

## 5. Windows 隐藏前置（macOS 不暴露，本次真机解锁，已补 docs/windows-setup.md §5）

`packages/model-runtime` 此前从未在 Windows 上编译过 `llama-cpp` feature。连续暴露并解决：

1. **libclang 缺失**：`llama-cpp-sys-4` 经 `bindgen` 生成 FFI 绑定，需 LLVM/libclang（macOS Xcode 自带）。装 `LLVM.LLVM`，设 `LIBCLANG_PATH`。
2. **CMake**：llama.cpp 编译前置。装 `Kitware.CMake`。
3. **MSBuild 不认 `-j8`**：默认 Visual Studio CMake 生成器下，`cmake` crate 传给 MSBuild 的 `-j8` 报 `MSB1001`。解法：在 VS 开发者环境（vcvars64）里设 `CMAKE_GENERATOR=Ninja` 用 VS 自带 Ninja。
4. **Vulkan GPU 加速**：装 Vulkan SDK（`KhronosGroup.VulkanSDK`），设 `VULKAN_SDK`，evals 用新增 feature `model-fallback-vulkan` 编译。

## 6. 结论与建议

1. **BETA-09(a) 通过，BETA-09 标 done**。准确性跨平台 0pp 一致——「本地优先、跨平台一致」核心承诺用硬数据兑现，M→B 模型侧硬门解除。
2. **延迟发现喂给 [BETA-17 基座选型实验](../../ROADMAP.md)**：1.5B 在弱核显 p95 ~22s → Qwen3-0.6B 等更小模型有望大幅压低。
3. **能力感知降级**：弱硬件默认走纯 parser（472/500=94.4%、即时），模型 fallback 作「检测到强 GPU 才启用」的可选增强（+8 case / +1.6pp 不值 ~20s 等待）。
4. **可复用工作流**：「Mac 训练 → 传 GGUF（校 sha256）→ Windows llama.cpp/Vulkan 推理」已验证；同架构同量化换模型只需换 `LOCIFIND_MODEL_PATH`，无需重编；换架构（如 Qwen3）需重验 llama.cpp 支持 + 小子集对标。

## 7. 复现命令

```bat
:: 前置：CMake / LLVM / VS BuildTools(C++) / Vulkan SDK 已装（见 windows-setup.md §5）
call "...\VC\Auxiliary\Build\vcvars64.bat"
set "CMAKE_GENERATOR=Ninja"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
set "VULKAN_SDK=C:\VulkanSDK\1.4.350.0"
set "LOCIFIND_MODEL_PATH=...\training\mlx-lora\main-v1-q4_k_m.gguf"
cargo run -p locifind-evals --features model-fallback-vulkan --bin evals --release -- --fixtures v0.5 --with-fallback --hybrid
```
