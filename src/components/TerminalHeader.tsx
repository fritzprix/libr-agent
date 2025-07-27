import { useSessionContext } from '@/context/SessionContext';
import { ReactNode } from 'react';

interface TerminalHeaderProps {
  children?: ReactNode;
}

export default function TerminalHeader({ children }: TerminalHeaderProps) {
  const { current: currentSession } = useSessionContext();

  const assistants = currentSession?.assistants || [];
  let assistantDisplay = 'None';
  if (assistants.length === 1) {
    assistantDisplay = assistants[0].name;
  } else if (assistants.length > 1) {
    assistantDisplay = `${assistants[0].name} + ${assistants.length - 1}`;
  }

  return (
    <div>
      <div className="px-4 py-3 flex items-center justify-between border-b flex-shrink-0">
        <div className="flex items-center gap-2">
          <span className="text-xs">Assistant:</span>
          <span className="text-xs">{assistantDisplay}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs">Session:</span>
          <span className="text-sm">
            {currentSession?.name} ({currentSession?.type})
          </span>
        </div>
      </div>

      {children && (
        <div className="px-4 py-2 border-b  flex-shrink-0">
          <div className="flex justify-between items-center">
            <div className="flex gap-2"></div>
            {children}
          </div>
        </div>
      )}
    </div>
  );
}
