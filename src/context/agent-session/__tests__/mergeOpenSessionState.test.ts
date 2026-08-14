import { describe, expect, it } from 'vitest';

import type { Message } from '@/models/chat';
import type { PendingApproval } from '../types';
import {
  mergeOpenSessionMessages,
  mergePendingApprovals,
} from '../mergeOpenSessionState';

function msg(
  id: string,
  overrides: Partial<Message> = {},
): Message {
  return {
    id,
    sessionId: 's1',
    threadId: 's1',
    role: 'user',
    content: [{ type: 'text', text: id }],
    createdAt: new Date(1000),
    ...overrides,
  } as Message;
}

describe('mergeOpenSessionMessages', () => {
  it('keeps event-only messages that the open snapshot missed', () => {
    const previous = [msg('a'), msg('event-only', { createdAt: new Date(3000) })];
    const incoming = [msg('a'), msg('b', { createdAt: new Date(2000) })];

    const merged = mergeOpenSessionMessages(previous, incoming);

    expect(merged.map((m) => m.id)).toEqual(['a', 'b', 'event-only']);
  });

  it('breaks createdAt ties by message id', () => {
    const previous = [
      msg('z-live', { createdAt: new Date(1000) }),
      msg('a-live', { createdAt: new Date(1000) }),
    ];
    const incoming = [msg('m-db', { createdAt: new Date(1000) })];

    const merged = mergeOpenSessionMessages(previous, incoming);

    expect(merged.map((m) => m.id)).toEqual(['a-live', 'm-db', 'z-live']);
  });

  it('prefers live streaming rows over the open snapshot', () => {
    const previous = [msg('a', { isStreaming: true, content: [{ type: 'text', text: 'live' }] })];
    const incoming = [msg('a', { isStreaming: false, content: [{ type: 'text', text: 'db' }] })];

    const merged = mergeOpenSessionMessages(previous, incoming);

    expect(merged).toHaveLength(1);
    expect(merged[0]?.isStreaming).toBe(true);
    expect(merged[0]?.content).toEqual([{ type: 'text', text: 'live' }]);
  });
});

describe('mergePendingApprovals', () => {
  it('keeps approvals that arrived via events during open', () => {
    const previous: PendingApproval[] = [
      {
        toolCallId: 't1',
        toolName: 'shell',
        arguments: '{}',
        approvalKind: 'standard',
      },
    ];
    const incoming: PendingApproval[] = [
      {
        toolCallId: 't2',
        toolName: 'browser',
        arguments: '{}',
        approvalKind: 'standard',
      },
    ];

    expect(mergePendingApprovals(previous, incoming).map((a) => a.toolCallId)).toEqual([
      't2',
      't1',
    ]);
  });
});
