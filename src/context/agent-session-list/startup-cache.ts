import { safeInvoke } from '@/lib/backend/core';
import type {
  AgentSessionListResponse,
  AgentSessionMetadata,
} from '@/models/agent-ipc';

import {
  normalizeAttentionSessions,
  normalizeSessionListResponse,
} from './mappings';

export const SESSION_LIST_PAGE_SIZE = 100;

export interface InitialSessionListData {
  page: AgentSessionListResponse;
  attentionSessions: AgentSessionMetadata[];
}

export type SessionMetadataLoadSource = 'cache' | 'inflight' | 'network';

export interface SessionMetadataLoadHandle {
  source: SessionMetadataLoadSource;
  promise: Promise<InitialSessionListData>;
}

let cachedInitialSessionListData: InitialSessionListData | null = null;
let cachedInitialSessionListPromise: Promise<InitialSessionListData> | null =
  null;
let sessionListCacheGeneration = 0;

export function invalidateSessionListStartupCache() {
  sessionListCacheGeneration += 1;
  cachedInitialSessionListData = null;
  cachedInitialSessionListPromise = null;
}

export function loadInitialSessionList(
  forceRefresh = false,
): SessionMetadataLoadHandle {
  if (forceRefresh) {
    invalidateSessionListStartupCache();
  }

  if (!forceRefresh && cachedInitialSessionListData !== null) {
    return {
      source: 'cache',
      promise: Promise.resolve(cachedInitialSessionListData),
    };
  }

  if (!forceRefresh && cachedInitialSessionListPromise) {
    return {
      source: 'inflight',
      promise: cachedInitialSessionListPromise,
    };
  }

  const requestGeneration = sessionListCacheGeneration;
  const request = Promise.all([
    safeInvoke<AgentSessionListResponse>('agent_list_sessions', {
      request: { limit: SESSION_LIST_PAGE_SIZE },
    }),
    safeInvoke<AgentSessionMetadata[]>('agent_list_attention_sessions'),
  ])
    .then(([page, attentionSessions]) => {
      const initialData: InitialSessionListData = {
        page: normalizeSessionListResponse(page),
        attentionSessions: normalizeAttentionSessions(attentionSessions),
      };

      if (
        requestGeneration === sessionListCacheGeneration &&
        cachedInitialSessionListPromise === request
      ) {
        cachedInitialSessionListData = initialData;
      }

      return initialData;
    })
    .finally(() => {
      if (cachedInitialSessionListPromise === request) {
        cachedInitialSessionListPromise = null;
      }
    });

  cachedInitialSessionListPromise = request;
  return {
    source: 'network',
    promise: request,
  };
}

export function resetAgentSessionListStartupCacheForTests() {
  invalidateSessionListStartupCache();
}
