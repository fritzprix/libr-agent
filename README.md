# 🚀 **SynapticFlow** - AI Agent Platform for Everyone

## 📋 Project Overview

### SynapticFlow: Making MCP Tool Integration Accessible to All Users

SynapticFlow is a desktop AI agent platform designed to solve two critical problems in the AI ecosystem:

1. **Accessibility Gap**: MCP (Model Context Protocol) tools are powerful but primarily accessible to developers. We make these tools available to general users through an intuitive interface.

2. **LLM Provider Lock-in**: Users shouldn't be restricted to a few major LLM providers. SynapticFlow provides freedom to choose from multiple providers, including increasingly powerful local LLMs.

## 🎯 What Problems We Solve

### 🔧 **MCP Tool Integration Made Simple**

- **Problem**: MCP tools require technical setup and command-line knowledge
- **Solution**: Built-in tools and easy MCP server management with GUI
- **Benefit**: Anyone can use powerful tools without technical barriers

### 🔓 **Freedom from LLM Vendor Lock-in**

- **Problem**: Most AI platforms tie users to specific LLM providers
- **Solution**: Support for multiple providers (OpenAI, Anthropic, Groq, local models, etc.)
- **Benefit**: Choose the best model for your needs and budget

### 🤖 **Personalized AI Agents**

- **Problem**: Generic AI assistants don't fit specific workflows
- **Solution**: Create custom agents with unique personalities and tool access
- **Benefit**: AI that works exactly how you want it to

## 🛠 What We Provide

### ✅ **Comprehensive Built-in Tool Ecosystem**

SynapticFlow provides a powerful suite of built-in tools that enable AI agents to interact with web browsers, manage files, execute code, and perform advanced automation tasks. These tools are organized into three main categories:

#### 🌐 **Browser Automation Tools**

**Session Management:**

- **Create Interactive Sessions**: Launch browser windows with full control
- **Session Persistence**: Maintain state across operations
- **Multi-Session Support**: Run multiple browser instances simultaneously

**Navigation & Interaction:**

- **Smart Navigation**: Navigate to URLs, back/forward through history
- **Element Interaction**: Click buttons, input text, scroll pages
- **Content Extraction**: Convert web pages to markdown, extract structured data
- **Interactive Element Discovery**: Automatically identify clickable elements

**Advanced Features:**

- **Error Recovery**: Comprehensive failure analysis and retry mechanisms
- **Content Processing**: Clean markdown conversion from complex web pages
- **Session Monitoring**: Real-time status tracking and management

#### 🛡️ **Secure File & Content Management**

**SecureFileManager:**

- **Path Validation**: Advanced security checks prevent unauthorized access
- **Sandboxed Operations**: Isolated file system interactions
- **MIME Type Handling**: Smart processing of PDFs, DOCX, XLSX, and more

**Content Store with BM25 Search:**

- **Document Indexing**: Upload and index files with semantic search
- **Full-Text Search**: BM25 algorithm for fast, accurate keyword matching
- **Content Chunking**: Intelligent processing of large documents
- **Multi-Format Support**: Handle various document types seamlessly

**File Operations:**

- **Read/Write Files**: Full file system access with permission controls
- **Directory Management**: Browse, search, and organize file structures
- **Import/Export**: Transfer files between systems and formats

#### ⚡ **Code Execution & Development Tools**

**Python Sandbox:**

- **Secure Execution**: Isolated Python runtime environment
- **Real-time Capture**: Live output streaming and error handling
- **Package Management**: Controlled dependency access

**TypeScript/JavaScript Runtime:**

- **Modern JS Support**: ES6+ features with TypeScript compilation
- **Module System**: Full npm ecosystem integration
- **Debugging Tools**: Comprehensive error reporting and stack traces

**Shell Command Execution:**

- **System Integration**: Execute terminal commands safely
- **Output Processing**: Structured result parsing and formatting
- **Environment Control**: Managed execution contexts

#### 🔗 **Advanced MCP Integration**

**Dual Backend Architecture:**

- **Rust Backend**: Native performance for system-level operations
- **Web Worker Backend**: Browser-based tools for client-side functionality

**Planning & Task Management:**

- **Goal Setting**: Define and track complex objectives
- **Todo System**: Structured task breakdown and progress tracking
- **Observation Logging**: Context-aware decision making
- **State Persistence**: Maintain planning state across sessions

**Security & Validation:**

- **SecurityValidator**: Built-in path traversal protection
- **Input Sanitization**: Automatic cleaning of all user inputs
- **Process Isolation**: MCP servers run in secure child processes

#### 🌍 **External MCP Server Integration**

**Unlimited Tool Expansion:**

- **Connect Any MCP Server**: Integrate third-party MCP servers via stdio protocol
- **Community Ecosystem**: Access tools from the growing MCP server community
- **Custom Tool Development**: Build and connect your own specialized MCP servers
- **Real-time Communication**: Secure stdio-based communication with external servers

**Easy Server Management:**

