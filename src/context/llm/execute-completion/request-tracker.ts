import { useCallback, useMemo, useRef } from 'react';
import type { AICompletionExecutionService } from '@/lib/ai-service/types';
import { getLogger } from '@/lib/logger';

const logger = getLogger('request-tracker');

export type RequestTerminationReason = 'aborted' | 'superseded';

export interface UseSessionRequestTrackerReturn {
  activeServicesRef: React.MutableRefObject<
    Map<string, AICompletionExecutionService>
  >;
  abortControllersRef: React.MutableRefObject<Map<string, AbortController>>;
  timeoutsRef: React.MutableRefObject<Map<string, number>>;
  lastStreamingUpdateRef: React.MutableRefObject<Map<string, number>>;
  activeRequestIdsRef: React.MutableRefObject<Map<string, string>>;
  terminatedRequestsRef: React.MutableRefObject<
    Map<string, RequestTerminationReason>
  >;
  getRequestKey: (sessionId: string, responseMessageId: string) => string;
  getRequestTerminationReason: (
    sessionId: string,
    responseMessageId: string,
  ) => RequestTerminationReason | undefined;
  isCurrentRequest: (sessionId: string, responseMessageId: string) => boolean;
  ensureRequestStillActive: (
    sessionId: string,
    responseMessageId: string,
    phase: string,
    abortSignal?: AbortSignal,
  ) => void;
  prepareForNewRequest: (
    sessionId: string,
    responseMessageId: string,
  ) => AbortController;
  cancelCompletionRequest: (
    sessionId: string,
    responseMessageId?: string,
    onCancelMessageCleanup?: (sessionId: string) => void,
  ) => void;
  cleanupRequestState: (
    sessionId: string,
    responseMessageId: string,
    abortController: AbortController,
    service: AICompletionExecutionService,
  ) => void;
}

