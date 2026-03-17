import React, { useState } from 'react';
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
import { PanelRight, FolderOpen, Copy, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import { useClipboard } from '@/hooks/useClipboard';

interface AgentChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function AgentChatHeader({
  children,
  assistantName,
}: AgentChatHeaderProps) {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { showPlanningPanel, togglePlanningPanel } = useAgentPlanning();
  const { showWorkspacePanel, toggleWorkspacePanel } = useAgentWorkspace();
  const { messages } = useAgentChat();
  const [isCopying, setIsCopying] = useState(false);
  const { copyToClipboard } = useClipboard();

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
    if (isCopying) return;
    setIsCopying(true);
    try {
      const json = JSON.stringify(messages, null, 2);
      await copyToClipboard(json);
      toast.success(t('agent.header.copySuccess'));
    } catch {
      toast.error(t('agent.header.copyError'));
    } finally {
      setIsCopying(false);
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
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleCopyMessages}
                disabled={isCopying}
                aria-label={t('agent.header.copyAria')}
                className="h-6 px-2"
              >
                {isCopying ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
            </TooltipTrigger>
            <TooltipContent>{t('agent.header.copyTooltip')}</TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleToggleWorkspace}
                aria-label={t('agent.header.toggleWorkspaceAria')}
                className="h-6 px-2"
              >
                <FolderOpen
                  className={`h-4 w-4 ${showWorkspacePanel ? 'text-primary' : ''}`}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.header.toggleWorkspaceTooltip')}
            </TooltipContent>
          </Tooltip>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleTogglePlanning}
                aria-label={t('agent.header.togglePlanningAria')}
                className="h-6 px-2"
              >
                <PanelRight
                  className={`h-4 w-4 ${showPlanningPanel ? 'text-primary' : ''}`}
                />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.header.togglePlanningTooltip')}
            </TooltipContent>
          </Tooltip>

          {session?.id && (
            <SessionFilesPopover key={session.id} sessionId={session.id} />
          )}
        </div>
      </div>
    </AgentTerminalHeader>
  );
}
