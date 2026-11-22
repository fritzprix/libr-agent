import { describe, expect, it } from 'vitest';
import {
  buildFallbackNoticeMessage,
  getFallbackNoticeMetadata,
  type FallbackNoticePayload,
} from '../fallback-notice';

const payload = (
  overrides: Partial<FallbackNoticePayload> = {},
): FallbackNoticePayload => ({
  type: 'llmFallbackNotice',
  sessionId: 's1',
  statusCode: 400,
  fallbackProvider: 'openai',
  fallbackModel: 'gpt-4o-mini',
  errorMessage: 'LLM provider returned HTTP 400: context length exceeded',
  ...overrides,
});

describe('buildFallbackNoticeMessage', () => {
  it('names the HTTP status and the fallback model in the text', () => {
    const message = buildFallbackNoticeMessage(payload());

    expect(message.role).toBe('system');
    expect(message.sessionId).toBe('s1');
    const text = message.content[0];
    expect(text.type).toBe('text');
    expect((text as { text: string }).text).toContain('HTTP 400');
    expect((text as { text: string }).text).toContain('gpt-4o-mini');
  });

  it('keeps the raw server error in metadata for the expandable detail', () => {
    const message = buildFallbackNoticeMessage(payload());

    const notice = getFallbackNoticeMetadata(message.metadata);
    expect(notice).not.toBeNull();
    expect(notice?.statusCode).toBe(400);
    expect(notice?.fallbackModel).toBe('gpt-4o-mini');
    expect(notice?.errorMessage).toBe(
      'LLM provider returned HTTP 400: context length exceeded',
    );
  });

  it('says the primary model failed when there is no HTTP status', () => {
    const message = buildFallbackNoticeMessage(
      payload({ statusCode: undefined, errorMessage: 'connection reset' }),
    );

    const text = message.content[0] as { text: string };
    expect(text.text).toContain('The primary model failed.');
    expect(text.text).toContain('gpt-4o-mini');
    expect(getFallbackNoticeMetadata(message.metadata)?.statusCode).toBeUndefined();
  });
});

describe('getFallbackNoticeMetadata', () => {
  it('returns null for ordinary messages', () => {
    expect(getFallbackNoticeMetadata(undefined)).toBeNull();
    expect(getFallbackNoticeMetadata({ executionTime: 12 })).toBeNull();
  });

  it('returns null when the notice fields are not shaped like a notice', () => {
    expect(
      getFallbackNoticeMetadata({ fallbackNotice: { fallbackModel: 42 } }),
    ).toBeNull();
  });
});
