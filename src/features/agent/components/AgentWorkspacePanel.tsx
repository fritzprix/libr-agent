import { useEffect, useCallback } from 'react';
import { useAgentChat, ServiceContext } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Folder, FileText } from 'lucide-react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentWorkspacePanel');

interface WorkspaceState {
  root_path?: string;
  selected_files_count?: number;
  file_tree_count?: number;
  // Add more fields as backend provides them
  selected_files?: string[];
  recent_files?: string[];
}

export function AgentWorkspacePanel() {
  const { currentSession } = useAgentSessionState();
  const { serviceContexts, updateServiceContexts } = useAgentChat();

  // Component lifecycle logging
  useEffect(() => {
    logger.info('AGENT_WORKSPACE_PANEL: Component mounted');
    return () => {
      logger.info('AGENT_WORKSPACE_PANEL: Component unmounted');
    };
  }, []);

  // Auto-update service contexts on every message arrival (Chat V1 pattern)
  // Debounce to ensure backend state is fully committed before fetching
  const handleMessageTrigger = useCallback(() => {
    logger.debug('AGENT_WORKSPACE_PANEL: Message detected, updating contexts');
    updateServiceContexts();
  }, [updateServiceContexts]);

  useAgentMessageTrigger(handleMessageTrigger, { debounceMs: 500 });

  // Subscribe to workspace context (auto-updated on message submission)
  const workspaceContext = serviceContexts['workspace'] as
    | ServiceContext
    | undefined;
  const workspaceState = workspaceContext?.structuredState as
    | WorkspaceState
    | undefined;

  if (!currentSession) return null;

  return (
    <Card className="w-80 h-full flex flex-col bg-background/95 backdrop-blur border-border/50">
      <CardHeader className="pb-3">
        <div className="flex items-center justify-between">
          <CardTitle className="text-lg">Workspace</CardTitle>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Root Path Section */}
        <div>
          <h4 className="font-medium text-sm text-muted-foreground mb-2">
            Root Path
          </h4>
          <div className="text-sm p-3 bg-muted rounded-md font-mono break-all">
            {workspaceState?.root_path || (
              <span className="text-muted-foreground italic not-italic">
                No root path set
              </span>
            )}
          </div>
        </div>

        {/* Stats Section */}
        <div className="grid grid-cols-2 gap-2">
          <div className="p-3 bg-muted rounded-md">
            <div className="flex items-center gap-2 mb-1">
              <FileText className="w-3.5 h-3.5 text-muted-foreground" />
              <div className="text-xs font-medium text-muted-foreground">
                Selected
              </div>
            </div>
            <div className="text-2xl font-semibold">
              {workspaceState?.selected_files_count ?? 0}
            </div>
          </div>
          <div className="p-3 bg-muted rounded-md">
            <div className="flex items-center gap-2 mb-1">
              <Folder className="w-3.5 h-3.5 text-muted-foreground" />
              <div className="text-xs font-medium text-muted-foreground">
                Total
              </div>
            </div>
            <div className="text-2xl font-semibold">
              {workspaceState?.file_tree_count ?? 0}
            </div>
          </div>
        </div>

        {/* Selected Files List (if available) */}
        {workspaceState?.selected_files &&
          workspaceState.selected_files.length > 0 && (
            <div>
              <h4 className="font-medium text-sm text-muted-foreground mb-2">
                Selected Files
              </h4>
              <div className="max-h-48 overflow-y-auto space-y-1">
                {workspaceState.selected_files.map((file, index) => (
                  <div
                    key={index}
                    className="text-xs p-2 bg-accent/50 rounded-sm font-mono truncate"
                    title={file}
                  >
                    {file.split('/').pop() || file}
                  </div>
                ))}
              </div>
            </div>
          )}

        {/* Recent Files (if available) */}
        {workspaceState?.recent_files &&
          workspaceState.recent_files.length > 0 && (
            <div>
              <h4 className="font-medium text-sm text-muted-foreground mb-2">
                Recent Files
              </h4>
              <div className="max-h-32 overflow-y-auto space-y-1">
                {workspaceState.recent_files.map((file, index) => (
                  <div
                    key={index}
                    className="text-xs p-2 bg-muted rounded-sm font-mono truncate"
                    title={file}
                  >
                    {file.split('/').pop() || file}
                  </div>
                ))}
              </div>
            </div>
          )}
      </CardContent>
    </Card>
  );
}
