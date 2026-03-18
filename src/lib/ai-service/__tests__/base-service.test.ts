import { describe, expect, it } from 'vitest';
import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import { BaseAIService } from '../base-service';
import { AIServiceProvider } from '../types';

class TestBaseAIService extends BaseAIService<string, string> {
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

  protected async *doStreamChat(): AsyncGenerator<string, void, void> {
    yield '';
  }

  dispose(): void {}

  public shouldRetryForTest(error: unknown): boolean {
    return this.shouldRetry(error);
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
