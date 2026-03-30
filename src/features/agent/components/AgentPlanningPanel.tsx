import { useEffect, useMemo } from 'react';
import { Circle, PanelRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui';
import { getLogger } from '@/lib/logger';
import { parsePlanningState, parseScratchpadState } from '@/models/planning';

const logger = getLogger('AgentPlanningPanel');

export function AgentPlanningPanel() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { serviceContexts, updateServiceContexts } = useAgentChat();

  // Component lifecycle logging
  useEffect(() => {
    logger.info('AGENT_PLANNING_PANEL: Component mounted');
    // Ensure we have the latest planning state when the panel opens
    updateServiceContexts();
    return () => {
      logger.info('AGENT_PLANNING_PANEL: Component unmounted');
    };
  }, [updateServiceContexts]);

  const planningState = useMemo(
    () => parsePlanningState(serviceContexts.planning?.structuredState),
    [serviceContexts.planning?.structuredState],
  );
  const scratchpadState = useMemo(
    () => parseScratchpadState(serviceContexts.scratchpad?.structuredState),
    [serviceContexts.scratchpad?.structuredState],
  );
  const completedTodos =
    planningState?.todos.filter((todo) => todo.checked).length ?? 0;
  const totalTodos = planningState?.todos.length ?? 0;
  const scratchpadCount = scratchpadState?.items.length ?? 0;
  const progressPercent =
    totalTodos > 0 ? Math.round((completedTodos / totalTodos) * 100) : 0;

  if (!session) return null;

  return (
    <Card className="h-full w-80 flex-shrink-0 rounded-none border-y-0 border-r-0 border-l border-border/40 bg-background py-0 shadow-none gap-0">
      <CardHeader className="border-b border-border/40 px-4 py-3">
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
            <PanelRight className="h-3.5 w-3.5" />
            <span>{t('agent.planning.title')}</span>
          </div>

          <div className="flex flex-wrap gap-2">
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {completedTodos}/{totalTodos} {t('agent.planning.tasks')}
            </div>
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {scratchpadCount} {t('agent.planning.scratchpad')}
            </div>
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-[11px] text-muted-foreground">
              <span>{t('agent.planning.tasks')}</span>
              <span>{progressPercent}%</span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-muted/60">
              <div
                className="h-full rounded-full bg-primary/80 transition-[width] duration-300"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-5 px-4 py-4">
        {/* Goal Section */}
        <section className="space-y-2">
          <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
            {t('agent.planning.currentGoal')}
          </h4>
          <div className="border-l border-border/60 pl-3 text-sm leading-relaxed text-foreground/90">
            {planningState?.goal || t('agent.planning.noGoal')}
          </div>
        </section>

        {/* Todos Section */}
        <section className="min-h-0 space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              {t('agent.planning.tasks')}
            </h4>
            <span className="text-[11px] text-muted-foreground">
              {totalTodos === 0 ? '0' : `${completedTodos}/${totalTodos}`}
            </span>
          </div>
          <ScrollArea className="max-h-56">
            <div className="overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]">
              {planningState?.todos && planningState.todos.length > 0 ? (
                planningState.todos.map((todo, index) => (
                  <div
                    key={todo.id}
                    className="border-b border-border/25 px-3 py-3 last:border-b-0"
                  >
                    <div className="flex items-start gap-3 text-sm">
                      <div
                        className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border text-[10px] ${
                          todo.checked
                            ? 'border-primary/40 bg-primary/10 text-primary'
                            : 'border-border/60 text-muted-foreground'
                        }`}
                      >
                        {todo.checked ? '✓' : '○'}
                      </div>
                      <span className="mt-0.5 inline-flex w-7 shrink-0 items-center justify-center text-[10px] font-mono text-muted-foreground/80">
                        #{todo.id ?? index}
                      </span>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-1.5">
                          <span
                            className={
                              todo.checked
                                ? 'text-muted-foreground line-through'
                                : 'font-medium text-foreground'
                            }
                            title={todo.description}
                          >
                            {todo.title}
                          </span>
                          {todo.priority && (
                            <Badge
                              variant="outline"
                              className="h-4 gap-1 border-border/40 bg-background/80 px-1 py-0 text-[10px] text-muted-foreground"
                            >
                              <Circle
                                className={`w-2 h-2 fill-current ${
                                  todo.priority === 'high'
                                    ? 'text-destructive'
                                    : todo.priority === 'medium'
                                      ? 'text-warning'
                                      : 'text-success'
                                }`}
                              />
                              {todo.priority === 'high'
                                ? t('agent.planning.priorityHigh')
                                : todo.priority === 'medium'
                                  ? t('agent.planning.priorityMedium')
                                  : t('agent.planning.priorityLow')}
                            </Badge>
                          )}
                        </div>
                        {todo.description && (
                          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                            {todo.description}
                          </p>
                        )}
                      </div>
                    </div>
                  </div>
                ))
              ) : (
                <div className="px-3 py-4 text-sm text-muted-foreground">
                  {t('agent.planning.noTasks')}
                </div>
              )}
            </div>
          </ScrollArea>
        </section>

        <section className="flex min-h-0 flex-1 flex-col space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              {t('agent.planning.scratchpad')}
            </h4>
            <span className="text-[11px] text-muted-foreground">
              {scratchpadCount}
            </span>
          </div>
          <ScrollArea className="min-h-0 flex-1">
            {scratchpadState?.items && scratchpadState.items.length > 0 ? (
              <div className="overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]">
                {scratchpadState.items.map((item) => (
                  <div
                    key={item.id}
                    className="border-b border-border/25 px-3 py-3 last:border-b-0"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="truncate text-sm font-medium text-foreground">
                        {item.title || t('agent.planning.untitledNote')}
                      </div>
                      <Badge
                        variant="outline"
                        className="shrink-0 border-border/40 bg-background/80 text-[10px] text-muted-foreground"
                      >
                        #{item.id}
                      </Badge>
                    </div>
                    <p className="mt-1.5 whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
                      {item.content}
                    </p>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded-lg border border-border/40 bg-muted/[0.18] px-3 py-4 text-sm text-muted-foreground">
                {t('agent.planning.noScratchpad')}
              </div>
            )}
          </ScrollArea>
        </section>
      </CardContent>
    </Card>
  );
}
