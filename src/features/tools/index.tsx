import { getLogger } from '@/lib/logger';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { toValidJsName } from '@/lib/utils';
import { ToolCall } from '@/models/chat';
import { useSystemPrompt } from '@/context/SystemPromptContext';
import { useSessionContext } from '@/context/SessionContext';
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';

const logger = getLogger('BuiltInToolProvider');

// --- Interfaces and Types ---

export interface ServiceContextOptions {
  sessionId?: string;
  // 향후 확장 가능한 다른 옵션들
  // storeId?: string;
  // userId?: string;
  // scope?: 'session' | 'global' | 'user';
}

export interface BuiltInService {
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
  getServiceContext?: (options?: ServiceContextOptions) => Promise<string>;
}

const BUILTIN_PREFIX = 'builtin.';

type ServiceStatus = 'loading' | 'error' | 'ready';

interface ServiceEntry {
  service: BuiltInService;
  status: ServiceStatus;
}

interface BuiltInToolContextType {
  register: (serviceId: string, service: BuiltInService) => void;
  unregister: (serviceId: string) => void;
  availableTools: MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse>;
  buildToolPrompt: () => Promise<string>; // 이름 변경 고려
  status: Record<string, ServiceStatus>;
}

interface BuiltInToolProviderProps {
  children: ReactNode;
}

// --- React Context ---

const BuiltInToolContext = createContext<BuiltInToolContextType | null>(null);

// --- Provider Component ---

