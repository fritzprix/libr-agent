import { useSessionContext } from '@/context/SessionContext';
import { ChatRouter } from '.';
import StartSingleChatView from './StartSingleChatView';

export default function SingleChatContainer() {
  const { current: currentSession } = useSessionContext();

  if (!currentSession) {
    return <StartSingleChatView />;
  }

  return <ChatRouter />;
}
