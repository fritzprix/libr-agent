import { safeInvoke } from './core';
import type { MCPServerEntity } from '@/models/chat';
import type { Page } from '@/lib/db/types';

/**
 * Backend DTO for MCP Server Config
 */
interface MCPServerDto {
  id: string; // Database ID (CUID2 format)
  name: string; // Human-readable name
  config: unknown; // JSON
  toolCount: number | null; // Cached tool count from last verification/connection
  createdAt: number;
  updatedAt: number;
}

// Convert backend DTO to frontend MCPServerEntity
function deserializeMCPServer(dto: MCPServerDto): MCPServerEntity {
  const config = dto.config as Partial<MCPServerEntity>;
  return {
    ...config,
    id: dto.id, // Use actual database ID (CUID2)
    name: dto.name,
    // ensure transport/auth are present if they were in config
    transport: (config as Record<string, unknown>).transport,
    authentication: (config as Record<string, unknown>).authentication,
    metadata: (config as Record<string, unknown>).metadata,
    toolCount: dto.toolCount !== null ? dto.toolCount : undefined, // Convert null to undefined
    isActive: config.isActive !== undefined ? config.isActive : true,
    // isActive is stored inside 'config' JSON in backend.

    createdAt: new Date(dto.createdAt),
    updatedAt: new Date(dto.updatedAt),
  } as MCPServerEntity;
}

// Convert frontend entity to backend params
function serializeMCPServer(server: MCPServerEntity): {
  name: string;
  config: string;
} {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const { id, name, createdAt, updatedAt, ...configRest } = server;
  // We store everything else in config
  return {
    name: name,
    config: JSON.stringify(configRest),
  };
}

export async function createMCPServer(
  server: MCPServerEntity,
): Promise<MCPServerEntity> {
  const params = serializeMCPServer(server);
  const dto = await safeInvoke<MCPServerDto>('create_mcp_server_config', {
    name: params.name,
    config: JSON.parse(params.config),
  });
  return deserializeMCPServer(dto);
}

export async function updateMCPServer(
  server: MCPServerEntity,
): Promise<MCPServerEntity> {
  const params = serializeMCPServer(server);
  const dto = await safeInvoke<MCPServerDto>('update_mcp_server_config', {
    id: server.id,
    name: params.name,
    config: JSON.parse(params.config),
  });
  return deserializeMCPServer(dto);
}

export async function deleteMCPServer(id: string): Promise<void> {
  await safeInvoke<void>('delete_mcp_server_config', { id });
}

export async function listMCPServers(): Promise<MCPServerEntity[]> {
  const dtos = await safeInvoke<MCPServerDto[]>('list_mcp_server_configs');
  return dtos.map(deserializeMCPServer);
}

export async function upsertMCPServer(server: MCPServerEntity): Promise<void> {
  const all = await listMCPServers();
  const exists = all.find((s) => s.name === server.name);

  if (exists) {
    await updateMCPServer(server);
  } else {
    await createMCPServer(server);
  }
}

export async function getMCPServersPage(
  page: number,
  pageSize: number,
): Promise<Page<MCPServerEntity>> {
  const all = await listMCPServers();
  const totalItems = all.length;

  if (pageSize === -1) {
    return {
      items: all,
      page: 1,
      pageSize: totalItems,
      totalItems,
      totalPages: 1,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  const totalPages = Math.ceil(totalItems / pageSize) || 1;
  const start = (page - 1) * pageSize;
  const end = start + pageSize;
  const items = all.slice(start, end);

  return {
    items,
    page,
    pageSize,
    totalItems,
    totalPages,
    hasNextPage: page * pageSize < totalItems,
    hasPreviousPage: page > 1,
  };
}

export async function getMCPServer(
  name: string,
): Promise<MCPServerEntity | undefined> {
  const all = await listMCPServers();
  return all.find((s) => s.name === name);
}

export interface MCPServerPreset {
  name: string;
  description?: string;
  logo?: string;
  transportType: 'stdio' | 'sse';
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  variableDefinitions?: Record<
    string,
    {
      required?: boolean;
      label?: string;
      description?: string;
      type?: 'text' | 'password';
      target?: 'env' | 'header' | 'bearer-token' | 'url-param';
    }
  >;
  url?: string;
}

export async function listMCPServerPresets(): Promise<MCPServerPreset[]> {
  return await safeInvoke<MCPServerPreset[]>('list_mcp_server_presets');
}
