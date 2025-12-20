import { useEffect } from 'react';
import { useMessageTrigger } from '@/hooks/use-message-trigger';
import { getLogger } from '@/lib/logger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { useServiceContext } from '@/features/tools/useServiceContext';
import {
  ScratchpadItem,
  PlanningState,
} from '@/lib/web-mcp/modules/planning-server';

const logger = getLogger('ChatPlanningPanel');

export function ChatPlanningPanel() {
  const planningState = useServiceContext<PlanningState>('planning');

  // Component lifecycle logging
  useEffect(() => {
    logger.info('PLANNING_PANEL: Component mounted');
    return () => {
      logger.info('PLANNING_PANEL: Component unmounted');
    };
  }, []);

  // Message-based state updates using custom hook
  useMessageTrigger(() => {
    // State is now automatically updated via useServiceContext
    logger.debug('PLANNING_PANEL: State updated via service context');
  });

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
            {planningState?.todos.length ? (
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
            {planningState?.scratchpad.length ? (
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
