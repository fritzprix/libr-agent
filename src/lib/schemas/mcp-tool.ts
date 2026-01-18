import type { MCPTool } from '@/lib/mcp/protocol/tool';

/**
 * Type guard to check if an object is a valid MCP tool
 * Validates the minimal required structure without deep validation
 */
export function isMCPTool(obj: unknown): obj is MCPTool {
  if (typeof obj !== 'object' || obj === null) {
    return false;
  }

  const tool = obj as Record<string, unknown>;

  return (
    typeof tool.name === 'string' &&
    typeof tool.description === 'string' &&
    typeof tool.inputSchema === 'object' &&
    tool.inputSchema !== null
  );
}

/**
 * Helper function to validate an array of tools
 * Filters out invalid tools and logs warnings
 */
export function validateMCPTools(tools: unknown[]): MCPTool[] {
  return tools.filter((tool): tool is MCPTool => {
    if (isMCPTool(tool)) {
      return true;
    }
    return false;
  });
}

/**
 * Safe parse with error handling
 */
export function parseMCPTool(obj: unknown): MCPTool | undefined {
  return isMCPTool(obj) ? obj : undefined;
}
