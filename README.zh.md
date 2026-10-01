# 🤖 LibrAgent

> **你自己运行的智能体操作环境 — 选模型，一键装工具，选协调模式。**
> _没有厂商套壳。没有 JSON 作业。成果以文件形式留在你的机器上。_

[English](./README.md) | [한국어](./README.ko.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![Version](https://img.shields.io/github/v/release/fritzprix/libr-agent)](https://github.com/fritzprix/libr-agent/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/React-18.3-61DAFB?logo=react)](https://react.dev)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.6-3178C6?logo=typescript)](https://www.typescriptlang.org)

![Hero 演示 — 像装应用一样安装工具。模型自选。结果落成文件。](./assets/hero-demo-60s.gif)

[HD WebM](./assets/hero-demo-60s.webm) · _像装应用一样安装工具。模型自选。结果落成文件。_

---

## LibrAgent 有何不同

多数智能体套件假定你会改 MCP JSON、泡在终端里、用代码拼编排（或把你锁进单一厂商栈）。

LibrAgent 是做同一件事的 **桌面产品**：

| 不再是… | 你得到… |
| ------- | ------- |
| 手改 MCP 配置 | **Extensions** — 一键预设（GitHub、Brave Search、Filesystem、…），并从 Cursor / VS Code / Claude Code / Windsurf 导入 |
| “我们有多智能体” | 作为捆绑技能的 **具名协调模式** — `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation`、… |
| 单一提供商的模型 + 工具 | **你的** LLM（API 密钥或 [Ollama](https://ollama.com)）和 **你的** MCP 栈 — MIT，本地优先 |

[下载最新发行版](https://github.com/fritzprix/libr-agent/releases/latest) · [5 分钟上手](#5-分钟上手路径) · [Hero 演示规格](docs/contributing/hero-demo-spec.md)

---

## 前 10 分钟你能做什么

### 1. 一键工具，再要一份交付物

- 打开 **Extensions**，安装预设（例如 GitHub）— 无需 JSON
- 将 **Workspace** 指向真实文件夹
- 提问：_"审查这个仓库对新贡献者的最大风险，并保存为 `DELIVERABLE.md`"_

### 2. 部署一键工作流配方

- 从 Chat 主页或 [Scheduled Tasks](docs/user/guides/scheduled-tasks.md) 启动 **Morning Briefing** 配方
- 安装 Hacker News + Yahoo Finance 预设，配置助手，并安排每天上午 9 点运行
- 醒来即可看到合成的科技与市场简报 — 无人值守

### 3. 选择协调模式（无需拼装框架）

- 描述工作形态，或按名称附加技能：
  - _"@skill:pipeline — 先调研，再起草，再审阅；留下一份最终报告"_
  - _"@skill:divide-conquer — 拆成独立片段再合并结果"_
- 模式是产品化技能 — 不是你自己接线的 SDK。参见 [Sub-agents & orchestration](docs/user/guides/sub-agents.md)。

### 4. 保持模型自由

- 云端：粘贴 OpenAI / Anthropic / Gemini / Groq API 密钥
- 本地：`ollama pull qwen3:14b` 并选择 Ollama — 同一套 harness

---

## 三项产品承诺

1. **不写套件作业就能上手** — GUI、Extensions 一键、配方、应用内审批、`@skill:` — 不是“先打开配置再开 shell”。
2. **编排即产品** — 通过技能选择 Sequential / Hub-and-spoke / Swarm 风格流程；需要持久团队或 cron 时再进入 `teamwork` → `org` 与 `schedule` — 仍无需自行拼装 LangGraph/CrewAI。
3. **提供商与栈自由** — 任意受支持的 LLM、MCP 作为基础设施、导入现有 IDE MCP 配置、MIT 许可、默认本地工作区与浏览器状态。

**最适合：** 想要 harness 深度却不想泡在 JSON 里的运营者与高级用户；拒绝单一厂商智能体栈的开发者；需要浏览器 + 知识 + 调度集于一品的研究者。

---

## 协调模式（捆绑技能）

按 **工作形态** 选模式，再在对话中运行：

| 技能 | 模式 | 何时使用 |
| ---- | ---- | -------- |
| `pipeline` | 顺序阶段 | 输出喂给下一步（调研 → 起草 → 审阅） |
| `hub-spoke` | 中心辐射 | 一名协调者整合多名工作者 |
| `divide-conquer` | 并行拆分 | 独立片段，再合并 |
| `consensus-delegation` | 多视角 | 同一问题交给多位专家，再调和 |
| `gatekeeper` / `pair-programming` | 审阅循环 | 严格审阅或双智能体编程 |
| `delegate` | 轻量交接 | 一个子会话，谱系可追踪 |
| `teamwork` → `org` | 持久团队 | 共享章程 + Org UI |
| `schedule` / `loop` / `call-me-back` | 时间与事件 | Cron、会话内延迟，或按进程/webhook 恢复 |

选择启发式：[framework-selection](src-tauri/bundled_skills/teamwork/references/framework-selection.md) · 完整指南：[Sub-agents](docs/user/guides/sub-agents.md)。

其他第一天技能：`setup-wizard`、`tool-installer`、`playbook-creator` 等 — **[Bundled Skills](docs/user/guides/skills.md)**。

---

## MCP 平台（仍具备高级用户能力）

- 传输：stdio、HTTP、SSE、OAuth 2.1
- 15+ 内置服务器（Workspace、Shell、Browser、Knowledge、Planning、Scheduled Tasks、…）
- 一键预设 + 智能体辅助安装（`tool-installer`）
- 按会话隔离工具；路径/命令守卫；自动化可选 YOLO / unsafe 模式

### 执行基座

| 基座 | 能力 |
| ---- | ---- |
| **Workspace** | 精确到行的编辑、多文件操作、`@file` / `@skill` / `@playbook` 上下文 |
| **Shell** | 隔离与持久 shell，异步进程监控 |
| **Browser** | 隔离浏览器 sidecar；可选已保存登录配置 |
| **Knowledge** | 图知识 + BM25 搜索 |
| **Export** | Markdown 报告与 ATIF 轨迹导出（[session export](docs/user/guides/session-export.md)） |

长会话通过上下文压缩、防循环、断路器与过期响应守卫保持可用。

---

## 真实场景

### 运营者 — 从空应用到每日简报

1. 运行 **Morning Briefing** 配方（预设 + 助手 + 上午 9 点调度）
2. 点一次 **Run now** 验证
3. 放着不管 — 报告会落地，无需打开终端

### 独立开发者 — 用预设，不是配置文件

1. Extensions → 安装 GitHub MCP 预设
2. 通过 Workspace 挂载本地仓库
3. 要求一份留在磁盘上的 Markdown 安全/审阅报告

### 非框架高级用户 — 具名编排

1. 按工作形态使用 `@skill:pipeline`（或 `hub-spoke` / `divide-conquer`）
2. 智能体在该模式下协作
3. 工作区里一份合并后的交付物 — 无需维护编排库

### 注重隐私的团队 — 同一产品，本地模型

1. `ollama pull qwen3:14b`
2. Workspace + Shell 留在本机
3. 以后想换云端密钥也可以 — harness 不会替你换厂商

---

## 文档

- **[User Guide](docs/user/README.md)** — 安装、首次对话、模型、技能（[docs site](https://fritzprix.github.io/libr-agent/)）
- **[Hero Demo Spec](docs/contributing/hero-demo-spec.md)** — 规范产品演示（EN/KO/ZH 字幕）
- **[Product Messaging Guide](docs/contributing/product-messaging-guide.md)** — 定位与文案
- **[Recipes](docs/user/guides/recipes.md)** · **[Scheduled Tasks](docs/user/guides/scheduled-tasks.md)** · **[Sub-agents](docs/user/guides/sub-agents.md)** · **[Skills](docs/user/guides/skills.md)**
- **[HTTP API](docs/api/http_api.md)** — 远程控制与可编程审批
- **[Architecture](docs/architecture/agent-workflow-architecture.md)** — 会话隔离与 Think-Act-Observe

---

## 开始使用

从 **[Releases 页面](https://github.com/fritzprix/libr-agent/releases/latest)** 下载最新安装包。

<!-- RELEASE_DOWNLOADS_START -->
- **Windows:** [`LibrAgent_0.9.21_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64-setup.exe) · [`LibrAgent_0.9.21_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_x64_en-US.msi)
- **macOS (Apple Silicon):** [`LibrAgent_0.9.21_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_aarch64.dmg)
- **Linux:** [`LibrAgent_0.9.21_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.AppImage) · [`LibrAgent_0.9.21_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent_0.9.21_amd64.deb) · [`LibrAgent-0.9.21-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.9.21/LibrAgent-0.9.21-1.x86_64.rpm)
- **All release assets:** [Releases page](https://github.com/fritzprix/libr-agent/releases/tag/v0.9.21)
<!-- RELEASE_DOWNLOADS_END -->

### 5 分钟上手路径

**第 1 步 — 连接模型**（Settings → LLM Providers）

- 云端：粘贴 OpenAI / Anthropic / Gemini / Groq API 密钥
- 本地：`ollama pull qwen3:14b`，然后在 Settings 中选择 Ollama

**第 2 步 — 不加 JSON 也能加工具**

- Extensions → 安装预设（例如 GitHub），**或**
- 告诉智能体：_"从 Cursor 导入我的 MCP 服务器"_

**第 3 步 — 挂上工作区，要一份你会保留的文件**

- 将 Workspace 指向真实项目文件夹
- _"审查这个工作区，然后将发现写入 `DELIVERABLE.md`。"_

**接下来 — 协调与自动化**

- _"@skill:pipeline — 调研、起草、再审阅；一份最终报告。"_
- _"为这个仓库准备 teamwork 工作区。"_
- _"设置每天早上 7 点的竞品简报日程。"_（或运行 Morning Briefing 配方）

### 可复制粘贴的首批提示

- _"从 Cursor 导入我的 MCP 服务器，并告诉我新增了什么。"_
- _"安装 GitHub MCP 预设，并挂到编码智能体上。"_
- _"审查这个工作区，然后将发现写入 `DELIVERABLE.md`。"_
- _"@skill:pipeline — 调研这个主题，起草摘要，再审阅；保存最终报告。"_
- _"设置每天早上 7 点的竞品简报日程。"_

### 开发者设置

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## LibrAgent 最适合哪里

| 如果你想要… | LibrAgent 强在… |
| ----------- | --------------- |
| **有 harness 深度，没有 harness 作业** | Extensions 预设、配方、`@skill:` 模式与审批 — 不是 JSON 优先上手 |
| **有编排，不必自建框架** | `pipeline`、`hub-spoke`、`divide-conquer`、`consensus-delegation`、`teamwork` / `org`、`schedule` 作为产品提供 |
| **摆脱厂商智能体栈** | 自带模型与 MCP 工具；MIT；默认本地优先 |
| **真正的执行基座** | Workspace、shell、browser、knowledge、playbooks 与长会话守卫 |
| **MCP 原生桌面产品** | 预设、导入与 15+ 内置 — 不是薄聊天壳 |

---

## 设计理念

- **产品优于工具包**：无需拼装即可使用 harness。
- **编排即技能**：协调模式有名称、可选、有文档 — 不埋在示例仓库里。
- **栈的自由**：模型与工具由用户选择；产品不绑定单一 AI 厂商。
- **本地优先**：工作区、会话、技能与浏览器状态由你掌控。仅在你选择时使用云端 LLM / 远程 MCP。
- **Harness 重于模型**：工具、会话状态、委派与治理比任何单一模型更重要。
- **稳定重于功能**：隔离、压缩、防循环 — 先于功能追逐。
- **开放标准**：MIT。MCP 作为互操作层。

---

## 贡献与许可

LibrAgent 采用 MIT 许可，公开构建。欢迎贡献 — 捆绑技能、MCP 集成、缺陷修复或架构改进。

- 📖 [Contributing Guide](CONTRIBUTING.md)
- 🐛 [Issue Tracker](https://github.com/fritzprix/libr-agent/issues) [![Good First Issues](https://img.shields.io/github/issues/fritzprix/libr-agent/good%20first%20issue)](https://github.com/fritzprix/libr-agent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good%20first%20issue%22)
- 💬 [Discussions](https://github.com/fritzprix/libr-agent/discussions)
- 🧪 基准测试（Harbor / Terminal-Bench）：见 [Harbor guide](benchmarks/harbor/README.md)（`pnpm bench:diverse`、`pnpm bench:terminal`、…）

**License**: MIT
