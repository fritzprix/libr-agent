import { MCPTool, MCPResponse } from '@/lib/mcp-types';

/**
 * Extended MCPTool type that includes execute function with strict type safety
 * Used locally within browser tools to define executable tool objects
 */
export type StrictLocalMCPTool = MCPTool & {
  execute: (args: Record<string, unknown>) => Promise<MCPResponse>;
};

/**
 * Browser tool execute function that may need executeScript injection
 * Legacy type for backward compatibility during migration
 */
export type BrowserToolExecuteFunction = (
  args: Record<string, unknown>,
  executeScript?: (sessionId: string, script: string) => Promise<string>,
) => Promise<unknown>;

/**
 * Enhanced LocalMCPTool for browser tools that need executeScript
 * Uses legacy type for backward compatibility during migration
 */
export type BrowserLocalMCPTool = MCPTool & {
  execute: BrowserToolExecuteFunction;
};

/**
 * Strict browser tool that needs executeScript and returns MCPResponse
 */
export type StrictBrowserMCPTool = MCPTool & {
  execute: (
    args: Record<string, unknown>,
    executeScript?: (sessionId: string, script: string) => Promise<string>,
  ) => Promise<MCPResponse>;
};
