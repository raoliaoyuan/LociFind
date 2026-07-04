# Mac 会话待跑清单

> 用途：记录**只能/最适合在 Mac（Apple Silicon + Metal）上跑**的任务，便于切到 Mac 会话时直接照做。
> 缘起：2026-06-03 BETA-13 收工后讨论「自然语言缺口能否靠本地 LLM 解决」+「Mac 跑实验是否更快」。
> 维护：跑完一项就勾掉 / 移除；新增 Mac-only 任务追加于此。Windows 侧已能做的**准备工作**标注在各项里。

## 机器对比结论（为什么放 Mac）

LLM 推理瓶颈是 GPU 加速 + 内存带宽，恰是 Apple Silicon 强项：
- 当前 Windows（i7-1165G7，纯 CPU、Iris Xe 用不上、仅 ~3GB 空闲）：0.6B Q4 约 2-8 秒/条，全 1000 条 ~1-1.5 小时。
- Mac（M5 Pro / 64GB，Metal）：预计 ~亚秒/条，全 1000 条 ~5-15 分钟（**约 10× 墙钟**），且 64GB 可从容跑更大模型（main-v1 0.92GB）测质量上限。

**本地 GGUF 模型**（`training/mlx-lora/`，无需下载）：
- `beta17-qwen3-0.6b-q4_k_m.gguf`（370MB，BETA-17 基座 bake-off winner，evals 默认模型）
- `main-v0-q4_k_m.gguf` / `main-v1-q4_k_m.gguf`（各 920MB，BETA-08 LoRA 训练版，更强）

---

## 1. LLM fallback 实验（最高优先 — 回答「自然语言缺口能否靠本地 LLM 解决」）

> **✅ 已核销（2026-06-20）——本节驱动问题已在产品路径上直接回答，无须再单独跑 Mac 上限实验。**
> - keyword 欠抽取触发信号 → **已落地**：BETA-23 加第七类「keywords 内容词覆盖检测」触发器（FileSearch 臂）、BETA-24 扩 MediaSearch 臂、G13 改 fill-empty-only（`packages/intent-parser/src/fallback.rs::analyze_structural_omissions`）。
> - 「LLM 能否补缺口」→ **已量并部署**：BETA-24 重训 LoRA 后 held-out keywords 补全 **90%**（旧 0%），with-fallback evals **regressions=0**，模型已部署本机产品路径。
> - 原计划的 `--force-fallback` 纯上限跑分 + 0.6B vs main-v1 质量对比，已被 BETA-17 基座 bake-off + BETA-24 重训路径取代，不再需要。
> 下方原始清单**逐字保留**仅供追溯。

**背景**：BETA-13 的 v0.9 baseline **51.4% 是 parser-only**，未走模型路径。hybrid 架构本有本地 Qwen fallback（MVP-17，GBNF 约束输出合法 SearchIntent），但两个障碍：
- **触发盲点**：`packages/intent-parser/src/fallback.rs::analyze_structural_omissions` 只检 time/size/sort/location/action/media 漏字段，**没有 keyword 欠抽取信号** → 头号 gap（183 例 keyword）的 case 不会触发 fallback，模型不被调用。
- **未测上限**：本地小模型对自然语言 query 的真实抽取能力没量过。

**Windows 侧准备工作（切 Mac 前先在本机做完 + 提交）**：
- [ ] evals bin 加 `--force-fallback`（所有 case 强制走模型，绕过触发器，用于量 LLM 纯上限）。状态：**未做**。
- [ ] （可选，也可作 BETA-13-G1 一部分）给 `analyze_structural_omissions` 加 keyword 欠抽取信号（query 有内容名词短语但 parser keywords=None → 触发 fallback）。状态：**未做**。
  > 这两项纯 Rust、与机器无关，本机快。做完提交后 Mac 直接拉取。

**Mac 侧执行**：
- [ ] 构建：`cargo build --features model-fallback-metal -p locifind-evals`（Metal 加速；前置 cmake + Xcode Command Line Tools）。
- [ ] 跑 0.6B：
  ```bash
  cargo run --features model-fallback-metal -p locifind-evals --bin evals -- \
    --fixtures v0.9 --with-fallback --force-fallback \
    --model-path training/mlx-lora/beta17-qwen3-0.6b-q4_k_m.gguf --json > v09-fallback-0.6b.json
  ```
- [ ] 跑 main-v1（更强模型，测质量上限）：同上换 `--model-path training/mlx-lora/main-v1-q4_k_m.gguf`。
- [ ] 量化对比：① coverage pass 从 8% 抬到多少 ② 按 gap 类别看 LLM 补了哪些（keywords/artist/媒体措辞/refine 标记…）③ 每条延迟 p50/p95 ④ 0.6B vs main-v1 质量差。
- [ ] 产出：写入 `packages/evals/fixtures/v0.9/README.md` 的 baseline 报告（加「parser-only vs +fallback」对比表），据此决定是否把 keyword 触发 + fallback 接进产品路径（关联 ROADMAP **BETA-13-G1**）。

---

## 2. Class A 双平台 evals（M→B 切换硬指标，长期 blocker）

**背景**：M→B 正式切换的硬指标「双平台 evals 差距 <5pp」**从未物理跑过**——macOS Spotlight 后端只能在 Mac 跑。

- [ ] Mac 装好**完整 Spotlight 索引**后，跑 v0.5 + v0.9 parser-only。
- [ ] 与 Windows 结果对比（byte-equal / pass 率），核验「双平台差距 <5pp」。
- [ ] 关联 ROADMAP：BETA-09(a) / MVP-26 跨平台一致性 / MVP-28 出场评测。

---

## 3. macOS-only 代码项

- [ ] **BETA-03 macOS Vision OCR**：trait 已抽象，需 Swift helper + 签名（留 Mac 会话）。
- [ ] **Mac bundle.targets 验证**：`npm run tauri build` 应直接出 `.app`/`.dmg`（`apps/desktop/src-tauri/tauri.macos.conf.json` 已加，Windows 不能实测，需 Mac 确认）。
- [ ] **macOS UI 回归**：BETA-16 表格 UI / BETA-20 预览面板 / BETA-21 隐私面板 / BETA-22 搜索历史 等前端在 macOS 的视觉/交互回归。

---

## 4. 分发（需外部条件，Mac 相关）

- [ ] **BETA-10 macOS DMG 签名 + 公证 + Stapler** — 阻塞于 Apple Developer 账号注册（ROADMAP §5）。不解锁账号无法做。

---

## 命令 / 路径速查

- GGUF 模型：`training/mlx-lora/*.gguf`
- Metal 构建 feature：`model-fallback-metal`（叠在 `model-fallback` 上，仅 macOS）
- evals bin 参数：`--fixtures {v0.1|v0.5|v0.9}` / `--with-fallback` / `--model-path <gguf>` / `--json` / `--only-failures` / `--case <id>`
- v0.9 说明 + baseline：`packages/evals/fixtures/v0.9/README.md`
- 相关 ROADMAP task：BETA-13-G1~G7（parser 缺口）、BETA-09(a)/MVP-26/28（双平台）、BETA-10（DMG 签名）
