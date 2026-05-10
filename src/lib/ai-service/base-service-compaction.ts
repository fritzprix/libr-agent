import type { Message } from '@/models/chat';
import { AIServiceError, type AIServiceProvider } from './types';
import type { CompactOptions } from './base-service-shared';
import { assembleRequestLayout } from './base-service-context';

interface CompactionContext {
  options?: CompactOptions;
  prepareContextInjection: (
    systemPrompt: string | undefined,
    sessionContext: string | undefined,
    messages: Message[],
  ) => {
    systemPrompt: string | undefined;
    sessionContext?: string;
    messages: Message[];
  };
  streamChat: (
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
      sessionContext?: string;
      availableTools?: CompactOptions['availableTools'];
      config?: CompactOptions['config'];
      forceToolUse: false;
      disableToolUse: true;
      signal?: AbortSignal;
    },
  ) => AsyncGenerator<string, void, void>;
  isAborted: () => boolean;
  getProvider: () => AIServiceProvider;
}

export async function compactMessages(
  messages: Message[],
  context: CompactionContext,
): Promise<string> {
  const {
    systemPrompt: effectiveSystemPrompt,
    sessionContext: effectiveSessionContext,
    messages: effectiveMessages,
  } = assembleRequestLayout(
    {
      systemPrompt: context.options?.systemPrompt,
      sessionContext: context.options?.sessionContext,
      messages,
    },
    {
      prepareContextInjection: context.prepareContextInjection,
    },
  );

  const streamGenerator = context.streamChat(effectiveMessages, {
    modelName: context.options?.modelName,
    systemPrompt: effectiveSystemPrompt,
    sessionContext: effectiveSessionContext,
    availableTools: context.options?.availableTools,
    config: context.options?.config,
    forceToolUse: false,
    disableToolUse: true,
    signal: context.options?.signal,
  });

  let summaryText = '';
  for await (const chunk of streamGenerator) {
    if (context.isAborted()) {
      throw new Error('Compaction request aborted');
    }

    let parsedChunk: Record<string, unknown>;
    try {
      parsedChunk = JSON.parse(chunk);
    } catch {
      parsedChunk = { content: chunk };
    }

    if (parsedChunk.content && typeof parsedChunk.content === 'string') {
      summaryText += parsedChunk.content;
    }
  }

  const trimmedSummary = summaryText.trim();
  if (!trimmedSummary) {
    throw new AIServiceError(
      'compact() received an empty response from streamChat',
      context.getProvider(),
    );
  }

  return trimmedSummary;
}
