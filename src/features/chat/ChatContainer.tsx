import { useSessionContext } from '@/context/SessionContext';
import { ChatRouter } from '.';
import StartChatView from './StartChatView';

export default function ChatContainer() {
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    return <StartChatView />;
  }

  return <ChatRouter />;
}
