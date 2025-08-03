# 🚀 **SynapticFlow** - Lightning Fast AI Companion

## 📋 Project Overview

**SynapticFlow: A High-Freedom AI Agent Platform - Infinitely Expandable with MCP!**

SynapticFlow is a next-generation desktop AI agent platform that combines the lightness of Tauri with the intuitiveness of React. Users can automate all daily tasks by giving AI agents their own unique personalities and abilities.

## 🎯 Key Features and Characteristics

SynapticFlow is a high-freedom AI agent tool that helps users define and manage their own AI agents. In particular, it is designed to support Multi-Agent Orchestration, allowing multiple agents to cooperatively perform complex tasks. These agent and Multi-Agent configurations can be easily shared and extracted, maximizing collaboration and reusability among users.

### ✅ Implemented Features

- **🤖 Role Management System**: Create/edit/delete various AI agent roles.
- **🧠 System Prompt**: Define custom AI personalities for each role.
- **🔗 Real-time MCP Connection**: Run local MCP servers via stdio protocol **[Completed!]**
- **⚡ Tool Calling System**: Call tools from the MCP server in real-time **[Completed!]**
- **💾 IndexedDB Storage**: Store roles/conversations in a browser local database.
- **⚡ Tauri Backend**: High-performance native desktop app framework.
- **🎨 UI Components**: Modern terminal-style interface.
- **⚙️ Centralized Configuration Management**: All settings, including API keys, models, and message window sizes, are managed and permanently stored within the app **[Completed!]**
- **🤝 Agent and Multi-Agent Configuration Sharing/Extraction**: Easily export and import agent and Multi-Agent configurations.

### 🚧 In Progress

- **🔄 AI Integration**: Connect with AI models like OpenAI/Claude.

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

## 📚 Documentation

For detailed information about the project, including setup, API reference, and contribution guidelines, please see our **[full documentation here](./docs/README.md)**.

## 🚀 Quick Start

1.  **Prerequisites**: Ensure you have [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/) (v18+), and [pnpm](https://pnpm.io/) installed.
2.  **Install Dependencies**:
    ```bash
    pnpm install
    ```
3.  **Run in Development Mode**:
    ```bash
    pnpm tauri dev
    ```

For more detailed instructions, see the [Getting Started](./docs/guides/getting-started.md) guide.

## 📈 Next Steps

1. Refer to **docs/migration.md** for detailed migration plans.
2. Full MCP protocol implementation.
3. AI model integration (OpenAI/Claude/local models).
4. Add advanced UI/UX features.
5. Cross-platform testing and deployment.

## 🎨 UI/UX Features

- **Terminal Style**: Developer-friendly dark theme.
- **Responsive Design**: Supports various screen sizes.
- **Modern Interface**: Clean design based on Tailwind CSS.
- **Intuitive Operation**: Drag-and-drop, modal dialogs, etc.

## 🧪 Current Status

- ✅ **Basic Tauri App Structure**: Completed
- ✅ **React Component Migration**: Completed and Verified
- ✅ **Rust MCP Server Management**: Implemented.
- ✅ **Centralized Configuration Management**: API keys and other settings are managed and permanently stored within the app.

---
