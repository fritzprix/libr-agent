/**
 * Ollama Core Logic (Pure Functions)
 *
 * This module contains the core business logic for Ollama API interactions,
 * extracted as pure functions with injectable logger for testing.
 * No Tauri/browser dependencies - can be tested in Node.js environment.
 */

import type { Tool } from 'ollama';
import type { Message } from '@/models/chat';
import type { MCPTool } from '../mcp-types';
import { tryParse, formatToolCall, generateToolCallId } from './utils';

/**
 * Logger interface for dependency injection
 */
export interface Logger {
  debug: (message: string, ...args: unknown[]) => void;
  info: (message: string, ...args: unknown[]) => void;
  warn: (message: string, ...args: unknown[]) => void;
  error: (message: string, ...args: unknown[]) => void;
}

/**
 * No-op logger for testing without console output
 */
export const noopLogger: Logger = {
  debug: () => {},
  info: () => {},
  warn: () => {},
  error: () => {},
};

/**
 * Console logger for Node.js testing
 */
export const consoleLogger: Logger = {
  debug: (message: string, ...args: unknown[]) =>
    console.log('[DEBUG]', message, ...args),
  info: (message: string, ...args: unknown[]) =>
    console.log('[INFO]', message, ...args),
  warn: (message: string, ...args: unknown[]) =>
    console.warn('[WARN]', message, ...args),
  error: (message: string, ...args: unknown[]) =>
    console.error('[ERROR]', message, ...args),
};

/**
 * Internal message format for Ollama API
 */
export interface SimpleOllamaMessage {
  role: 'user' | 'assistant' | 'system';
  content: string;
  tool_calls?: Array<{
    id: string;
    type: 'function';
    function: {
      name: string;
      arguments: Record<string, unknown>;
    };
  }>;
  tool_call_id?: string;
}

/**
 * Converts MCP tools to Ollama tool format
 */
export function convertMCPToolsToOllamaTools(
  mcpTools?: MCPTool[],
  logger: Logger = noopLogger,
): Tool[] {
  if (!mcpTools || mcpTools.length === 0) {
    return [];
  }

  return mcpTools.map((tool) => {
    const schema = tool.inputSchema || { type: 'object', properties: {} };

    // Build parameters object
    // NOTE: Some models (e.g., DeepSeek via Fireworks) reject empty required arrays
    // Only include 'required' if it has actual values
    const parameters: {
      type: 'object';
      properties: Record<
        string,
        {
          type?: string | string[];
          items?: unknown;
          description?: string;
          enum?: unknown[];
        }
      >;
      required?: string[];
    } = {
      type: 'object' as const,
      properties: (schema.properties || {}) as Record<
        string,
        {
          type?: string | string[];
          items?: unknown;
          description?: string;
          enum?: unknown[];
        }
      >,
    };

    // Only add required field if it exists AND has values
    if (schema.required && schema.required.length > 0) {
      parameters.required = schema.required;
    }

    logger.debug('Converting MCP tool to Ollama format', {
      name: tool.name,
      hasRequired: !!schema.required,
      requiredCount: schema.required?.length || 0,
    });

    return {
      type: 'function',
      function: {
        name: tool.name,
        description: tool.description || '', // Ensure description is never undefined
        parameters,
      },
    };
  });
}

/**
 * Processes content from Message to string format
 */
export function processMessageContent(content: Message['content']): string {
  if (!content) return '';
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .map((item) => {
        if ('text' in item) return item.text;
        if ('image_url' in item) return `[Image: ${item.image_url}]`;
        return '';
      })
      .join('\n');
  }
  return '';
}

/**
 * Converts a user message to Ollama format
 */
export function convertUserMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  const content = processMessageContent(message.content);
  logger.debug('Converting user message', {
    messageId: message.id,
    contentLength: content.length,
  });

  return {
    role: 'user',
    content,
  };
}

/**
 * Converts an assistant message to Ollama format
 */
export function convertAssistantMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  const result: SimpleOllamaMessage = {
    role: 'assistant',
    content: processMessageContent(message.content) || '',
  };

  if (message.tool_calls && message.tool_calls.length > 0) {
    result.tool_calls = message.tool_calls.map((tc) => {
      const callId = tc.id || generateToolCallId();
      const args =
        tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};

      logger.debug('Converting assistant tool call', {
        id: callId,
        name: tc.function.name,
        argsType: typeof tc.function.arguments,
      });

      return {
        id: callId,
        type: 'function' as const,
        function: {
          name: tc.function.name,
          arguments: args,
        },
      };
    });
  }

  return result;
}

/**
 * Converts a single Message to SimpleOllamaMessage
 */
export function convertMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  if (!message?.role) {
    logger.warn('Invalid message structure', { message });
    return null;
  }

  switch (message.role) {
    case 'user':
      return convertUserMessage(message, logger);

    case 'assistant':
      return convertAssistantMessage(message, logger);

    case 'system':
      return {
        role: 'system',
        content: processMessageContent(message.content) || '',
      };

    case 'tool':
      return {
        role: 'user',
        content: processMessageContent(message.content) || '',
        tool_call_id: message.tool_call_id,
      };

    default:
      logger.warn(`Unsupported message role: ${message.role}`);
      return null;
  }
}

/**
 * Converts array of Messages to Ollama format with optional system prompt
 */
