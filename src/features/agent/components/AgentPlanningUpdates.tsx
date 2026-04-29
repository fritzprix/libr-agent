import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { Message } from '@/models/chat';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import equal from 'fast-deep-equal';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import { Checkbox } from '@/components/ui';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentPlanning } from '@/context/AgentPlanningContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import {
  parsePlanningState,
  parseScratchpadState,
  type PlanningState,
  type ScratchpadState,
} from '@/models/planning';

const logger = getLogger('AgentPlanningUpdates');

const PLANNING_TOAST_ID_PREFIX = 'agent-planning-update';

interface PlanningToastSummaryProps {
  goal: string | null;
  todos: PlanningState['todos'];
  previousTodos?: PlanningState['todos'];
  scratchpad: ScratchpadState | undefined;
  scratchpadChanged: boolean;
}

function PlanningToastSummary({
  goal,
  todos,
  previousTodos,
  scratchpad,
  scratchpadChanged,
}: PlanningToastSummaryProps) {
  const { t } = useTranslation();

  const visibleTodos = useMemo(() => {
    if (todos.length <= 5) return todos;

    // Identify added and changed todos
    const prevIds = new Set((previousTodos || []).map((t) => t.id));
    const added = todos.filter((t) => !prevIds.has(t.id));

    const prevCheckedMap = new Map(
      (previousTodos || []).map((t) => [t.id, t.checked]),
    );
    const changed = todos.filter(
      (t) => prevCheckedMap.has(t.id) && prevCheckedMap.get(t.id) !== t.checked,
    );

    // Prioritize added and changed items
    let relevant = [...added, ...changed];
    // Remove duplicates if any
    relevant = Array.from(new Map(relevant.map((t) => [t.id, t])).values());

    if (relevant.length === 0) {
      // If no specific changes, show the last 5 (likely the most recent)
      return todos.slice(-5);
    }

    // Use a precomputed O(1) index lookup in the sort callback instead of
    // repeated O(N) position scans. This keeps the sort at O(N log N) and
    // reduces main-thread JavaScript work during streaming updates.
    const todoIndexMap = new Map(todos.map((t, i) => [t.id, i]));

    if (relevant.length >= 5) {
      // If many things changed, show the last 5 of those
      return relevant
        .sort((a, b) => todoIndexMap.get(a.id)! - todoIndexMap.get(b.id)!)
        .slice(-5);
    }

    // Fill up to 5 items, prioritizing relevant ones, then filling from the end of the list
    const result = [...relevant];
    const resultIds = new Set(result.map((t) => t.id));

    for (let i = todos.length - 1; i >= 0; i--) {
      if (result.length >= 5) break;
      if (!resultIds.has(todos[i].id)) {
        result.push(todos[i]);
      }
    }

    // Sort by original order to maintain context
    return result.sort(
      (a, b) => todoIndexMap.get(a.id)! - todoIndexMap.get(b.id)!,
    );
  }, [todos, previousTodos]);

  const firstVisibleIndex =
    visibleTodos.length > 0 ? todos.indexOf(visibleTodos[0]) : -1;
  const lastVisibleIndex =
    visibleTodos.length > 0
      ? todos.indexOf(visibleTodos[visibleTodos.length - 1])
      : -1;

  const hiddenAbove = firstVisibleIndex > 0 ? firstVisibleIndex : 0;
  const hiddenBelow =
    lastVisibleIndex >= 0
      ? Math.max(todos.length - (lastVisibleIndex + 1), 0)
      : 0;

  // Performance optimization: Memoize the reduction of completed todos
  // to avoid O(N) recalculation on every render cycle
  const completedTodos = useMemo(() => {
    return todos.reduce(
      (acc, todo) => (todo.checked ? acc + 1 : acc),
      0,
    );
  }, [todos]);

  const progressPercent =
    todos.length > 0 ? Math.round((completedTodos / todos.length) * 100) : 0;

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>
            {completedTodos}/{todos.length} {t('agent.planning.tasks')}
          </span>
          <span>{progressPercent}%</span>
        </div>
        <div className="h-1.5 overflow-hidden rounded-full bg-muted/60">
          <div
            className="h-full rounded-full bg-primary/80 transition-[width] duration-300"
            style={{ width: `${progressPercent}%` }}
          />
        </div>
      </div>

      <div className="space-y-1">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('agent.planning.currentGoal')}
        </div>
        <div className="text-sm leading-relaxed">
          {goal ?? t('agent.planning.noGoal')}
        </div>
      </div>

      <div className="space-y-2">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('agent.planning.tasks')}
        </div>
        {visibleTodos.length > 0 ? (
          <div className="space-y-2">
            {hiddenAbove > 0 && (
              <div className="text-[10px] text-muted-foreground/60 italic">
                +{hiddenAbove} ...
              </div>
            )}
            {visibleTodos.map((todo) => {
              const isNew =
                previousTodos && !previousTodos.some((t) => t.id === todo.id);
              const isChanged =
                previousTodos &&
                previousTodos.some(
                  (t) => t.id === todo.id && t.checked !== todo.checked,
                );

              return (
                <div key={todo.id} className="flex items-start gap-2 text-sm">
                  <Checkbox
                    checked={todo.checked}
                    disabled
                    className="mt-0.5"
                  />
                  <span
                    className={
                      todo.checked ? 'text-muted-foreground line-through' : ''
                    }
                  >
                    {todo.title}
                    {(isNew || isChanged) && (
                      <span
                        className="ml-1.5 inline-flex h-1.5 w-1.5 rounded-full bg-primary"
                        title="Updated"
                      />
                    )}
                  </span>
                </div>
              );
            })}
            {hiddenBelow > 0 && (
              <div className="text-xs text-muted-foreground">
                +{hiddenBelow}
              </div>
            )}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">
            {t('agent.planning.noTasks')}
          </div>
        )}
      </div>

      <div className="space-y-1">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {t('agent.planning.scratchpad')}
        </div>
        <div className="text-sm text-muted-foreground">
          {scratchpad?.count
            ? scratchpad.count
            : t('agent.planning.noScratchpad')}
          {scratchpadChanged ? ` • ${t('agent.planning.updated')}` : ''}
        </div>
      </div>
    </div>
  );
}

