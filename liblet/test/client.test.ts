import { describe, expect, it } from 'vitest';

import {
  SessionApiClient,
  normalizeSession,
  parseSseBlock,
} from '../src/client.js';

describe('SSE block parsing', () => {
  it('parses a named event with JSON data', () => {
    const event = parseSseBlock('event:status\ndata:{"status":"idle"}');
    expect(event).toEqual({ event: 'status', data: { status: 'idle' } });
  });

  it('keeps keep-alive comments out', () => {
    expect(parseSseBlock(': keep-alive')).toBeNull();
  });

  it('joins multi-line data and keeps non-JSON as text', () => {
    const event = parseSseBlock('data:line1\ndata:line2');
    expect(event?.data).toBe('line1\nline2');
  });
});

describe('session response normalization', () => {
  it('accepts a flat session and defaults pendingApprovals', () => {
    const session = normalizeSession({ id: 's1', status: 'idle' });
    expect(session.id).toBe('s1');
    expect(session.pendingApprovals).toEqual([]);
  });

  it('unwraps a { session: ... } envelope', () => {
    const session = normalizeSession({ session: { id: 's2' } });
    expect(session.id).toBe('s2');
  });

  it('rejects a response without a session id', () => {
    expect(() => normalizeSession({ status: 'idle' })).toThrow();
  });
});

describe('client request behavior', () => {
  const jsonResponse = (body: unknown, ok = true, status = 200) =>
    (async () =>
      ({
        ok,
        status,
        text: async () => JSON.stringify(body),
      }) as unknown as Response)();

  it('sends the bearer key only when one is configured', async () => {
    let seen: Record<string, string> | undefined;
    const fetchImpl = (async (_url: string, init?: RequestInit) => {
      seen = init?.headers as Record<string, string>;
      return jsonResponse({ status: 'ok', ready: true });
    }) as unknown as typeof fetch;

    await new SessionApiClient(7000, 'libr_key', fetchImpl).health();
    expect(seen?.authorization).toBe('Bearer libr_key');

    seen = undefined;
    await new SessionApiClient(7000, undefined, fetchImpl).health();
    expect(seen?.authorization).toBeUndefined();
  });

  it('turns an error body into an ApiError with the backend message', async () => {
    const fetchImpl = (async () =>
      jsonResponse(
        { error: 'Session not found: x' },
        false,
        404,
      )) as unknown as typeof fetch;
    await expect(
      new SessionApiClient(7000, undefined, fetchImpl).getSession('x'),
    ).rejects.toThrow('Session not found: x');
  });
});
