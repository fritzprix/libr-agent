/**
 * Response Builder for Web MCP Tools
 *
 * This module provides a type-safe, fluent API for constructing MCP tool responses
 * with consistent error handling, suggestions, and next action guidance.
 */

import type { MCPResult } from '@/lib/mcp/protocol/response';
import {
  createMCPErrorToolResult,
  createMCPStructuredToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPErrorData } from './error-codes';

/**
 * Builder class for constructing MCP tool responses with a fluent API
 *
 * @example
 * // Error response with suggestions
 * return new MCPResponseBuilder({ results: [] })
 *   .withMessage('No servers found matching "test"')
 *   .withSuggestions(['Try searchMode: "bm25"', 'Use list_servers'])
 *   .asError('MCP_MANAGER.SERVER_NOT_FOUND');
 *
 * @example
 * // Success response with next actions
 * return new MCPResponseBuilder({ server, scope })
 *   .withMessage('Server "test" connected to global scope')
 *   .withNextActions(['Use list_servers to verify', 'Check available tools'])
 *   .asSuccess();
 */
export class MCPResponseBuilder<T = Record<string, unknown>> {
  private message: string = '';
  private data: T;
  private suggestions: string[] = [];
  private nextActions: string[] = [];
  private additionalErrorData?: Record<string, unknown>;

  constructor(data: T) {
    this.data = data;
  }

  /**
   * Set the main response message (max 200 chars recommended)
   */
  withMessage(msg: string): this {
    this.message = msg;
    return this;
  }

  /**
   * Add suggestions for resolving errors or improving usage
   */
  withSuggestions(suggestions: string[]): this {
    this.suggestions = suggestions;
    return this;
  }

  /**
   * Add next action guidance for successful operations
   */
  withNextActions(actions: string[]): this {
    this.nextActions = actions;
    return this;
  }

  /**
   * Add additional error-specific data (only used for error responses)
   */
  withErrorData(data: Record<string, unknown>): this {
    this.additionalErrorData = data;
    return this;
  }

  /**
   * Build as error response
   */
  asError(code: string): MCPResult<T & { error: MCPErrorData }> {
    const suggestionsText =
      this.suggestions.length > 0
        ? `\n\nSuggestions:\n${this.suggestions.map((s) => `  - ${s}`).join('\n')}`
        : '';

    const errorData: MCPErrorData = {
      code,
      suggestions: this.suggestions.length > 0 ? this.suggestions : undefined,
      ...this.additionalErrorData,
    };

    return createMCPErrorToolResult(this.message + suggestionsText, {
      ...this.data,
      error: errorData,
    }) as MCPResult<T & { error: MCPErrorData }>;
  }

  /**
   * Build as success response
   */
  asSuccess(): MCPResult<T & { nextActions?: string[] }> {
    const actionsText =
      this.nextActions.length > 0
        ? `\n\nNext steps:\n${this.nextActions.map((a, i) => `  ${i + 1}. ${a}`).join('\n')}`
        : '';

    const suggestionsText =
      this.suggestions.length > 0
        ? `\n\nSuggestions:\n${this.suggestions.map((s) => `  - ${s}`).join('\n')}`
        : '';

    const resultData = {
      ...this.data,
      nextActions: this.nextActions.length > 0 ? this.nextActions : undefined,
    };

    return createMCPStructuredToolResult(
      this.message + actionsText + suggestionsText,
      resultData,
    ) as MCPResult<T & { nextActions?: string[] }>;
  }
}

/**
 * Helper function to create error response with suggestions
 *
 * @deprecated Use MCPResponseBuilder for better type safety
 */
export function createErrorWithSuggestions(
  message: string,
  code: string,
  suggestions: string[],
  additionalData?: Record<string, unknown>,
): MCPResult<{ error: MCPErrorData }> {
  return new MCPResponseBuilder(additionalData || {})
    .withMessage(message)
    .withSuggestions(suggestions)
    .asError(code);
}

/**
 * Helper function to create success response with next actions
 *
 * @deprecated Use MCPResponseBuilder for better type safety
 */
export function createSuccessWithNextActions<T>(
  message: string,
  data: T,
  nextActions: string[],
): MCPResult<T & { nextActions?: string[] }> {
  return new MCPResponseBuilder(data)
    .withMessage(message)
    .withNextActions(nextActions)
    .asSuccess();
}

/**
 * Utility to format suggestions list
 */
export function formatSuggestions(suggestions: string[]): string {
  if (suggestions.length === 0) return '';
  return `\n\nSuggestions:\n${suggestions.map((s) => `  - ${s}`).join('\n')}`;
}

/**
 * Utility to format next actions list
 */
export function formatNextActions(actions: string[]): string {
  if (actions.length === 0) return '';
  return `\n\nNext steps:\n${actions.map((a, i) => `  ${i + 1}. ${a}`).join('\n')}`;
}

/**
 * Utility to truncate message to recommended length (200 chars)
 * while preserving word boundaries
 */
export function truncateMessage(
  message: string,
  maxLength: number = 200,
): string {
  if (message.length <= maxLength) return message;

  const truncated = message.slice(0, maxLength);
  const lastSpace = truncated.lastIndexOf(' ');

  return lastSpace > 0
    ? truncated.slice(0, lastSpace) + '...'
    : truncated + '...';
}
