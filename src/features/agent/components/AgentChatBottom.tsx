import { useAgentSessionState } from '@/context/AgentSessionContext';

export function AgentChatBottom() {
  const { currentSession } = useAgentSessionState();

  if (!currentSession) {
    return null;
  }

  return (
    <div className="border-t border-border px-4 py-2 text-xs text-muted-foreground flex items-center justify-between">
      <span>Agent V2 Architecture</span>
      <div className="flex items-center gap-4">
        <span>Session: {currentSession.id.slice(0, 8)}</span>
      </div>
    </div>
  );
}
