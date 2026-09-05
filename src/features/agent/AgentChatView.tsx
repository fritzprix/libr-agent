import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useRef,
  type ReactNode,
  type CSSProperties,
} from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { agentCallBuiltinTool } from '@/lib/backend/agent-commands';
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
  AgentPanelsProvider,
  useAgentPanels,
} from '@/context/AgentPanelsContext';
import { AgentChatHeader } from './components/AgentChatHeader';
import { AgentChatStatusBar } from './components/AgentChatStatusBar';
import { AgentChatMessages } from './components/AgentChatMessages';
import { AgentChatInput } from './components/AgentChatInput';
import { AgentChatAttachedFiles } from './components/AgentChatAttachedFiles';
import { AgentSidePanelShell } from './components/AgentSidePanelShell';
import { AgentPlanningUpdates } from './components/AgentPlanningUpdates';
import { AgentProcessAttentionUpdates } from './components/AgentProcessAttentionUpdates';
import { SessionLoadingOverlay } from './components/SessionLoadingOverlay';
import { PanelResizeHandle } from './components/PanelResizeHandle';
import { usePanelShortcuts } from './hooks/usePanelShortcuts';
import { MIN_PANEL_WIDTH, usePanelWidth } from './hooks/usePanelWidth';
import { getLogger } from '@/lib/logger';
import { AgentResourceAttachmentProvider } from './hooks/useAgentResourceAttachment';
import { useMcpDiscoveryToasts } from './hooks/useMcpDiscoveryToasts';
import { useTranslation } from 'react-i18next';
import { useIsMobile } from '@/hooks/use-mobile';
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from '@/components/ui/sheet';
import { cn } from '@/lib/utils';

const logger = getLogger('AgentChatView');

const InteractiveShellPromptDialog = lazy(
  () => import('./components/InteractiveShellPromptDialog'),
);

const AGENT_CHAT_COMPOSER_OVERLAP_STYLE = {
  '--agent-chat-composer-overlap': '64px',
} as CSSProperties;

function getSessionLoadingLabel(
  isStartingSession: boolean,
  isDockerProvisioning: boolean,
  dockerImage: string | undefined,
  t: (key: string, options?: Record<string, unknown>) => string,
): string {
  if (isDockerProvisioning) {
    return t('agent.chat.preparingDocker', {
      image: dockerImage ?? 'Docker image',
      defaultValue: 'Preparing Docker workspace ({{image}})',
    });
  }
  return isStartingSession
    ? t('agent.start.startingSession')
    : t('agent.chat.loadingSession');
}

interface DesktopPanelRailProps {
  side: 'left' | 'right';
  open: boolean;
  width: number;
  children: ReactNode;
}

