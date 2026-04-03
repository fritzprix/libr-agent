import type { MCPTool } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { normalizeAvailableTools } from './base-service-utils';
import type {
  PrepareStreamChatOptions,
  PrepareStreamChatResult,
} from './base-service-shared';
import type { AIServiceConfig } from './types';

interface StreamPreparationContext<TProviderTool> {
  options: PrepareStreamChatOptions;
  messages: Message[];
  mergeConfig: (options?: { config?: AIServiceConfig }) => AIServiceConfig;
  convertTools: (tools: MCPTool[]) => TProviderTool[];
  sanitizeMessages: (messages: Message[]) => Message[];
}

export function prepareStreamChatRequest<TProviderTool>(
  context: StreamPreparationContext<TProviderTool>,
): PrepareStreamChatResult<TProviderTool> {
  const config = context.mergeConfig(context.options);

  const normalizedTools = context.options.availableTools
    ? normalizeAvailableTools(context.options.availableTools)
    : undefined;

  const tools = normalizedTools
    ? context.convertTools(normalizedTools)
    : undefined;

  const sanitizedMessages = context.sanitizeMessages(context.messages);

  return { config, tools, sanitizedMessages };
}
