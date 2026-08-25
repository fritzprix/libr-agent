import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import {
  applyCompactionRetryTailInstruction,
  COMPACTION_RETRY_NO_TOOL_NOTE,
} from '../compact-cache-alignment';

function makeMessage(
  id: string,
  role: Message['role'],
  text: string,
  source?: Message['source'],
): Message {
  return {
    id,
    sessionId: 's1',
    threadId: 's1',
    role,
    content: [{ type: 'text', text }],
    createdAt: new Date(1),
    source,
  };
}

describe('applyCompactionRetryTailInstruction', () => {
  it('appends retry guidance to the compaction-instruction tail only', () => {
    const messages = [
      makeMessage('u1', 'user', 'do the work', 'ui'),
      makeMessage('a1', 'assistant', 'working'),
      makeMessage(
        'compaction-instruction-9',
        'user',
        'Summarise the previous conversation.',
        'compaction-instruction',
      ),
    ];

    const next = applyCompactionRetryTailInstruction(messages);

    expect(next[0].content).toEqual(messages[0].content);
    expect(next[1].content).toEqual(messages[1].content);
    const instructionText = next[2].content
      .map((part) => (part.type === 'text' ? part.text : ''))
      .join('');
    expect(instructionText).toContain('Summarise the previous conversation.');
    expect(instructionText).toContain(COMPACTION_RETRY_NO_TOOL_NOTE);
  });

  it('does not mutate the system-prompt path (messages-only change)', () => {
    const messages = [
      makeMessage(
        'compaction-instruction-1',
        'user',
        'Write a compact summary.',
        'compaction-instruction',
      ),
    ];
    const next = applyCompactionRetryTailInstruction(messages);
    expect(next).toHaveLength(1);
    expect(next[0].id).toBe('compaction-instruction-1');
  });

  it('is idempotent when the retry note is already present', () => {
    const messages = [
      makeMessage(
        'compaction-instruction-1',
        'user',
        `Write a compact summary.\n\n${COMPACTION_RETRY_NO_TOOL_NOTE}`,
        'compaction-instruction',
      ),
    ];
    const next = applyCompactionRetryTailInstruction(messages);
    const text = next[0].content
      .map((part) => (part.type === 'text' ? part.text : ''))
      .join('');
    expect(text.match(new RegExp(COMPACTION_RETRY_NO_TOOL_NOTE, 'g'))).toHaveLength(
      1,
    );
  });

  it('appends a synthetic tail when no compaction-instruction marker exists', () => {
    const messages = [
      makeMessage('u1', 'user', 'do the work', 'ui'),
      makeMessage('a1', 'assistant', 'working'),
    ];

    const next = applyCompactionRetryTailInstruction(messages);

    expect(next).toHaveLength(3);
    expect(next[0].content).toEqual(messages[0].content);
    expect(next[1].content).toEqual(messages[1].content);
    expect(next[2].source).toBe('compaction-instruction');
    expect(next[2].id.startsWith('compaction-instruction-retry-')).toBe(true);
    expect(next[2].role).toBe('user');
    const text = next[2].content
      .map((part) => (part.type === 'text' ? part.text : ''))
      .join('');
    expect(text).toBe(COMPACTION_RETRY_NO_TOOL_NOTE);
  });
});
