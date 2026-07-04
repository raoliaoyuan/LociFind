use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("模型加载失败: {0}")]
    LoadError(String),
    #[error("推理失败: {0}")]
    InferenceError(String),
    #[error("模型路径无效: {0}")]
    InvalidPath(String),
    #[error("后端初始化失败: {0}")]
    BackendError(String),
    #[error("未知错误: {0}")]
    Unknown(String),
}

/// 模型生成参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateParams {
    /// 最大生成 token 数
    pub max_tokens: usize,
    /// 温度参数
    pub temperature: f32,
    /// Top-P 采样
    pub top_p: f32,
    /// 停止词列表
    pub stop_sequences: Vec<String>,
    /// 随机种子
    pub seed: u32,
    /// 可选 GBNF 语法约束（MVP-17 受限解码）。仅 llama-cpp 后端生效，stub/candle 忽略。
    /// `None` 表示不约束（默认采样链）。
    #[serde(default)]
    pub grammar: Option<String>,
    /// BETA-17：JSON 任务下，第一个完整 JSON 对象闭合即停止生成。
    /// 小模型输完一个对象后常"复读"到 `max_tokens`，而调用方只取首个对象 —— 多出的
    /// token 在弱核显上纯属浪费 decode。开启后大幅缩短弱硬件单次推理延迟。
    /// 仅 llama-cpp 后端生效，stub/candle 忽略。
    #[serde(default)]
    pub stop_at_json: bool,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            stop_sequences: Vec::new(),
            seed: 42,
            grammar: None,
            stop_at_json: false,
        }
    }
}

/// BETA-17：判断 `s` 中第一个完整 JSON 对象是否已闭合（花括号深度归零）。
///
/// 用于 [`GenerateParams::stop_at_json`] 的提前停止：小模型输完一个 JSON 对象后常
/// "复读"到 `max_tokens`，而调用方只取首个对象，多出的 token 在弱核显上是纯浪费的
/// decode。正确忽略字符串字面量内的 `{`/`}` 与 `\"` 转义，避免值（如文件名）里的
/// 括号造成误判。
#[cfg_attr(not(feature = "llama-cpp"), allow(dead_code))]
pub(crate) fn first_json_object_complete(s: &str) -> bool {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escape = false;
    let mut seen_open = false;
    for ch in s.chars() {
        if escape {
            escape = false;
        } else if in_string {
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
        } else {
            match ch {
                '"' => in_string = true,
                '{' => {
                    depth += 1;
                    seen_open = true;
                }
                '}' => {
                    depth -= 1;
                    if seen_open && depth <= 0 {
                        return true;
                    }
                }
                _ => {}
            }
        }
    }
    false
}

/// 模型加载参数
#[derive(Debug, Clone, Copy, Default)]
pub struct ModelLoadParams {
    /// GPU 层数 (Metal/CUDA)
    pub gpu_layers: u32,
    /// 上下文大小
    pub context_size: u32,
}

/// 模型运行时 trait
pub trait LlamaModelRuntime: Send + Sync {
    /// 生成文本
    fn generate(&self, prompt: &str, params: &GenerateParams) -> Result<String, ModelError>;

    /// BETA-17：带固定前缀缓存的生成。`prefix` 在多次调用间复用其 KV（仅 llama-cpp 后端
    /// 实现真缓存；默认实现直接拼接 `prefix + suffix` 走 [`generate`]，stub/candle 用此回退）。
    /// 调用方应保证 `prefix` 跨调用稳定（如 hybrid prompt 的固定指令块）才有收益。
    fn generate_cached_prefix(
        &self,
        prefix: &str,
        suffix: &str,
        params: &GenerateParams,
    ) -> Result<String, ModelError> {
        let mut prompt = String::with_capacity(prefix.len() + suffix.len());
        prompt.push_str(prefix);
        prompt.push_str(suffix);
        self.generate(&prompt, params)
    }

    /// 生成单段文本的句向量（已做 L2 归一化；具体池化方式由后端决定）。
    /// 仅 embedding 模型有意义；BETA-26 探针用。
    /// 默认实现返回错误：非 embedding 后端（stub/candle）不支持。
    fn embed(&self, _text: &str) -> Result<Vec<f32>, ModelError> {
        Err(ModelError::InferenceError(
            "embed not supported by this backend".to_owned(),
        ))
    }
}

/// 模型加载器 trait
pub trait ModelLoader: Send + Sync {
    /// 加载模型
    fn load(
        &self,
        path: &Path,
        params: &ModelLoadParams,
    ) -> Result<Box<dyn LlamaModelRuntime>, ModelError>;
}

#[cfg(feature = "candle")]
pub mod candle_loader;
pub mod daemon;
#[cfg(feature = "llama-cpp")]
pub mod llama;
#[cfg(feature = "llama-cpp")]
mod pooling;
pub mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "candle")]
pub use candle_loader::CandleLoader;
pub use daemon::{DaemonStatus, ModelDaemon, SharedModelDaemon};
#[cfg(feature = "llama-cpp")]
pub use llama::LlamaLoader;
pub use stub::StubLoader;

/// 根据 feature 选择默认加载器
#[must_use]
pub fn get_default_loader() -> Box<dyn ModelLoader> {
    #[cfg(feature = "llama-cpp")]
    {
        if let Ok(loader) = LlamaLoader::new() {
            return Box::new(loader);
        }
    }

    #[cfg(feature = "candle")]
    {
        return Box::new(CandleLoader::new());
    }

    Box::new(StubLoader)
}
