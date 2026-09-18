# Playbook Schema & Tool Reference

This reference documents the data structure required by the `playbook__createPlaybook` and `playbook__updatePlaybook` tools.

## Playbook Structure

A playbook is a high-level representation of a repeatable workflow.

### Top-Level Fields

| Field | Type | Description |
| :--- | :--- | :--- |
| `goal` | `string` | The high-level objective of the playbook. |
| `initialCommand` | `string` | The original prompt or command that inspired this playbook. |
| `defaultTargetSession` | `object` | (Optional) Default session targeting configuration for all steps (`mode`, `sessionId`, `configId`, `sessionSlot`). |
| `sessionSlots` | `object` | (Optional) Map of logical slot names to slot configs (`mode`, `configId`, `sessionId`, `reuseAcrossSteps`). |
| `successCriteria` | `object` | Defines what completion looks like. |
| `workflow` | `array` | A list of discrete steps to achieve the goal. |

### Session Targeting & Slots

```json
{
  "defaultTargetSession": {
    "mode": "self"
  },
  "sessionSlots": {
    "analyst": {
      "mode": "spawn",
      "configId": "data-analyst-assistant-id",
      "reuseAcrossSteps": true
    },
    "reviewer": {
      "mode": "pin",
      "sessionId": "pinned-session-id"
    }
  }
}
```

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
| `sessionSlot` | `string` | (Optional) Logical slot name defined in `sessionSlots` (e.g., `analyst`, `reviewer`) for role-based delegation. |
| `targetSession` | `object` | (Optional) Direct step session target override without a named slot (`mode`: "self" \| "pin" \| "spawn", `sessionId`, `configId`). |
| `promptTemplate` | `string` | (Optional) Prompt template supporting `{variable}` interpolation placeholders. |
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
| `playbook__selectPlaybook` | Load a playbook into the active workflow context (supports `variables` for automated prompt template interpolation, and `pinnedSessionSlots` / `targetSessionId` runtime binding). |
| `playbook__deletePlaybook` | Remove a playbook from the system. |
