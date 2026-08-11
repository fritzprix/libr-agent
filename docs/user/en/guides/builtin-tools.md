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

### 1. Workspace (`workspace__*` & `runShell`)

| Tool Name                               | Description                                       | Key Parameters                                      |
| :-------------------------------------- | :------------------------------------------------ | :-------------------------------------------------- |
| `workspace__readFile`                   | Read file content with line slicing               | `AbsolutePath`, `StartLine`, `EndLine`              |
| `workspace__writeFile`                  | Create or overwrite file                          | `TargetFile`, `CodeContent`, `Overwrite`            |
| `workspace__replace_file_content`       | Perform precise line-range edits                  | `TargetFile`, `TargetContent`, `ReplacementContent` |
| `workspace__multi_replace_file_content` | Edit multiple non-adjacent line blocks            | `TargetFile`, `ReplacementChunks`                   |
| `workspace__listDirectory`              | List directory contents                           | `DirectoryPath`                                     |
| `workspace__runShell` / `runPowerShell` | Execute shell command (supports background async) | `CommandLine`, `Cwd`, `WaitMsBeforeAsync`           |

### 2. Media (`media__*`) 🎨 _(Optional)_

| Tool Name              | Description                            | Key Parameters        |
| :--------------------- | :------------------------------------- | :-------------------- |
| `media__seeContent`    | Inspect and analyze image/visual media | `AbsolutePath`        |
| `media__listenContent` | Parse and analyze audio media          | `AbsolutePath`        |
| `generate_image`       | AI image generation and visual mockups | `Prompt`, `ImageName` |

### 3. Interactive UI (`ask_question` / `ui__*`)

| Tool Name      | Description                                     |
| :------------- | :---------------------------------------------- |
| `ask_question` | Render interactive multi-choice selection modal |
| `ui__wait`     | Pause execution for user input                  |

### 4. Browser (`browser__*`) _(Optional)_

| Tool Name          | Description                |
| :----------------- | :------------------------- |
| `navigateToUrl`    | Navigate to target web URL |
| `clickElement`     | Click DOM element          |
| `inputText`        | Type text into web input   |
| `scrollPage`       | Scroll web page view       |
| `listInteractable` | Extract clickable elements |
| `evaluateJS`       | Execute custom JS snippet  |

### 5. Planning & Reflection (`planning__*`) _(Optional)_

| Tool Name              | Description                                     |
| :--------------------- | :---------------------------------------------- |
| `planning__createGoal` | Establish multi-step goals for complex tasks    |
| `planning__updateGoal` | Update step progress and goal status            |
| `planning__addTodo`    | Manage granular todo items                      |
| `planning__reflect`    | Generate structured reflection on tool failures |

### 6. Attachments & Scheduled Tasks (`attachments__*` / `scheduled_task__*`)

| Tool Name                         | Description                                      |
| :-------------------------------- | :----------------------------------------------- |
| `attachments__readAttachment`     | Read session attachment contents                 |
| `attachments__searchAttachments`  | Search through uploaded attachments              |
| `scheduled_task__create`          | Create one-shot timer or recurring Cron schedule |
| `scheduled_task__list` / `delete` | Manage active scheduled tasks                    |

---

## 🧩 How to Add More Tools

When specialized capabilities beyond built-in tools are required (e.g. GitHub PR management, Slack messaging, ComfyUI image generation, arXiv paper search):

1. **One-Click Presets (Recommended Extensions)**:
   - Go to **Extensions → Tools → Recommended Extensions** to add Brave Search, Exa, GitHub, Slack, arXiv, etc. (See [Extensions Guide](extensions.md)).
2. **Add Custom MCP Server**:
   - Register local `npx`/`uvx` processes or remote HTTP SSE servers. (See [Custom MCP Guide](custom-mcp.md)).
3. **Add Skills**:
   - Install domain-specific workflow instructions as Skills. (See [Skills Guide](skills.md)).
