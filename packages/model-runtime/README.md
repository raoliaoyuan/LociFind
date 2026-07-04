# locifind-model-runtime

LociFind 本地小模型推理运行时。

## 功能

- 支持 GGUF 格式模型（量化 Q4_K_M）
- 抽象 `ModelLoader` 与 `LlamaModelRuntime` trait
- 多后端支持：
  - `llama-cpp` (默认，通过 `llama-cpp-4` 绑定，需系统安装 `cmake`)
  - `candle` (纯 Rust 实现，无需 `cmake`，作为 fallback 或轻量环境使用)
  - `stub` (仅回声，用于 CI 和快速原型)
- 跨平台特征：`metal` (macOS), `cuda` (NVIDIA), `vulkan` (Windows/Linux)

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
locifind-model-runtime = { path = "../../packages/model-runtime", features = ["candle"] }
```

### 2. 调用示例

```rust
use locifind_model_runtime::{get_default_loader, ModelLoadParams, GenerateParams};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loader = get_default_loader();
    let model = loader.load(
        &PathBuf::from("models/qwen2.5-1.5b-instruct-q4_k_m.gguf"),
        &ModelLoadParams::default()
    )?;

    let params = GenerateParams {
        max_tokens: 128,
        temperature: 0.7,
        ..Default::default()
    };

    let response = model.generate("查找昨天修改过的 PDF 文件", &params)?;
    println!("模型输出: {}", response);

    Ok(())
}
```

## 环境变量

- `LOCIFIND_MODEL_PATH`: 模型文件搜索的基础路径。

## 故障排除

### 无法编译 `llama-cpp` 特性

`llama.cpp` 的编译依赖 `cmake` 和 C++17 编译器。如果在 macOS 上遇到 `cmake not found`，请确保已安装：

```bash
brew install cmake
```

如果无法安装 `cmake`，请改用 `candle` 特性：

```bash
cargo build --features candle --no-default-features
```

## 测试模型推荐

- **基座模型**: Qwen2.5-1.5B-Instruct-GGUF
- **量化格式**: Q4_K_M
- **下载地址**: [Hugging Face - Qwen2.5-1.5B-Instruct-GGUF](https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF)
