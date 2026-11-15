import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';

import type {
  AgentEventPayload,
  LlmStreamDeltaPayload,
} from '@/context/agent-session/types';
import { providerSupportsReasoningBudgetCap } from '@/lib/ai-service/openai/reasoning-budget';
import { getLogger } from '@/lib/logger';
import type { Settings } from '@/lib/services/settings-service';
import type {
  CompletionCancelRequest,
  MaxOutputTokensSource,
} from '@/models/agent-ipc';
import type { Message } from '@/models/chat';
import { StreamAccumulator } from './execute-completion';
import { buildStreamingMessage } from './streaming-message-utils';

const logger = getLogger('useRustStreamListener');

type SetStreamingMessages = React.Dispatch<
  React.SetStateAction<Map<string, Partial<Message>>>
>;

/** The turn a session is streaming right now. */
interface ActiveTurn {
  responseMessageId: string;
  accumulator: StreamAccumulator;
  baseMessage: Partial<Message>;
  lastUpdateMs: number;
  /** A loop or budget issue was reported; the rest of this turn is ignored. */
  stopped: boolean;
}

const FALLBACK_MAX_OUTPUT_TOKENS = 8192;

function resolveMaxOutputTokens(
  requestMaxTokens: number | null | undefined,
  settings: Settings,
): { maxTokens: number; source: MaxOutputTokensSource } {
  if (requestMaxTokens) {
    return { maxTokens: requestMaxTokens, source: 'assistant' };
  }
  const settingsMaxTokens = settings.advanced?.defaultMaxOutputTokens;
  if (settingsMaxTokens) {
    return { maxTokens: settingsMaxTokens, source: 'settings' };
  }
  return { maxTokens: FALLBACK_MAX_OUTPUT_TOKENS, source: 'fallback' };
}

/**
 * Turns the backend's `llmStreamDelta` events into the live streaming message.
 *
 * A Rust-routed turn has no WebView request, so nothing else paints it. Each
 * delta goes through the same `StreamAccumulator` the WebView path uses, which
 * keeps the loop and reasoning-budget detection in one place.
 */
