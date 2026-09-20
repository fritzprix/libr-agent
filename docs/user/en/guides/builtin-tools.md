---
title: Built-in Tools Guide
---

# Built-in Tools Guide

LibrAgent comes with an out-of-the-box set of native **Built-in Tools**, enabling agents to inspect files, edit code, browse the web, capture screenshots, and execute terminal commands immediately without requiring external tool setups.

When you send a request, the agent autonomously selects and invokes the most appropriate tools to complete your task.

---

## 🛡️ Safety & Execution Approval Policies

Tool execution remains strictly under user supervision:

| Tool Category                 | Safety Policy                     | Key Operations                                                                              |
| ----------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------- |
| **Read-Only Tools**           | Automatic execution               | Reading files, viewing web pages, searching session history                                 |
| **System Modification Tools** | **Approval Dialog** (Normal mode) | Running shell commands (`runShell`), creating or modifying files                            |
| **Desktop Control Tools**     | **Explicit Approval Required**    | Taking desktop screenshots (`captureScreen`), simulating mouse/keyboard (`computerControl`) |

> [!TIP]
> For unattended background runs (e.g., Scheduled Tasks), you can switch the execution mode to **YOLO** or **Unsafe** to auto-approve standard tool executions without blocking on confirmation popups.

---

## 🛠️ Core Tool Capabilities

### 1. Workspace & Files (`workspace`)

Agents inspect and edit files directly within your configured project directory.

- **Reading & Searching**: Understands codebase structure and text documents.
- **Precise Line Edits**: Makes surgical replacements without overwriting entire files.
- **Terminal Execution**: Runs builds, test suites, or package managers and captures output.

### 2. Web Browsing (`browser`)

Interacts with the web through an isolated browser process.

- **Web Navigation**: Reads online documentation, technical articles, and live feeds.
- **Visual Verification**: Captures screenshots of rendered web pages.

### 3. Media & Desktop Interaction (`media`, `desktop`)

Handles visual analysis and operating system interactions.

- **Screen Capture**: Inspects error windows or UI layouts visually.
- **Mouse & Keyboard Control**: Simulates clicks, typing, and shortcuts for GUI workflows. _(Always requires manual approval)_

### 4. Planning & Scheduling (`planning`, `scheduled_task`)

Organizes complex goals into structured sub-tasks.

- **Multi-step Goals**: Breaks down large instructions into a structured Todo list and reports progress.
- **Scheduled Automations**: Registers recurring background tasks according to Cron expressions.

---

## ⚙️ Optimizing Tools per Assistant

Enabling every tool in all sessions increases the context tokens the LLM must consume to read tool definitions.

1. Navigate to **Assistants** in the sidebar.
2. Select **Edit → Tools** on any assistant profile.
3. Keep only relevant tools active (e.g., enable `workspace` for coding assistants, `browser` for research assistants).

---

## 📖 Technical Reference: Tool Identifiers (`{server}__{tool}`)

For power users and prompt engineering, below is the primary built-in tools inventory:

| Server               | Tool Identifier                       | Description                                 | Default Status               |
| -------------------- | ------------------------------------- | ------------------------------------------- | ---------------------------- |
| **`workspace`**      | `workspace__readFile`                 | Slice and read file contents                | Core (Default)               |
|                      | `workspace__writeFile`                | Create or overwrite files                   | Core (Default)               |
|                      | `workspace__strReplace`               | Exact string replacement in file            | Core (Default)               |
|                      | `workspace__listDirectory`            | List files and directories                  | Core (Default)               |
|                      | `workspace__runShell`                 | Execute shell command (async supported)     | Core (Default)               |
| **`browser`**        | `browser__navigateToUrl`              | Navigate to URL and wait for load           | Optional                     |
|                      | `browser__takeScreenshot`             | Capture viewport or full page screenshot    | Optional                     |
|                      | `browser__getPageContent`             | Extract webpage text & DOM structure        | Optional                     |
|                      | `browser__clickElement`               | Click DOM element                           | Optional                     |
| **`desktop`**        | `desktop__computerControl`            | Mouse clicks, movement, and keyboard typing | Optional (Approval required) |
| **`media`**          | `media__captureScreen`                | Capture desktop display or area             | Optional (Approval required) |
|                      | `media__seeContent`                   | Analyze image visual content                | Optional                     |
| **`planning`**       | `planning__createGoal`                | Create multi-step goals & todos             | Optional                     |
|                      | `planning__updateGoal`                | Update goal & step progress                 | Optional                     |
| **`scheduled_task`** | `scheduled_task__createScheduledTask` | Register recurring background Cron task     | Core (Default)               |
|                      | `scheduled_task__listScheduledTasks`  | List registered scheduled tasks             | Core (Default)               |

---

## 🧩 Need Additional Tools?

If you require integration with third-party services (GitHub, Slack, remote databases):

- Visit the [Extensions Guide](extensions.md) to add verified MCP presets with one click or register custom MCP servers.
