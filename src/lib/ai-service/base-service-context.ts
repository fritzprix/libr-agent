import type { Message } from '@/models/chat';
import type { SyntheticSessionContextMessageOptions } from './base-service-shared';

const SESSION_CONTEXT_BACKGROUND_HEADER =
  '[Current session context — background reference only, do not respond to this block]';

const SESSION_CONTEXT_BACKGROUND_FOOTER = '[End of session context]';

export function buildCompactionInstruction(messages: Message[]): string {
  let instruction =
    'Summarise the previous conversation history concisely using structured Markdown.\n\n' +
    'Organize the summary into clear sections (e.g., "Key Decisions", "Context", "Tool Results", "Pending Actions") ' +
    'to preserve all information needed to continue the conversation effectively.\n\n' +
    'IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.';

  const firstMsg = messages[0];
  if (firstMsg?.id.startsWith('compact-summary-')) {
    instruction =
      'The first message is a previously accumulated compact summary that represents ALL earlier conversation history.\n\n' +
      'CRITICAL RESIDUAL RULE: Every fact, decision, action, and context item recorded in that prior summary ' +
      'MUST be preserved verbatim or re-stated with equivalent fidelity in your new summary. ' +
      'Do NOT compress, omit, or paraphrase the prior summary — treat it as an immutable residual that carries forward in full. ' +
      'Your new summary = (prior summary, preserved completely) + (new messages, summarised).\n\n' +
      instruction;
  }

  return instruction;
}

export function createCompactionInstructionMessage(
  instruction: string,
): Message {
  return {
    id: `compaction_instruction_${Date.now()}`,
    sessionId: 'internal',
    threadId: 'internal',
    role: 'user',
    content: [{ type: 'text', text: instruction }],
    createdAt: new Date(),
  };
}

export function mergeSessionContextIntoSystemPrompt(
  systemPrompt: string | undefined,
  sessionContext: string,
): string {
  return [systemPrompt, sessionContext].filter(Boolean).join('\n\n');
}

export function formatSessionContextAsBackgroundReference(
  sessionContext: string,
): string {
  return `${SESSION_CONTEXT_BACKGROUND_HEADER}\n\n${sessionContext}\n\n${SESSION_CONTEXT_BACKGROUND_FOOTER}`;
}

export function createSyntheticSessionContextMessage(
  sessionContext: string,
  messages: Message[],
  options?: SyntheticSessionContextMessageOptions,
): Message {
  const referenceMessage = messages[messages.length - 1];
  const idPrefix = options?.idPrefix ?? 'session-context';

  return {
    id: `${idPrefix}-${referenceMessage?.id ?? 'system'}`,
    sessionId:
      referenceMessage?.sessionId ??
      options?.sessionIdFallback ??
      `${idPrefix}`,
    threadId:
      referenceMessage?.threadId ??
      referenceMessage?.sessionId ??
      options?.threadIdFallback ??
      `${idPrefix}`,
    role: 'user',
    content: [
      {
        type: 'text',
        text: options?.contentText ?? sessionContext,
      },
    ],
    metadata: options?.metadata,
    createdAt: options?.createdAt,
  };
}
