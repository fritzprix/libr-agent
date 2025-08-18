import { invoke } from '@tauri-apps/api/core';
import {
  MCPServerConfig,
  MCPTool,
  MCPResponse,
  SamplingOptions,
  SamplingResponse,
} from './mcp-types';
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
      const result = (await invoke('start_mcp_server', { config })) as string;
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
      logger.debug('Calling MCP tool', {
        serverName,
        toolName,
        arguments: arguments_,
      });
      const result = (await invoke('call_mcp_tool', {
        serverName,
        toolName,
        arguments: arguments_,
      })) as MCPResponse;
      logger.debug('MCP tool call completed', { serverName, toolName, result });
      return result;
    } catch (error) {
      logger.error('Failed to call MCP tool', error);
      throw error;
    }
  }

  /**
   * MCP 서버에서 sampling을 수행합니다
   * @param serverName 서버 이름
   * @param prompt 입력 프롬프트
   * @param options 샘플링 옵션
   * @returns 샘플링 결과
   */
  async sampleFromModel(
    serverName: string,
    prompt: string,
    options?: SamplingOptions,
  ): Promise<SamplingResponse> {
    try {
      logger.debug('Sampling from MCP server', { serverName, prompt, options });
      const result = (await invoke('sample_from_mcp_server', {
        serverName,
        prompt,
        options,
      })) as SamplingResponse;
      logger.debug('MCP sampling completed', { result });
      return result;
    } catch (error) {
      logger.error('Failed to sample from MCP server', error);
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
      const tools = (await invoke('list_mcp_tools', {
        serverName,
      })) as MCPTool[];
      logger.debug('Tools listed successfully', {
        serverName,
        toolCount: tools.length,
      });
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
        serverCount: Object.keys(config.mcpServers || {}).length,
      });
      const tools = (await invoke('list_tools_from_config', {
        config,
      })) as MCPTool[];
      logger.debug('Tools listed from config successfully', {
        toolCount: tools.length,
      });
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
      const servers = (await invoke('get_connected_servers')) as string[];
      logger.debug('Connected servers retrieved', {
        serverCount: servers.length,
        servers,
      });
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
      const status = (await invoke('check_server_status', {
        serverName,
      })) as boolean;
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
      const statusMap = (await invoke('check_all_servers_status')) as Record<
        string,
        boolean
      >;
      logger.debug('All servers status checked', { statusMap });
      return statusMap;
    } catch (error) {
      logger.error('Failed to check all servers status', error);
      throw error;
    }
  }

  // Built-in MCP server methods

  /**
   * 사용 가능한 내장 서버 목록을 조회합니다
   * @returns 내장 서버 이름 배열
   */
  async listBuiltinServers(): Promise<string[]> {
    try {
      logger.debug('Listing builtin servers');
      const servers = (await invoke('list_builtin.servers')) as string[];
      logger.debug('Builtin servers listed', {
        serverCount: servers.length,
        servers,
      });
      return servers;
    } catch (error) {
      logger.error('Failed to list builtin servers', error);
      throw error;
    }
  }

  /**
   * 내장 서버의 모든 도구 목록을 조회합니다
   * @returns 내장 서버 도구 목록
   */
  async listBuiltinTools(): Promise<MCPTool[]> {
    try {
      logger.debug('Listing builtin tools');
      const tools = (await invoke('list_builtin_tools')) as MCPTool[];
      logger.debug('Builtin tools listed', { toolCount: tools.length });
      return tools;
    } catch (error) {
      logger.error('Failed to list builtin tools', error);
      throw error;
    }
  }

  /**
   * 내장 서버의 도구를 호출합니다
   * @param serverName 내장 서버 이름 (예: "builtin.filesystem")
   * @param toolName 도구 이름
   * @param arguments_ 도구 인수
   * @returns 도구 실행 결과
   */
  async callBuiltinTool(
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> {
    try {
      logger.debug('Calling builtin tool', {
        serverName,
        toolName,
        arguments: arguments_,
      });
      const result = (await invoke('call_builtin_tool', {
        serverName,
        toolName,
        arguments: arguments_,
      })) as MCPResponse;
      logger.debug('Builtin tool call completed', {
        serverName,
        toolName,
        result,
      });
      return result;
    } catch (error) {
      logger.error('Failed to call builtin tool', error);
      throw error;
    }
  }

  /**
   * 외부 + 내장 서버의 모든 도구 목록을 조회합니다 (통합 API)
   * @returns 모든 도구 목록
   */
  async listAllToolsUnified(): Promise<MCPTool[]> {
    try {
      logger.debug('Listing all tools (unified)');
      const tools = (await invoke('list_all_tools_unified')) as MCPTool[];
      logger.debug('All tools listed (unified)', { toolCount: tools.length });
      return tools;
    } catch (error) {
      logger.error('Failed to list all tools (unified)', error);
      throw error;
    }
  }

  /**
   * 외부 또는 내장 서버의 도구를 자동으로 라우팅하여 호출합니다 (통합 API)
   * @param serverName 서버 이름
   * @param toolName 도구 이름
   * @param arguments_ 도구 인수
   * @returns 도구 실행 결과
   */
  async callToolUnified(
    serverName: string,
    toolName: string,
    arguments_: Record<string, unknown>,
  ): Promise<MCPResponse> {
    try {
      logger.debug('Calling tool (unified)', {
        serverName,
        toolName,
        arguments: arguments_,
      });
      const result = (await invoke('call_tool_unified', {
        serverName,
        toolName,
        arguments: arguments_,
      })) as MCPResponse;
      logger.debug('Tool call completed (unified)', {
        serverName,
        toolName,
        result,
      });
      return result;
    } catch (error) {
      logger.error('Failed to call tool (unified)', error);
      throw error;
    }
  }
}

export const tauriMCPClient = new TauriMCPClient();
