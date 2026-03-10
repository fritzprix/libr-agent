import { describe, it, expect } from 'vitest';
import { isValidMessage } from '../validation';
import type { Message } from '../chat';

describe('isValidMessage', () => {
  it('should return true for a valid full message', () => {
    const validMessage: Message = {
      id: 'msg_1',
      sessionId: 'sess_1',
      threadId: 'thread_1',
      role: 'user',
      content: [{ type: 'text', text: 'Hello' }],
      createdAt: new Date(),
    };
    expect(isValidMessage(validMessage)).toBe(true);
  });

  it('should return false for undefined or null', () => {
    expect(isValidMessage(undefined)).toBe(false);
    // null is a legitimate runtime value that must also be rejected
    expect(isValidMessage(null as unknown as Partial<Message>)).toBe(false);
  });

  it('should return false if id is missing or not a string', () => {
    const msg = {
      sessionId: 'sess_1',
      threadId: 'thread_1',
      role: 'user',
      content: [],
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);

    expect(isValidMessage({ ...msg, id: 123 as unknown as string })).toBe(
      false,
    );
  });

  it('should return false if sessionId is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      threadId: 'thread_1',
      role: 'user',
      content: [],
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);

    expect(
      isValidMessage({ ...msg, sessionId: 42 as unknown as string }),
    ).toBe(false);
  });

  it('should return false if threadId is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      role: 'user',
      content: [],
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);

    expect(
      isValidMessage({ ...msg, threadId: true as unknown as string }),
    ).toBe(false);
  });

  it('should return false if role is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      threadId: 'thread_1',
      content: [],
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);

    expect(isValidMessage({ ...msg, role: 99 as unknown as string })).toBe(
      false,
    );
  });

  it('should return false if content is not an array', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      threadId: 'thread_1',
      role: 'user',
      content: 'hello' as unknown as Message['content'],
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);
  });
});
