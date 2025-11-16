import type { MCPResponse, MCPContent, MCPResult } from './mcp-types';
import { createId } from '@paralleldrive/cuid2';

/**
 * Creates a standard MCP response containing only a text message.
 *
 * @param text The text content for the response.
 * @param id Optional JSON-RPC request ID. If not provided, a new one is generated.
 * @returns An `MCPResponse` object with the specified text content.
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
 * Creates an MCP response that includes both a text message and a structured content payload.
 *
 * @template T The type of the structured content.
 * @param text The text content for the response.
 * @param structuredContent The structured data payload.
 * @param id Optional JSON-RPC request ID. If not provided, a new one is generated.
 * @returns An `MCPResponse` object with both text and structured content.
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
 * A type guard to check if a given object is a valid MCPResponse.
 * It verifies the object's structure and the `jsonrpc` version.
 *
 * @param obj The object to check.
 * @returns True if the object is a valid `MCPResponse`, false otherwise.
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
 * Creates a standard MCP error response.
 *
 * @param message A string providing a short description of the error.
 * @param code A number that indicates the error type that occurred. Defaults to -32603 (Internal error).
 * @param data Optional additional information about the error.
 * @param id Optional JSON-RPC request ID. Can be null if the request ID is not available.
 * @returns An `MCPResponse` object formatted as an error.
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
 * Creates an empty MCP success response with no content.
 * This can be used to acknowledge a request without sending back any specific data.
 *
 * @param id Optional JSON-RPC request ID. If not provided, a new one is generated.
 * @returns An empty but valid `MCPResponse` object.
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

/**
 * Creates an MCP response that includes an arbitrary content array (multiple parts)
 * and a structured content payload. Use this when the response should include
 * UI resources or other non-text content alongside structured data.
 *
 * @template T The type of the structured content.
 * @param contents Array of MCPContent items to include in the response result.content
 * @param structuredContent The structured data payload to include under structuredContent
 * @param id Optional JSON-RPC request ID. If not provided, a new one is generated.
 * @returns An `MCPResponse` object with both content and structuredContent.
 */
export function createMCPStructuredMultipartResponse<T>(
  contents: MCPContent[],
  structuredContent: T,
  id?: string | number | null,
): MCPResponse<T> {
  return {
    jsonrpc: '2.0',
    id: id ?? createId(),
    result: {
      content: contents,
      structuredContent,
    },
  };
}

// ============================================================================
// Tool Result Helpers (for Built-in Tools - No Transport Layer)
// ============================================================================

/**
 * Creates a successful tool result (MCPResult) for built-in tools.
 * Use this for built-in tools that don't need JSON-RPC transport layer.
 *
 * @param text The text content for the result.
 * @returns An `MCPResult` object with the specified text content.
 *
 * @example
 * ```typescript
 * return createMCPSuccessToolResult('Operation completed successfully');
 * ```
 */
export function createMCPSuccessToolResult(text: string): MCPResult<unknown> {
  return {
    content: [{ type: 'text', text }],
    isError: false,
  };
}

/**
 * Creates a successful tool result with structured content for built-in tools.
 *
 * @template T The type of the structured content.
 * @param text The text content for the result.
 * @param structuredContent The structured data payload.
 * @returns An `MCPResult` object with both text and structured content.
 *
 * @example
 * ```typescript
 * return createMCPStructuredToolResult('Found 3 items', { items: [...], count: 3 });
 * ```
 */
export function createMCPStructuredToolResult<T>(
  text: string,
  structuredContent: T,
): MCPResult<T> {
  return {
    content: [{ type: 'text', text }],
    structuredContent,
    isError: false,
  };
}

/**
 * Creates a multipart successful tool result with arbitrary content for built-in tools.
 *
 * @template T The type of the structured content.
 * @param contents Array of MCPContent items (can include UI resources, images, etc.)
 * @param structuredContent Optional structured data payload.
 * @returns An `MCPResult` object with the specified content and structured data.
 *
 * @example
 * ```typescript
 * return createMCPMultipartToolResult(
 *   [
 *     { type: 'text', text: 'Chart generated' },
 *     { type: 'resource', resource: { uri: 'ui://chart', mimeType: 'text/html' } }
 *   ],
 *   { chartData: [...] }
 * );
 * ```
 */
export function createMCPMultipartToolResult<T>(
  contents: MCPContent[],
  structuredContent?: T,
): MCPResult<T> {
  return {
    content: contents,
    structuredContent,
    isError: false,
  };
}

/**
 * Creates an error tool result (MCPResult with isError flag) for built-in tools.
 * This represents a tool execution that failed due to business logic errors.
 *
 * Note: This is different from protocol-level errors (MCPError in MCPResponse).
 * Use this when the tool executed but encountered a logical error (not found, validation, etc.)
 *
 * @param message The error message.
 * @param data Optional additional error context data.
 * @returns An `MCPResult` object with isError flag set to true.
 *
 * @example
 * ```typescript
 * // Not found error
 * return createMCPErrorToolResult('Server not found', { serverName: 'test' });
 *
 * // Validation error
 * return createMCPErrorToolResult('Invalid scope parameter', { validScopes: ['assistant', 'global'] });
 * ```
 */
export function createMCPErrorToolResult(
  message: string,
  data?: unknown,
): MCPResult<unknown> {
  return {
    content: [{ type: 'text', text: message }],
    isError: true,
    structuredContent: data ? { error: data } : undefined,
  };
}
