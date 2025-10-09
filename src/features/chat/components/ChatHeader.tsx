import React from 'react';
import TerminalHeader from '@/components/TerminalHeader';
import { useSessionContext } from '@/context/SessionContext';
import { useChatPlanning } from '../context/ChatPlanningContext';
import { useChatWorkspace } from '../context/ChatWorkspaceContext';
import { useChatState, useChatActions } from '@/context/ChatContext';
import { SessionFilesPopover } from './SessionFilesPopover';
import { Button } from '@/components/ui/button';
import { PanelRight, FolderOpen, Bot } from 'lucide-react';

interface ChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function ChatHeader({ children, assistantName }: ChatHeaderProps) {
  const { current: currentSession } = useSessionContext();
  const { showPlanningPanel, togglePlanningPanel } = useChatPlanning();
  const { showWorkspacePanel, toggleWorkspacePanel } = useChatWorkspace();
  const { agenticMode } = useChatState();
  const { setAgenticMode } = useChatActions();

  // Planning toggle comes from ChatPlanningContext to keep state in sync
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

  // Workspace toggle comes from context to ensure correct provider instance

  const handleToggleAgenticMode = () => {
    setAgenticMode(!agenticMode);
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
          <Button
            variant="ghost"
            size="sm"
            onClick={handleToggleAgenticMode}
            title={`Agent Mode: ${agenticMode ? 'ON (AI will always use tools)' : 'OFF (AI decides when to use tools)'}`}
            className="h-6 px-2"
          >
            <Bot className={`h-4 w-4 ${agenticMode ? 'text-green-400' : ''}`} />
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
