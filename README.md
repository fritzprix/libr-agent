# 🤖 LibrAgent

> **The Orchestration Layer for Autonomous Intelligence.**

[한국어](./README.ko.md) | [简体中文](./README.zh.md) | [日本語](./README.ja.md) | [Français](./README.fr.md) | [Español](./README.es.md) | [Deutsch](./README.de.md) | [Português](./README.pt.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-24C8DB?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-Latest-CE422B?logo=rust)](https://www.rust-lang.org)

LibrAgent is a high-performance **Meta Agent Platform** designed to industrialize autonomous workflows. Moving beyond simple chat interfaces, it provides a robust orchestration engine and a secure execution substrate where specialized agents collaborate to solve complex, multi-step missions.

By implementing open standards like the **Model Context Protocol (MCP)** and a recursive delegation architecture, LibrAgent transforms raw LLM capabilities into a coordinated swarm of intelligence.

---

## Why LibrAgent?

Modern AI work requires more than a stateless window; it requires **Strategic Autonomy**. LibrAgent bridges the gap between manual prompts and fully autonomous systems by providing a local-first environment where humans can design, deploy, and govern agentic teams with precision.

---

## 🎬 Platform in Action

![LibrAgent Demo](assets/demo_1280_4x_optimized.gif)

_Recursive delegation and high-fidelity tool usage in a unified, stateful substrate._

---

## Core Pillars

### 1. Multi-Agent Orchestration

LibrAgent is built for scale. It allows agents to spawn, brief, and manage specialized sub-agents with strict governance.

- **Hierarchical Delegation**: Transparent parent-child lineages with configurable depth and fan-out limits.
- **Role-Based Specialization**: Define unique "Souls" and toolsets for specific mission phases.
- **Swarm Coordination**: Real-time message routing and terminal results monitoring across the agent tree.

### 2. MCP-First Ecosystem

Standardization is at the heart of the platform. We use the Model Context Protocol to ensure infinite extensibility.

- **Universal Tooling**: Instantly connect to any MCP-compliant server (GitHub, Brave, Slack, etc.).
- **Dynamic Service Proxying**: Isolated tool instances per session to prevent context leakage.
- **One-Click Integration**: A curated catalog of essential agent capabilities.

### 3. Context & Substrate Persistence

Agents operate within a long-lived environment that preserves the state of their work.

- **Shared Workspace**: A secure, persistent file substrate where all agents in a lineage can collaborate.
- **Live Execution Environment**: Persistent browser sessions (Tauri) and sandboxed shells (Python/Node.js) that stay alive between turns.
- **Context Compaction**: Intelligent history management for sustained performance in long-running missions.

### 4. Operational Governance & Autonomy

Take control of autonomous execution with robust safety and scheduling features.

- **YOLO Mode**: Configurable autonomous execution for high-trust environments.
- **Scheduled Missions**: CRON-based automation with automatic recovery and workspace targeting.
- **Observability**: Real-time performance metrics (TPS) and prompt caching transparency.

---

## 📖 Documentation & Guides

LibrAgent is an industrial-grade platform. Explore our detailed resources:

- **[Navigation Guide](docs/guides/navigation-guide.md)**: Explore the Command & Control hub, including `/assistants` (Role Definitions) and `/playbooks` (Workflow Blueprints).
- **[Architecture Guide](docs/architecture/agent-workflow-architecture.md)**: Deep dive into the orchestration engine and session isolation logic.

---

## 📦 Getting Started

Download the latest binaries for Windows, macOS, or Linux from the [Release page](https://github.com/fritzprix/libr-agent/releases/latest).

**Developer Setup:**

```bash
git clone https://github.com/fritzprix/libr-agent
cd libr-agent
pnpm install
pnpm tauri dev
```

---

## Design Philosophy

- **Local First**: Your data, keys, and agent "souls" remain under your exclusive control.
- **Memory Safety**: Powered by Rust and Tauri for maximum security and performance.
- **Open Standards**: Fully committed to MCP and open-source interoperability.

---

## Contributing & License

We are building the future of autonomous intelligence. Join us on GitHub.

**License**: MIT
