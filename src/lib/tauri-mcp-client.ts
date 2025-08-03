import { invoke } from '@tauri-apps/api/core';
import { MCPServerConfig, MCPTool, LegacyToolCallResult } from './mcp-types';

/**
 * 🔌 Tauri MCP Client
 *
 * Tauri 백엔드와 통신하여 MCP 서버를 관리하고 도구를 호출하는 클라이언트
 * 레거시 ToolCallResult를 반환하지만, 점진적으로 MCPResponse로 마이그레이션 예정
 */

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
  ): Promise<LegacyToolCallResult> {
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
