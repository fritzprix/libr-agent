import React from 'react';
import { useOptionalAgentSessionState } from '@/context/AgentSessionContext';
import { cn } from '@/lib/utils';

interface AgentSessionHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
  sessionName?: string;
  sessionType?: string;
  assistantNameClassName?: string;
  sessionNameClassName?: string;
}

export default function AgentSessionHeader({
  children,
  assistantName,
  sessionName,
  sessionType = 'Agent',
  assistantNameClassName,
  sessionNameClassName,
}: AgentSessionHeaderProps) {
  const optionalSessionState = useOptionalAgentSessionState();
  const session = optionalSessionState?.session;

  // Fallback display if session is not yet loaded
  const resolvedSessionName =
    sessionName ?? session?.name ?? 'Untitled Session';
  const resolvedAssistantName =
    assistantName ?? session?.assistant?.name ?? 'Agent';

  return (
    <div>
      <div className="flex shrink-0 items-center justify-between border-b border-border/40 bg-background px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            Assistant
          </span>
          <span
            className={cn(
              'truncate text-xs font-medium text-foreground',
              assistantNameClassName,
            )}
          >
            {resolvedAssistantName}
          </span>
        </div>
        <div className="flex min-w-0 items-center gap-2">
          <span className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
            Session
          </span>
          <span
            className={cn(
              'max-w-xs truncate text-sm font-medium text-foreground/90',
              sessionNameClassName,
            )}
            title={resolvedSessionName}
          >
            {resolvedSessionName} ({sessionType})
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
