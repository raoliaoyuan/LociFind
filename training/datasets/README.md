# training/datasets

LoRA 微调用训练数据集。

**状态**：v0 已落地（[v0.5-patch/v0](./v0.5-patch/v0/)，BETA-08 启动，2026-05-27，498 训练样本）。**BETA-24 新增 [lora-aug-keywords/v1](./lora-aug-keywords/v1/)**（2026-06-13，122 训练样本，keywords 补全，模板 + 手写汇编；来源见 [packages/evals/fixtures/lora-aug-keywords/v1/README.md](../../packages/evals/fixtures/lora-aug-keywords/v1/README.md)）。后续 Tier 2/3 augmentation 待评估。

## 计划内容

- 中文 / 英文 / 中英混合自然语言查询
- 常见错别字、口语表达、文件类型同义词
- 时间 / 路径 / 大小 / 排序表达
- 多轮上下文样本
- 首版 3,000–5,000 条

## 数据版本化（强制）

每个数据集必须有元信息：

```
dataset_name / version / source / license / generation_method / privacy_review_status / created_at / reviewer
```

## 严禁

- 真实用户文件名、路径、搜索词
- 未授权抓取的商业产品查询日志
- 未清洗的第三方敏感语料

详见 [docs/LociFind项目注意事项与风险清单.md §5](../../docs/LociFind项目注意事项与风险清单.md)。

`raw/` 子目录已在 `.gitignore` 中排除，避免大文件入库。
