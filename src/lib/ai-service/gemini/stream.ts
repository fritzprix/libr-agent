import { formatToolCall } from '../utils';
import { generateToolCallId } from './mapper';
import { getLogger } from '../../logger';
import type { TokenUsage } from '../types';
import {
  createSerializableDirectToolCall,
  serializeStreamContent,
  serializeStreamThinking,
  serializeStreamUsage,
  serializeDirectToolCalls,
  serializeThinkingSignature,
} from '../stream-events';

// Type definition for Gemini Experimental Thoughts
// See: https://github.com/google/generative-ai-js/issues/186
interface GeminiThoughtChunk {
  candidates?: Array<{
    content?: {
      parts?: Array<
        | {
            thought?: boolean; // Sometimes boolean flag?
            text?: string;
          }
        | {
            // Another possible schema seen in discussions
            thought?: string;
          }
      >;
    };
  }>;
  // Possible schema for direct parts
  parts?: Array<{
    thought?: string;
  }>;
}

interface GeminiFunctionCall {
  id?: string;
  name?: string;
  args?: unknown;
}

interface GeminiChunkPart {
  functionCall?: GeminiFunctionCall;
  thoughtSignature?: string;
}

interface GeminiChunkCandidate {
  content?: {
    parts?: GeminiChunkPart[];
  };
  finishReason?: string;
  safetyRatings?: unknown;
}

interface GeminiUsageMetadata {
  promptTokenCount?: number;
  candidatesTokenCount?: number;
  cachedContentTokenCount?: number;
  thoughtsTokenCount?: number;
  toolUsePromptTokenCount?: number;
  totalTokenCount?: number;
}

interface GeminiStreamChunk {
  usageMetadata?: GeminiUsageMetadata;
  candidates?: GeminiChunkCandidate[];
  text?: string;
}
function isFunctionCallPart(part: GeminiChunkPart): part is GeminiChunkPart & {
  functionCall: GeminiFunctionCall;
} {
  return typeof part.functionCall === 'object' && part.functionCall !== null;
}

/**
 * Processes the Gemini stream result and yields formatted JSON strings.
 * @param result Result stream from the provider API.
 * @param signal Optional AbortSignal to cancel the stream.
 * @param logger Logger instance to track events.
 */