export function AgentPlanningUpdates() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { showPlanningPanel } = useAgentPlanning();
  const { serviceContexts, updateServiceContexts } = useAgentChat();
  const previousPlanningRef = useRef<PlanningState | undefined>(undefined);
  const previousScratchpadRef = useRef<ScratchpadState | undefined>(undefined);
  const hasHydratedRef = useRef(false);

  const planningState = useMemo(
    () => parsePlanningState(serviceContexts.planning?.structuredState),
    [serviceContexts.planning?.structuredState],
  );
  const scratchpadState = useMemo(
    () => parseScratchpadState(serviceContexts.scratchpad?.structuredState),
    [serviceContexts.scratchpad?.structuredState],
  );

  useEffect(() => {
    if (!session?.id) {
      hasHydratedRef.current = false;
      previousPlanningRef.current = undefined;
      previousScratchpadRef.current = undefined;
    }
  }, [session?.id]);

  const triggerCallback = useCallback(() => {
    updateServiceContexts().catch((error: unknown) => {
      logger.error('Failed to refresh planning contexts after tool update', {
        sessionId: session?.id,
        error,
      });
    });
  }, [session?.id, updateServiceContexts]);

  const triggerOptions = useMemo(
    () => ({
      debounceMs: 200,
      messageFilter: (message: Message) => {
        if (!session?.id || message.sessionId !== session.id) return false;

        // Tool result messages are emitted with role === 'tool' and tool_call_id.
        // Refresh contexts after any successful tool completion so planning and
        // scratchpad state stay in sync even when the backend does not attach
        // tool_use metadata to tool result messages.
        if (
          message.role === 'tool' &&
          typeof message.tool_call_id === 'string' &&
          message.tool_call_id.length > 0 &&
          !message.error &&
          message.metadata?.toolError !== true
        ) {
          return true;
        }

        return false;
      },
    }),
    [session?.id],
  );

  useAgentMessageTrigger(triggerCallback, triggerOptions);

  useEffect(() => {
    if (!session?.id) {
      return;
    }

    if (!hasHydratedRef.current) {
      previousPlanningRef.current = planningState;
      previousScratchpadRef.current = scratchpadState;
      hasHydratedRef.current = true;
      return;
    }

    const planningChanged = !equal(previousPlanningRef.current, planningState);
    const scratchpadChanged = !equal(
      previousScratchpadRef.current,
      scratchpadState,
    );

    if ((planningChanged || scratchpadChanged) && !showPlanningPanel) {
      toast(t('agent.planning.title'), {
        id: `${PLANNING_TOAST_ID_PREFIX}-${session.id}`,
        duration: 5000,
        description: (
          <PlanningToastSummary
            goal={planningState?.goal ?? null}
            todos={planningState?.todos ?? []}
            previousTodos={previousPlanningRef.current?.todos}
            scratchpad={scratchpadState}
            scratchpadChanged={scratchpadChanged}
          />
        ),
      });
    }

    previousPlanningRef.current = planningState;
    previousScratchpadRef.current = scratchpadState;
  }, [planningState, scratchpadState, session?.id, showPlanningPanel, t]);

  return null;
}
