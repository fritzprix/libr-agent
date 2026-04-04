import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import type { AIServiceConfig, SamplingOptions } from './types';

export interface StreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
  forceToolUse?: boolean;
  disableToolUse?: boolean;
}

export interface PrepareStreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
  disableToolUse?: boolean;
}

export interface PrepareStreamChatResult<TProviderTool> {
  config: AIServiceConfig;
  tools?: TProviderTool[];
  sanitizedMessages: Message[];
}

export interface StreamingErrorContext {
  messages: Message[];
  options: PrepareStreamChatOptions;
  config: AIServiceConfig;
}

export interface CompactOptions {
  modelName?: string;
  config?: AIServiceConfig;
  systemPrompt?: string;
  sessionContext?: string;
  availableTools?: MCPTool[];
}

export interface SampleTextOptions {
  modelName?: string;
  samplingOptions?: SamplingOptions;
  config?: AIServiceConfig;
}

export interface SyntheticSessionContextMessageOptions {
  idPrefix?: string;
  contentText?: string;
  metadata?: Record<string, unknown>;
  sessionIdFallback?: string;
  threadIdFallback?: string;
  createdAt?: Date;
}
