import { useEffect, useCallback } from 'react';
import { Circle } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAgentChat, ServiceContext } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { getLogger } from '@/lib/logger';
import type { PlanningState } from '@/models/planning';

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

  // Auto-update service contexts on every message arrival (Chat V1 pattern)
  // Debounce to ensure backend state is fully committed before fetching
  const handleMessageTrigger = useCallback(() => {
    logger.debug('AGENT_PLANNING_PANEL: Message detected, updating contexts');
    updateServiceContexts();
  }, [updateServiceContexts]);

  useAgentMessageTrigger(handleMessageTrigger, { debounceMs: 500 });

  // Subscribe to planning context (auto-updated on message submission)
  const planningContext = serviceContexts['planning'] as
    | ServiceContext
    | undefined;
  const planningState = planningContext?.structuredState as
    | PlanningState
    | undefined;

  if (!session) return null;

  return (
    <Card className="w-80 h-full flex flex-col bg-background/95 backdrop-blur border-border/50">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg">{t('agent.planning.title')}</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Goal Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            {t('agent.planning.currentGoal')}
          </h4>
          <div className="text-sm p-3 bg-muted rounded-md">
            {planningState?.goal || t('agent.planning.noGoal')}
          </div>
        </div>

        {/* Todos Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            {t('agent.planning.tasks')}
          </h4>
          <div className="max-h-48 overflow-y-auto space-y-2">
            {planningState?.todos && planningState.todos.length > 0 ? (
              planningState.todos.map((todo, index) => (
                <div key={todo.id} className="space-y-1">
                  <div className="flex items-start gap-2 text-sm">
                    <Badge
                      variant={todo.checked ? 'default' : 'secondary'}
                      className="mt-0.5 shrink-0"
                    >
                      {todo.checked ? '✓' : '○'}
                    </Badge>
                    {/* Index Badge for AI interaction */}
                    <span className="mt-1 inline-flex items-center justify-center text-xs font-mono text-muted-foreground w-6 shrink-0">
                      {index}
                    </span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-1.5">
                        <span
                          className={
                            todo.checked
                              ? 'line-through text-muted-foreground'
                              : 'font-medium'
                          }
                          title={todo.description}
                        >
                          {todo.title}
                        </span>
                        {todo.priority && (
                          <Badge
                            variant="outline"
                            className="text-xs px-1 py-0 h-4 flex items-center gap-1"
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
                    </div>
                  </div>
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">{t('agent.planning.noTasks')}</div>
            )}
          </div>
        </div>

        {/* Notes section moved to Memory panel */}
      </CardContent>
    </Card>
  );
}
