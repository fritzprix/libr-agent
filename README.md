# 🤖 LibrAgent

> **A lightweight, stateful platform for autonomous AI agents.**

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent is a local-first agent runner designed to maintain context across interactions. Unlike stateless clients, it keeps browser tabs and terminal sessions alive between turns, allowing agents to work more fluidly within a persistent workspace.

It implements open standards like **MCP (Model Context Protocol)** and **Skills** to remain modular and extensible.

---

## Why LibrAgent?

The goal of this project is to make autonomous agents accessible. Many existing tools remain trapped behind terminal commands and manual JSON configurations, creating a gap that excludes many potential users. LibrAgent aims to bridge this gap by providing a local-first environment where anyone can deploy and manage agents without needing to be a developer.

---

## 🎬 Demo

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

*Browser automation and shell execution in a single, stateful workflow.*

---

## Core Features

### 1. Persistent Workspace
Agents operate within a long-lived environment rather than spawning fresh processes for every turn.
- **Live Webview**: Real-time browser automation using Tauri webviews. Sessions and cookies persist across turns.
- **Unified Terminal**: A persistent, sandboxed shell (Python/Node.js supported) that shares state with the workspace.

### 2. Multi-Agent Orchestration
LibrAgent allows agents to delegate tasks to specialized sub-agents.
- **Assistants**: Manage agent profiles with unique system prompts and tool configurations.
- **Swarm Intelligence**: Parent agents can spawn, message, and await results from sub-agents to solve complex tasks.

### 3. Extensibility
The platform is designed to be expanded via community standards.
- **Extensions (MCP)**: Full support for the Model Context Protocol. Connect to any MCP server instantly.
- **One-Click Presets**: Curated catalog for GitHub, Brave Search, etc., available directly in the UI.
- **Skills & Playbooks**: Reusable behavior snippets and structured workflow templates.

### 4. Autonomy & Scheduling
- **YOLO Mode**: Optional autonomous execution for sensitive tools without manual approval.
- **Scheduled Tasks**: Cron-based automation with workspace-specific targeting and automatic recovery.

### 5. Context & Metrics
- **@mentions**: Direct injection of files, skills, or playbooks into chat.
- **Multimodal**: Handles images and audio for OpenAI, Anthropic, and Gemini models.
- **Observability**: Real-time TPS metrics and prompt caching hits (for Anthropic/Gemini).

---

## 📦 Installation

Download the latest binaries for Windows, macOS, or Linux from the [Release page](https://github.com/fritzprix/libr-agent/releases/latest).

**Build from source:**
```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Design Choices

- **Local First**: Your data and API keys stay on your machine.
- **Tauri + Rust**: Chosen for security (memory safety), performance, and small binary size.
- **SQLite (SeaORM)**: Used for robust, local persistence of sessions and configurations.

---

## Contributing & License

Contributions are welcome. Please see `CONTRIBUTING.md`.

**License**: MIT
