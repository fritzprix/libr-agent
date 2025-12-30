import React from 'react';
import TerminalHeader from '@/components/TerminalHeader';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentPlanning } from '@/context/AgentPlanningContext';
import { useAgentWorkspace } from '@/context/AgentWorkspaceContext';
import { useAgentChat } from '@/context/AgentChatContext';
import { SessionFilesPopover } from '@/features/chat/components/SessionFilesPopover';
import { Button } from '@/components/ui/button';
import { PanelRight, FolderOpen, Brain, Copy } from 'lucide-react';
import { toast } from 'sonner';

interface AgentChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function AgentChatHeader({
  children,
  assistantName,
}: AgentChatHeaderProps) {
  const { currentSession } = useAgentSessionState();
  const { showPlanningPanel, togglePlanningPanel } = useAgentPlanning();
  const { showWorkspacePanel, toggleWorkspacePanel } = useAgentWorkspace();
  const { reasoningEnabled, canUseReasoning, messages, toggleReasoning } =
    useAgentChat();

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
    <TerminalHeader>
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-blue-400">
              [{assistantName}]
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {canUseReasoning && (
            <Button
              variant="ghost"
              size="sm"
              onClick={toggleReasoning}
              title={`Reasoning Mode: ${reasoningEnabled ? 'ON (Deep reasoning enabled - higher cost)' : 'OFF (Standard mode)'}`}
              className="h-6 px-2"
            >
              <Brain
                className={`h-4 w-4 ${reasoningEnabled ? 'text-purple-400' : ''}`}
              />
            </Button>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={handleCopyMessages}
            title="Copy conversation as JSON"
            className="h-6 px-2"
          >
            <Copy className="h-4 w-4" />
          </Button>

          <Button
            variant="ghost"
            size="sm"
            onClick={handleToggleWorkspace}
            title="Toggle Workspace Files Panel"
            className="h-6 px-2"
          >
            <FolderOpen
              className={`h-4 w-4 ${showWorkspacePanel ? 'text-blue-400' : ''}`}
            />
          </Button>

          <Button
            variant="ghost"
            size="sm"
            onClick={handleTogglePlanning}
            title="Toggle AI Planning Panel"
            className="h-6 px-2"
          >
            <PanelRight
              className={`h-4 w-4 ${showPlanningPanel ? 'text-blue-400' : ''}`}
            />
          </Button>

          {currentSession?.id && (
            <SessionFilesPopover sessionId={currentSession.id} />
          )}
        </div>
      </div>
    </TerminalHeader>
  );
}
