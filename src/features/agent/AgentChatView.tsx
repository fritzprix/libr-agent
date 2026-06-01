import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
  type CSSProperties,
  type FormEvent,
} from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import {
  agentCallBuiltinTool,
  cancelInteractiveShellInput,
  submitInteractiveShellInput,
} from '@/lib/backend/agent-commands';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { MCPContent } from '@/lib/mcp';
import { toast } from 'sonner';
import {
  useAgentChatActions,
  useAgentChatState,
} from '@/context/AgentChatContext';
import {
  AgentSessionProvider,
  useAgentSessionState,
  useOptionalAgentSessionState,
} from '@/context/AgentSessionContext';
import { AgentChatProvider } from '@/context/AgentChatContext';
import {
  AgentWorkspaceProvider,
  useAgentWorkspace,
} from '@/context/AgentWorkspaceContext';
import {
  AgentPlanningProvider,
  useAgentPlanning,
} from '@/context/AgentPlanningContext';
import { AgentChatHeader } from './components/AgentChatHeader';
import { AgentChatStatusBar } from './components/AgentChatStatusBar';
import { AgentChatMessages } from './components/AgentChatMessages';
import { AgentChatInput } from './components/AgentChatInput';
import { AgentChatAttachedFiles } from './components/AgentChatAttachedFiles';
import { AgentWorkspacePanel } from './components/AgentWorkspacePanel';
import { AgentPlanningPanel } from './components/AgentPlanningPanel';
import { AgentPlanningUpdates } from './components/AgentPlanningUpdates';
import { SessionLoadingOverlay } from './components/SessionLoadingOverlay';
import { getLogger } from '@/lib/logger';
import { AgentResourceAttachmentProvider } from './hooks/useAgentResourceAttachment';
import { useTranslation } from 'react-i18next';
import { useIsMobile } from '@/hooks/use-mobile';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';

const logger = getLogger('AgentChatView');

function getSessionLoadingLabel(
  isStartingSession: boolean,
  t: (key: string) => string,
): string {
  return isStartingSession
    ? t('agent.start.startingSession')
    : t('agent.chat.loadingSession');
}

interface DesktopPanelRailProps {
  side: 'left' | 'right';
  open: boolean;
  children: ReactNode;
}

function DesktopPanelRail({ side, open, children }: DesktopPanelRailProps) {
  const railRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const rail = railRef.current;
    if (!rail) {
      return;
    }

    if (!open) {
      rail.setAttribute('inert', '');
      return;
    }

    rail.removeAttribute('inert');
  }, [open]);

  return (
    <aside
      ref={railRef}
      aria-hidden={!open}
      data-open={open}
      className={cn(
        'absolute inset-y-0 z-20 w-80 transition-transform duration-300 ease-out',
        !open && 'pointer-events-none',
        side === 'left'
          ? 'left-0 data-[open=false]:-translate-x-full'
          : 'right-0 data-[open=false]:translate-x-full',
      )}
    >
      {children}
    </aside>
  );
}

interface MobilePanelSheetProps {
  description: string;
  open: boolean;
  side: 'left' | 'right';
  title: string;
  onOpenChange: (open: boolean) => void;
  children: ReactNode;
}

function MobilePanelSheet({
  children,
  description,
  open,
  side,
  title,
  onOpenChange,
}: MobilePanelSheetProps) {
  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent
        side={side}
        className="w-full max-w-none gap-0 p-0 sm:max-w-sm"
      >
        <SheetHeader className="sr-only">
          <SheetTitle>{title}</SheetTitle>
          <SheetDescription>{description}</SheetDescription>
        </SheetHeader>
        <div className="h-full pt-12">{children}</div>
      </SheetContent>
    </Sheet>
  );
}

