import { getLogger } from '../logger';
import {
  MCPTool,
  MCPResponse,
  MCPResult,
  LegacyToolCallResult,
  normalizeLegacyResponse,
  mcpResponseToString,
  isMCPError,
} from '../mcp-types';
import { Message } from '@/models/chat';

const logger = getLogger('AIService');

/**
 * 🔍 Message Validator
 *
 * 메시지와 도구 관련 검증 및 변환 로직을 담당합니다.
 * MCP 프로토콜 표준을 준수하며 레거시 호환성을 제공합니다.
 */

export class MessageValidator {
  static validateMessage(message: Message): void {
    if (!message.id || typeof message.id !== 'string') {
      throw new Error('Message must have a valid id');
    }
    if (
      (!message.content &&
        (message.role === 'user' || message.role === 'system')) ||
      typeof message.content !== 'string'
    ) {
      logger.error(`Invalid message content: `, { message });
      throw new Error('Message must have valid content');
    }
    if (!['user', 'assistant', 'system', 'tool'].includes(message.role)) {
      throw new Error('Message must have a valid role');
    }

    // Sanitize content length
    if (message.content.length > 100000) {
      throw new Error('Message content too long');
    }
  }

  static validateTool(tool: MCPTool): void {
    if (!tool.name || typeof tool.name !== 'string') {
      throw new Error('Tool must have a valid name');
    }
    if (!tool.description || typeof tool.description !== 'string') {
      throw new Error('Tool must have a valid description');
    }
    if (!tool.inputSchema || typeof tool.inputSchema !== 'object') {
      throw new Error('Tool must have a valid inputSchema');
    }
    if (tool.inputSchema.type !== 'object') {
      throw new Error('Tool inputSchema must be of type "object"');
    }
  }

  static sanitizeToolArguments(args: string): Record<string, unknown> {
    try {
      const parsed = JSON.parse(args);
      if (typeof parsed !== 'object' || parsed === null) {
        throw new Error('Tool arguments must be an object');
      }
      return parsed;
    } catch (error: unknown) {
      throw new Error(
        `Invalid tool arguments: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  /**
   * 🔄 Validates and normalizes responses to standard MCP format
   * Handles legacy responses and ensures JSON-RPC 2.0 compliance
   */
  static validateAndNormalizeMCPResponse(
    response: MCPResponse | LegacyToolCallResult | Record<string, unknown>,
    toolName: string,
  ): MCPResponse {
    // If response is already a proper MCPResponse, validate it
    if (response && typeof response === 'object' && 'jsonrpc' in response) {
      const mcpResponse = response as MCPResponse;
      if (mcpResponse.jsonrpc === '2.0') {
        return mcpResponse;
      }
    }

    // Handle legacy LegacyToolCallResult format
    if (response && typeof response === 'object' && 'success' in response) {
      const toolResult = response as LegacyToolCallResult;
      return normalizeLegacyResponse(toolResult, toolName);
    }

    // Handle raw responses that might have error indicators
    if (!response || typeof response !== 'object') {
      return {
        jsonrpc: '2.0',
        id: 'unknown',
        error: {
          code: -32602,
          message: `Invalid response from ${toolName}`,
        },
      };
    }

    const responseObj = response as Record<string, unknown>;

    // Critical: Detect error situations (core logic from refactoring plan)
    if (responseObj.error !== undefined || responseObj.isError === true) {
      return {
        jsonrpc: '2.0',
        id:
          typeof responseObj.id === 'string' ||
          typeof responseObj.id === 'number'
            ? responseObj.id
            : 'unknown',
        error: {
          code: -32603,
          message:
            typeof responseObj.error === 'string'
              ? responseObj.error
              : `Tool execution failed: ${toolName}`,
          data: responseObj.error,
        },
      };
    }

    // Success case - normalize to MCP format
    return {
      jsonrpc: '2.0',
      id:
        typeof responseObj.id === 'string' || typeof responseObj.id === 'number'
          ? responseObj.id
          : 'success',
      result: (responseObj.result as MCPResult) || {
        content: [
          {
            type: 'text',
            text:
              typeof responseObj.content === 'string'
                ? responseObj.content
                : JSON.stringify(responseObj),
          },
        ],
      },
    };
  }

  /**
   * 💬 Formats MCP response for chat system consumption
   * Converts MCP content to simple string format expected by chat
   */
  static formatMCPResponseForChat(
    mcpResponse: MCPResponse,
    toolCallId: string,
  ): { role: 'tool'; tool_call_id: string; content: string; error?: string } {
    if (isMCPError(mcpResponse)) {
      return {
        role: 'tool',
        tool_call_id: toolCallId,
        content: JSON.stringify({
          error: mcpResponse.error?.message || 'Tool execution failed',
          success: false,
        }),
        error: mcpResponse.error?.message || 'Tool execution failed',
      };
    }

    // Success case - extract content
    const content = mcpResponseToString(mcpResponse);

    return {
      role: 'tool',
      tool_call_id: toolCallId,
      content,
    };
  }
}
