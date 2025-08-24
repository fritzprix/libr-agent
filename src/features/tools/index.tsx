import { getLogger } from '@/lib/logger';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { ToolCall } from '@/models/chat';
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

export interface BuiltInService {
  listTools: () => MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse>;
  loadService?: () => Promise<void>;
  unloadService?: () => Promise<void>;
}

const BUILTIN_PREFIX = 'builtin.';

type ServiceStatus = 'loading' | 'error' | 'ready';

interface BuiltInToolContextType {
  register: (serviceId: string, service: BuiltInService) => void;
  unregister: (serviceId: string) => void;
  availableTools: MCPTool[];
  executeTool: (toolCall: ToolCall) => Promise<MCPResponse>;
  status: Record<string, ServiceStatus>;
}

interface BuiltInToolProviderProps {
  children: ReactNode;
}

const BuiltInToolContext = createContext<BuiltInToolContextType | null>(null);

export function BuiltInToolProvider({ children }: BuiltInToolProviderProps) {
  const [services, setServices] = useState<Map<string, BuiltInService>>(
    new Map(),
  );
  const [status, setStatus] = useState<Map<string, ServiceStatus>>(new Map());
  const servicesRef = useRef<Map<string, BuiltInService>>(new Map());

  useEffect(() => {
    servicesRef.current = services;
  }, [services]);

  const availableTools: MCPTool[] = useMemo(() => {
    const tools: MCPTool[] = [];
    for (const [serviceId, service] of services.entries()) {
      tools.push(
        ...service.listTools().map(
          (t) =>
            ({
              ...t,
              name: `${BUILTIN_PREFIX}${serviceId}__${t.name}`,
            }) satisfies MCPTool,
        ),
      );
    }
    return tools;
  }, [services]);

  const register = useCallback((serviceId: string, service: BuiltInService) => {
    // set loading status
    setStatus((prev) => {
      const next = new Map(prev);
      next.set(serviceId, 'loading');
      return next;
    });

    if (service.loadService) {
      service
        .loadService()
        .then(() => {
          setServices((prev) => {
            const next = new Map(prev);
            next.set(serviceId, service);
            logger.debug('service registered', { serviceId });
            return next;
          });
          setStatus((prev) => {
            const next = new Map(prev);
            next.set(serviceId, 'ready');
            return next;
          });
          logger.debug('service loaded', { serviceId });
        })
        .catch((err) => {
          logger.error('service load failed', { serviceId, err });
          setStatus((prev) => {
            const next = new Map(prev);
            next.set(serviceId, 'error');
            return next;
          });
        });
    } else {
      // immediate register
      setServices((prev) => {
        const next = new Map(prev);
        next.set(serviceId, service);
        logger.debug('service registered', { serviceId });
        return next;
      });
      setStatus((prev) => {
        const next = new Map(prev);
        next.set(serviceId, 'ready');
        return next;
      });
    }
  }, []);

  const unregister = useCallback((serviceId: string) => {
    const serviceToRemove = servicesRef.current.get(serviceId);
    if (!serviceToRemove) {
      logger.warn('unregister called for unknown service', { serviceId });
      return;
    }

    const removeFromState = () => {
      setServices((prev) => {
        const next = new Map(prev);
        const removed = next.delete(serviceId);
        logger.debug('service removed from registry', { serviceId, removed });
        return next;
      });
      setStatus((prev) => {
        const next = new Map(prev);
        next.delete(serviceId);
        return next;
      });
    };

    if (serviceToRemove.unloadService) {
      serviceToRemove
        .unloadService()
        .then(() => {
          removeFromState();
          logger.debug('service unloaded and removed', { serviceId });
        })
        .catch((err) => {
          logger.error('service unload failed', { serviceId, err });
          setStatus((prev) => {
            const next = new Map(prev);
            next.set(serviceId, 'error');
            return next;
          });
        });
    } else {
      removeFromState();
    }
  }, []);

  const executeTool = useCallback(async (toolcall: ToolCall) => {
    let strippedToolName;
    if (!toolcall.function.name.startsWith(BUILTIN_PREFIX)) {
      strippedToolName = toolcall.function.name;
      logger.warn('tool call does not have builtin prefix', { toolcall });
    } else {
      strippedToolName = toolcall.function.name.replace(BUILTIN_PREFIX, '');
    }
    
    // Safe name parsing - split on first '__' only
    const idx = strippedToolName.indexOf('__');
    if (idx === -1) {
      throw new Error(`Invalid builtin tool name: ${strippedToolName}`);
    }
    const serviceId = strippedToolName.slice(0, idx);
    const toolName = strippedToolName.slice(idx + 2);
    
    const service = servicesRef.current.get(serviceId);
    if (service) {
      return service.executeTool({
        ...toolcall,
        function: {
          ...toolcall.function,
          name: toolName,
        },
      });
    }
    throw Error(`No built-in service found for serviceId: ${serviceId}`);
  }, []);

  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools,
      unregister,
      register,
      executeTool,
      status: Object.fromEntries(status.entries()),
    }),
    [availableTools, unregister, register, status],
  );

  logger.info('context value: ', { contextValue });

  return (
    <BuiltInToolContext.Provider value={contextValue}>
      {children}
    </BuiltInToolContext.Provider>
  );
}

export function useBuiltInTool() {
  const context = useContext(BuiltInToolContext);
  if (context === null) {
    throw Error('useBuiltInTool must be used within a BuiltInToolProvider');
  }
  return context;
}
