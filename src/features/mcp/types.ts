import type { MCPTool, MCPResponse } from '@/lib/mcp';
import type { ToolCall } from '../../models/chat';

/**
 * Metadata describing a built-in service.
 */
export interface ServiceMetadata {
  displayName: string;
  description: string;
  category: 'automation' | 'system' | 'data' | 'other';
  icon?: string;
}

/**
 * Options passed to getServiceContext.
 */
export interface ServiceContextOptions {
  sessionId?: string;
  assistantId?: string;
  threadId?: string;
}

/**
 * The structure returned by getServiceContext.
 */
export interface ServiceContext<T = unknown> {
  contextPrompt: string;
  structuredState?: T;
}

export interface BuiltInService<T = unknown> {
  metadata: ServiceMetadata;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
  listTools: () => MCPTool[];
  executeTool: (
    toolCall: ToolCall,
  ) => Promise<MCPResponse<unknown>>;
  getServiceContext: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<T>>;
}
