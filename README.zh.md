# 🤖 LibrAgent

> **一款本地优先的 AI 代理桌面应用：能调用真实工具、并行工作，而且控制权在你手里。**
> _连接任意 LLM，接入任意 MCP 服务器，让代理读取文件、运行 Shell、浏览网页，并把自动化真正做完。_

[English](./README.md) | [한국어](./README.ko.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent 是一个基于 Tauri + Rust + React 构建的**本地优先代理工作空间**。它不是另一个聊天壳子，而是为真实文件访问、Shell 执行、浏览器自动化、MCP 扩展能力，以及能连续运行数小时而不是演示一轮就散架的多代理工作流而设计。

你可以连接云模型，也可以连接像 Ollama 这样的本地运行时；导入你已经在用的 MCP 服务器；然后把代码检查、文件编辑、命令执行、网页浏览、知识沉淀和子任务委派交给代理，而不必把整套工作流交给别人的云端 VM。

**从这里开始：**[下载最新版本](https://github.com/fritzprix/libr-agent/releases/latest) · [跳转到 5 分钟入门](#5-分钟入门路径) · [查看真实场景](#-真实使用场景)

---

## 为什么选择 LibrAgent？

大多数代理产品还在强迫你接受这种糟糕的取舍：

- **界面简单，但执行力很弱**
- **自动化很强，但产品体验粗糙**
- **云端很方便，但隐私控制很差**
- **框架很灵活，但整套系统都得你自己拼**

LibrAgent 想做的是人们真正想要的那个中间点：

- **本地优先控制**：文件、工作空间、会话和浏览器状态默认都留在本机
- **通过 MCP 获得开放扩展性**，而不是封闭插件故事
- **真正能干活的执行层**：Shell、浏览器、工作空间、知识工具都能联动
- **正常人能用的 GUI**，又不牺牲高级能力
- **从一个代理自然扩展到多个代理团队**

### LibrAgent 适合谁

- **独立开发者**：需要能够实际读取、编辑、运行、浏览并在本地保持上下文的代理
- **高级用户和运营者**：希望从本地模型、API 提供商、MCP 服务器和计划工作流中构建自己堆栈的人
- **研究者和分析师**：需要浏览器自动化、知识捕获、可重复的剧本和长时间运行的会话
- **注重隐私的团队**：需要本地执行、明确治理和从单个代理到协调组织的路径

---

## 🎬 平台演示

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_从单个代理到协调群集——递归委派、MCP 工具和持久工作空间在一个统一的底层架构中。_

---

## 前 10 分钟你就能做的事

### 1. 用真实工具审查仓库

- 用 Workspace 工具连接本地仓库
- 添加 GitHub MCP 预设
- 直接问：_“检查 PR #42 的安全问题，并把报告保存下来”_

### 2. 搭一套完全本地的代理工作栈

- 运行 `ollama pull qwen3:14b`
- 连接 Workspace + Shell
- 让代理在不把代码发到云端 VM 的前提下读取、修改、测试并持续迭代

### 3. 把研究工作变成可重复流程

- 添加 Browser + Knowledge
- 直接问：_“跟踪这 5 个竞品博客，并每天早上给我总结”_
- 把一次性任务变成定时管线

### 4. 从一个助手扩展成一个真正的团队

- 用 `specialist-creator` 创建专职代理
- 用 `delegate` 拆分工作
- 用 `teamwork` 或 `org` 把重复协作沉淀成共享工作空间

---

## 为什么它不是只会演示的玩具

### 1. 🔐 本地优先安全——数据留在你的机器上

LibrAgent 将安全作为首要架构关注点：

- **会话隔离**：每个代理会话都有自己的专用 `MCPServiceProxy` 实例——零跨会话数据泄漏
- **内置 SecurityValidator**：在系统级别阻止路径遍历攻击和命令注入
- **无需云底层架构**：核心执行都在本地完成，外部连接也主要限于你选择使用的云端 LLM 提供商和远程 MCP/HTTP 服务，此外生产构建还可能检查新版本更新
- **完全离线支持**：与 [Ollama](https://ollama.ai) 配对实现完全气隙隔离的代理堆栈

#### 保留在本地 vs 离开你的机器

- **始终本地**：工作空间、本地文件、捆绑技能、会话状态、MCP 服务器配置、浏览器状态和本地工具执行
- **在需要时才离开**：对你明确配置的云端 LLM 提供商或远程 MCP/HTTP 服务的请求，以及生产环境中检查新版本的更新请求
- **完全离线模式**：使用 Ollama 或其他本地运行时加上本地 MCP 服务器实现气隙隔离工作流

### 2. 🧩 MCP 原生生态系统——设计即无限可扩展

MCP（模型上下文协议）是 LibrAgent 可扩展性模型背后的开放标准。LibrAgent 将其视为架构骨干而非功能：

- **完整传输支持**：stdio、HTTP、SSE 和 OAuth 2.1——完整规范
- **12+ 内置服务器**：Planning、Knowledge(RAG)、Browser Automation、Workspace、Shell Execution、Content Store 等
- **预设目录**：一键安装 GitHub、Brave Search、Filesystem 等流行服务器
- **会话隔离实例**：每个代理会话拥有独立的 MCP 服务器状态——并行代理间无干扰
- **从任何地方导入**：自动从 Cursor、VS Code、Claude Code 或 Windsurf 迁移 MCP 配置

### 3. 🦾 生产级执行底层架构

大多数 AI 工具在演示中令人印象深刻，在生产中却很脆弱。LibrAgent 为长时间运行的实际工作而精心设计：

| 底层架构      | 功能                                                                        |
| ------------- | --------------------------------------------------------------------------- |
| **Workspace** | 行级精确编辑、多文件操作、统一搜索、`@file`/`@skill`/`@playbook` 上下文注入 |
| **Shell**     | 隔离执行 AND 持久 Shell——异步进程监控(`poll`、`read output`、`list`)        |
| **Browser**   | 采用类 Playwright 交互模型的无头浏览器自动化，并提供缓存一致性保证          |
| **Knowledge** | 带实体/关系提取(v2)、BM25 全文搜索的基于图的知识管理                        |

**包含可靠性工程**：上下文压缩、循环预防、断路器、陈旧响应保护器在持续数小时的会话中保持代理的生产力。

### 4. 🤝 群集→团队→组织：各规模的多代理

LibrAgent 从 solo 执行到显式组织协调拥有连贯的多代理故事：

- **`delegate`**：父代理使用显式谱系跟踪生成、简报和监控子会话
- **`teamwork`**：一条命令构建完整任务组工作空间(agents.md、MISSION.md、KANBAN.md)
- **`org`**：通过持久组织身份、根会话恢复和 org-visible 成员层次结构正式化团队
- **`schedule`**：CRON 基础自动化——代理无人值守、按计划、带工作空间宪法执行
- **Concurrency Gate**：对并行会话和 Shell 进程设置硬性限制，防止死锁和成本失控

### 5. ⚡ 捆绑技能——从空白安装到工作群集的最快路径

LibrAgent 附带不断增长数量的**捆绑技能**库。它们不是随机拼接的提示——而是任何代理都可以按名称调用的可重用操作程序。

最重要的 day-one 技能：

| 技能                 | 功能                                                  |
| -------------------- | ----------------------------------------------------- |
| `system-setup`       | 检测并安装所有平台上缺少的运行时(Python、Node.js、uv) |
| `mcp-installer`      | 从 npm 包、GitHub URL 或 JSON 配置块注册 MCP 服务器   |
| `mcp-importer`       | 从 Cursor、VS Code、Windsurf 等导入现有 MCP 配置      |
| `specialist-creator` | 从角色描述设计完整的代理配置(系统提示、模型、工具)    |
| `crew-constructor`   | 扫描可用工具并自动批量创建匹配的专家团队              |
| `agent-tooling`      | 审计代理、检测能力不匹配、动态重新平衡工具分配        |
| `delegate`           | 引导父→子会话移交，带显式上下文传递和谱系跟踪         |
| `teamwork`           | 为协调多代理工作构建共享工作空间宪法                  |
| `org`                | 正式化持久组织身份和 org-visible 成员层次结构         |
| `schedule`           | 创建和管理无人值守自动化的定期计划任务组              |
| `soul-awakening`     | 将代理锚定到 `SOUL.md` 人格——语气、立场、身份         |

这只是运营层。LibrAgent 还提供领域技能：

- **知识和研究**：`deep-research-report`、`knowledge-distiller`
- **文档工作流**：`document-to-markdown`、`docx`、`pptx`
- **技能和流程作者**：`skill-creator`、`skill-deployer`、`playbook-creator`、`mcp-builder`
- **特殊操作**：`computer-diagnosis` 和其他专注辅助工具

_重要：`bootstrap` 是经常与这些技能一起使用的内置功能。捆绑技能是可重用的程序；内置功能和 MCP 工具是其下的执行底层架构。_

---

## 🌍 现实世界场景

### 独立开发者——自动化代码审查

1. 通过 Workspace 工具连接你的本地仓库
2. 安装 GitHub MCP 预设（一键）
3. 请求：_"查找 PR #42 中的安全问题并生成 Markdown 报告"_
4. 代理读取代码、运行分析、将发现保存到 Knowledge 服务器以供将来参考

### 市场营销——竞争对手情报自动驾驶

1. 通过 Browser 工具配置 5 个竞争对手博客
2. 告诉代理：_"每天早上 7 点创建竞争对手简报"_——代理可以使用 `schedule` 技能为你连接定期任务组
3. 代理浏览、摘要并追加到 Knowledge 存储
4. 随时询问：_"总结上周竞争对手的动向"_

### 工程团队——离线代理堆栈

1. `ollama pull qwen3:14b`——无需 API 密钥，无需云端
2. 将 Workspace + Shell 工具连接到你的代码库
3. 敏感 IP 永远不会离开机器
4. 代理读取、修改、测试和提交——完全本地

### 高级用户——多代理研究管道

1. 使用 `crew-constructor` 自动生成：Researcher × 3、Analyst × 1、Writer × 1
2. 协调者通过 `delegate` 技能并行委派
3. 结果合并到 Content Store 中的单个结构化报告中
4. 通过 `schedule` 每周计划整个工作流程

---

## 📖 文档和指南

- **[导航指南](docs/guides/navigation-guide.md)**：Command & Control 中心——`/assistants`(角色定义) 和 `/playbooks`(工作流程蓝图)。
- **[架构指南](docs/architecture/agent-workflow-architecture.md)**：会话隔离、编排引擎和 Rust 驱动的 Think-Act-Observe 循环。
- **[内置工具指南](docs/guides/builtin_tool_bp.md)**：工具设计标准和 MCP 响应模式。

---

## 📦 开始使用

从[发布页面](https://github.com/fritzprix/libr-agent/releases/latest)下载你平台的最新安装程序。

<!-- RELEASE_DOWNLOADS_START -->

- **Windows：** [`LibrAgent_0.7.26_x64-setup.exe`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent_0.7.26_x64-setup.exe) · [`LibrAgent_0.7.26_x64_en-US.msi`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent_0.7.26_x64_en-US.msi)
- **macOS（Apple Silicon）：** [`LibrAgent_0.7.26_aarch64.dmg`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent_0.7.26_aarch64.dmg)
- **Linux：** [`LibrAgent_0.7.26_amd64.AppImage`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent_0.7.26_amd64.AppImage) · [`LibrAgent_0.7.26_amd64.deb`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent_0.7.26_amd64.deb) · [`LibrAgent-0.7.26-1.x86_64.rpm`](https://github.com/fritzprix/libr-agent/releases/download/v0.7.26/LibrAgent-0.7.26-1.x86_64.rpm)
- **完整发布资源：** [发布页面](https://github.com/fritzprix/libr-agent/releases/tag/v0.7.26)
<!-- RELEASE_DOWNLOADS_END -->

**开发者设置：**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

### 5 分钟入门路径

**第 1 步——连接模型**（Settings → LLM Providers）

- 云端：粘贴 OpenAI / Anthropic / Gemini / Groq API 密钥
- 本地：`ollama pull qwen3:14b` 然后在 Settings 中选择 Ollama
- 正在使用 Cursor 或 VS Code？告诉任何代理：*"从 Cursor 导入我的 MCP 服务器"*→ `mcp-importer` 处理

**第 2 步——添加 MCP 工具**（Extensions 侧边栏）

- 浏览预设目录并点击 Install，或
- 告诉代理：*"Install @modelcontextprotocol/server-everything"*→ `mcp-installer` 自动注册

**第 3 步——创建你的第一个代理**

- *"为竞争情报创建研究者代理"*→ `specialist-creator` 设计完整配置
- *"用我的当前工具构建研究团队"*→ `crew-constructor` 批量创建匹配专家
- *"优化所有代理间的工具分配"*→ `agent-tooling` 自动审计和重新平衡

**第 4 步——使用 `delegate` 并行工作**

- 请求任何代理将子任务委派给子会话
- `delegate` 技能管理上下文移交、谱系跟踪和结果合并

**第 5 步——构建持久团队**

- `teamwork`→ 使用 `agents.md`、`MISSION.md`、`KANBAN.md` 构建共享工作空间
- `org`→ 通过持久身份和 org-root 会话管理正式化团队
- `schedule`→ 让代理为你创建和管理 CRON 基础自动化

### 可复制粘贴的首批提示

- _"从 Cursor 导入我的 MCP 服务器并显示添加了什么。"_
- _"用我的当前工具为竞争情报创建研究者代理。"_
- _"安装 GitHub MCP 预设并将其附加到编码代理。"_
- _"将仓库分析委派给子会话并带回摘要。"_
- _"为此仓库准备 teamwork 工作空间，然后创建 org-ready 专家团队。"_
- _"设置每天早上 7 点的计划每日竞争对手简报并保持在共享 teamwork 工作空间中。"_

---

## LibrAgent 最适合什么场景

| 如果你想要……                   | LibrAgent 的优势在于……                                                |
| ------------------------------ | --------------------------------------------------------------------- |
| **一台本地 AI 工作站**         | 文件、会话、工作空间和浏览器状态默认都保留在你的机器上                |
| **真正 MCP 原生的桌面产品**    | 你可以直接在产品里安装、导入和管理 MCP 服务器，而不是把它当成一个薄壳 |
| **能真正干活的代理**           | Workspace、Shell、Browser 和 Knowledge 工具都是为长时间执行设计的     |
| **不用先造框架的多代理工作流** | `delegate`、`teamwork`、`org` 和 `schedule` 已经是产品内建能力        |
| **兼顾 GUI 易用性和高级深度**  | 你得到桌面 UI，同时不丢失扩展性和控制力                               |

---

## 设计理念

- **本地优先**：你的数据、密钥和代理"souls"完全由你控制。无需云底层架构。
- **编排系统优于模型**：执行环境——工具、会话状态、委派、治理——比任何单个模型都重要。LibrAgent 旨在最大化任何模型的能力。
- **稳定性优于功能**：CHANGELOG 反映了对运行时正确性的痴迷关注——会话隔离、压缩、循环预防、陈旧响应保护器——而不仅仅是新功能。
- **MCP 作为基础设施**：不是插件系统。整个工具生态系统围绕 MCP 作为主要互操作层组织。
- **开放标准**：MIT 许可。完全致力于 MCP、开源互操作性和用户数据主权。

---

## 贡献和许可

LibrAgent 以 MIT 许可开源构建。欢迎贡献——无论是新的捆绑技能、MCP 集成、错误修复还是架构改进。

- 📖 [贡献指南](CONTRIBUTING.md)
- 🐛 [问题追踪器](https://github.com/fritzprix/libr-agent/issues)
- 💬 [讨论](https://github.com/fritzprix/libr-agent/discussions)

**许可**: MIT
