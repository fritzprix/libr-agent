import { useEffect, useRef, useState, useCallback } from 'react';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import { useChatContext } from '@/context/ChatContext';
import type { PlanningServerProxy, PlanningState } from '@/models/planning';
import { getLogger } from '@/lib/logger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { RefreshCw } from 'lucide-react';

const logger = getLogger('ChatPlanningPanel');

export function ChatPlanningPanel() {
  const { server, loading, error } =
    useWebMCPServer<PlanningServerProxy>('planning');
  const { messages } = useChatContext();
  const [planningState, setPlanningState] = useState<PlanningState | null>(
    null,
  );
  const [isRefreshing, setIsRefreshing] = useState(false);
  const lastHandledMessageRef = useRef<{ id?: string }>({});

  // Component lifecycle logging
  useEffect(() => {
    logger.info('PLANNING_PANEL: Component mounted');
    return () => {
      logger.info('PLANNING_PANEL: Component unmounted');
    };
  }, []);

  // Server status logging
  useEffect(() => {
    logger.info('PLANNING_PANEL: Server status changed', {
      loading,
      hasServer: !!server,
      serverId: server?.name,
      hasError: !!error,
      errorMessage: error || null,
    });
  }, [server, loading, error]);

  const refreshState = useCallback(async () => {
    if (!server?.get_current_state) {
      logger.warn(
        'PLANNING_PANEL: No server or get_current_state method available',
      );
      return;
    }

    logger.info('PLANNING_PANEL: Starting state refresh', {
      serverId: server.name,
      isLoaded: server.isLoaded,
      toolCount: server.tools?.length || 0,
    });

    try {
      setIsRefreshing(true);
      const state = await server.get_current_state();
      setPlanningState(state);
      logger.info('PLANNING_PANEL: State refresh successful', {
        hasGoal: !!state.goal,
        todoCount: state.todos?.length || 0,
        observationCount: state.observations?.length || 0,
        goalText: state.goal ? `"${state.goal.substring(0, 50)}..."` : null,
      });
    } catch (err) {
      logger.error('PLANNING_PANEL: Failed to refresh planning state', {
        error: err instanceof Error ? err.message : String(err),
        serverId: server.name,
      });
    } finally {
      setIsRefreshing(false);
      logger.debug('PLANNING_PANEL: Refresh operation completed');
    }
  }, [server]);

  // Message-based state updates (instead of 30s polling)
  useEffect(() => {
    if (!server?.get_current_state || messages.length === 0) {
      logger.info('PLANNING_PANEL: Skipping message-based update', {
        hasServer: !!server,
        hasGetState: !!server?.get_current_state,
        messageCount: messages.length,
      });
      return;
    }

    const lastMessage = messages[messages.length - 1];
    logger.info('PLANNING_PANEL: Checking message for state update', {
      messageId: lastMessage.id,
      messageRole: lastMessage.role,
      lastHandledId: lastHandledMessageRef.current.id,
      messageCount: messages.length,
    });

    if (lastHandledMessageRef.current.id === lastMessage.id) {
      logger.info('PLANNING_PANEL: Message already handled, skipping update');
      return;
    }

    lastHandledMessageRef.current.id = lastMessage.id;
    logger.info(
      'PLANNING_PANEL: New message detected, triggering state update',
      {
        messageId: lastMessage.id,
        messageRole: lastMessage.role,
      },
    );

    // Update Planning state only when new messages arrive
    refreshState();
  }, [messages, refreshState]);

  if (loading) {
    return (
      <Card className="w-80 m-4">
        <CardContent className="p-6">
          <div className="animate-pulse text-muted-foreground">
            Loading planning state...
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error) {
    return (
      <Card className="w-80 m-4">
        <CardContent className="p-6">
          <div className="text-destructive text-sm mb-3">
            Error loading planning server
          </div>
          <Button onClick={refreshState} variant="outline" size="sm">
            Retry
          </Button>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card className="w-80 m-4">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg">AI Planning</CardTitle>
          <Button
            onClick={refreshState}
            disabled={isRefreshing}
            variant="outline"
            size="sm"
          >
            <RefreshCw
              className={`h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`}
            />
          </Button>
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
          <div className="space-y-2">
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
