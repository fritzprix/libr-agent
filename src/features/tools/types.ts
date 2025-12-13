import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { ToolCall } from '@/models/chat';

export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;

  /**
   * Thread ID for tool context isolation.
   * Tools executed with the same threadId share context.
   *
   * OPTIONAL: If not provided, defaults to sessionId (top thread).
   * This allows backward compatibility with existing services.
   */
  threadId?: string;
}

export interface ServiceContext<T = unknown> {
  contextPrompt: string;
  structuredState?: T;
}

export interface ServiceMetadata {
  displayName: string;
  description: string;
  icon?: string;
}

export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;

  // BREAKING CHANGE: 모든 서비스는 반드시 구조화된 ServiceContext를 반환해야 함
  getServiceContext: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;

  // BREAKING CHANGE: 세션/어시스턴트 전환 시 서비스가 내부 정리/프리로드를 수행하도록 구현을 요구
  switchContext: (options?: ServiceContextOptions) => Promise<void>;
}
