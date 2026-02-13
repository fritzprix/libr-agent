import { Content, Part, createPartFromFunctionResponse } from '@google/genai';
import { Message } from '@/models/chat';
import { MCPContent } from '@/lib/mcp';
import { getLogger } from '../../logger';
import {
  processMessageContent,
  tryParse,
  generateToolCallId as generateId,
} from '../utils';

const logger = getLogger('GeminiMapper');

const GEMINI_DUMMY_THOUGHT_SIGNATURE = 'skip_thought_signature_validator';

/**
 * Generates a unique ID for a tool call.
 */
export function generateToolCallId(): string {
  return generateId();
}

/**
 * Attempts to parse tool result content into a structured object.
 * If parsing fails or content is not text, wraps it in a standard response object.
 */
export function tryParseResult(content: MCPContent[]): Record<string, unknown> {
  const text = processMessageContent(content);
  const parsed = tryParse<Record<string, unknown>>(text);
  if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    return parsed;
  }
  return { result: text };
}

/**
 * Converts an array of standard `Message` objects into the `Content` format
 * required by the Gemini API.
 */
export function convertToGeminiMessages(messages: Message[]): Content[] {
  if (messages.length === 0) return [];

  const toolCallNames = new Map<string, string>();
  let firstValidIndex = -1;

  // Phase 1: Identify start index (skip system messages)
  for (let i = 0; i < messages.length; i++) {
    const m = messages[i];

    // Collect tool names for O(1) lookup
    if (m.tool_calls) {
      for (const tc of m.tool_calls) {
        if (tc.id) toolCallNames.set(tc.id, tc.function.name);
      }
    }

    if (firstValidIndex === -1 && m.role !== 'system') {
      firstValidIndex = i;
    }
  }

  if (firstValidIndex === -1) {
    logger.warn('No valid messages found to start Gemini conversation');
    return [];
  }

  const geminiMessages: Content[] = [];

  // Phase 2: Convert messages starting from the first valid index
  for (let i = firstValidIndex; i < messages.length; i++) {
    const m = messages[i];

    logger.debug('🔄 Converting message to Gemini format', {
      index: i,
      role: m.role,
      hasToolCalls: !!m.tool_calls,
      toolCallsCount: m.tool_calls?.length,
      hasThinkingSignature: !!m.thinkingSignature,
      signaturePreview: m.thinkingSignature?.substring(0, 20),
    });

    if (m.role === 'system') continue;

    // Helper to identify a tool response message
    const isToolResponse = (msg: Message) =>
      msg.role === 'tool' || (msg.role === 'user' && !!msg.tool_call_id);

    if (isToolResponse(m)) {
      // Start of a tool response sequence - Look ahead and group ALL consecutive tool responses
      // Gemini strict requirement: Model (Calls) -> User (All Responses)
      // We must batch all responses into a SINGLE user message with multiple parts
      const responseParts: Part[] = [];
      let j = i;

      logger.info('🔧 Starting tool response batch sequence', {
        startIndex: i,
        firstMessageId: messages[i].id,
        firstMessageRole: messages[i].role,
        toolCallId: messages[i].tool_call_id,
      });

      // Consume all consecutive tool responses
      while (j < messages.length && isToolResponse(messages[j])) {
        const toolMsg = messages[j];
        if (toolMsg.tool_call_id) {
          const name = toolCallNames.get(toolMsg.tool_call_id);
          if (name) {
            const parsed = tryParseResult(toolMsg.content as MCPContent[]);
            responseParts.push(
              createPartFromFunctionResponse(
                toolMsg.tool_call_id,
                name, // Gemini relies on function name matching
                parsed,
              ),
            );
            logger.info('  - Added component to batch', {
              index: j,
              toolCallId: toolMsg.tool_call_id,
              name,
              parsedContentPreview: JSON.stringify(parsed).substring(0, 50),
            });
          } else {
            logger.warn(
              '⚠️ Skipping tool response with unknown ID, falling back to text part',
              {
                toolCallId: toolMsg.tool_call_id,
                availableIds: Array.from(toolCallNames.keys()),
              },
            );
            // Fallback: Add as a text part to salvage the message content
            const text = processMessageContent(toolMsg.content as MCPContent[]);
            responseParts.push({
              text,
            } as Part);
          }
        } else {
          logger.warn('⚠️ Skipping tool response with missing tool_call_id', {
            messageId: toolMsg.id,
            role: toolMsg.role,
          });
        }
        j++;
      }

      logger.info('✅ Finished batching tool responses', {
        totalParts: responseParts.length,
        consumedCount: j - i,
        nextIndex: j,
      });

      // Push the consolidated user message with all response parts
      if (responseParts.length > 0) {
        // Check if we can merge into previous user message (e.g. text + tool results)
        // But usually tool executions are distinct turns.
        // For safety, we keep it as a distinct message unless the last message was also User.
        const lastMsg = geminiMessages[geminiMessages.length - 1];
        // Determine if we should merge: only if last message was USER role.
        // If last message was MODEL (calls), this must be a NEW USER message.
        if (lastMsg && lastMsg.role === 'user') {
          if (!lastMsg.parts) lastMsg.parts = [];
          lastMsg.parts.push(...responseParts);
          logger.info(
            '➕ Merged tool response batch into previous user message',
            {
              batchSize: responseParts.length,
              totalParts: lastMsg.parts.length,
            },
          );
        } else {
          geminiMessages.push({
            role: 'user',
            parts: responseParts,
          });
          logger.info('📦 Created new user message for tool response batch', {
            batchSize: responseParts.length,
          });
        }
      } else {
        logger.error(
          '❌ Tool response batch resulted in NO parts! This WILL cause turn order errors.',
          {
            startIndex: i,
            endIndex: j,
          },
        );
      }

      // Advance the outer loop index to skip processed messages
      // j points to the first NON-tool-response message (or end of array)
      // The loop increments i, so set i to j - 1
      i = j - 1;
      continue;
    }

    if (m.role === 'user') {
      // Standard user text message (already handled tool responses above)
      // Merge with previous user message if possible to reduce turns
      const lastMsg = geminiMessages[geminiMessages.length - 1];
      const textPart = {
        text: processMessageContent(m.content as MCPContent[]),
      };

      if (lastMsg && lastMsg.role === 'user') {
        if (!lastMsg.parts) lastMsg.parts = [];
        lastMsg.parts.push(textPart);
      } else {
        geminiMessages.push({
          role: 'user',
          parts: [textPart],
        });
      }
    }
    if (m.role === 'assistant') {
      if (m.tool_calls && m.tool_calls.length > 0) {
        // Gemini 3 thought signature requirement:
        // - Sequential function calls: each step must include signature on first FC part
        // - Parallel function calls: only the first FC part needs the signature
        logger.debug('🔧 Converting tool calls with signature', {
          toolCallsCount: m.tool_calls.length,
          hasSignature: !!m.thinkingSignature,
          signature: m.thinkingSignature?.substring(0, 20),
        });

        const signatureForToolCalls =
          m.thinkingSignature ?? GEMINI_DUMMY_THOUGHT_SIGNATURE;

        if (!m.thinkingSignature) {
          logger.warn(
            'Missing thinking signature for assistant tool calls; applying Gemini dummy signature fallback',
            {
              toolCallsCount: m.tool_calls.length,
              fallback: GEMINI_DUMMY_THOUGHT_SIGNATURE,
            },
          );
        }

        geminiMessages.push({
          role: 'model',
          parts: m.tool_calls.map((tc, index) => {
            const args =
              tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
            const functionCallPart: {
              functionCall: {
                name: string;
                args: Record<string, unknown>;
              };
              thoughtSignature?: string;
            } = {
              functionCall: {
                name: tc.function.name,
                args,
              },
            };

            // Attach thought signature to the first function call part only
            if (index === 0) {
              functionCallPart.thoughtSignature = signatureForToolCalls;
              logger.debug('✅ Attached signature to first FC', {
                toolName: tc.function.name,
                signature: signatureForToolCalls.substring(0, 20) + '...',
                isFallback: !m.thinkingSignature,
              });
            }

            return functionCallPart;
          }),
        });
      } else if (m.content) {
        // Thought signatures in text responses (optional but recommended)
        const textPart: { text: string; thoughtSignature?: string } = {
          text: processMessageContent(m.content as MCPContent[]),
        };

        if (m.thinkingSignature) {
          textPart.thoughtSignature = m.thinkingSignature;
        }

        geminiMessages.push({
          role: 'model',
          parts: [textPart],
        });
      }
    }
  }

  logger.info(
    `Gemini conversion: ${messages.length} -> ${geminiMessages.length} messages`,
    {
      toolMappings: toolCallNames.size,
    },
  );

  return geminiMessages;
}

