import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';
import { createId } from '@paralleldrive/cuid2';
import { getLogger } from '@/lib/logger';

const logger = getLogger('SystemPromptProvider');

type SystemPrompt = string | (() => string) | (() => Promise<string>);

interface SystemPromptExtension {
  id: string;
  key: string;
  content: SystemPrompt;
  priority: number;
}

interface SystemPromptContextType {
  // register returns a generated id; unregister removes by that id (Option A)
  register(key: string, prompt: SystemPrompt, priority?: number): string;
  unregister(id: string): void;
  getSystemPrompt(): Promise<string>;
}

const SystemPromptContext = createContext<SystemPromptContextType | null>(null);

interface SystemPromptProviderProps {
  children: React.ReactNode;
}

export function SystemPromptProvider({ children }: SystemPromptProviderProps) {
  const [extensions, setExtensions] = useState<SystemPromptExtension[]>([]);

  const register = useCallback(
    (key: string, prompt: SystemPrompt, priority: number = 0): string => {
      const id = createId();
      const extension: SystemPromptExtension = {
        id,
        key,
        content: prompt,
        priority,
      };

      setExtensions((prev) => {
        // Remove existing extension with same key
        const filtered = prev.filter((ext) => ext.key !== key);
        const updated = [...filtered, extension];
        // Sort by priority (higher priority first)
        return updated.sort((a, b) => b.priority - a.priority);
      });

      logger.debug('Registered system prompt extension', {
        id,
        key,
        priority,
      });
      return id;
    },
    [],
  );

  const unregister = useCallback((id: string) => {
    setExtensions((prev) => prev.filter((ext) => ext.id !== id));
    logger.debug('Unregistered system prompt extension', { id });
  }, []);

  const getSystemPrompt = useCallback(async (): Promise<string> => {
    const prompts: string[] = [];

    for (const extension of extensions) {
      try {
        let content: string;
        if (typeof extension.content === 'function') {
          content = await extension.content();
        } else {
          content = extension.content;
        }

        if (content && content.trim()) {
          prompts.push(content);
        }
      } catch (error) {
        logger.error('Failed to resolve system prompt extension', {
          key: extension.key,
          error: error instanceof Error ? error.message : String(error),
        });
      }
    }

    return prompts.join('\n\n');
  }, [extensions]);

  const contextValue = useMemo(
    () => ({
      register,
      unregister,
      getSystemPrompt,
    }),
    [register, unregister, getSystemPrompt],
  );

  return (
    <SystemPromptContext.Provider value={contextValue}>
      {children}
    </SystemPromptContext.Provider>
  );
}

export function useSystemPrompt() {
  const context = useContext(SystemPromptContext);
  if (!context) {
    throw new Error('useSystemPrompt must be used within SystemPromptProvider');
  }
  return context;
}
