import { useEffect, useRef } from 'react';
import { useBuiltInTool, ServiceContextOptions } from '.';
import { useBrowserInvoker } from '@/hooks/use-browser-invoker';
import { MCPResponse } from '@/lib/mcp-types';
import { isMCPResponse, createMCPTextResponse } from '@/lib/mcp-response-utils';
import { getLogger } from '@/lib/logger';
import { listBrowserSessions, BrowserSession } from '@/lib/rust-backend-client';
import { ToolCall } from '@/models/chat';
import {
  // Simple tools
  createSessionTool,
  closeSessionTool,
  listSessionsTool,
  navigateToUrlTool,
  // Tools requiring executeScript
  getCurrentUrlTool,
  getPageTitleTool,
  scrollPageTool,
  navigateBackTool,
  navigateForwardTool,
  // Complex tools
  clickElementTool,
  inputTextTool,
  extractContentTool,
  // Types
  StrictLocalMCPTool,
} from './browser-tools';

const logger = getLogger('BrowserToolProvider');

/**
 * Provider component that registers browser-specific tools with the BuiltInToolProvider.
 * Uses the useBrowserInvoker hook to provide non-blocking browser script execution.
 */
export function BrowserToolProvider() {
  const { register, unregister } = useBuiltInTool();
  const { executeScript } = useBrowserInvoker();
  const hasRegistered = useRef(false);

  useEffect(() => {
    // Prevent duplicate registrations
    if (hasRegistered.current) {
      logger.debug('Browser tools already registered, skipping...');
      return;
    }

    logger.debug('Registering browser tools...');

    // Create browser tools using modular approach
    const simpleBrowserTools = [
      createSessionTool,
      closeSessionTool,
      listSessionsTool,
      navigateToUrlTool,
    ];

    const scriptDependentTools = [
      getCurrentUrlTool,
      getPageTitleTool,
      scrollPageTool,
      navigateBackTool,
      navigateForwardTool,
      clickElementTool,
      inputTextTool,
      extractContentTool,
    ];

    // Inject executeScript function for tools that need it
    const enhancedScriptTools: StrictLocalMCPTool[] = scriptDependentTools.map(
      (tool) => ({
        ...tool,
        execute: async (args: Record<string, unknown>) => {
          return await tool.execute(args, executeScript);
        },
      }),
    );

    const browserTools: StrictLocalMCPTool[] = [
      ...simpleBrowserTools,
      ...enhancedScriptTools,
    ];

    logger.info('Registering browser tools', {
      toolCount: browserTools.length,
      toolNames: browserTools.map((t) => t.name),
    });

    const serviceId = 'browser';
    const service = {
      listTools: () =>
        browserTools.map((tool) => {
          // Extract meta data without execute function
          return {
            name: tool.name,
            description: tool.description,
            inputSchema: tool.inputSchema,
          };
        }),
      executeTool: async (toolCall: ToolCall): Promise<MCPResponse> => {
        const toolName = toolCall.function.name;
        const tool = browserTools.find((t) => t.name === toolName);

        if (!tool) {
          throw new Error(`Browser tool not found: ${toolName}`);
        }

        // Parse arguments
        let args: Record<string, unknown> = {};
        try {
          const raw = toolCall.function.arguments;
          if (typeof raw === 'string') {
            args = raw.length ? JSON.parse(raw) : {};
          } else if (typeof raw === 'object' && raw !== null) {
            args = raw as Record<string, unknown>;
          }
        } catch (error) {
          logger.warn('Failed parsing browser tool arguments', {
            toolName,
            error,
          });
          args = {};
        }

        // Execute the tool
        const result = await tool.execute(args);

        // All tools now return MCPResponse, ensure correct ID is set
        if (isMCPResponse(result)) {
          return { ...result, id: toolCall.id };
        }

        // This should not happen with unified MCP types, but keep as safety fallback
        logger.warn('Tool returned non-MCPResponse, this should not happen', {
          toolName,
          resultType: typeof result,
        });
        return createMCPTextResponse(
          typeof result === 'string' ? result : JSON.stringify(result),
          toolCall.id,
        );
      },
      getServiceContext: async (
        options?: ServiceContextOptions,
      ): Promise<string> => {
        try {
          const sessions = await listBrowserSessions();
          if (sessions.length === 0) {
            return '# Browser Sessions\nNo active browser sessions.';
          }

          const sessionInfo = sessions
            .map(
              (s: BrowserSession) =>
                `Session ${s.id}: ${s.url || 'No URL'} (${s.title || 'Untitled'})`,
            )
            .join('\n');

          // Note: 브라우저 세션은 현재 전역적으로 관리되므로 sessionId를 직접 사용하지 않지만,
          // 향후 세션별 브라우저 관리를 위해 인터페이스는 동일하게 맞춰둠
          return `# Browser Sessions\n${sessionInfo}`;
        } catch (error) {
          logger.error('Failed to get browser sessions', {
            sessionId: options?.sessionId,
            error,
          });
          return '# Browser Sessions\nError loading browser sessions.';
        }
      },
    };

    register(serviceId, service);
    hasRegistered.current = true;

    logger.debug('Browser tools registered successfully');

    // Return cleanup function
    return () => {
      if (hasRegistered.current) {
        unregister(serviceId);
        hasRegistered.current = false;
        logger.debug('Browser tools unregistered');
      }
    };
  }, [register, unregister, executeScript]);

  // This is a provider component, it doesn't render anything
  return null;
}
