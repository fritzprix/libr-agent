# Modular Chat Component

A flexible, composable chat interface built with React using a compound component pattern. The Chat component has been refactored from a monolithic 940-line component into a modular architecture with separate components, hooks, and clear separation of concerns.

## Overview

The Chat component has been refactored from a single large file into:

- **Main Container**: `Chat.tsx` (50 lines) - provides context and layout structure
- **Components**: Individual UI components in `components/` directory
- **Hooks**: Custom hooks in `hooks/` directory for state and file management
- **Compound Pattern**: Static properties on Chat for accessing subcomponents

This approach provides maximum flexibility while maintaining all the original functionality including drag-and-drop file attachments, tool integration, and message management.

## Features

- 🧩 **Modular Design**: Compose your chat interface from individual components
- 🔄 **Shared State**: All components automatically share state through React Context (`ChatProvider`)
- 📁 **File Attachments**: Drag-and-drop and button-based file uploads with debouncing
- 🔧 **Tool Integration**: Built-in support for MCP servers and browser tools
- 💬 **Message History**: Automatic message management with proper scrolling
- 🎨 **Customizable**: Easy to extend and customize each section
- 📱 **Responsive**: Works across different screen sizes
- ⌨️ **Keyboard Shortcuts**: Submit with Enter, file shortcuts
- 🪝 **Custom Hooks**: `useChatState` and `useFileAttachment` for state management

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

## Architecture

### Directory Structure

```
src/features/chat/
├── Chat.tsx                 # Main container component (50 lines)
├── components/              # UI Components
│   ├── ChatHeader.tsx      # Terminal-style header with session files
│   ├── ChatMessages.tsx    # Message history display with auto-scroll
│   ├── ChatStatusBar.tsx   # Model picker and tools status
│   ├── ChatAttachedFiles.tsx # File attachment display
│   ├── ChatInput.tsx       # Input form with file support
│   ├── ChatBottom.tsx      # Container for bottom elements
│   └── SessionFilesPopover.tsx # Session file browser modal
├── hooks/                  # Custom Hooks
│   ├── useChatState.ts     # Tools modal state management
│   └── useFileAttachment.ts # File drag-drop and attachment logic
└── README.md               # This documentation
```

### Component Responsibilities

#### Chat.tsx (Main Container)

- Provides `ChatProvider` context to all children
- Manages `ToolsModal` visibility with `useChatState` hook
- Validates session existence
- Defines compound component pattern via static properties

#### Components Directory

- **ChatHeader**: Session title, terminal styling, session files access
- **ChatMessages**: Message display, auto-scroll, loading states
- **ChatStatusBar**: Model selection, tool count display, extensible status
- **ChatAttachedFiles**: File attachment preview and removal
- **ChatInput**: Text input, file attachment, form submission
- **ChatBottom**: Layout container for bottom UI elements
- **SessionFilesPopover**: Modal for browsing session file attachments

#### Hooks Directory

- **useChatState**: Simple state management for tools modal visibility
- **useFileAttachment**: Complex file handling including drag-drop events, validation, Rust backend integration, and pending file management

## File Attachment Support

### Supported File Types

- Text files (`.txt`, `.md`, `.json`)
- Document files (`.pdf`, `.docx`, `.xlsx`)

### Features

- **Drag & Drop**: Drop files directly into the chat interface
- **Button Upload**: Traditional file picker button
- **Debouncing**: Prevents duplicate processing from rapid events (10ms timeout)
- **Validation**: Automatic file type and format checking
- **Preview**: File content preview in attachment bubbles
- **Duplicate Prevention**: Single hook instance prevents double processing

### File Processing Pipeline

1. File dropped/selected → `useFileAttachment` hook
2. Validation (type, size) → Error handling if invalid
3. Rust backend processing → `rustBackend.readDroppedFile()`
4. Blob URL creation → Added to pending files via `ResourceAttachmentContext`
5. UI display → `ChatAttachedFiles` component
6. Message submission → Included as `AttachmentReference[]`

## State Management

State is managed through a combination of React Context and custom hooks:

### ChatContext (Global State)

- `messages` - Message history array
- `isLoading` - Chat submission state
- `submit()` - Function to send messages
- `cancel()` - Function to cancel current request

### Custom Hooks (Local State)

- **useChatState**: `showToolsDetail` - Tools modal visibility
- **useFileAttachment**: File handling state including `pendingFiles`, `dragState`, attachment functions

### Context Dependencies

- `SessionContext` - Current session and validation
- `AssistantContext` - Current assistant configuration
- `ResourceAttachmentContext` - File attachment management

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

### Core Dependencies

- React 18+ with hooks (useState, useCallback, useEffect, useRef)
- `@paralleldrive/cuid2` - Message ID generation

### Tauri Integration

- `@tauri-apps/api/webview` - Drag-and-drop event handling
- Custom Rust backend client for file processing

### Internal Dependencies

- `@/context/ChatContext` - Chat state management
- `@/context/SessionContext` - Session validation
- `@/context/ResourceAttachmentContext` - File attachment state
- `@/components/ui/*` - Shared UI components
- Tailwind CSS for styling

### File Processing

- Rust backend for secure file reading
- Blob URL management for file previews
- MIME type detection and validation

## Recent Improvements

### Drag & Drop Bug Fixes (Phase 3 Completion)

- **Fixed infinite re-rendering** in `useFileAttachment` hook by using empty dependency array `[]`
- **Eliminated duplicate file processing** by removing dual hook instantiation in `ChatInput`
- **Added proper debouncing** with 10ms timeout to prevent rapid-fire events
- **Maintained type safety** with proper TypeScript interfaces for Tauri events

### Refactoring Highlights

- **Reduced main component size** from 940 lines to 50 lines
- **Improved maintainability** with clear separation of concerns
- **Enhanced testability** with isolated hooks and components
- **Preserved all functionality** including file attachments and tool integration

## Contributing

When extending the Chat component:

1. Keep state management in React Context or custom hooks
2. Use the compound component pattern for new UI elements
3. Follow the established directory structure (`components/`, `hooks/`)
4. Maintain backward compatibility
5. Add proper TypeScript types and interfaces
6. Include comprehensive error handling
7. Test drag-and-drop functionality thoroughly
8. Update this README with new features

## Troubleshooting

### Common Issues

1. **"Chat component should only be rendered when currentSession exists"**
   - Ensure `SessionContext` is properly initialized before rendering Chat

2. **Drag and drop not working**
   - Check that `useFileAttachment` is only called once per component
   - Verify Tauri permissions for file access

3. **Duplicate file processing**
   - Ensure single instance of `useFileAttachment` hook per component
   - Check for proper debouncing in drag handlers

4. **Files not appearing in UI**
   - Verify `ResourceAttachmentContext` is properly connected
   - Check that `addPendingFiles` is being called successfully

## License

[Your License Here]
