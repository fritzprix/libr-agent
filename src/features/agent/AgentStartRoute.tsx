import AgentChatStartView from './AgentChatStartView';

/**
 * Start route for /agent.
 *
 * This exists as its own lazy boundary so the assistant-picker screen does not
 * pay for the heavier session chat route on first load.
 */
export default function AgentStartRoute() {
  return <AgentChatStartView />;
}
