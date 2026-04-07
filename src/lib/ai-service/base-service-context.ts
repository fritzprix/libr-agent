import type { Message } from '@/models/chat';
import type { ContextInjectionResult } from './types';
import type { CompactOptions } from './base-service-shared';
import type { SyntheticSessionContextMessageOptions } from './base-service-shared';

const SESSION_CONTEXT_BACKGROUND_HEADER =
  '[Current session context — background reference only, do not respond to this block]';

const SESSION_CONTEXT_BACKGROUND_FOOTER = '[End of session context]';

interface CompactionInstructionOptions
  extends Pick<CompactOptions, 'targetMaxTokens' | 'hardMaxTokens'> {
  recursivePass?: boolean;
}

export function buildCompactionInstruction(
  messages: Message[],
  options?: CompactionInstructionOptions,
): string {
  const targetLine = options?.targetMaxTokens
    ? `Aim for <= ${options.targetMaxTokens} tokens in the final summary.`
    : undefined;
  const hardMaxLine = options?.hardMaxTokens
    ? `Hard max: ${options.hardMaxTokens} tokens. If you are close, compress harder rather than preserving prose.`
    : undefined;

  let instruction =
    'Summarise the previous conversation history concisely using structured Markdown.\n\n' +
    'Organize the summary into clear sections (e.g., "Key Decisions", "Context", "Tool Results", "Pending Actions") ' +
    'to preserve all information needed to continue the conversation effectively.\n\n' +
    [
      targetLine,
      hardMaxLine,
      'Preserve action-critical facts: user intent, decisions, IDs, file paths, error messages, tool outcomes, and unresolved next steps.',
      'Aggressively remove repetition, stale narration, verbose prose, and bulky raw output once the essential fact has been captured.',
      'IMPORTANT: Do NOT attempt to use tools in this response. Just output plain text.',
    ]
      .filter(Boolean)
      .join('\n');

  const firstMsg = messages[0];
  if (firstMsg?.id.startsWith('compact-summary-')) {
    instruction =
      'The first message is a previously accumulated compact summary that represents earlier conversation history.\n\n' +
      'Treat that prior summary as authoritative source material, but you MAY rewrite and compress it further. ' +
      'Do not preserve it verbatim if doing so wastes budget. Preserve the facts, not the wording.\n\n' +
      'Compression priorities:\n' +
      '1. Keep user goals, decisions, IDs, paths, errors, tool outcomes, and pending actions.\n' +
      '2. Dedupe repeated facts across the prior summary and newer messages.\n' +
      '3. Collapse stale implementation detail and bulky raw output into short factual statements.\n' +
      '4. Prefer terse bullets over narrative prose.\n\n' +
      (options?.recursivePass
        ? 'This is a bounded recursive compaction pass because the prior summary was still too large. Compress aggressively and keep only continuation-critical facts.\n\n'
        : '') +
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
