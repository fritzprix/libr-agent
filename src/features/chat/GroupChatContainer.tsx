
import { Switch } from '@/components/ui/switch';
import { useSessionContext } from '@/context/SessionContext';
import Chat from './Chat';
import StartGroupChatView from './StartGroupChatView';
import { MultiAgentOrchestrator } from './orchestrators/MultiAgentOrchestrator';
import { ChatRouter } from '.';


export default function GroupChatContainer() {
  const { current: currentSession, isAgenticMode, toggleAgenticMode } = useSessionContext();

  if (!currentSession) {
    return (
      <StartGroupChatView />
    );
  }

    return <ChatRouter/>;
  
}
