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
import { PanelRight } from 'lucide-react';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import { HeaderStatusBadges } from './HeaderStatusBadges';
import { PanelAttentionDot } from './PanelAttentionDot';
import { SessionExportMenu } from './SessionExportMenu';

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
  const [bookmarkOverride, setBookmarkOverride] = useState<
    boolean | undefined
  >();
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
      assistantName={assistantName}
      onRenameSession={renameSession}
      isBookmarked={isBookmarked ?? false}
      onToggleBookmark={() => {
        void handleToggleBookmark();
      }}
    >
      {children}
      {session?.id ? (
        <SessionExportMenu
          sessionId={session.id}
          messages={messages}
          showClipboardCopy
        />
      ) : null}

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

      {session?.id ? (
        <SessionFilesPopover key={session.id} sessionId={session.id} />
      ) : null}
    </AgentSessionHeader>
  );
}
