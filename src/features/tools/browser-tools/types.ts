import { MCPTool } from '@/lib/mcp-types';

/**
 * Extended MCPTool type that includes execute function
 * Used locally within browser tools to define executable tool objects
 */
export type LocalMCPTool = MCPTool & {
  execute: (args: Record<string, unknown>) => Promise<unknown>;
};

/**
 * Browser tool execute function that may need executeScript injection
 */
export type BrowserToolExecuteFunction = (
  args: Record<string, unknown>,
  executeScript?: (sessionId: string, script: string) => Promise<string>,
) => Promise<unknown>;

/**
 * Enhanced LocalMCPTool for browser tools that need executeScript
 */
export type BrowserLocalMCPTool = MCPTool & {
  execute: BrowserToolExecuteFunction;
};
