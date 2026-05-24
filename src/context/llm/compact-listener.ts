import {
  handleCompactError,
  handleCompactResponse,
} from '@/lib/backend/agent-commands';
import { AIServiceFactory, AIServiceProvider } from '@/lib/ai-service';
import type {
  AIContextCompactionService,
  AIServiceConfig,
} from '@/lib/ai-service/types';
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

interface CompactRequestListenerOptions {
  settingsRef: MutableRefObject<Settings>;
  clearCompactionPressureForSession: (sessionId: string) => void;
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

export async function setupCompactRequestListener({
  settingsRef,
  clearCompactionPressureForSession,
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
        fromId,
        toId,
        parentRequest,
      } = event.payload;
      const messages = rawMessages.map(normalizeRustMessage);
      logger.info(
        `📦 Compact request received: session=${sessionId}, fromId=${fromId}, toId=${toId}`,
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

      const provider = (parentRequest?.provider ??
        settings.preferredModel.provider) as AIServiceProvider;
      const apiKey = settings.serviceConfigs?.[provider]?.apiKey ?? '';
      const model = parentRequest?.model ?? settings.preferredModel.model;
      const providerConfig: AIServiceConfig =
        settings.serviceConfigs?.[provider] ?? {};

      try {
        const runtimeConfig = buildServiceRuntimeConfig(
          settings,
          providerConfig,
        );
        const service: AIContextCompactionService = AIServiceFactory.getService(
          provider,
          apiKey,
          providerConfig,
        );
        const summary = await service.compact(messages, {
          modelName: model,
          systemPrompt: parentRequest?.systemPrompt,
          availableTools: parentRequest?.availableTools,
          config: runtimeConfig,
        });
        await handleCompactResponse(sessionId, fromId, toId, summary);
        setCompactedRangeForSession(sessionId, {
          fromId,
          toId,
          summary,
        });
        clearCompactionPressureForSession(sessionId);
        logger.info(`✅ Compact summary stored: session=${sessionId}`);
      } catch (error) {
        const compactRuntimeError = toAgentRuntimeError(error);
        logger.error(
          `Compact LLM call failed: session=${sessionId}`,
          compactRuntimeError,
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
