import React, { useEffect, useRef, useState } from 'react';
import { useOptionalAgentSessionState } from '@/context/AgentSessionContext';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import {
  Check,
  Loader2,
  Pencil,
  X,
  Bookmark,
  BookmarkCheck,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { toast } from 'sonner';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';

interface AgentSessionHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
  sessionName?: string;
  sessionType?: string;
  isBookmarked?: boolean;
  onToggleBookmark?: () => void;
  assistantNameClassName?: string;
  sessionNameClassName?: string;
  onRenameSession?: (name: string) => Promise<void>;
}

export default function AgentSessionHeader({
  children,
  assistantName,
  sessionName,
  sessionType = 'Agent',
  isBookmarked,
  onToggleBookmark,
  assistantNameClassName,
  sessionNameClassName,
  onRenameSession,
}: AgentSessionHeaderProps) {
  const { t } = useTranslation('common');
  const optionalSessionState = useOptionalAgentSessionState();
  const session = optionalSessionState?.session;
  const [isEditingSessionName, setIsEditingSessionName] = useState(false);
  const [draftSessionName, setDraftSessionName] = useState('');
  const [isSavingSessionName, setIsSavingSessionName] = useState(false);
  const sessionNameInputRef = useRef<HTMLInputElement | null>(null);

  // Fallback display if session is not yet loaded
  const resolvedSessionName =
    sessionName ??
    session?.name ??
    t('agent.header.untitledSession', 'Untitled Session');
  const resolvedAssistantName =
    assistantName ??
    session?.assistant?.name ??
    t('agent.header.defaultAssistant', 'Agent');
  const canEditSessionName = Boolean(session?.id && onRenameSession);

  const renderSessionTypeOrBookmark = () => {
    if (isBookmarked !== undefined && onToggleBookmark) {
      const tooltipLabel = isBookmarked
        ? t('sessionHistory.actions.unbookmarkAria', 'Remove bookmark')
        : t('sessionHistory.actions.bookmarkAria', 'Bookmark session');

      return (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-6 w-6 shrink-0"
              onClick={onToggleBookmark}
              aria-label={tooltipLabel}
            >
              {isBookmarked ? (
                <BookmarkCheck className="h-3.5 w-3.5 text-warning" />
              ) : (
                <Bookmark className="h-3.5 w-3.5 text-muted-foreground" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{tooltipLabel}</TooltipContent>
        </Tooltip>
      );
    }
    if (sessionType) {
      return (
        <span className="shrink-0 text-xs text-muted-foreground">
          ({sessionType})
        </span>
      );
    }
    return null;
  };

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
    <div>
      <div className="flex shrink-0 items-center justify-between border-b border-border/40 bg-background px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            Assistant
          </span>
          <span
            className={cn(
              'truncate text-xs font-medium text-foreground',
              assistantNameClassName,
            )}
          >
            {resolvedAssistantName}
          </span>
        </div>
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            {t('agent.header.sessionLabel', 'Session')}
          </span>
          {isEditingSessionName ? (
            <div className="flex min-w-0 items-center gap-2">
              <Input
                ref={sessionNameInputRef}
                value={draftSessionName}
                onChange={(event) => setDraftSessionName(event.target.value)}
                onKeyDown={handleSessionNameKeyDown}
                disabled={isSavingSessionName}
                className="h-8 w-[16rem] max-w-[50vw]"
                aria-label={t(
                  'agent.header.renameInputAria',
                  'Edit session title',
                )}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="h-8 w-8"
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
                className="h-8 w-8"
                onClick={handleCancelEditing}
                disabled={isSavingSessionName}
                aria-label={t(
                  'agent.header.renameCancelAria',
                  'Cancel editing session title',
                )}
              >
                <X className="h-4 w-4" />
              </Button>
              {renderSessionTypeOrBookmark()}
            </div>
          ) : (
            <>
              <span
                className={cn(
                  'max-w-xs truncate text-sm font-medium text-foreground/90',
                  sessionNameClassName,
                )}
                title={resolvedSessionName}
              >
                {resolvedSessionName}
              </span>
              {renderSessionTypeOrBookmark()}
              {canEditSessionName && (
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
              )}
            </>
          )}
        </div>
      </div>

      {children && (
        <div className="shrink-0 border-b border-border/40 bg-background/90 px-4 py-2.5">
          <div className="flex items-center justify-between">
            <div className="flex gap-2"></div>
            {children}
          </div>
        </div>
      )}
    </div>
  );
}