- **GUI Configuration**: Add and manage external MCP servers through the interface
- **Server Discovery**: Automatic tool enumeration and capability detection
- **Connection Monitoring**: Real-time status tracking and error reporting
- **Security Validation**: Built-in validation for all external server connections

**Supported Server Types:**

- **File System Servers**: Enhanced file operations and management
- **Database Servers**: SQL and NoSQL database interactions
- **API Integration Servers**: REST, GraphQL, and webhook integrations
- **Specialized Tools**: Domain-specific tools for various industries
- **Custom Servers**: Your own MCP server implementations

### ✅ **Advanced Multi-LLM Ecosystem**

**8 Major Providers, 50+ Models:**

- **🤖 OpenAI**: GPT-4.1 series, o3/o4-mini reasoning models, GPT-4o variants
- **🧠 Anthropic**: Claude 4 Opus/Sonnet, Claude 3.5 series with advanced tool calling
- **🚀 Google**: Gemini 2.5 Pro/Flash (2M context), Gemini 2.0 agentic models
- **⚡ Groq**: Llama 3.3 70B, DeepSeek R1 Distill, Qwen3 32B reasoning (1,800+ tokens/sec)
- **🔥 Fireworks**: DeepSeek R1, Qwen3 235B MoE, Llama 4 Maverick/Scout
- **🧠 Cerebras**: Ultra-fast inference with industry-leading speed
- **🏠 Ollama**: Local models with zero cost (Llama, Mistral, Qwen, CodeLlama)

**Advanced Features:**

- **🤔 Reasoning Models**: o3, DeepSeek R1, Qwen3 thinking models for complex problem-solving
- **💰 Cost Optimization**: Real-time cost tracking and model comparison
- **📚 Massive Context**: Up to 2M tokens (Gemini 2.5 Pro)
- **👁️ Multimodal**: Vision, document processing, and code understanding

### ✅ **User-Friendly Features**

- **🤖 Custom Agents**: Create AI assistants with specific roles and tool access
- **👥 Multi-Agent Collaboration**: Multiple agents working together on complex tasks
- **💬 Session Management**: Organize conversations with file attachments and context
- **📤 Export/Import**: Share agent configurations and setups with others
- **🎨 Modern UI**: Clean, terminal-style interface that's both powerful and intuitive

## 🛠 Technology Stack

**Core Framework:**

- **Tauri 2.x**: Latest cross-platform framework with enhanced security and performance
- **React 18.3**: Modern UI with concurrent features and advanced hooks
- **TypeScript 5.6**: Latest language features with strict type safety
- **RMCP 0.6.4**: Rust-based Model Context Protocol with child process transport

**Backend Technologies:**

- **Rust**: High-performance native operations with async/await architecture
- **Tokio**: Advanced async runtime for concurrent MCP server management
- **SecurityValidator**: Built-in path validation and process sandboxing
- **Warp**: HTTP server infrastructure for browser automation capabilities

**Frontend Technologies:**

- **Vite**: Modern, fast build tool and dev server
- **Tailwind CSS 4.x**: Latest utility-first styling with performance optimizations
- **Radix UI**: Accessible component primitives for robust UI
- **shadcn/ui**: Component library for building accessible and customizable UI components
- **Dexie**: TypeScript-friendly IndexedDB wrapper for local data
- **Zustand**: Lightweight, scalable state management solution

## 🚀 Getting Started & Development

This guide covers how to get the SynapticFlow application running for both regular use and local development.

### Option 1: Download Release (Recommended for Users)

