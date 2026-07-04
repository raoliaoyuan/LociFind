# LociFind 知识产权保护计划书

## 1. 文档目的

本文件用于指导 LociFind 本地个人搜索 Agent 项目的知识产权保护工作，覆盖商标、域名、版权、第三方开源授权、系统搜索后端（Spotlight / Windows Search / Everything）集成合规、Apple / Microsoft 品牌使用、本地模型与训练数据权利、跨平台上架与发布前法务检查等事项。

本项目目标是跨平台（macOS + Windows）。下文凡涉及"系统搜索集成"默认覆盖 Spotlight 和 Windows Search 两套实现，Everything 作为 Windows 可选加速后端。

本文件不是法律意见，不能替代律师或商标代理机构的正式意见。正式商业发布、融资、上架应用商店或进入企业客户前，应由专业律师/商标代理对商标、版权、许可协议和隐私条款进行审查。

## 2. 项目品牌概况

拟定品牌：

```text
LociFind
```

中文表达：

```text
LociFind 本地个人搜索 Agent
洛希搜索
```

推荐英文描述：

```text
LociFind is a local-first, cross-platform personal search agent for your files, documents, media, and memories on macOS and Windows.
```

推荐中文描述：

```text
LociFind 是一个本地优先、跨平台（macOS 与 Windows）的个人搜索 Agent，帮助用户用自然语言查找电脑里的文件、文档、音乐、图片和记忆线索。
```

品牌使用原则：

- 对外统一使用 `LociFind`，不要拆成 `Loci Find`。
- 不要单独突出 `LOCI` 作为品牌主体。
- 不要使用 `LOCI AI`、`Loci Search`、`Loci Agent` 作为主品牌。
- 未获得注册前可以使用 `LociFind™`。
- 商标注册成功后，才能在对应司法辖区使用 `LociFind®`。

## 3. 当前初筛结论

### 3.1 精确名称

公开初筛未发现 `LociFind`、`LOCIFIND`、`Loci Find` 的明显同名成熟产品或精确商标冲突。

### 3.2 近似名称风险

`LOCI` 作为核心词，在软件、AI、SaaS、AI memory、3D asset search、能源软件等领域已有使用和注册痕迹。

风险点：

- 美国有 `LOCI` 注册商标，覆盖 Class 42 SaaS/PaaS。
- 欧盟有 `Loci` 注册，覆盖软件、硬件、能源/充电管理相关 Class 9/42 等。
- 市面上存在 `LOCI AI`、`Loci` AI 资产管理、`Loci` 物品定位/记忆类产品。

初步风险评级：

```text
精确冲突：低
近似冲突：中
整体商标风险：中等
```

### 3.3 建议

可以继续推进 `LociFind`，但建议在公开发布前尽快提交商标申请，且始终使用完整品牌 `LociFind`，避免只使用 `Loci`。

## 4. 商标保护计划

### 4.1 商标申请主体

优先方案：

```text
公司主体申请
```

原因：

- 便于融资、商业授权、应用商店上架和企业合同。
- 避免后续个人转让给公司产生额外手续和税务/合规问题。

过渡方案：

```text
个人先申请，后续转让给公司
```

适用条件：

- 公司尚未成立。
- 希望尽快锁定申请日。

注意事项：

- 后续转让需办理商标转让。
- 公司成立后应尽快完成权属归集。

### 4.2 申请商标形式

第一优先级：

```text
LociFind
```

类型：

```text
文字商标 / Word Mark
```

原因：

- 保护范围比 logo 更灵活。
- 不依赖字体、颜色、图形。
- 适合早期产品名保护。

第二优先级：

```text
LociFind logo
```

可在产品视觉确定后再申请。

第三优先级：

```text
中文名：洛希搜索
```

是否申请中文名取决于中国市场推广强度。如果主要面向中国用户，建议一起申请中文名。

### 4.3 推荐申请地区

第一批：

```text
中国
美国
```

原因：

- 中国：开发、销售、中文传播和潜在用户市场。
- 美国：AI/软件商业化、融资、应用商店、国际品牌保护。

第二批：

```text
欧盟
英国
新加坡
香港
```

适用条件：

- 产品进入国际化阶段。
- 公司主体、支付主体或客户市场涉及这些地区。
- 有企业客户或渠道合作需求。

### 4.4 推荐 Nice 类别

第一优先级类别：

```text
Class 9
Class 42
```

Class 9 覆盖方向：

