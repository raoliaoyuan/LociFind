# LociFind LoRA Releases

此目录登记每个 LoRA adapter release 的元数据（sha256 / size / 训练参数 / 实测指标 / 使用方法），作为分发追溯的**单一信源**。

训练产物（adapter / GGUF）本身 git-ignored；本目录文档化它们的 hash 与生产参数，便于下载方 verify。

## Releases

| 版本 | 日期 | 状态 | 入口 | 推荐推理 GGUF |
|---|---|---|---|---|
| v1 | 2026-05-28 | ready | [v1.md](./v1.md) | `main-v1-q4_k_m.gguf` (940 MB) |

## 添加新 release

每个新 release 加一个独立 `vN.md`（与 v1.md 同结构），并在上表追加一行。模板字段：

1. 元数据（日期 / commit / 训练机器 / 基座 / spec / plan / 出场报告）
2. 训练参数（dataset + sha256 / mlx-lm 版本 / 超参）
3. Artifacts（sha256 + size 表）
4. v0.5 evals 实测对比表
5. 使用方法
6. 已知限制
7. 变更历史 vs 上一版
8. 依赖与许可
9. 下一版本路标
10. 追溯信息
