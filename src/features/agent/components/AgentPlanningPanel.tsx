import { useEffect, useCallback } from 'react';
import { useAgentChat, ServiceContext } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { getLogger } from '@/lib/logger';
import type {
  ScratchpadItem,
  PlanningState,
} from '@/lib/web-mcp/modules/planning-server/types';

const logger = getLogger('AgentPlanningPanel');

export function AgentPlanningPanel() {
  const { session } = useAgentSessionState();
  const { serviceContexts, updateServiceContexts } = useAgentChat();

  // Component lifecycle logging
  useEffect(() => {
    logger.info('AGENT_PLANNING_PANEL: Component mounted');
    return () => {
      logger.info('AGENT_PLANNING_PANEL: Component unmounted');
    };
  }, []);

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
          <CardTitle className="text-lg">AI Planning</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Goal Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            Current Goal
          </h4>
          <div className="text-sm p-3 bg-muted rounded-md">
            {planningState?.goal || 'No active goal'}
          </div>
        </div>

        {/* Todos Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            Tasks
          </h4>
          <div className="max-h-48 overflow-y-auto space-y-2">
            {planningState?.todos && planningState.todos.length > 0 ? (
              planningState.todos.map((todo, index) => (
                <div key={index} className="space-y-1">
                  {/* Parent Todo */}
                  <div className="flex items-start gap-2 text-sm">
                    <Badge
                      variant={todo.checked ? 'default' : 'secondary'}
                      className="mt-0.5 shrink-0"
                    >
                      {todo.checked ? '✓' : '○'}
                    </Badge>
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
                            className="text-xs px-1 py-0 h-4"
                          >
                            {todo.priority === 'high'
                              ? '🔴'
                              : todo.priority === 'medium'
                                ? '🟡'
                                : '🟢'}
                          </Badge>
                        )}
                      </div>
                      {/* Subtasks Progress */}
                      {todo.subtasks && todo.subtasks.length > 0 && (
                        <div className="text-xs text-muted-foreground mt-0.5">
                          {todo.subtasks.filter((st) => st.checked).length}/
                          {todo.subtasks.length} subtasks
                        </div>
                      )}
                    </div>
                  </div>

                  {/* Subtasks */}
                  {todo.subtasks && todo.subtasks.length > 0 && (
                    <div className="ml-6 space-y-1 border-l-2 border-muted pl-2">
                      {todo.subtasks.map((subtask) => (
                        <div
                          key={subtask.id}
                          className="flex items-start gap-2 text-xs"
                        >
                          <Badge
                            variant={subtask.checked ? 'default' : 'outline'}
                            className="mt-0.5 shrink-0 h-4 px-1"
                          >
                            {subtask.checked ? '✓' : '○'}
                          </Badge>
                          <span
                            className={
                              subtask.checked
                                ? 'line-through text-muted-foreground'
                                : ''
                            }
                            title={subtask.description}
                          >
                            {subtask.title}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">No tasks</div>
            )}
          </div>
        </div>

        {/* Notes Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            Scratchpad
          </h4>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {planningState?.scratchpad &&
            planningState.scratchpad.length > 0 ? (
              planningState.scratchpad.map((m: ScratchpadItem) => (
                <div
                  key={m.id}
                  className="text-xs p-2 bg-accent/50 rounded-sm border-l-2 border-accent"
                >
                  {m.content}
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">No notes</div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
