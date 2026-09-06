import React, { useEffect, useRef, useState } from 'react';
import { useOptionalAgentSessionState } from '@/context/AgentSessionContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';
import {
  Bookmark,
  BookmarkCheck,
  Check,
  Loader2,
  Pencil,
  X,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';

interface AgentSessionHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
  sessionName?: string;
  sessionType?: string;
  isBookmarked?: boolean;
  assistantNameClassName?: string;
  sessionNameClassName?: string;
  onToggleBookmark?: () => void;
  onRenameSession?: (name: string) => Promise<void>;
}

export default function AgentSessionHeader({
  children,
  assistantName,
  sessionName,
  sessionType = 'Agent',
  isBookmarked,
  assistantNameClassName,
  sessionNameClassName,
  onToggleBookmark,
  onRenameSession,
}: AgentSessionHeaderProps) {
  const { t } = useTranslation('common');
  const optionalSessionState = useOptionalAgentSessionState();
  const session = optionalSessionState?.session;
  const [isEditingSessionName, setIsEditingSessionName] = useState(false);
  const [draftSessionName, setDraftSessionName] = useState('');
  const [isSavingSessionName, setIsSavingSessionName] = useState(false);
  const sessionNameInputRef = useRef<HTMLInputElement | null>(null);

  const resolvedSessionName =
    sessionName ??
    session?.name ??
    t('agent.header.untitledSession', 'Untitled Session');
  const resolvedAssistantName =
    assistantName ??
    session?.assistant?.name ??
    t('agent.header.defaultAssistant', 'Agent');
  const canEditSessionName = Boolean(session?.id && onRenameSession);
  const bookmarked = isBookmarked ?? false;
  // Prefer bookmark control whenever a toggle is wired — avoid flashing
  // `(sessionType)` while bookmark state is still undefined on first paint.
  const sessionMetaAction = onToggleBookmark ? (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn(
            'h-7 w-7 shrink-0',
            bookmarked &&
              'border-warning/20 bg-warning/10 text-warning-foreground hover:bg-warning/20',
          )}
          onClick={onToggleBookmark}
          aria-label={
            bookmarked
              ? t('sessionHistory.actions.unbookmarkAria', 'Remove bookmark')
              : t('sessionHistory.actions.bookmarkAria', 'Bookmark session')
          }
        >
          {bookmarked ? (
            <BookmarkCheck className="h-3.5 w-3.5 text-warning" />
          ) : (
            <Bookmark className="h-3.5 w-3.5 text-muted-foreground" />
          )}
        </Button>
      </TooltipTrigger>
      <TooltipContent>
        {bookmarked
          ? t('sessionHistory.actions.unbookmark', 'Remove bookmark')
          : t('sessionHistory.actions.bookmark', 'Bookmark')}
      </TooltipContent>
    </Tooltip>
  ) : sessionType ? (
    <span className="shrink-0 text-xs text-muted-foreground">
      ({sessionType})
    </span>
  ) : null;

  useEffect(() => {
    if (!isEditingSessionName) {
      return;
    }

    const frameId = window.requestAnimationFrame(() => {
      sessionNameInputRef.current?.focus();
      sessionNameInputRef.current?.select();
    });

    return () => window.cancelAnimationFrame(frameId);
  }, [isEditingSessionName]);

  const handleStartEditing = () => {
    setDraftSessionName(resolvedSessionName);
    setIsEditingSessionName(true);
  };

  const handleCancelEditing = () => {
    setDraftSessionName(resolvedSessionName);
    setIsEditingSessionName(false);
    setIsSavingSessionName(false);
  };

  const handleSaveSessionName = async () => {
    if (!onRenameSession) {
      return;
    }

    const normalizedName = draftSessionName.trim();
    if (!normalizedName) {
      toast.error(
        t('agent.header.renameEmptyError', 'Session title cannot be empty'),
      );
      return;
    }

    if (normalizedName === resolvedSessionName) {
      handleCancelEditing();
      return;
    }

    setIsSavingSessionName(true);
    try {
      await onRenameSession(normalizedName);
      toast.success(t('agent.header.renameSuccess', 'Session title updated'));
      setIsEditingSessionName(false);
    } catch {
      toast.error(
        t('agent.header.renameError', 'Failed to update session title'),
      );
    } finally {
      setIsSavingSessionName(false);
    }
  };

  const handleSessionNameKeyDown = async (
    event: React.KeyboardEvent<HTMLInputElement>,
  ) => {
    if (event.key === 'Enter') {
      event.preventDefault();
      await handleSaveSessionName();
      return;
    }

    if (event.key === 'Escape') {
      event.preventDefault();
      handleCancelEditing();
    }
  };

  return (
    <div
      className="flex shrink-0 items-center gap-3 border-b border-border/40 bg-background px-4 py-2"
      data-testid="agent-session-header"
    >
      <div className="flex min-w-0 flex-1 items-center gap-2">
        <span
          className={cn(
            'min-w-0 max-w-[160px] shrink truncate text-xs font-medium text-muted-foreground',
            assistantNameClassName,
          )}
          title={resolvedAssistantName}
        >
          {resolvedAssistantName}
        </span>
        <span className="shrink-0 text-muted-foreground/35" aria-hidden="true">
          ·
        </span>
        {isEditingSessionName ? (
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <Input
              ref={sessionNameInputRef}
              value={draftSessionName}
              onChange={(event) => setDraftSessionName(event.target.value)}
              onKeyDown={handleSessionNameKeyDown}
              disabled={isSavingSessionName}
              className="h-7 min-w-0 flex-1 max-w-[16rem]"
              aria-label={t(
                'agent.header.renameInputAria',
                'Edit session title',
              )}
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-7 w-7 shrink-0"
              onClick={() => void handleSaveSessionName()}
              disabled={isSavingSessionName}
              aria-label={t(
                'agent.header.renameSaveAria',
                'Save session title',
              )}
            >
              {isSavingSessionName ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Check className="h-4 w-4" />
              )}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-7 w-7 shrink-0"
              onClick={handleCancelEditing}
              disabled={isSavingSessionName}
              aria-label={t(
                'agent.header.renameCancelAria',
                'Cancel editing session title',
              )}
            >
              <X className="h-4 w-4" />
            </Button>
            {sessionMetaAction}
          </div>
        ) : (
          <div className="flex min-w-0 flex-1 items-center gap-2">
            <span
              className={cn(
                'min-w-0 flex-1 truncate text-sm font-medium text-foreground/90',
                sessionNameClassName,
              )}
              title={resolvedSessionName}
            >
              {resolvedSessionName}
            </span>
            {canEditSessionName ? (
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-7 w-7 shrink-0"
                onClick={handleStartEditing}
                aria-label={t('agent.header.renameAria', 'Rename session')}
              >
                <Pencil className="h-3.5 w-3.5" />
              </Button>
            ) : null}
            {sessionMetaAction}
          </div>
        )}
      </div>

      {children ? (
        <div className="flex shrink-0 items-center gap-2">{children}</div>
      ) : null}
    </div>
  );
}