Visit our [Releases page](https://github.com/fritzprix/synaptic-flow/releases) to download the latest version for your operating system.

### Option 2: Build and Run from Source (for Developers)

Follow these steps to set up your local development environment.

#### 1. Prerequisites

Ensure you have the following software installed:

- [Rust](https://rustup.rs/) and Cargo
- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) package manager

#### 2. Installation

Clone the repository and install the required dependencies:

```bash
git clone https://github.com/fritzprix/synaptic-flow.git
cd synaptic-flow
pnpm install
```

#### 3. Environment Variables

API keys are managed in-app via the settings modal, so no `.env` file is required for them. However, you can configure the database path:

- `SYNAPTICFLOW_DB_PATH`: Overrides the default location for the application's SQLite database.

#### 4. Running the Application

- **Full Development Mode (Recommended)**:

  ```bash
  pnpm tauri dev
  ```

  This command starts both the Rust backend and the React frontend in a single, hot-reloading development environment.

- **Frontend-Only Mode**:

  ```bash
  pnpm dev
  ```

  This command runs only the React frontend. Note that all backend functionality (Tauri commands) will be unavailable.

#### 5. Code Quality & Testing

To ensure code quality, consistency, and stability, run the comprehensive validation pipeline:

```bash
pnpm refactor:validate
```

This single command runs all necessary checks, including:

- **Linting** (`pnpm lint`)
- **Formatting** (`pnpm format`)
- **Rust Validation** (`pnpm rust:validate`)
- **Application Build** (`pnpm build`)
- **Dead Code Analysis** (`pnpm dead-code`)

It is **mandatory** to run this command after making changes and before submitting contributions.

Individual checks can also be run:

- **Linting**: `pnpm lint` or `pnpm lint:fix`
- **Formatting**: `pnpm format` or `pnpm format:check`
- **Testing**: `pnpm test`

#### 6. Building for Production

To create an optimized, production-ready desktop application:

```bash
pnpm tauri build
```

## 📁 Project Structure

The codebase is organized into a Rust backend and a React frontend, with a focus on modularity and clear separation of concerns. The source code is now **fully documented** with Rustdoc and JSDoc comments, so feel free to explore it for more in-depth understanding.

```bash
synaptic-flow/
├── src/                        # React Frontend (Feature-Driven Architecture)
│   ├── app/                    # App entry, root layout, global providers
│   ├── assets/                 # Static assets (images, svgs)
│   ├── components/             # Shared UI components (shadcn/ui, layout, etc.)
│   ├── features/               # Feature modules (chat, assistant, settings, etc.)
│   ├── context/                # React context providers
│   ├── hooks/                  # Custom React hooks for business logic
│   ├── lib/                    # Core business logic, services, and utilities
│   │   ├── ai-service/         # LLM provider integration services
│   │   ├── db/                 # IndexedDB (Dexie) service
│   │   └── ...                 # Other core utilities
│   └── types/                  # TypeScript type definitions
├── src-tauri/                  # Rust Backend (High-Performance Core)
│   ├── src/
│   │   ├── lib.rs              # Main Tauri application library
│   │   ├── main.rs             # Application entry point
│   │   ├── mcp/                # MCP server integration modules
│   │   ├── services/           # Core backend services (browser, file manager)
│   │   ├── session/            # Session management logic
│   │   └── commands/           # Tauri command definitions
│   ├── Cargo.toml              # Rust dependencies
│   └── tauri.conf.json         # Tauri 2.x configuration
├── docs/                       # Documentation and guides
├── package.json                # Node.js dependencies and scripts
└── ...                         # Configuration files (vite, tailwind, etc.)
```

## 🛡️ Security & Performance

**Built-in Security:**

- **SecurityValidator**: Advanced path traversal protection and sandboxed operations
- **MIME Type Validation**: Safe file handling across all supported formats
- **Process Isolation**: MCP servers run in isolated child processes for maximum security
- **API Key Management**: Secure in-app credential storage with encryption
- **Content Sanitization**: Automatic cleaning and validation of all user inputs

**Performance Optimizations:**

- **Streaming Responses**: Real-time AI model outputs with minimal latency
- **Concurrent Tool Execution**: Parallel MCP server operations for faster results
- **Smart Caching**: Intelligent resource caching for improved response times
- **Memory Management**: Optimized for long-running sessions and large datasets
- **Ultra-Fast Models**: Cerebras integration delivering 1,800+ tokens/second

## 🖥️ Supported Platforms

SynapticFlow is a **cross-platform desktop application** that runs natively on:

### Windows

- **Version**: Windows 10 and later
- **Architecture**: x64
- **Installation**: MSI installer (`SynapticFlow_x64_en-US.msi`)

### macOS

- **Version**: macOS 10.15 (Catalina) and later
- **Architecture**: Intel and Apple Silicon (universal binary)
- **Installation**: Application bundle (`.app.tar.gz`)

### Linux

- **Distributions**: Ubuntu, Debian, Fedora, Arch Linux, and others
- **Architecture**: x64
- **Installation**: `.deb` package or a universal AppImage.

## 📚 Documentation

- **[Built-in Tools Documentation](docs/builtin-tools.md)**: Comprehensive guide to all available tools including browser automation, file management, code execution, and MCP integration
- **[External MCP Integration](docs/external-mcp-integration.md)**: Detailed guide on connecting and managing external MCP servers
- **[Architecture Documentation](docs/architecture/)**: Technical details about system design and implementation
- **[Contributing Guide](CONTRIBUTING.md)**: Guidelines for contributors and development setup

## 🤝 Contributing

We welcome contributions! Here's how you can help:

- **🐛 Report Issues**: Found a bug? [Open an issue](https://github.com/fritzprix/synaptic-flow/issues)
- **💡 Suggest Features**: Have ideas? Share them in our discussions
- **🔧 Submit Code**: Read our [Contributing Guide](CONTRIBUTING.md) to get started
- **📚 Improve Docs**: Help make our documentation even better

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Ready to experience the most advanced AI agent platform? SynapticFlow combines enterprise-grade security, lightning-fast performance, and unlimited LLM freedom in one powerful desktop application!** 🚀

[Download SynapticFlow](https://github.com/fritzprix/synaptic-flow/releases) | [View Source](https://github.com/fritzprix/synaptic-flow) | [Join Community](https://github.com/fritzprix/synaptic-flow/discussions)
