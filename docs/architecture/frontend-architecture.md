# Frontend Architecture

Guidelines for React/TypeScript code in `src/`.

## Coding Style

- **camelCase** for variables and functions
- **PascalCase** for React components and TypeScript interfaces
- Prefer functional components with hooks over class components
- Use TypeScript interfaces for type definitions
- **2 spaces** indentation across all files
- **All comments must be written in English**

## Component Patterns

- Feature components follow compound patterns: `Chat.Header`, `Chat.Messages`, `Chat.Input`
- Each feature directory contains `components/`, `hooks/`, and `README.md`
- Use React Context for cross-component state sharing, not prop drilling
- Agent V2 uses `AgentSessionContext` + `AgentChatContext` for reactive state management

### Component Structure

```typescript
// src/components/ComponentName.tsx
interface ComponentNameProps {
  // Type definitions
}

export default function ComponentName({ props }: ComponentNameProps) {
  // Component implementation
}
```

## Service Layer

`src/lib/backend/` contains Tauri command wrappers with centralized `safeInvoke()` utility.

```typescript
// src/lib/backend/module-name.ts
import { safeInvoke } from './core';

export async function someBackendOperation(param: Type): Promise<Result> {
  return safeInvoke<Result>('rust_command_name', { param });
}
```

- Typed wrappers around Tauri commands using `safeInvoke()`
- Centralized error handling and logging
- Type-safe API contracts between frontend and backend
- Organized by domain (assistants, browser, mcp-server, workspace, etc.)

## Logging System

Use the centralized logger instead of `console.*` methods.

```typescript
import { getLogger } from '@/lib/logger';
const logger = getLogger('ComponentName');

logger.debug('Debug information', data);
logger.info('General information', data);
logger.warn('Warning message', data);
logger.error('Error occurred', error);
```

- **Context naming**: Use descriptive context names that match the component/module name
- **Log levels**: Use appropriate log levels (debug, info, warn, error) based on importance
- **Error logging**: Pass the Error object as the last parameter for proper error handling

## CSP Warning — CRITICAL

**DO NOT add CSP configuration to `tauri.conf.json` for desktop applications.**

- CSP is designed for web browsers, not desktop environments
- Tauri desktop apps require unrestricted access for native operations
- Adding CSP will cause blank white screens in release builds
- Dev mode has relaxed CSP enforcement, masking production issues
- Use Tauri's native security features (allowlist, capability system) instead

## Tailwind CSS

- Use `shadcn/ui` components as the primary building blocks for UI
- Avoid arbitrary class names (e.g., `content-text`) that are not Tailwind utilities — they may be removed by PurgeCSS
- Use Tailwind utility classes: `className="text-sm text-gray-700 leading-relaxed"`
- For dynamic/conditional styling, use arbitrary value syntax: `className="[custom-value]"`

## No Inline Import Types

**❌ Bad:**

```typescript
interface Config {
  tools?: import('@/lib/mcp').MCPTool[];
}
```

**✅ Good:**

```typescript
import type { MCPTool } from '@/lib/mcp';

interface Config {
  tools?: MCPTool[];
}
```
