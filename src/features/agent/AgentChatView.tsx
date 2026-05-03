import { useCallback, useEffect, useRef, type CSSProperties } from 'react';
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
import { getLogger } from '@/lib/logger';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { AgentResourceAttachmentProvider } from './hooks/useAgentResourceAttachment';

const logger = getLogger('AgentChatView');

function getSessionLoadingLabel(isStartingSession: boolean): string {
  return isStartingSession ? 'Starting session...' : 'Loading session...';
}

const InitializationStatusDisplay = () => {
  const { initializationStep } = useAgentSessionState();
  if (!initializationStep) return null;

  return (
    <span className="animate-in fade-in slide-in-from-bottom-1 duration-300">
      {initializationStep.step}
    </span>
  );
};

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
  const { showWorkspacePanel } = useAgentWorkspace();
  const { showPlanningPanel } = useAgentPlanning();

  const [searchParams, setSearchParams] = useSearchParams();
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { workflowStatus } = useAgentChatState();
  const hasExecutedPlaybookRef = useRef(false);
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
        toast.success('Playbook started automatically');
      } catch (error) {
        logger.error('Failed to auto-select playbook', error);
        toast.error('Failed to start playbook workflow');
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

  return (
    <>
      <div className="flex h-full w-full overflow-hidden rounded-2xl border border-border/50 bg-background font-sans shadow-[0_18px_48px_-28px_rgba(0,0,0,0.35)]">
        {/* Workspace side panel */}
        {showWorkspacePanel && <AgentWorkspacePanel />}

        {/* Main chat area - components rendered directly inside provider scope */}
        <div
          className="flex-1 flex flex-col min-h-0 min-w-0"
          style={
            {
              '--agent-chat-composer-overlap': '64px',
            } as CSSProperties
          }
        >
          <AgentChatHeader />
          <AgentChatStatusBar />
          <AgentChatMessages />

          <div className="relative shrink-0 px-4 pb-4">
            <div
              aria-hidden="true"
              style={{ height: 'var(--agent-chat-composer-overlap, 64px)' }}
            />
            <div
              className="relative z-10"
              style={{
                marginTop:
                  'calc(var(--agent-chat-composer-overlap, 64px) * -1)',
              }}
            >
              <div className="pointer-events-none absolute inset-x-0 -top-12 h-32 bg-gradient-to-t from-background/80 via-background/28 to-transparent" />
              <AgentChatAttachedFiles />
              <AgentChatInput />
            </div>
          </div>
        </div>

        {/* Planning side panel */}
        {showPlanningPanel && <AgentPlanningPanel />}
      </div>
      <AgentPlanningUpdates />
    </>
  );
}

export default function AgentChatView() {
  const { sessionId: routeSessionId } = useParams<{ sessionId?: string }>();
  const optionalSessionState = useOptionalAgentSessionState();
  const shouldProvideSession =
    optionalSessionState === undefined && !!routeSessionId;

  if (shouldProvideSession && routeSessionId) {
    return (
      <AgentSessionProvider sessionId={routeSessionId}>
        <AgentChatView />
      </AgentSessionProvider>
    );
  }

  if (!optionalSessionState) {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <div className="text-destructive">Session context is unavailable.</div>
      </div>
    );
  }

  const { session, isSessionLoading } = optionalSessionState;
  const attachmentSessionId = session?.id ?? routeSessionId ?? '';
  const isStartingSession =
    session?.status === 'idle' && session?.id === routeSessionId;
  const sessionLoadingLabel = getSessionLoadingLabel(isStartingSession);
  const shouldShowBlockingLoader = isSessionLoading && !session;
  const shouldShowOptimisticLoadingOverlay = isSessionLoading && !!session;

  return (
    <AgentResourceAttachmentProvider sessionId={attachmentSessionId}>
      {shouldShowBlockingLoader ? (
        <div className="flex h-full items-center justify-center p-4">
          <div className="flex flex-col items-center gap-3">
            <LoadingSpinner
              size="lg"
              className="border-4"
              label={sessionLoadingLabel}
            />

            <div className="flex flex-col items-center gap-1">
              <div
                className="text-muted-foreground font-medium animate-pulse"
                aria-hidden="true"
              >
                {sessionLoadingLabel}
              </div>

              <div className="text-xs text-muted-foreground/70 h-4">
                <InitializationStatusDisplay />
              </div>
            </div>
          </div>
        </div>
      ) : !session ? (
        <div className="flex h-full items-center justify-center p-4">
          <div className="text-destructive">
            Session not found or failed to load.
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
            <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/60 backdrop-blur-[1px]">
              <div className="flex flex-col items-center gap-3 rounded-xl border border-border/60 bg-background/90 px-6 py-5 shadow-lg">
                <LoadingSpinner
                  size="lg"
                  className="border-4"
                  label={sessionLoadingLabel}
                />

                <div className="flex flex-col items-center gap-1">
                  <div className="text-muted-foreground font-medium">
                    {sessionLoadingLabel}
                  </div>

                  <div className="text-xs text-muted-foreground/70 h-4">
                    <InitializationStatusDisplay />
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </AgentResourceAttachmentProvider>
  );
}

// Compound component exports
AgentChatView.Header = AgentChatHeader;
AgentChatView.StatusBar = AgentChatStatusBar;
AgentChatView.Messages = AgentChatMessages;
AgentChatView.Input = AgentChatInput;
AgentChatView.AttachedFiles = AgentChatAttachedFiles;
AgentChatView.WorkspacePanel = AgentWorkspacePanel;
AgentChatView.PlanningPanel = AgentPlanningPanel;
