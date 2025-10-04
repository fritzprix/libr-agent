import { createId } from '@paralleldrive/cuid2';
import { Message } from '@/models/chat';
import { stringToMCPContentArray } from '@/lib/utils';
import { MCPContent } from '@/lib/mcp-types';

/**
 * Creates a system message object.
 * System messages are typically used to provide instructions or context to the AI model.
 *
 * @param text The content of the system message.
 * @param sessionId The ID of the session this message belongs to.
 * @param assistantId Optional ID of the assistant associated with this message.
 * @param source Optional source indicator - 'assistant' for AI-generated, 'ui' for user interface interactions.
 * @returns A message object with the role 'system'.
 */
export const createSystemMessage = (
  text: string,
  sessionId: string,
  assistantId?: string,
  source?: 'assistant' | 'ui',
): Message => ({
  id: createId(),
  content: stringToMCPContentArray(text),
  role: 'system',
  sessionId,
  assistantId,
  source,
});

/**
 * Creates a user message object.
 * User messages represent the input from the end-user in a conversation.
 *
 * @param text The content of the user's message.
 * @param sessionId The ID of the session this message belongs to.
 * @param assistantId Optional ID of the assistant associated with this message.
 * @param source Optional source indicator - 'assistant' for AI-generated, 'ui' for user interface interactions.
 * @returns A message object with the role 'user'.
 */
export const createUserMessage = (
  text: string,
  sessionId: string,
  assistantId?: string,
  source?: 'assistant' | 'ui',
): Message => ({
  id: createId(),
  content: stringToMCPContentArray(text),
  role: 'user',
  sessionId,
  assistantId,
  source,
});

/**
 * Creates a tool message object.
 * Tool messages are used to provide the result of a tool execution back to the AI model.
 * This function ensures that the `tool_call_id` is present, which is required by AI services.
 *
 * @param content The content of the tool's result, as an array of MCPContent objects.
 * @param toolCallId The ID of the tool call that this message is a result of. This is mandatory.
 * @param sessionId The ID of the session this message belongs to.
 * @param assistantId Optional ID of the assistant associated with this message.
 * @returns A message object with the role 'tool'.
 * @throws Will throw an error if `toolCallId` is not provided.
 */
export const createToolMessage = (
  content: MCPContent[],
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
): Message => {
  if (!toolCallId) {
    throw new Error('tool_call_id is required for tool messages');
  }

  return {
    id: createId(),
    content,
    role: 'tool',
    tool_call_id: toolCallId,
    sessionId,
    assistantId,
  };
};

/**
 * Creates a tool message indicating a successful tool execution.
 * This is a convenience wrapper around `createToolMessage` that formats
 * the result string with a success indicator.
 *
 * @param result The successful result string from the tool execution.
 * @param toolCallId The ID of the tool call that this message is a result of.
 * @param sessionId The ID of the session this message belongs to.
 * @param assistantId Optional ID of the assistant associated with this message.
 * @returns A message object with the role 'tool' and a formatted success message.
 */
export const createToolSuccessMessage = (
  result: string,
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
): Message =>
  createToolMessage(
    stringToMCPContentArray(`✅ ${result}`),
    toolCallId,
    sessionId,
    assistantId,
  );

/**
 * Creates a pair of messages for a tool call and its result.
 * This is designed for an atomic tool chain pattern, where the tool call
 * from the assistant and the corresponding tool result are created together.
 *
 * @param toolName The name of the tool that was called.
 * @param params The parameters that were passed to the tool.
 * @param result The result of the tool execution, as an array of MCPContent objects.
 * @param toolCallId The unique ID for this tool call.
 * @param sessionId The ID of the session this message pair belongs to.
 * @param assistantId Optional ID of the assistant associated with this message pair.
 * @param source Optional source indicator - 'assistant' for AI-generated, 'ui' for user interface interactions.
 * @param delayMs Optional delay in milliseconds between call and result message timestamps.
 * @returns A tuple containing two message objects: the assistant's tool call message and the tool result message.
 */
export const createToolMessagePair = (
  toolName: string,
  params: Record<string, unknown>,
  result: MCPContent[],
  toolCallId: string,
  sessionId: string,
  assistantId?: string,
  source?: 'assistant' | 'ui',
  delayMs?: number,
): [Message, Message] => {
  const delayMsValue = delayMs ?? 300;
  const callTime = new Date();
  const toolCallMessage: Message = {
    id: createId(),
    content: [], // Tool calls can have empty content
    role: 'assistant',
    tool_calls: [
      {
        id: toolCallId,
        type: 'function',
        function: {
          name: toolName,
          arguments: JSON.stringify(params),
        },
      },
    ],
    sessionId,
    assistantId,
    source,
    createdAt: callTime,
  };

  const toolResultMessage: Message = {
    id: createId(),
    content: result,
    role: 'tool',
    tool_call_id: toolCallId,
    sessionId,
    assistantId,
    source,
    createdAt: new Date(callTime.getTime() + delayMsValue),
  };

  return [toolCallMessage, toolResultMessage];
};
