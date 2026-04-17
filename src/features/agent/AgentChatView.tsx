import { useEffect, useRef } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { useTranslation } from 'react-i18next';
import { agentCallBuiltinTool } from '@/lib/backend/agent-commands';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { MCPContent } from '@/lib/mcp';
import { toast } from 'sonner';
import {
  useAgentChatActions,
  useAgentChatState,
} from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { AgentChatProvider } from '@/context/AgentChatContext';
import {
  AgentWorkspaceProvider,
  useAgentWorkspace,
} from '@/context/AgentWorkspaceContext';
import {
  AgentPlanningProvider,
  useAgentPlanning,
} from '@/context/AgentPlanningContext';
import { AgentResourceAttachmentProvider } from './context/AgentResourceAttachmentContext';
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

const logger = getLogger('AgentChatView');

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
 * - Message virtualization with react-virtuoso
 * - Tool call visualization with AgentToolCallGroup
 * - Side panel management with mutual exclusion
 * - Workflow status display
 */

function AgentChatInner() {
  const { t } = useTranslation();
  const { showWorkspacePanel } = useAgentWorkspace();
  const { showPlanningPanel } = useAgentPlanning();

  const [searchParams, setSearchParams] = useSearchParams();
  const { session } = useAgentSessionState();
  const { injectMessages } = useAgentChatActions();
  const { workflowStatus } = useAgentChatState();
  const hasExecutedPlaybookRef = useRef(false);

  useEffect(() => {
    const playbookId = searchParams.get('playbookId');
    if (
      playbookId &&
      session &&
      workflowStatus === 'idle' &&
      !hasExecutedPlaybookRef.current
    ) {
      hasExecutedPlaybookRef.current = true;
      executePlaybookSelection(playbookId);
      // Remove query param to prevent re-execution on refresh or render
      setSearchParams(
        (prev) => {
          const newParams = new URLSearchParams(prev);
          newParams.delete('playbookId');
          return newParams;
        },
        { replace: true },
      );
    }
  }, [session, workflowStatus, searchParams]);

  const executePlaybookSelection = async (playbookId: string) => {
    if (!session?.id) return;
    if (!session?.id) return;
    logger.info('Auto-executing playbook', { playbookId });

    try {
      const result = await agentCallBuiltinTool<{ content: MCPContent[] }>(
        session.id,
        'playbook__selectPlaybook',
        { id: playbookId },
      );

      const toolCallId = createId();
      const [toolCallMsg, toolResultMsg] = createToolMessagePair(
        'playbook__selectPlaybook',
        { id: playbookId },
        result.content ?? [],
        toolCallId,
        session.id,
        undefined,
        session.assistant?.id,
        'ui',
      );

      await injectMessages([toolCallMsg, toolResultMsg], true);
      toast.success(t('agent.chat.playbookStarted', 'Playbook started automatically'));
    } catch (error) {
      logger.error('Failed to auto-select playbook', error);
      toast.error(t('agent.chat.startPlaybookFailed', 'Failed to start playbook workflow'));
    } finally {
      // No-op
    }
  };

  return (
    <>
      <div className="flex h-full w-full overflow-hidden rounded-2xl border border-border/50 bg-background font-sans shadow-[0_18px_48px_-28px_rgba(0,0,0,0.35)]">
        {/* Workspace side panel */}
        {showWorkspacePanel && <AgentWorkspacePanel />}

        {/* Main chat area - components rendered directly inside provider scope */}
        <div className="flex-1 flex flex-col min-h-0 min-w-0 relative">
          <AgentChatHeader />
          <AgentChatStatusBar />
          <AgentChatMessages />

          {/* Floating Input Container */}
          <div className="absolute bottom-0 left-0 right-0 z-10 pointer-events-none">
            <div className="h-24 bg-gradient-to-t from-background/90 via-background/40 to-transparent w-full" />
            <div className="p-4 pt-0">
              <div className="w-full pointer-events-auto">
                <AgentChatAttachedFiles />
                <AgentChatInput />
              </div>
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
  const { t } = useTranslation();
  const { sessionId: routeSessionId } = useParams<{ sessionId?: string }>();
  const { session, isSessionLoading } = useAgentSessionState();
  const attachmentSessionId = session?.id ?? routeSessionId ?? '';

  return (
    <AgentResourceAttachmentProvider sessionId={attachmentSessionId}>
      {isSessionLoading ? (
        <div className="flex h-full items-center justify-center p-4">
          <div className="flex flex-col items-center gap-3">
            <LoadingSpinner
              size="lg"
              className="border-4"
              label={
                session?.status === 'idle'
                  ? t('agent.chat.startingSession', 'Starting session...')
                  : t('agent.chat.loadingSession', 'Loading session...')
              }
            />

            <div className="flex flex-col items-center gap-1">
              <div
                className="text-muted-foreground font-medium animate-pulse"
                aria-hidden="true"
              >
                {session?.status === 'idle'
                  ? t('agent.chat.startingSession', 'Starting session...')
                  : t('agent.chat.loadingSession', 'Loading session...')}
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
            {t('agent.chat.sessionNotFound', 'Session not found or failed to load.')}
          </div>
        </div>
      ) : (
        <AgentChatProvider>
          <AgentPlanningProvider>
            <AgentWorkspaceProvider>
              <AgentChatInner />
            </AgentWorkspaceProvider>
          </AgentPlanningProvider>
        </AgentChatProvider>
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