export function createRustStreamHandler(
  settingsRef: React.MutableRefObject<Settings>,
  setStreamingMessages: SetStreamingMessages,
) {
  const turns = new Map<string, ActiveTurn>();

  const startTurn = (payload: LlmStreamDeltaPayload): ActiveTurn => {
    const { maxTokens, source } = resolveMaxOutputTokens(
      payload.maxTokens,
      settingsRef.current,
    );
    const startTime = performance.now();
    const turn: ActiveTurn = {
      responseMessageId: payload.responseMessageId,
      accumulator: new StreamAccumulator(
        payload.sessionId,
        payload.responseMessageId,
        settingsRef,
        startTime,
        providerSupportsReasoningBudgetCap(payload.provider)
          ? {
              reasoningBudgetMaxTokens: maxTokens,
              reasoningBudgetMaxTokensSource: source,
            }
          : undefined,
      ),
      baseMessage: {
        id: payload.responseMessageId,
        sessionId: payload.sessionId,
        threadId: payload.sessionId,
        role: 'assistant',
        content: [],
        createdAt: new Date(),
      },
      lastUpdateMs: 0,
      stopped: false,
    };
    turns.set(payload.sessionId, turn);
    return turn;
  };

  const publish = (sessionId: string, turn: ActiveTurn) => {
    const { accumulator } = turn;
    const message = buildStreamingMessage(
      turn.baseMessage,
      accumulator.content,
      accumulator.thinkingSignature,
      accumulator.currentThinkingTime,
      accumulator.finalUsage,
      {
        toolCalls: accumulator.getStreamingToolCalls(),
        thinkingText: accumulator.currentThinkingText,
        streamingPhase: accumulator.currentPhase,
      },
    );
    setStreamingMessages((prev) => {
      const next = new Map(prev);
      next.set(sessionId, message);
      return next;
    });
  };

  /** Drop the live message of a finished turn, unless a newer turn replaced it. */
  const clearStreaming = (sessionId: string, responseMessageId: string) => {
    setStreamingMessages((prev) => {
      if (prev.get(sessionId)?.id !== responseMessageId) {
        return prev;
      }
      const next = new Map(prev);
      next.delete(sessionId);
      return next;
    });
  };

  return {
    handleDelta(payload: LlmStreamDeltaPayload) {
      const { sessionId, responseMessageId, delta } = payload;
      let turn = turns.get(sessionId);
      if (!turn || turn.responseMessageId !== responseMessageId) {
        turn = startTurn(payload);
      }
      if (turn.stopped) {
        return;
      }

      const {
        hasToolCallUpdate,
        shouldFlushToolCallImmediately,
        shouldAbortStream,
      } = turn.accumulator.processChunk(delta);
      if (shouldAbortStream) {
        // The report to the backend is already on its way; it cancels the turn.
        turn.stopped = true;
        return;
      }

      const nowMs = performance.now();
      const isLast = delta.finishReason !== undefined;
      if (
        isLast ||
        turn.accumulator.shouldThrottleUpdate(
          turn.lastUpdateMs,
          nowMs,
          hasToolCallUpdate,
          shouldFlushToolCallImmediately,
        )
      ) {
        turn.lastUpdateMs = nowMs;
        publish(sessionId, turn);
      }
      if (isLast) {
        turn.accumulator.finalizeOutputBudgetCheck();
      }
    },

    /**
     * The saved message replaces the live one. Without this the live message
     * would stay on screen next to its stored copy.
     */
    handleMessageAdded(sessionId: string, messageId: string) {
      const turn = turns.get(sessionId);
      if (turn && turn.responseMessageId === messageId) {
        turn.stopped = true;
        clearStreaming(sessionId, messageId);
      }
    },

    /** Drop a turn the backend cancelled so a late delta cannot revive it. */
    handleCancel(sessionId: string, responseMessageId: string) {
      const turn = turns.get(sessionId);
      if (turn && turn.responseMessageId === responseMessageId) {
        turn.stopped = true;
        clearStreaming(sessionId, responseMessageId);
      }
    },
  };
}

/**
 * Mounts the Rust stream handler on the `agent:event` and
 * `llm:completion-cancel` channels.
 */
export function useRustStreamListener({
  settingsRef,
  setStreamingMessages,
}: {
  settingsRef: React.MutableRefObject<Settings>;
  setStreamingMessages: SetStreamingMessages;
}) {
  useEffect(() => {
    const handler = createRustStreamHandler(settingsRef, setStreamingMessages);
    let isMounted = true;
    const unlisteners: Array<() => void> = [];

    const register = async () => {
      const unlistenDelta = await listen<AgentEventPayload>(
        'agent:event',
        (event) => {
          const payload = event.payload;
          if (payload.type === 'llmStreamDelta') {
            handler.handleDelta(payload);
          } else if (
            payload.type === 'messageAdded' &&
            payload.message.role === 'assistant'
          ) {
            handler.handleMessageAdded(payload.sessionId, payload.message.id);
          }
        },
      );
      const unlistenCancel = await listen<CompletionCancelRequest>(
        'llm:completion-cancel',
        (event) => {
          handler.handleCancel(
            event.payload.sessionId,
            event.payload.responseMessageId,
          );
        },
      );
      if (isMounted) {
        unlisteners.push(unlistenDelta, unlistenCancel);
      } else {
        unlistenDelta();
        unlistenCancel();
      }
    };

    register().catch((error: unknown) => {
      logger.error('Failed to register Rust stream listeners', error);
    });

    return () => {
      isMounted = false;
      unlisteners.forEach((unlisten) => unlisten());
    };
    // Refs and state setters are stable; listen once for the app lifetime.
  }, [settingsRef, setStreamingMessages]);
}
