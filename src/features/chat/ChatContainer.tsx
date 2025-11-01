import { useSessionContext } from '@/context/SessionContext';
import { ChatRouter } from '.';
import StartChatView from './StartChatView';
import { WebMCPProvider } from '@/context/WebMCPContext';
import { WebMCPServiceRegistry } from '@/features/tools/WebMCPServiceRegistry';
import { BrowserToolProvider } from '@/features/tools/BrowserToolProvider';
import { RustMCPToolProvider } from '@/features/tools/RustMCPToolProvider';

export default function ChatContainer() {
  const { current: currentSession } = useSessionContext();

  return (
    <WebMCPProvider>
      <WebMCPServiceRegistry servers={['planning', 'playbook', 'ui']} />
      <BrowserToolProvider />
      <RustMCPToolProvider />
      {currentSession ? <ChatRouter /> : <StartChatView />}
    </WebMCPProvider>
  );
}
