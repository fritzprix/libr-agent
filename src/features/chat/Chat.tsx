import React from 'react';
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
import ToolsModal from '../tools/ToolsModal';
import { TimeLocationSystemPrompt } from '../prompts/TimeLocationSystemPrompt';
import { getLogger } from '@/lib/logger';

const logger = getLogger('Chat');

interface ChatProps {
  children?: React.ReactNode;
}

function ChatInner({ children }: ChatProps) {
  const { showToolsDetail, setShowToolsDetail } = useChatState();
  const { showPlanningPanel } = useChatPlanning();

  logger.info('CHAT_INNER: Render with planning panel state', {
    showPlanningPanel,
    showToolsDetail,
  });

  return (
    <div className="h-full w-full font-mono flex rounded-lg overflow-hidden shadow-2xl">
      {/* Main chat area */}
      <div className="flex-1 flex flex-col">
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

  if (!currentSession) {
    throw new Error(
      'Chat component should only be rendered when currentSession exists',
    );
  }

  return (
    <ChatProvider>
      <ChatPlanningProvider>
        <TimeLocationSystemPrompt />
        <ChatInner>{children}</ChatInner>
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

export default Chat;
