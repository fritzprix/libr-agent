# Flexible Event System for Remote DOM Builder

The RemoteDomBuilder now supports a flexible event system that allows you to define structured event handlers instead of just simple strings.

## Event Handler Types

### 1. Legacy String Handlers (Backward Compatible)

```typescript
button({
  text: 'Submit',
  onClick: 'submit_form', // Simple string - generates intent message
});
```

### 2. Structured Event Configuration

```typescript
button({
  text: 'Submit',
  onClick: {
    messageType: 'prompt', // Type of message to send
    payloadTemplate: { action: 'submit' }, // Static payload structure
    useValue: false, // Don't include element value
  },
});
```

## Event Configuration Interface

```typescript
interface EventConfig {
  messageType: 'prompt' | 'intent' | 'tool_call' | 'custom';
  payloadTemplate?: unknown; // Template for the payload
  useValue?: boolean; // Whether to include element's value
}
```

## Message Types

- **`prompt`**: Sends user input data back to the chat
- **`intent`**: Triggers a specific intent/action
- **`tool_call`**: Invokes a specific tool function
- **`custom`**: Custom message type for special handling

## Examples

### Text Input with Value Extraction

```typescript
textInput({
  label: 'Your name',
  onSubmit: {
    messageType: 'prompt',
    payloadTemplate: '${value}', // Use the input value directly
    useValue: true,
  },
});
```

**Generated JavaScript:**

```javascript
input.addEventListener('keypress', (e) => {
  window.parent.postMessage(
    {
      type: 'prompt',
      payload: e.target.value.trim(),
    },
    '*',
  );
});
```

### Button with Structured Payload

```typescript
button({
  text: 'Save Changes',
  onClick: {
    messageType: 'tool_call',
    payloadTemplate: {
      tool: 'save_document',
      params: { format: 'json' },
    },
  },
});
```

**Generated JavaScript:**

```javascript
button.addEventListener('click', (e) => {
  window.parent.postMessage(
    {
      type: 'tool_call',
      payload: { tool: 'save_document', params: { format: 'json' } },
    },
    '*',
  );
});
```

### Select with Complex Payload

```typescript
selectInput({
  label: 'Choose priority',
  options: ['high', 'medium', 'low'],
  onSubmit: {
    messageType: 'prompt',
    payloadTemplate: {
      priority: '${value}',
      timestamp: new Date().toISOString(),
    },
    useValue: true,
  },
});
```

**Generated JavaScript:**

```javascript
select.addEventListener('change', (e) => {
  window.parent.postMessage(
    {
      type: 'prompt',
      payload: {
        priority: e.target.value,
        timestamp: '2024-01-01T00:00:00.000Z',
      },
    },
    '*',
  );
});
```

## Benefits

1. **Flexible**: Support both simple strings and complex structured payloads
2. **Type-safe**: Full TypeScript support with proper interfaces
3. **Backward compatible**: Existing string handlers continue to work
4. **Extensible**: Easy to add new message types and payload structures
5. **Clear intent**: Explicit about what data gets sent and how

## Implementation in Planning Server

The planning server now uses this flexible system:

```typescript
// Simple prompt message with input value
textInput({
  label: 'Your response',
  onSubmit: {
    messageType: 'prompt',
    payloadTemplate: '${value}',
    useValue: true,
  },
});

// Complex structured message
button({
  text: 'Submit',
  onClick: {
    messageType: 'prompt',
    payloadTemplate: { action: 'submit', form: 'user_input' },
    useValue: false,
  },
});
```

This approach gives you the flexibility to define exactly what gets sent to the parent window while maintaining clean, readable code.
