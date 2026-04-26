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

export interface RequestAssemblyContext {
  prepareContextInjection: (
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ) => ContextInjectionResult;
}

export interface RequestAssemblyOptions {
  systemPrompt?: string;
  sessionContext?: string;
  messages: Message[];
  compactionInstruction?: string;
}

export function assembleRequestLayout(
  options: RequestAssemblyOptions,
  context: RequestAssemblyContext,
): ContextInjectionResult {
  const messages = options.compactionInstruction
    ? [
        ...options.messages,
        createCompactionInstructionMessage(options.compactionInstruction),
      ]
    : options.messages;

  return context.prepareContextInjection(
    options.systemPrompt,
    options.sessionContext,
    messages,
  );
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
