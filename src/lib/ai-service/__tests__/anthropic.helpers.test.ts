import { describe, expect, it, vi } from 'vitest';
import type { Message } from '@/models/chat';

vi.mock('../../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

import {
  applyAnthropicMessageDeltaUsage,
  applyAnthropicMessageStartUsage,
  buildAnthropicPromptCacheMetadata,
  buildAnthropicSystemBlocks,
} from '../anthropic/cache';
import { convertToAnthropicMessages } from '../anthropic/message-converter';
import { buildAnthropicToolResultBlocks } from '../anthropic/format';
import { parseAnthropicToolInput } from '../anthropic/tool-input';

describe('Anthropic helper modules', () => {
  it('builds separate stable and volatile Anthropic system blocks', () => {
    const blocks = buildAnthropicSystemBlocks(
      'Stable header',
      '# Current Context Information\nvolatile bits',
    );

    expect(blocks).toHaveLength(2);
    expect(blocks?.[0]).toMatchObject({
      type: 'text',
      text: 'Stable header',
      cache_control: { type: 'ephemeral' },
    });
    expect(blocks?.[1]).toMatchObject({
      type: 'text',
      text: '# Current Context Information\nvolatile bits',
    });
  });

  it('tracks Anthropic cache usage across message start and delta events', () => {
    const startUsage = applyAnthropicMessageStartUsage(
      {
        promptTokens: 0,
        completionTokens: 0,
        totalTokens: 0,
      },
      {
        input_tokens: 120,
        output_tokens: 0,
        cache_creation_input_tokens: 30,
        cache_read_input_tokens: 90,
      },
    );

    const updatedUsage = applyAnthropicMessageDeltaUsage(startUsage, {
      input_tokens: 120,
      output_tokens: 45,
    });

    expect(updatedUsage).toMatchObject({
      promptTokens: 240,
      completionTokens: 45,
      totalTokens: 285,
      cachedPromptTokens: 90,
    });
    expect(updatedUsage.details).toMatchObject({
      cacheCreationInputTokens: 30,
      cacheReadInputTokens: 90,
    });
  });

  it('builds Anthropic cache metadata for logging without losing prior cache details', () => {
    const metadata = buildAnthropicPromptCacheMetadata(
      {
        input_tokens: 120,
        output_tokens: 45,
      },
      {
        cacheCreationInputTokens: 30,
        cacheReadInputTokens: 90,
      },
    );

    expect(metadata).toEqual({
      promptTokens: 240,
      completionTokens: 45,
      totalTokens: 285,
      cachedPromptTokens: 90,
      cacheCreationInputTokens: 30,
      cacheReadInputTokens: 90,
    });
  });

  it('parses JSON tool input into an object and rejects non-object payloads', () => {
    expect(parseAnthropicToolInput('{"query":"hello"}', {})).toEqual({
      query: 'hello',
    });

    expect(() => parseAnthropicToolInput('[]', {})).toThrow(
      'Parsed tool call arguments must be an object',
    );
  });

  it('normalizes tool-result image/jpg payloads for Anthropic', () => {
    const result = buildAnthropicToolResultBlocks(
      {
        id: 'message-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'toolu_123',
        content: [
          { type: 'text', text: 'Rendered preview' },
          { type: 'image', data: 'abc123', mimeType: 'image/jpg' },
        ],
      },
      'toolu_123',
      'message-1',
      { warn: vi.fn() },
    );

    expect(result.content[0]).toMatchObject({
      type: 'tool_result',
      tool_use_id: 'toolu_123',
    });

    const toolResultContent = result.content[0].content;
    expect(Array.isArray(toolResultContent)).toBe(true);
    expect(toolResultContent).toEqual([
      { type: 'text', text: 'Rendered preview' },
      {
        type: 'image',
        source: {
          type: 'base64',
          media_type: 'image/jpeg',
          data: 'abc123',
        },
      },
    ]);
  });

  it('converts Anthropic messages while preserving tool results and ui-originated users', () => {
    const messages: Message[] = [
      {
        id: 'user-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        source: 'ui',
        content: [{ type: 'text', text: 'Look at this' }],
      },
      {
        id: 'assistant-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Calling tool' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: {
              name: 'search',
              arguments: '{"query":"hello"}',
            },
          },
        ],
      },
      {
        id: 'tool-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call_1',
        content: [{ type: 'text', text: 'done' }],
      },
    ];

    const result = convertToAnthropicMessages(messages);

    expect(result).toHaveLength(3);
    expect(result[0]).toMatchObject({
      role: 'user',
      content: 'Look at this',
    });
    expect(result[1]).toMatchObject({
      role: 'assistant',
    });
    expect(result[1]?.content).toEqual([
      { type: 'text', text: 'Calling tool' },
      {
        type: 'tool_use',
        id: 'call_1',
        name: 'search',
        input: { query: 'hello' },
      },
    ]);
    expect(result[2]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'tool_result',
          tool_use_id: 'call_1',
          content: 'done',
        },
      ],
    });
  });

  it('uses tool content text instead of structured metadata in Anthropic tool results', () => {
    const messages: Message[] = [
      {
        id: 'assistant-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Checking session' }],
        tool_calls: [
          {
            id: 'call_structured',
            type: 'function',
            function: {
              name: 'agent__checkSession',
              arguments: '{"sessionId":"session-1"}',
            },
          },
        ],
      },
      {
        id: 'tool-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call_structured',
        content: [{ type: 'text', text: '✓ Session session-1 is terminal.' }],
        metadata: {
          structuredContent: {
            sessionId: 'session-1',
            status: 'idle',
            responseStatus: 'success',
            result: 'Hidden structured payload',
          },
        },
      },
    ];

    const result = convertToAnthropicMessages(messages);

    expect(result[1]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'tool_result',
          tool_use_id: 'call_structured',
          content: '✓ Session session-1 is terminal.',
        },
      ],
    });
  });

  it('batches consecutive tool results into a single user message after one assistant tool_use turn', () => {
    const messages: Message[] = [
      {
        id: 'assistant-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Calling tools' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: {
              name: 'search',
              arguments: '{"query":"hello"}',
            },
          },
          {
            id: 'call_2',
            type: 'function',
            function: {
              name: 'fetch',
              arguments: '{"url":"https://example.com"}',
            },
          },
        ],
      },
      {
        id: 'tool-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call_1',
        content: [{ type: 'text', text: 'first result' }],
      },
      {
        id: 'tool-2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call_2',
        content: [{ type: 'text', text: 'second result' }],
      },
    ];

    const result = convertToAnthropicMessages(messages);

    expect(result).toHaveLength(2);
    expect(result[0]).toMatchObject({ role: 'assistant' });
    expect(result[1]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'tool_result',
          tool_use_id: 'call_1',
          content: 'first result',
        },
        {
          type: 'tool_result',
          tool_use_id: 'call_2',
          content: 'second result',
        },
      ],
    });
  });

  it('moves the cache breakpoint to the last stable message before a synthetic session-context tail', () => {
    const messages: Message[] = [
      {
        id: 'user-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: 'Stable user prompt' }],
      },
      {
        id: 'session-context',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: '# Current Context Information\nvolatile' }],
        metadata: {
          anthropicSyntheticSessionContext: true,
        },
      },
    ];

    const result = convertToAnthropicMessages(messages);

    expect(result).toHaveLength(2);
    expect(result[0]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'text',
          text: 'Stable user prompt',
          cache_control: { type: 'ephemeral' },
        },
      ],
    });
    expect(result[1]).toMatchObject({
      role: 'user',
      content: '# Current Context Information\nvolatile',
    });
  });

  it('adds an extra midpoint cache breakpoint for long Anthropic conversations', () => {
    const messages: Message[] = Array.from({ length: 8 }, (_, index) => ({
      id: `user-${index + 1}`,
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'user',
      content: [{ type: 'text', text: `Stable message ${index + 1}` }],
    }));

    const result = convertToAnthropicMessages(messages);

    expect(result).toHaveLength(8);
    expect(result[3]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'text',
          text: 'Stable message 4',
          cache_control: { type: 'ephemeral' },
        },
      ],
    });
    expect(result[7]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'text',
          text: 'Stable message 8',
          cache_control: { type: 'ephemeral' },
        },
      ],
    });
  });

  it('keeps the extra Anthropic midpoint breakpoint on stable history before a synthetic session-context tail', () => {
    const stableMessages: Message[] = Array.from({ length: 8 }, (_, index) => ({
      id: `user-${index + 1}`,
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'user',
      content: [{ type: 'text', text: `Stable message ${index + 1}` }],
    }));
    const messages: Message[] = [
      ...stableMessages,
      {
        id: 'session-context',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'user',
        content: [{ type: 'text', text: '# Current Context Information\nvolatile' }],
        metadata: {
          anthropicSyntheticSessionContext: true,
        },
      },
    ];

    const result = convertToAnthropicMessages(messages);

    expect(result).toHaveLength(9);
    expect(result[3]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'text',
          text: 'Stable message 4',
          cache_control: { type: 'ephemeral' },
        },
      ],
    });
    expect(result[7]).toMatchObject({
      role: 'user',
      content: [
        {
          type: 'text',
          text: 'Stable message 8',
          cache_control: { type: 'ephemeral' },
        },
      ],
    });
    expect(result[8]).toMatchObject({
      role: 'user',
      content: '# Current Context Information\nvolatile',
    });
  });
});
