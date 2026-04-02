import type { ChatCompletionTool as OpenAIChatCompletionTool } from 'openai/resources/chat/completions.mjs';
import OpenAI from 'openai';

import { stableHashKeyPart, stableStringify } from '../base-service';
import type {
  OpenAIMessageFingerprint,
  OpenAIPromptSnapshot,
  OpenAIResponseUsageDetails,
  OpenAILoggerLike,
} from './types';

function contentToFingerprintString(content: unknown): string {
  if (typeof content === 'string') {
    return content;
  }

  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (
          typeof part === 'object' &&
          part !== null &&
          'type' in part &&
          part.type === 'text' &&
          'text' in part &&
          typeof part.text === 'string'
        ) {
          return part.text;
        }
        return stableStringify(part);
      })
      .join('\n');
  }

  return stableStringify(content ?? '');
}

function classifyMessageContentTag(
  role: string,
  content: string,
): OpenAIMessageFingerprint['contentTag'] {
  if (
    role === 'user' &&
    content.startsWith('[Current session context — background reference only')
  ) {
    return 'session_context';
  }

  if (
    role === 'user' &&
    content.startsWith('Tool result media from tool_call_id=')
  ) {
    return 'tool_result_media';
  }

  return 'regular';
}

export function fingerprintOpenAIMessage(
  message: OpenAI.Chat.Completions.ChatCompletionMessageParam,
): OpenAIMessageFingerprint {
  if (message.role === 'tool') {
    const content = contentToFingerprintString(message.content);
    return {
      role: message.role,
      contentLength: content.length,
      contentHash: stableHashKeyPart(content),
      contentTag: classifyMessageContentTag(message.role, content),
      toolCallCount: 0,
      toolCallId: message.tool_call_id,
      toolCallIdHash: stableHashKeyPart(message.tool_call_id ?? ''),
    };
  }

  const content = contentToFingerprintString(message.content);
  const toolCalls =
    'tool_calls' in message && Array.isArray(message.tool_calls)
      ? message.tool_calls
      : [];

  return {
    role: message.role,
    contentLength: content.length,
    contentHash: stableHashKeyPart(content),
    contentTag: classifyMessageContentTag(message.role, content),
    toolCallCount: toolCalls.length,
    toolCallNames: toolCalls.map((toolCall) =>
      'function' in toolCall &&
      typeof toolCall.function === 'object' &&
      toolCall.function !== null &&
      'name' in toolCall.function &&
      typeof toolCall.function.name === 'string'
        ? toolCall.function.name
        : 'custom',
    ),
    toolCallHash: stableHashKeyPart(stableStringify(toolCalls)),
  };
}

function buildPromptSnapshot(args: {
  mode: 'stream' | 'non-stream';
  model: string;
  systemPrompt?: string;
  request: {
    prompt_cache_key?: string;
    prompt_cache_retention?: 'in_memory' | '24h';
    cache_prompt?: boolean;
  };
  messages: OpenAI.Chat.Completions.ChatCompletionMessageParam[];
  tools?: OpenAIChatCompletionTool[];
}): OpenAIPromptSnapshot {
  const messageFingerprints = args.messages.map((message) =>
    fingerprintOpenAIMessage(message),
  );
  const serializedFingerprints = stableStringify(messageFingerprints);
  const toolsPayload = stableStringify(args.tools ?? []);

  return {
    mode: args.mode,
    model: args.model,
    systemPromptLength: args.systemPrompt?.length ?? 0,
    systemPromptHash: stableHashKeyPart(args.systemPrompt ?? ''),
    toolsHash: stableHashKeyPart(toolsPayload),
    toolCount: args.tools?.length ?? 0,
    messagesFingerprintHash: stableHashKeyPart(serializedFingerprints),
    messageFingerprints,
    promptCacheKey: args.request.prompt_cache_key,
    promptCacheRetention: args.request.prompt_cache_retention,
    compatibleCachePrompt: args.request.cache_prompt ?? false,
  };
}

export class OpenAIPromptDiagnosticsTracker {
  private readonly lastPromptSnapshots = new Map<
    'stream' | 'non-stream',
    OpenAIPromptSnapshot
  >();

  constructor(private readonly logger: OpenAILoggerLike) {}

