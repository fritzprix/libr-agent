import { MCPResponse } from './mcp-types';
import { createId } from '@paralleldrive/cuid2';

/**
 * Create a standard MCP text response
 */
export function createMCPTextResponse(
  text: string,
  id?: string | number | null,
): MCPResponse<unknown> {
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
export function createMCPStructuredResponse<T>(
  text: string,
  structuredContent: T,
  id?: string | number | null,
): MCPResponse<T> {
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
export function isMCPResponse(obj: unknown): obj is MCPResponse<unknown> {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'jsonrpc' in obj &&
    (obj as MCPResponse<unknown>).jsonrpc === '2.0'
  );
}

/**
 * Create an MCP error response
 */
export function createMCPErrorResponse(
  message: string,
  code: number = -32603,
  data?: unknown,
  id?: string | number | null,
): MCPResponse<unknown> {
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

/**
 * Create an empty MCP response with no content
 */
export function createMCPEmptyResponse(
  id?: string | number | null,
): MCPResponse<unknown> {
  return {
    jsonrpc: '2.0',
    id: id ?? createId(),
    result: { content: [] },
  };
}
