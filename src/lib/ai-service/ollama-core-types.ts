import type { TokenUsage } from './types';

export interface Logger {
  debug: (message: string, ...args: unknown[]) => void;
  info: (message: string, ...args: unknown[]) => void;
  warn: (message: string, ...args: unknown[]) => void;
  error: (message: string, ...args: unknown[]) => void;
}

export const noopLogger: Logger = {
  debug: () => {},
  info: () => {},
  warn: () => {},
  error: () => {},
};

export const consoleLogger: Logger = {
  debug: (message: string, ...args: unknown[]) =>
    console.log('[DEBUG]', message, ...args),
  info: (message: string, ...args: unknown[]) =>
    console.log('[INFO]', message, ...args),
  warn: (message: string, ...args: unknown[]) =>
    console.warn('[WARN]', message, ...args),
  error: (message: string, ...args: unknown[]) =>
    console.error('[ERROR]', message, ...args),
};

export interface SimpleOllamaMessage {
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  images?: string[];
  tool_calls?: Array<{
    id: string;
    type: 'function';
    function: {
      name: string;
      arguments: Record<string, unknown>;
    };
  }>;
  tool_call_id?: string;
}

export interface ProcessedChunk {
  content?: string;
  thinking?: string;
  tool_calls?: Array<{
    id: string;
    type: string;
    function: {
      name: string;
      arguments: string;
    };
  }>;
  usage?: TokenUsage;
  error?: string;
}

export interface OllamaToolCallAccumulator {
  id: string;
  name: string;
  partialJson: string;
  index: number;
  yielded: boolean;
  lastChunkTime: number;
}