- 可下载计算机软件
- 桌面应用程序
- 本地 AI 软件
- 文件搜索软件
- 数据索引软件
- 自然语言搜索软件
- 个人信息管理软件

Class 42 覆盖方向：

- 软件即服务 SaaS
- 平台即服务 PaaS
- 人工智能软件设计和开发
- 搜索引擎技术服务
- 数据检索技术服务
- 本地/私有化 AI 搜索技术服务
- 计算机软件维护与更新

可选类别：

```text
Class 35
Class 38
Class 41
```

适用情况：

- Class 35：企业知识管理、数据管理咨询、商业信息检索。
- Class 38：在线数据传输、通信服务。
- Class 41：教育培训、软件教程、知识库内容服务。

MVP 阶段建议先申请 Class 9 + Class 42。

### 4.5 正式商标检索清单

找商标代理或律师时，应要求覆盖：

- 精确检索：`LociFind`
- 大小写检索：`LOCIFIND`
- 空格检索：`Loci Find`
- 近似拼写：`LociFinder`、`LocaFind`、`LocalFind`、`LocusFind`、`LociFinder AI`
- 音近检索：`LowSeeFind`、`LoqiFind`、`LokiFind`
- 词根检索：`Loci`、`Locus`
- 中文近似：`洛希`、`洛西`、`罗希`、`洛希搜索`
- 同类别检索：Class 9、Class 42
- 相关类别检索：Class 35、38、41
- 互联网使用痕迹检索
- App Store / Microsoft Store / Google Play 检索
- GitHub / PyPI / npm / crates.io 检索
- 域名和公司名检索

### 4.6 申请流程

通用流程：

```text
确定申请主体
  ↓
确定商标文字和类别
  ↓
正式商标检索
  ↓
律师/代理出具风险意见
  ↓
提交申请
  ↓
受理
  ↓
审查
  ↓
公告
  ↓
注册
```

大致周期：

- 中国：通常 6-9 个月左右，可能更久。
- 美国：通常 8-12 个月左右，可能更久。
- 欧盟：通常 4-8 个月左右，视异议情况而定。

### 4.7 使用规范

注册前：

```text
LociFind™
```

注册后，在已注册地区：

```text
LociFind®
```

禁止：

- 未注册前使用 `®`。
- 暗示获得第三方背书。
- 将 `Loci` 单独作为主品牌。
- 使用与已有 `LOCI AI`、`Loci` 产品过于相似的 logo 或视觉识别。

## 5. 域名保护计划

### 5.1 当前域名状态

初步 RDAP 查询结果：

| 域名 | 状态 | 备注 |
|---|---|---|
| locifind.com | 已注册 | 2025-10-07 注册，Hostinger，当前像停放域名 |
| locifind.ai | 看起来可注册 | RDAP 返回 404 |
| locifind.app | 看起来可注册 | RDAP 返回 404 |
| locifind.dev | 看起来可注册 | RDAP 返回 404 |
| locifind.io | 看起来可注册 | RDAP 返回 404 |
| locifind.net | 看起来可注册 | RDAP 返回 404 |
| locifind.co | 看起来可注册 | RDAP 返回 404 |
| locifind.org | 看起来可注册 | RDAP 返回 404 |
| locifind.xyz | 看起来可注册 | RDAP 返回 404 |
| locifind.cn | 看起来可注册 | RDAP 返回 404 |

域名可用性会随时变化，实际注册前应以注册商实时查询为准。

### 5.2 优先注册组合

第一优先级：

```text
locifind.ai
locifind.app
locifind.dev
```

用途建议：

- `locifind.ai`：主官网、品牌首页。
- `locifind.app`：下载页、应用介绍页。
- `locifind.dev`：开发者文档、API、MCP、插件生态。

第二优先级：

```text
locifind.io
locifind.net
locifind.cn
```

第三优先级：

```text
locifind.co
locifind.org
locifind.xyz
```

### 5.3 .com 处理策略

`locifind.com` 已注册，不建议早期高价购买。

建议：

- 先注册可用后缀。
- 正式商标申请提交后再观察 `.com`。
- 若项目增长明显，再考虑通过经纪人匿名询价。
- 不要公开表达强烈购买意图，以免抬高价格。

### 5.4 域名安全配置

注册后立即：

- 开启自动续费。
- 开启注册商账户 2FA。
- 开启域名锁定。
- 使用隐私保护。
- 配置独立管理员邮箱。
- 记录域名注册商、到期时间、DNS 服务商。
- 为关键域名设置多管理员或公司账户托管。

