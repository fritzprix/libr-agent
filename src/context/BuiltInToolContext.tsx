import { useMCPServer } from '@/hooks/use-mcp-server';
import { useWebMCPTools } from '@/hooks/use-web-mcp';
import { MCPResponse, MCPTool } from '@/lib/mcp-types';
import { MCPConfig } from '@/models/chat';
import {
  createContext,
  ReactNode,
  useCallback,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import { useWebMCPServer, WebMCPProvider } from './WebMCPContext';
import builtinMCPServers from '@/config/builtin-mcp-servers.json';
import { useChatContext } from './ChatContext';
import { ContentStoreServer } from '@/lib/web-mcp/modules/content-store';
import { useSessionContext } from './SessionContext';
import { getLogger } from '@/lib/logger';

// Built-in MCP 설정을 JSON에서 로드
const BUILTIN_MCP_CONFIGS: MCPConfig = {
  mcpServers: builtinMCPServers.servers,
};

interface BuiltInToolContextType {
  availableTools: MCPTool[];
  executeToolCall: (toolCall: {
    id: string;
    type: 'function';
    function: { name: string; arguments: string };
  }) => Promise<MCPResponse>;
}

interface BuiltInToolProviderProps {
  children: ReactNode;
}

const BuiltInToolContext = createContext<BuiltInToolContextType | null>(null);

function BuiltInToolProviderInternal({ children }: BuiltInToolProviderProps) {
  const { connectServers, executeToolCall: executeRemoteTool } = useMCPServer();
  const { serverStates, getServerTools, executeCall } = useWebMCPTools();
  const servers: string[] = useMemo(
    () => Object.keys(serverStates),
    [serverStates],
  );
  const webWorkerMcptools = useMemo(
    () => servers.flatMap((s) => getServerTools(s)),
    [getServerTools, servers],
  );

  useEffect(() => {
    connectServers(BUILTIN_MCP_CONFIGS);
  }, [connectServers]);

  // 통합된 tool 실행 함수
  const executeToolCall = useMemo(
    () =>
      async (toolCall: {
        id: string;
        type: 'function';
        function: { name: string; arguments: string };
      }): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;

        // Web Worker 도구인지 확인 (server__tool 형식)
        const isWebWorkerTool = webWorkerMcptools.some(
          (tool) => tool.name === toolName,
        );

        if (isWebWorkerTool) {
          // Web Worker 도구 실행
          try {
            const args = JSON.parse(toolCall.function.arguments);
            const result = await executeCall(toolName, args);
            return {
              jsonrpc: '2.0',
              id: toolCall.id,
              result: {
                content: [
                  {
                    type: 'text',
                    text:
                      typeof result === 'string'
                        ? result
                        : JSON.stringify(result, null, 2),
                  },
                ],
              },
            };
          } catch (error) {
            return {
              jsonrpc: '2.0',
              id: toolCall.id,
              error: {
                code: -32603,
                message: error instanceof Error ? error.message : String(error),
              },
            };
          }
        }

        // Remote MCP 도구 실행
        return executeRemoteTool(toolCall);
      },
    [webWorkerMcptools, executeCall, executeRemoteTool],
  );

  const contextValue: BuiltInToolContextType = useMemo(
    () => ({
      availableTools: [...webWorkerMcptools],
      executeToolCall,
    }),
    [webWorkerMcptools, executeToolCall],
  );

  return (
    <BuiltInToolContext.Provider value={contextValue}>
      {children}
    </BuiltInToolContext.Provider>
  );
}

/**
 * Built-in tools context provider that integrates both Web Worker MCP tools
 * and remote MCP server tools into a unified interface.
 */
function BuiltInToolProvider({ children }: BuiltInToolProviderProps) {
  return (
    <WebMCPProvider servers={['content-store']} autoLoad={true}>
      <BuiltInToolProviderInternal>{children}</BuiltInToolProviderInternal>
    </WebMCPProvider>
  );
}

/**
 * Built-in 도구들을 사용하기 위한 hook
 */
function useBuiltInTools() {
  const context = useContext(BuiltInToolContext);
  if (!context) {
    throw new Error(
      'useBuiltInTools must be used within a BuiltInToolProvider',
    );
  }
  return context;
}


/**
 * System prompt component that dynamically injects information about attached files
 * into the chat context. This allows the AI to be aware of files that have been
 * uploaded to the current session's content store.
 */
function BuiltInToolsSystemPrompt() {
  const logger = getLogger('BuiltInToolsSystemPrompt');
  const { registerSystemPrompt, unregisterSystemPrompt } = useChatContext();
  const { server } = useWebMCPServer<ContentStoreServer>("content-store");
  const { current: currentSession } = useSessionContext();
  
  const buildPrompt = useCallback(async () => {
    if (!currentSession?.id) {
      logger.warn('No current session available for building attached files prompt');
      return '# Attached Files\nNo files currently attached.';
    }
    
    try {
      const result = await server?.listContent({ storeId: currentSession.id });
      
      if (!result?.contents || result.contents.length === 0) {
        return '# Attached Files\nNo files currently attached.';
      }
      
      const attachedResources = result.contents
        .map(c => JSON.stringify({
          storeId: c.storeId,
          contentId: c.contentId,
          preview: c.preview,
          filename: c.filename,
          type: c.mimeType,
          size: c.size
        }))
        .join('\n');
      
      logger.debug('Built attached files prompt', { 
        sessionId: currentSession.id, 
        fileCount: result.contents.length 
      });
      
      return `# Attached Files\n${attachedResources}`;
    } catch (error) {
      logger.error('Failed to build attached files prompt', {
        sessionId: currentSession.id,
        error: error instanceof Error ? error.message : String(error)
      });
      return '# Attached Files\nError loading attached files.';
    }
  }, [currentSession?.id, server, logger]);
  
  useEffect(() => {
    if (!currentSession?.id || !server) {
      logger.debug('Skipping system prompt registration - missing session or server');
      return;
    }
    
    const id = registerSystemPrompt({
      content: buildPrompt,
      priority: 1,
    });
    
    logger.debug('Registered attached files system prompt', { 
      sessionId: currentSession.id,
      promptId: id 
    });
    
    return () => {
      unregisterSystemPrompt(id);
      logger.debug('Unregistered attached files system prompt', { promptId: id });
    };
  }, [currentSession?.id, server, buildPrompt, registerSystemPrompt, unregisterSystemPrompt, logger]);
  
  return null;
}

export { BuiltInToolProvider, useBuiltInTools, BuiltInToolsSystemPrompt };
