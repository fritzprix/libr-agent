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
  listTools: () => import('../../lib/mcp-types').MCPTool[]; // Lazy import to avoid cycle if needed, or import at top
  executeTool: (
    toolCall: import('../../models/chat').ToolCall,
  ) => Promise<import('../../lib/mcp-types').MCPResponse<unknown>>;
  getServiceContext: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<T>>;
}
