import { MCPResponse } from './mcp-types';
import { createId } from '@paralleldrive/cuid2';

/**
 * Create a standard MCP text response
 */
export function createMCPTextResponse(
  text: string,
  id?: string | number | null,
): MCPResponse {
  return {
    jsonrpc: '2.0',
    id: id ?? createId(),
    result: {
      content: [{ type: 'text', text }],
    },
  };
}

/**
 * Create a structured MCP response with both text and structured content
 */
export function createMCPStructuredResponse(
  text: string,
  structuredContent: Record<string, unknown>,
  id?: string | number | null,
): MCPResponse {
  return {
    jsonrpc: '2.0',
    id: id ?? createId(),
    result: {
      content: [{ type: 'text', text }],
      structuredContent,
    },
  };
}

/**
 * Type guard to check if an object is an MCPResponse
 */
export function isMCPResponse(obj: unknown): obj is MCPResponse {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'jsonrpc' in obj &&
    (obj as MCPResponse).jsonrpc === '2.0'
  );
}

/**
 * Create an MCP error response
 */
export function createMCPErrorResponse(
  code: number,
  message: string,
  data?: unknown,
  id?: string | number | null,
): MCPResponse {
  return {
    jsonrpc: '2.0',
    id: id ?? createId(),
    error: {
      code,
      message,
      data,
    },
  };
}
