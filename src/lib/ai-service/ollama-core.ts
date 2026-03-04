/**
 * Ollama Core Logic (Pure Functions)
 *
 * This module contains the core business logic for Ollama API interactions,
 * extracted as pure functions with injectable logger for testing.
 * No Tauri/browser dependencies - can be tested in Node.js environment.
 */

import type { Tool } from 'ollama';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { TokenUsage } from './types';
import {
  tryParse,
  formatToolCall,
  generateToolCallId,
  processMultiModalContent,
} from './utils';

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
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  /** Base64-encoded images for vision-capable models (e.g. LLaVA, llama3.2-vision) */
  images?: string[];
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
 * @param mcpTools An array of generic MCP tools.
 * @param logger Logger instance to track events.
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
 * @param content The content of the message.
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
 * Converts a user message to Ollama format.
 * For vision-capable models, image content is extracted from `message.content`
 * and placed in the `images` field as base64 strings (Ollama native API format).
 * @param message The chat message object.
 * @param logger Logger instance to track events.
 */
export function convertUserMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  const multimodal = processMultiModalContent(message.content);

  // Separate text and image parts
  const textParts = multimodal.filter((p) => p.type === 'text');
  const imageParts = multimodal.filter((p) => p.type === 'image');

  // Build text content from text parts only
  const content = textParts.map((p) => p.text ?? '').join('\n');

  // Extract base64 image data strings
  const images = imageParts
    .map((p) => p.image)
    .filter((img): img is string => typeof img === 'string' && img.length > 0);

  logger.debug('Converting user message', {
    messageId: message.id,
    contentLength: content.length,
    imageCount: images.length,
  });

  const result: SimpleOllamaMessage = { role: 'user', content };
  if (images.length > 0) {
    result.images = images;
  }
  return result;
}

/**
 * Converts an assistant message to Ollama format
 * @param message The chat message object.
 * @param logger Logger instance to track events.
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
 * @param message The chat message object.
 * @param logger Logger instance to track events.
 */
export function convertMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  if (!message?.role) {
    logger.warn('Invalid message structure', { message });
    return null;
  }

  logger.debug(`🔄 Converting message: role=${message.role}, id=${message.id}`);

  switch (message.role) {
    case 'user': {
      const userResult = convertUserMessage(message, logger);
      logger.debug('User message converted', {
        messageId: message.id,
        hasResult: !!userResult,
        contentLength: userResult?.content?.length ?? 0,
      });
      return userResult;
    }

    case 'assistant': {
      const assistantResult = convertAssistantMessage(message, logger);
      logger.debug('Assistant message converted', {
        messageId: message.id,
        hasResult: !!assistantResult,
        hasToolCalls: !!assistantResult?.tool_calls,
        toolCallCount: assistantResult?.tool_calls?.length ?? 0,
      });
      return assistantResult;
    }

    case 'system': {
      const systemContent = processMessageContent(message.content) || '';
      logger.debug('System message converted', {
        messageId: message.id,
        contentLength: systemContent.length,
      });
      return {
        role: 'system',
        content: systemContent,
      };
    }

    case 'tool': {
      const toolContent = processMessageContent(message.content) || '';
      logger.debug('🔧 Tool message converted', {
        messageId: message.id,
        toolCallId: message.tool_call_id,
        contentLength: toolContent.length,
        contentPreview: toolContent.substring(0, 100),
      });
      return {
        role: 'tool',
        content: toolContent,
        tool_call_id: message.tool_call_id,
      };
    }

    default:
      logger.warn(`Unsupported message role: ${message.role}`);
      return null;
  }
}

/**
 * Converts array of Messages to Ollama format with optional system prompt
 * @param messages Array of chat messages.
 * @param systemPrompt The high level instruction given to the model.
 * @param logger Logger instance to track events.
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

  let skippedCount = 0;
  for (const message of messages) {
    const converted = convertMessage(message, logger);
    if (converted) {
      ollamaMessages.push(converted);
    } else {
      skippedCount++;
      logger.warn('⚠️ Message conversion returned null - SKIPPED', {
        messageId: message.id,
        role: message.role,
        hasContent: !!message.content,
        contentLength: message.content?.length ?? 0,
        hasToolCalls: !!message.tool_calls,
        toolCallId: message.tool_call_id,
      });
    }
  }

  logger.info('Converted messages to Ollama format', {
    inputCount: messages.length,
    outputCount: ollamaMessages.length,
    skippedCount,
  });

  return ollamaMessages;
}

/**
 * Processed chunk result containing content, tool calls, thinking, or usage metrics
 */