function AgentChatComposer() {
  return (
    <div className="relative shrink-0 px-4 pb-4">
      <div
        aria-hidden="true"
        style={{ height: 'var(--agent-chat-composer-overlap, 64px)' }}
      />
      <div
        className="relative z-10"
        style={{
          marginTop: 'calc(var(--agent-chat-composer-overlap, 64px) * -1)',
        }}
      >
        <div className="pointer-events-none absolute inset-x-0 -top-12 h-32 bg-gradient-to-t from-background/80 via-background/28 to-transparent" />
        <AgentChatAttachedFiles />
        <AgentChatInput />
      </div>
    </div>
  );
}

interface InteractiveShellPromptDialogProps {
  sessionId: string;
  promptState: ReturnType<
    typeof useAgentSessionState
  >['pendingInteractiveShellPrompt'];
}

function InteractiveShellPromptDialog({
  sessionId,
  promptState,
}: InteractiveShellPromptDialogProps) {
  const [inputValue, setInputValue] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    setInputValue('');
    setIsSubmitting(false);
  }, [promptState?.executionId]);

  const handleCancel = useCallback(async () => {
    if (!promptState || isSubmitting) {
      return;
    }

    setIsSubmitting(true);
    try {
      await cancelInteractiveShellInput(sessionId, promptState.executionId);
    } catch (error) {
      logger.error('Failed to cancel interactive shell prompt', error);
      toast.error('Failed to cancel interactive prompt.');
      setIsSubmitting(false);
    }
  }, [isSubmitting, promptState, sessionId]);

  const handleSubmit = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();

      if (!promptState || isSubmitting) {
        return;
      }

      setIsSubmitting(true);
      try {
        await submitInteractiveShellInput(
          sessionId,
          promptState.executionId,
          inputValue,
        );
        setInputValue('');
      } catch (error) {
        logger.error('Failed to submit interactive shell input', error);
        toast.error('Failed to submit interactive input.');
        setIsSubmitting(false);
      }
    },
    [inputValue, isSubmitting, promptState, sessionId],
  );

  return (
    <Dialog
      open={promptState !== null}
      onOpenChange={(open) => {
        if (!open) {
          void handleCancel();
        }
      }}
    >
      <DialogContent
        showCloseButton={false}
        onEscapeKeyDown={(event) => {
          event.preventDefault();
          void handleCancel();
        }}
        onInteractOutside={(event) => {
          event.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle>Interactive shell input required</DialogTitle>
          <DialogDescription>
            {promptState?.command ?? 'A shell command'} is waiting for local
            user input.
          </DialogDescription>
        </DialogHeader>

        {promptState ? (
          <form className="space-y-4" onSubmit={handleSubmit}>
            <div className="space-y-2">
              <Label htmlFor="interactive-shell-input">
                {promptState.prompt}
              </Label>
              <Input
                autoFocus
                id="interactive-shell-input"
                type={promptState.inputType}
                value={inputValue}
                onChange={(event) => setInputValue(event.target.value)}
                autoComplete="off"
                spellCheck={false}
                disabled={isSubmitting}
              />
            </div>

            <DialogFooter>
              <Button
                type="button"
                variant="outline"
                onClick={() => void handleCancel()}
                disabled={isSubmitting}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={isSubmitting}>
                Submit
              </Button>
            </DialogFooter>
          </form>
        ) : null}
      </DialogContent>
    </Dialog>
  );
}

/**
 * Agent Chat View - Compound Component Pattern
 *
 * Enhanced UI for agent chat interaction with three-column layout.
 *
 * Layout:
 * - Left: Workspace panel (optional)
 * - Center: Chat interface
 * - Right: Planning panel (optional)
 *
 * Features:
 * - Virtualized message rendering with react-virtuoso
 * - Tool call visualization with AgentToolCallGroup
 * - Side panel management with mutual exclusion
 * - Workflow status display
 */

function AgentChatInner() {
  const isMobile = useIsMobile();
  const { closeWorkspacePanel, openWorkspacePanel, showWorkspacePanel } =
    useAgentWorkspace();
  const { closePlanningPanel, openPlanningPanel, showPlanningPanel } =
    useAgentPlanning();
  const { t } = useTranslation();

  const [searchParams, setSearchParams] = useSearchParams();
  const { pendingInteractiveShellPrompt, session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { workflowStatus } = useAgentChatState();
  const hasExecutedPlaybookRef = useRef(false);
  const hasOpenedWorkspaceRef = useRef(!isMobile && showWorkspacePanel);
  const hasOpenedPlanningRef = useRef(!isMobile && showPlanningPanel);

  useEffect(() => {
    if (!isMobile && showWorkspacePanel) {
      hasOpenedWorkspaceRef.current = true;
    }
  }, [isMobile, showWorkspacePanel]);

  useEffect(() => {
    if (!isMobile && showPlanningPanel) {
      hasOpenedPlanningRef.current = true;
    }
  }, [isMobile, showPlanningPanel]);

  const hasOpenedWorkspaceDesktop =
    !isMobile && (showWorkspacePanel || hasOpenedWorkspaceRef.current);
  const hasOpenedPlanningDesktop =
    !isMobile && (showPlanningPanel || hasOpenedPlanningRef.current);

  const playbookId = searchParams.get('playbookId');
  const sessionId = session?.id;
  const assistantId = session?.assistant?.id;

  const executePlaybookSelection = useCallback(
    async (selectedPlaybookId: string) => {
      if (!sessionId) return;
      logger.info('Auto-executing playbook', {
        playbookId: selectedPlaybookId,
      });

      try {
        const result = await agentCallBuiltinTool<{ content: MCPContent[] }>(
          sessionId,
          'playbook__selectPlaybook',
          { id: selectedPlaybookId },
        );

        const toolCallId = createId();
        const [toolCallMsg, toolResultMsg] = createToolMessagePair(
          'playbook__selectPlaybook',
          { id: selectedPlaybookId },
          result.content ?? [],
          toolCallId,
          sessionId,
          undefined,
          assistantId,
          'ui',
        );

        await injectMessages([toolCallMsg, toolResultMsg]);
        toast.success(t('agent.chat.playbookStartedAutomatically'));
      } catch (error) {
        logger.error('Failed to auto-select playbook', error);
        toast.error(t('agent.chat.failedToStartPlaybookWorkflow'));
      }
    },
    [assistantId, injectMessages, sessionId],
  );

  useEffect(() => {
    if (
      !playbookId ||
      !sessionId ||
      workflowStatus !== 'idle' ||
      hasExecutedPlaybookRef.current
    ) {
      return;
    }

    hasExecutedPlaybookRef.current = true;
    void executePlaybookSelection(playbookId);

    // Remove query param to prevent re-execution on refresh or render.
    setSearchParams(
      (prev) => {
        const nextParams = new URLSearchParams(prev);
        nextParams.delete('playbookId');
        return nextParams;
      },
      { replace: true },
    );
  }, [
    executePlaybookSelection,
    playbookId,
    sessionId,
    setSearchParams,
    workflowStatus,
  ]);

  const handleWorkspaceSheetOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (nextOpen) {
        openWorkspacePanel();
        return;
      }
      closeWorkspacePanel();
    },
    [closeWorkspacePanel, openWorkspacePanel],
  );

  const handlePlanningSheetOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (nextOpen) {
        openPlanningPanel();
        return;
      }
      closePlanningPanel();
    },
    [closePlanningPanel, openPlanningPanel],
  );

  return (
    <>
      {sessionId ? (
        <InteractiveShellPromptDialog
          sessionId={sessionId}
          promptState={pendingInteractiveShellPrompt}
        />
      ) : null}
      <div
        className="flex h-full w-full flex-col overflow-hidden rounded-2xl border border-border/50 bg-background font-sans shadow-[0_18px_48px_-28px_rgba(0,0,0,0.35)]"
        style={
          {
            '--agent-chat-composer-overlap': '64px',
          } as CSSProperties
        }
      >
        <AgentChatHeader />
        <div
          className="relative flex min-h-0 flex-1 overflow-hidden"
          data-testid="agent-chat-body"
        >
          {!isMobile && hasOpenedWorkspaceDesktop && (
            <DesktopPanelRail side="left" open={showWorkspacePanel}>
              <AgentWorkspacePanel
                isVisible={showWorkspacePanel}
                variant="rail"
              />
            </DesktopPanelRail>
          )}

          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <AgentChatStatusBar />
            <AgentChatMessages />
          </div>

          {!isMobile && hasOpenedPlanningDesktop && (
            <DesktopPanelRail side="right" open={showPlanningPanel}>
              <AgentPlanningPanel
                isVisible={showPlanningPanel}
                variant="rail"
              />
            </DesktopPanelRail>
          )}
        </div>
        <AgentChatComposer />
      </div>

      {isMobile && (
        <>
          <MobilePanelSheet
            open={showWorkspacePanel}
            onOpenChange={handleWorkspaceSheetOpenChange}
            side="left"
            title={t('agent.workspace.title')}
            description={t('agent.workspace.title')}
          >
            <AgentWorkspacePanel isVisible variant="sheet" />
          </MobilePanelSheet>
          <MobilePanelSheet
            open={showPlanningPanel}
            onOpenChange={handlePlanningSheetOpenChange}
            side="right"
            title={t('agent.planning.title')}
            description={t('agent.planning.title')}
          >
            <AgentPlanningPanel isVisible variant="sheet" />
          </MobilePanelSheet>
        </>
      )}
      <AgentPlanningUpdates />
    </>
  );
}

