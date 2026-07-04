# training/generators

合成训练数据的生成脚本。

**状态**：v0 生成器已落地，住在 [`packages/evals/src/bin/build_lora_dataset.rs`](../../packages/evals/src/bin/build_lora_dataset.rs)（Rust binary，复用 evals fixture loader + intent-parser hybrid 模块）。本目录后续如需更复杂的 LLM-augmented 生成器（Tier 2/3）才会再独立成 Python 项目。

## 计划职责

- 模板化生成各类查询样本（文件搜索、媒体搜索、文件操作）
- 同义词扩展、时间表达变体、错别字注入
- 输出符合 Search Intent JSON Schema 的标注样本
- 自动跑 Schema 校验
- 输出版本化的 dataset（写入 [training/datasets/](../datasets/)）
