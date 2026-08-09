---
title: Built-in MCP Tools Reference
---

# Built-in MCP Tools Reference

> LibrAgent includes a unified set of **high-performance Rust-native Built-in MCP Servers**, enabling code editing, file manipulation, web browsing, task planning, and background automation without needing external MCP server setup.

All built-in tools follow a consistent `{server}__{tool}` naming convention.

---

## 1. Workspace (`workspace__*`)

Core file management, precise line editing, and terminal command execution tools for software development.

| Tool Name | Description | Key Parameters |
| :--- | :--- | :--- |
| `workspace__read_file` | Read file content with optional line slicing | `path`, `start_line`, `end_line` |
| `workspace__write_file` | Create or overwrite a file | `path`, `content` |
| `workspace__edit_file` | Perform precise line-range edits | `path`, `edits: [{ old_line, new_line, content }]` |
| `workspace__list_directory` | List directory structure and contents | `path` |
| `workspace__delete_file` | Delete a target file | `path` |
| `workspace__execute_command` | Execute shell command (supports async background tasks) | `command`, `cwd`, `is_background` |

---

## 2. Interactive UI (`ui__*`)

Renders dynamic UI widgets (selection cards, input forms, charts, circuit break pause) directly inside the chat view.

| Tool Name | Description | Key Feature |
| :--- | :--- | :--- |
| `ui__select_prompt` | Render multi-choice selection buttons | Sends selected option back to agent on user click |
| `ui__text_prompt` | Render text input card | Allows user to input response directly |
| `ui__line_chart` / `ui__bar_chart` | Render interactive data charts | Returns visual chart resources |
| `ui__circuitBreak` | Detect and break infinite tool execution loops | Renders Amber safety card with `Resume Execution` button |
| `ui__wait` | Pause execution for user input or external events | Switches session to Idle state |

---

## 3. Planning & Reflection (`planning__*`)

Autonomous reasoning tools for creating structured execution plans and reflecting on task failures.

| Tool Name | Description | Best Used When |
| :--- | :--- | :--- |
| `planning__create_plan` | Create step-by-step session plan | Starting multi-step architecture or refactoring tasks |
| `planning__update_plan` | Update step statuses (`todo`, `in_progress`, `done`) | Progressing through plan steps |
| `planning__reflect` | Generate structured critique and reflection on failures | Encrypted or repeated tool failures occur |

---

## 4. Playbooks (`playbook__*`)

Save, reuse, and execute pre-defined automation workflows and templates.

| Tool Name | Description |
| :--- | :--- |
| `playbook__list_playbooks` | List available playbooks |
| `playbook__show_playbook` | View playbook details and steps |
| `playbook__run_playbook` | Execute target playbook autonomously |
| `playbook__save_playbook` | Save new playbook template |

---

## 5. Browser (`browser__*`)

Headless browser automation tools for web research and UI verification.

| Tool Name | Description |
| :--- | :--- |
| `browser__navigate` | Open target URL in headless browser |
| `browser__click` | Click DOM element on page |
| `browser__type` | Type input text into web forms |
| `browser__screenshot` | Capture current web page screenshot |

---

## 6. Scheduled Tasks (`scheduled_task__*`)

Background automation timers and recurring Cron schedules.

| Tool Name | Description |
| :--- | :--- |
| `scheduled_task__create` | Create one-shot timer or recurring Cron schedule |
| `scheduled_task__list` | List active background tasks |
| `scheduled_task__delete` | Cancel and delete scheduled task |

---

## 7. Knowledge & Scratchpad (`knowledge__*`, `scratchpad__*`)

Semantic project memory and temporary reasoning scratchpad.

- `knowledge__search` / `store`: Semantic search and storage for persistent project memory
- `scratchpad__think`: Write transient reasoning notes during complex tasks

---

## Assistant Tool Permissions

Configure allowed built-in tool sets for each assistant:
1. Open sidebar **Assistants** menu.
2. Select target assistant and click **Edit**.
3. Under **Tools** tab, select allowed Built-in servers. (See [Assistants Guide](assistants.md)).
