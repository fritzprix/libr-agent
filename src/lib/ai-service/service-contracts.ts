import type { MCPTool, SamplingOptions, SamplingResponse } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import type { AIServiceConfig, ModelInfo } from './types';

export interface AIStreamChatOptions {
  modelName?: string;
  systemPrompt?: string;
  availableTools?: MCPTool[];
  config?: AIServiceConfig;
  forceToolUse?: boolean;
  disableToolUse?: boolean;
  signal?: AbortSignal;
}

export interface AISampleTextOptions {
  modelName?: string;
  samplingOptions?: SamplingOptions;
  config?: AIServiceConfig;
  signal?: AbortSignal;
}

export interface AICompactOptions {
  modelName?: string;
  config?: AIServiceConfig;
  systemPrompt?: string;
  availableTools?: MCPTool[];
  signal?: AbortSignal;
}

export interface AIStreamingService {
  streamChat(
    messages: Message[],
    options?: AIStreamChatOptions,
  ): AsyncGenerator<string, void, void>;

  cancel(): void;
}

export interface AISamplingService {
  sampleText(
    prompt: string,
    options?: AISampleTextOptions,
  ): Promise<SamplingResponse>;
}

export interface AIModelDiscoveryService {
  listModels(): Promise<ModelInfo[]>;
}

export interface AIToolSupportService {
  convertTools(mcpTools: MCPTool[]): unknown[];
  supportsTools(modelName: string): boolean;
  estimateContextWindow(modelName: string): number;
}

export interface AICompactionService {
  compact(messages: Message[], options?: AICompactOptions): Promise<string>;
}

export interface AIMessageSanitizationService {
  sanitizeMessages(messages: Message[]): Message[];
  sanitizeSingleMessage(message: Message): Message | null;
}

export interface AIServiceLifecycle {
  dispose(): void;
}

export type AICompletionExecutionService = AIStreamingService &
  AIModelDiscoveryService &
  AIMessageSanitizationService &
  AIServiceLifecycle;

export type AIModelLookupService = AIModelDiscoveryService;

export type AIContextCompactionService = AICompactionService;

export interface IAIService
  extends AIStreamingService,
    AISamplingService,
    AIModelDiscoveryService,
    AIToolSupportService,
    AICompactionService,
    AIMessageSanitizationService,
    AIServiceLifecycle {}
