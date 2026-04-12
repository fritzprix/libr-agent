import type { Message } from '@/models/chat';
import type { ContextInjectionResult } from './types';
import type { SyntheticSessionContextMessageOptions } from './base-service-shared';

const SESSION_CONTEXT_BACKGROUND_HEADER =
  '[Current session context — background reference only, do not respond to this block]';

const SESSION_CONTEXT_BACKGROUND_FOOTER = '[End of session context]';

export function isCompactSummaryMessage(
  message: Pick<Message, 'id' | 'source'> | undefined,
): boolean {
  if (!message) {
    return false;
  }

  return (
    message.source === 'compact-summary' ||
    message.id.startsWith('compact-summary-')
  );
}

export function buildCompactionInstruction(messages: Message[]): string {
  let instruction =
    'Summarise the previous conversation history using strict compact Markdown.\n\n' +
    'Use EXACTLY these sections in this order:\n' +
    '1. Stable Context\n' +
    '2. Key Decisions & Constraints\n' +
    '3. Current State\n' +
    '4. Recent Tool Results\n' +
    '5. Next Actions\n\n' +
    'Compression rules:\n' +
    '- Use terse bullet points, not prose paragraphs.\n' +
    '- Prefer noun phrases and short action statements.\n' +
    '- Minimize adjectives, adverbs, filler, and repetition.\n' +
    '- Do not restate obvious chronology or narration.\n' +
    '- Preserve durable facts, decisions, constraints, user preferences, and unresolved work.\n' +
    '- Keep volatile/recent details in Current State, Recent Tool Results, or Next Actions.\n' +
    '- If a detail is recoverable from recent tool results, do not duplicate it in stable sections.\n\n' +
    'Section limits:\n' +
    '- Stable Context: at most 6 bullets\n' +
    '- Key Decisions & Constraints: at most 6 bullets\n' +
    '- Current State: at most 6 bullets\n' +
    '- Recent Tool Results: at most 5 bullets\n' +
    '- Next Actions: at most 5 bullets\n' +
    '- Each bullet should be one short sentence or fragment.\n\n' +
    'IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.';

  const firstMsg = messages[0];
  if (isCompactSummaryMessage(firstMsg)) {
    instruction =
      'The first message is a previously accumulated compact summary that represents ALL earlier conversation history.\n\n' +
      'CRITICAL RESIDUAL RULE: Every fact, decision, action, and context item recorded in that prior summary ' +
      'MUST be preserved verbatim or re-stated with equivalent fidelity in your new summary. ' +
      'Do NOT drop durable information from the prior summary. ' +
      'You may tighten wording, remove duplication, and relocate items into the required sections, but you must preserve the same meaning and operational usefulness. ' +
      'Your new summary = (prior summary, preserved faithfully and reorganized if needed) + (new messages, summarised under the same schema).\n\n' +
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

export function createEphemeralSessionContextInjection(
  systemPrompt: string | undefined,
  sessionContext: string | undefined,
  messages: Message[],
  options?: SyntheticSessionContextMessageOptions,
): ContextInjectionResult {
  if (!sessionContext) {
    return { systemPrompt, sessionContext: undefined, messages };
  }

  const syntheticSessionContextMessage = createSyntheticSessionContextMessage(
    sessionContext,
    messages,
    options,
  );

  return {
    systemPrompt,
    sessionContext: undefined,
    messages: [...messages, syntheticSessionContextMessage],
  };
}
