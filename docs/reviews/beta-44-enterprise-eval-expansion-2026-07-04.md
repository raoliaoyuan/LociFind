# BETA-44：enterprise eval 扩容 22 → 53 case（2026-07-04）

> 护城河规划第 2 层（评测资产，详 [moat-plan-2026-07-04.md](./moat-plan-2026-07-04.md)）。
> 承接 [beta-40-enterprise-eval-2026-07-04.md](./beta-40-enterprise-eval-2026-07-04.md) 的 22 case baseline。

## 1. 结果

**真实模型首跑 53/53 全过**（`enterprise_scenarios --require-all`）：

| scenario | passed | total |
|---|---|---|
| lawfirm | 18 | 18 |
| audit | 16 | 16 |
| offboarding | 19 | 19 |
| **OVERALL** | **53** | **53** |

- 环境：Windows 11 本机、真实模型 `embeddinggemma-300m-q8_0.gguf`（CPU）、locifindd dev build（`locifind-model-runtime/llama-cpp`，含当日 BETA-43 出处/闸门代码）、topk=10、semantic_weight=default。
- 质量口径：42 条正样本中绝大多数期望路径 **top-1 命中**，最深排名第 3（L-01 判决书、O-04 账号清单 csv）；11 条越权负样本**缺省检索零跨集合泄漏 + 显式指名未授权集合全部被拒**。
- 既有 22 case 排名与 7-04 baseline 一致（无回归；O-09 图片语义 case 仍顶位）。

## 2. 新增 31 case 的设计（验收 ② 四类优先）

| 类别 | case | 说明 |
|---|---|---|
| **越权负样本**（8 新增，累计 11） | L-16/L-17/L-18、A-15/A-16、O-17/O-18/O-19 | 补齐跨 subject 矩阵：同场景不同案件利益冲突墙（zhang.san↛北原）、法务↛离职、审计↛法务/HR、HR↛技术、技术继任↛审计、同场景其他员工集合（wang.yangben↛王样本） |
| **跨语言 / 别名召回**（4 新增） | L-08（Northridge 当事人英文代称）、L-15（英文 query→北原尽调清单）、A-11（Morningstar 供应商别名）、A-12（Project Orion 项目代称） | 全部命中 top-1/2；配合既有 L-05/A-05/O-07 |
| **近重复干扰下排名稳定**（4 新增） | L-11/L-12（和解协议草稿 vs 签署版按措辞取对版本，各 top-1）、A-14/O-16（版本对全召回 @1+@2） | 新增和解协议签署版近重复对；既有 L-06/O-08 继续守 |
| **低清复扫件 OCR**（3 新增） | L-13、A-13、O-15（全部 top-1 定点命中） | 新增银行回单 / NDA 低清复扫材料，与原件构成近重复版本对 |
| 其余（12） | L-09/L-10/L-14、A-07~A-10、O-10~O-14 | 消化此前零覆盖材料（匿名举报 eml、报价打分 csv、验收异常/发票扫描件、值班 runbook、未结缺陷清单、离职确认单）；li.si 首次获得正样本（此前仅负样本） |

`.msg` 相关 case 仍挂 BETA-37b（等真实样本），PST 不做（验收 ③）。

## 3. 新增材料（4 份，合成、约整数占位、example.com 域名红线沿用）

- `lawfirm/case-2026-blueharbor/duplicates/settlement-draft-v2-signed.md` — 和解协议签署版（与草稿近重复）
- `lawfirm/case-2026-northfield/duediligence/northfield-financial-dd-checklist.md` — 北原尽调清单（li.si 正样本 + 英文别名）
- `audit/audit-2026-procurement/vouchers/bank-payment-receipt-rescan-low-quality.txt` — 银行回单低清复扫
- `offboarding/lishili-hr/scan-source/nda-rescan-low-quality.txt` — NDA 低清复扫

## 4. 闸门随 TSV 自然生长（验收 ④）

- `enterprise_scenarios_gate` 三条 fixture 完整性测试（TSV 合法 / 期望路径存在 / 期望内容落在 subject 授权 roots 内）对 53 case 常跑 CI 全过，未改一行测试代码；
- env 门控端到端（`LOCIFIND_DAEMON_BIN` + `LOCIFIND_MODEL_PATH`）自动覆盖新 case；
- query 编写经验沿用：每条正样本 query 至少含一个目标文件内真实出现的 ≥3 字 CJK 连续词或英文词（FTS trigram 锚点），语义臂负责口语化/跨语言部分——L-08"Northridge + 一审判决书"双轨即是此模式。

## 5. 机读报告

JSON 全量：本轮运行产物（含逐 case ranks / result_count / degraded）未入仓；复跑命令：

```text
cargo build -p locifindd --features locifind-model-runtime/llama-cpp
cargo run -p locifind-evals --bin enterprise_scenarios -- \
    --daemon-binary target/debug/locifindd.exe \
    --model-path <embeddinggemma-300m-q8_0.gguf> \
    --json report.json --require-all
```

## 6. 遗留

- 真实语料 case（BETA-44 卡注明"随真实样本滚动"）：待设计伙伴/首个真实部署（ROADMAP §5 P0）后补充真实脱敏样本；
- 2 字 CJK 泛词类 case 维持 O-09 一条（语义臂唯一兜底路径已实证），不为凑数复制。
