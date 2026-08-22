import type { TokenUsage } from '@/lib/ai-service/types';
import {
  isParsedDirectToolCall,
  isParsedIndexedToolCallDelta,
  parseStreamChunk,
} from '@/lib/ai-service/stream-events';
import { reportLLMStreamingIssue } from '@/lib/backend/agent-commands';
import type {
  MCPContent,
  MCPTextContent,
  MCPThinkingContent,
  MCPToolCallContent,
} from '@/lib/mcp';
import type { ToolCall } from '@/models/chat';
import type { Settings } from '@/lib/services/settings-service';
import { getLogger } from '@/lib/logger';
import {
  estimateOutputBudgetTokens,
  reasoningBudgetThresholdTokens,
} from '@/lib/ai-service/openai/reasoning-budget';
import {
  detectRepeatedThinkingLoop,
  REPEATED_THINKING_TAIL_CHARS,
  REPEATED_THINKING_MIN_PATTERN_LENGTH,
  REPEATED_THINKING_MIN_REPETITIONS,
} from '../repeatedThinkingDetector';
import {
  detectRepeatedTextLoop,
  type RepeatedTailDetectorConfig,
} from '../repeatedTailDetector';

const logger = getLogger('stream-accumulator');

const STREAMING_TEXT_THROTTLE_MS = 50;
const STREAMING_TOOL_CALL_THROTTLE_MS = 100;
const REPEATED_TAIL_CHECK_INTERVAL = 5;

export interface StreamAccumulatorOptions {
  /**
   * When set, abort+retry once non-tool output (thinking and/or content, or
   * provider completion_tokens) reaches 90% of this value with no tool calls.
   */
  reasoningBudgetMaxTokens?: number;
}

export interface StreamAccumulatorState {
  content: MCPContent[];
  indexedToolCalls: Map<number, ToolCall>;
  directToolCalls: ToolCall[];
  thinkingStartTime?: number;
  currentThinkingTime?: number;
  currentThinkingText?: string;
  finalUsage?: TokenUsage;
  firstChunkTime?: number;
  thinkingSignature?: string;
  currentStreamingText: string;
}

export class StreamAccumulator {
  public content: MCPContent[] = [];
  public activeToolCallIndices = new Map<number, number>();
  public indexedToolCalls = new Map<number, ToolCall>();
  public directToolCalls: ToolCall[] = [];

  public thinkingStartTime?: number;
  public currentThinkingTime?: number;
  public currentThinkingText?: string;
  public finalUsage?: TokenUsage;
  public firstChunkTime?: number;
  public thinkingSignature?: string;

  private repeatedThinkingIssueReported = false;
  private repeatedThinkingCheckCounter = 0;
  private thinkingConfig?: RepeatedTailDetectorConfig;
  private currentStreamingText = '';
  private hasToolCallInStream = false;
  private repeatedTextIssueReported = false;
  private repeatedTextCheckCounter = 0;
  private reasoningBudgetIssueReported = false;
  private readonly reasoningBudgetThreshold?: number;

  private sessionId: string;
  private responseMessageId: string;
  private settingsRef: React.MutableRefObject<Settings>;
  private startTime: number;

  constructor(
    sessionId: string,
    responseMessageId: string,
    settingsRef: React.MutableRefObject<Settings>,
    startTime: number,
    options?: StreamAccumulatorOptions,
  ) {
    this.sessionId = sessionId;
    this.responseMessageId = responseMessageId;
    this.settingsRef = settingsRef;
    this.startTime = startTime;
    if (
      options?.reasoningBudgetMaxTokens != null &&
      Number.isFinite(options.reasoningBudgetMaxTokens) &&
      options.reasoningBudgetMaxTokens >= 1
    ) {
      this.reasoningBudgetThreshold = reasoningBudgetThresholdTokens(
        options.reasoningBudgetMaxTokens,
      );
    }
  }

