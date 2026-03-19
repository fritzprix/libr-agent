# 🤖 LibrAgent

> **一个轻量级、有状态的自主 AI 代理平台。**

[English](./README.md) | [한국어](./README.ko.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent 是一个优先考虑本地运行（local-first）的代理运行环境，旨在跨交互保持上下文。与无状态客户端不同，它在多轮对话之间保持浏览器标签页和终端会话处于活动状态，使代理能够在持久的工作空间中更流畅地工作。

它支持 **MCP (Model Context Protocol)** 和 **Skills** 等开放标准，保持模块化和可扩展性。

---

## 为什么创建 LibrAgent？

该项目的目标是让自主代理变得触手可及。许多现有的工具仍然局限于终端命令行和复杂的 JSON 配置，这为许多潜在用户创造了难以逾越的技术鸿沟。LibrAgent 旨在通过提供一个本地优先的环境，让任何人都能在不需要成为开发人员的情况下部署和管理代理，从而弥合这一差距。

---

## 🎬 演示

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_在单个有状态的工作流中实现浏览器自动化和 shell 执行。_

---

## 核心功能

### 1. 持久工作区 (Persistent Workspace)

代理在长期存在的环境中运行，而不是在每一轮对话中都启动全新的进程。

- **实时 Web 视图**: 使用 Tauri webviews 实现实时浏览器自动化。会话和 Cookie 在各轮对话之间持久存在。
- **统一终端**: 持久的、沙盒化的 shell（支持 Python/Node.js），与工作区共享状态。

### 2. 多代理编排

LibrAgent 允许代理将任务委托给专门的子代理。

- **助手 (Assistants)**: 管理具有独特系统提示和工具配置的代理配置文件。
- **群智 (Swarm Intelligence)**: 父代理可以生成、发送消息并等待子代理的结果，以解决复杂任务。

### 3. 可扩展性

该平台设计为通过社区标准进行扩展。

- **扩展 (MCP)**: 完全支持模型上下文协议（Model Context Protocol）。立即连接到任何 MCP 服务器。
- **一键预设**: 直接在 UI 中提供 GitHub、Brave Search 等精心挑选的目录。
- **技能与剧本 (Skills & Playbooks)**: 可重用的行为片段和结构化的工作流模板。

### 4. 自主与调度

- **YOLO 模式**: 可选的自主执行敏感工具，无需人工审批。
- **计划任务**: 基于 Cron 的自动化，在重启后可自动恢复，并支持特定工作区。

### 5. 上下文与指标

- **@提及 (@mentions)**: 直接在聊天中注入文件、技能或剧本。
- **多模态**: 处理 OpenAI、Anthropic 和 Gemini 模型的图像和音频。
- **可观察性**: 实时 TPS 指标和提示词缓存命中率（适用于 Anthropic/Gemini）。

---

## 📦 安装

从 [发布页面](https://github.com/fritzprix/libr-agent/releases/latest) 下载适用于 Windows、macOS 或 Linux 的最新二进制文件。

**从源码构建:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## 设计选择

- **本地优先**: 您的数据和 API 密钥保留在您的机器上。
- **Tauri + Rust**: 选择是为了安全性（内存安全）、性能和较小的二进制体积。
- **SQLite (SeaORM)**: 用于会话和配置的高可靠本地持久化。

---

## 贡献与许可

欢迎贡献。请参阅 `CONTRIBUTING.md`。

**许可**: MIT
