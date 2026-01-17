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
}
