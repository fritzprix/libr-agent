import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react';

export interface AgentFilePreviewTarget {
  path: string;
  name: string;
  size?: number | null;
  sessionId?: string;
}

export interface AgentFilePreviewContextValue {
  previewFile: AgentFilePreviewTarget | null;
  openFilePreview: (file: AgentFilePreviewTarget) => void;
  closeFilePreview: () => void;
}

const AgentFilePreviewContext = createContext<
  AgentFilePreviewContextValue | undefined
>(undefined);

export function AgentFilePreviewProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [previewFile, setPreviewFile] = useState<AgentFilePreviewTarget | null>(
    null,
  );

  const openFilePreview = useCallback((file: AgentFilePreviewTarget) => {
    setPreviewFile(file);
  }, []);

  const closeFilePreview = useCallback(() => {
    setPreviewFile(null);
  }, []);

  const value = useMemo<AgentFilePreviewContextValue>(
    () => ({
      previewFile,
      openFilePreview,
      closeFilePreview,
    }),
    [closeFilePreview, openFilePreview, previewFile],
  );

  return (
    <AgentFilePreviewContext.Provider value={value}>
      {children}
    </AgentFilePreviewContext.Provider>
  );
}

export function useOptionalAgentFilePreview():
  | AgentFilePreviewContextValue
  | undefined {
  return useContext(AgentFilePreviewContext);
}

export function useAgentFilePreview(): AgentFilePreviewContextValue {
  const context = useContext(AgentFilePreviewContext);
  if (!context) {
    throw new Error(
      'useAgentFilePreview must be used within an AgentFilePreviewProvider',
    );
  }
  return context;
}