export function convertToOllamaMessages(
  messages: Message[],
  systemPrompt?: string,
  logger: Logger = noopLogger,
): SimpleOllamaMessage[] {
  if (!Array.isArray(messages) || messages.length === 0) {
    throw new Error('Messages must be a non-empty array');
  }

  const ollamaMessages: SimpleOllamaMessage[] = [];

  if (systemPrompt?.trim()) {
    ollamaMessages.push({
      role: 'system',
      content: systemPrompt.trim(),
    });
    logger.debug('Added system prompt', {
      length: systemPrompt.trim().length,
    });
  }

  for (const message of messages) {
    const converted = convertMessage(message, logger);
    if (converted) {
      ollamaMessages.push(converted);
    }
  }

  logger.info('Converted messages to Ollama format', {
    inputCount: messages.length,
    outputCount: ollamaMessages.length,
  });

  return ollamaMessages;
}

/**
 * Processes a streaming chunk from Ollama API
 */
export function processChunk(
  chunk: unknown,
  logger: Logger = noopLogger,
): string | null {
  try {
    if (
      !chunk ||
      typeof chunk !== 'object' ||
      !('message' in chunk) ||
      !chunk.message ||
      typeof chunk.message !== 'object'
    ) {
      logger.debug('Chunk missing expected structure, skipping', {
        hasChunk: !!chunk,
        chunkType: typeof chunk,
        hasMessage: chunk && typeof chunk === 'object' && 'message' in chunk,
      });
      return null;
    }

    const message = chunk.message as {
      content?: string;
      thinking?: string;
      tool_calls?: Array<{
        id?: string;
        type: string;
        function: {
          name: string;
          arguments: Record<string, unknown> | string;
        };
      }>;
    };

    const result: {
      content?: string;
      thinking?: string;
      tool_calls?: Array<{
        id: string;
        type: string;
        function: {
          name: string;
          arguments: string;
        };
      }>;
      error?: string;
    } = {};

    if (message.content && typeof message.content === 'string') {
      // Ollama may include thinking in content field wrapped in <think> tags
      // Extract thinking and content separately
      const thinkMatch = message.content.match(
        /<think[^>]*>([\s\S]*?)<\/think>/i,
      );

      if (thinkMatch) {
        // Extract thinking content (without tags)
        const thinkingContent = thinkMatch[1].trim();
        if (thinkingContent) {
          result.thinking = thinkingContent;
          logger.debug('Thinking extracted from content field', {
            thinkingLength: thinkingContent.length,
          });
        }

        // Remove <think> block from content and clean up
        const contentWithoutThink = message.content
          .replace(/<think[^>]*>[\s\S]*?<\/think>/gi, '')
          .trim();

        if (contentWithoutThink) {
          result.content = contentWithoutThink;
          logger.debug('Content extracted (thinking removed)', {
            contentLength: contentWithoutThink.length,
          });
        }
      } else {
        // No thinking tags, use content as-is
        result.content = message.content;
        logger.debug('Content extracted from chunk', {
          contentLength: message.content.length,
        });
      }
    }

    if (message.thinking && typeof message.thinking === 'string') {
      // Remove <think> tags from Ollama's thinking content
      // Ollama returns thinking wrapped in <think>...</think> tags
      result.thinking = message.thinking
        .replace(/<think[^>]*>/gi, '') // Remove opening tag (with any attributes)
        .replace(/<\/think>/gi, '') // Remove closing tag
        .trim();

      logger.debug('Thinking extracted from chunk', {
        thinkingLength: result.thinking.length,
        hadTags: message.thinking !== result.thinking,
      });
    }

    if (message.tool_calls && Array.isArray(message.tool_calls)) {
      result.tool_calls = message.tool_calls.map((tc) => {
        const callId = tc.id || generateToolCallId();
        const args =
          typeof tc.function.arguments === 'string'
            ? (tryParse<Record<string, unknown>>(tc.function.arguments) ?? {})
            : tc.function.arguments;

        const formatted = formatToolCall(callId, tc.function.name, args);
        return {
          ...formatted,
          type: 'function' as const,
        };
      });

      logger.debug('Tool calls detected in chunk', {
        toolCallCount: result.tool_calls.length,
      });
    }

    if (result.content || result.thinking || result.tool_calls) {
      return JSON.stringify(result);
    }

    logger.debug('Chunk has no content, thinking, or tool_calls');
    return null;
  } catch (error: unknown) {
    logger.error('Failed to process chunk', { error, chunk });
    return JSON.stringify({ error: 'Failed to process response chunk' });
  }
}

/**
 * Checks if a model name supports tool calling
 */
export function getModelToolSupport(modelName: string): boolean {
  const toolSupportModels = [
    'llama3.1',
    'llama3.2',
    'qwen',
    'mistral',
    'dolphin',
    'deepseek',
  ];

  // Extract base model name (before colon if present)
  const baseName = modelName.split(':')[0].toLowerCase();

  return toolSupportModels.some((model) => baseName.includes(model));
}

/**
 * Determines reasoning parameter based on config
 */
export function determineThinkParam(
  enableReasoning: boolean,
  reasoningEffort?: 'low' | 'medium' | 'high',
  modelSupportsThinking: boolean = true,
  logger: Logger = noopLogger,
): boolean | 'low' | 'medium' | 'high' | undefined {
  if (!enableReasoning) {
    return undefined;
  }

  if (!modelSupportsThinking) {
    logger.debug('Model may not support thinking, but will try anyway');
  }

  const thinkParam = reasoningEffort || true;
  logger.info('Reasoning mode enabled', {
    thinkParam,
    modelSupportsThinking,
  });

  return thinkParam;
}