### 5.5 防御性注册

可考虑注册常见错误拼写：

```text
locifinder.ai
locifindr.ai
locifinds.ai
locifindapp.com
```

是否注册取决于预算，不是 MVP 必需。

## 6. 版权保护计划

### 6.1 自动版权

项目代码、文档、UI 文案、训练数据生成脚本、官网内容、logo 设计等作品在创作完成时通常自动产生版权。

但商业保护建议保留证据：

- Git 提交记录。
- 设计稿源文件。
- 文档版本历史。
- 发布记录。
- 代码仓库时间戳。
- 需求文档和产品计划书。

### 6.2 代码版权归属

若多人参与开发，应提前约定：

- 雇员开发成果归公司。
- 外包开发成果需签署著作权转让或工作成果归属协议。
- 合作开发需明确代码、模型、数据、商标、域名归属。

建议准备：

```text
员工/外包知识产权归属协议
保密协议 NDA
贡献者许可协议 CLA
```

### 6.3 开源策略

早期建议：

```text
核心产品闭源
协议/schema/插件模板可开源
```

可开源内容：

- Search Intent JSON schema
- Everything adapter 示例
- MCP/插件开发模板
- evals 样例

不建议早期开源：

- Harness 核心实现
- 本地模型 adapter
- 商业索引器
- 排序算法和权限策略实现

### 6.4 版权登记

如在中国商业化，可考虑：

- 软件著作权登记。
- logo 美术作品登记。
- 产品文档版权存证。

这不是版权产生的必要条件，但有助于维权、投标、融资和企业客户采购。

## 7. 第三方组件与开源授权合规

### 7.1 第三方清单

项目必须维护：

```text
THIRD_PARTY_NOTICES.md
LICENSES/
```

记录：

- 组件名称
- 版本
- 来源
- license
- 是否修改
- 是否随产品分发
- 合规要求

### 7.2 Everything 合规

Everything 官方 License 是 MIT-like 宽松授权，允许使用、复制、修改、发布、分发、再授权、销售副本，但必须保留版权声明和许可声明。

建议策略：

- 产品定位为”支持 Everything（Windows 上的可选加速）”，不是 Everything 的改版。
- 默认检测用户是否已安装 Everything。
- 未安装时引导用户到 voidtools 官方下载。
- 如随产品分发 ES/SDK/Everything portable，必须包含 voidtools 和 PCRE 等许可声明。
- 不使用 `Everything AI`、`Everything Agent` 等名称。
- 不暗示 voidtools 背书。

官方资料：

- Everything License: https://www.voidtools.com/License.txt
- Everything SDK: https://www.voidtools.com/support/everything/sdk/
- Everything downloads: https://www.voidtools.com/downloads/
- ES CLI: https://github.com/voidtools/ES
- Enterprise: https://www.voidtools.com/en-us/enterprise/

### 7.3 Apple / Microsoft 品牌与系统 API 使用规范

LociFind 调用 macOS Spotlight（`mdfind` / `NSMetadataQuery`）与 Windows Search（OLE DB SystemIndex / WinRT）属于公开系统 API，不需要 Apple 或 Microsoft 的额外授权即可调用，但品牌使用和 API 调用方式必须符合各自的指引。

Apple：

- 不在 logo、官网、Hero 文案中使用 Apple logo、苹果图形或苹果产品截图（除非取得 Apple 的明确许可）。
- 不在产品名称中包含 `Apple`、`Mac`、`macOS`、`Spotlight`、`iCloud` 等 Apple 商标。
- 提到 macOS / Spotlight 仅以”运行在 macOS 上”、”支持 Spotlight”、”Works on macOS” 等中性表述。
- 不使用 “Made for Apple”、”Apple-approved”、”Designed by Apple” 等暗示官方背书的措辞。
- 遵循 Apple Marketing Guidelines 与 Apple Trademark List。
- 如使用 SF Symbols 等 Apple 提供的资源，需遵守对应许可协议。

Microsoft：

- 不在 logo、官网中使用 Microsoft、Windows、Office 的 logo（除非取得 Microsoft Trademark/Brand Tools 授权）。
- 不在产品名称中包含 `Microsoft`、`Windows`、`Office`、`Excel` 等 Microsoft 商标。
- 提到 Windows / Windows Search 仅以”运行在 Windows 上”、”支持 Windows Search”、”Works on Windows” 等中性表述。
- 不使用 “Microsoft-certified”、”Official Windows Search Agent” 等暗示官方背书的措辞。
- 遵循 Microsoft Trademark and Brand Guidelines。

