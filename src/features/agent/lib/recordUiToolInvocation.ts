import { createId } from '@paralleldrive/cuid2';

import {
  createToolMessagePair,
  createUserMessage,
} from '@/lib/chat-utils';
import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';

import {
  getUiToolInjectionMode,
  type UiToolInjectionMode,
} from './uiToolInjectionPolicy';

export type RecordUiToolInvocationDeps = {
  /** History-only persistence — must never start a workflow. */
  appendToolMessages: (messages: Message[]) => Promise<void>;
  /** Chat SSOT for starting/continuing an LLM turn. */
  submit: (message: Message) => Promise<void>;
};

export type RecordUiToolInvocationInput = {
  toolName: string;
  params: Record<string, unknown>;
  result: MCPContent[];
  sessionId: string;
  assistantId?: string;
  /**
   * User text submitted after history-only tools that need an LLM turn.
   * Required when `getUiToolInjectionMode(toolName) === 'history-then-submit'`.
   */
  kickoffUserText?: string;
};

export type RecordUiToolInvocationResult = {
  mode: UiToolInjectionMode;
};

/**
 * Persist a UI-executed tool call/result pair, then optionally kick off LLM
 * via `submit` according to {@link getUiToolInjectionMode}.
 *
 * Never uses `injectMessages` for tool pairs — that mixed recording with
 * workflow start and produced divergent busy behavior.
 */
export async function recordUiToolInvocation(
  deps: RecordUiToolInvocationDeps,
  input: RecordUiToolInvocationInput,
): Promise<RecordUiToolInvocationResult> {
  const mode = getUiToolInjectionMode(input.toolName);
  const toolCallId = createId();
  const [toolCallMsg, toolResultMsg] = createToolMessagePair(
    input.toolName,
    input.params,
    input.result,
    toolCallId,
    input.sessionId,
    undefined,
    input.assistantId,
    'ui',
  );

  await deps.appendToolMessages([toolCallMsg, toolResultMsg]);

  if (mode === 'history-then-submit') {
    const kickoff = input.kickoffUserText?.trim();
    if (!kickoff) {
      throw new Error(
        `kickoffUserText is required for UI tool "${input.toolName}" (not on history-only neg-list)`,
      );
    }

    await deps.submit(
      createUserMessage(
        kickoff,
        input.sessionId,
        undefined,
        input.assistantId,
        'ui',
      ),
    );
  }

  return { mode };
}
