import { useEffect } from 'react';
import { useMessageTrigger } from '@/hooks/use-message-trigger';
import { getLogger } from '@/lib/logger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { useServiceContext } from '@/features/tools/useServiceContext';
import { PlanningState } from '@/lib/web-mcp/modules/planning-server';

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
                <div key={index} className="flex items-start gap-2 text-sm">
                  <Badge
                    variant={
                      todo.status === 'completed' ? 'default' : 'secondary'
                    }
                    className="mt-0.5"
                  >
                    {todo.status === 'completed' ? '✓' : '○'}
                  </Badge>
                  <span
                    className={
                      todo.status === 'completed'
                        ? 'line-through text-muted-foreground'
                        : ''
                    }
                  >
                    {todo.name}
                  </span>
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">No tasks</div>
            )}
          </div>
        </div>

        {/* Observations Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            Recent Observations
          </h4>
          <div className="max-h-32 overflow-y-auto space-y-1">
            {planningState?.observations.length ? (
              planningState.observations.map((obs, index) => (
                <div
                  key={index}
                  className="text-xs p-2 bg-accent/50 rounded-sm border-l-2 border-accent"
                >
                  {obs}
                </div>
              ))
            ) : (
              <div className="text-sm text-muted-foreground">
                No observations
              </div>
            )}
          </div>
        </div>
      </CardContent>
    </Card>
  );
}
