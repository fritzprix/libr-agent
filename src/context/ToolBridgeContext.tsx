import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { createContext, ReactNode, useContext, useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import { useUnifiedMCP } from '@/hooks/use-unified-mcp';
import type { ToolCall } from '@/models/chat';

const logger = getLogger('ToolBridgeContext');

/**
 * Tool execution request from Rust backend
 */
interface ToolExecutionRequest {
  sessionId: string;
  toolCall: ToolCall;
}

/**
 * Tool execution result to send back to Rust
 */
interface ToolExecutionResult {
  success: boolean;
  content?: string;
  error?: string;
  isError?: boolean;
}

/**
 * Context value for Tool Bridge Provider
 */
// eslint-disable-next-line @typescript-eslint/no-empty-object-type
interface ToolBridgeContextValue {
  // Currently no exposed values, this is a pure listener context
}

const ToolBridgeContext = createContext<ToolBridgeContextValue | undefined>(
  undefined,
);

/**
 * Hook to access Tool Bridge Context
 */
export function useToolBridge(): ToolBridgeContextValue {
  const context = useContext(ToolBridgeContext);
  if (!context) {
    throw new Error('useToolBridge must be used within ToolBridgeProvider');
  }
  return context;
}

interface ToolBridgeProviderProps {
  children: ReactNode;
}

/**
 * Tool Bridge Provider
 * Enables Rust backend to execute tools in the TypeScript runtime
 * This is necessary for Web MCP tools and other TS-based tools
 */
export function ToolBridgeProvider({ children }: ToolBridgeProviderProps) {
  const { executeToolCall } = useUnifiedMCP();

  /**
   * Listen for tool execution requests from Rust backend
   */
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      logger.info('Setting up tool execution request listener');

      unlisten = await listen<ToolExecutionRequest>(
        'tool:execute-request',
        async (event) => {
          const { sessionId, toolCall } = event.payload;

          logger.debug('Received tool execution request', {
            sessionId,
            toolName: toolCall.function.name,
            toolCallId: toolCall.id,
          });

          try {
            // Execute the tool via unified MCP interface
            const result = await executeToolCall(toolCall);

            logger.info('Tool execution completed', {
              sessionId,
              toolName: toolCall.function.name,
              toolCallId: toolCall.id,
            });

            // Send result back to Rust
            const toolResult: ToolExecutionResult = {
              success: !result.error,
              content: Array.isArray(result.result?.content)
                ? result.result.content
                    .map((c) => (c.type === 'text' ? c.text : ''))
                    .join('\n')
                : '',
              error: result.error ? JSON.stringify(result.error) : undefined,
              isError: !!result.error,
            };

            await invoke('agent_handle_tool_result', {
              sessionId,
              toolCallId: toolCall.id,
              result: toolResult,
            });

            logger.debug('Tool result sent back to Rust', {
              sessionId,
              toolName: toolCall.function.name,
              toolCallId: toolCall.id,
            });
          } catch (error) {
            logger.error('Failed to execute tool', error);

            // Send error back to Rust
            const errorResult: ToolExecutionResult = {
              success: false,
              error: error instanceof Error ? error.message : String(error),
              isError: true,
            };

            await invoke('agent_handle_tool_result', {
              sessionId,
              toolCallId: toolCall.id,
              result: errorResult,
            });
          }
        },
      );

      logger.info('Tool execution request listener registered');
    };

    setupListener();

    return () => {
      if (unlisten) {
        unlisten();
        logger.info('Tool execution request listener cleaned up');
      }
    };
  }, [executeToolCall]);

  const value: ToolBridgeContextValue = {};

  return (
    <ToolBridgeContext.Provider value={value}>
      {children}
    </ToolBridgeContext.Provider>
  );
}
