# Modular Chat Component

A flexible, composable chat interface built with React that allows you to create customizable chat experiences using a compound component pattern.

## Overview

The Chat component has been refactored from a monolithic structure into modular, reusable components that can be composed together to create different chat layouts and experiences. This approach provides maximum flexibility while maintaining all the original functionality.

## Features

- 🧩 **Modular Design**: Compose your chat interface from individual components
- 🔄 **Shared State**: All components automatically share state through React Context
- 📁 **File Attachments**: Support for text file uploads with preview
- 🔧 **Tool Integration**: Built-in support for MCP and local tools
- 💬 **Message History**: Automatic message management and display
- 🎨 **Customizable**: Easy to extend and customize each section
- 📱 **Responsive**: Works across different screen sizes
- ⌨️ **Keyboard Shortcuts**: Submit with Enter, file shortcuts

## Quick Start

### Basic Usage

```jsx
import Chat from './Chat';

function App() {
  return (
    <Chat>
      <Chat.Header />
      <Chat.Messages />
      <Chat.Bottom>
        <Chat.StatusBar />
        <Chat.AttachedFiles />
        <Chat.Input />
      </Chat.Bottom>
    </Chat>
  );
}
```

### Minimal Setup

```jsx
function MinimalChat() {
  return (
    <Chat>
      <Chat.Messages />
      <Chat.Input />
    </Chat>
  );
}
```

## Components

### `<Chat>`

The main container component that provides context and shared state to all child components.

**Props:**

- `children?: React.ReactNode` - Child components to render

**Required Context:**

- Must be wrapped with required context providers (`useAssistantContext`, `useSessionContext`, `useChatContext`)

### `<Chat.Header>`

Renders the terminal-style header section.

**Props:**

- `children?: React.ReactNode` - Additional content to display in header

**Example:**

```jsx
<Chat.Header>
  <Button variant="ghost" size="sm">
    Settings
  </Button>
</Chat.Header>
```

### `<Chat.Messages>`

Displays the message history and handles auto-scrolling to new messages.

**Features:**

- Auto-scroll to newest messages
- Loading indicator when AI is thinking
- Message bubbles with role identification
- Overflow scrolling with custom scrollbar

### `<Chat.StatusBar>`

Shows the model picker and available tools count.

**Props:**

- `children?: React.ReactNode` - Additional status information

**Features:**

- Model selection dropdown
- Tools count with modal trigger
- Extensible with custom status items

**Example:**

```jsx
<Chat.StatusBar>
  <span className="text-xs ml-2 text-green-500">Connected</span>
</Chat.StatusBar>
```

### `<Chat.AttachedFiles>`

Displays attached files with remove functionality. Only shows when files are attached.

**Features:**

- File name display with truncation
- Remove file buttons
- Responsive layout
- Auto-hide when empty

### `<Chat.Input>`

The main input form with file attachment support and submit button.

**Props:**

- `children?: React.ReactNode` - Additional input controls (buttons, etc.)

**Features:**

- Text input with placeholder states
- File attachment button
- Submit button with enter key support
- Disabled state during loading

**Example:**

```jsx
<Chat.Input>
  <Button variant="ghost" size="sm" title="Voice input">
    🎤
  </Button>
  <Button variant="ghost" size="sm" title="Quick actions">
    ⚡
  </Button>
</Chat.Input>
```

### `<Chat.Bottom>`

Container for bottom UI elements (status bar, files, input).

**Props:**

- `children?: React.ReactNode` - Bottom section components

## Advanced Examples

### Custom Layout with Additional Controls

```jsx
function AdvancedChat() {
  return (
    <Chat>
      <Chat.Header>
        <div className="flex gap-2">
          <Button variant="ghost" size="sm">
            Clear History
          </Button>
          <Button variant="ghost" size="sm">
            Export Chat
          </Button>
          <Button variant="ghost" size="sm">
            Settings
          </Button>
        </div>
      </Chat.Header>

      <Chat.Messages />

      <Chat.Bottom>
        <Chat.AttachedFiles />

        <Chat.StatusBar>
          <div className="flex items-center gap-2 ml-2">
            <span className="text-xs text-green-500">●</span>
            <span className="text-xs">Connected</span>
          </div>
        </Chat.StatusBar>

        <Chat.Input>
          <Button variant="ghost" size="sm" title="Templates">
            📋
          </Button>
          <Button variant="ghost" size="sm" title="Voice">
            🎤
          </Button>
        </Chat.Input>
      </Chat.Bottom>
    </Chat>
  );
}
```

