import { MCPResponse, MCPContent } from './mcp-types';
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
