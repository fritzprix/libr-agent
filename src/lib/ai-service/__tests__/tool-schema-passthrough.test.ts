import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { MCPTool } from '@/lib/mcp';

import { OpenAIService } from '../openai';

vi.mock('openai', () => ({
  default: vi.fn().mockImplementation(() => ({
    chat: {
      completions: {
        create: vi.fn(),
      },
    },
    models: {
      list: vi.fn(),
    },
  })),
}));

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

vi.mock('../../llm-config-manager', () => ({
  llmConfigManager: {
    getModelsForProvider: vi.fn().mockReturnValue({}),
    getModel: vi.fn().mockReturnValue(null),
  },
}));

vi.mock('../model-capabilities', () => ({
  supportsThinking: vi.fn().mockResolvedValue(false),
  getContextWindow: vi.fn().mockResolvedValue(128000),
}));

describe('tool schema passthrough', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('preserves nested oneOf contracts when converting OpenAI tools', () => {
    const service = new OpenAIService('sk-test');
    const tool: MCPTool = {
      name: 'editFile',
      description: 'Unified edit tool',
      inputSchema: {
        type: 'object',
        properties: {
          edits: {
            type: 'array',
            items: {
              type: 'object',
              oneOf: [
                {
                  type: 'object',
                  properties: {
                    op: { type: 'string', const: 'replace' },
                    content: { type: 'string' },
                  },
                  required: ['op', 'content'],
                  additionalProperties: false,
                },
                {
                  type: 'object',
                  properties: {
                    op: { type: 'string', const: 'delete' },
                  },
                  required: ['op'],
                  additionalProperties: false,
                },
              ],
            },
          },
        },
        required: ['edits'],
      },
    };

    const converted = service.convertTools([tool]);
    const parameters = converted[0].function.parameters as {
      properties?: {
        edits?: {
          items?: {
            oneOf?: Array<{ properties?: { op?: { const?: string } } }>;
          };
        };
      };
    };

    const variants = parameters.properties?.edits?.items?.oneOf;
    expect(variants).toHaveLength(2);
    expect(variants?.[0]?.properties?.op?.const).toBe('replace');
    expect(variants?.[1]?.properties?.op?.const).toBe('delete');
  });
});