  /**
   * Abort when non-tool output burns ~90% of maxTokens.
   * Covers thinking-channel dumps and content-only runaways (common on some
   * OpenAI-compatible hosts). Skips once any tool call appears in the stream.
   */
  private tryReportReasoningBudgetExceeded(): boolean {
    if (
      this.reasoningBudgetIssueReported ||
      this.reasoningBudgetThreshold == null ||
      this.hasToolCallInStream
    ) {
      return false;
    }

    const thinkingText = this.currentThinkingText ?? '';
    const contentText = this.currentStreamingText;
    const estimatedTokens = estimateOutputBudgetTokens({
      thinkingText,
      contentText,
      completionTokens: this.finalUsage?.completionTokens,
    });
    if (estimatedTokens < this.reasoningBudgetThreshold) {
      return false;
    }

    this.reasoningBudgetIssueReported = true;
    // Suppress loop detector noise once budget abort owns recovery.
    this.repeatedThinkingIssueReported = true;
    this.repeatedTextIssueReported = true;
    const observedTailChars = Math.max(thinkingText.length, contentText.length);
    logger.warn('Output/reasoning budget exceeded during streaming', {
      sessionId: this.sessionId,
      responseMessageId: this.responseMessageId,
      thinkingChars: thinkingText.length,
      contentChars: contentText.length,
      completionTokens: this.finalUsage?.completionTokens ?? null,
      estimatedOutputTokens: estimatedTokens,
      reasoningBudgetThreshold: this.reasoningBudgetThreshold,
    });
    void reportLLMStreamingIssue({
      sessionId: this.sessionId,
      responseMessageId: this.responseMessageId,
      issueKind: 'REASONING_BUDGET_EXCEEDED',
      observedTailChars,
      patternLength: this.reasoningBudgetThreshold,
      repetitionCount: estimatedTokens,
    }).catch((error: unknown) => {
      logger.warn('Failed to report output/reasoning budget exceeded', {
        sessionId: this.sessionId,
        responseMessageId: this.responseMessageId,
        error,
      });
    });
    return true;
  }

  /**
   * End-of-stream / usage gate: catch denser tokenization than chars/4 after
   * the provider reports completion_tokens.
   */
  public finalizeOutputBudgetCheck(): boolean {
    return this.tryReportReasoningBudgetExceeded();
  }

  private getThinkingConfig(): RepeatedTailDetectorConfig {
    if (!this.thinkingConfig) {
      this.thinkingConfig = {
        minPatternLength:
          this.settingsRef.current.advanced?.thinkingLoopMinPatternLength ??
          REPEATED_THINKING_MIN_PATTERN_LENGTH,
        minRepetitions:
          this.settingsRef.current.advanced?.thinkingLoopMinRepetitions ??
          REPEATED_THINKING_MIN_REPETITIONS,
        tailChars: REPEATED_THINKING_TAIL_CHARS,
      };
    }
    return this.thinkingConfig;
  }

  public getStreamingToolCalls(): ToolCall[] {
    return [
      ...[...this.indexedToolCalls.entries()]
        .sort(([leftIndex], [rightIndex]) => leftIndex - rightIndex)
        .map(([, toolCall]) => toolCall),
      ...this.directToolCalls,
    ];
  }

