import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type { IMcpServerService } from '@/lib/services/mcp-server-service';
import type { MCPServerEntity } from '@/models/chat';
import type { TransportConfig } from '@/lib/mcp/config/transport';
import { normalizeTransportConfig } from '../utils/validation';
import { clearBM25Cache } from '@/lib/search/bm25';
import type { CreateServerInput, CreateServerOutput } from '../types';

export async function createServer(
  mcpService: IMcpServerService,
  args: Record<string, unknown>,
): Promise<MCPResult<CreateServerOutput>> {
  const input: CreateServerInput = {
    name: String(args.name || '').trim(),
    description: args.description ? String(args.description) : undefined,
    transport: args.transport,
    tags: args.tags as string[] | undefined,
  };

  // Validate required fields
  if (!input.name) {
    return createMCPErrorToolResult(
      'Server name is required',
    ) as MCPResult<CreateServerOutput>;
  }

  // Normalize and validate transport configuration
  let normalizedTransport: TransportConfig;
  try {
    normalizedTransport = normalizeTransportConfig(input.transport);
  } catch (error) {
    return createMCPErrorToolResult(
      `Invalid transport configuration: ${error instanceof Error ? error.message : String(error)}`,
    ) as MCPResult<CreateServerOutput>;
  }

  // Generate unique ID using crypto.randomUUID() with fallback
  const id =
    typeof crypto !== 'undefined' && 'randomUUID' in crypto
      ? `mcp-${crypto.randomUUID()}`
      : `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

  // Build server entity
  const now = new Date();
  const server: MCPServerEntity = {
    id,
    name: input.name,
    isActive: true,
    createdAt: now,
    updatedAt: now,
    transport: normalizedTransport,
    metadata: {
      description: input.description || '',
    },
  };

  // Try to save (will throw if name exists)
  try {
    await mcpService.save(server);

    // Clear BM25 cache when data changes
    clearBM25Cache();

    const transportType = normalizedTransport.type;
    const summary = [
      `✅ MCP Server Created Successfully`,
      `   Name: ${server.name}`,
      `   ID: ${server.id}`,
      `   Transport: ${transportType}`,
      `   Status: Active`,
      ``,
      `💡 Use "connectServer" to enable this server for the current assistant.`,
    ].join('\n');

    return createMCPStructuredToolResult(summary, {
      server,
      message: `Server "${server.name}" created successfully`,
    });
  } catch (error) {
    return createMCPErrorToolResult(
      `Failed to create server: ${error instanceof Error ? error.message : String(error)}`,
    ) as MCPResult<CreateServerOutput>;
  }
}