### Reordered Layout

```jsx
function ReorderedChat() {
  return (
    <Chat>
      {/* Status at top */}
      <Chat.StatusBar />

      <Chat.Messages />

      {/* Files above input */}
      <Chat.AttachedFiles />
      <Chat.Input />
    </Chat>
  );
}
```

### Conditional Rendering

```jsx
function ConditionalChat({ showHeader = true, showStatus = true }) {
  return (
    <Chat>
      {showHeader && <Chat.Header />}
      <Chat.Messages />
      <Chat.Bottom>
        {showStatus && <Chat.StatusBar />}
        <Chat.AttachedFiles />
        <Chat.Input />
      </Chat.Bottom>
    </Chat>
  );
}
```

## Migration Guide

### From Monolithic Component

**Before:**

```jsx
import Chat from './Chat';

function App() {
  return <Chat />;
}
```

**After:**

```jsx
import Chat from './Chat';

function App() {
  return (
    <Chat>
      <Chat.Header />
      <Chat.Messages />
      <Chat.Bottom>
        <Chat.StatusBar />
        <Chat.AttachedFiles />
        <Chat.Input />
      </Chat.Bottom>
    </Chat>
  );
}
```

### Breaking Changes

- The `Chat` component now requires explicit child components
- All original functionality is preserved but must be composed explicitly
- No props have changed - all customization now happens through composition

## File Attachment Support

### Supported File Types

- Text files (`.txt`, `.md`, `.json`)
- Code files (`.js`, `.ts`, `.tsx`, `.jsx`, `.py`, `.java`, `.cpp`, `.c`, `.h`)
- Web files (`.css`, `.html`, `.xml`)
- Data files (`.yaml`, `.yml`, `.csv`)

### File Size Limits

- Maximum file size: 1MB per file
- Multiple files can be attached
- Files are automatically validated

### File Handling

```jsx
// Files are automatically processed and included in message content
// Format: [File: filename.ext]\n{content}\n
```

## State Management

All state is managed internally by the `Chat` component and shared through React Context:

- `input` - Current input text
- `attachedFiles` - Array of attached files
- `showToolsDetail` - Tools modal visibility
- `availableTools` - Combined MCP and local tools
- `isLoading` - Chat submission state
- `messages` - Message history
- `currentSession` - Active chat session

## Error Handling

The component includes comprehensive error handling:

- File validation (type and size)
- Context validation (throws helpful errors if used incorrectly)
- Submission error logging
- Graceful fallbacks for missing data

## Styling

The component uses Tailwind CSS with a terminal/console theme:

- Dark theme optimized
- Monospace font
- Terminal-style scrollbars
- Responsive design
- Accessible color contrasts

### Custom Styling

```jsx
// Override specific sections
<Chat.Input className="bg-blue-900">
  {/* Custom styled input */}
</Chat.Input>

// Add custom CSS classes
<Chat.StatusBar>
  <div className="custom-status-indicator">
    {/* Your custom status */}
  </div>
</Chat.StatusBar>
```

## Best Practices

### 1. Always Use the Container

```jsx
// ✅ Good
<Chat>
  <Chat.Messages />
  <Chat.Input />
</Chat>

// ❌ Bad - will throw error
<Chat.Messages />
```

### 2. Compose for Your Use Case

```jsx
// ✅ Good - only include what you need
<Chat>
  <Chat.Messages />
  <Chat.Input />
</Chat>

// ✅ Also good - full featured
<Chat>
  <Chat.Header />
  <Chat.Messages />
  <Chat.Bottom>
    <Chat.StatusBar />
    <Chat.AttachedFiles />
    <Chat.Input />
  </Chat.Bottom>
</Chat>
```

### 3. Extend with Children

```jsx
// ✅ Good - extend functionality
<Chat.Input>
  <CustomButton />
  <AnotherButton />
</Chat.Input>
```

### 4. Use Chat.Bottom for Layout

```jsx
// ✅ Good - logical grouping
<Chat.Bottom>
  <Chat.StatusBar />
  <Chat.AttachedFiles />
  <Chat.Input />
</Chat.Bottom>
```

## Dependencies

- React 18+
- `@paralleldrive/cuid2` - ID generation
- Custom hooks and contexts (see imports)
- Tailwind CSS for styling

## Contributing

When extending the Chat component:

1. Keep state management in the main `Chat` component
2. Use the shared context for accessing state
3. Follow the compound component pattern
4. Maintain backward compatibility
5. Add proper TypeScript types
6. Include examples in documentation

## License

[Your License Here]