官方资料：

- Apple Marketing Guidelines: https://www.apple.com/legal/intellectual-property/guidelinesfor3rdparties.html
- Apple Trademark List: https://www.apple.com/legal/intellectual-property/trademark/appletmlist.html
- Microsoft Trademark and Brand Guidelines: https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks

### 7.4 跨平台开源组件清单（待维护）

以下是首版依赖的主要开源组件，须在 `THIRD_PARTY_NOTICES.md` 中维护完整版本号、来源、license、是否修改、是否随产品分发的记录。

| 组件 | 用途 | 常见 license |
|---|---|---|
| Tauri | 跨平台桌面客户端框架 | MIT 或 Apache-2.0 |
| llama.cpp | 跨平台本地模型推理 | MIT |
| Qwen2.5-1.5B-Instruct 等基座模型 | 本地小模型 | Apache 2.0（以模型卡为准） |
| SQLite / SQLite FTS5 | 本地索引存储 | Public Domain |
| windows crate（如使用 Rust） | Windows API 绑定 | MIT 或 Apache-2.0 |
| objc / objc2 crate（如使用 Rust） | macOS API 绑定 | MIT |
| Everything SDK | Windows 可选加速 | voidtools License |
| Tesseract（如使用） | 跨平台 OCR | Apache-2.0 |
| Apple Vision framework | macOS OCR | macOS 系统 API（无单独 license，遵守 Apple 平台条款） |
| Windows.Media.Ocr | Windows OCR | Windows 系统 API（同上） |

每次添加 / 升级 / 替换组件，须同步更新此清单与 `THIRD_PARTY_NOTICES.md`。

### 7.5 模型授权

推荐模型：

```text
Qwen2.5-1.5B-Instruct
Qwen3-1.7B
```

使用前必须确认：

- 模型 license 是否允许商业使用。
- 是否允许微调。
- 是否允许合并 adapter 后分发。
- 是否允许量化后分发。
- 是否需要保留模型 license 和 attribution。
- 是否有使用限制或可接受使用政策。

模型随产品分发时，应在 About / Licenses 中列出：

- 基座模型名称和版本。
- 模型来源。
- 模型 license。
- 微调说明。
- 量化说明。

### 7.6 训练数据权利

训练数据建议使用：

- 自己生成的合成数据。
- 自己手工标注的数据。
- 用户授权的匿名样本。

避免：

- 未授权抓取商业产品查询日志。
- 包含真实用户文件名、路径、隐私内容的数据。
- 未清洗的第三方敏感语料。

训练数据应版本化：

```text
dataset_name
version
source
license
generation_method
privacy_review_status
```

## 8. 专利与商业秘密

### 8.1 专利

本项目可能涉及：

- 本地搜索 Agent Harness
- 多源本地检索结果合并
- 自然语言 Search Intent JSON 中间层
- 权限感知工具调用
- 本地隐私保护索引
- 模型 + 规则混合解析

大多数软件方法是否适合申请专利，取决于地区和具体技术创新。早期不建议把专利作为主线，但应保留发明记录。

建议：

- 对关键创新写 invention disclosure。
- 记录问题、方案、技术效果、替代方案。
- 在公开发表技术细节前，先咨询专利律师。

### 8.2 商业秘密

建议作为商业秘密保护：

- 训练数据生成策略。
- evals golden set。
- 排序算法。
- Harness 权限策略细节。
- 本地索引优化策略。
- 用户行为反馈和质量数据。

措施：

- 私有仓库。
- 权限分级。
- NDA。
- 日志脱敏。
- 内部文档访问控制。

## 9. 隐私与数据保护

虽然本文件重点是知识产权，但隐私也会影响品牌和合规。

产品应明确：

- 默认本地处理。
- 默认不上传文件名、路径、内容、索引和搜索词。
- 用户可删除本地索引。
- 用户可关闭内容索引/OCR。
- 用户可查看 Agent 行为日志。
- 多 Windows 用户账户隔离。

需要准备：

```text
Privacy Policy
Terms of Use
Data Processing Addendum
Security Whitepaper
```

MVP 阶段至少需要：

- 隐私说明。
- 第三方组件许可。
- Everything 集成说明。
- 本地索引删除说明。

## 10. 上架与发布前检查清单

### 10.1 品牌

