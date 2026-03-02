import { safeInvoke as invoke } from '@/lib/backend/core';
import type { AgentResponse } from '../../models/agent-ipc';
import type { RustMessage } from '../../models/chat';
import type { MCPResult } from '../mcp/protocol/response';
import type { MCPTool } from '@/lib/mcp';
import { createId } from '@paralleldrive/cuid2';

/**
 * Handle LLM response from frontend by sending it to Rust backend
 * This is the primary way to sync frontend-generated messages (like UI tool calls) with the Rust backend.
 */
export async function handleLLMResponse(
  sessionId: string,
  assistantMessage: RustMessage,
): Promise<void> {
  await invoke<AgentResponse>('agent_handle_llm_response', {
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
  args: Record<string, unknown>,
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

/**
 * Get available tools for a specific agent session
 * Returns the filtered tool list based on agent configuration
 * This ensures UI displays the same tools that LLM can actually use
 *
 * @param sessionId - The active session ID
 * @returns Array of MCPTool objects that are available for this session
 */
export async function getAgentAvailableTools(
  sessionId: string,
): Promise<MCPTool[]> {
  return invoke<MCPTool[]>('agent_get_available_tools', { sessionId });
}

/**
 * Call a builtin tool directly via proxy_manager (session-aware)
 * This ensures the tool runs within the correct session context
 *
 * @param sessionId - The active session ID
 * @param toolName - The name of the tool to execute
 * @param args - The arguments for the tool
 * @returns Promise resolving to MCPResult with optional structured content
 */
export async function agentCallBuiltinTool<T = unknown>(
  sessionId: string,
  toolName: string,
  args: Record<string, unknown>,
): Promise<MCPResult<T>> {
  return invoke<MCPResult<T>>('agent_call_builtin_tool', {
    sessionId,
    toolName,
    args,
  });
}
