import { getLogger } from '@/lib/logger';

const logger = getLogger('FailureTracker');

interface SessionFailure {
  count: number;
  timestamp: number;
}

// In-memory store: sessionId -> SessionFailure
const failureStore = new Map<string, SessionFailure>();

// Clean up old sessions periodically (e.g., every 1 hour)
const CLEANUP_INTERVAL = 60 * 60 * 1000;
const MAX_AGE = 24 * 60 * 60 * 1000; // 24 hours

setInterval(() => {
  const now = Date.now();
  for (const [id, session] of failureStore.entries()) {
    if (now - session.timestamp > MAX_AGE) {
      failureStore.delete(id);
    }
  }
}, CLEANUP_INTERVAL);

export const FailureTracker = {
  recordFailure: (sessionId: string): number => {
    const current = failureStore.get(sessionId) || {
      count: 0,
      timestamp: Date.now(),
    };
    const newCount = current.count + 1;

    failureStore.set(sessionId, {
      count: newCount,
      timestamp: Date.now(),
    });

    logger.debug('Failure recorded', { sessionId, count: newCount });
    return newCount;
  },

  deleteSession: (sessionId: string) => {
    if (failureStore.has(sessionId)) {
      failureStore.delete(sessionId);
      logger.debug('Failure tracker session removed', { sessionId });
    }
  },

  resetFailure: (sessionId: string) => {
    if (failureStore.has(sessionId)) {
      failureStore.delete(sessionId);
      logger.debug('Failure count reset', { sessionId });
    }
  },

  getFailureCount: (sessionId: string): number => {
    return failureStore.get(sessionId)?.count || 0;
  },
};
