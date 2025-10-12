import React, { useEffect, useRef } from 'react';
import { ChatProvider } from '@/context/ChatContext';
import { useSessionContext } from '@/context/SessionContext';
import { useChatState } from './hooks/useChatState';
import {
  ChatPlanningProvider,
  useChatPlanning,
} from './context/ChatPlanningContext';
import { ChatHeader } from './components/ChatHeader';
import { ChatMessages } from './components/ChatMessages';
import { ChatStatusBar } from './components/ChatStatusBar';
import { ChatAttachedFiles } from './components/ChatAttachedFiles';
import { ChatInput } from './components/ChatInput';
import { ChatBottom } from './components/ChatBottom';
import { ChatPlanningPanel } from './components/ChatPlanningPanel';
import { WorkspaceFilesPanel } from './components/WorkspaceFilesPanel';
import {
  ChatWorkspaceProvider,
  useChatWorkspace,
} from './context/ChatWorkspaceContext';
import ToolsModal from '../tools/ToolsModal';
import { TimeLocationSystemPrompt } from '../prompts/TimeLocationSystemPrompt';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useWebMCPServer } from '@/hooks/use-web-mcp-server';
import { PlanningServerProxy } from '@/lib/web-mcp/modules/planning-server';
import AgentIdSystemPrompt from '../prompts/AgentIdSystemPrompt';

const logger = getLogger('Chat');

interface ChatProps {
  children?: React.ReactNode;
}

function ChatInner({ children }: ChatProps) {
  const { showToolsDetail, setShowToolsDetail } = useChatState();
  const { showPlanningPanel } = useChatPlanning();
  const { showWorkspacePanel } = useChatWorkspace();

  logger.info('CHAT_INNER: Render with panel states', {
    showPlanningPanel,
    showWorkspacePanel,
    showToolsDetail,
  });

  return (
    // Limit chat container height to viewport so sticky headers and panels behave predictably
    <div className="h-full w-full max-h-[100vh] font-mono flex rounded-lg overflow-hidden shadow-2xl">
      {/* Workspace side panel */}
      {showWorkspacePanel && <WorkspaceFilesPanel />}

      {/* Main chat area */}
      <div className="flex-1 flex flex-col min-h-0">
        {children}
        <ToolsModal
          isOpen={showToolsDetail}
          onClose={() => setShowToolsDetail(false)}
        />
      </div>

      {/* Planning side panel */}
      {showPlanningPanel && <ChatPlanningPanel />}
    </div>
  );
}
function Chat({ children }: ChatProps) {
  const { current: currentSession } = useSessionContext();
  const previousSessionIdRef = useRef<string | null>(null);
  const { server: planningServer } =
    useWebMCPServer<PlanningServerProxy>('planning');

  // 세션 변경 감지 및 planning 상태 초기화
  useEffect(() => {
    const currentSessionId = currentSession?.id || null;
    const previousSessionId = previousSessionIdRef.current;

    // 세션이 변경되었고, planning 서버가 사용 가능할 때 상태 초기화
    if (
      currentSessionId !== previousSessionId &&
      currentSessionId &&
      planningServer
    ) {
      logger.info('Session changed, clearing planning state', {
        from: previousSessionId,
        to: currentSessionId,
      });

      planningServer.clear_session().catch((error: Error) => {
        logger.error('Failed to clear planning session', { error });
        toast.error('플래닝 세션 클리어에 실패했습니다');
      });
    }

    previousSessionIdRef.current = currentSessionId;
  }, [currentSession?.id, planningServer]);

  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }

  return (
    <ChatProvider>
      <ChatPlanningProvider>
        <ChatWorkspaceProvider>
          <TimeLocationSystemPrompt />
          <AgentIdSystemPrompt />
          <ChatInner>{children}</ChatInner>
        </ChatWorkspaceProvider>
      </ChatPlanningProvider>
    </ChatProvider>
  );
}

Chat.Header = ChatHeader;
Chat.Messages = ChatMessages;
Chat.StatusBar = ChatStatusBar;
Chat.AttachedFiles = ChatAttachedFiles;
Chat.Input = ChatInput;
Chat.Bottom = ChatBottom;
Chat.PlanningPanel = ChatPlanningPanel;
Chat.WorkspacePanel = WorkspaceFilesPanel;

export default Chat;
