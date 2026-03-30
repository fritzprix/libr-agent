import { useEffect, useMemo, useRef } from 'react';
import equal from 'fast-deep-equal';
import { listen } from '@tauri-apps/api/event';
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
import type { AgentEventPayload } from '@/context/agent-session/types';

const logger = getLogger('AgentPlanningUpdates');

const PLANNING_TOAST_ID_PREFIX = 'agent-planning-update';

interface PlanningToastSummaryProps {
  goal: string | null;
  todos: PlanningState['todos'];
  scratchpad: ScratchpadState | undefined;
  scratchpadChanged: boolean;
  scratchpadLabel: string;
  currentGoalLabel: string;
  tasksLabel: string;
  noGoalLabel: string;
  noTasksLabel: string;
  noScratchpadLabel: string;
  scratchpadUpdatedLabel: string;
}

function PlanningToastSummary({
  goal,
  todos,
  scratchpad,
  scratchpadChanged,
  scratchpadLabel,
  currentGoalLabel,
  tasksLabel,
  noGoalLabel,
  noTasksLabel,
  noScratchpadLabel,
  scratchpadUpdatedLabel,
}: PlanningToastSummaryProps) {
  const visibleTodos = todos.slice(0, 5);
  const hiddenTodoCount = Math.max(todos.length - visibleTodos.length, 0);
  const completedTodos = todos.filter((todo) => todo.checked).length;
  const progressPercent =
    todos.length > 0 ? Math.round((completedTodos / todos.length) * 100) : 0;

  return (
    <div className="space-y-3">
      <div className="space-y-1.5">
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>
            {completedTodos}/{todos.length} {tasksLabel}
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
          {currentGoalLabel}
        </div>
        <div className="text-sm leading-relaxed">{goal ?? noGoalLabel}</div>
      </div>

      <div className="space-y-2">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {tasksLabel}
        </div>
        {visibleTodos.length > 0 ? (
          <div className="space-y-2">
            {visibleTodos.map((todo) => (
              <div key={todo.id} className="flex items-start gap-2 text-sm">
                <Checkbox checked={todo.checked} disabled className="mt-0.5" />
                <span
                  className={
                    todo.checked ? 'text-muted-foreground line-through' : ''
                  }
                >
                  {todo.title}
                </span>
              </div>
            ))}
            {hiddenTodoCount > 0 && (
              <div className="text-xs text-muted-foreground">
                +{hiddenTodoCount}
              </div>
            )}
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">{noTasksLabel}</div>
        )}
      </div>

      <div className="space-y-1">
        <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {scratchpadLabel}
        </div>
        <div className="text-sm text-muted-foreground">
          {scratchpad?.count ? scratchpad.count : noScratchpadLabel}
          {scratchpadChanged ? ` • ${scratchpadUpdatedLabel}` : ''}
        </div>
      </div>
    </div>
  );
}

function isPlanningRelatedTool(toolName: string): boolean {
  return (
    toolName.startsWith('planning__') || toolName.startsWith('scratchpad__')
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
  const refreshTimeoutRef = useRef<ReturnType<typeof setTimeout>>();

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
      return;
    }

    let unlisten: (() => void) | undefined;
    let isMounted = true;

    const initListener = async () => {
      unlisten = await listen<AgentEventPayload>('agent:event', (event) => {
        if (!isMounted || event.payload.type !== 'toolExecutionCompleted') {
          return;
        }

        if (
          event.payload.sessionId !== session.id ||
          !event.payload.success ||
          !isPlanningRelatedTool(event.payload.toolName)
        ) {
          return;
        }

        if (refreshTimeoutRef.current) {
          clearTimeout(refreshTimeoutRef.current);
        }

        const toolName = event.payload.toolName;

        refreshTimeoutRef.current = setTimeout(() => {
          updateServiceContexts().catch((error: unknown) => {
            logger.error(
              'Failed to refresh planning contexts after tool update',
              {
                sessionId: session.id,
                toolName,
                error,
              },
            );
          });
        }, 200);
      });
    };

    initListener().catch((error: unknown) => {
      logger.error('Failed to initialize planning update listener', error);
    });

    return () => {
      isMounted = false;
      if (refreshTimeoutRef.current) {
        clearTimeout(refreshTimeoutRef.current);
      }
      unlisten?.();
    };
  }, [session?.id, updateServiceContexts]);

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
            scratchpad={scratchpadState}
            scratchpadChanged={scratchpadChanged}
            scratchpadLabel={t('agent.planning.scratchpad')}
            currentGoalLabel={t('agent.planning.currentGoal')}
            tasksLabel={t('agent.planning.tasks')}
            noGoalLabel={t('agent.planning.noGoal')}
            noTasksLabel={t('agent.planning.noTasks')}
            noScratchpadLabel={t('agent.planning.noScratchpad')}
            scratchpadUpdatedLabel={t('agent.planning.updated')}
          />
        ),
      });
    }

    previousPlanningRef.current = planningState;
    previousScratchpadRef.current = scratchpadState;
  }, [planningState, scratchpadState, session?.id, showPlanningPanel, t]);

  return null;
}