export function BuiltInToolProvider({ children }: BuiltInToolProviderProps) {
  const { register: registerPrompt, unregister: unregisterPrompt } =
    useSystemPrompt();
  const { getCurrentSession } = useSessionContext();
  // Simplified state: A single map holds the service and its status.
  const [serviceEntries, setServiceEntries] = useState<
    Map<string, ServiceEntry>
  >(new Map());

  // Refs are used for stable callbacks that don't need to change on re-renders.
  const serviceEntriesRef = useRef(serviceEntries);
  const aliasToIdTableRef = useRef<Map<string, string>>(new Map());

  // Keep the ref updated with the latest state.
  useEffect(() => {
    serviceEntriesRef.current = serviceEntries;
  }, [serviceEntries]);

  // Memoized calculation of available tools, derived from the single state source.
  const availableTools: MCPTool[] = useMemo(() => {
    const tools: MCPTool[] = [];
    for (const [serviceId, entry] of serviceEntries.entries()) {
      // Only list tools from services that are fully 'ready'.
      if (entry.status === 'ready') {
        const alias = toValidJsName(serviceId);
        tools.push(
          ...entry.service.listTools().map(
            (t) =>
              ({
                ...t,
                name: `${BUILTIN_PREFIX}${alias}__${t.name}`,
              }) satisfies MCPTool,
          ),
        );
      }
    }
    return tools;
  }, [serviceEntries]);

  const register = useCallback((serviceId: string, service: BuiltInService) => {
    const alias = toValidJsName(serviceId);
    aliasToIdTableRef.current.set(alias, serviceId);

    // Set initial status to 'loading'.
    setServiceEntries((prev) => {
      const next = new Map(prev);
      next.set(serviceId, { service, status: 'loading' });
      return next;
    });

    const loadPromise = service.loadService
      ? service.loadService()
      : Promise.resolve();

    loadPromise
      .then(() => {
        setServiceEntries((prev) => {
          const next = new Map(prev);
          // On success, update status to 'ready'.
          next.set(serviceId, { service, status: 'ready' });
          logger.debug('service registered and ready', { serviceId });
          return next;
        });
      })
      .catch((err) => {
        logger.error('service load failed', { serviceId, err });
        setServiceEntries((prev) => {
          const next = new Map(prev);
          // On failure, update status to 'error'.
          next.set(serviceId, { service, status: 'error' });
          return next;
        });
      });
  }, []);

  const unregister = useCallback((serviceId: string) => {
    const entryToRemove = serviceEntriesRef.current.get(serviceId);
    if (!entryToRemove) {
      logger.warn('unregister called for unknown service', { serviceId });
      return;
    }

    const alias = toValidJsName(serviceId);
    aliasToIdTableRef.current.delete(alias);

    // This function removes the service from state, to be called after unload attempt.
    const removeFromState = () => {
      setServiceEntries((prev) => {
        const next = new Map(prev);
        const removed = next.delete(serviceId);
        logger.debug('service removed from registry', { serviceId, removed });
        return next;
      });
    };

    const unloadPromise = entryToRemove.service.unloadService
      ? entryToRemove.service.unloadService()
      : Promise.resolve();

    unloadPromise
      .then(() => {
        logger.debug('service unloaded successfully', { serviceId });
      })
      .catch((err) => {
        // If unload fails, log the error but still proceed to remove the service from the registry.
        logger.error('service unload failed, removing anyway', {
          serviceId,
          err,
        });
      })
      .finally(() => {
        // Always remove the service from the state after attempting to unload.
        removeFromState();
      });
  }, []);

  const executeTool = useCallback(async (toolcall: ToolCall) => {
    let strippedToolName;
    if (!toolcall.function.name.startsWith(BUILTIN_PREFIX)) {
      strippedToolName = toolcall.function.name;
      logger.warn('tool call does not have builtin prefix', { toolcall });
    } else {
      strippedToolName = toolcall.function.name.replace(BUILTIN_PREFIX, '');
    }

    const idx = strippedToolName.indexOf('__');
    if (idx === -1) {
      throw new Error(`Invalid builtin tool name format: ${strippedToolName}`);
    }

    // Improved error handling for more specific feedback.
    const alias = strippedToolName.slice(0, idx);
    const serviceId = aliasToIdTableRef.current.get(alias);
    if (!serviceId) {
      throw new Error(`No service registered for alias: "${alias}"`);
    }

    const entry = serviceEntriesRef.current.get(serviceId);
    if (!entry) {
      // This is a safeguard for state consistency.
      throw new Error(`Service with ID "${serviceId}" not found in registry.`);
    }

    if (entry.status !== 'ready') {
      throw new Error(
        `Service "${serviceId}" is not ready. Current status: ${entry.status}`,
      );
    }

    const toolName = strippedToolName.slice(idx + 2);
    return entry.service.executeTool({
      ...toolcall,
      function: {
        ...toolcall.function,
        name: toolName,
      },
    });
  }, []);

  const buildToolPrompt = useCallback(async (): Promise<string> => {
    const prompts: string[] = [];

    // 1. Built-in Tools Section
    const availableToolsCount = availableTools.length;
    const isLoadingTauriTools = false; // No longer tracked in new API

    prompts.push(`# Available Built-in Tools

You have access to built-in tools for file operations, code execution, and web-based processing.
Tool details and usage instructions are provided separately.

**Available Built-In Tools:** ${availableToolsCount} ${isLoadingTauriTools ? '(Loading...)' : ''}

**Important Instruction:** When calling built-in tools, you MUST use the tool name exactly as it appears in the available tools list. Do not add or remove the "builtin." prefix - use it "as is" (e.g., if the tool name is "builtin.file_read", call it as "builtin.file_read", not "file_read" or "builtin.builtin.file_read").
`);

    // 2. Service Contexts Section
    const currentSession = getCurrentSession();
    const contextOptions: ServiceContextOptions = {
      sessionId: currentSession?.id,
    };

    // Skip service contexts if no valid session is available
    if (currentSession?.id) {
      for (const [serviceId, entry] of serviceEntries.entries()) {
        if (entry.status === 'ready' && entry.service.getServiceContext) {
          try {
            const prompt =
              await entry.service.getServiceContext(contextOptions);
            if (prompt) {
              prompts.push(prompt);
            }
          } catch (err) {
            logger.error('Failed to get service context from service', {
              serviceId,
              sessionId: contextOptions.sessionId,
              err,
            });
          }
        }
      }
    } else {
      logger.debug('Skipping service contexts - no valid session available', {
        sessionExists: !!currentSession,
        sessionId: currentSession?.id,
      });
    }
    logger.info('service prompts : ', { prompts });
    return prompts.join('\n\n');
  }, [serviceEntries, availableTools.length, getCurrentSession]);

  // Register system prompt when component mounts
  useEffect(() => {
    const promptId = registerPrompt('builtin-tools', buildToolPrompt, 1);
    return () => {
      unregisterPrompt(promptId);
    };
  }, [buildToolPrompt, registerPrompt, unregisterPrompt]);

  // Memoize the context value to prevent unnecessary re-renders of consumers.
  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools,
      unregister,
      register,
      executeTool,
      buildToolPrompt,
      // Derive the public status object from the serviceEntries state.
      status: Object.fromEntries(
        Array.from(serviceEntries.entries()).map(([id, entry]) => [
          id,
          entry.status,
        ]),
      ),
    }),
    [
      availableTools,
      unregister,
      register,
      executeTool,
      buildToolPrompt,
      serviceEntries,
    ],
  );

  logger.info('BuiltInToolProvider context updated', {
    status: contextValue.status,
  });

  return (
    <BuiltInToolContext.Provider value={contextValue}>
      {children}
    </BuiltInToolContext.Provider>
  );
}

// --- Custom Hook ---

export function useBuiltInTool() {
  const context = useContext(BuiltInToolContext);
  if (context === null) {
    throw new Error('useBuiltInTool must be used within a BuiltInToolProvider');
  }
  return context;
}
