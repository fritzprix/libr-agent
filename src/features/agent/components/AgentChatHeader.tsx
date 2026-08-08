import React, { useEffect, useMemo, useState } from 'react';
import AgentSessionHeader from './AgentSessionHeader';
import {
  useAgentSessionActions,
  useAgentSessionState,
} from '@/context/AgentSessionContext';
import {
  useAgentSessionListActions,
  useAgentSessionListState,
} from '@/context/AgentSessionListContext';
import { AGENT_PANEL_IDS, useAgentPanels } from '@/context/AgentPanelsContext';
import { useAgentChat } from '@/context/AgentChatContext';
import { SessionFilesPopover } from '@/components/shared/SessionFilesPopover';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Copy, Loader2, PanelRight } from 'lucide-react';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import { useClipboard } from '@/hooks/useClipboard';
import { messagesToMarkdown } from '@/lib/message-utils';
import { HeaderStatusBadges } from './HeaderStatusBadges';
import { PanelAttentionDot } from './PanelAttentionDot';

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
  const { renameSession } = useAgentSessionActions();
  const { toggleBookmark } = useAgentSessionListActions();
  const { sessions, notificationSessions } = useAgentSessionListState();
  const { isShellOpen, toggleShell, hasPanelAttention } = useAgentPanels();
  const shellOpen = isShellOpen();
  const shellAttention = AGENT_PANEL_IDS.some((id) => hasPanelAttention(id));
  const { messages } = useAgentChat();
  const [isCopying, setIsCopying] = useState(false);
  const [bookmarkOverride, setBookmarkOverride] = useState<
    boolean | undefined
  >();
  const { copyToClipboard } = useClipboard();
  const activeSessionMetadata = useMemo(() => {
    if (!session?.id) {
      return undefined;
    }

    return (
      sessions.find((candidate) => candidate.id === session.id) ??
      notificationSessions.find((candidate) => candidate.id === session.id)
    );
  }, [notificationSessions, session?.id, sessions]);
  const isBookmarked =
    bookmarkOverride ??
    activeSessionMetadata?.isBookmarked ??
    session?.isBookmarked;

  useEffect(() => {
    setBookmarkOverride(undefined);
  }, [activeSessionMetadata?.isBookmarked, session?.id, session?.isBookmarked]);

  const handleCopyMessages = async () => {
    if (isCopying) return;
    setIsCopying(true);
    try {
      const { content, truncated } = messagesToMarkdown(messages);
      await copyToClipboard(content);
      toast.success(
        truncated
          ? t('agent.header.copySuccessPartial')
          : t('agent.header.copySuccess'),
      );
    } catch (error) {
      if (error instanceof DOMException && error.name === 'NotAllowedError') {
        toast.error(t('agent.header.copyDenied'));
      } else {
        toast.error(t('agent.header.copyError'));
      }
    } finally {
      setIsCopying(false);
    }
  };

  const handleToggleBookmark = async () => {
    if (!session?.id) {
      return;
    }

    const nextValue = !(isBookmarked ?? false);
    setBookmarkOverride(nextValue);

    try {
      await toggleBookmark(session.id);
    } catch {
      setBookmarkOverride(undefined);
      toast.error(t('agent.header.bookmarkError', 'Failed to update bookmark'));
    }
  };

  return (
    <AgentSessionHeader
      onRenameSession={renameSession}
      isBookmarked={isBookmarked}
      onToggleBookmark={() => {
        void handleToggleBookmark();
      }}
    >
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
                className="h-6 px-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
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

          <HeaderStatusBadges />

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => {
                  toggleShell();
                }}
                aria-label={
                  shellAttention
                    ? t(
                        'agent.header.toggleShellHasUpdatesAria',
                        'Toggle agent panels (has updates)',
                      )
                    : t('agent.header.toggleShellAria', 'Toggle agent panels')
                }
                aria-controls="agent-side-panel-shell"
                aria-expanded={shellOpen}
                className="relative h-6 px-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <PanelRight
                  className={`h-4 w-4 ${shellOpen ? 'text-primary' : ''}`}
                />
                <PanelAttentionDot visible={shellAttention} />
              </Button>
            </TooltipTrigger>
            <TooltipContent>
              {t('agent.header.toggleShellTooltip', 'Toggle agent panels')}
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
