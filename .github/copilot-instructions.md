# 🚀 SynapticFlow Project Guidelines

## Project Overview

**SynapticFlow: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

SynapticFlow is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

## Key Features

- Role Management System: Create/edit/delete various AI agent roles.
- System Prompt: Define custom AI personalities for each role.
- Real-time MCP Connection: Run local MCP servers via stdio protocol.
- Tool Calling System: Call tools from the MCP server in real-time.
- IndexedDB Storage: Store roles/conversations in a browser local database.
- Tauri Backend: High-performance native desktop app framework.
- UI Components: Modern terminal-style interface.
- Centralized Configuration Management: All settings, including API keys, models, and message window sizes, are managed and permanently stored within the app.
- Agent and Multi-Agent Configuration Sharing/Extraction: Easily export and import agent and Multi-Agent configurations.

## Technology Stack

- PNPM
- Tauri (Rust + WebView)
- React 18
- TypeScript
- RMCP (Rust-based Model Context Protocol client)
- Tailwind CSS
- IndexedDB
- Vite

## File Structure

```bash
synaptic-flow/
├── src/                        # React Frontend
│   ├── app/                    # App entry, root layout, global providers
│   ├── assets/                 # Static assets (images, svgs, etc.)
│   ├── components/             # Shared, generic UI components (reusable)
│   ├── features/               # Feature-specific components, logic, and hooks
│   ├── config/                 # Static config files
│   ├── context/                # React context providers
│   ├── hooks/                  # Generic, reusable hooks
│   ├── lib/                    # Service layer, business logic, data, API
│   ├── models/                 # TypeScript types and interfaces
│   ├── styles/                 # Global or shared CSS
│   ├── README.md
│   └── vite-env.d.ts
├── src-tauri/                 # Rust Backend
│   ├── src/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/                      # Documentation
├── dist/                      # Build artifacts
├── package.json
├── tailwind.config.js
└── vite.config.ts
```

## Quick Start

1. Install Rust ([rustup.rs](https://rustup.rs/)), Node.js (v18+), and pnpm (`npm install -g pnpm`).
2. Run `pnpm install` to install dependencies.
3. Start development: `pnpm tauri dev`
4. Build for production: `pnpm tauri build`
5. API keys are managed in-app via the settings modal (not in .env files).
# SynapticFlow Project Guidelines

## Coding Style

### General

- Use 2 spaces for indentation across all files.
- Use descriptive variable names in both Rust and TypeScript.
- Follow consistent naming conventions for files and directories.

### Rust Backend (`src-tauri/`)

- Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/) and use `rustfmt`.
- Use snake_case for functions, variables, and module names.
- Use PascalCase for types, structs, and enums.
- Add comprehensive documentation comments (`///`) for public APIs.
- Handle errors explicitly using `Result<T, E>` types.

### Frontend (`src/`)

- Follow Prettier and ESLint configurations for TypeScript/React code.
- Use camelCase for variables and functions.
- Use PascalCase for React components and TypeScript interfaces.
- Prefer functional components with hooks over class components.
- Use TypeScript interfaces for type definitions.
- **Do not use `any` in TypeScript.** The lint configuration is extremely strict; always use precise types and interfaces. Use unknown or generics if absolutely necessary, but avoid `any` as much as possible.

### CSS/Styling

- Use `shadcn/ui` components for building accessible, consistent, and customizable UI elements. Prefer shadcn/ui for new UI components unless a custom solution is required.

## Architecture

- `shadcn/ui`: Component library for building accessible and customizable UI components

### Layer Responsibilities

- Use `shadcn/ui` components as the primary building blocks for UI, customizing as needed for project requirements.
- Manages local UI state and user input validation.
- Communicates with Tauri backend through the service layer.

#### Service Layer (`src/lib/`)

- Business logic and data transformation.
- Tauri command invocations and API integrations.
- IndexedDB operations and local data management.
- MCP client communication protocols.

#### Backend Layer (`src-tauri/src/`)

- Native system operations and file I/O.
- MCP server process management and stdio communication.
- Cross-platform compatibility handling.
- Security and permission management.

### Data Flow

1. User interaction in React components
2. Service layer processes requests and calls Tauri commands
3. Rust backend executes native operations or MCP communications
4. Results flow back through the same layers
5. UI updates reflect the changes

## Dependencies

### Core Framework

- `@tauri-apps/api`: Version 1.x - Frontend-backend communication
- `@tauri-apps/cli`: Version 1.x - Development and build tools
- `tauri`: Version 1.x - Rust backend framework

### Frontend Dependencies

- `react`: Version 18.x - UI library
- `react-dom`: Version 18.x - React DOM renderer
- `typescript`: Version 5.x - Type safety
- `vite`: Version 4.x - Build tool and dev server
- `tailwindcss`: Version 3.x - Utility-first CSS framework

### Backend Dependencies (Rust)

- `tauri`: Main framework for desktop app development
- `serde`: JSON serialization/deserialization
- `tokio`: Async runtime for concurrent operations
- `rmcp`: Model Context Protocol implementation

### Development Dependencies

- `@vitejs/plugin-react`: React support for Vite
- `autoprefixer`: CSS vendor prefixing
- `postcss`: CSS processing
- `eslint`: JavaScript/TypeScript linting
- `prettier`: Code formatting

## File Organization

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

### Service Layer Structure

```typescript
// src/lib/service-name.ts
export class ServiceName {
  // Public methods for component usage
}

export const serviceInstance = new ServiceName();
```

### Tauri Command Structure

```rust
// src-tauri/src/commands/module_name.rs
#[tauri::command]
pub async fn command_name(param: Type) -> Result<ReturnType, String> {
    // Implementation
}
```

## Development Workflow

### Environment Setup

1. Install Rust via rustup.rs
2. Install Node.js (v18+) and pnpm
3. Copy `.env.example` to `.env` and configure API keys
4. Run `pnpm install` for dependencies

### Development Commands

- `pnpm tauri dev` - Start development server
- `pnpm tauri build` - Create production build
- `pnpm lint` - Run ESLint checks
- `pnpm format` - Format code with Prettier
- `cargo fmt` - Format Rust code
- `cargo clippy` - Rust linting

### Testing Guidelines

- Write unit tests for utility functions
- Test Tauri commands with mock data
- Verify cross-platform compatibility
- Test MCP server integration scenarios

## Security Considerations

### Tauri Security

- Use allowlist configuration to restrict API access
- Validate all input from frontend to backend
- Sanitize data before MCP server communication
- Handle sensitive data (API keys) securely

### API Key Management

- Store API keys in environment variables
- Never commit API keys to version control
- Use secure storage for production deployments
- Implement key rotation strategies

## Performance Guidelines

### Frontend Optimization

- Use React.memo for expensive components
- Implement proper dependency arrays in useEffect
- Lazy load components when appropriate
- Optimize IndexedDB queries

### Backend Optimization

- Use async/await for non-blocking operations
- Implement proper error handling to prevent crashes
- Cache frequently accessed data
- Optimize MCP server communication protocols

## Documentation Standards

### Code Documentation

- Document all public APIs with clear examples
- Include type information in TypeScript interfaces
- Add inline comments for complex business logic
- Maintain up-to-date README files

### Architecture Documentation

- Document component relationships and data flow
- Maintain API documentation for Tauri commands
- Document MCP integration patterns
- Keep deployment guides current
