import React from 'react';
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
import { ResourceAttachmentProvider } from '@/context/ResourceAttachmentContext';
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

interface AgentChatInnerProps {
  children?: React.ReactNode;
}

function AgentChatInner({ children }: AgentChatInnerProps) {
  const { showWorkspacePanel } = useAgentWorkspace();
  const { showPlanningPanel } = useAgentPlanning();

  logger.info('AGENT_CHAT_INNER: Render with panel states', {
    showPlanningPanel,
    showWorkspacePanel,
  });

  return (
    <div className="h-full w-full max-h-[100vh] font-mono flex rounded-lg overflow-hidden shadow-2xl">
      {/* Workspace side panel */}
      {showWorkspacePanel && <AgentWorkspacePanel />}

      {/* Main chat area */}
      <div className="flex-1 flex flex-col min-h-0 min-w-0">{children}</div>

      {/* Planning side panel */}
      {showPlanningPanel && <AgentPlanningPanel />}
    </div>
  );
}

interface AgentChatViewProps {
  children?: React.ReactNode;
}

export default function AgentChatView({ children }: AgentChatViewProps) {
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
    <ResourceAttachmentProvider>
      <AgentChatProvider>
        <AgentPlanningProvider>
          <AgentWorkspaceProvider>
            <TimeLocationSystemPrompt />
            <AgentChatInner>{children}</AgentChatInner>
          </AgentWorkspaceProvider>
        </AgentPlanningProvider>
      </AgentChatProvider>
    </ResourceAttachmentProvider>
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