  createRequestId(): string {
    return `req_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
  }

  logPromptCacheMetadata(args: {
    mode: 'stream' | 'non-stream';
    model: string;
    request: {
      model: string;
      prompt_cache_key?: string;
      prompt_cache_retention?: 'in_memory' | '24h';
      cache_prompt?: boolean;
    };
    usage: OpenAIResponseUsageDetails & {
      prompt_tokens?: number;
      completion_tokens?: number;
      total_tokens?: number;
    };
  }): void {
    const cachedPromptTokens =
      args.usage.prompt_tokens_details?.cached_tokens ??
      args.usage.prompt_cache_hit_tokens;

    this.logger.info('OpenAI prompt cache metadata', {
      mode: args.mode,
      model: args.model,
      promptCacheKey: args.request.prompt_cache_key,
      promptCacheRetention: args.request.prompt_cache_retention,
      compatibleCachePrompt: args.request.cache_prompt ?? false,
      promptTokens: args.usage.prompt_tokens,
      completionTokens: args.usage.completion_tokens,
      totalTokens: args.usage.total_tokens,
      cachedPromptTokens,
      promptTokensDetails: args.usage.prompt_tokens_details,
      completionTokensDetails: args.usage.completion_tokens_details,
      promptCacheHitTokens: args.usage.prompt_cache_hit_tokens,
    });
  }

  logPromptDiagnostics(args: {
    mode: 'stream' | 'non-stream';
    model: string;
    systemPrompt?: string;
    request: {
      prompt_cache_key?: string;
      prompt_cache_retention?: 'in_memory' | '24h';
      cache_prompt?: boolean;
    };
    messages: OpenAI.Chat.Completions.ChatCompletionMessageParam[];
    tools?: OpenAIChatCompletionTool[];
  }): void {
    const snapshot = buildPromptSnapshot(args);

    this.logger.debug('OpenAI prompt diagnostics', {
      mode: args.mode,
      model: args.model,
      promptCacheKey: args.request.prompt_cache_key,
      promptCacheRetention: args.request.prompt_cache_retention,
      compatibleCachePrompt: args.request.cache_prompt ?? false,
      systemPromptLength: snapshot.systemPromptLength,
      systemPromptHash: snapshot.systemPromptHash,
      toolCount: snapshot.toolCount,
      toolsHash: snapshot.toolsHash,
      messageCount: snapshot.messageFingerprints.length,
      messagesFingerprintHash: snapshot.messagesFingerprintHash,
      messageFingerprints: snapshot.messageFingerprints,
    });
    this.logPromptDrift(snapshot);
  }

  logFetchDiagnostics(args: {
    mode: 'stream' | 'non-stream';
    requestId: string;
    model: string;
    request: {
      model: string;
      prompt_cache_key?: string;
      prompt_cache_retention?: 'in_memory' | '24h';
      cache_prompt?: boolean;
      tool_choice?: unknown;
      max_completion_tokens?: number | null;
      max_tokens?: number | null;
      messages: OpenAI.Chat.Completions.ChatCompletionMessageParam[];
      tools?: OpenAIChatCompletionTool[];
      reasoning_effort?: string | null;
    };
  }): void {
    const bodyFingerprint = stableHashKeyPart(
      stableStringify({
        model: args.request.model,
        messages: args.request.messages.map((message) =>
          fingerprintOpenAIMessage(message),
        ),
        tools: args.request.tools ?? [],
        tool_choice: args.request.tool_choice,
        max_completion_tokens: args.request.max_completion_tokens,
        max_tokens: args.request.max_tokens,
        prompt_cache_key: args.request.prompt_cache_key,
        prompt_cache_retention: args.request.prompt_cache_retention,
        cache_prompt: args.request.cache_prompt,
        reasoning_effort: args.request.reasoning_effort,
      }),
    );

    this.logger.debug('OpenAI fetch diagnostics', {
      mode: args.mode,
      requestId: args.requestId,
      model: args.model,
      promptCacheKey: args.request.prompt_cache_key,
      promptCacheRetention: args.request.prompt_cache_retention,
      compatibleCachePrompt: args.request.cache_prompt ?? false,
      bodyFingerprint,
      messageCount: args.request.messages.length,
      toolCount: args.request.tools?.length ?? 0,
      toolChoice: args.request.tool_choice,
      maxCompletionTokens: args.request.max_completion_tokens,
      maxTokens: args.request.max_tokens,
      reasoningEffort: args.request.reasoning_effort,
    });
  }

  private logPromptDrift(snapshot: OpenAIPromptSnapshot): void {
    const previous = this.lastPromptSnapshots.get(snapshot.mode);
    this.lastPromptSnapshots.set(snapshot.mode, snapshot);

    if (!previous) {
      return;
    }

    const minMessageCount = Math.min(
      previous.messageFingerprints.length,
      snapshot.messageFingerprints.length,
    );
    let firstDivergenceIndex = -1;
    for (let index = 0; index < minMessageCount; index += 1) {
      if (
        stableStringify(previous.messageFingerprints[index]) !==
        stableStringify(snapshot.messageFingerprints[index])
      ) {
        firstDivergenceIndex = index;
        break;
      }
    }

    if (
      firstDivergenceIndex === -1 &&
      previous.messageFingerprints.length !==
        snapshot.messageFingerprints.length
    ) {
      firstDivergenceIndex = minMessageCount;
    }

    const firstDivergenceComponent =
      previous.model !== snapshot.model
        ? 'model'
        : previous.systemPromptHash !== snapshot.systemPromptHash
          ? 'system_prompt'
          : previous.toolsHash !== snapshot.toolsHash
            ? 'tools'
            : firstDivergenceIndex >= 0
              ? 'messages'
              : 'none';

    const commonPrefixMessages =
      firstDivergenceComponent === 'messages'
        ? firstDivergenceIndex
        : Math.min(
            previous.messageFingerprints.length,
            snapshot.messageFingerprints.length,
          );

    this.logger.debug('OpenAI prompt cache drift', {
      mode: snapshot.mode,
      previousModel: previous.model,
      model: snapshot.model,
      previousPromptCacheKey: previous.promptCacheKey,
      promptCacheKey: snapshot.promptCacheKey,
      firstDivergenceComponent,
      firstDivergenceIndex:
        firstDivergenceComponent === 'messages'
          ? firstDivergenceIndex
          : undefined,
      commonPrefixMessages,
      previousMessageCount: previous.messageFingerprints.length,
      messageCount: snapshot.messageFingerprints.length,
      systemPromptChanged:
        previous.systemPromptHash !== snapshot.systemPromptHash,
      toolsChanged: previous.toolsHash !== snapshot.toolsHash,
      messagesChanged:
        previous.messagesFingerprintHash !== snapshot.messagesFingerprintHash,
      previousFingerprintHash: previous.messagesFingerprintHash,
      fingerprintHash: snapshot.messagesFingerprintHash,
      previousMessageAtDivergence:
        firstDivergenceIndex >= 0
          ? previous.messageFingerprints[firstDivergenceIndex]
          : undefined,
      currentMessageAtDivergence:
        firstDivergenceIndex >= 0
          ? snapshot.messageFingerprints[firstDivergenceIndex]
          : undefined,
    });
  }
}
