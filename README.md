# 🚀 **SynapticFlow** - AI Agent Platform for Everyone

## 📋 Project Overview

**SynapticFlow: Making MCP Tool Integration Accessible to All Users**

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

### ✅ **Built-in Tools Ready to Use**

- **🗂️ File Management**: Read, write, and organize files safely with built-in filesystem tools
- **⚡ Code Execution**: Run Python and TypeScript code in secure sandboxed environments
- **📁 Content Store**: Upload, manage, and reference files in your conversations
- **🔗 MCP Integration**: Connect external MCP servers for extended functionality

### ✅ **Multi-LLM Support**

- **🌐 Cloud Providers**: OpenAI, Anthropic Claude, Groq, Fireworks, Cerebras, Google Gemini
- **🏠 Local Models**: Ollama and other local LLM solutions
- **🔄 Easy Switching**: Change providers and models without losing your workflow
- **⚙️ Centralized Config**: Manage all API keys and settings in one place

### ✅ **User-Friendly Features**

- **🤖 Custom Agents**: Create AI assistants with specific roles and tool access
- **👥 Multi-Agent Collaboration**: Multiple agents working together on complex tasks
- **💬 Session Management**: Organize conversations with file attachments and context
- **📤 Export/Import**: Share agent configurations and setups with others
- **🎨 Modern UI**: Clean, terminal-style interface that's both powerful and intuitive

## 🛠 Technology Stack

- **Tauri**: High-performance cross-platform desktop app framework (Rust + WebView).
- **React 18**: Modern UI library.
- **TypeScript**: Type safety and developer experience.
- **RMCP**: Rust-based Model Context Protocol client.
- **Tailwind CSS**: Utility-first CSS framework.
- **IndexedDB**: Browser-based NoSQL database.
- **Vite**: Fast development server and build tool.

## 📁 Project Structure

```bash
synaptic-flow/
├── src/                        # React Frontend
│   ├── app/                    # App entry, root layout, global providers
│   │   ├── App.tsx
│   │   ├── main.tsx
│   │   ├── App.css
│   │   └── globals.css
│   ├── assets/                 # Static assets (images, svgs, etc.)
│   │   └── react.svg
│   ├── components/             # Shared, generic UI components (reusable)
│   │   ├── ui/
│   │   │   └── ...shadcn/ui components...
│   │   ├── layout/
│   │   │   ├── AppHeader.tsx
│   │   │   └── AppSidebar.tsx
│   │   │   └── ...
│   │   └── common/
│   │       └── ThemeToggle.tsx
│   │       └── ...
│   ├── features/               # Feature-specific components, logic, and hooks
│   │   ├── chat/
│   │   │   ├── Chat.tsx
│   │   │   ├── ChatContainer.tsx
│   │   │   ├── MessageBubble.tsx
│   │   │   ├── useChat.ts
│   │   │   └── ...
│   │   ├── group/
│   │   │   ├── Group.tsx
│   │   │   ├── GroupCreationModal.tsx
│   │   │   └── ...
│   │   ├── history/
│   │   │   ├── History.tsx
│   │   │   └── ...
│   │   ├── assistant/
│   │   │   ├── AssistantDetailList.tsx
│   │   │   ├── AssistantEditor.tsx
│   │   │   └── ...
│   │   ├── session/
│   │   │   ├── SessionList.tsx
│   │   │   ├── SessionItem.tsx
│   │   │   └── ...
│   │   ├── settings/
│   │   │   ├── SettingsModal.tsx
│   │   │   └── ...
│   │   └── tools/
│   │       ├── ToolsModal.tsx
│   │       ├── WeatherTool.tsx
│   │       └── ...
│   ├── config/                 # Static config files
│   │   └── llm-config.json
│   ├── context/                # React context providers
│   │   └── AssistantContext.tsx
│   │   └── ...
│   ├── hooks/                  # Generic, reusable hooks
│   │   ├── useAiService.ts
│   │   ├── useMcpServer.ts
│   │   └── ...
│   ├── lib/                    # Service layer, business logic, data, API
│   │   ├── aiService.ts
│   │   ├── db.ts
│   │   ├── llmConfigManager.ts
│   │   ├── logger.ts
│   │   ├── tauriMcpClient.ts
│   │   └── utils.ts
│   ├── models/                 # TypeScript types and interfaces
│   │   ├── chat.ts
│   │   └── llmConfig.ts
│   ├── styles/                 # Global or shared CSS
│   │   └── tailwind.css
│   ├── README.md
│   └── vite-env.d.ts
├── src-tauri/                 # Rust Backend
│   ├── src/
│   │   ├── lib.rs             # Tauri commands definition
│   │   └── mcp.rs             # MCP server management logic
│   ├── Cargo.toml             # Rust dependencies
│   └── tauri.conf.json        # Tauri configuration
├── docs/                      # Documentation
│   └── migration.md           # Detailed migration plan
├── dist/                      # Build artifacts
├── package.json               # Node.js dependencies
├── tailwind.config.js         # Tailwind CSS configuration
└── vite.config.ts             # Vite configuration
```

## 🚀 Getting Started

Ready to use SynapticFlow? Here's how to get up and running:

### Option 1: Download Release (Recommended)

Visit our [Releases](https://github.com/SynapticFlow/SynapticFlow/releases) page to download the latest version for your operating system.

### Option 2: Build from Source

1. **Prerequisites**: Ensure you have [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) (v18+), and [pnpm](https://pnpm.io/) installed.

2. **Install Dependencies**:

   ```bash
   pnpm install
   ```

3. **Run in Development Mode**:

   ```bash
   pnpm tauri dev
   ```

### Next Steps

1. **Configure Your First LLM**: Open Settings and add your preferred AI provider's API key
2. **Create an Agent**: Set up your first AI assistant with specific tools and personality
3. **Connect MCP Tools**: Add external MCP servers or use our built-in tools
4. **Start Collaborating**: Begin conversations with your AI agents

## 📚 Documentation

- **📖 [User Guide](docs/guides/getting-started.md)**: Complete setup and usage instructions
- **🏗️ [Architecture](docs/app-architecture.md)**: Technical details for developers
- **🔧 [MCP Integration](docs/mcp.md)**: How to connect and use MCP servers
- **❓ [Troubleshooting](docs/guides/troubleshooting.md)**: Common issues and solutions

## 🤝 Contributing

We welcome contributions! Here's how you can help:

- **🐛 Report Issues**: Found a bug? [Open an issue](https://github.com/SynapticFlow/SynapticFlow/issues)
- **💡 Suggest Features**: Have ideas? Share them in our discussions
- **🔧 Submit Code**: Read our [Contributing Guide](CONTRIBUTING.md) to get started
- **📚 Improve Docs**: Help make our documentation even better

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🌟 Support

If SynapticFlow helps you work more efficiently with AI tools, consider:

- ⭐ **Star this repository** to show your support
- 🗣️ **Share** with others who might find it useful
- 🐛 **Report issues** to help us improve
- 💬 **Join discussions** to shape the future of the project

---

**Ready to revolutionize your AI workflow? [Download SynapticFlow](https://github.com/SynapticFlow/SynapticFlow/releases) and start building with unlimited AI possibilities!** 🚀
