import { createId } from '@paralleldrive/cuid2';
import type { Message } from '@/models/chat';
import type { AgentEventPayload } from './types';

/** The `llmFallbackNotice` arm of the agent event payload (#2961). */
export type FallbackNoticePayload = Extract<
  AgentEventPayload,
  { type: 'llmFallbackNotice' }
>;

/** Structured notice data kept in the message metadata; the chat bubble
 * renders it (localized line + expandable raw error) from here. */
export interface FallbackNoticeMetadata {
  /** HTTP status of the rejected primary request; absent for non-HTTP failures. */
  statusCode?: number;
  fallbackModel: string;
  errorMessage: string;
}

export function getFallbackNoticeMetadata(
  metadata: Message['metadata'],
): FallbackNoticeMetadata | null {
  const notice = metadata?.fallbackNotice;
  if (
    typeof notice !== 'object' ||
    notice === null ||
    typeof (notice as FallbackNoticeMetadata).fallbackModel !== 'string' ||
    typeof (notice as FallbackNoticeMetadata).errorMessage !== 'string'
  ) {
    return null;
  }
  return notice as FallbackNoticeMetadata;
}

/**
 * Builds the chat notice shown when a completion falls back to the fallback
 * model (#2961). The bubble renders the localized line and the expandable
 * raw server error from `metadata.fallbackNotice` — that is the user-visible
 * text. The sentence in `content` is a plain-text form for exports and
 * simple renderers only, so changing the wording means changing the locale
 * keys (`agent.bubble.fallbackNotice*`), not this string.
 */
export function buildFallbackNoticeMessage(
  payload: FallbackNoticePayload,
): Message {
  const rejected =
    typeof payload.statusCode === 'number'
      ? `The server rejected the request (HTTP ${payload.statusCode}).`
      : 'The primary model failed.';
  const text = `${rejected} Retrying with fallback model ${payload.fallbackModel}.`;
  const now = new Date();
  return {
    id: createId(),
    sessionId: payload.sessionId,
    threadId: payload.sessionId,
    role: 'system',
    content: [{ type: 'text', text }],
    createdAt: now,
    updatedAt: now,
    metadata: {
      fallbackNotice: {
        statusCode: payload.statusCode,
        fallbackModel: payload.fallbackModel,
        errorMessage: payload.errorMessage,
      } satisfies FallbackNoticeMetadata,
    },
  };
}
