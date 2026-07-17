import {
  createContext,
  useContext,
  useState,
  type ReactNode,
  useCallback,
} from 'react';

interface AgentScratchpadContextType {
  showScratchpadPanel: boolean;
  toggleScratchpadPanel: () => void;
  openScratchpadPanel: () => void;
  closeScratchpadPanel: () => void;
}

const AgentScratchpadContext = createContext<
  AgentScratchpadContextType | undefined
>(undefined);

export function AgentScratchpadProvider({ children }: { children: ReactNode }) {
  const [showScratchpadPanel, setShowScratchpadPanel] = useState(false);

  const toggleScratchpadPanel = useCallback(() => {
    setShowScratchpadPanel((prev) => !prev);
  }, []);

  const openScratchpadPanel = useCallback(() => {
    setShowScratchpadPanel(true);
  }, []);

  const closeScratchpadPanel = useCallback(() => {
    setShowScratchpadPanel(false);
  }, []);

  return (
    <AgentScratchpadContext.Provider
      value={{
        showScratchpadPanel,
        toggleScratchpadPanel,
        openScratchpadPanel,
        closeScratchpadPanel,
      }}
    >
      {children}
    </AgentScratchpadContext.Provider>
  );
}

export function useAgentScratchpad() {
  const context = useContext(AgentScratchpadContext);
  if (!context) {
    throw new Error(
      'useAgentScratchpad must be used within an AgentScratchpadProvider',
    );
  }
  return context;
}