export async function* processGeminiStream(
  result: AsyncIterable<GeminiStreamChunk>,
  signal: AbortSignal,
  logger: ReturnType<typeof getLogger>,
): AsyncGenerator<string, void, void> {
  if (signal.aborted) {
    logger.debug('Stream aborted before iteration');
    return;
  }

  // Measure TTFT (Gemini doesn't provide native prefill timing)
  const startTime = performance.now();
  let firstChunkReceived = false;

  // Track cumulative usage metadata across chunks
  const currentUsage: TokenUsage & {
    details: Record<string, unknown>;
  } = {
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
    details: {} as Record<string, unknown>,
  };
  let usageUpdateCount = 0;

  for await (const chunk of result) {
    if (signal.aborted) {
      logger.info('Stream aborted during iteration');
      break;
    }

    // Inject TTFT metric on first chunk
    if (!firstChunkReceived) {
      const ttft = performance.now() - startTime;
      firstChunkReceived = true;
      currentUsage.details.timeToFirstToken = ttft;
    }

    // Extract usage metadata from chunk if available
    if (chunk.usageMetadata) {
      let usageUpdated = false;

      if (chunk.usageMetadata.promptTokenCount !== undefined) {
        currentUsage.promptTokens = chunk.usageMetadata.promptTokenCount;
        usageUpdated = true;
      }

      if (chunk.usageMetadata.candidatesTokenCount !== undefined) {
        currentUsage.completionTokens =
          chunk.usageMetadata.candidatesTokenCount;
        usageUpdated = true;
      }

      // Add cached content tokens if present
      if (chunk.usageMetadata.cachedContentTokenCount !== undefined) {
        currentUsage.cachedPromptTokens =
          chunk.usageMetadata.cachedContentTokenCount;
        currentUsage.details.cachedContentTokenCount =
          chunk.usageMetadata.cachedContentTokenCount;
        usageUpdated = true;
      }

      if (chunk.usageMetadata.toolUsePromptTokenCount !== undefined) {
        currentUsage.details.toolUsePromptTokenCount =
          chunk.usageMetadata.toolUsePromptTokenCount;
        usageUpdated = true;
      }

      if (chunk.usageMetadata.totalTokenCount !== undefined) {
        currentUsage.details.providerReportedTotalTokenCount =
          chunk.usageMetadata.totalTokenCount;
        usageUpdated = true;
      }

      // Add thinking tokens if present (for reasoning models)
      if (chunk.usageMetadata.thoughtsTokenCount !== undefined) {
        currentUsage.details.thoughtsTokenCount =
          chunk.usageMetadata.thoughtsTokenCount;
        usageUpdated = true;
      }

      if (usageUpdated) {
        currentUsage.totalTokens =
          currentUsage.promptTokens + currentUsage.completionTokens;
        usageUpdateCount += 1;
        logger.info('Gemini usage metadata update', {
          usageUpdateCount,
          promptTokenCount: chunk.usageMetadata.promptTokenCount,
          cachedContentTokenCount: chunk.usageMetadata.cachedContentTokenCount,
          candidatesTokenCount: chunk.usageMetadata.candidatesTokenCount,
          thoughtsTokenCount: chunk.usageMetadata.thoughtsTokenCount,
          toolUsePromptTokenCount: chunk.usageMetadata.toolUsePromptTokenCount,
          providerReportedTotalTokenCount: chunk.usageMetadata.totalTokenCount,
          normalizedPromptTokens: currentUsage.promptTokens,
          normalizedCompletionTokens: currentUsage.completionTokens,
          normalizedCachedPromptTokens: currentUsage.cachedPromptTokens,
          normalizedComputedTotalTokens: currentUsage.totalTokens,
        });
        yield serializeStreamUsage(currentUsage);
      }
    }

    const thoughtChunk = chunk as unknown as GeminiThoughtChunk;
    let thoughtContent = '';

    // Attempt to find thoughts in candidates
    if (thoughtChunk.candidates?.[0]?.content?.parts) {
      for (const part of thoughtChunk.candidates[0].content.parts) {
        // Schema 1: part has 'thought' property with string
        if ('thought' in part && typeof part.thought === 'string') {
          thoughtContent += part.thought;
        }
        // Schema 2: part has 'thoughtSignature'
        if (
          'thoughtSignature' in part &&
          typeof part.thoughtSignature === 'string'
        ) {
          // Yield the signature as a separate event or combined with thinking?
          // Based on Message model, we have `thinkingSignature` field.
          yield serializeThinkingSignature(part.thoughtSignature);
        }
      }
    }

    // Attempt to find thoughts in top-level parts (sometimes seen in simplified chunks)
    if (thoughtChunk.parts) {
      for (const part of thoughtChunk.parts) {
        if (typeof part.thought === 'string') {
          thoughtContent += part.thought;
        }
      }
    }

    if (thoughtContent) {
      yield serializeStreamThinking(thoughtContent);
    }

    // Extract function calls from raw parts to preserve thoughtSignature
    // The SDK's chunk.functionCalls loses the signature which is a sibling property
    const candidates = chunk.candidates || [];
    const candidate = candidates[0];
    let extractedSignature: string | undefined;

    logger.debug('🔍 Processing chunk for function calls', {
      hasCandidates: !!candidate,
      hasParts: !!candidate?.content?.parts,
      partsCount: candidate?.content?.parts?.length,
    });

    if (candidate?.content?.parts) {
      const functionCallParts =
        candidate.content.parts.filter(isFunctionCallPart);

      if (functionCallParts.length > 0) {
        logger.debug('🔍 Found function call parts', {
          count: functionCallParts.length,
          parts: functionCallParts.map((p, i: number) => ({
            index: i,
            hasSignature: 'thoughtSignature' in p,
            signatureValue:
              'thoughtSignature' in p
                ? (p as { thoughtSignature?: string }).thoughtSignature
                : undefined,
            functionName: p.functionCall?.name,
          })),
        });

        const toolCalls = functionCallParts
          .map((part, index: number) => {
            const fc = part.functionCall;
            if (!fc || !fc.name) return null;

            // Capture thoughtSignature from the FIRST function call part only
            // Per Gemini docs: parallel calls have signature only on first part
            if (
              index === 0 &&
              'thoughtSignature' in part &&
              typeof part.thoughtSignature === 'string'
            ) {
              extractedSignature = part.thoughtSignature;
              logger.debug('✅ Captured thought signature from first FC', {
                signature: extractedSignature?.substring(0, 20) + '...',
              });
            }

            const callId =
              fc.id && typeof fc.id === 'string' && fc.id.length > 0
                ? fc.id
                : generateToolCallId();
            const formattedToolCall = formatToolCall(
              callId,
              fc.name,
              fc.args ?? {},
            );

            return createSerializableDirectToolCall(
              formattedToolCall.id,
              formattedToolCall.function.name,
              formattedToolCall.function.arguments,
            );
          })
          .filter(
            (tc): tc is ReturnType<typeof createSerializableDirectToolCall> =>
              tc !== null,
          );

        if (toolCalls.length > 0) {
          logger.debug('📤 Emitting tool calls', {
            count: toolCalls.length,
            hasSignature: !!extractedSignature,
          });
          yield serializeDirectToolCalls(toolCalls);

          // Emit the captured signature separately
          if (extractedSignature) {
            logger.debug('📤 Emitting thought signature', {
              signature: extractedSignature.substring(0, 20) + '...',
            });
            yield serializeThinkingSignature(extractedSignature);
          }
        }
      } else if (chunk.text) {
        yield serializeStreamContent(chunk.text);
      } else {
        const finishReason = candidate?.finishReason;

        if (finishReason === 'UNEXPECTED_TOOL_CALL') {
          logger.warn(
            'Gemini stream ended with UNEXPECTED_TOOL_CALL. The model attempted to call a tool that was not properly defined or permitted in this context.',
            { chunk, finishReason },
          );
        } else if (finishReason === 'STOP') {
          logger.debug('Gemini stream stopped normally with empty chunk', {
            chunk,
          });
        } else {
          logger.warn('Gemini chunk has no text or functionCalls', {
            chunk,
            finishReason,
            safetyRatings: candidate?.safetyRatings,
          });
        }
      }
    } else if (chunk.text) {
      yield serializeStreamContent(chunk.text);
    }
  }
}
