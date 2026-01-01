import { useParams } from 'react-router-dom';
import { AgentSessionProvider } from '@/context/AgentSessionContext';
import AgentChatView from './AgentChatView';
import AgentChatStartView from './AgentChatStartView';

/**
 * Agent V2 Container
 *
 * Parallel Session Support:
 * - /agent → AgentChatStartView (Global List Context)
 * - /agent/:sessionId → AgentChatView (Local Session Context)
 *
 * Each session has its own AgentSessionProvider instance.
 */
export default function AgentContainer() {
  const { sessionId } = useParams<{ sessionId?: string }>();

  if (sessionId) {
    // Session View: Isolated provider for this session
    return (
      <AgentSessionProvider sessionId={sessionId} key={sessionId}>
        <AgentChatView>
          <AgentChatView.Header />
          <AgentChatView.StatusBar />
          <AgentChatView.Messages />
          <AgentChatView.AttachedFiles />
          <AgentChatView.Input />
          <AgentChatView.Bottom />
        </AgentChatView>
      </AgentSessionProvider>
    );
  }

  // Start View: Uses global list context
  return <AgentChatStartView />;
}
