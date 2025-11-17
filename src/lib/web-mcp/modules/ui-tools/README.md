# UI Tools Module

Built-in MCP server for user interaction tools. Provides interactive UI elements that communicate back to the agent.

## Overview

UI Tools enables AI agents to create interactive user interfaces for:

- **User Prompts**: Text input, single/multi-select options
- **Data Visualization**: Bar charts and line charts
- **Wait States**: Display wait UI with continue button

## Available Tools

### 1. `prompt_user`

Display an interactive prompt to the user.

**Parameters:**

- `prompt` (string, required): Question or instruction to show the user
- `type` (string, required): Type of prompt - `"text"`, `"select"`, or `"multiselect"`
- `options` (string[], required for select/multiselect): List of options

**Usage Examples:**

```typescript
// Text input
{
  prompt: "What is your name?",
  type: "text"
}

// Single select
{
  prompt: "Choose your favorite color",
  type: "select",
  options: ["Red", "Blue", "Green"]
}

// Multi-select
{
  prompt: "Select features to enable",
  type: "multiselect",
  options: ["Feature A", "Feature B", "Feature C"]
}
```

**Returns:**

- Creates an interactive UI element
- Generates a unique `messageId` for tracking the response
- Status: `awaiting_response`

---

### 2. `reply_prompt`

Receive user response from prompt UI (automatically called by UI action).

**Parameters:**

- `messageId` (string, required): ID of the prompt being replied to
- `answer` (string | string[] | null): User's answer
- `cancelled` (boolean): Whether the user cancelled the prompt

**Response Format:**

- For text prompts: `answer` is a string
- For select prompts: `answer` is a string (selected option)
- For multiselect prompts: `answer` is an array of strings
- If cancelled: `answer` is null and `cancelled` is true

**Note:** This tool is typically called automatically by the UI, not manually by the agent.

---

### 3. `visualize_data`

Create a simple data visualization (bar or line chart).

**Parameters:**

- `type` (string, required): Chart type - `"bar"` or `"line"`
- `data` (array, required): Data points to visualize
  - Each item must have:
    - `label` (string, required): Non-empty label for the data point
    - `value` (number, required): Finite numeric value (no NaN, Infinity)
  - Minimum: 1 data point
  - Maximum: 50 data points
  - **Recommended**: 20 or fewer points for optimal readability
- `title` (string, optional): Title for the chart

**Usage Examples:**

```typescript
// Bar chart
{
  type: "bar",
  data: [
    { label: "Jan", value: 100 },
    { label: "Feb", value: 150 },
    { label: "Mar", value: 120 }
  ],
  title: "Monthly Sales"
}

// Line chart
{
  type: "line",
  data: [
    { label: "Week 1", value: 45 },
    { label: "Week 2", value: 52 },
    { label: "Week 3", value: 48 }
  ],
  title: "Weekly Performance"
}
```

**Data Validation:**

- Labels must be non-empty strings
- Values must be finite numbers (rejects NaN, Infinity, -Infinity)
- Negative values are allowed
- Warning displayed if more than 20 data points provided

---

### 4. `wait_for_user_resume`

Display wait UI with continue button for long operations.

**Parameters:**

- `message` (string, required): Message to display to user
- `resumeInstruction` (string, required): What to do after the user resumes (for agent context)

**Usage Example:**

```typescript
{
  message: "Processing data, please wait...",
  resumeInstruction: "Continue after data processing completes"
}
```

**Use Cases:**

- Long-running operations requiring periodic user confirmation
- Repetitive polling scenarios
- Multi-step workflows with user checkpoints

---

### 5. `resume_from_wait`

Resume from wait state (automatically called by UI button click).

**Parameters:**

- `resumeInstruction` (string, required): Resume instruction that was set when waiting started
- `startedAt` (string, required): ISO 8601 timestamp when waiting started
- `sessionId` (string, optional): Optional session ID for validation

**Response:**

- Duration information (formatted as "Xh Ym" or "Xm Ys" or "Xs")
- Resume context for the agent

**Note:** This tool is typically called automatically by the UI continue button.

---

## Features

### Security

- ⚠️ **Note:** Current implementation uses `postMessage('*')` for cross-frame communication. Consider restricting origin in production environments.

### Memory Management

- **TTL (Time-To-Live)**: Prompts expire after 1 hour
- **Automatic Cleanup**: Expired prompts are removed every 5 minutes to prevent memory leaks
- Active prompts are stored in memory with creation timestamp

### Accessibility

- Auto-focus on input fields
- Keyboard navigation support (Enter to submit)
- ARIA attributes for screen readers
- Semantic HTML structure

### HTML Template Rendering

- Uses Handlebars templates for consistent UI
- Automatic HTML escaping for security
- Responsive design with mobile support

---

## Implementation Details

### Template Files

- `templates/text-prompt.hbs`: Text input UI
- `templates/select-prompt.hbs`: Single/multi-select UI
- `templates/bar-chart.hbs`: Bar chart visualization
- `templates/line-chart.hbs`: Line chart visualization
- `templates/wait.hbs`: Wait UI with continue button

### State Management

Prompts are tracked in an in-memory `Map`:

```typescript
interface PromptState {
  messageId: string;
  prompt: string;
  type: 'text' | 'select' | 'multiselect';
  options?: string[];
  createdAt: number;
}
```

### Error Handling

All tools return structured error responses for:

- Missing required parameters
- Invalid data formats
- Expired or unknown prompt IDs
- Invalid numeric values (NaN, Infinity)

---

## Best Practices

### When to Use Each Tool

1. **`prompt_user`** - Gather specific user input:
   - Simple questions requiring text answers
   - Selection from predefined options
   - Multiple choice selections

2. **`visualize_data`** - Present numeric data:
   - Comparisons (use bar charts)
   - Trends over time (use line charts)
   - Keep data points to 20 or fewer for readability

3. **`wait_for_user_resume`** - Long operations:
   - Operations that might take several minutes
   - Multi-step workflows requiring user approval
   - Polling scenarios where periodic user intervention is needed

### Recommendations

- **Prompt Design**: Keep prompts clear and concise
- **Option Count**: Limit select options to 10 or fewer for usability
- **Data Visualization**:
  - Use bar charts for categorical comparisons
  - Use line charts for temporal trends
  - Limit to 20 data points for readability
- **Error Messages**: Provide clear, actionable error messages to users

---

## Migration Notes

Previous versions used a `context` object with `{reason, command, nextAction}` for wait/resume tools. This has been simplified to a direct `resumeInstruction` parameter.

**Old format (deprecated):**

```typescript
{
  context: {
    reason: "waiting for data",
    command: "fetch",
    nextAction: "continue"
  }
}
```

**New format:**

```typescript
{
  resumeInstruction: 'Continue after data processing';
}
```

---

## Future Improvements

### Planned Features

- Origin validation for `postMessage` security
- Enhanced ARIA support for better accessibility
- Configurable TTL for prompts
- Additional chart types (pie, scatter)
- File upload prompts
- Date/time picker prompts

### Known Limitations

- Maximum 50 data points per chart
- No custom styling options for charts
- Prompts expire after 1 hour (not configurable)
- No persistent storage across sessions
