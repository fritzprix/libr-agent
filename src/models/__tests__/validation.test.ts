import { describe, it, expect, vi } from 'vitest';
import { parseAssistant, isValidMessage } from '../validation';

vi.mock('@/lib/logger', () => ({
  getLogger: vi.fn(() => ({
    error: vi.fn(),
    warn: vi.fn(),
    info: vi.fn(),
  })),
}));

describe('Models Validation', () => {
  describe('parseAssistant', () => {
    it('successfully parses a valid assistant DTO', () => {
      const now = Date.now();
      const validDto = {
        id: '123',
        name: 'Test Assistant',
        config: {
          description: 'A test assistant',
          avatar: 'bot.png',
          systemPrompt: 'You are a test assistant',
          mcpServerIds: ['server1'],
          localServices: ['service1'],
          allowedBuiltInServiceAliases: ['alias1'],
          disabledSkills: ['skill1'],
          deletionProtected: true,
        },
        createdAt: now,
        updatedAt: now,
      };

      const result = parseAssistant(validDto);
      expect(result.id).toBe('123');
      expect(result.name).toBe('Test Assistant');
      expect(result.description).toBe('A test assistant');
      expect(result.avatar).toBe('bot.png');
      expect(result.systemPrompt).toBe('You are a test assistant');
      expect(result.mcpServerIds).toEqual(['server1']);
      expect(result.localServices).toEqual(['service1']);
      expect(result.allowedBuiltInServiceAliases).toEqual(['alias1']);
      expect(result.disabledSkills).toEqual(['skill1']);
      expect(result.deletionProtected).toBe(true);
      expect(result.createdAt.getTime()).toBe(now);
      expect(result.updatedAt.getTime()).toBe(now);
    });

    it('handles stringified config', () => {
      const now = Date.now();
      const dto = {
        id: '123',
        name: 'Test Assistant',
        config: JSON.stringify({
          systemPrompt: 'Stringified prompt',
        }),
        createdAt: now,
        updatedAt: now,
      };

      const result = parseAssistant(dto);
      expect(result.systemPrompt).toBe('Stringified prompt');
    });

    it('falls back to defaults for invalid stringified config', () => {
      const now = Date.now();
      const dto = {
        id: '123',
        name: 'Test Assistant',
        config: 'invalid json',
        createdAt: now,
        updatedAt: now,
      };

      const result = parseAssistant(dto);
      expect(result.systemPrompt).toBe('You are a helpful assistant.');
      expect(result.deletionProtected).toBe(false);
    });

    it('falls back to defaults for invalid config object', () => {
      const now = Date.now();
      const dto = {
        id: '123',
        name: 'Test Assistant',
        config: {
          systemPrompt: 123, // invalid type
        },
        createdAt: now,
        updatedAt: now,
      };

      const result = parseAssistant(dto);
      expect(result.systemPrompt).toBe('You are a helpful assistant.');
    });

    it('throws error for missing required fields', () => {
      const invalidDto = {
        id: '123',
        // missing name
        createdAt: Date.now(),
        updatedAt: Date.now(),
      };

      expect(() => parseAssistant(invalidDto)).toThrow();
    });
  });

  describe('isValidMessage', () => {
    it('returns true for a valid full message', () => {
      const msg = {
        id: 'msg1',
        sessionId: 'sess1',
        threadId: 'thread1',
        role: 'user',
        content: [{ type: 'text', text: 'hello' }],
      };
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage(msg)).toBe(true);
    });

    it('returns false for undefined or null', () => {
      expect(isValidMessage(undefined)).toBe(false);
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage(null)).toBe(false);
    });

    it('returns false if required string fields are missing or wrong type', () => {
      const msgTemplate = {
        id: 'msg1',
        sessionId: 'sess1',
        threadId: 'thread1',
        role: 'user',
        content: [],
      };

      // @ts-expect-error Type check bypass for test
      expect(isValidMessage({ ...msgTemplate, id: 123 })).toBe(false);
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage({ ...msgTemplate, sessionId: undefined })).toBe(false);
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage({ ...msgTemplate, threadId: null })).toBe(false);
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage({ ...msgTemplate, role: {} })).toBe(false);
    });

    it('returns false if content is not an array', () => {
      const msg = {
        id: 'msg1',
        sessionId: 'sess1',
        threadId: 'thread1',
        role: 'user',
        content: 'not an array',
      };
      // @ts-expect-error Type check bypass for test
      expect(isValidMessage(msg)).toBe(false);
    });
  });
});