export function useSessionRequestTracker(): UseSessionRequestTrackerReturn {
  const activeServicesRef = useRef<Map<string, AICompletionExecutionService>>(
    new Map(),
  );
  const abortControllersRef = useRef<Map<string, AbortController>>(new Map());
  const timeoutsRef = useRef<Map<string, number>>(new Map());
  const lastStreamingUpdateRef = useRef<Map<string, number>>(new Map());
  const activeRequestIdsRef = useRef<Map<string, string>>(new Map());
  const terminatedRequestsRef = useRef<Map<string, RequestTerminationReason>>(
    new Map(),
  );

  const getRequestKey = useCallback(
    (sessionId: string, responseMessageId: string) =>
      `${sessionId}:${responseMessageId}`,
    [],
  );

  const getRequestTerminationReason = useCallback(
    (sessionId: string, responseMessageId: string) =>
      terminatedRequestsRef.current.get(
        getRequestKey(sessionId, responseMessageId),
      ),
    [getRequestKey],
  );

  const isCurrentRequest = useCallback(
    (sessionId: string, responseMessageId: string) =>
      activeRequestIdsRef.current.get(sessionId) === responseMessageId,
    [],
  );

  const ensureRequestStillActive = useCallback(
    (
      sessionId: string,
      responseMessageId: string,
      phase: string,
      abortSignal?: AbortSignal,
    ) => {
      const terminationReason = getRequestTerminationReason(
        sessionId,
        responseMessageId,
      );
      const activeRequestId = activeRequestIdsRef.current.get(sessionId);

      if (terminationReason === 'superseded') {
        logger.info(`Dropping ${phase} for superseded request`, {
          sessionId,
          responseMessageId,
          activeRequestId,
        });
        throw new Error('Request superseded');
      }

      if (terminationReason === 'aborted') {
        logger.warn(`Completion request aborted during ${phase}`, {
          sessionId,
          responseMessageId,
        });
        throw new Error('Request aborted');
      }

      if (!isCurrentRequest(sessionId, responseMessageId)) {
        if (
          activeRequestId !== undefined &&
          activeRequestId !== responseMessageId
        ) {
          logger.info(`Dropping ${phase} for superseded request`, {
            sessionId,
            responseMessageId,
            activeRequestId,
          });
          throw new Error('Request superseded');
        }

        if (abortSignal?.aborted) {
          logger.warn(`Completion request aborted during ${phase}`, {
            sessionId,
            responseMessageId,
          });
          throw new Error('Request aborted');
        }

        logger.info(`Dropping ${phase} for inactive request`, {
          sessionId,
          responseMessageId,
          activeRequestId,
        });
        throw new Error('Request superseded');
      }

      if (abortSignal?.aborted) {
        logger.warn(`Completion request aborted during ${phase}`, {
          sessionId,
          responseMessageId,
        });
        throw new Error('Request aborted');
      }
    },
    [getRequestTerminationReason, isCurrentRequest],
  );

  const prepareForNewRequest = useCallback(
    (sessionId: string, responseMessageId: string): AbortController => {
      // Cancel any previously active request for this session
      terminatedRequestsRef.current.delete(
        getRequestKey(sessionId, responseMessageId),
      );
      const previousRequestId = activeRequestIdsRef.current.get(sessionId);
      const previousController = abortControllersRef.current.get(sessionId);
      if (previousController && previousRequestId) {
        terminatedRequestsRef.current.set(
          getRequestKey(sessionId, previousRequestId),
          'superseded',
        );
        previousController.abort();
      }
      terminatedRequestsRef.current.delete(
        getRequestKey(sessionId, responseMessageId),
      );
      activeServicesRef.current.delete(sessionId);
      const previousTimeoutId = timeoutsRef.current.get(sessionId);
      if (previousTimeoutId) {
        clearTimeout(previousTimeoutId);
        timeoutsRef.current.delete(sessionId);
      }
      activeRequestIdsRef.current.set(sessionId, responseMessageId);

      const abortController = new AbortController();
      abortControllersRef.current.set(sessionId, abortController);
      return abortController;
    },
    [getRequestKey],
  );

  const cancelCompletionRequest = useCallback(
    (
      sessionId: string,
      responseMessageId?: string,
      onCancelMessageCleanup?: (sessionId: string) => void,
    ) => {
      const activeResponseMessageId =
        activeRequestIdsRef.current.get(sessionId);
      if (
        responseMessageId !== undefined &&
        activeResponseMessageId !== responseMessageId
      ) {
        logger.info('Ignoring stale completion cancel request', {
          sessionId,
          responseMessageId,
          activeResponseMessageId,
        });
        return;
      }

      logger.info('Manually cancelling completion request', { sessionId });
      if (activeResponseMessageId) {
        terminatedRequestsRef.current.set(
          getRequestKey(sessionId, activeResponseMessageId),
          'aborted',
        );
      }
      activeRequestIdsRef.current.delete(sessionId);
      lastStreamingUpdateRef.current.delete(sessionId);

      if (onCancelMessageCleanup) {
        onCancelMessageCleanup(sessionId);
      }

      const timeoutId = timeoutsRef.current.get(sessionId);
      if (timeoutId) {
        clearTimeout(timeoutId);
        timeoutsRef.current.delete(sessionId);
      }
      const service = activeServicesRef.current.get(sessionId);
      const abortController = abortControllersRef.current.get(sessionId);
      if (abortController) {
        abortControllersRef.current.delete(sessionId);
        abortController.abort();
      }
      if (service) {
        activeServicesRef.current.delete(sessionId);
      }
    },
    [getRequestKey],
  );

  const cleanupRequestState = useCallback(
    (
      sessionId: string,
      responseMessageId: string,
      abortController: AbortController,
      service: AICompletionExecutionService,
    ) => {
      if (activeRequestIdsRef.current.get(sessionId) === responseMessageId) {
        activeRequestIdsRef.current.delete(sessionId);
      }
      if (abortControllersRef.current.get(sessionId) === abortController) {
        abortControllersRef.current.delete(sessionId);
      }
      if (activeServicesRef.current.get(sessionId) === service) {
        activeServicesRef.current.delete(sessionId);
      }
      terminatedRequestsRef.current.delete(
        getRequestKey(sessionId, responseMessageId),
      );
    },
    [getRequestKey],
  );

  return useMemo(
    () => ({
      activeServicesRef,
      abortControllersRef,
      timeoutsRef,
      lastStreamingUpdateRef,
      activeRequestIdsRef,
      terminatedRequestsRef,
      getRequestKey,
      getRequestTerminationReason,
      isCurrentRequest,
      ensureRequestStillActive,
      prepareForNewRequest,
      cancelCompletionRequest,
      cleanupRequestState,
    }),
    [
      getRequestKey,
      getRequestTerminationReason,
      isCurrentRequest,
      ensureRequestStillActive,
      prepareForNewRequest,
      cancelCompletionRequest,
      cleanupRequestState,
    ],
  );
}