export default function AgentChatView() {
  const { t } = useTranslation();
  const { sessionId: routeSessionId } = useParams<{ sessionId?: string }>();
  const optionalSessionState = useOptionalAgentSessionState();
  const shouldProvideSession =
    optionalSessionState === undefined && !!routeSessionId;

  if (shouldProvideSession) {
    return (
      <AgentSessionProvider sessionId={routeSessionId}>
        <AgentChatView />
      </AgentSessionProvider>
    );
  }

  if (!optionalSessionState) {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <div className="text-destructive">
          {t('agent.chat.sessionContextUnavailable')}
        </div>
      </div>
    );
  }

  const { session, isSessionLoading } = optionalSessionState;
  const attachmentSessionId = session?.id ?? routeSessionId ?? '';
  const isStartingSession =
    session?.status === 'idle' && session?.id === routeSessionId;
  const sessionLoadingLabel = getSessionLoadingLabel(isStartingSession, t);
  const initializationStep = optionalSessionState.initializationStep?.step;
  const shouldShowBlockingLoader = isSessionLoading && !session;
  const shouldShowOptimisticLoadingOverlay = isSessionLoading && !!session;

  return (
    <AgentResourceAttachmentProvider sessionId={attachmentSessionId}>
      {shouldShowBlockingLoader ? (
        <SessionLoadingOverlay
          label={sessionLoadingLabel}
          initializationStep={initializationStep}
          variant="blocking"
        />
      ) : !session ? (
        <div className="flex h-full items-center justify-center p-4">
          <div className="text-destructive">
            {t('agent.chat.sessionNotFoundOrFailed')}
          </div>
        </div>
      ) : (
        <div className="relative h-full">
          <AgentChatProvider>
            <AgentPlanningProvider>
              <AgentWorkspaceProvider>
                <AgentChatInner />
              </AgentWorkspaceProvider>
            </AgentPlanningProvider>
          </AgentChatProvider>

          {shouldShowOptimisticLoadingOverlay && (
            <SessionLoadingOverlay
              label={sessionLoadingLabel}
              initializationStep={initializationStep}
              variant="overlay"
            />
          )}
        </div>
      )}
    </AgentResourceAttachmentProvider>
  );
}
