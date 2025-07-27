import { useSessionContext } from '@/context/SessionContext';
import { createContext, ReactNode, useContext, useState } from 'react';
import { CompactModelPicker } from './ui';

interface TerminalHeaderContextType {
  isAgentMode: boolean;
  toggleMode: () => void;
}

const TerminalHeaderContext = createContext<
  TerminalHeaderContextType | undefined
>(undefined);

export const useTerminalHeaderContext = () => {
  const context = useContext(TerminalHeaderContext);
  if (!context) {
    throw new Error(
      'useTerminalHeaderContext must be used within a TerminalHeaderProvider',
    );
  }
  return context;
};

interface TerminalHeaderProps {
  children?: ReactNode;
}

export default function TerminalHeader({ children }: TerminalHeaderProps) {
  const { current: currentSession } = useSessionContext();
  const [isAgentMode, setIsAgentMode] = useState(true); // Default to agent mode as per plan

  const toggleMode = () => {
    setIsAgentMode((prev) => !prev);
  };

  return (
    <TerminalHeaderContext.Provider value={{ isAgentMode, toggleMode }}>
      {/* Terminal Header */}
      <div className="px-4 py-3 flex items-center justify-between border-b flex-shrink-0">
        <div className="flex items-center gap-4"></div>
        <div className="flex items-center gap-2">
          <span className="text-xs">Session:</span>
          <span className="text-sm">
            {currentSession?.name} ({currentSession?.type})
          </span>
        </div>
      </div>

      {/* Mode Switcher */}
      <div className="px-4 py-2 border-b  flex-shrink-0">
        <div className="flex justify-between items-center">
          <div className="flex gap-2"></div>
          {children}
        </div>
      </div>

      {/* Model Picker */}
      <div className="px-4 py-3 border-b flex-shrink-0">
        <CompactModelPicker />
      </div>
    </TerminalHeaderContext.Provider>
  );
}
