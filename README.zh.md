<p align="center">
  <img src="./src-tauri/icons/128x128.png" width="96" alt="LibrAgent" />
</p>

# LibrAgent

> **用户自主掌控的本地 AI 智能体桌面应用。**
> 自由连接任意大语言模型，一键安装扩展工具，运用成熟的多智能体协同模式实现自动化。
> _无需手动编写繁琐的 JSON 配置，无厂商锁定，所有工作成果直接保存在本地计算机文件中。_

[English](./README.md) | [한국어](./README.ko.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Hero 演示 — 像安装应用一样配置工具，自主选择模型，结果沉淀为本地文件。](./assets/hero-demo-60s.gif)

[查看高清 WebM 演示](./assets/hero-demo-60s.webm) · _像安装应用一样配置工具，自主选择模型，结果沉淀为本地文件。_

---

## LibrAgent 的核心优势

多数智能体开发框架通常要求开发者精通复杂的终端命令行操作、手动编写繁琐的 MCP JSON 配置文件，并需自行编写代码实现多智能体编排。此外，许多产品还将用户深度绑定在特定的云厂商生态中。

LibrAgent 为解决上述问题提供了 **开箱即用的完整桌面端产品**：

| 传统方式 | LibrAgent 解决方案 |
| -------- | ------------------ |
| 手动编写复杂的 MCP JSON 配置文件 | **一键式扩展工具** — 预置 GitHub、Brave Search、本地文件系统等常用工具，支持一键导入 Cursor、VS Code、Claude Code 配置 |
| 必须用代码自行组装多智能体系统 | **内置成熟协同模式** — 开箱即用的 `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation` 等原生技能 |
| 被特定云厂商的模型和工具绑定 | **自由的技术栈选择** — 支持接入主流云端 API 或本地部署的 [Ollama](https://ollama.com)，基于 MIT 协议保障本地优先与隐私安全 |

[下载最新发行版](https://github.com/fritzprix/libr-agent/releases/latest) · [5 分钟上手路径](#5-分钟上手路径) · [演示规格文档](docs/contributing/hero-demo-spec.md)

---

## 10 分钟快速体验

### 1. 一键安装工具并生成任务成果

- 在 **Extensions** 页面一键安装工具预设（例如 GitHub），无需编辑任何 JSON 文件。
- 在 **Workspace** 中选定本地项目文件夹。
- 输入指令：_"分析此代码仓库，找出新贡献者需要注意的最大潜在风险，并保存为 `DELIVERABLE.md`"_

### 2. 执行自动化工作流配方

- 在聊天主页或 [定时任务指南](docs/user/guides/scheduled-tasks.md) 中启动 **Morning Briefing** 配方。
- 自动完成 Hacker News 与 Yahoo Finance 工具配置，并排期于每天上午 09:00 自动执行。
- 每日定时自动生成最新科技与市场动态综合简报。

### 3. 无需编码即可运用协同模式

- 无需编写编排代码，直接在对话中指定多智能体协作方式：
  - _"@skill:pipeline — 按照'调研、草拟、复审'步骤推进，最后输出一份完整报告"_
  - _"@skill:divide-conquer — 将此任务拆分为独立的子任务并行处理，最后合并结果"_
- 经过实践验证的协同模式已封装为技能。详情参见 [子智能体与编排指南](docs/user/guides/sub-agents.md)。

### 4. 自由切换与掌控模型

- **云端模型**: 填入 OpenAI、Anthropic、Gemini、Groq 等主流 API 密钥即可立即使用。
- **本地模型**: 运行 `ollama pull qwen3:14b`，在设置中选择 Ollama，即可实现完全无外网依赖的本地离线运行。

---

## 产品三大核心承诺

1. **零配置门槛的现代化 UI** — 提供可视化图形界面、一键工具安装、预设工作流配方和应用内操作审批，彻底告别命令行与配置文件编辑。
2. **产品化的智能体编排系统** — 通过技能自由选用顺序执行（Sequential）、中心辐射（Hub-and-spoke）、分布式（Swarm）工作流，并可随时平滑扩展至长期团队运作（`teamwork`）与定时调度（`schedule`）。
3. **模型与数据的完全掌控权** — 基于开放的标准协议 MCP（Model Context Protocol）构建，所有工作区代码、会话历史与浏览器状态默认均妥善保留在用户本地机器中。

**适用人群:** 追求强大智能体能力但不想编写配置文件的业务人员；拒绝单一厂商技术锁定的专业开发者；需要浏览器自动化、知识库与定时调度一体化产品的技术研究人员。

---

## 内置协同模式（捆绑技能）

根据实际工作任务的结构，选择合适的模式并在对话框中直接调用：

| 技能名称 | 协同模式 | 适用场景 |
| -------- | -------- | -------- |
| `pipeline` | 顺序流转流水线 | 前一阶段的产出作为下一阶段的输入（调研 → 起草 → 复核） |
| `hub-spoke` | 核心辐射协调 | 由一个主协调智能体统筹调度多个专业工作智能体 |
| `divide-conquer` | 任务并行分治 | 将任务拆解为相互独立的子模块并行执行，最后汇总结算 |
| `consensus-delegation` | 多角度综合研判 | 就同一议题向多个专业智能体征询意见并调和差异 |
| `gatekeeper` / `pair-programming` | 审查与配对回路 | 实施严格的代码审查机制，或开展两智能体结对协作 |
| `delegate` | 轻量单项委托 | 分派单次子会话处理具体任务并追踪完整执行链条 |
| `teamwork` → `org` | 长期持久化团队 | 建立具有共同守则与组织架构 UI 的专业协作团队 |
| `schedule` / `loop` / `call-me-back` | 时间与事件驱动 | 实现 Cron 周期定时、会话内延时触发或基于外部事件唤醒 |

模式选型参考: [框架选型指南](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · 详细指引: [子智能体与编排](docs/user/guides/sub-agents.md)

其他常用内置技能: `setup-wizard`、`tool-installer`、`playbook-creator` 等 — 详见 **[内置技能列表](docs/user/guides/skills.md)**

---

## 强大的 MCP 运行基础设施

- **全协议通信支持**: 支持 stdio、HTTP、SSE 与 OAuth 2.1
- **15+ 内置工具服务端**: 预装 Workspace、Shell、Browser、Knowledge、Planning、Scheduled Tasks 等
- **一键预设与智能引导安装**: 借助 `tool-installer` 体验交互式智能工具配置
- **完备的隔离保护**: 具备会话级工具隔离、敏感文件路径拦截、危险命令拦截与无人值守自动化安全模式

### 核心运行能力

| 核心领域 | 功能特性 |
| -------- | -------- |
| **Workspace** | 行级精准代码编辑、多文件批处理操作、支持注入 `@file` / `@skill` / `@playbook` 上下文 |
| **Shell** | 隔离持久化的终端环境与异步后台进程监控 |
| **Browser** | 独立的浏览器沙箱环境，支持通过专用扩展桥接日常 Chrome 会话 |
| **Knowledge** | 知识图谱结构存储与高性能 BM25 混合检索 |
| **Export** | 结构化 Markdown 报告导出与标准化 ATIF 执行轨迹输出（[会话导出指南](docs/user/guides/session-export.md)） |

通过内置的上下文智能压缩、死循环熔断器、故障阻断保护与响应时效校验，确保长周期会话持续高效稳定运行。

---

## 真实应用场景

### 业务运营人员 — 自动化早报生成

1. 运行 **Morning Briefing** 配方（自动安装工具预设、配置助手、排期每日 09:00 执行）。
2. 点击 **Run now** 验证运行逻辑。
3. 无需打开任何命令行终端，每日定时获取整理完毕的市场动态与科技资讯报告。

### 独立开发者 — 免配置即刻集成工具

1. 在 Extensions 页面一键安装 GitHub MCP 预设。
2. 在 Workspace 中关联本地代码仓库。
3. 指示智能体自动分析代码结构，并生成保存于本地的 Markdown 安全审查与重构报告。

### 高级业务用户 — 基于成熟模板委派工作

1. 在输入栏键入 `@skill:pipeline`（或 `hub-spoke`、`divide-conquer`）声明任务拓扑结构。
2. 智能体集群严格按照既定模式协同推进任务。
3. 直接在工作区获取最终合并的单一交付成果，免去手动维护编排框架的成本。

### 隐私敏感团队 — 纯本地离线模型运行

1. 执行 `ollama pull qwen3:14b` 本地拉取模型。
2. Workspace 与 Shell 的所有数据交互完全限制在本地计算环境中。
3. 未来若需切换至商用云端模型，只需输入 API Key，操作流程保持完全一致。

---

## 官方文档指南

- **[用户使用指南 (User Guide)](docs/user/README.md)** — 安装流程、初次使用、模型配置与技能玩法（[访问在线文档站点](https://fritzprix.github.io/libr-agent/)）
- **[演示视频规格 (Hero Demo Spec)](docs/contributing/hero-demo-spec.md)** — 官方产品演示视频规范（支持中/英/韩三语字幕）
- **[产品定位与文案指南](docs/contributing/product-messaging-guide.md)** — 产品定位、设计原则与文档编写标准
- **[配方指南](docs/user/guides/recipes.md)** · **[定时任务](docs/user/guides/scheduled-tasks.md)** · **[子智能体](docs/user/guides/sub-agents.md)** · **[技能指引](docs/user/guides/skills.md)**
- **[HTTP API 规范](docs/api/http_api.md)** — 远程调用与程序化操作审批接口
- **[系统架构解析](docs/architecture/agent-workflow-architecture.md)** — 会话隔离与 Think-Act-Observe 状态循环机制

---

## 开始使用

从 **[Releases 页面](https://github.com/fritzprix/libr-agent/releases/latest)** 下载最新安装包。

<!-- RELEASE_DOWNLOADS_START -->
- **Windows：** [`LibrAgent_0.9.24_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent_0.9.24_x64-setup.exe) · [`LibrAgent_0.9.24_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent_0.9.24_x64_en-US.msi)
- **macOS（Apple Silicon）：** [`LibrAgent_0.9.24_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent_0.9.24_aarch64.dmg)
- **Linux：** [`LibrAgent_0.9.24_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent_0.9.24_amd64.AppImage) · [`LibrAgent_0.9.24_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent_0.9.24_amd64.deb) · [`LibrAgent-0.9.24-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.24/LibrAgent-0.9.24-1.x86_64.rpm)
- **完整发布资源：** [发布页面](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.24)
<!-- RELEASE_DOWNLOADS_END -->

### 5 分钟上手路径

**步骤 1 — 接入大模型**（设置 → LLM Providers）

- 云端服务: 粘贴 OpenAI、Anthropic、Gemini、Groq 等 API 密钥。
- 本地部署: 执行 `ollama pull qwen3:14b`，在设置中选定 Ollama。

**步骤 2 — 添加工具（无需编写 JSON）**

- 在 Extensions 界面一键安装预设（例如 GitHub），或
- 直接向智能体发出指令：_"导入我在 Cursor 中配置的 MCP 服务器"_

**步骤 3 — 绑定工作区并生成交付文件**

- 在 Workspace 中关联本地目标项目目录。
- 发出指令：_"分析当前工作区代码，并将发现输出至 `DELIVERABLE.md`"_

**后续操作 — 进阶协同与自动化**

- _"@skill:pipeline — 调研该主题，整理摘要初稿并进行审核，输出最终归档报告"_
- _"为当前代码仓库配置专属的 teamwork 工作区"_
- _"建立每日早间 7 点自动运行的竞品监控任务"_（或直接运行 Morning Briefing 配方）

### 常用命令提示词范例

- _"导入我在 Cursor 中使用的 MCP 服务器配置，并告知我新增了哪些工具。"_
- _"安装 GitHub MCP 预设，并将其挂载至代码审查智能体。"_
- _"审查当前工作区中的代码实现，将改进建议保存至 `DELIVERABLE.md`。"_
- _"@skill:pipeline — 深入调研此技术方案，撰写分析初稿并完成交叉复审，最后形成归档报告。"_
- _"配置一个定时任务，使其在每天清晨 7 点自动生成行业动态简报。"_

### 开发者环境搭建

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 设计哲学

- **交付成品而非半成品套件**: 消除繁琐的组装环节，软件安装即可直接服务于生产业务。
- **以技能具象化编排模式**: 协同模式不再隐藏于复杂的示例代码中，而是转化为具有明确命名与规范的技能资产。
- **技术栈的自主掌控权**: 模型与工具完全由用户决定，不强制依附于任何特定的商业 AI 服务商。
- **本地优先原则 (Local First)**: 工作空间数据、会话记录、浏览器缓存均严格存储于用户本地环境。
- **优于单模型的稳健运行环境**: 完善的工具生态、可靠的会话状态管理和操作权限管控，比依赖单一模型更能产出稳定成果。
- **系统稳健性高于功能堆砌**: 上下文隔离、内存压缩和循环保护始终优先于盲目的功能扩张。
- **坚定拥抱开放标准**: 基于宽松的 MIT 许可，将 MCP（Model Context Protocol）作为核心互操作基石。

---

## 贡献与开源许可

LibrAgent 遵循 MIT 开源许可协议并在开源社区中协同演进。非常欢迎贡献内置技能、扩展 MCP 工具集成、提交缺陷修复或参与架构优化。

- [贡献指南 (Contributing Guide)](CONTRIBUTING.md)
- [问题跟踪器 (Issue Tracker)](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- [社区讨论区 (Discussions)](https://github.com/fritzprix/libr-agent/discussions)
- 基准评测 (Harbor / Terminal-Bench): 参见 [Harbor 评测指南](benchmarks/harbor/README.md)（`pnpm bench:diverse`、`pnpm bench:terminal` 等）

**License**: MIT
