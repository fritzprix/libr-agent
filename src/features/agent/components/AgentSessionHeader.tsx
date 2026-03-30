import React from 'react';
import { useAgentSessionState } from '@/context/AgentSessionContext';

interface AgentSessionHeaderProps {
  children?: React.ReactNode;
}

export default function AgentSessionHeader({
  children,
}: AgentSessionHeaderProps) {
  const { session } = useAgentSessionState();

  // Fallback display if session is not yet loaded
  const sessionName = session?.name || 'Untitled Session';
  const sessionType = 'Agent'; // Fixed type for Agent V2 sessions
  const assistantName = session?.assistant?.name || 'Agent';

  return (
    <div>
      <div className="flex shrink-0 items-center justify-between border-b border-border/40 bg-background px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            Assistant
          </span>
          <span className="truncate text-xs font-medium text-foreground">
            {assistantName}
          </span>
        </div>
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            Session
          </span>
          <span
            className="max-w-xs truncate text-sm font-medium text-foreground/90"
            title={sessionName}
          >
            {sessionName} ({sessionType})
          </span>
        </div>
      </div>

      {children && (
        <div className="shrink-0 border-b border-border/40 bg-background/90 px-4 py-2.5">
          <div className="flex items-center justify-between">
            <div className="flex gap-2"></div>
            {children}
          </div>
        </div>
      )}
    </div>
  );
}
