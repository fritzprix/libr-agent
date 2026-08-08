import type { Message, MessageError, ToolCall } from '@/models/chat';
import type { MCPContent, MCPTextContent } from '@/lib/mcp';
import type { TokenUsage } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';
import {
  extractThinkingText,
  extractToolCalls,
  hasRenderableAssistantOutput,
} from '../streaming-message-utils';

const logger = getLogger('completion-validators');

export function createExecutionError(
  type: MessageError['type'],
  displayMessage: string,
  originalError: unknown,
  context?: Record<string, unknown>,
): MessageError {
  return {
    type,
    displayMessage,
    recoverable: true,
    details: {
      originalError,
      timestamp: new Date().toISOString(),
      context,
    },
  };
}

export interface FinalizeCompletionParams {
  sessionId: string;
  responseMessageId: string;
  content: MCPContent[];
  streamingToolCalls: ToolCall[];
  currentThinkingText?: string;
  thinkingSignature?: string;
  thinkingStartTime?: number;
  finalUsage?: TokenUsage;
  startTime: number;
  endTime: number;
  firstChunkTime?: number;
}

export function validateAndFinalizeMessage({
  sessionId,
  responseMessageId,
  content,
  streamingToolCalls,
  currentThinkingText,
  thinkingSignature,
  thinkingStartTime,
  finalUsage,
  startTime,
  endTime,
  firstChunkTime,
}: FinalizeCompletionParams): Message {
  const totalDurationMs = endTime - startTime;

  if (finalUsage && finalUsage.completionTokens > 0) {
    if (!finalUsage.details) {
      finalUsage.details = {};
    }
    if (!finalUsage.details.evalDuration) {
      if (firstChunkTime) {
        finalUsage.details.promptEvalDuration = firstChunkTime - startTime;
        finalUsage.details.evalDuration = endTime - firstChunkTime;
      } else {
        finalUsage.details.evalDuration = totalDurationMs;
      }
    }
    if (!finalUsage.details.timeToFirstToken && firstChunkTime) {
      finalUsage.details.timeToFirstToken = firstChunkTime - startTime;
    }
  }

  const finalToolCalls: ToolCall[] =
    streamingToolCalls.length > 0
      ? streamingToolCalls
      : extractToolCalls(content);
  const finalThinking = currentThinkingText ?? extractThinkingText(content);

  const finalMessage: Message = {
    id: responseMessageId,
    sessionId,
    threadId: sessionId,
    role: 'assistant',
    content,
    createdAt: new Date(),
    tool_calls: finalToolCalls.length > 0 ? finalToolCalls : undefined,
    thinking: finalThinking,
    thinkingSignature,
    thinkingTime: thinkingStartTime
      ? (performance.now() - thinkingStartTime) / 1000
      : undefined,
    usage: finalUsage,
    isStreaming: false,
  };

  logger.info('Completion request completed', {
    sessionId,
    contentLength: content.length,
    toolCallCount: finalToolCalls.length,
    finalUsage: finalUsage
      ? {
          promptTokens: finalUsage.promptTokens,
          completionTokens: finalUsage.completionTokens,
          totalTokens: finalUsage.totalTokens,
          cachedPromptTokens: finalUsage.cachedPromptTokens,
          details: finalUsage.details,
        }
      : undefined,
  });

  const hasRenderableContent = finalMessage.content.some((item) =>
    item.type === 'text'
      ? !!(item as MCPTextContent).text?.trim()
      : item.type !== 'thinking',
  );
  const hasToolCalls = !!finalMessage.tool_calls?.length;
  const hasThinking = !!finalMessage.thinking;

  if (hasThinking && !hasRenderableContent && !hasToolCalls) {
    logger.warn(
      'Thinking-only completion detected; forwarding to Rust recovery',
      {
        sessionId,
        thinkingLength: finalMessage.thinking?.length,
      },
    );
  }

  const hasContent = hasRenderableAssistantOutput(finalMessage);
  const hasUsage =
    finalMessage.usage && finalMessage.usage.completionTokens > 0;

  if (!hasContent && !hasUsage) {
    logger.error('❌ Empty response detected', {
      sessionId,
      finalMessage: {
        ...finalMessage,
        content: finalMessage.content?.length,
      },
      hasContent,
      hasUsage,
    });
    throw createExecutionError(
      'AI_SERVICE_ERROR',
      'Received empty response from LLM provider',
      'empty_response_from_provider',
      { sessionId },
    );
  } else if (!hasContent && hasUsage) {
    logger.warn(
      '⚠️ Response has usage but no content - treating as empty response',
      {
        sessionId,
        usage: finalMessage.usage,
      },
    );
    throw createExecutionError(
      'AI_SERVICE_ERROR',
      'Received empty response from LLM provider',
      'empty_response_from_provider',
      { sessionId },
    );
  }

  return finalMessage;
}
