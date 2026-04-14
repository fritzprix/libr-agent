import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import { BaseAIService, stableStringify } from '../base-service';
import { buildCompactionInstruction } from '../base-service-context';
import { AIServiceError, AIServiceProvider } from '../types';

class TestBaseAIService extends BaseAIService<string, string> {
  private compactChunks: string[] = [''];

  getProvider(): AIServiceProvider {
    return AIServiceProvider.Empty;
  }

  convertTools(mcpTools: MCPTool[]): string[] {
    void mcpTools;
    return [];
  }

  sanitizeSingleMessage(message: Message): Message {
    return message;
  }

  supportsTools(modelName: string): boolean {
    void modelName;
    return false;
  }

  estimateContextWindow(modelName: string): number {
    void modelName;
    return 0;
  }

  protected convertMessages(
    messages: Message[],
    systemPrompt?: string,
  ): string[] {
    void messages;
    void systemPrompt;
    return [];
  }

  protected async *doStreamChat(
    messages?: Message[],
    options?: {
      modelName?: string;
      systemPrompt?: string;
      sessionContext?: string;
    },
  ): AsyncGenerator<string, void, void> {
    void messages;
    void options;
    for (const chunk of this.compactChunks) {
      yield chunk;
    }
  }

  dispose(): void {}

  public shouldRetryForTest(error: unknown): boolean {
    return this.shouldRetry(error);
  }

  public setCompactChunks(chunks: string[]): void {
    this.compactChunks = chunks;
  }
}

describe('BaseAIService.shouldRetry', () => {
  const service = new TestBaseAIService('test-key');

  it('retries transient RESOURCE_EXHAUSTED 429 rate limits', () => {
    expect(
      service.shouldRetryForTest({
        status: 429,
        message: '429 RESOURCE_EXHAUSTED: Rate limit exceeded, please retry later',
      }),
    ).toBe(true);
  });

  it('does not retry spending cap 429 errors', () => {
    expect(
      service.shouldRetryForTest({
        status: 429,
        message:
          '429 RESOURCE_EXHAUSTED: spending cap reached for this project quota',
      }),
    ).toBe(false);
  });
});

describe('BaseAIService.sanitizeMessages', () => {
  const service = new TestBaseAIService('test-key');

  it('replaces malformed tool calls with repair guidance instead of resending them', () => {
    const sanitized = service.sanitizeMessages([
      {
        id: 'assistant-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [],
        tool_calls: [
          {
            id: 'call-bad',
            type: 'function',
            function: {
              name: 'readFile',
              arguments: '{"path":"foo.txt"',
            },
          },
        ],
      },
    ]);

    expect(sanitized).toHaveLength(1);
    expect(sanitized[0].tool_calls).toBeUndefined();
    expect(sanitized[0].content).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'text',
          text: expect.stringContaining(
            'invalid tool call arguments were removed',
          ),
        }),
      ]),
    );
    expect(
      (sanitized[0].content[0] as { type: 'text'; text: string }).text,
    ).toContain('readFile');
  });

  it('keeps valid tool calls while repairing malformed siblings', () => {
    const sanitized = service.sanitizeMessages([
      {
        id: 'assistant-2',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        content: [{ type: 'text', text: 'Working...' }],
        tool_calls: [
          {
            id: 'call-good',
            type: 'function',
            function: {
              name: 'listFiles',
              arguments: '{"path":"src"}',
            },
          },
          {
            id: 'call-bad',
            type: 'function',
            function: {
              name: 'readFile',
              arguments: '{path:"broken"}',
            },
          },
        ],
      },
      {
        id: 'tool-good',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'tool',
        tool_call_id: 'call-good',
        content: [{ type: 'text', text: 'ok' }],
      },
    ]);

    expect(sanitized).toHaveLength(2);
    expect(sanitized[0].tool_calls).toHaveLength(1);
    expect(sanitized[0].tool_calls?.[0].id).toBe('call-good');
    expect(
      sanitized[0].content.some(
        (part) =>
          part.type === 'text' &&
          part.text.includes('Treat each omitted tool call as a failed attempt'),
      ),
    ).toBe(true);
    expect(sanitized[1].tool_call_id).toBe('call-good');
  });
});

describe('stableStringify', () => {
  it('returns a string for undefined values', () => {
    expect(stableStringify(undefined)).toBe('"undefined"');
  });

  it('returns a stable string for bigint values', () => {
    expect(stableStringify(42n)).toBe('{"$bigint":"42"}');
  });
});

describe('BaseAIService.compact', () => {
  const createMessages = (): Message[] => [
    {
      id: 'user-1',
      sessionId: 'session-1',
      threadId: 'session-1',
      role: 'user',
      content: [{ type: 'text', text: 'Summarize this conversation.' }],
    },
  ];

  it('combines JSON and raw stream chunks into a trimmed summary', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunks([
      JSON.stringify({ content: 'Hello ' }),
      'world',
      JSON.stringify({ content: '  ' }),
    ]);

    await expect(service.compact(createMessages())).resolves.toBe('Hello world');
  });

  it('throws when compaction returns only empty output', async () => {
    const service = new TestBaseAIService('test-key');
    service.setCompactChunks(['', JSON.stringify({ content: '   ' })]);

    await expect(service.compact(createMessages())).rejects.toBeInstanceOf(
      AIServiceError,
    );
  });
});

describe('buildCompactionInstruction', () => {
  it('enforces a fixed compact schema and compression rules', () => {
    const instruction = buildCompactionInstruction([]);

    expect(instruction).toContain('Use EXACTLY these sections in this order');
    expect(instruction).toContain('1. Stable Context');
    expect(instruction).toContain('2. Key Decisions & Constraints');
    expect(instruction).toContain('3. Current State');
    expect(instruction).toContain('4. Recent Tool Results');
    expect(instruction).toContain('5. Next Actions');
    expect(instruction).toContain('Compression rules:');
    expect(instruction).toContain('Use terse bullet points, not prose paragraphs.');
    expect(instruction).toContain('Minimize adjectives, adverbs, filler, and repetition.');
    expect(instruction).toContain('Section limits:');
    expect(instruction).toContain('Stable Context: at most 6 bullets');
    expect(instruction).toContain('Recent Tool Results: at most 5 bullets');
  });

  it('treats source-marked compact summaries as residual anchors', () => {
    const instruction = buildCompactionInstruction([
      {
        id: 'summary-1',
        sessionId: 'session-1',
        threadId: 'session-1',
        role: 'assistant',
        source: 'compact-summary',
        content: [{ type: 'text', text: 'Previous compact summary' }],
      },
    ]);

    expect(instruction).toContain(
      'The first message is a previously accumulated compact summary',
    );
    expect(instruction).toContain('CRITICAL RESIDUAL RULE');
    expect(instruction).toContain(
      'You may tighten wording, remove duplication, and relocate items into the required sections',
    );
  });
});