/**
 * Converts a single `Message` into the format expected by the Gemini API.
 */
export function convertSingleMessage(message: Message): unknown {
  logger.debug('🔄 convertSingleMessage called', {
    role: message.role,
    hasToolCalls: !!message.tool_calls,
    hasSignature: !!message.thinkingSignature,
  });

  if (message.role === 'system') {
    // System messages are handled separately in the API call
    return null;
  }

  if (message.role === 'user' && message.content) {
    return {
      role: 'user',
      parts: [{ text: processMessageContent(message.content as MCPContent[]) }],
    };
  } else if (message.role === 'assistant') {
    if (message.tool_calls && message.tool_calls.length > 0) {
      logger.debug('🔧 convertSingleMessage: processing tool calls', {
        count: message.tool_calls.length,
        hasSignature: !!message.thinkingSignature,
      });

      const signatureForToolCalls =
        message.thinkingSignature ?? GEMINI_DUMMY_THOUGHT_SIGNATURE;

      if (!message.thinkingSignature) {
        logger.warn(
          'convertSingleMessage: missing thinking signature for assistant tool calls; applying Gemini dummy signature fallback',
          {
            toolCallsCount: message.tool_calls.length,
            fallback: GEMINI_DUMMY_THOUGHT_SIGNATURE,
          },
        );
      }

      return {
        role: 'model',
        parts: message.tool_calls.map((tc, index) => {
          const args =
            tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
          const functionCallPart: {
            functionCall: {
              name: string;
              args: Record<string, unknown>;
            };
            thoughtSignature?: string;
          } = {
            functionCall: {
              name: tc.function.name,
              args,
            },
          };

          if (index === 0) {
            functionCallPart.thoughtSignature = signatureForToolCalls;
          }

          return functionCallPart;
        }),
      };
    } else if (message.content) {
      const textPart: { text: string; thoughtSignature?: string } = {
        text: processMessageContent(message.content as MCPContent[]),
      };

      if (message.thinkingSignature) {
        textPart.thoughtSignature = message.thinkingSignature;
      }

      return {
        role: 'model',
        parts: [textPart],
      };
    }
  } else if (message.role === 'tool') {
    // Convert tool message into a FunctionResponse part if possible.
    if (message.tool_calls && message.tool_calls.length > 0) {
      const parts = message.tool_calls.map((tc) => {
        const parsed =
          tryParse<Record<string, unknown>>(tc.function.arguments) ?? {};
        const id =
          tc.id && typeof tc.id === 'string' ? tc.id : generateToolCallId();
        return createPartFromFunctionResponse(id, tc.function.name, parsed);
      });
      return {
        role: 'user',
        parts,
      };
    }
    if (message.content) {
      return {
        role: 'user',
        parts: [
          { text: processMessageContent(message.content as MCPContent[]) },
        ],
      };
    }
    return null;
  }
  return null;
}
