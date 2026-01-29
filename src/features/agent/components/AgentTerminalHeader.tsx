import React from 'react';
import { useAgentSessionState } from '@/context/AgentSessionContext';

interface AgentTerminalHeaderProps {
  children?: React.ReactNode;
}

export default function AgentTerminalHeader({
  children,
}: AgentTerminalHeaderProps) {
  const { session } = useAgentSessionState();

  // Fallback display if session is not yet loaded
  const sessionName = session?.name || 'Untitled Session';
  const sessionType = 'Agent'; // Fixed type for Agent V2 sessions
  const assistantName = session?.assistant?.name || 'Agent';

  return (
    <div>
      <div className="px-4 py-3 flex items-center justify-between border-b flex-shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-xs">Assistant:</span>
          <span className="text-xs">{assistantName}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs">Session:</span>
          <span className="text-sm truncate max-w-xs" title={sessionName}>
            {sessionName} ({sessionType})
          </span>
        </div>
      </div>

      {children && (
        <div className="px-4 py-2 border-b flex-shrink-0">
          <div className="flex justify-between items-center">
            <div className="flex gap-2"></div>
            {children}
          </div>
        </div>
      )}
    </div>
  );
}
