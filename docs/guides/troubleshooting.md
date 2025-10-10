# Troubleshooting Guide

This guide provides solutions to common problems you might encounter while developing or using SynapticFlow.

## Linux Environment Issues

### WebKit Crashes or Fails to Load

**Symptom**: The application window is blank, or you see errors related to `webkit2gtk` in the console.

**Cause**: This is often due to missing or incompatible libraries for WebKit, especially in headless or containerized environments.

**Solution**:

1. **Install Required Packages**: On Debian/Ubuntu, install the necessary development libraries:

   ```bash
   sudo apt-get update
   sudo apt-get install -y libwebkit2gtk-4.1-dev
   ```

2. **Set Environment Variables**: The application attempts to set these, but you can also set them manually before running `pnpm tauri dev`:

   ```bash
   export WEBKIT_DISABLE_COMPOSITING_MODE=1
   export WEBKIT_DISABLE_DMABUF_RENDERER=1
   ```

**Source Reference**: [`src-tauri/src/lib.rs`](../src-tauri/src/lib.rs) (Lines 188-250)

---

## MCP Server Issues

### MCP Server Connection Failure

**Symptom**: The application reports that it cannot connect to an MCP server.

**Cause**: The server process failed to start, or the configuration is incorrect.

**Solution**:

1. **Verify Server Configuration**: Double-check the server settings in the UI. Ensure the `command` and `args` are correct and that the command is available in your system's `PATH`.
2. **Check for Port Conflicts**: If using an HTTP or WebSocket server, ensure the specified port is not already in use by another application.
3. **Inspect Logs**: Look at the application logs for error messages related to the server process.

**Source Reference**: [`src-tauri/src/mcp.rs`](../src-tauri/src/mcp.rs) (Lines 200-250)

---

## Tool-Related Problems

### Tool Call Fails

**Symptom**: An assistant attempts to use a tool, but it results in an error.

**Cause**: The tool's input schema might not match the arguments provided by the AI, or the tool itself encountered an error during execution.

**Solution**:

1. **Validate Tool Schema**: Use the `validate_tool_schema` command or check the tool's definition to ensure the schema is correct and robust.
2. **Check AI-Generated Arguments**: Inspect the `tool_calls` object in the message to see what arguments the AI generated. This can help identify if the AI is consistently providing malformed input.
3. **Examine Tool Output**: Look at the `ToolOutputBubble` for any error messages returned by the tool itself.

**Source Reference**: [`src/features/chat/orchestrators/ToolCaller.tsx`](../src/features/chat/orchestrators/ToolCaller.tsx) (Lines 25-45)

---

### Type Mismatches

**Symptom**: You encounter runtime errors in the frontend related to type inconsistencies, especially when handling data from the Tauri backend.

**Cause**: The TypeScript types in the frontend (`src/models/`) do not perfectly match the Rust structs in the backend (`src-tauri/src/mcp.rs`).

**Solution**:

1. **Compare Definitions**: Carefully compare the field names and types between the Rust struct and the corresponding TypeScript interface.
2. **Refer to `types.md`**: Our documentation at [`docs/api/types.md`](../api/types.md) provides a canonical reference for these data structures.
3. **Ensure Serde Compatibility**: Check `serde` attributes in the Rust code (e.g., `#[serde(rename = "..."`)) as they can alter the serialized field names.

**Source Reference**: [`src/lib/tauri-mcp-client.ts`](../src/lib/tauri-mcp-client.ts)
