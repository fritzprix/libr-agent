import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Checkbox } from '@/components/ui';
import type { PlanningState, ScratchpadState } from '@/models/planning';

interface PlanningToastSummaryProps {
  goal: string | null;
  todos: PlanningState['todos'];
  previousTodos?: PlanningState['todos'];
  scratchpad: ScratchpadState | undefined;
  scratchpadChanged: boolean;
}

export function PlanningToastSummary({
  goal,
  todos,
  previousTodos,
  scratchpad,
  scratchpadChanged,
}: PlanningToastSummaryProps) {
  const { t } = useTranslation();

  const visibleTodos = useMemo(() => {
    if (todos.length <= 5) return todos;

    const prevIds = new Set((previousTodos || []).map((todo) => todo.id));
    const added = todos.filter((todo) => !prevIds.has(todo.id));

    const prevCheckedMap = new Map(
      (previousTodos || []).map((todo) => [todo.id, todo.checked]),
    );
    const changed = todos.filter(
      (todo) =>
        prevCheckedMap.has(todo.id) &&
        prevCheckedMap.get(todo.id) !== todo.checked,
    );

    let relevant = [...added, ...changed];
    relevant = Array.from(
      new Map(relevant.map((todo) => [todo.id, todo])).values(),
    );

    if (relevant.length === 0) {
      return todos.slice(-5);
    }

    const todoIndexMap = new Map(todos.map((todo, index) => [todo.id, index]));

    if (relevant.length >= 5) {
      return relevant
        .sort((a, b) => todoIndexMap.get(a.id)! - todoIndexMap.get(b.id)!)
        .slice(-5);
    }

    const result = [...relevant];
    const resultIds = new Set(result.map((todo) => todo.id));

    for (let index = todos.length - 1; index >= 0; index--) {
      const todo = todos[index];
      if (result.length >= 5) break;
      if (!resultIds.has(todo.id)) {
        result.push(todo);
      }
    }

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

  const completedTodos = useMemo(() => {
    return todos.reduce((count, todo) => (todo.checked ? count + 1 : count), 0);
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
                previousTodos &&
                !previousTodos.some((item) => item.id === todo.id);
              const isChanged =
                previousTodos &&
                previousTodos.some(
                  (item) =>
                    item.id === todo.id && item.checked !== todo.checked,
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
