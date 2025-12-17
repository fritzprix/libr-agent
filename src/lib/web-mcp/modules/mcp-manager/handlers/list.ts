import { createMCPStructuredToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type { IMcpServerService } from '@/lib/services/mcp-server-service';
import type { IAssistantService } from '@/lib/services/assistant-service';
import type { MCPServerEntity } from '@/models/chat';
import { normalizePagination } from '../utils/pagination';
import type { ListServersInput, ListServersOutput } from '../types';

export async function listServers(
  mcpService: IMcpServerService,
  assistantService: IAssistantService,
  args: Record<string, unknown>,
  assistantId: string | null,
): Promise<MCPResult<ListServersOutput>> {
  const input: ListServersInput = {
    page: args.page !== undefined ? Number(args.page) : 1,
    pageSize: args.pageSize !== undefined ? Number(args.pageSize) : 20,
    filterByAssistant: Boolean(args.filterByAssistant),
    includeInactive: Boolean(args.includeInactive ?? true),
  };

  let servers: MCPServerEntity[];

  if (input.filterByAssistant && assistantId) {
    // Get assistant's connected servers
    const assistant = await assistantService.getById(assistantId);
    if (
      !assistant ||
      !assistant.mcpServerIds ||
      assistant.mcpServerIds.length === 0
    ) {
      const emptyPage = normalizePagination([], input.page!, input.pageSize!);
      return createMCPStructuredToolResult(
        `No MCP servers connected to assistant "${assistant?.name || 'current'}"`,
        emptyPage,
      );
    }

    // Fetch servers by IDs
    // Note: IMcpServerService doesn't have getByIds, so we fetch all and filter or fetch individually
    // For efficiency with remote, we might want to add getByIds to service, but for now let's fetch all
    // or fetch individually if count is small.
    // Let's fetch all for now as it's safer for remote consistency if getByIds isn't available
    const allServers = await mcpService.getAll();
    servers = allServers.filter((s) => assistant.mcpServerIds?.includes(s.id));
  } else {
    // Get all servers
    servers = await mcpService.getAll();
  }

  // Filter by active status
  if (!input.includeInactive) {
    servers = servers.filter((s: MCPServerEntity) => s.isActive);
  }

  // Sort by name
  servers.sort((a, b) => a.name.localeCompare(b.name));

  // Apply pagination
  const result = normalizePagination(servers, input.page!, input.pageSize!);

  // Build summary with server details
  const summaryLines = [
    `📋 MCP Servers (${result.totalItems} total)`,
    input.filterByAssistant ? `   Filtered by current assistant` : '',
    `   Page ${result.page}/${result.totalPages}`,
    `   Showing ${result.items.length} server(s)`,
  ];

  // Add server details
  if (result.items.length > 0) {
    summaryLines.push('');
    summaryLines.push('Servers:');
    result.items.forEach((server, idx) => {
      const status = server.isActive ? '🟢' : '🔴';
      const desc = server.metadata?.description
        ? ` - ${server.metadata.description.slice(0, 60)}${server.metadata.description.length > 60 ? '...' : ''}`
        : '';
      summaryLines.push(`  ${idx + 1}. ${status} ${server.name}${desc}`);
    });
  }

  const summary = summaryLines.filter(Boolean).join('\n');

  return createMCPStructuredToolResult(summary, result);
}