export interface ProcessedChunk {
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
  usage?: TokenUsage;
  error?: string;
}

/**
 * Tool call accumulator for handling partial JSON across multiple chunks
 * @internal
 */
export interface OllamaToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean;
  lastChunkTime: number;
}

// Maximum JSON buffer size (200KB)
const MAX_PARTIAL_TOOL_INPUT_LENGTH = 200_000;

// Accumulator timeout (30 seconds)
const MAX_ACCUMULATOR_AGE_MS = 30_000;

/**
 * Processes a streaming chunk from Ollama API with partial JSON accumulation
 * @param chunk A single chunk from the stream.
 * @param logger Logger instance to track events.
 * @param accumulators Aggregated state of ongoing multipart tool calls.
 */
export function processChunk(
  chunk: unknown,
  logger: Logger = noopLogger,
  accumulators?: Map<number, OllamaToolCallAccumulator>,
): ProcessedChunk | null {
  try {
    if (!chunk || typeof chunk !== 'object') {
      return null;
    }

    const c = chunk as Record<string, unknown>;
    const result: ProcessedChunk = {};

    // 1. Extract Usage Metrics (Final Chunk)
    if (c.done === true) {
      const promptTokens = Number(c.prompt_eval_count) || 0;
      const completionTokens = Number(c.eval_count) || 0;

      result.usage = {
        promptTokens,
        completionTokens,
        totalTokens: promptTokens + completionTokens,
        details: {
          promptEvalDuration: Number(c.prompt_eval_duration) / 1_000_000, // ns to ms
          evalDuration: Number(c.eval_duration) / 1_000_000, // ns to ms
          totalDuration: Number(c.total_duration) / 1_000_000, // ns to ms
          loadDuration: Number(c.load_duration) / 1_000_000, // ns to ms
        },
      };

      logger.info('📊 Ollama usage metrics extracted', {
        inputTokens: result.usage.promptTokens,
        outputTokens: result.usage.completionTokens,
        totalTokens: result.usage.totalTokens,
        evalDurationMs: result.usage.details?.evalDuration?.toFixed(2),
      });
    }

    // 2. Extract Message Content (Streaming Chunks)
    if ('message' in c && c.message && typeof c.message === 'object') {
      const message = c.message as {
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

      // DIAGNOSTIC LOGGING: Log keys to see if we are missing anything
      const messageKeys = Object.keys(message);
      if (
        !message.content &&
        !message.tool_calls &&
        !message.thinking &&
        messageKeys.length > 0
      ) {
        logger.warn('⚠️ Chunk has message but no known fields', {
          keys: messageKeys,
          rawMessage: JSON.stringify(message).substring(0, 200),
        });
      }

      if (message.content && typeof message.content === 'string') {
        // Ollama may include thinking in content field wrapped in <think> tags
        // Extract thinking and content separately
        const thinkMatch = message.content.match(
          /<think[^>]*>([\s\S]*?)<\/think>/i,
        );

        if (thinkMatch) {
          // Extract thinking content (without tags)
          const thinkingContent = thinkMatch[1];
          if (thinkingContent) {
            result.thinking = thinkingContent;
            logger.debug('Thinking extracted from content field', {
              thinkingLength: thinkingContent.length,
            });
          }

          // Remove <think> block from content and clean up
          const contentWithoutThink = message.content.replace(
            /<think[^>]*>[\s\S]*?<\/think>/gi,
            '',
          );

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
          .replace(/<\/think>/gi, '');

        logger.debug('Thinking extracted from chunk', {
          thinkingLength: result.thinking.length,
          hadTags: message.thinking !== result.thinking,
        });
      }

      if (message.tool_calls && Array.isArray(message.tool_calls)) {
        const processedToolCalls: Array<{
          id: string;
          type: 'function';
          function: {
            name: string;
            arguments: string;
          };
        }> = [];

        for (const [idx, tc] of message.tool_calls.entries()) {
          const callId = tc.id || generateToolCallId();

          // Get or create accumulator for this tool call
          let accumulator = accumulators?.get(idx);
          if (!accumulator) {
            accumulator = {
              id: callId,
              name: tc.function.name,
              partialJson: '',
              index: idx,
              yielded: false,
              lastChunkTime: Date.now(),
            };
            accumulators?.set(idx, accumulator);
          }

          // Check for timeout (stale accumulator)
          const age = Date.now() - accumulator.lastChunkTime;
          if (age > MAX_ACCUMULATOR_AGE_MS) {
            logger.warn('Tool call accumulator timeout, discarding', {
              id: accumulator.id,
              name: accumulator.name,
              ageMs: age,
            });
            accumulators?.delete(idx);
            continue;
          }

          // Update timestamp
          accumulator.lastChunkTime = Date.now();

          // Handle string arguments (potential partial JSON)
          if (typeof tc.function.arguments === 'string') {
            accumulator.partialJson += tc.function.arguments;

            // Buffer size limit
            if (
              accumulator.partialJson.length > MAX_PARTIAL_TOOL_INPUT_LENGTH
            ) {
              logger.error('Tool call JSON exceeded buffer limit', {
                id: accumulator.id,
                name: accumulator.name,
                length: accumulator.partialJson.length,
              });
              accumulators?.delete(idx);
              continue;
            }

            // Attempt to parse accumulated JSON
            const trimmedJson = accumulator.partialJson.trim();
            if (trimmedJson.length === 0) {
              logger.debug('No complete JSON fragment yet; waiting', {
                index: idx,
                id: accumulator.id,
              });
              continue;
            }

            try {
              const parsed = JSON.parse(trimmedJson) as Record<string, unknown>;

              // Success! Add the tool call if not already yielded
              if (!accumulator.yielded) {
                const formatted = formatToolCall(
                  callId,
                  tc.function.name,
                  parsed,
                );
                processedToolCalls.push({
                  ...formatted,
                  type: 'function' as const,
                });
                accumulator.yielded = true;
                logger.info(
                  'Tool call successfully parsed from accumulated JSON',
                  {
                    id: callId,
                    name: tc.function.name,
                    jsonLength: trimmedJson.length,
                  },
                );
              }
            } catch {
              // JSON still incomplete, continue accumulating
              logger.debug('JSON incomplete, waiting for more chunks', {
                id: callId,
                name: tc.function.name,
                currentLength: accumulator.partialJson.length,
              });
            }
          } else {
            // Already parsed object (complete)
            const formatted = formatToolCall(
              callId,
              tc.function.name,
              tc.function.arguments,
            );
            processedToolCalls.push({
              ...formatted,
              type: 'function' as const,
            });
            logger.debug('Tool call already parsed', {
              id: callId,
              name: tc.function.name,
            });
          }
        }

        if (processedToolCalls.length > 0) {
          result.tool_calls = processedToolCalls;
          logger.debug('Tool calls detected in chunk', {
            toolCallCount: processedToolCalls.length,
          });
        }
      }
    }

    if (
      result.content ||
      result.thinking ||
      result.tool_calls ||
      result.usage
    ) {
      return result;
    }

    logger.debug('Chunk has no content, thinking, tool_calls or usage', {
      rawChunk: JSON.stringify(chunk),
    });
    return null;
  } catch (error: unknown) {
    logger.error('Failed to process chunk', { error, chunk });
    // For error, we return an object with error property now
    return { error: 'Failed to process response chunk' };
  }
}

/**
 * Checks if a model name supports tool calling
 * @param modelName The identifier of the model.
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
 * @param enableReasoning True if reasoning output should be enabled.
 * @param reasoningEffort The requested effort or tokens given to the model to reason.
 * @param modelSupportsThinking True if the given model natively supports thinking mechanisms.
 * @param logger Logger instance to track events.
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
