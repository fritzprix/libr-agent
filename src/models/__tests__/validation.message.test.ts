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
      createdAt: new Date()
    };
    expect(isValidMessage(validMessage)).toBe(true);
  });

  it('should return false for undefined or null', () => {
    expect(isValidMessage(undefined)).toBe(false);
    expect(isValidMessage(null as unknown as undefined)).toBe(false);
  });

  it('should return false if id is missing or not a string', () => {
    const msg = {
      sessionId: 'sess_1',
      threadId: 'thread_1',
      role: 'user',
      content: []
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);

    expect(isValidMessage({ ...msg, id: 123 as unknown as string })).toBe(false);
  });

  it('should return false if sessionId is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      threadId: 'thread_1',
      role: 'user',
      content: []
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);
  });

  it('should return false if threadId is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      role: 'user',
      content: []
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);
  });

  it('should return false if role is missing or not a string', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      threadId: 'thread_1',
      content: []
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);
  });

  it('should return false if content is not an array', () => {
    const msg = {
      id: 'msg_1',
      sessionId: 'sess_1',
      threadId: 'thread_1',
      role: 'user',
      content: 'hello' as unknown as any[]
    } as Partial<Message>;
    expect(isValidMessage(msg)).toBe(false);
  });
});
