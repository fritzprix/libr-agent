import { describe, expect, it } from 'vitest';

import type { MCPTool } from '@/lib/mcp';
import type { JSONSchema, JSONSchemaArray, JSONSchemaObject } from '@/lib/mcp';

import { normalizeAvailableTools } from '../base-service-utils';

function isArraySchema(schema: JSONSchema): schema is JSONSchemaArray {
  return schema.type === 'array';
}

function isObjectSchema(schema: JSONSchema): schema is JSONSchemaObject {
  return schema.type === 'object';
}

function asArraySchema(
  schema: JSONSchema | undefined,
): JSONSchemaArray | undefined {
  return schema && isArraySchema(schema) ? schema : undefined;
}

function asObjectSchema(
  schema: JSONSchema | undefined,
): JSONSchemaObject | undefined {
  return schema && isObjectSchema(schema) ? schema : undefined;
}

describe('normalizeAvailableTools', () => {
  it('preserves inputSchema property insertion order for provider tool payloads', () => {
    const writeFileTool: MCPTool = {
      name: 'writeFile',
      description: 'Write a file',
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string' },
          mode: { type: 'string' },
          content: { type: 'string' },
        },
        required: ['path', 'content'],
      },
    };

    const [normalized] = normalizeAvailableTools([writeFileTool]);

    expect(Object.keys(normalized.inputSchema.properties ?? {})).toEqual([
      'path',
      'mode',
      'content',
    ]);
  });

  it('preserves nested oneOf property order inside inputSchema', () => {
    const editFileTool: MCPTool = {
      name: 'editFile',
      description: 'Edit a file',
      inputSchema: {
        type: 'object',
        properties: {
          path: { type: 'string' },
          edits: {
            type: 'array',
            items: {
              type: 'object',
              oneOf: [
                {
                  type: 'object',
                  properties: {
                    startLine: { type: 'integer' },
                    op: { type: 'string' },
                    content: { type: 'string' },
                  },
                },
              ],
            },
          },
        },
        required: ['path', 'edits'],
      },
    };

    const [normalized] = normalizeAvailableTools([editFileTool]);
    const editsSchema = asArraySchema(normalized.inputSchema.properties?.edits);
    const editItemSchema = Array.isArray(editsSchema?.items)
      ? editsSchema?.items[0]
      : editsSchema?.items;
    const variant = editItemSchema?.oneOf?.[0];

    expect(Object.keys(asObjectSchema(variant)?.properties ?? {})).toEqual([
      'startLine',
      'op',
      'content',
    ]);
  });

  it('preserves presentInteractive and recordKnowledge property order', () => {
    const presentInteractive: MCPTool = {
      name: 'presentInteractive',
      description: 'Render content',
      inputSchema: {
        type: 'object',
        properties: {
          format: { type: 'string' },
          title: { type: 'string' },
          interaction: { type: 'object', properties: {} },
          content: { type: 'string' },
        },
        required: ['content'],
      },
    };

    const recordKnowledge: MCPTool = {
      name: 'recordKnowledge',
      description: 'Record knowledge',
      inputSchema: {
        type: 'object',
        properties: {
          tags: { type: 'array', items: { type: 'string' } },
          source: { type: 'string' },
          auto_extract: { type: 'boolean' },
          entities: { type: 'array', items: { type: 'object', properties: {} } },
          relationships: {
            type: 'array',
            items: { type: 'object', properties: {} },
          },
          content: { type: 'string' },
        },
        required: ['content'],
      },
    };

    const normalized = normalizeAvailableTools([
      presentInteractive,
      recordKnowledge,
    ]);

    expect(
      Object.keys(
        normalized.find((tool) => tool.name === 'presentInteractive')
          ?.inputSchema.properties ?? {},
      ),
    ).toEqual(['format', 'title', 'interaction', 'content']);

    expect(
      Object.keys(
        normalized.find((tool) => tool.name === 'recordKnowledge')?.inputSchema
          .properties ?? {},
      ),
    ).toEqual([
      'tags',
      'source',
      'auto_extract',
      'entities',
      'relationships',
      'content',
    ]);
  });

  it('still canonicalizes tool list order for prompt-cache stability', () => {
    const alphaTool: MCPTool = {
      name: 'alpha',
      description: 'Alpha tool',
      inputSchema: {
        type: 'object',
        properties: {
          zeta: { type: 'string' },
          alpha: { type: 'string' },
        },
        required: ['alpha'],
      },
    };

    const betaTool: MCPTool = {
      name: 'beta',
      description: 'Beta tool',
      inputSchema: {
        type: 'object',
        properties: {
          beta: { type: 'number' },
        },
      },
    };

    const forward = normalizeAvailableTools([betaTool, alphaTool]).map(
      (tool) => tool.name,
    );
    const reverse = normalizeAvailableTools([alphaTool, betaTool]).map(
      (tool) => tool.name,
    );

    expect(forward).toEqual(reverse);
    expect(forward).toEqual(['alpha', 'beta']);
  });
});
