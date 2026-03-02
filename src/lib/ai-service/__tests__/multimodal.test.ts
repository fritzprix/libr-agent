import { describe, it, expect } from 'vitest';
import { convertToGeminiMessages, convertSingleMessage as convertSingleGeminiMessage } from '../gemini/mapper';
import { OpenAIService } from '../openai';
import { AnthropicService } from '../anthropic';
import { Message } from '@/models/chat';

import { AIServiceConfig } from '../types';

type TestMessageResult = { role: string; content?: Array<Record<string, unknown>>; parts?: Array<Record<string, unknown>> };

class TestOpenAIService extends OpenAIService {
  constructor() {
    super('sk-1234567890abcdef1234567890abcdef', { provider: 'openai', modelId: 'gpt-4o' } as unknown as AIServiceConfig);
  }
  public testConvertSingleMessage(m: Message) {
    return this.convertSingleMessage(m);
  }
}

class TestAnthropicService extends AnthropicService {
  constructor() {
    super('sk-ant-1234567890abcdef1234567890abcdef', { provider: 'anthropic', modelId: 'claude-3-5-sonnet-20241022' } as unknown as AIServiceConfig);
  }
  public testConvertSingleMessage(m: Message) {
    return this.convertSingleMessage(m);
  }
}

describe('Multimodal Payload Construction', () => {
  const multimodalMessage: Message = {
    id: 'test-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'user',
    content: [
      { type: 'text', text: 'Analyze this data' },
      { type: 'image', data: 'imagedata', mimeType: 'image/png' },
      { type: 'audio', data: 'audiodata', mimeType: 'audio/mp3' },
    ],
  };

  it('formats content correctly for OpenAI', () => {
    const service = new TestOpenAIService();
    const result = service.testConvertSingleMessage(multimodalMessage) as TestMessageResult;
    
    expect(result.role).toBe('user');
    expect(Array.isArray(result.content)).toBe(true);
    expect(result.content!).toHaveLength(3);
    
    expect(result.content![0]).toEqual({ type: 'text', text: 'Analyze this data' });
    expect(result.content![1]).toEqual({ type: 'image_url', image_url: { url: 'data:image/png;base64,imagedata' } });
    expect(result.content![2]).toEqual({ type: 'input_audio', input_audio: { data: 'audiodata', format: 'mp3' } });
  });

  it('formats content correctly for Anthropic', () => {
    const service = new TestAnthropicService();
    const result = service.testConvertSingleMessage(multimodalMessage) as TestMessageResult;
    
    expect(result.role).toBe('user');
    expect(Array.isArray(result.content)).toBe(true);
    // Anthropic filters out audio natively in our adapter with a placeholder text until official support
    expect(result.content!).toHaveLength(3);
    
    expect(result.content![0]).toEqual({ type: 'text', text: 'Analyze this data' });
    expect(result.content![1]).toEqual({ 
      type: 'image', 
      source: { type: 'base64', media_type: 'image/png', data: 'imagedata' } 
    });
    expect(result.content![2].text).toContain('[Unsupported content format for Anthropic: audio]');
  });

  it('formats content correctly for Gemini using constructGeminiMessages', () => {
    const result = convertToGeminiMessages([multimodalMessage]);
    
    expect(result).toHaveLength(1);
    expect(result[0].role).toBe('user');
    expect(result[0].parts).toHaveLength(3);
    
    expect(result[0].parts![0]).toEqual({ text: 'Analyze this data' });
    expect(result[0].parts![1]).toEqual({ inlineData: { mimeType: 'image/png', data: 'imagedata' } });
    expect(result[0].parts![2]).toEqual({ inlineData: { mimeType: 'audio/mp3', data: 'audiodata' } });
  });

  it('formats content correctly for Gemini using convertSingleMessage', () => {
    const result = convertSingleGeminiMessage(multimodalMessage) as TestMessageResult;
    
    expect(result.role).toBe('user');
    expect(result.parts).toHaveLength(3);
    
    expect(result.parts![1]).toEqual({ inlineData: { mimeType: 'image/png', data: 'imagedata' } });
    expect(result.parts![2]).toEqual({ inlineData: { mimeType: 'audio/mp3', data: 'audiodata' } });
  });
});
