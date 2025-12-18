import { createMCPErrorToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type { IMcpServerService } from '@/lib/services/mcp-server-service';
import type { IAssistantService } from '@/lib/services/assistant-service';
import type { MCPServerEntity } from '@/models/chat';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';
import type {
  ConnectInput,
  ConnectServerOutput,
  DisconnectInput,
} from '../types';

/**
 * Helper function to find a server by ID or name
 */
export async function findServer(
  service: IMcpServerService,
  serverId?: string,
  serverName?: string,
): Promise<MCPServerEntity | undefined> {
  if (serverId) {
    return await service.getById(serverId);
  }
  if (serverName) {
    return await service.getByName(serverName);
  }
  return undefined;
}

export async function connectServer(
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
  assistantId: string | null,
): Promise<MCPResult<unknown>> {
  const input: ConnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
    autoStart: Boolean(args.autoStart ?? true),
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPErrorToolResult(
      'Scope must be "assistant" or "global"',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Validate that at least one identifier is provided
  if (!input.serverId && !input.serverName) {
    return createMCPErrorToolResult(
      'Either serverId or serverName is required',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Find server using helper
  const server = await findServer(mcpService, input.serverId, input.serverName);

  if (!server) {
    const searchTerm = input.serverId || input.serverName || '';
    const suggestions: string[] = [
      'Use search_server to find servers by name',
      'Use list_servers to see all available servers',
      'Check spelling and try exact server name',
    ];

    return new MCPResponseBuilder({
      requestedId: input.serverId,
      requestedName: input.serverName,
      suggestions,
    })
      .withMessage(`Server not found: "${searchTerm}".`)
      .withSuggestions(suggestions)
      .asError(WebMCPErrorCodes.MCP_MANAGER.SERVER_NOT_FOUND);
  }

  // Connect based on scope
  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPErrorToolResult(
        'Assistant context not available. Cannot connect to assistant scope.',
        { tool: 'connectServer', scope: input.scope },
      ) as MCPResult<ConnectServerOutput>;
    }

    const assistant = await assistantService.getById(assistantId);
    if (!assistant) {
      return createMCPErrorToolResult(
        `Assistant not found: ${assistantId}`,
      ) as MCPResult<ConnectServerOutput>;
    }

    // Add to assistant's server list if not already connected
    const serverIds = assistant.mcpServerIds || [];
    if (!serverIds.includes(server.id)) {
      assistant.mcpServerIds = [...serverIds, server.id];
      await assistantService.save(assistant);
    }

    const nextActions = [
      'Use listServers with filterByAssistant=true to verify connection',
      "Check your assistant's tool list for new capabilities",
      'Server tools are now available for this assistant',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'assistant' as const,
      assistantId,
    })
      .withMessage(
        `Server "${server.name}" connected to assistant "${assistant.name}".\n` +
          `Scope: assistant\n` +
          (input.autoStart
            ? `Status: Starting...`
            : `Status: Registered (manual start required)`),
      )
      .withNextActions(nextActions)
      .asSuccess();
  } else {
    // Global scope: mark server as globally enabled
    server.isActive = true;
    server.updatedAt = new Date();
    await mcpService.save(server);

    const nextActions = [
      'Use listServers to verify the server is active',
      'Tools from this server are now available system-wide',
      "All assistants can access this server's capabilities",
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'global' as const,
    })
      .withMessage(
        `Server "${server.name}" enabled globally.\n` + `Status: Active`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  }
}

export async function disconnectServer(
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
  assistantId: string | null,
): Promise<MCPResult<unknown>> {
  const input: DisconnectInput = {
    serverId: args.serverId ? String(args.serverId) : undefined,
    serverName: args.serverName ? String(args.serverName) : undefined,
    scope: (args.scope as 'assistant' | 'global') || 'assistant',
  };

  // Validate scope
  if (input.scope !== 'assistant' && input.scope !== 'global') {
    return createMCPErrorToolResult(
      'Scope must be "assistant" or "global"',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Validate that at least one identifier is provided
  if (!input.serverId && !input.serverName) {
    return createMCPErrorToolResult(
      'Either serverId or serverName is required',
    ) as MCPResult<ConnectServerOutput>;
  }

  // Find server using helper
  const server = await findServer(mcpService, input.serverId, input.serverName);

  if (!server) {
    const searchTerm = input.serverId || input.serverName || '';
    const suggestions: string[] = [
      'Use search_server to find servers by name',
      'Use list_servers to see all available servers',
      'Check spelling and try exact server name',
    ];

    return new MCPResponseBuilder({
      requestedId: input.serverId,
      requestedName: input.serverName,
      suggestions,
    })
      .withMessage(`Server not found: "${searchTerm}".`)
      .withSuggestions(suggestions)
      .asError(WebMCPErrorCodes.MCP_MANAGER.SERVER_NOT_FOUND);
  }

  if (input.scope === 'assistant') {
    if (!assistantId) {
      return createMCPErrorToolResult('Assistant context not available.', {
        tool: 'disconnectServer',
        scope: input.scope,
      });
    }

    const assistant = await assistantService.getById(assistantId);
    if (!assistant) {
      return createMCPErrorToolResult(`Assistant not found: ${assistantId}`);
    }

    const serverIds = assistant.mcpServerIds || [];
    assistant.mcpServerIds = serverIds.filter((id) => id !== server.id);
    await assistantService.save(assistant);

    const nextActions = [
      'Tools from this server are no longer available to this assistant',
      'Other assistants are not affected',
      'Use connectServer to reconnect anytime',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'assistant' as const,
      impactScope: 'single_assistant' as const,
    })
      .withMessage(
        `Server "${server.name}" disconnected from assistant "${assistant.name}".\n` +
          `Impact: Current assistant only`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  } else {
    server.isActive = false;
    server.updatedAt = new Date();
    await mcpService.save(server);

    const nextActions = [
      'Tools from this server are no longer available',
      'Impact: All assistants system-wide',
      'Server configuration preserved (reconnect anytime with connectServer)',
    ];

    return new MCPResponseBuilder({
      success: true,
      server,
      scope: 'global' as const,
      impactScope: 'global' as const,
    })
      .withMessage(
        `Server "${server.name}" disabled globally.\n` + `Impact: System-wide`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  }
}
