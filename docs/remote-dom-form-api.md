# Remote DOM Builder - Form-Based API

The Remote DOM Builder now supports elegant form semantics with your proposed pattern:

```typescript
builder
  .formStart()
  .selectInput({
    label: 'Please choose an option',
    options: options,
    required: true,
    onSubmit: (value) => ({ type: 'prompt', payload: value }),
  })
  .button({
    text: 'Confirm',
    variant: 'default',
    type: 'submit', // This makes clicking the button trigger onSubmit
  })
  .formEnd()
  .buildScript();
```

## 🎯 Key Features

### 1. Elegant Function Pattern

Your brilliant function-based event handlers make the API intuitive and flexible:

```typescript
// Simple value passing
onSubmit: (value) => ({ type: 'prompt', payload: value });

// Complex payloads
onClick: (value) => ({
  type: 'tool',
  payload: {
    toolName: 'save_data',
    params: { data: value, timestamp: Date.now() },
  },
});

// Notifications
onClick: () => ({
  type: 'notify',
  payload: { message: 'Button clicked!' },
});
```

### 2. Form Semantics

- `formStart()` - Creates a semantic form container
- `formEnd()` - Closes the form container
- `type: 'submit'` - Buttons automatically trigger form submission
- Automatic form data collection and submission

### 3. MCP-UI Compliant

All generated messages match the UIActionResult interface expected by MessageRenderer:

```typescript
// Generated postMessage format
window.parent.postMessage(
  {
    type: 'prompt' | 'tool' | 'intent' | 'link' | 'notify',
    payload: any,
  },
  '*',
);
```

## 📋 Complete API Examples

### Text Input Form

```typescript
builder
  .formStart()
  .textInput({
    label: 'Your name',
    placeholder: 'Enter your name...',
    onSubmit: (value) => ({ type: 'prompt', payload: value }),
  })
  .button({
    text: 'Submit',
    type: 'submit',
  })
  .formEnd()
  .buildScript();
```

### Selection Form with Tool Call

```typescript
builder
  .formStart()
  .selectInput({
    label: 'Choose action',
    options: ['create', 'update', 'delete'],
    onSubmit: (value) => ({
      type: 'tool',
      payload: {
        toolName: 'execute_action',
        params: { action: value },
      },
    }),
  })
  .button({
    text: 'Execute',
    variant: 'destructive',
    type: 'submit',
  })
  .formEnd()
  .buildScript();
```

### Multiple Inputs with Complex Logic

```typescript
builder
  .formStart()
  .card({ title: 'User Registration' })
  .textInput({
    label: 'Email',
    onSubmit: (email) => ({
      type: 'intent',
      payload: {
        intent: 'validate_email',
        params: { email },
      },
    }),
  })
  .selectInput({
    label: 'Role',
    options: ['admin', 'user', 'guest'],
    onSubmit: (role) => ({
      type: 'tool',
      payload: {
        toolName: 'create_user',
        params: { role },
      },
    }),
  })
  .button({
    text: 'Register',
    variant: 'default',
    type: 'submit',
  })
  .button({
    text: 'Cancel',
    variant: 'outline',
    onClick: () => ({ type: 'link', payload: { url: '/cancel' } }),
  })
  .formEnd()
  .buildScript();
```

### Notification Patterns

```typescript
// Success notification
onClick: () => ({
  type: 'notify',
  payload: { message: 'Data saved successfully!' },
});

// Error notification
onClick: () => ({
  type: 'notify',
  payload: { message: 'Failed to save data. Please try again.' },
});

// Progress notification
onClick: () => ({
  type: 'notify',
  payload: { message: 'Processing... Please wait.' },
});
```

## 🔄 Form Submission Flow

1. **User fills input** → Input value stored
2. **User clicks submit button** → Button detects `type: 'submit'`
3. **Form submission triggered** → Finds first input with `onSubmit` handler
4. **Handler executed** → `(value) => ({ type: '...', payload: ... })` called
5. **Message sent** → `postMessage` sent to MessageRenderer
6. **Action processed** → MessageRenderer handles the UIActionResult

## 🎨 Styling & Variants

All components use shadcn/ui classes for consistent styling:

```typescript
button({
  text: 'Save',
  variant:
    'default' | 'destructive' | 'outline' | 'secondary' | 'ghost' | 'link',
  size: 'default' | 'sm' | 'lg' | 'icon',
  type: 'button' | 'submit',
});
```

## 🔧 Advanced Patterns

### Conditional Logic

```typescript
const createSubmitHandler = (userRole: string) => {
  return (value) => {
    if (userRole === 'admin') {
      return {
        type: 'tool',
        payload: { toolName: 'admin_action', params: { value } },
      };
    } else {
      return { type: 'prompt', payload: `User input: ${value}` };
    }
  };
};

builder.textInput({ onSubmit: createSubmitHandler('admin') }).buildScript();
```

### Multi-step Forms

```typescript
builder
  .formStart()
  .textInput({
    label: 'Step 1: Name',
    onSubmit: (name) => ({
      type: 'tool',
      payload: {
        toolName: 'next_step',
        params: { step: 1, name },
      },
    }),
  })
  .button({ text: 'Next', type: 'submit' })
  .formEnd()
  .buildScript();
```

This elegant API provides maximum flexibility while maintaining semantic HTML structure and MCP-UI compliance!
