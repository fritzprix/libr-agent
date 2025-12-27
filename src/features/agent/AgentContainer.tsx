import { useAgentSessionState } from '@/context/AgentSessionContext';
import AgentChatView from './AgentChatView';
import StartAgentView from './StartAgentView';

/**
 * Agent V2 Container
 *
 * Smart container that routes between start view and chat view
 * based on agent session state.
 *
 * Pattern: Mirrors V1's ChatContainer.tsx for consistency
 */
export default function AgentContainer() {
  const { currentSession } = useAgentSessionState();

  return currentSession ? <AgentChatView /> : <StartAgentView />;
}
