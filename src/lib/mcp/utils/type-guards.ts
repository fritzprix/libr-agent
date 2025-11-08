/**
 * @file MCP Type Guard Functions
 * @description Runtime type checking utilities
 */

import type { MCPResponse, MCPResult, ExtendedMCPResponse } from '../protocol';

/**
 * A type guard to check if an MCP response is a success response.
 * @param response The MCP response to check.
 * @returns True if the response has a `result` property and no `error` property.
 */
export function isMCPSuccess(
  response: MCPResponse<unknown>,
): response is MCPResponse<unknown> & { result: MCPResult } {
  return response.error === undefined && response.result !== undefined;
}

/**
 * A type guard to check if an MCP response is an error response.
 * @param response The MCP response to check.
 * @returns True if the response has an `error` property.
 */
export function isMCPError(
  response: MCPResponse<unknown>,
): response is MCPResponse<unknown> & {
  error: { code: number; message: string; data?: unknown };
} {
  return response.error !== undefined;
}

/**
 * Checks if an `MCPResult` object contains any valid content.
 * @param result The result object to check.
 * @returns True if the result has either `content` or `structuredContent`.
 */
export function isValidMCPResult(result: MCPResult): boolean {
  return !!(result.content?.length || result.structuredContent);
}

/**
 * Safely extracts the `structuredContent` from an MCP response.
 * @template T The expected type of the structured content.
 * @param response The MCP response.
 * @returns The `structuredContent` if it exists, otherwise null.
 */
export function extractStructuredContent<T>(
  response: MCPResponse<T>,
): T | null {
  if (!response.result || response.error) {
    return null;
  }

  // This is not a standard MCPResult, but a SamplingResult, so it can't have structuredContent.
  if ('sampling' in response.result) {
    return null;
  }

  return (response.result as MCPResult<T>).structuredContent || null;
}

/**
 * A type guard to check if an MCP response is successful and contains structured content.
 * @template T The expected type of the structured content.
 * @param response The MCP response to check.
 * @returns True if the response is successful and has `structuredContent`.
 */
export function hasStructuredContent<T>(
  response: MCPResponse<T>,
): response is MCPResponse<T> & {
  result: MCPResult<T> & { structuredContent: T };
} {
  const structured = extractStructuredContent(response);
  return structured !== null && structured !== undefined;
}

/**
 * A type guard to check if a response is an `ExtendedMCPResponse`.
 * @param response The response object to check.
 * @returns True if the response is an `ExtendedMCPResponse`, false otherwise.
 */
export function isExtendedResponse(
  response: MCPResponse<unknown>,
): response is ExtendedMCPResponse {
  return response && typeof response === 'object' && 'serviceInfo' in response;
}
