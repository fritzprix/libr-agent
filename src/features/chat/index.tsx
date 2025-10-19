import { useSessionContext } from '@/context/SessionContext';
import Chat from './Chat';

export function ChatRouter() {
  const { current } = useSessionContext();

  if (!current) {
    return null;
  }
  if (current.assistants.length > 1) {
    return (
      <Chat>
        <Chat.Header></Chat.Header>
        <Chat.Messages />
        <Chat.Bottom></Chat.Bottom>
      </Chat>
    );
  }
  return (
    <Chat>
      <Chat.Header />
      <Chat.Messages />
      <Chat.Bottom></Chat.Bottom>
    </Chat>
  );
}
