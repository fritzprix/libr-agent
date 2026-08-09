---
title: Built-in MCP Tools Reference
---

# Built-in MCP Tools Reference

> LibrAgent includes a unified set of **high-performance Rust-native Built-in MCP Servers**, enabling code editing, file manipulation, web browsing, task planning, media processing, and background automation without needing external MCP server setup.

All built-in tools follow a consistent `{server}__{tool}` naming convention.

---

## 📌 Core Built-ins vs Optional Built-ins

LibrAgent built-in tools are categorized into **Core tools (enabled by default on all sessions)** and **Optional tools (configurable per assistant)**.

### 1️⃣ Core Built-ins (Enabled by Default)
Essential tools automatically available for basic agent operations and UI interactions:

- **`workspace__*`**: File reading/writing/line-range editing, directory listing, terminal command execution
- **`ui__*`**: Interactive selection cards (`select_prompt`), text forms (`text_prompt`), data charts, circuit break pause (`circuitBreak`)
- **`agent__*`**: Autonomous sub-agent spawning and multi-agent orchestration
- **`skills__*`**: Skill execution and context loading
- **`playbook__*`**: Automation playbook listing, execution, and saving
- **`attachments__*`**: Session file attachment management and search
- **`scheduled_task__*`**: Background timers and recurring Cron task management
- **`scratchpad__*`**: Reasoning steps log and scratchpad calculation notes
- **`tool__*`**: Tool discovery and system tool management

### 2️⃣ Optional Built-ins (Configurable in Assistant Settings)
Domain-specific tools that can be enabled or disabled under **Assistants → Edit → Tools**:

- **`media__*`**: Image analysis, resizing, audio/image text extraction
- **`browser__*`**: Headless web browsing, DOM clicks, form typing, screenshot capture
- **`planning__*`**: Multi-step plan creation (`create_plan`), progress tracking, failure reflection (`reflect`)
- **`knowledge__*`**: Semantic memory storage and persistent knowledge retrieval
- **`setup-wizard__*`**: Python/Node/uv environment diagnostics and setup wizard
- **`history__*`**: Previous session history lookup

---

## 🛠️ Built-in Tools Detailed Reference

### 1. Workspace (`workspace__*`)
| Tool Name | Description | Key Parameters |
| :--- | :--- | :--- |
| `workspace__read_file` | Read file content with line slicing | `path`, `start_line`, `end_line` |
| `workspace__write_file` | Create or overwrite file | `path`, `content` |
| `workspace__edit_file` | Perform precise line-range edits | `path`, `edits` |
| `workspace__list_directory` | List directory contents | `path` |
| `workspace__delete_file` | Delete target file | `path` |
| `workspace__execute_command` | Execute shell command (supports background async) | `command`, `cwd`, `is_background` |

### 2. Media (`media__*`) 🎨 *(Optional)*
| Tool Name | Description | Key Parameters |
| :--- | :--- | :--- |
| `media__process_image` | Image analysis and metadata extraction | `image_path` |
| `media__resize_image` | Resize image resolution | `image_path`, `width`, `height` |
| `media__extract_text` | Extract text/speech from images/audio | `file_path` |

### 3. Interactive UI (`ui__*`)
| Tool Name | Description |
| :--- | :--- |
| `ui__select_prompt` | Render multi-choice selection buttons |
| `ui__text_prompt` | Render text input form |
| `ui__line_chart` / `ui__bar_chart` | Render interactive data charts |
| `ui__circuitBreak` | Loop detection card with `Resume Execution` button |
| `ui__wait` | Pause execution for user input |

### 4. Browser (`browser__*`) *(Optional)*
| Tool Name | Description |
| :--- | :--- |
| `browser__navigate` | Navigate to web URL |
| `browser__click` | Click DOM element |
| `browser__type` | Type text into web form |
| `browser__screenshot` | Capture web page screenshot |

### 5. Planning & Reflection (`planning__*`) *(Optional)*
| Tool Name | Description |
| :--- | :--- |
| `planning__create_plan` | Create step-by-step plan for complex multi-step tasks |
| `planning__update_plan` | Update step progress (`todo`, `in_progress`, `done`) |
| `planning__reflect` | Generate structured reflection on tool failures |

### 6. Scheduled Tasks (`scheduled_task__*`)
| Tool Name | Description |
| :--- | :--- |
| `scheduled_task__create` | Create one-shot timer or recurring Cron schedule |
| `scheduled_task__list` / `delete` | Manage active scheduled tasks |

---

## 🧩 How to Add More Tools

When specialized capabilities beyond built-in tools are required (e.g. GitHub PR management, Slack messaging, ComfyUI image generation, arXiv paper search):

1. **One-Click Presets (Recommended Extensions)**:
   - Go to **Extensions → Tools → Recommended Extensions** to add Brave Search, Exa, GitHub, Slack, arXiv, etc. (See [Extensions Guide](extensions.md)).
2. **Add Custom MCP Server**:
   - Register local `npx`/`uvx` processes or remote HTTP SSE servers. (See [Custom MCP Guide](custom-mcp.md)).
3. **Add Skills**:
   - Install domain-specific workflow instructions as Skills. (See [Skills Guide](skills.md)).
