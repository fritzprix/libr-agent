import { describe, it, expect } from 'vitest';
import { parseAssistant } from '../validation';

describe('parseAssistant', () => {
  it('should parse a valid flat DTO correctly', () => {
    const now = Date.now();
    const dto = {
      id: 'test-id',
      name: 'Test Assistant',
      config: {
        systemPrompt: 'You are helpful',
        model: 'gpt-4',
      },
      createdAt: now,
      updatedAt: now,
    };

    const result = parseAssistant(dto);

    expect(result.id).toBe('test-id');
    expect(result.name).toBe('Test Assistant');
    expect(result.systemPrompt).toBe('You are helpful');
    expect(result.model).toBe('gpt-4');
    expect(result.createdAt).toBeInstanceOf(Date);
    expect(result.createdAt.getTime()).toBe(now);
  });

  it('should handle missing config gracefully', () => {
    const now = Date.now();
    const dto = {
      id: 'test-id',
      name: 'Test Assistant',
      config: null,
      createdAt: now,
      updatedAt: now,
    };

    const result = parseAssistant(dto);

    expect(result.systemPrompt).toBe(''); // Default
    expect(result.deletionProtected).toBe(false); // Default
  });

  it('should throw on invalid DTO structure', () => {
    const invalidDto = {
      id: 'test-id',
      // missing name
      createdAt: 123,
    };

    expect(() => parseAssistant(invalidDto)).toThrow();
  });
});
