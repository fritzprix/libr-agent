import {
  handleCompactError,
  handleCompactResponse,
  getAgentCompactContext,
} from '@/lib/backend/agent-commands';
import {
  AIServiceFactory,
  resolveProviderRuntimeConfig,
} from '@/lib/ai-service';
import { summarizeCompactionRequestSizes } from '@/lib/ai-service/request-ingredients';
import type { AIContextCompactionService } from '@/lib/ai-service/types';
import { normalizeRustMessage } from '@/lib/ai-service/utils';
import { getLogger } from '@/lib/logger';
import type { Settings } from '@/context/SettingsContext';
import { listen } from '@tauri-apps/api/event';
import type { MutableRefObject } from 'react';
import { toast } from 'sonner';
import { buildServiceRuntimeConfig } from './service-runtime-config';
import { toAgentRuntimeError } from './listener-utils';
import type { CompactRequest, CompactedRange } from './types';

const logger = getLogger('compact-listener');
const compactionRetrySessions = new Set<string>();
const COMPACTION_RETRY_NO_TOOL_NOTE =
  'RETRY INSTRUCTION: The previous compaction response contained tool-call markup or execution syntax. Output plain markdown summary text only. Do not call tools, emit tool-call markup, or include pseudo-tool XML/JSON.';

interface CompactRequestListenerOptions {
  settingsRef: MutableRefObject<Settings>;
  setCompactedRangeForSession: (
    sessionId: string,
    range: CompactedRange | undefined,
  ) => void;
  isMounted: () => boolean;
  onRegistered: () => void;
}

interface CompactStateEventPayload {
  sessionId: string;
  sessionName?: string;
  compacting: boolean;
  awaitingCompact: boolean;
  phase: 'STARTED' | 'SUCCEEDED' | 'FAILED';
  error?: string;
}

interface CompactStateListenerOptions {
  setCompactingFromEvent: (sessionId: string, value: boolean) => void;
  setAwaitingCompactForSession: (sessionId: string, value: boolean) => void;
  isMounted: () => boolean;
  onRegistered: () => void;
}

function buildCompactionRetrySystemPrompt(
  systemPrompt: string | undefined,
  retryWithoutTools: boolean,
): string | undefined {
  if (!retryWithoutTools) {
    return systemPrompt;
  }

  return systemPrompt
    ? `${systemPrompt}\n\n${COMPACTION_RETRY_NO_TOOL_NOTE}`
    : COMPACTION_RETRY_NO_TOOL_NOTE;
}

