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
- **`ui__*`**: Interactive UI component rendering and result reporting (`presentInteractive`, `reportResult`)
- **`agent__*`**: Autonomous sub-agent spawning and multi-agent orchestration
- **`skills__*`**: Skill execution and context loading
- **`playbook__*`**: Automation playbook listing, execution, and saving
- **`attachments__*`**: Session file attachment management and search
- **`scheduled_task__*`**: Background timers and recurring Cron task management
- **`scratchpad__*`**: Reasoning steps log and scratchpad calculation notes
- **`tool__*`**: Tool discovery and system tool management

### 2️⃣ Optional Built-ins (Configurable in Assistant Settings)

Domain-specific tools that can be enabled or disabled under **Assistants → Edit → Tools**:

- **`media__*`**: Image/visual media and audio media parsing and analysis (`seeContent`, `listenContent`)
- **`browser__*`**: Headless web browsing, DOM clicks, form typing, screenshot capture
- **`planning__*`**: Multi-step plan creation (`createGoal`), progress tracking, failure reflection (`reflect`)
- **`knowledge__*`**: Semantic memory storage and persistent knowledge retrieval
- **`setup-wizard__*`**: Python/Node/uv environment diagnostics and setup wizard
- **`history__*`**: Previous session history lookup

---

## 🛠️ Built-in Tools Detailed Reference

### 1. Workspace (`workspace__*`)

| Tool Name                               | Description                                       | Key Parameters                                    |
| :-------------------------------------- | :------------------------------------------------ | :------------------------------------------------ |
| `workspace__readFile`                   | Read file content with line slicing               | `path`, `offset`, `size`                          |
| `workspace__writeFile`                  | Create, overwrite, or append file                 | `path`, `mode`, `content`                         |
| `workspace__strReplace`                 | Exact string replacement in an existing file      | `path`, `old_string`, `new_string`, `replace_all` |
| `workspace__listDirectory`              | List directory contents                           | `path`, `limit`                                   |
| `workspace__runShell` / `runPowerShell` | Execute shell command (supports background async) | `command`, `timeout`                              |

### 2. Media (`media__*`) 🎨 _(Optional)_

| Tool Name              | Description                            | Key Parameters |
| :--------------------- | :------------------------------------- | :------------- |
| `media__seeContent`    | Inspect and analyze image/visual media | `url`          |
| `media__listenContent` | Parse and analyze audio media          | `url`          |

### 3. Interactive UI (`ui__*`)

| Tool Name                | Description                                                        |
| :----------------------- | :----------------------------------------------------------------- |
| `ui__presentInteractive` | Render interactive UI components (selection buttons, forms, cards) |
| `ui__reportResult`       | Report user selections and form inputs back to the active workflow |

### 4. Browser (`browser__*`) _(Optional)_

| Tool Name                   | Description                 |
| :-------------------------- | :-------------------------- |
| `browser__createSession`    | Start the browser session   |
| `browser__closeSession`     | Close the browser session   |
| `browser__navigateToUrl`    | Navigate to target web URL  |
| `browser__getCurrentUrl`    | Get the current page URL    |
| `browser__getPageTitle`     | Get the current page title  |
| `browser__getPageContent`   | Extract page content        |
| `browser__fetchUrl`         | Fetch URL without a session |
| `browser__clickElement`     | Click DOM element           |
| `browser__inputText`        | Type text into web input    |
| `browser__scrollPage`       | Scroll web page view        |
| `browser__listInteractable` | Extract clickable elements  |
| `browser__takeScreenshot`   | Capture the page as a PNG   |
| `browser__evaluateJS`       | Execute custom JS snippet   |

`browser__takeScreenshot` accepts an optional `fullPage` boolean. It captures the
current viewport by default; set `fullPage` to `true` to capture the entire page
within the 64-million-pixel and 8 MiB PNG limits.

### 5. Planning & Reflection (`planning__*`) _(Optional)_

| Tool Name                   | Description                                     |
| :-------------------------- | :---------------------------------------------- |
| `planning__createGoal`      | Establish multi-step goals for complex tasks    |
| `planning__updateGoal`      | Update step progress and goal status            |
| `planning__clearGoal`       | Clear active goal                               |
| `planning__addTodo`         | Manage granular todo items                      |
| `planning__updateTodo`      | Update todo item status                         |
| `planning__clearSession`    | Clear session planning data                     |
| `planning__getCurrentState` | Fetch current planning and goal state           |
| `planning__reflect`         | Generate structured reflection on tool failures |

### 6. Attachments & Scheduled Tasks (`attachments__*` / `scheduled_task__*`)

| Tool Name                             | Description                                      |
| :------------------------------------ | :----------------------------------------------- |
| `attachments__readAttachment`         | Read session attachment contents                 |
| `attachments__searchAttachments`      | Search through uploaded attachments              |
| `scheduled_task__createScheduledTask` | Create one-shot timer or recurring Cron schedule |
| `scheduled_task__listScheduledTasks`  | List all active scheduled tasks                  |
| `scheduled_task__getScheduledTask`    | Get details of a specific scheduled task         |
| `scheduled_task__updateScheduledTask` | Update scheduled task parameters                 |
| `scheduled_task__toggleScheduledTask` | Toggle scheduled task active status              |
| `scheduled_task__deleteScheduledTask` | Delete a scheduled task                          |

---

## 🧩 How to Add More Tools

When specialized capabilities beyond built-in tools are required (e.g. GitHub PR management, Slack messaging, ComfyUI image generation, arXiv paper search):

1. **One-Click Presets (Recommended Extensions)**:
   - Go to **Extensions → Tools → Recommended Extensions** to add Brave Search, Exa, GitHub, Slack, arXiv, etc. (See [Extensions Guide](extensions.md)).
2. **Add Custom MCP Server**:
   - Register local `npx`/`uvx` processes or remote HTTP SSE servers. (See [Custom MCP Guide](custom-mcp.md)).
3. **Add Skills**:
   - Install domain-specific workflow instructions as Skills. (See [Skills Guide](skills.md)).
