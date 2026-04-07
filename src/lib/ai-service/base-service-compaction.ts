import type { Message } from '@/models/chat';
import { estimateTextTokens } from '@/lib/message-preprocessor';
import { AIServiceError, type AIServiceProvider } from './types';
import type { CompactOptions } from './base-service-shared';
import {
  buildCompactionInstruction,
  createCompactionInstructionMessage,
} from './base-service-context';

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
    },
  ) => AsyncGenerator<string, void, void>;
  isAborted: () => boolean;
  getProvider: () => AIServiceProvider;
}

function estimateSummaryTokens(summary: string): number {
  return Math.ceil(estimateTextTokens(summary) * 1.1);
}

function createRecursiveSummarySeed(
  summary: string,
  sourceMessages: Message[],
  passIndex: number,
): Message {
  const referenceMessage = sourceMessages[sourceMessages.length - 1];

  return {
    id: `compact-summary-recursive-${passIndex}`,
    sessionId: referenceMessage?.sessionId ?? 'internal',
    threadId:
      referenceMessage?.threadId ?? referenceMessage?.sessionId ?? 'internal',
    role: 'user',
    content: [{ type: 'text', text: summary }],
    createdAt: referenceMessage?.createdAt ?? new Date(),
  };
}

async function runCompactionPass(
  messages: Message[],
  context: CompactionContext,
  recursivePass: boolean,
): Promise<string> {
  const compactMessages = [...messages];
  const instruction = buildCompactionInstruction(compactMessages, {
    targetMaxTokens: context.options?.targetMaxTokens,
    hardMaxTokens: context.options?.hardMaxTokens,
    recursivePass,
  });
  compactMessages.push(createCompactionInstructionMessage(instruction));

  const {
    systemPrompt: effectiveSystemPrompt,
    sessionContext: effectiveSessionContext,
    messages: effectiveMessages,
  } = context.prepareContextInjection(
    context.options?.systemPrompt,
    context.options?.sessionContext,
    compactMessages,
  );

  const streamGenerator = context.streamChat(effectiveMessages, {
    modelName: context.options?.modelName,
    systemPrompt: effectiveSystemPrompt,
    sessionContext: effectiveSessionContext,
    availableTools: context.options?.availableTools,
    config: context.options?.config,
    forceToolUse: false,
    disableToolUse: true,
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

export async function compactMessages(
  messages: Message[],
  context: CompactionContext,
): Promise<string> {
  const maxRecursivePasses = context.options?.maxRecursivePasses ?? 0;
  const targetMaxTokens = context.options?.targetMaxTokens;
  const hardMaxTokens = context.options?.hardMaxTokens;

  let currentMessages = messages;
  let recursivePassesUsed = 0;
  let summary = await runCompactionPass(currentMessages, context, false);

  while (true) {
    const estimatedTokens = estimateSummaryTokens(summary);
    const exceedsTarget =
      targetMaxTokens !== undefined && estimatedTokens > targetMaxTokens;
    const exceedsHardMax =
      hardMaxTokens !== undefined && estimatedTokens > hardMaxTokens;

    if (exceedsHardMax && recursivePassesUsed >= maxRecursivePasses) {
      throw new AIServiceError(
        `compact() summary exceeded hard cap after bounded recursive compaction (estimated ${estimatedTokens} > hard max ${hardMaxTokens})`,
        context.getProvider(),
      );
    }

    if (
      (!exceedsTarget && !exceedsHardMax) ||
      recursivePassesUsed >= maxRecursivePasses
    ) {
      return summary;
    }

    recursivePassesUsed += 1;
    currentMessages = [
      createRecursiveSummarySeed(summary, currentMessages, recursivePassesUsed),
    ];
    summary = await runCompactionPass(currentMessages, context, true);
  }
}
