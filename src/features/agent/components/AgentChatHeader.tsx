import React from 'react';
import AgentTerminalHeader from './AgentTerminalHeader';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentPlanning } from '@/context/AgentPlanningContext';
import { useAgentWorkspace } from '@/context/AgentWorkspaceContext';
import { useAgentChat } from '@/context/AgentChatContext';
import { SessionFilesPopover } from '@/components/shared/SessionFilesPopover';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { PanelRight, FolderOpen, Copy } from 'lucide-react';
import { toast } from 'sonner';

interface AgentChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function AgentChatHeader({
  children,
  assistantName,
}: AgentChatHeaderProps) {
  const { session } = useAgentSessionState();
  const { showPlanningPanel, togglePlanningPanel } = useAgentPlanning();
  const { showWorkspacePanel, toggleWorkspacePanel } = useAgentWorkspace();
  const { messages } = useAgentChat();

  // Planning toggle comes from AgentPlanningContext to keep state in sync
  const handleTogglePlanning = () => {
    if (!showPlanningPanel) {
      // About to open planning; ensure workspace is closed
      if (showWorkspacePanel) toggleWorkspacePanel();
    }
    togglePlanningPanel();
  };

  const handleToggleWorkspace = () => {
    if (!showWorkspacePanel) {
      // About to open workspace; ensure planning is closed
      if (showPlanningPanel) togglePlanningPanel();
    }
    toggleWorkspacePanel();
  };

  const handleCopyMessages = async () => {
    try {
      const json = JSON.stringify(messages, null, 2);
      await navigator.clipboard.writeText(json);
      toast.success('대화 내용이 클립보드에 복사되었습니다');
    } catch {
      toast.error('클립보드 복사에 실패했습니다');
    }
  };

  return (
    <AgentTerminalHeader>
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-primary">[{assistantName}]</span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleCopyMessages}
                aria-label="Copy conversation as JSON"
                className="h-6 px-2"
              >
                <Copy className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Copy conversation as JSON</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleToggleWorkspace}
                aria-label="Toggle Workspace Files Panel"
                className="h-6 px-2"
              >
                <FolderOpen
                  className={`h-4 w-4 ${showWorkspacePanel ? 'text-primary' : ''}`}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Toggle Workspace Files Panel</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                onClick={handleTogglePlanning}
                aria-label="Toggle AI Planning Panel"
                className="h-6 px-2"
              >
                <PanelRight
                  className={`h-4 w-4 ${showPlanningPanel ? 'text-primary' : ''}`}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Toggle AI Planning Panel</TooltipContent>
          </Tooltip>

          {session?.id && <SessionFilesPopover sessionId={session.id} />}
        </div>
      </div>
    </AgentTerminalHeader>
  );
}