function DesktopPanelRail({
  side,
  open,
  width,
  children,
}: DesktopPanelRailProps) {
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
      data-testid="desktop-panel-rail"
      data-panel-width={width}
      style={{ width }}
      className={cn(
        'absolute inset-y-0 z-20',
        // Open/close uses transform only — do not animate width/padding (jitter).
        'transition-transform duration-300 ease-out',
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

/**
 * Agent Chat View - Compound Component Pattern
 *
 * Enhanced UI for agent chat interaction with three-column layout.
 *
 * Layout:
 * - Left: Workspace or Processes panel (mutually exclusive)
 * - Center: Chat interface
 * - Right: Planning panel (optional)
 *
 * Features:
 * - Virtualized message rendering with react-virtuoso
 * - Tool call visualization with AgentToolCallGroup
 * - Side panel management via AgentPanelsContext
 * - Workflow status display
 */

function AgentChatInner() {
  const isMobile = useIsMobile();
  const { closeShell, openShell, isShellOpen } = useAgentPanels();
  const showSidePanel = isShellOpen();
  const { t } = useTranslation();
  usePanelShortcuts();
  const {
    containerRef,
    panelWidth,
    maxWidth,
    setPanelWidth,
    commitPanelWidth,
    resetPanelWidth,
  } = usePanelWidth();

  const [searchParams, setSearchParams] = useSearchParams();
  const { pendingInteractiveShellPrompt, session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { workflowStatus } = useAgentChatState();
  const hasExecutedPlaybookRef = useRef(false);
  const hasOpenedShellRef = useRef(!isMobile && showSidePanel);

  useEffect(() => {
    if (!isMobile && showSidePanel) {
      hasOpenedShellRef.current = true;
    }
  }, [isMobile, showSidePanel]);

  const hasOpenedShellDesktop =
    !isMobile && (showSidePanel || hasOpenedShellRef.current);

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

  const handleSidePanelSheetOpenChange = useCallback(
    (nextOpen: boolean) => {
      if (nextOpen) {
        openShell();
        return;
      }
      closeShell();
    },
    [closeShell, openShell],
  );

  return (
    <>
      {sessionId && pendingInteractiveShellPrompt ? (
        <Suspense fallback={null}>
          <InteractiveShellPromptDialog
            sessionId={sessionId}
            promptState={pendingInteractiveShellPrompt}
          />
        </Suspense>
      ) : null}
      <div
        className="flex h-full w-full flex-col overflow-hidden rounded-2xl border border-border/50 bg-background font-sans shadow-[0_18px_48px_-28px_rgba(0,0,0,0.35)]"
        style={AGENT_CHAT_COMPOSER_OVERLAP_STYLE}
      >
        <AgentChatHeader />
        <div
          ref={containerRef}
          className="relative flex min-h-0 flex-1 overflow-hidden"
          data-testid="agent-chat-body"
          style={
            {
              '--agent-side-panel-inset':
                !isMobile && showSidePanel ? `${panelWidth}px` : '0px',
            } as CSSProperties
          }
        >
          {/* Chat keeps full width — panel is a pure overlay, never shrinks this column. */}
          <div className="flex min-h-0 min-w-0 flex-1 flex-col">
            <AgentChatStatusBar />
            <AgentChatMessages />
          </div>

          {!isMobile && hasOpenedShellDesktop ? (
            <DesktopPanelRail
              side="right"
              open={showSidePanel}
              width={panelWidth}
            >
              <PanelResizeHandle
                panelWidth={panelWidth}
                minWidth={MIN_PANEL_WIDTH}
                maxWidth={maxWidth}
                disabled={!showSidePanel}
                onResize={setPanelWidth}
                onResizeEnd={commitPanelWidth}
                onReset={resetPanelWidth}
              />
              <AgentSidePanelShell isVisible={showSidePanel} variant="rail" />
            </DesktopPanelRail>
          ) : null}
        </div>
        <AgentChatComposer />
      </div>

      {isMobile ? (
        <MobilePanelSheet
          open={showSidePanel}
          onOpenChange={handleSidePanelSheetOpenChange}
          side="right"
          title={t('agent.panels.shellTitle', 'Agent panels')}
          description={t('agent.panels.shellTitle', 'Agent panels')}
        >
          <AgentSidePanelShell isVisible variant="sheet" />
        </MobilePanelSheet>
      ) : null}
      <AgentPlanningUpdates />
      <AgentProcessAttentionUpdates />
    </>
  );
}

function AgentChatViewContent({
  optionalSessionState,
  routeSessionId,
}: {
  optionalSessionState: NonNullable<
    ReturnType<typeof useOptionalAgentSessionState>
  >;
  routeSessionId?: string;
}) {
  const { t } = useTranslation();
  const { session, isSessionLoading } = optionalSessionState;
  const runtimeState = optionalSessionState.runtimeState;
  const attachmentSessionId = session?.id ?? routeSessionId ?? '';
  const isStartingSession =
    session?.status === 'idle' && session?.id === routeSessionId;
  const isDockerProvisioning =
    session?.status === 'provisioning' ||
    runtimeState.initialization.docker !== undefined;
  const sessionLoadingLabel = getSessionLoadingLabel(
    isStartingSession,
    isDockerProvisioning,
    session?.dockerConfig?.image ??
      (session?.dockerConfig?.attachContainer
        ? `attach:${session.dockerConfig.attachContainer}`
        : undefined) ??
      runtimeState.initialization.docker?.image,
    t,
  );
  const initializationStep =
    runtimeState.initialization.docker?.step ??
    runtimeState.initialization.currentStep ??
    optionalSessionState.initializationStep?.step;
  const initializationError =
    runtimeState.phase === 'failed'
      ? (runtimeState.initialization.docker?.error ??
        runtimeState.initialization.error ??
        null)
      : null;
  const isProxyReady = optionalSessionState.isProxyReady;
  const shouldShowBlockingLoader =
    (!session || runtimeState.phase === 'failed') && isSessionLoading;
  useMcpDiscoveryToasts({
    hasSession: Boolean(session),
    isProxyReady,
    phase: runtimeState.phase,
    initResult: runtimeState.initialization.result,
    servers: runtimeState.servers,
    sessionId: session?.id,
    currentStep: initializationStep,
  });

  return (
    <AgentResourceAttachmentProvider sessionId={attachmentSessionId}>
      {shouldShowBlockingLoader ? (
        <SessionLoadingOverlay
          label={sessionLoadingLabel}
          initializationStep={initializationStep}
          initializationError={initializationError}
          servers={runtimeState.servers}
          initResult={runtimeState.initialization.result}
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
            <AgentPanelsProvider>
              <AgentChatInner />
            </AgentPanelsProvider>
          </AgentChatProvider>
        </div>
      )}
    </AgentResourceAttachmentProvider>
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

  return (
    <AgentChatViewContent
      optionalSessionState={optionalSessionState}
      routeSessionId={routeSessionId}
    />
  );
}
