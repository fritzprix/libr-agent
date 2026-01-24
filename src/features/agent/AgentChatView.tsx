import { useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { agentCallBuiltinTool } from '@/lib/backend/agent-commands';
import { createId } from '@paralleldrive/cuid2';
import { createToolMessagePair } from '@/lib/chat-utils';
import { MCPContent } from '@/lib/mcp-types';
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
import { TimeLocationSystemPrompt } from '@/features/prompts/TimeLocationSystemPrompt';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentChatView');

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
        'builtin_playbook__selectPlaybook',
        { id: playbookId },
      );

      const toolCallId = createId();
      const [toolCallMsg, toolResultMsg] = createToolMessagePair(
        'builtin_playbook__selectPlaybook',
        { id: playbookId },
        result.content ?? [],
        toolCallId,
        session.id,
        undefined,
        session.assistant?.id,
        'ui',
      );

      await injectMessages([toolCallMsg, toolResultMsg], true);
      toast.success('Playbook started automatically');
    } catch (error) {
      logger.error('Failed to auto-select playbook', error);
      toast.error('Failed to start playbook workflow');
    } finally {
      // No-op
    }
  };

  logger.info('AGENT_CHAT_INNER: Render with panel states', {
    showPlanningPanel,
    showWorkspacePanel,
  });

  return (
    <>
      <TimeLocationSystemPrompt />
      <div className="h-full w-full max-h-[100vh] font-mono flex rounded-lg overflow-hidden shadow-2xl">
        {/* Workspace side panel */}
        {showWorkspacePanel && <AgentWorkspacePanel />}

        {/* Main chat area - components rendered directly inside provider scope */}
        <div className="flex-1 flex flex-col min-h-0 min-w-0">
          <AgentChatHeader />
          <AgentChatStatusBar />
          <AgentChatMessages />
          <AgentChatAttachedFiles />
          <AgentChatInput />
        </div>

        {/* Planning side panel */}
        {showPlanningPanel && <AgentPlanningPanel />}
      </div>
    </>
  );
}

export default function AgentChatView() {
  const { session, isSessionLoading } = useAgentSessionState();

  if (isSessionLoading) {
    return (
      <div className="flex h-full items-center justify-center p-4">
        {/* Placeholder for loading state, could be a spinner */}
        <div className="text-muted-foreground animate-pulse">
          Loading session...
        </div>
      </div>
    );
  }

  if (!session) {
    return (
      <div className="flex h-full items-center justify-center p-4">
        <div className="text-destructive">
          Session not found or failed to load.
        </div>
      </div>
    );
  }

  return (
    <AgentResourceAttachmentProvider>
      <AgentChatProvider>
        <AgentPlanningProvider>
          <AgentWorkspaceProvider>
            <AgentChatInner />
          </AgentWorkspaceProvider>
        </AgentPlanningProvider>
      </AgentChatProvider>
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
