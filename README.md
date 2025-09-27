# 🚀 **SynapticFlow** - AI Agent Platform for Everyone

## 📋 Project Overview

**SynapticFlow: Making MCP Tool Integration Accessible to All Users**

SynapticFlow is a desktop AI agent platform designed to solve two critical problems in the AI ecosystem:

1.  **Accessibility Gap**: MCP (Model Context Protocol) tools are powerful but primarily accessible to developers. We make these tools available to general users through an intuitive interface.

2.  **LLM Provider Lock-in**: Users shouldn't be restricted to a few major LLM providers. SynapticFlow provides freedom to choose from multiple providers, including increasingly powerful local LLMs.

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

**🛡️ Secure File Management:**

- **SecureFileManager**: Advanced path validation and sandboxed operations
- **Content Store**: Upload, index, and full-text search across PDF, DOCX, XLSX files
- **File Attachments**: Smart MIME type handling with preview capabilities
- **Document Processing**: Extract and analyze content from multiple formats

**⚡ Code Execution & Development:**

- **Python Sandbox**: Secure code execution with real-time result capture
- **TypeScript Runtime**: JavaScript/TypeScript evaluation environment
- **Output Management**: Comprehensive execution logging and error handling
- **Development Tools**: Built-in debugging and testing utilities

**🌐 Browser Automation:**

- **Interactive Browser Server**: Automated web interactions and scraping
- **Session Management**: Persistent browser sessions with state management
- **Content Extraction**: Clean markdown conversion from web pages
- **Web Integration**: Seamless web data processing pipeline

**🔗 Advanced MCP Integration:**

- **Dual Backend Support**: Both Rust Tauri and Web Worker implementations
- **Security Validation**: Built-in SecurityValidator with comprehensive protection
- **Tool Execution Context**: Unified calling interface across all backends
- **Error Normalization**: Robust error handling and reporting system

### ✅ **Advanced Multi-LLM Ecosystem**

**8 Major Providers, 50+ Models:**

- **🤖 OpenAI**: GPT-4.1 series, o3/o4-mini reasoning models, GPT-4o variants
- **🧠 Anthropic**: Claude 4 Opus/Sonnet, Claude 3.5 series with advanced tool calling
- **🚀 Google**: Gemini 2.5 Pro/Flash (2M context), Gemini 2.0 agentic models
- **⚡ Groq**: Llama 3.3 70B, DeepSeek R1 Distill, Qwen3 32B reasoning (1,800+ tokens/sec)
- **🔥 Fireworks**: DeepSeek R1, Qwen3 235B MoE, Llama 4 Maverick/Scout
- **🧠 Cerebras**: Ultra-fast inference with industry-leading speed
- **🏠 Ollama**: Local models with zero cost (Llama, Mistral, Qwen, CodeLlama)
- **🎯 Empty**: Custom provider configurations

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

## 🛠 Advanced Technology Stack

**Core Framework:**

- **Tauri 2.x**: Latest cross-platform framework with enhanced security and performance
- **React 18.3**: Modern UI with concurrent features and advanced hooks
- **TypeScript 5.6**: Latest language features with strict type safety
- **RMCP 0.2.1**: Rust-based Model Context Protocol with child process transport

**Backend Technologies:**

- **Rust**: High-performance native operations with async/await architecture
- **Tokio**: Advanced async runtime for concurrent MCP server management
- **SecurityValidator**: Built-in path validation and process sandboxing
- **Warp**: HTTP server infrastructure for browser automation capabilities

**Frontend Technologies:**

- **Tailwind CSS 4.x**: Latest utility-first styling with performance optimizations
- **Radix UI**: Accessible component primitives for robust UI
- **Dexie**: TypeScript-friendly IndexedDB wrapper for local data
- **Zustand**: Lightweight, scalable state management solution

## 🚀 Getting Started & Development

This guide covers how to get the SynapticFlow application running for both regular use and local development.

### Option 1: Download Release (Recommended for Users)

Visit our [Releases page](https://github.com/SynapticFlow/SynapticFlow/releases) to download the latest version for your operating system.

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
git clone https://github.com/SynapticFlow/SynapticFlow.git
cd SynapticFlow
pnpm install
```

#### 3. Environment Variables

For some features, particularly those involving cloud-based LLM providers, you will need to configure API keys. Create a `.env` file in the root of the project to store your keys.

Additionally, you can configure the database path:

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

- **Linting**:
  ```bash
  pnpm lint      # Check for code quality issues
  pnpm lint:fix  # Automatically fix lint issues
  ```
- **Formatting**:
  ```bash
  pnpm format         # Format all files with Prettier
  pnpm format:check   # Check for formatting compliance
  ```
- **Testing**:
  ```bash
  pnpm test      # Run the test suite
  ```

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

## 🤝 Contributing

We welcome contributions! Here's how you can help:

- **🐛 Report Issues**: Found a bug? [Open an issue](https://github.com/SynapticFlow/SynapticFlow/issues)
- **💡 Suggest Features**: Have ideas? Share them in our discussions
- **🔧 Submit Code**: Read our [Contributing Guide](CONTRIBUTING.md) to get started
- **📚 Improve Docs**: Help make our documentation even better

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**Ready to experience the most advanced AI agent platform? SynapticFlow combines enterprise-grade security, lightning-fast performance, and unlimited LLM freedom in one powerful desktop application!** 🚀

[Download SynapticFlow](https://github.com/SynapticFlow/SynapticFlow/releases) | [View Source](https://github.com/SynapticFlow/SynapticFlow) | [Join Community](https://github.com/SynapticFlow/SynapticFlow/discussions)
