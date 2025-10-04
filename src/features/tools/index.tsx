import { getLogger } from '@/lib/logger';
import { switchSession } from '@/lib/rust-backend-client';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { toValidJsName } from '@/lib/utils';
import { ToolCall } from '@/models/chat';
import { useSystemPrompt } from '@/context/SystemPromptContext';
import { useSessionContext } from '@/context/SessionContext';
import { useAssistantContext } from '@/context/AssistantContext';
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
  assistantId?: string;
  // 확장 가능: userId?: string; env?: Record<string,string>
}

export interface ServiceContext<T = unknown> {
  contextPrompt: string;
  structuredState?: T;
}

export interface ServiceMetadata {
  displayName: string;
  description: string;
  category?: 'automation' | 'storage' | 'planning' | 'execution';
  icon?: string;
}

export interface BuiltInService {
  metadata: ServiceMetadata;
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;

  // BREAKING CHANGE: 모든 서비스는 반드시 구조화된 ServiceContext를 반환해야 함
  getServiceContext: (
    options?: ServiceContextOptions,
  ) => Promise<ServiceContext<unknown>>;

  // BREAKING CHANGE: 세션/어시스턴트 전환 시 서비스가 내부 정리/프리로드를 수행하도록 구현을 요구
  switchContext: (options?: ServiceContextOptions) => Promise<void>;
}

const BUILTIN_PREFIX = 'builtin_';

type ServiceStatus = 'loading' | 'error' | 'ready';

interface ServiceEntry {
  service: BuiltInService;
  status: ServiceStatus;
}

interface BuiltInToolContextType {
  register: (serviceId: string, service: BuiltInService) => void;
  unregister: (serviceId: string) => void;
  availableTools: MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse<unknown>>;
  buildToolPrompt: () => Promise<string>; // 이름 변경 고려
  status: Record<string, ServiceStatus>;
  getServiceMetadata: (alias: string) => ServiceMetadata | null;
  serviceContexts: Record<string, unknown>;
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
  const { getCurrentSession, current: currentSession } = useSessionContext();
  const { currentAssistant } = useAssistantContext();
  // Simplified state: A single map holds the service and its status.
  const [serviceEntries, setServiceEntries] = useState<
    Map<string, ServiceEntry>
  >(new Map());

  const [serviceContexts, setServiceContexts] = useState<
    Record<string, unknown>
  >({});

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
      strippedToolName = toolcall.function.name.slice(BUILTIN_PREFIX.length);
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
    const newServiceContexts: Record<string, unknown> = {};

    // 1. Built-in Tools Section
    const availableToolsCount = availableTools.length;
    const isLoadingTauriTools = false; // No longer tracked in new API

    prompts.push(`# Available Built-in Tools

You have access to built-in tools for file operations, code execution, and web-based processing.
Tool details and usage instructions are provided separately.

**Available Built-In Tools:** ${availableToolsCount} ${isLoadingTauriTools ? '(Loading...)' : ''}

**Important Instruction:** When calling built-in tools, you MUST use the tool name exactly as it appears in the available tools list. Do not add or remove the "${BUILTIN_PREFIX}" prefix - use it "as is" (e.g., if the tool name is "${BUILTIN_PREFIX}file_read", call it as "${BUILTIN_PREFIX}file_read", not "file_read" or "${BUILTIN_PREFIX}${BUILTIN_PREFIX}file_read").
`);

    // 2. Service Contexts Section
    const currentSession = getCurrentSession();
    const contextOptions: ServiceContextOptions = {
      sessionId: currentSession?.id,
      assistantId: currentAssistant?.id,
    };

    // Collect MCP server status information
    const mcpServerStatuses: string[] = [];

    // Skip service contexts if no valid session is available
    if (currentSession?.id) {
      for (const [serviceId, entry] of serviceEntries.entries()) {
        if (entry.status === 'ready') {
          try {
            const result =
              await entry.service.getServiceContext(contextOptions);

            // Check if this is an MCP server (Rust-based) by serviceId pattern
            if (
              serviceId !== 'browser' &&
              serviceId !== 'planning' &&
              serviceId !== 'playbook'
            ) {
              // Collect MCP server status for unified section
              if (result.contextPrompt) {
                mcpServerStatuses.push(result.contextPrompt);
              }
            } else {
              // Non-MCP services (browser, planning, playbook) keep individual sections
              if (result.contextPrompt) {
                prompts.push(result.contextPrompt);
              }
            }

            if (result.structuredState !== undefined) {
              newServiceContexts[serviceId] = result.structuredState;
            }
          } catch (err) {
            logger.error('Failed to get service context', {
              serviceId,
              sessionId: contextOptions.sessionId,
              err,
            });
          }
        }
      }

      // Add unified MCP servers status section
      if (mcpServerStatuses.length > 0) {
        prompts.push(`# MCP Servers Status\n${mcpServerStatuses.join('\n')}`);
      }
    } else {
      logger.debug('Skipping service contexts - no valid session available', {
        sessionExists: !!currentSession,
        sessionId: currentSession?.id,
      });
    }

    setServiceContexts((prev) => ({ ...prev, ...newServiceContexts }));
    logger.info('Built tool prompt with service contexts', {
      promptsCount: prompts.length,
      totalLength: prompts.join('\n\n').length,
    });
    return prompts.join('\n\n');
  }, [
    serviceEntries,
    availableTools.length,
    getCurrentSession,
    currentAssistant,
  ]);

  // 세션/어시스턴트 변경 시 switchContext를 호출하여 서비스가 내부 정리/프리로드를 수행하도록 함
  useEffect(() => {
    const sessionId = currentSession?.id;
    const assistantId = currentAssistant?.id;

    // Session backend management: switch to the new session
    if (sessionId) {
      switchSession(sessionId, true).catch((error) => {
        logger.error('Failed to switch session in backend', {
          sessionId,
          error,
        });
      });
    }

    const readyServices = Array.from(serviceEntriesRef.current.values()).filter(
      (e) => e.status === 'ready',
    );

    if (!readyServices.length) {
      return;
    }

    Promise.allSettled(
      readyServices.map((entry) =>
        entry.service.switchContext({ sessionId, assistantId }),
      ),
    ).then((results) => {
      const failureCount = results.filter(
        (r) => r.status === 'rejected',
      ).length;

      if (failureCount > 0) {
        logger.warn('switchContext completed with failures', {
          total: results.length,
          failed: failureCount,
        });

        results.forEach((r, i) => {
          if (r.status === 'rejected') {
            const svc = readyServices[i].service;
            logger.error('switchContext failed for service', {
              serviceId: svc.metadata.displayName,
              error: (r as PromiseRejectedResult).reason,
            });
          }
        });
      }
    });
  }, [
    currentSession?.id,
    currentAssistant?.id,
    Array.from(serviceEntries.keys()),
  ]);

  // Get service metadata by alias
  const getServiceMetadata = useCallback(
    (alias: string): ServiceMetadata | null => {
      const serviceId = aliasToIdTableRef.current.get(alias);
      if (!serviceId) return null;

      const entry = serviceEntriesRef.current.get(serviceId);
      if (!entry) return null;

      return entry.service.metadata;
    },
    [],
  );

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
      getServiceMetadata,
      serviceContexts,
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
      getServiceMetadata,
      serviceContexts,
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