export async function setupCompactRequestListener({
  settingsRef,
  setCompactedRangeForSession,
  isMounted,
  onRegistered,
}: CompactRequestListenerOptions): Promise<(() => void) | undefined> {
  const unlisten = await listen<CompactRequest>(
    'llm:compact-request',
    async (event) => {
      const {
        sessionId,
        messages: rawMessages,
        toId,
        compactedDeltaCount,
        parentRequest,
      } = event.payload;
      const messages = rawMessages.map(normalizeRustMessage);
      const retryWithoutTools = compactionRetrySessions.has(sessionId);
      logger.info(
        `📦 Compact request received: session=${sessionId}, toId=${toId}, compactedDeltaCount=${compactedDeltaCount}`,
      );

      const settings = settingsRef.current;
      if (!settings) {
        logger.error('No settings available for compact request');
        await handleCompactError(
          sessionId,
          toAgentRuntimeError(
            new Error('No settings available for compact request'),
          ),
        );
        return;
      }

      const provider =
        parentRequest?.provider ?? settings.preferredModel.provider;
      const resolved = resolveProviderRuntimeConfig(provider, settings);
      const apiKey = resolved.apiKey ?? '';
      const model = parentRequest?.model ?? settings.preferredModel.model;
      const systemPrompt = buildCompactionRetrySystemPrompt(
        parentRequest?.systemPrompt,
        retryWithoutTools,
      );
      const availableTools = retryWithoutTools
        ? undefined
        : parentRequest?.availableTools;
      const runtimeConfig = buildServiceRuntimeConfig(
        settings,
        resolved.serviceConfig,
      );
      const requestComposition = summarizeCompactionRequestSizes({
        messages,
        systemPrompt,
        availableTools,
      });

      try {
        const service: AIContextCompactionService = AIServiceFactory.getService(
          provider,
          apiKey,
          runtimeConfig,
        );
        logger.info(
          `🧪 Compact provider handoff ingredients: session=${sessionId}, provider=${provider}, model=${model}`,
          {
            ...requestComposition,
            retryWithoutTools,
            reasoningEnabled: runtimeConfig.enableReasoning ?? false,
            maxTokens: runtimeConfig.maxTokens,
          },
        );
        const summary = await service.compact(messages, {
          modelName: model,
          systemPrompt,
          availableTools,
          config: runtimeConfig,
        });
        const response = await handleCompactResponse(
          sessionId,
          toId,
          compactedDeltaCount,
          summary,
        );
        if (response.data?.retried) {
          compactionRetrySessions.add(sessionId);
          logger.info(
            `🔁 Compact summary rejected by backend; retry requested: session=${sessionId}`,
          );
          return;
        }
        compactionRetrySessions.delete(sessionId);
        try {
          const freshContext = await getAgentCompactContext(sessionId);
          if (freshContext) {
            setCompactedRangeForSession(sessionId, freshContext);
          } else {
            setCompactedRangeForSession(sessionId, {
              toId,
              condensedCount: compactedDeltaCount,
              summary,
            });
          }
        } catch (fetchError) {
          logger.warn(
            `Failed to hydrate compact context after live compaction: session=${sessionId}`,
            fetchError,
          );
          setCompactedRangeForSession(sessionId, {
            toId,
            condensedCount: compactedDeltaCount,
            summary,
          });
        }
        logger.info(`✅ Compact summary stored: session=${sessionId}`);
      } catch (error) {
        compactionRetrySessions.delete(sessionId);
        const compactRuntimeError = toAgentRuntimeError(error);
        logger.error(
          `Compact LLM call failed: session=${sessionId}`,
          compactRuntimeError,
        );
        logger.error(
          `🧪 Compact failure request composition: session=${sessionId}, provider=${provider}, model=${model}`,
          {
            ...requestComposition,
            reasoningEnabled: runtimeConfig.enableReasoning ?? false,
            maxTokens: runtimeConfig.maxTokens,
          },
        );
        await handleCompactError(sessionId, compactRuntimeError);
      }
    },
  );

  if (!isMounted()) {
    unlisten();
    return undefined;
  }

  onRegistered();
  return () => {
    unlisten();
    logger.info('LLM compact request listener cleaned up');
  };
}

export async function setupCompactStateListener({
  setCompactingFromEvent,
  setAwaitingCompactForSession,
  isMounted,
  onRegistered,
}: CompactStateListenerOptions): Promise<(() => void) | undefined> {
  const unlisten = await listen<CompactStateEventPayload>(
    'llm:compact-state',
    (event) => {
      const {
        sessionId,
        sessionName,
        compacting,
        awaitingCompact,
        phase,
        error,
      } = event.payload;
      const toastId = `compact-${sessionId}`;
      const description = sessionName ?? sessionId.slice(0, 8);

      setCompactingFromEvent(sessionId, compacting);
      setAwaitingCompactForSession(sessionId, awaitingCompact);

      if (phase === 'STARTED') {
        toast.loading(`Compacting context…`, {
          id: toastId,
          description,
          duration: Infinity,
        });
      } else if (phase === 'SUCCEEDED') {
        toast.success(`Context compacted`, {
          id: toastId,
          description,
          duration: 3000,
        });
      } else if (phase === 'FAILED') {
        toast.error(`Compaction failed`, {
          id: toastId,
          description: error ? `${description} - ${error}` : description,
          duration: 4000,
        });
      }
    },
  );

  if (!isMounted()) {
    unlisten();
    return undefined;
  }

  onRegistered();
  return () => {
    unlisten();
    logger.info('LLM compact state listener cleaned up');
  };
}
