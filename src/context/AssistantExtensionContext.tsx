import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
} from 'react';
import { getLogger } from '@/lib/logger';
import { MCPServerConfig } from '@/lib/mcp-types';

const logger = getLogger('AssistantExtensionContext');

// --- Type Definitions Updated for Dynamic Prompts ---

/**
 * The content of a system prompt, which can be a static string
 * or a function that returns a dynamic string.
 * The function will be evaluated just before calling the LLM.
 */
export type SystemPromptContent = string | (() => string);

/**
 * Represents a single system prompt with evaluated content.
 * The prompt is always a string (functions are evaluated).
 */
interface SystemPrompt {
  key: string;
  prompt: string;
  priority?: number;
}

// --- Service Extension Types (Unchanged) ---

interface RemoteService {
  type: 'remote';
  service: MCPServerConfig;
}

export type ServiceExtension = RemoteService;

// --- AssistantExtension Type Updated ---

/**
 * Defines the structure of an extension that can be registered.
 * It can provide system prompts (static or dynamic) and services.
 */
export interface AssistantExtension {
  systemPrompts?: SystemPromptContent[];
  services?: ServiceExtension[];
}

// --- Context Value Type Definition ---

interface AssistantExtensionContextValue {
  /** Registers a new extension or updates an existing one. */
  registerExtension: (key: string, extension: AssistantExtension) => void;
  /** Unregisters an extension by its key. */
  unregisterExtension: (key: string) => void;
  /** Returns a record of all currently active extensions. */
  getActiveExtensions: () => Record<string, AssistantExtension>;
  /**
   * Returns an array of all system prompts from registered extensions,
   * sorted by priority. Functions are evaluated at call time for dynamic prompts.
   */
  getExtensionSystemPrompts: () => SystemPrompt[];
  /** Returns a flattened array of all services from registered extensions. */
  getExtensionServices: () => ServiceExtension[];
}

const AssistantExtensionContext = createContext<
  AssistantExtensionContextValue | undefined
>(undefined);

// --- Provider Component ---

interface AssistantExtensionProviderProps {
  children: React.ReactNode;
}

export function AssistantExtensionProvider({
  children,
}: AssistantExtensionProviderProps) {
  const extensionsRef = useRef<Record<string, AssistantExtension>>({});

  const registerExtension = useCallback(
    (key: string, extension: AssistantExtension) => {
      extensionsRef.current[key] = extension;

      logger.info('Assistant extension registered', {
        key,
        systemPromptsCount: extension.systemPrompts?.length ?? 0,
        servicesCount: extension.services?.length ?? 0,
        services:
          extension.services?.map((s) => `${s.type}:${s.service.name}`) ?? [],
      });
    },
    [],
  );

  const unregisterExtension = useCallback((key: string) => {
    const extension = extensionsRef.current[key];
    if (extension) {
      delete extensionsRef.current[key];
      logger.info('Assistant extension unregistered', {
        key,
        removedSystemPrompts: extension.systemPrompts?.length ?? 0,
        removedServices: extension.services?.length ?? 0,
      });
    }
  }, []);

  const getActiveExtensions = useCallback(() => {
    return { ...extensionsRef.current };
  }, []);

  const getExtensionSystemPrompts = useCallback((): SystemPrompt[] => {
    const prompts: SystemPrompt[] = [];

    Object.entries(extensionsRef.current).forEach(
      ([extensionKey, extension]) => {
        extension.systemPrompts?.forEach((promptContent, index) => {
          const promptKey = `${extensionKey}-prompt-${index}`;
          let promptValue: string;

          if (typeof promptContent === 'function') {
            try {
              promptValue = promptContent();
            } catch (error) {
              logger.error('System prompt function threw error', {
                error,
                extensionKey,
                promptIndex: index,
              });
              promptValue = '';
            }
          } else {
            promptValue = promptContent;
          }

          prompts.push({
            key: promptKey,
            prompt: promptValue,
            priority: 10, // Default priority for extensions
          });
        });
      },
    );

    return prompts.sort((a, b) => (b.priority || 0) - (a.priority || 0));
  }, []);

  const getExtensionServices = useCallback(() => {
    return Object.values(extensionsRef.current).flatMap(
      (extension) => extension.services || [],
    );
  }, []);

  const value: AssistantExtensionContextValue = useMemo(
    () => ({
      registerExtension,
      unregisterExtension,
      getActiveExtensions,
      getExtensionSystemPrompts,
      getExtensionServices,
    }),
    [
      registerExtension,
      unregisterExtension,
      getActiveExtensions,
      getExtensionSystemPrompts,
      getExtensionServices,
    ],
  );

  return (
    <AssistantExtensionContext.Provider value={value}>
      {children}
    </AssistantExtensionContext.Provider>
  );
}

// --- Custom Hook ---

export function useAssistantExtension(): AssistantExtensionContextValue {
  const context = useContext(AssistantExtensionContext);
  if (context === undefined) {
    throw new Error(
      'useAssistantExtension must be used within an AssistantExtensionProvider',
    );
  }
  return context;
}
