#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::float_cmp
)]

use crate::{
    first_json_object_complete, get_default_loader, GenerateParams, ModelLoadParams, ModelLoader,
    StubLoader,
};
use std::path::PathBuf;

#[test]
fn test_generate_params_default() {
    let params = GenerateParams::default();
    assert_eq!(params.max_tokens, 512);
    assert_eq!(params.temperature, 0.7);
    assert_eq!(params.seed, 42);
    // BETA-17：默认不提前停（向后兼容，full 路径行为不变除非显式开启）。
    assert!(!params.stop_at_json);
}

// BETA-17：`first_json_object_complete` —— 首个 JSON 对象闭合检测（stop_at_json 用）。
#[test]
fn json_complete_flat_object() {
    assert!(first_json_object_complete(r#"{"size":null}"#));
}

#[test]
fn json_incomplete_while_open() {
    // 还没闭合 —— 仍在生成中。
    assert!(!first_json_object_complete(r#"{"size":"#));
    assert!(!first_json_object_complete(r#"{"a":{"b":1}"#)); // 外层未闭
    assert!(!first_json_object_complete(""));
    assert!(!first_json_object_complete("前导文字还没出现对象"));
}

#[test]
fn json_complete_nested_object() {
    // 嵌套对象：只有外层闭合才算完成。
    assert!(first_json_object_complete(
        r#"{"modified_time":{"type":"relative","value":"last_7_days"}}"#
    ));
}

#[test]
fn json_braces_inside_string_ignored() {
    // 字符串值里的花括号不参与深度计数（否则会过早或过晚停止）。
    assert!(first_json_object_complete(r#"{"name":"a}b{c"}"#));
    assert!(!first_json_object_complete(r#"{"name":"}"#)); // 括号在未闭合字符串内
}

#[test]
fn json_escaped_quote_in_string() {
    // 转义引号不应误判字符串结束。
    assert!(first_json_object_complete(r#"{"k":"he said \"hi\" }"}"#));
}

#[test]
fn json_stops_at_first_object_repeated() {
    // 模型"复读"场景：首个对象闭合即视为完成（后续重复内容被忽略）。
    assert!(first_json_object_complete(r#"{"sort":"size_desc"}{"sort"#));
}

#[test]
fn test_stub_loader() {
    let loader = StubLoader;
    let model = loader
        .load(&PathBuf::from("mock.gguf"), &ModelLoadParams::default())
        .unwrap();
    let response = model.generate("Hello", &GenerateParams::default()).unwrap();
    assert_eq!(response, "Echo: Hello");
}
#[test]
fn test_get_default_loader() {
    let loader = get_default_loader();
    // Default in test should be stub unless features are enabled.
    // 当 `cargo test --workspace` 经 feature 统一把 llama-cpp 真 loader 拉进来时
    // （如 throwaway 的 spike-retrieval 无条件开它），占位 "mock.gguf" 无法被真 loader
    // 加载 → 加载失败即跳过（本测试只验默认 stub loader 的 echo 行为）。
    let Ok(model) = loader.load(&PathBuf::from("mock.gguf"), &ModelLoadParams::default()) else {
        return;
    };
    let response = model.generate("Test", &GenerateParams::default()).unwrap();
    assert!(response.contains("Test"));
}

#[test]
#[ignore = "需要真实 candle 模型文件，CI 无模型时跳过"]
fn test_candle_e2e() {
    #[cfg(feature = "candle")]
    {
        let loader = CandleLoader::new();
        // This requires a real model file
        let model_path = PathBuf::from("models/qwen2.5-1.5b-instruct-q4_k_m.gguf");
        if model_path.exists() {
            let model = loader
                .load(&model_path, &ModelLoadParams::default())
                .unwrap();
            let response = model.generate("你好", &GenerateParams::default()).unwrap();
            println!("Response: {response}");
            assert!(!response.is_empty());
        }
    }
}

// BETA-25：真机冒烟——验证静态链接的 llama 后端能加载已部署模型并产出非空生成。
// 默认 ignore（需真实 gguf + Metal）。运行：
//   cargo test -p locifind-model-runtime --features llama-cpp,metal beta25_static_llama_smoke -- --ignored --nocapture
#[cfg(feature = "llama-cpp")]
#[test]
#[ignore = "需真实 gguf 模型 + llama-cpp 后端；CI 无模型时跳过"]
fn beta25_static_llama_smoke() {
    use crate::{GenerateParams, LlamaLoader, ModelLoadParams, ModelLoader};

    // dirs 不是 model-runtime 依赖；优先读环境变量，回退绝对路径（macOS 部署位置）。
    let model_path: PathBuf = std::env::var("LOCIFIND_BETA25_MODEL").map_or_else(
        |_| {
            PathBuf::from(
                "/Users/alice/Library/Application Support/LociFind/models/qwen3-0.6b-q4_k_m.gguf",
            )
        },
        PathBuf::from,
    );

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
    println!("生成结果：{out}");
    assert!(!out.trim().is_empty(), "生成结果为空");
}
