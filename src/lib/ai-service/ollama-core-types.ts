import { getLogger } from '../logger';
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

export function createOllamaCoreLogger(contextName = 'OllamaCore'): Logger {
  const logger = getLogger(contextName);
  return {
    debug: (message: string, ...args: unknown[]) =>
      logger.debug(message, ...args),
    info: (message: string, ...args: unknown[]) =>
      logger.info(message, ...args),
    warn: (message: string, ...args: unknown[]) =>
      logger.warn(message, ...args),
    error: (message: string, ...args: unknown[]) =>
      logger.error(message, ...args),
  };
}

export const consoleLogger: Logger = createOllamaCoreLogger('OllamaCore');

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

export type {
  ThinkTagFeedResult,
  ThinkTagStreamMode,
  ThinkTagStreamState,
} from './think-tag-stream-parser';
