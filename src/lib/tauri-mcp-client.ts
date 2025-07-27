import { invoke } from '@tauri-apps/api/core';

export interface MCPServerConfig {
  name: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  transport: 'stdio' | 'http' | 'websocket';
  url?: string;
  port?: number;
}

export interface MCPTool {
  name: string;
  description: string;
  input_schema: {
    type: 'object';
    properties: Record<string, unknown>;
    required?: string[];
  };
}

export interface ToolCallResult {
  success: boolean;
  result?: unknown;
  error?: string;
}

export class TauriMCPClient {
  async startServer(config: MCPServerConfig): Promise<string> {
    return await invoke('start_mcp_server', { config });
  }

  async stopServer(serverName: string): Promise<void> {
    return await invoke('stop_mcp_server', { serverName });
  }

  async callTool(
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<ToolCallResult> {
    return await invoke('call_mcp_tool', {
      serverName,
      toolName,
      arguments: arguments_,
    });
  }

  async listTools(serverName: string): Promise<MCPTool[]> {
    return await invoke('list_mcp_tools', { serverName });
  }

  async listToolsFromConfig(config: {
    mcpServers?: Record<
      string,
      { command: string; args?: string[]; env?: Record<string, string> }
    >;
  }): Promise<MCPTool[]> {
    return await invoke('list_tools_from_config', { config });
  }

  async getConnectedServers(): Promise<string[]> {
    return await invoke('get_connected_servers');
  }

  async checkServerStatus(serverName: string): Promise<boolean> {
    return await invoke('check_server_status', { serverName });
  }

  async checkAllServersStatus(): Promise<Record<string, boolean>> {
    return await invoke('check_all_servers_status');
  }
}

export const tauriMCPClient = new TauriMCPClient();
