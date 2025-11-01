/**
 * @file MCP Response Types
 * @description JSON-RPC 2.0 compliant response types
 * @see https://modelcontextprotocol.io/
 */

import type { MCPContent } from './content';

/**
 * Represents the result part of a standard MCP response.
 * @template T The type of the structured content.
 */
export interface MCPResult<T = unknown> {
  content?: MCPContent[];
  structuredContent?: T;
  /** A flag indicating if the result is from a tool execution that resulted in an error. */
  isError?: boolean;
}

/**
 * Extends the standard MCP result with information specific to a sampling operation.
 */
export interface SamplingResult extends MCPResult {
  sampling?: {
    finishReason?: 'stop' | 'length' | 'tool_use' | 'error';
    usage?: {
      promptTokens: number;
      completionTokens: number;
      totalTokens: number;
    };
    model?: string;
  };
}

/**
 * Represents a JSON-RPC error object.
 */
export interface MCPError {
  code: number;
  message: string;
  data?: unknown;
}

/**
 * The standard MCP response structure, compliant with JSON-RPC 2.0.
 * All MCP responses must follow this format.
 * @template T The type of the structured content in the result.
 */
export interface MCPResponse<T> {
  jsonrpc: '2.0';
  id: string | number | null;
  result?: MCPResult<T> | SamplingResult;
  error?: MCPError;
}

/**
 * An extended MCP response that includes service context information.
 * This preserves the service context to support accurate tool re-invocation from the UI.
 */
export interface ExtendedMCPResponse extends MCPResponse<unknown> {
  serviceInfo?: {
    serverName: string;
    toolName: string;
    backendType: 'ExternalMCP' | 'BuiltInWeb' | 'BuiltInRust';
  };
}
