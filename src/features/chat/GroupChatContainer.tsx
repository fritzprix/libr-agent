import { useSessionContext } from '@/context/SessionContext';
import { ChatRouter } from '.';
import StartGroupChatView from './StartGroupChatView';

export default function GroupChatContainer() {
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    return <StartGroupChatView />;
  }

  return <ChatRouter />;
}
