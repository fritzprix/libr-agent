/**
 * MCP Tool Result utilities for dual response contract handling.
 *
 * Provides helper types and functions for accessing both text and structured
 * data from MCP responses, supporting the dual response contract where servers
 * can provide both human-friendly text (content) and structured data (structuredContent).
 */

/**
 * Standard MCP result envelope structure.
 */
export interface MCPResultEnvelope {
  content?: Array<{ type: string; text?: string }>;
  structuredContent?: unknown;
}

/**
 * Enhanced tool result that provides access to both text and structured data.
 *
 * @template T - The expected type of the structured data
 */
export interface ToolResult<T = unknown> {
  /** Human-readable text content from result.content[0].text */
  text?: string;
  /** Structured data from result.structuredContent */
  data?: T;
  /** Raw MCP result envelope for custom processing */
  raw: MCPResultEnvelope;
}

/**
 * Converts an MCP response into a ToolResult with both text and structured data access.
 *
 * This is useful when you need access to both the text representation and structured
 * data from an MCP tool response. The WebMCPContext proxy normally returns only one
 * format based on availability, but this helper preserves both.
 *
 * @param mcpResponse - The raw MCP response from a tool call
 * @returns ToolResult with separated text, data, and raw fields
 *
 * @example
 * ```typescript
 * const response = await proxy.callTool('server', 'tool', args);
 * const both = toToolResult<AddContentOutput>(response);
 *
 * // Access structured data
 * if (both.data) {
 *   console.log('Store ID:', both.data.storeId);
 * }
 *
 * // Access human-readable text
 * if (both.text) {
 *   console.log('Summary:', both.text);
 * }
 * ```
 */
export function toToolResult<T = unknown>(mcpResponse: unknown): ToolResult<T> {
  const result =
    mcpResponse && typeof mcpResponse === 'object'
      ? ((mcpResponse as { result?: MCPResultEnvelope }).result ?? {})
      : {};

  // Extract structured data
  const data = (result as { structuredContent?: unknown }).structuredContent as
    | T
    | undefined;

  // Extract text content
  const text =
    Array.isArray(result.content) && result.content[0]?.type === 'text'
      ? result.content[0].text
      : undefined;

  return {
    text,
    data,
    raw: result,
  };
}

/**
 * Type guard to check if a ToolResult has structured data.
 *
 * @param result - The ToolResult to check
 * @returns True if the result has structured data
 */
export function hasStructuredData<T>(
  result: ToolResult<T>,
): result is ToolResult<T> & { data: T } {
  return result.data !== undefined && result.data !== null;
}

/**
 * Type guard to check if a ToolResult has text content.
 *
 * @param result - The ToolResult to check
 * @returns True if the result has text content
 */
export function hasTextContent<T>(
  result: ToolResult<T>,
): result is ToolResult<T> & { text: string } {
  return typeof result.text === 'string' && result.text.length > 0;
}