- [ ] 确认最终品牌名。
- [ ] 提交商标申请。
- [ ] 注册核心域名。
- [ ] 统一产品文案。
- [ ] 不使用未注册地区的 `®`。

### 10.2 域名

- [ ] 注册 `locifind.ai`。
- [ ] 注册 `locifind.app`。
- [ ] 注册 `locifind.dev`。
- [ ] 开启自动续费。
- [ ] 开启 2FA。
- [ ] 开启域名锁。

### 10.3 代码与版权

- [ ] 明确仓库 license。
- [ ] 建立第三方组件清单。
- [ ] 建立贡献者协议或开发合同。
- [ ] 保留设计和代码历史。
- [ ] 准备软件著作权登记材料。

### 10.4 第三方授权

- [ ] Everything license 已纳入 Third-party Notices。
- [ ] ES/SDK 分发方式明确。
- [ ] Tauri、llama.cpp、SQLite、windows/objc 等开源依赖 license 已核对。
- [ ] 模型 license 已核对。
- [ ] OCR、PDF、Office、音频 metadata 库 license 已核对。
- [ ] Apple / Microsoft 商标使用经审查（不暗示官方背书）。
- [ ] 安装包包含必要许可文件。

### 10.5 模型与数据

- [ ] 训练数据来源记录完整。
- [ ] 数据不含真实敏感信息。
- [ ] 模型 license 允许商业分发。
- [ ] LoRA adapter 权属明确。
- [ ] 量化模型分发许可明确。
- [ ] 同一份模型在 macOS（Metal）与 Windows（CPU/CUDA/Vulkan）上的分发许可均明确。

### 10.6 平台分发与上架

macOS（Developer ID 分发）：

- [ ] 已注册 Apple Developer Program。
- [ ] Developer ID Application 证书已签名所有可执行文件与 sidecar。
- [ ] Hardened Runtime 已启用，entitlements 已最小化。
- [ ] 应用已提交 Apple Notary Service 公证并 staple。
- [ ] DMG 包含清晰的安装说明与第三方许可。

macOS（Mac App Store，可选，建议 Beta 之后）：

- [ ] App Sandbox 兼容性已评估（可能与 Developer ID 版本功能不一致）。
- [ ] Privacy Manifest（NSPrivacyAccessedAPITypes / NSPrivacyTrackingDomains 等）已填写。
- [ ] App Store Review Guidelines 中与 AI、文件访问、网络相关条款已检查。

Windows（Developer 分发）：

- [ ] OV/EV Code Signing 证书已签名安装包与可执行文件。
- [ ] MSI / MSIX 安装与卸载流程通过测试。
- [ ] SmartScreen reputation 累积策略已规划。

Windows（Microsoft Store，可选）：

- [ ] Microsoft Partner Center 账号已注册。
- [ ] MSIX 打包符合 Microsoft Store 政策。
- [ ] 应用与隐私声明已通过 Store 审核要求。

### 10.7 法务文档

- [ ] Privacy Policy（含 macOS 与 Windows 各自的数据访问范围说明）。
- [ ] Terms of Use。
- [ ] Third-party Notices。
- [ ] EULA。
- [ ] 商标使用声明（含 LociFind 自身及对 Apple / Microsoft / voidtools 商标的引用规范）。
- [ ] Spotlight / Windows Search / Everything 兼容性与免责声明。

## 11. 推荐执行时间线

### 第 0-1 周

- 注册核心域名。
- 确认申请主体。
- 找商标代理做正式检索。
- 准备商标申请商品/服务描述。
- 建立 `THIRD_PARTY_NOTICES.md`。
- 注册 Apple Developer Program（macOS 签名/公证需要，提前注册可避免发布前等待）。

### 第 1-2 周

- 提交中国和美国商标申请。
- 建立品牌使用规范（含对 Apple / Microsoft / voidtools 商标的引用规范）。
- 建立私有代码仓库。
- 明确贡献者/外包 IP 归属协议。

### 第 1 个月

- 准备隐私政策草案（含 macOS / Windows 各自数据访问范围）。
- 准备 EULA 草案。
- 建立模型和训练数据 license 台账。
- 确认 Spotlight / Windows Search / Everything 集成与分发策略。
- 申请 Windows OV/EV Code Signing 证书（签发周期较长，建议提前）。

### MVP 发布前

- 核查第三方组件（含 Tauri、llama.cpp、SQLite、平台 binding crate、Everything SDK 等）。
- 核查模型在 macOS 与 Windows 两个平台上的分发权利。
- 完成 macOS Developer ID 签名与公证流程演练。
- 完成 Windows 安装包签名演练。
- 完成隐私说明。
- 完成第三方许可说明。
- 明确商标状态：申请中 / 已注册。

