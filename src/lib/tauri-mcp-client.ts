import { invoke } from '@tauri-apps/api/core';
import { MCPServerConfig, MCPTool, MCPResponse } from './mcp-types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('TauriMCPClient');

/**
 * 🔌 Tauri MCP Client
 *
 * Tauri 백엔드와 통신하여 MCP 서버를 관리하고 도구를 호출하는 클라이언트
 */

export class TauriMCPClient {
  /**
   * MCP 서버를 시작합니다
   * @param config MCP 서버 설정
   * @returns 서버 식별자
   */
  async startServer(config: MCPServerConfig): Promise<string> {
    try {
      logger.debug('Starting MCP server', { config });
      const result = await invoke('start_mcp_server', { config }) as string;
      logger.info('MCP server started successfully', { serverId: result });
      return result;
    } catch (error) {
      logger.error('Failed to start MCP server', error);
      throw error;
    }
  }

  /**
   * MCP 서버를 중지합니다
   * @param serverName 서버 이름
   */
  async stopServer(serverName: string): Promise<void> {
    try {
      logger.debug('Stopping MCP server', { serverName });
      await invoke('stop_mcp_server', { serverName });
      logger.info('MCP server stopped successfully', { serverName });
    } catch (error) {
      logger.error('Failed to stop MCP server', error);
      throw error;
    }
  }

  /**
   * MCP 도구를 호출합니다
   * @param serverName 서버 이름
   * @param toolName 도구 이름
   * @param arguments_ 도구 인수
   * @returns 도구 실행 결과
   */
  async callTool(
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> {
    try {
      logger.debug('Calling MCP tool', { serverName, toolName, arguments: arguments_ });
      const result = await invoke('call_mcp_tool', {
        serverName,
        toolName,
        arguments: arguments_,
      }) as MCPResponse;
      logger.debug('MCP tool call completed', { serverName, toolName, result });
      return result;
    } catch (error) {
      logger.error('Failed to call MCP tool', error);
      throw error;
    }
  }

  /**
   * 특정 서버의 도구 목록을 조회합니다
   * @param serverName 서버 이름
   * @returns 도구 목록
   */
  async listTools(serverName: string): Promise<MCPTool[]> {
    try {
      logger.debug('Listing tools for server', { serverName });
      const tools = await invoke('list_mcp_tools', { serverName }) as MCPTool[];
      logger.debug('Tools listed successfully', { serverName, toolCount: tools.length });
      return tools;
    } catch (error) {
      logger.error('Failed to list tools', error);
      throw error;
    }
  }

  /**
   * 설정에서 모든 도구 목록을 조회합니다
   * @param config MCP 서버 설정
   * @returns 모든 도구 목록
   */
  async listToolsFromConfig(config: {
    mcpServers?: Record<
      string,
      { command: string; args?: string[]; env?: Record<string, string> }
    >;
  }): Promise<MCPTool[]> {
    try {
      logger.debug('Listing tools from config', { 
        serverCount: Object.keys(config.mcpServers || {}).length 
      });
      const tools = await invoke('list_tools_from_config', { config }) as MCPTool[];
      logger.debug('Tools listed from config successfully', { toolCount: tools.length });
      return tools;
    } catch (error) {
      logger.error('Failed to list tools from config', error);
      throw error;
    }
  }

  /**
   * 연결된 서버 목록을 조회합니다
   * @returns 연결된 서버 이름 배열
   */
  async getConnectedServers(): Promise<string[]> {
    try {
      logger.debug('Getting connected servers');
      const servers = await invoke('get_connected_servers') as string[];
      logger.debug('Connected servers retrieved', { serverCount: servers.length, servers });
      return servers;
    } catch (error) {
      logger.error('Failed to get connected servers', error);
      throw error;
    }
  }

  /**
   * 특정 서버의 상태를 확인합니다
   * @param serverName 서버 이름
   * @returns 서버 연결 상태
   */
  async checkServerStatus(serverName: string): Promise<boolean> {
    try {
      logger.debug('Checking server status', { serverName });
      const status = await invoke('check_server_status', { serverName }) as boolean;
      logger.debug('Server status checked', { serverName, status });
      return status;
    } catch (error) {
      logger.error('Failed to check server status', error);
      throw error;
    }
  }

  /**
   * 모든 서버의 상태를 확인합니다
   * @returns 서버별 연결 상태 맵
   */
  async checkAllServersStatus(): Promise<Record<string, boolean>> {
    try {
      logger.debug('Checking all servers status');
      const statusMap = await invoke('check_all_servers_status') as Record<string, boolean>;
      logger.debug('All servers status checked', { statusMap });
      return statusMap;
    } catch (error) {
      logger.error('Failed to check all servers status', error);
      throw error;
    }
  }
}

export const tauriMCPClient = new TauriMCPClient();
