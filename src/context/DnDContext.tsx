import { getLogger } from '@/lib/logger';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import type { ReactNode, RefObject } from 'react';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
} from 'react';

interface DnDContextProps {
  children: ReactNode;
}

// Public event types for subscribers
export type DragAndDropEvent = 'drag-over' | 'drop' | 'leave';

// Payload passed to subscribers
export interface DragAndDropPayload {
  position?: { x: number; y: number };
  paths?: string[]; // Tauri file paths when available
  // Future: add web Files/Text if needed
}

// Internal Tauri event payload shape (best-effort typing)
interface TauriDragDropPayload {
  type: 'enter' | 'over' | 'leave' | 'drop';
  position?: { x: number; y: number };
  paths?: string[];
}

interface DnDRegistry {
  id: string;
  ref: RefObject<HTMLElement>;
  handler: (event: DragAndDropEvent, payload: DragAndDropPayload) => void;
  priority: number; // higher wins when overlapping
}

type DnDUnlisten = () => void;
export interface DnDContextReturnType {
  subscribe: (
    ref: RefObject<HTMLElement>,
    handler: (event: DragAndDropEvent, payload: DragAndDropPayload) => void,
    options?: { priority?: number },
  ) => DnDUnlisten;
}

const DnDContext = createContext<DnDContextReturnType | null>(null);

const logger = getLogger('DnDContext');

function DnDContextProvider({ children }: DnDContextProps) {
  const unlistenRef = useRef<(() => void) | undefined>(undefined);
  const registries = useRef<Map<string, DnDRegistry>>(new Map());
  const currentTarget = useRef<DnDRegistry | null>(null);
  const pathsRef = useRef<string[] | undefined>(undefined);

  // Helper: find matching registry by position with priority and smallest area tie-breaker
  const findTarget = (x: number, y: number): DnDRegistry | null => {
    let best: DnDRegistry | null = null;
    let bestArea = Infinity;
    let bestPriority = -Infinity;
    registries.current.forEach((reg) => {
      const el = reg.ref.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      if (
        x >= rect.left &&
        x <= rect.right &&
        y >= rect.top &&
        y <= rect.bottom
      ) {
        const area = rect.width * rect.height;
        if (
          reg.priority > bestPriority ||
          (reg.priority === bestPriority && area < bestArea)
        ) {
          best = reg;
          bestArea = area;
          bestPriority = reg.priority;
        }
      }
    });
    return best;
  };

  useEffect(() => {
    let mounted = true;
    const attach = async () => {
      try {
        const webview = getCurrentWebview();
        const unlisten = await webview.onDragDropEvent((evt) => {
          const payload = (evt.payload ?? {}) as TauriDragDropPayload;
          const { type, position, paths } = payload;

          // Track paths from enter event
          if (type === 'enter' && paths) {
            pathsRef.current = paths;
          }

          // Special-case: 'leave' may not include a position when exiting the window.
          if (type === 'leave') {
            // Clear current target and send leave to all zones
            if (currentTarget.current) {
              currentTarget.current.handler('leave', {
                position,
                paths: pathsRef.current,
              });
              currentTarget.current = null;
            }
            if (position) {
              const tgt = findTarget(position.x, position.y);
              if (tgt && tgt !== currentTarget.current) {
                tgt.handler('leave', { position, paths: pathsRef.current });
              }
            } else {
              // Broadcast leave to all zones so UIs can clear hover state
              registries.current.forEach((reg) =>
                reg.handler('leave', { paths: pathsRef.current }),
              );
            }
            // Clear paths when leaving
            pathsRef.current = undefined;
            return;
          }

          // For other events, we need a position to route
          if (!position) return;

          const target = findTarget(position.x, position.y);

          const data: DragAndDropPayload = {
            position,
            paths: pathsRef.current,
          };
          if (type === 'enter' || type === 'over') {
            // If target changed, send 'leave' to previous target
            if (currentTarget.current && currentTarget.current !== target) {
              currentTarget.current.handler('leave', data);
            }

            // Update current target and send 'drag-over' if we have a target
            if (target) {
              currentTarget.current = target;
              target.handler('drag-over', data);
            } else {
              currentTarget.current = null;
            }
          } else if (type === 'drop') {
            if (target) {
              target.handler('drop', data);
            }
            // Clear current target after drop
            currentTarget.current = null;
            // Clear paths after drop
            pathsRef.current = undefined;
          }
        });
        if (mounted) unlistenRef.current = unlisten;
      } catch (e) {
        logger.error('Failed to attach Tauri drag & drop listener', e);
      }
    };
    attach();

    return () => {
      mounted = false;
      if (unlistenRef.current) {
        try {
          unlistenRef.current();
        } catch (e) {
          logger.warn('Error during DnD listener cleanup', e);
        }
      }
      unlistenRef.current = undefined;
    };
  }, []);

  const subscribe = useCallback(
    (
      ref: RefObject<HTMLElement>,
      handler: (event: DragAndDropEvent, payload: DragAndDropPayload) => void,
      options?: { priority?: number },
    ): DnDUnlisten => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      const priority = options?.priority ?? 0;
      registries.current.set(id, { id, ref, handler, priority });

      return () => {
        const registry = registries.current.get(id);
        if (registry && currentTarget.current === registry) {
          // If the zone being unregistered is the current target, clear it
          currentTarget.current = null;
        }
        registries.current.delete(id);
      };
    },
    [],
  );

  const value = useMemo<DnDContextReturnType>(
    () => ({ subscribe }),
    [subscribe],
  );

  return <DnDContext.Provider value={value}>{children}</DnDContext.Provider>;
}

export function useDnDContext(): DnDContextReturnType {
  const ctx = useContext(DnDContext);
  if (!ctx) {
    throw new Error('useDnDContext must be used within DnDContextProvider');
  }
  return ctx;
}

export { DnDContext, DnDContextProvider };
