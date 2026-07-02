import type { Message } from '@/models/chat';
import { AIServiceError, type AIServiceProvider } from './types';
import type { CompactOptions } from './base-service-shared';

const COMPACTION_CONTAMINATION_SENTINEL = '<tool_call>';
const COMPACTION_TOOL_MARKUP_PATTERNS = [
  /<tool_call\b/i,
  /<\/tool_call>/i,
  /<function\s*=/i,
  /<parameter\s*=/i,
  /"tool_calls"\s*:/i,
  /"function_call"\s*:/i,
  /"tool_call_starts"\s*:/i,
] as const;

interface CompactionContext {
  options?: CompactOptions;
  streamChat: (
    messages: Message[],
    options: {
      modelName?: string;
      systemPrompt?: string;
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

function detectCompactionOutputContamination(text: string): string | null {
  for (const pattern of COMPACTION_TOOL_MARKUP_PATTERNS) {
    const match = text.match(pattern);
    if (match) {
      return match[0];
    }
  }

  return null;
}

function detectParsedChunkToolCallPayload(
  parsedChunk: Record<string, unknown>,
): string | null {
  const toolPayloadFields = ['tool_calls', 'function_call', 'tool_call_starts'];
  for (const fieldName of toolPayloadFields) {
    const fieldValue = parsedChunk[fieldName];
    const hasPayload = Array.isArray(fieldValue)
      ? fieldValue.length > 0
      : typeof fieldValue === 'object' && fieldValue !== null;
    if (hasPayload) {
      return `"${fieldName}"`;
    }
  }

  const contentValue = parsedChunk['content'];
  if (
    Array.isArray(contentValue) &&
    contentValue.some(
      (item) =>
        typeof item === 'object' &&
        item !== null &&
        (item as Record<string, unknown>).type === 'tool_call',
    )
  ) {
    return '"content[].type":"tool_call"';
  }

  return null;
}

function buildContaminatedCompactionSummary(
  summaryText: string,
  contaminationMarker: string,
): string {
  const trimmed = summaryText.trimEnd();
  if (
    trimmed.includes(COMPACTION_CONTAMINATION_SENTINEL) ||
    trimmed.includes(contaminationMarker)
  ) {
    return trimmed;
  }

  if (trimmed) {
    return `${trimmed}\n${COMPACTION_CONTAMINATION_SENTINEL}\n${contaminationMarker}`;
  }

  return `${COMPACTION_CONTAMINATION_SENTINEL}\n${contaminationMarker}`;
}

export async function compactMessages(
  messages: Message[],
  context: CompactionContext,
): Promise<string> {
  const internalAbortController = new AbortController();
  const upstreamAbortSignal = context.options?.signal;
  const abortFromUpstream = () => {
    internalAbortController.abort();
  };

  if (upstreamAbortSignal?.aborted) {
    internalAbortController.abort();
  } else {
    upstreamAbortSignal?.addEventListener('abort', abortFromUpstream, {
      once: true,
    });
  }

  const streamGenerator = context.streamChat(messages, {
    modelName: context.options?.modelName,
    systemPrompt: context.options?.systemPrompt,
    availableTools: context.options?.availableTools,
    config: context.options?.config,
    forceToolUse: false,
    disableToolUse: true,
    signal: internalAbortController.signal,
  });

  let summaryText = '';
  let abortedForContamination = false;
  try {
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

      const toolPayloadMarker = detectParsedChunkToolCallPayload(parsedChunk);
      if (toolPayloadMarker) {
        abortedForContamination = true;
        summaryText = buildContaminatedCompactionSummary(
          summaryText,
          toolPayloadMarker,
        );
        internalAbortController.abort();
        break;
      }

      if (parsedChunk.content && typeof parsedChunk.content === 'string') {
        const nextSummaryText = `${summaryText}${parsedChunk.content}`;
        const contaminationMarker =
          detectCompactionOutputContamination(nextSummaryText);
        if (contaminationMarker) {
          abortedForContamination = true;
          summaryText = buildContaminatedCompactionSummary(
            nextSummaryText,
            contaminationMarker,
          );
          internalAbortController.abort();
          break;
        }
        summaryText = nextSummaryText;
      }
    }
  } finally {
    upstreamAbortSignal?.removeEventListener('abort', abortFromUpstream);
  }

  if (abortedForContamination) {
    return summaryText.trim();
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
