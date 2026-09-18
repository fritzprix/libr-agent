# Playbook Schema & Tool Reference

This reference documents the data structure required by the `playbook__createPlaybook` and `playbook__updatePlaybook` tools.

## Playbook Structure

A playbook is a high-level representation of a repeatable workflow.

### Top-Level Fields

| Field | Type | Description |
| :--- | :--- | :--- |
| `goal` | `string` | The high-level objective of the playbook. |
| `initialCommand` | `string` | The original prompt or command that inspired this playbook. |
| `defaultTargetSession` | `object` | (Optional) Start launch target, stored in its own DB column (not inside `workflow`). `{ "mode": "pin", "sessionId": "<existing>" }` makes Playbook Card Start open that session and UI-inject `selectPlaybook` instead of creating a new session. `{ "mode": "self" }` (or omit) keeps the default new-session Start path. When creating a "세션 플레이북" / session playbook, set `{ "mode": "pin" }` (sessionId can be omitted to pin calling session). Prefer the exact session id; legacy short / `session-…` refs are still accepted and canonicalized on write. |
| `successCriteria` | `object` | Defines what completion looks like. |
| `workflow` | `array` | A list of discrete steps to achieve the goal. |

> **Note:** `defaultTargetSession` is a Start UX pin only. When a user asks for a "세션 플레이북" (Session Playbook), always supply `{ mode: "pin" }`. It does not change `selectPlaybook` prompts or add multi-session routing / slots.

### successCriteria Object

| Field | Type | Description |
| :--- | :--- | :--- |
| `description` | `string` | Qualitative description of a successful outcome. |
| `requiredArtifacts` | `string[]` | Specific files, logs, or states that MUST exist for success. |

### Workflow Step Object

| Field | Type | Description |
| :--- | :--- | :--- |
| `stepId` | `string` | A unique identifier for the step (e.g., `step_1`, `read_config`). |
| `description` | `string` | Clear instruction of what this step accomplishes. |
| `action` | `object` | The specific tool call associated with this step. |
| `requiredData` | `string[]` | List of `outputVariable` names from previous steps needed as input. |
| `outputVariable` | `string` | Name to store this step's result (for use in `requiredData`). |

### Action Object

| Field | Type | Description |
| :--- | :--- | :--- |
| `toolName` | `string` | The exact name of the tool to be called (e.g., `workspace__readFile`). |
| `purpose` | `string` | Why this tool is being used in this context. |

## Tool Group: `playbook`

| Tool | Purpose |
| :--- | :--- |
| `playbook__createPlaybook` | Register a new reusable workflow. |
| `playbook__listPlaybooks` | Search/list available playbooks. |
| `playbook__getPlaybook` | Retrieve the full JSON definition of a playbook. |
| `playbook__updatePlaybook` | Modify an existing playbook. |
| `playbook__selectPlaybook` | Load a playbook into the current session context. |
| `playbook__deletePlaybook` | Remove a playbook from the system. |