### Beta 发布前

- 评估是否追加欧盟、英国、新加坡、香港商标申请。
- 考虑软件著作权登记。
- 考虑 logo 商标申请。
- 准备企业客户版法务文档。
- 如计划上架 Mac App Store / Microsoft Store，启动 App Sandbox / Privacy Manifest / MSIX 等适配。

## 12. 给商标代理的需求说明模板

可以复制以下内容发给商标代理：

```text
我们计划为一款本地个人搜索 Agent 产品申请商标，拟申请名称为 LociFind。

产品说明：
LociFind 是一款本地优先的桌面软件，帮助用户通过自然语言搜索本机文件、文档、音乐、图片、截图和其他个人数据。产品包含本地 AI 模型、文件索引、搜索结果排序、权限控制和本地隐私保护能力。

请帮忙做商标可注册性检索和风险评估，重点覆盖：
1. LociFind / LOCIFIND / Loci Find 的精确和近似检索；
2. Loci / Locus / LociFinder / LocalFind / LoqiFind / LokiFind 等近似检索；
3. 中文名“洛希搜索”的检索；
4. 类别 Class 9 和 Class 42；
5. 相关类别 Class 35、38、41；
6. 中国、美国、欧盟的注册风险；
7. 是否建议同时申请 logo 或中文名；
8. 商品/服务描述建议。

请输出：
- 可注册性判断；
- 主要冲突商标列表；
- 驳回/异议风险；
- 推荐申请类别；
- 推荐商品/服务描述；
- 是否建议更换品牌名。
```

## 13. 给律师的产品合规说明模板

```text
我们正在开发一款名为 LociFind 的跨平台（macOS + Windows）本地个人搜索 Agent。

核心功能：
- 自然语言搜索本地文件；
- 通过 SearchBackend 抽象调用系统搜索（macOS Spotlight via mdfind / NSMetadataQuery；Windows Search via OLE DB SystemIndex）以及可选的 Everything 加速后端；
- 使用本地小模型解析搜索意图；
- 建立本地 metadata、全文、OCR 和音乐标签索引；
- 默认不上传用户文件、文件名、路径、搜索词或索引数据；
- 支持用户打开、复制、移动文件，但敏感操作会进行权限确认。

请协助审查：
1. 商标和品牌使用风险；
2. Apple 商标 / Apple Marketing Guidelines 合规；
3. Microsoft 商标 / Microsoft Brand Guidelines 合规；
4. Everything / voidtools 集成和分发合规；
5. macOS 系统 API（mdfind、NSMetadataQuery、Vision framework）调用与分发合规；
6. Windows 系统 API（OLE DB SystemIndex、WinRT、Windows.Media.Ocr）调用与分发合规；
7. 开源组件 license 合规（Tauri、llama.cpp、SQLite、平台绑定 crate 等）；
8. 模型 license 和微调/量化/跨平台分发合规；
9. 隐私政策和本地索引合规（macOS Full Disk Access、Windows 索引位置等）；
10. macOS Developer ID 签名 / 公证 / Hardened Runtime / entitlements 合规；
11. Windows 代码签名与 SmartScreen 合规；
12. EULA 和免责声明；
13. Mac App Store / Microsoft Store 上架风险；
14. 企业客户使用时的数据和安全条款。
```

## 14. 结论

LociFind 是一个可继续推进的品牌名，但由于 `Loci` 在 AI、软件、SaaS 等领域已有使用，建议尽快完成正式商标检索并提交 `LociFind` 文字商标申请。

最小保护组合：

```text
商标：LociFind
地区：中国 + 美国
类别：Class 9 + Class 42
域名：locifind.ai + locifind.app + locifind.dev
文档：Privacy Policy + Terms of Use + Third-party Notices + EULA
跨平台分发账号：Apple Developer Program + Windows OV/EV 代码签名证书
品牌引用规范：不暗示 Apple / Microsoft / voidtools 官方背书
```

项目早期最重要的事情：

1. 先注册可用域名。
2. 尽快提交商标申请。
3. 统一使用完整品牌 `LociFind`。
4. 提前注册 Apple Developer Program 与采购 Windows 代码签名证书（签发周期长）。
5. 保留代码、文档、设计和训练数据的权属证据。
6. 建立第三方组件、平台 API、模型授权台账。
