# Getting Started with SynapticFlow

This guide will walk you through setting up your development environment and connecting to your first MCP server.

## 1. Environment Setup

### Prerequisites

- **Rust**: Install from [rustup.rs](https://rustup.rs/).
- **Node.js**: Version 18+.
- **pnpm**: Install with `npm install -g pnpm`.

### Installation

1. **Clone the repository**:

   ```bash
   git clone https://github.com/your-username/synaptic-flow.git
   cd synaptic-flow
   ```

2. **Install dependencies**:

   ```bash
   pnpm install
   ```

3. **API Keys**:

   API keys are managed within the application's settings modal. You do not need a `.env` file for development.

## 2. Running the Application

Start the development server:

```bash
pnpm tauri dev
```

This will open the SynapticFlow desktop application.

## 3. Connecting to Your First MCP Server

1. **Open the Settings Modal**: Click the gear icon in the application to open the settings.
2. **Go to MCP Servers**: Navigate to the MCP Servers section.
3. **Add a New Server**: Click "Add Server" and enter the following configuration for a simple filesystem server:
   - **Name**: `filesystem`
   - **Command**: `npx`
   - **Arguments**: `-y @modelcontextprotocol/server-filesystem /tmp`

4. **Save the Configuration**: Click "Save".
5. **Start the Server**: The application will automatically attempt to start the server. You can check its status in the UI.

## 4. Verifying the Connection

Once the server is running, you can interact with its tools:

1. **Start a new chat**.
2. **Type a prompt** that uses the filesystem tools, for example:

   > "List the files in the /tmp directory."

3. The assistant should be able to call the `listFiles` tool from the `filesystem` server and show you the output.

Congratulations! You have successfully set up SynapticFlow and connected to your first MCP server.
