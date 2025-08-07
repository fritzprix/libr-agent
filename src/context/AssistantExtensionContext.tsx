import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
} from 'react';
import { getLogger } from '@/lib/logger';
import { MCPServerConfig } from '@/lib/mcp-types';
import { LocalService } from './LocalToolContext';

const logger = getLogger('AssistantExtensionContext');

interface SystemPrompt {
  key: string;
  prompt: string;
  priority?: number;
}

interface RemoteService {
  type: 'remote';
  service: MCPServerConfig;
}

interface NativeService {
  type: 'native';
  service: LocalService;
}

export type ServiceExtension = RemoteService | NativeService;

export interface AssistantExtension {
  systemPrompts?: string[];
  services?: ServiceExtension[];
}

interface AssistantExtensionContextValue {
  registerExtension: (key: string, extension: AssistantExtension) => void;
  unregisterExtension: (key: string) => void;
  getActiveExtensions: () => Record<string, AssistantExtension>;
  getExtensionSystemPrompts: () => SystemPrompt[];
  getExtensionServices: () => ServiceExtension[];
}

const AssistantExtensionContext = createContext<AssistantExtensionContextValue | undefined>(
  undefined,
);

interface AssistantExtensionProviderProps {
  children: React.ReactNode;
}

export function AssistantExtensionProvider({ children }: AssistantExtensionProviderProps) {
  const extensionsRef = useRef<Record<string, AssistantExtension>>({});
  const systemPromptsRef = useRef<Record<string, SystemPrompt>>({});

  const registerExtension = useCallback((key: string, extension: AssistantExtension) => {
    extensionsRef.current[key] = extension;
    
    // Register system prompts from extension
    if (extension.systemPrompts) {
      extension.systemPrompts.forEach((prompt, index) => {
        const promptKey = `${key}-prompt-${index}`;
        systemPromptsRef.current[promptKey] = {
          key: promptKey,
          prompt,
          priority: 10, // Extensions get higher priority
        };
      });
    }
    
    logger.info('Assistant extension registered', {
      key,
      systemPromptsCount: extension.systemPrompts?.length ?? 0,
      servicesCount: extension.services?.length ?? 0,
      services: extension.services?.map(s => `${s.type}:${s.service.name}`) ?? [],
    });
  }, []);

  const unregisterExtension = useCallback((key: string) => {
    const extension = extensionsRef.current[key];
    if (extension) {
      // Remove associated system prompts
      if (extension.systemPrompts) {
        extension.systemPrompts.forEach((_, index) => {
          const promptKey = `${key}-prompt-${index}`;
          delete systemPromptsRef.current[promptKey];
        });
      }
      
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

  const getExtensionSystemPrompts = useCallback(() => {
    return Object.values(systemPromptsRef.current)
      .sort((a, b) => (b.priority || 0) - (a.priority || 0));
  }, []);

  const getExtensionServices = useCallback(() => {
    return Object.values(extensionsRef.current)
      .flatMap(extension => extension.services || []);
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

export function useAssistantExtension(): AssistantExtensionContextValue {
  const context = useContext(AssistantExtensionContext);
  if (context === undefined) {
    throw new Error('useAssistantExtension must be used within an AssistantExtensionProvider');
  }
  return context;
}

// Export types for use in other components
export type { RemoteService, NativeService };
