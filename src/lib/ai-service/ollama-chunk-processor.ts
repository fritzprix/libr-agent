import { formatToolCall, generateToolCallId } from './utils';
import {
  noopLogger,
  type Logger,
  type OllamaToolCallAccumulator,
  type ProcessedChunk,
} from './ollama-core-types';

const MAX_PARTIAL_TOOL_INPUT_LENGTH = 200_000;
const MAX_ACCUMULATOR_AGE_MS = 30_000;

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

    if (c.done === true) {
      const promptTokens = Number(c.prompt_eval_count) || 0;
      const completionTokens = Number(c.eval_count) || 0;

      result.usage = {
        promptTokens,
        completionTokens,
        totalTokens: promptTokens + completionTokens,
        details: {
          promptEvalDuration: Number(c.prompt_eval_duration) / 1_000_000,
          evalDuration: Number(c.eval_duration) / 1_000_000,
          totalDuration: Number(c.total_duration) / 1_000_000,
          loadDuration: Number(c.load_duration) / 1_000_000,
        },
      };

      logger.info('📊 Ollama usage metrics extracted', {
        inputTokens: result.usage.promptTokens,
        outputTokens: result.usage.completionTokens,
        totalTokens: result.usage.totalTokens,
        evalDurationMs: result.usage.details?.evalDuration?.toFixed(2),
      });
    }

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
        const thinkMatch = message.content.match(
          /<think[^>]*>([\s\S]*?)<\/think>/i,
        );

        if (thinkMatch) {
          const thinkingContent = thinkMatch[1];
          if (thinkingContent) {
            result.thinking = thinkingContent;
            logger.debug('Thinking extracted from content field', {
              thinkingLength: thinkingContent.length,
            });
          }

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
          result.content = message.content;
          logger.debug('Content extracted from chunk', {
            contentLength: message.content.length,
          });
        }
      }

      if (message.thinking && typeof message.thinking === 'string') {
        result.thinking = message.thinking
          .replace(/<think[^>]*>/gi, '')
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

        for (const [idx, toolCall] of message.tool_calls.entries()) {
          const callId = toolCall.id || generateToolCallId();

          let accumulator = accumulators?.get(idx);
          if (!accumulator) {
            accumulator = {
              id: callId,
              name: toolCall.function.name,
              partialJson: '',
              index: idx,
              yielded: false,
              lastChunkTime: Date.now(),
            };
            accumulators?.set(idx, accumulator);
          }

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

          accumulator.lastChunkTime = Date.now();

          if (typeof toolCall.function.arguments === 'string') {
            accumulator.partialJson += toolCall.function.arguments;

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

              if (!accumulator.yielded) {
                const formatted = formatToolCall(
                  callId,
                  toolCall.function.name,
                  parsed,
                );
                processedToolCalls.push({
                  ...formatted,
                  type: 'function',
                });
                accumulator.yielded = true;
                logger.info(
                  'Tool call successfully parsed from accumulated JSON',
                  {
                    id: callId,
                    name: toolCall.function.name,
                    jsonLength: trimmedJson.length,
                  },
                );
              }
            } catch {
              logger.debug('JSON incomplete, waiting for more chunks', {
                id: callId,
                name: toolCall.function.name,
                currentLength: accumulator.partialJson.length,
              });
            }
          } else {
            const formatted = formatToolCall(
              callId,
              toolCall.function.name,
              toolCall.function.arguments,
            );
            processedToolCalls.push({
              ...formatted,
              type: 'function',
            });
            logger.debug('Tool call already parsed', {
              id: callId,
              name: toolCall.function.name,
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
    return { error: 'Failed to process response chunk' };
  }
}
