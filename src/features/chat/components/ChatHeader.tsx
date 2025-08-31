import React from 'react';
import TerminalHeader from '@/components/TerminalHeader';
import { useSessionContext } from '@/context/SessionContext';
import { SessionFilesPopover } from './SessionFilesPopover';

interface ChatHeaderProps {
  children?: React.ReactNode;
  assistantName?: string;
}

export function ChatHeader({ children, assistantName }: ChatHeaderProps) {
  const { current: currentSession } = useSessionContext();

  return (
    <TerminalHeader>
      <div className="flex items-center justify-between w-full">
        <div className="flex items-center">
          {children}
          {assistantName && (
            <span className="ml-2 text-xs text-blue-400">
              [{assistantName}]
            </span>
          )}
        </div>

        {currentSession?.storeId && (
          <SessionFilesPopover storeId={currentSession.storeId} />
        )}
      </div>
    </TerminalHeader>
  );
}
