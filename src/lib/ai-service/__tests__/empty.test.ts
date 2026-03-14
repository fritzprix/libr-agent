import { describe, it, expect, vi } from 'vitest';
import { EmptyAIService } from '../empty';
import { AIServiceProvider, AIServiceError } from '../types';
import { MCPTool } from '@/lib/mcp';
import { Message } from '@/models/chat';

describe('EmptyAIService', () => {
  it('should construct an instance with a dummy API key', () => {
    const service = new EmptyAIService();
    expect(service).toBeInstanceOf(EmptyAIService);
  });

  it('should return the correct AIServiceProvider enum', () => {
    const service = new EmptyAIService();
    expect(service.getProvider()).toBe(AIServiceProvider.Empty);
  });

  it('should always throw when converting tools', () => {
    const service = new EmptyAIService();
    const mockTools: MCPTool[] = [];

    expect(() => service.convertTools(mockTools)).toThrow(AIServiceError);
    expect(() => service.convertTools(mockTools)).toThrow('Tool conversion not supported');
  });

  it('should throw an error and yield empty string when streaming chat', async () => {
    const service = new EmptyAIService();
    const mockMessages: Message[] = [];

    // We expect the generator to throw, so we catch it
    let caughtError: Error | undefined;
    const output: string[] = [];

    try {
      // @ts-expect-error accessing protected method for testing
      for await (const chunk of service.doStreamChat(mockMessages)) {
        output.push(chunk);
      }
    } catch (e) {
      caughtError = e as Error;
    }

    expect(caughtError).toBeInstanceOf(AIServiceError);
    expect(caughtError?.message).toContain('EmptyAIService does not support streaming chat');
    expect(output).toEqual(['']); // Yields empty string before throwing
  });

  it('should return empty array when converting messages', () => {
    const service = new EmptyAIService();
    const mockMessages: Message[] = [];

    // @ts-expect-error accessing protected method for testing
    const result = service.convertMessages(mockMessages);
    expect(result).toEqual([]);
  });

  it('should safely dispose without doing anything', () => {
    const service = new EmptyAIService();
    // It should not throw any error on dispose
    expect(() => service.dispose()).not.toThrow();
  });
});
