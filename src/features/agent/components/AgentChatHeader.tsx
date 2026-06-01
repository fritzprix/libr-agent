import React, { useState } from 'react';
import AgentSessionHeader from './AgentSessionHeader';
import {
  useAgentSessionActions,
  useAgentSessionState,
} from '@/context/AgentSessionContext';
import { useAgentSessionListActions } from '@/context/AgentSessionListContext';
import { useAgentPlanning } from '@/context/AgentPlanningContext';
import { useAgentWorkspace } from '@/context/AgentWorkspaceContext';
import { useAgentChatState } from '@/context/AgentChatContext';
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

function CopyMessagesButton() {
  const { t } = useTranslation();
  const { messages } = useAgentChatState();
  const [isCopying, setIsCopying] = useState(false);
  const { copyToClipboard } = useClipboard();

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
  );
}

export function AgentChatHeader({
  children,
  assistantName,
}: AgentChatHeaderProps) {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { renameSession } = useAgentSessionActions();
  const { toggleBookmark } = useAgentSessionListActions();
  const { showPlanningPanel, togglePlanningPanel } = useAgentPlanning();
  const { showWorkspacePanel, toggleWorkspacePanel } = useAgentWorkspace();

  return (
    <AgentSessionHeader
      onRenameSession={renameSession}
      isBookmarked={session?.isBookmarked}
      onToggleBookmark={() => session?.id && toggleBookmark(session.id)}
    >
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-primary">[{assistantName}]</span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <CopyMessagesButton />

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={toggleWorkspacePanel}
                aria-label={t('agent.header.toggleWorkspaceAria')}
                aria-controls="agent-workspace-panel"
                aria-expanded={showWorkspacePanel}
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
                onClick={togglePlanningPanel}
                aria-label={t('agent.header.togglePlanningAria')}
                aria-controls="agent-planning-panel"
                aria-expanded={showPlanningPanel}
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
    </AgentSessionHeader>
  );
}
