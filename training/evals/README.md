# training/evals

训练期评测脚本与训练专用评测集。

**状态**：未开始。MVP 末期开工。

## 与 [packages/evals](../../packages/evals/) 的区别

- `packages/evals`：**产品级** golden evals，用于回归 Harness 全链路（含规则解析、Schema、Policy、SearchBackend、UI）
- `training/evals`：**模型级**评测，专测模型本身的 NL → SearchIntent JSON 能力，用于训练期快速迭代

两者可以共享部分用例，但角色不同。
