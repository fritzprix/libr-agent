import { safeInvoke } from '@/lib/backend/core';
import { isWorkflowCancelledError } from '@/context/llm/types';
import type {
  AgentResponse,
  AgentRuntimeError,
  CompletionCancelRequest,
  ExecuteUiTauriActionRequest,
  AgentOpenSessionResponse,
  StreamingIssueReport,
} from '../../models/agent-ipc';
import type { RustMessage } from '../../models/chat';
import type { MCPResult } from '../mcp/protocol/response';
import type { MCPTool } from '@/lib/mcp';
import { createId } from '@paralleldrive/cuid2';

export interface CompactContextRecord {
  id: string;
  sessionId: string;
  toId: string;
  summary: string;
  createdAt: number;
  latestIncludedPreview?: string;
  condensedCount?: number;
}

export interface CompactResponseOutcome {
  retried: boolean;
}

/**
 * Handle LLM response from frontend by sending it to Rust backend
 * This is the primary way to sync frontend-generated messages (like UI tool calls) with the Rust backend.
 */
export async function handleLLMResponse(
  sessionId: string,
  assistantMessage: RustMessage,
): Promise<AgentResponse> {
  return safeInvoke<AgentResponse>(
    'agent_handle_llm_response',
    {
      sessionId,
      assistantMessage,
    },
    {
      shouldSuppressErrorLogging: isWorkflowCancelledError,
    },
  );
}

/**
 * Handle LLM error from frontend by sending it to Rust backend
 */
export async function handleLLMError(
  sessionId: string,
  error: AgentRuntimeError,
): Promise<void> {
  await safeInvoke<AgentResponse>('agent_handle_llm_error', {
    sessionId,
    error,
  });
}

export async function reportLLMStreamingIssue(
  report: StreamingIssueReport,
): Promise<AgentResponse<{ action: 'ignored' | 'retried' | 'failed' }>> {
  return safeInvoke<
    AgentResponse<{ action: 'ignored' | 'retried' | 'failed' }>
  >('agent_report_llm_streaming_issue', {
    report,
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

export async function executeUiTauriAction(
  sessionId: string,
  toolName: string,
  params: Record<string, unknown>,
): Promise<AgentResponse> {
  const request: ExecuteUiTauriActionRequest = {
    sessionId,
    toolName,
    params,
  };

  return safeInvoke<AgentResponse>('agent_execute_ui_tauri_action', {
    request,
  });
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
  return safeInvoke<MCPTool[]>('agent_get_available_tools', { sessionId });
}

export async function openAgentSession(
  sessionId: string,
  initialMessageLimit = 40,
): Promise<AgentOpenSessionResponse> {
  return safeInvoke<AgentOpenSessionResponse>('agent_open_session', {
    sessionId,
    initialMessageLimit,
  });
}

export type { CompletionCancelRequest };

/**
 * Notify Rust backend that frontend compaction LLM call succeeded.
 * Rust stores the record and clears the in-flight flag.
 */
export async function handleCompactResponse(
  sessionId: string,
  toId: string,
  compactedDeltaCount: number,
  summary: string,
): Promise<AgentResponse<CompactResponseOutcome>> {
  return safeInvoke<AgentResponse<CompactResponseOutcome>>(
    'agent_handle_compact_response',
    {
      sessionId,
      toId,
      compactedDeltaCount,
      summary,
    },
  );
}

export async function handleCompactError(
  sessionId: string,
  error: AgentRuntimeError,
): Promise<void> {
  await safeInvoke<AgentResponse>('agent_handle_compact_error', {
    sessionId,
    error,
  });
}

export async function getAgentCompactContext(
  sessionId: string,
): Promise<CompactContextRecord | null> {
  return safeInvoke<CompactContextRecord | null>('agent_get_compact_context', {
    sessionId,
  });
}

/**
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
  return safeInvoke<MCPResult<T>>('agent_call_builtin_tool', {
    sessionId,
    toolName,
    args,
  });
}

export async function submitInteractiveShellInput(
  sessionId: string,
  executionId: string,
  input: string,
): Promise<void> {
  await safeInvoke(
    'submit_interactive_shell_input',
    {
      request: {
        sessionId,
        executionId,
        input,
      },
    },
    {
      loggedArgs: {
        request: {
          sessionId,
          executionId,
          redacted: true,
        },
      },
    },
  );
}

export async function cancelInteractiveShellInput(
  sessionId: string,
  executionId: string,
): Promise<void> {
  await safeInvoke('cancel_interactive_shell_input', {
    request: {
      sessionId,
      executionId,
    },
  });
}
