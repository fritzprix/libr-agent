import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import { resolveToolResultUiOverride } from '../presentation';

function makeResult(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'tool',
    content: [{ type: 'text', text: 'ok' }],
    tool_call_id: 'call-1',
    ...overrides,
  };
}

describe('resolveToolResultUiOverride', () => {
  it('returns alwaysVisible for MCP UI resources', () => {
    const result = makeResult({
      content: [
        {
          type: 'resource',
          resource: {
            uri: 'ui://test',
            mimeType: 'text/html',
            text: '<p>widget</p>',
          },
        },
      ],
    });

    expect(resolveToolResultUiOverride('any__tool', result, 'simple')).toEqual({
      alwaysVisible: true,
      hideParameters: true,
    });
    expect(
      resolveToolResultUiOverride('any__tool', result, 'developer'),
    ).toEqual({
      alwaysVisible: true,
      hideParameters: false,
    });
  });

  it('returns alwaysVisible when structured content has a registered UI', () => {
    const result = makeResult({
      metadata: {
        structuredContent: {
          path: '/tmp/a.ts',
          action: 'created',
          bytes_written: 10,
          lines: 1,
        },
      },
    });

    expect(
      resolveToolResultUiOverride('workspace__writeFile', result, 'simple'),
    ).toEqual({
      alwaysVisible: true,
      hideParameters: true,
    });
    expect(
      resolveToolResultUiOverride('workspace__writeFile', result, 'developer'),
    ).toEqual({
      alwaysVisible: true,
      hideParameters: false,
    });
  });

  it('returns null for structured content without a UI renderer', () => {
    const result = makeResult({
      metadata: {
        structuredContent: {
          mode: 'semantic',
          results: [],
        },
      },
    });

    expect(
      resolveToolResultUiOverride(
        'knowledge__searchKnowledge',
        result,
        'simple',
      ),
    ).toBeNull();
  });

  it('returns null for invalid structured payloads of supported tools', () => {
    const result = makeResult({
      metadata: {
        structuredContent: { action: 'created' },
      },
    });

    expect(
      resolveToolResultUiOverride('workspace__writeFile', result, 'developer'),
    ).toBeNull();
  });

  it('returns null when there is no result or no override signal', () => {
    expect(
      resolveToolResultUiOverride('workspace__writeFile', undefined, 'simple'),
    ).toBeNull();
    expect(
      resolveToolResultUiOverride(
        'workspace__writeFile',
        makeResult(),
        'simple',
      ),
    ).toBeNull();
  });

  it('returns alwaysVisible for in-flight checkSession wait tools', () => {
    expect(
      resolveToolResultUiOverride(
        'agent__checkSession',
        undefined,
        'simple',
        { sessionId: 'abc1234567', wait: true },
      ),
    ).toEqual({
      alwaysVisible: true,
      hideParameters: true,
    });
  });

  it('does not force visibility for non-waiting checkSession', () => {
    expect(
      resolveToolResultUiOverride(
        'agent__checkSession',
        undefined,
        'developer',
        { sessionId: 'abc1234567', wait: false },
      ),
    ).toBeNull();
  });
});
