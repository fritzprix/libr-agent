import { invoke } from '@tauri-apps/api/core';
import type { RustMessage } from '../../models/chat';
import { createId } from '@paralleldrive/cuid2';

/**
 * Handle LLM response from frontend by sending it to Rust backend
 * This is the primary way to sync frontend-generated messages (like UI tool calls) with the Rust backend.
 */
export async function handleLLMResponse(
  sessionId: string,
  assistantMessage: RustMessage,
): Promise<void> {
  await invoke('agent_handle_llm_response', {
    sessionId,
    assistantMessage,
  });
}

/**
 * Trigger a tool execution as if it was a User request (but with tool_calls)
 * This allows direct execution of tools from UI actions, recorded as User activity.
 *
 * This function constructs a message with `role: 'user'` that contains `toolCalls`.
 * The Rust backend will treat this as a signal to execute the tool and resume the workflow.
 *
 * @param sessionId - The active session ID
 * @param toolName - The full name of the tool to execute (including prefixes if any)
 * @param args - The arguments for the tool execution
 */
export async function handleUserToolCall(
  sessionId: string,
  toolName: string,
  args: any, // eslint-disable-line @typescript-eslint/no-explicit-any
): Promise<void> {
  const toolCallId = createId();
  const now = Date.now();

  const message: RustMessage = {
    id: createId(),
    sessionId,
    role: 'assistant', // UI actions are now represented as assistant role for semantic clarity
    content: [], // Empty content as the intent is pure tool execution
    toolCalls: [
      {
        id: toolCallId,
        type: 'function',
        function: {
          name: toolName,
          arguments: JSON.stringify(args),
        },
      },
    ],
    createdAt: now,
    updatedAt: now,
  };

  await handleLLMResponse(sessionId, message);
}
