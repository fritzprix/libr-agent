# Refactoring Overview: 2025-08-17 ~ 2025-08-18

**1. Executive Summary 📜**

This document outlines a comprehensive plan to refactor and enhance the SynapticFlow platform, focusing on improving code quality, expanding AI agent capabilities, standardizing protocols, and refining the user experience. The plan addresses several key areas: **AI Service & Data Flow** enhancements, including simplifying the `GeminiService` and introducing MCP Sampling; a major **MCP Built-in Server Overhaul** to standardize environments and evolve web-scraping tools into an Interactive Browser Agent; **Frontend State & UX** refinements to fix race conditions and clarify state management; and improvements to the **Web MCP Module Architecture** to resolve inconsistencies. Through these efforts, our goal is to evolve SynapticFlow into a more robust, scalable, and powerful platform for developing AI agents.

**2. AI Service & Data Flow Enhancements 🧠**

**_GeminiService Simplification_** aims to remove unnecessary complexity from the `GeminiService` class. The current implementation suffers from a convoluted, manually-implemented ID generation system, inconsistent logging, and unsafe JSON parsing. The solution involves replacing the complex ID logic with a simple, unique ID from a library like **`@paralleldrive/cuid2`**, consolidating logging to a single file-level logger, and implementing a `tryParse` utility for safe JSON handling.

**_Direct MCPResponse Integration_** will simplify the data flow by eliminating the `serializeToolResult` function. Currently, the data flow is overly complex (`MCPResponse` → `SerializedToolResult` → `Message`), fragments data, and limits content types. The solution is to convert `MCPResponse` directly to a `Message` object, expand the `Message.content` type to `string | MCPContent[]` to support various media, and introduce a `MessageRenderer` component to handle the display of this unified content array.

**_MCP Sampling Protocol Implementation_** adds a feature for the AI agent to request text generation directly from a language model via the MCP server, a capability the current `tool_calling`-only system lacks. This will be achieved by expanding types and interfaces in the frontend, implementing a `sample_from_mcp_server` command in the Tauri backend, and extending the Web Worker's message handler to support these new requests.

**3. MCP Built-in Server Overhaul 🛠️**

**_Working Directory Standardization_** will unify the working directory for all built-in MCP servers to enhance consistency and security. The `FilesystemServer` currently uses the CWD while the `SandboxServer` uses a temporary directory, leading to unpredictable behavior. The solution is to standardize on the system's temporary directory as the default for all servers, with an override option via the `SYNAPTICFLOW_PROJECT_ROOT` environment variable.

**_Phase 1: Foundational Tool Expansion_** will make the default Filesystem and Sandbox tools more practical. Currently, `read_file` is inefficient for large files, there is no file search capability, and the `SandboxServer` can't execute basic shell commands. We will enhance `read_file` with `start_line` and `end_line` parameters, and introduce a `search_files` tool with glob pattern support and an `execute_shell` tool for running basic commands like `ls` and `grep`.

**_Phase 2: WebView-based Web Crawler_** implements an advanced crawler for dynamic, JavaScript-heavy websites, as standard HTTP crawling fails on modern SPAs. This will be a new `WebViewCrawlerServer` that uses a hidden Tauri WebView to fully render pages. It will provide tools like `crawl_page`, `screenshot`, and `extract_data`, and will save results to a cache directory.

**_Phase 3: Interactive Browser Agent_** evolves the crawler into a fully interactive browser that the user can observe and the AI can control. Since the `WebViewCrawler` runs invisibly and cannot perform actions, we will use Tauri's multi-webview pattern to display the browser session in a side drawer. A new `InteractiveBrowserServer` will manage sessions and translate AI commands (`start_browser_session`, `click_element`, `fill_input`) into real DOM events, with support for multiple tabbed sessions.

**4. Frontend State Management & UX Refinements ✨**

**_MCP Server Context & UI State Clarity_** will improve state management and UI feedback. The current `isConnecting` state name is ambiguous and there is no global error state, leading to unclear UI. We will rename `isConnecting` to `isLoading`, add an `error?: string` state to the context, and enhance the `ChatStatusBar` to show loading spinners, error icons, and status-appropriate colors.

**_Assistant Tool Synchronization Fix_** resolves a race condition where the wrong tools are displayed after switching Assistants. This happens because the chat session starts before the new Assistant's tools have finished connecting. The solution is to convert the `handleAssistantSelect` function to `async` and `await` the `connectServers` function, ensuring a complete connection _before_ starting the chat session, while displaying a "Starting..." indicator in the UI.

**5. Web MCP Module Architecture Improvements 🏗️**

This section aims to fix structural issues in our Web Worker-based MCP modules. The current architecture suffers from inconsistent server naming (`'content-store'` vs. `'file-store'`), a cumbersome module registration process, improper dependencies between the worker and the main thread, and dead code. The solution is to enforce consistent naming, introduce a `MODULE_REGISTRY` for centralized management, refactor to use `postMessage` for communication to isolate dependencies, and remove all unused code.
