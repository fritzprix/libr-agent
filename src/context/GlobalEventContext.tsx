import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from 'react';
import { listen } from '@tauri-apps/api/event';
import { getLogger } from '../lib/logger';

const logger = getLogger('GlobalEventContext');

export type ResourceType = 'assistant' | 'mcpServer' | 'playbook' | 'session';

export interface ResourceUpdatedPayload {
  type: 'resourceUpdated';
  resourceType: ResourceType;
  action: 'create' | 'update' | 'delete' | 'verify';
  resourceId?: string;
}

export type GlobalEventPayload =
  | ResourceUpdatedPayload
  | { type: string; [key: string]: unknown };

interface GlobalEventContextValue {
  subscribe: (resourceType: ResourceType, callback: () => void) => () => void;
}

const GlobalEventContext = createContext<GlobalEventContextValue | undefined>(
  undefined,
);

interface GlobalEventProviderProps {
  children: React.ReactNode;
}

/**
 * GlobalEventProvider
 *
 * Centralized listener for Tauri 'agent:event' emissions.
 * Manages event distribution and debouncing to prevent UI thrashing.
 */
export function GlobalEventProvider({ children }: GlobalEventProviderProps) {
  // Registry of listeners grouped by resource type
  const listeners = useRef<Map<ResourceType, Set<() => void>>>(new Map());
  const debounceTimers = useRef<Map<ResourceType, NodeJS.Timeout>>(new Map());

  // Subscribe to Tauri events on mount
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        unlisten = await listen<GlobalEventPayload>('agent:event', (event) => {
          const payload = event.payload;

          if (payload.type === 'resourceUpdated') {
            const { resourceType } = payload as ResourceUpdatedPayload;

            logger.debug(
              `Resource updated event received: ${resourceType}`,
              payload,
            );

            // Debounce the update trigger
            const existingTimer = debounceTimers.current.get(resourceType);
            if (existingTimer) {
              clearTimeout(existingTimer);
            }

            const timer = setTimeout(() => {
              const resourceListeners = listeners.current.get(resourceType);
              if (resourceListeners) {
                logger.debug(
                  `Triggering ${resourceListeners.size} listeners for ${resourceType}`,
                );
                resourceListeners.forEach((cb) => cb());
              }
              debounceTimers.current.delete(resourceType);
            }, 300);

            debounceTimers.current.set(resourceType, timer);
          }
        });
      } catch (err) {
        logger.error('Failed to setup global event listener', err);
      }
    };

    setupListener();

    return () => {
      if (unlisten) unlisten();
      // Clear all timers on unmount
      debounceTimers.current.forEach((timer) => clearTimeout(timer));
    };
  }, []);

  const subscribe = useCallback(
    (resourceType: ResourceType, callback: () => void) => {
      if (!listeners.current.has(resourceType)) {
        listeners.current.set(resourceType, new Set());
      }
      listeners.current.get(resourceType)!.add(callback);

      return () => {
        const resourceListeners = listeners.current.get(resourceType);
        if (resourceListeners) {
          resourceListeners.delete(callback);
          if (resourceListeners.size === 0) {
            listeners.current.delete(resourceType);
          }
        }
      };
    },
    [],
  );

  const value = useMemo(() => ({ subscribe }), [subscribe]);

  return (
    <GlobalEventContext.Provider value={value}>
      {children}
    </GlobalEventContext.Provider>
  );
}

/**
 * Hook to subscribe to backend resource updates
 */
export function useBackendResource(
  resourceType: ResourceType,
  onUpdate: () => void,
) {
  const context = useContext(GlobalEventContext);
  if (!context) {
    throw new Error(
      'useBackendResource must be used within GlobalEventProvider',
    );
  }

  const onUpdateRef = useRef(onUpdate);
  onUpdateRef.current = onUpdate;

  useEffect(() => {
    const callback = () => onUpdateRef.current();
    return context.subscribe(resourceType, callback);
  }, [resourceType, context]);
}
