import { describe, expect, it, vi } from 'vitest';
import { Type } from '@google/genai';
import type { JSONSchema } from '@/lib/mcp';
import { convertMCPSchemaToGeminiParameters } from '../gemini/mapper';

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

const transportSchema: JSONSchema = {
  type: 'object',
  description: 'Transport configuration',
  oneOf: [
    {
      type: 'object',
      properties: {
        type: { type: 'string', const: 'stdio' },
        command: { type: 'string', minLength: 1 },
        args: { type: 'array', items: { type: 'string' } },
      },
      required: ['type', 'command'],
    },
    {
      type: 'object',
      properties: {
        type: { type: 'string', enum: ['http', 'http-sse'] },
        url: { type: 'string', minLength: 1 },
      },
      required: ['type', 'url'],
    },
  ],
};

describe('convertMCPSchemaToGeminiParameters', () => {
  it('maps oneOf variants to anyOf instead of collapsing to a string', () => {
    const result = convertMCPSchemaToGeminiParameters(transportSchema);

    expect(result.type).toBeUndefined();
    expect(result.description).toBe('Transport configuration');
    expect(result.anyOf).toHaveLength(2);
  });

  it('keeps the string const discriminator and per-variant required fields', () => {
    const [stdio, http] = convertMCPSchemaToGeminiParameters(transportSchema)
      .anyOf ?? [];

    expect(stdio).toMatchObject({
      type: Type.OBJECT,
      required: ['type', 'command'],
      properties: { type: { type: Type.STRING, enum: ['stdio'] } },
    });
    expect(http).toMatchObject({
      type: Type.OBJECT,
      required: ['type', 'url'],
      properties: { type: { type: Type.STRING, enum: ['http', 'http-sse'] } },
    });
  });

  it('drops required keys that have no matching property', () => {
    const result = convertMCPSchemaToGeminiParameters({
      type: 'object',
      properties: { name: { type: 'string' } },
      required: ['name', 'missing'],
    });

    expect(result.required).toEqual(['name']);
  });

  it('omits required when the object declares none', () => {
    const result = convertMCPSchemaToGeminiParameters({
      type: 'object',
      properties: { name: { type: 'string' } },
    });

    expect(result.required).toBeUndefined();
  });
});