  public processChunk(rawChunk: unknown): {
    hasToolCallUpdate: boolean;
    shouldFlushToolCallImmediately: boolean;
  } {
    if (this.firstChunkTime === undefined) {
      this.firstChunkTime = performance.now();
    }

    const chunk = parseStreamChunk(rawChunk);

    // 1. Accumulate Text
    if (typeof chunk.content === 'string') {
      const lastItem = this.content[this.content.length - 1];
      if (lastItem && lastItem.type === 'text') {
        (lastItem as MCPTextContent).text += chunk.content;
      } else {
        this.content.push({ type: 'text', text: chunk.content });
      }

      this.currentStreamingText += chunk.content;

      if (this.tryReportReasoningBudgetExceeded()) {
        // Budget abort takes precedence over text-loop detection.
      } else if (!this.hasToolCallInStream && !this.repeatedTextIssueReported) {
        this.repeatedTextCheckCounter += 1;
        const textDetection =
          this.repeatedTextCheckCounter % REPEATED_TAIL_CHECK_INTERVAL === 0 &&
          this.currentStreamingText
            ? detectRepeatedTextLoop(this.currentStreamingText)
            : null;
        if (textDetection) {
          this.repeatedTextIssueReported = true;
          logger.warn('Detected repeated text pattern during streaming', {
            sessionId: this.sessionId,
            responseMessageId: this.responseMessageId,
            ...textDetection,
          });
          void reportLLMStreamingIssue({
            sessionId: this.sessionId,
            responseMessageId: this.responseMessageId,
            issueKind: 'REPEATED_TEXT_LOOP',
            observedTailChars: textDetection.observedTailChars,
            patternLength: textDetection.patternLength,
            repetitionCount: textDetection.repetitionCount,
          }).catch((error: unknown) => {
            logger.warn('Failed to report repeated text pattern', {
              sessionId: this.sessionId,
              responseMessageId: this.responseMessageId,
              error,
            });
          });
        }
      }
    } else if (chunk.content) {
      const rawContent = chunk.content as unknown as MCPContent | MCPContent[];
      if (Array.isArray(rawContent)) {
        this.content.push(...rawContent);
      } else {
        this.content.push(rawContent);
      }
    }

    // 2. Accumulate Thinking
    if (typeof chunk.thinking === 'string') {
      if (this.thinkingStartTime === undefined) {
        this.thinkingStartTime = performance.now();
      }
      const lastItem = this.content[this.content.length - 1];
      const appendedToExistingThinkingBlock =
        !!lastItem && lastItem.type === 'thinking';
      if (lastItem && lastItem.type === 'thinking') {
        (lastItem as MCPThinkingContent).thinking += chunk.thinking;
      } else {
        this.content.push({ type: 'thinking', thinking: chunk.thinking });
      }

      this.currentThinkingText = appendedToExistingThinkingBlock
        ? `${this.currentThinkingText ?? ''}${chunk.thinking}` || undefined
        : this.currentThinkingText
          ? `${this.currentThinkingText}\n${chunk.thinking}`
          : chunk.thinking;

      if (this.tryReportReasoningBudgetExceeded()) {
        // Budget abort takes precedence over thinking-loop detection.
      } else if (!this.repeatedThinkingIssueReported) {
        this.repeatedThinkingCheckCounter += 1;
        const thinkingConfig = this.getThinkingConfig();
        const detection =
          this.repeatedThinkingCheckCounter % REPEATED_TAIL_CHECK_INTERVAL ===
            0 && this.currentThinkingText
            ? detectRepeatedThinkingLoop(
                this.currentThinkingText,
                thinkingConfig,
              )
            : null;
        if (detection) {
          this.repeatedThinkingIssueReported = true;
          logger.warn('Detected repeated thinking pattern during streaming', {
            sessionId: this.sessionId,
            responseMessageId: this.responseMessageId,
            ...detection,
          });
          void reportLLMStreamingIssue({
            sessionId: this.sessionId,
            responseMessageId: this.responseMessageId,
            issueKind: 'REPEATED_THINKING_LOOP',
            observedTailChars: detection.observedTailChars,
            patternLength: detection.patternLength,
            repetitionCount: detection.repetitionCount,
          }).catch((error: unknown) => {
            logger.warn('Failed to report repeated thinking pattern', {
              sessionId: this.sessionId,
              responseMessageId: this.responseMessageId,
              error,
            });
          });
        }
      }
    }

    // 3. Accumulate Thinking Signature
    if (typeof chunk.thinkingSignature === 'string') {
      this.thinkingSignature = chunk.thinkingSignature;
    }

    // 4. Accumulate Tool Calls
    const toolCallStartChunks = chunk.tool_call_starts ?? [];
    const toolCallDeltaChunks = chunk.tool_calls ?? [];
    const toolCallChunks = [...toolCallStartChunks, ...toolCallDeltaChunks];
    const hasToolCallUpdate = toolCallChunks.length > 0;
    const previousToolCallCount =
      this.indexedToolCalls.size + this.directToolCalls.length;

    if (hasToolCallUpdate) {
      this.hasToolCallInStream = true;
      toolCallChunks.forEach((toolCallChunk) => {
        if (isParsedIndexedToolCallDelta(toolCallChunk)) {
          const { index } = toolCallChunk;

          if (this.activeToolCallIndices.has(index)) {
            const contentIndex = this.activeToolCallIndices.get(index)!;
            const targetBlock = this.content[
              contentIndex
            ] as MCPToolCallContent;
            const existingToolCall = this.indexedToolCalls.get(index) ?? {
              id: targetBlock.id,
              type: 'function' as const,
              function: {
                name: targetBlock.name,
                arguments: targetBlock.arguments,
              },
            };
            if (toolCallChunk.id && !targetBlock.id) {
              targetBlock.id = toolCallChunk.id;
            }
            if (toolCallChunk.function?.name) {
              if (!targetBlock.name) {
                targetBlock.name = toolCallChunk.function.name;
              } else if (
                targetBlock.name !== toolCallChunk.function.name &&
                !toolCallChunk.function.name.startsWith(targetBlock.name)
              ) {
                targetBlock.name = toolCallChunk.function.name;
              }
            }
            if (toolCallChunk.function?.arguments) {
              targetBlock.arguments += toolCallChunk.function.arguments;
            }

            this.indexedToolCalls.set(index, {
              id: targetBlock.id || existingToolCall.id,
              type: 'function',
              function: {
                name: targetBlock.name || existingToolCall.function.name,
                arguments:
                  targetBlock.arguments || existingToolCall.function.arguments,
              },
            });
          } else {
            const newBlock: MCPToolCallContent = {
              type: 'tool_call',
              id: toolCallChunk.id || '',
              name: toolCallChunk.function?.name || '',
              arguments: toolCallChunk.function?.arguments || '',
            };
            this.content.push(newBlock);
            this.activeToolCallIndices.set(index, this.content.length - 1);
            this.indexedToolCalls.set(index, {
              id: newBlock.id,
              type: 'function',
              function: {
                name: newBlock.name,
                arguments: newBlock.arguments,
              },
            });
          }
          return;
        }

        if (isParsedDirectToolCall(toolCallChunk)) {
          const directToolCall: ToolCall = {
            id: toolCallChunk.id,
            type: 'function',
            function: {
              name: toolCallChunk.function.name,
              arguments: toolCallChunk.function.arguments,
            },
          };
          this.content.push({
            type: 'tool_call',
            id: directToolCall.id,
            name: directToolCall.function.name,
            arguments: directToolCall.function.arguments,
          });
          this.directToolCalls.push(directToolCall);
        }
      });
    }

    const shouldFlushToolCallImmediately =
      hasToolCallUpdate &&
      this.indexedToolCalls.size + this.directToolCalls.length >
        previousToolCallCount;

    if (this.thinkingStartTime !== undefined) {
      this.currentThinkingTime =
        (performance.now() - this.thinkingStartTime) / 1000;
    }

    if (chunk.usage) {
      const incomingUsage = chunk.usage;
      if (this.finalUsage) {
        this.finalUsage = {
          promptTokens:
            incomingUsage.promptTokens || this.finalUsage.promptTokens,
          completionTokens:
            incomingUsage.completionTokens || this.finalUsage.completionTokens,
          totalTokens: incomingUsage.totalTokens || this.finalUsage.totalTokens,
          cachedPromptTokens:
            incomingUsage.cachedPromptTokens ??
            this.finalUsage.cachedPromptTokens,
          details: {
            ...this.finalUsage.details,
            ...incomingUsage.details,
          },
        };
      } else {
        this.finalUsage = {
          promptTokens: incomingUsage.promptTokens ?? 0,
          completionTokens: incomingUsage.completionTokens ?? 0,
          totalTokens: incomingUsage.totalTokens ?? 0,
          cachedPromptTokens: incomingUsage.cachedPromptTokens,
          details: incomingUsage.details,
        };
      }
      // Provider usage often arrives on the final chunk after content/thinking.
      this.tryReportReasoningBudgetExceeded();
    }

    if (this.finalUsage) {
      if (!this.finalUsage.details) this.finalUsage.details = {};
      const currentTime = performance.now();
      this.finalUsage.details.evalDuration =
        currentTime - (this.firstChunkTime || this.startTime);
    }

    return {
      hasToolCallUpdate,
      shouldFlushToolCallImmediately,
    };
  }

  public shouldThrottleUpdate(
    lastUpdateMs: number,
    nowMs: number,
    hasToolCallUpdate: boolean,
    shouldFlushToolCallImmediately: boolean,
  ): boolean {
    const throttleMs = hasToolCallUpdate
      ? STREAMING_TOOL_CALL_THROTTLE_MS
      : STREAMING_TEXT_THROTTLE_MS;
    return shouldFlushToolCallImmediately || nowMs - lastUpdateMs >= throttleMs;
  }
}
