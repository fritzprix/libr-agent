import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import { assistantManagerTools } from './tools';
import { AssistantService } from '@/lib/services/assistant-service';
import { LocalSettingsService } from '@/lib/services/settings-service';
import { createId } from '@paralleldrive/cuid2';
import type { Assistant } from '@/models/chat';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';

let cachedService: {
  service: AssistantService;
  agentHubUrl: string | undefined;
} | null = null;

async function getService() {
  const settingsService = new LocalSettingsService();
  const settings = await settingsService.getSettings();
  const agentHubUrl = settings.agentHubUrl;

  if (cachedService && cachedService.agentHubUrl === agentHubUrl) {
    return cachedService.service;
  }

  const service = new AssistantService(agentHubUrl);
  cachedService = {
    service,
    agentHubUrl,
  };
  return service;
}

interface AssistantToolArgs {
  id?: string;
  name?: string;
  systemPrompt?: string;
  description?: string;
  mcpServerIds?: string[];
  allowedBuiltInServiceAliases?: string[];
  page?: number;
  pageSize?: number;
  query?: string;
  limit?: number;
}

export const assistantManagerServer: WebMCPServer = {
  name: 'assistant_manager',
  displayName: 'Assistant Manager',
  description: 'Manage AI assistants (create, update, delete, list)',
  version: '1.0.0',
  tools: assistantManagerTools,

  async callTool(name: string, args: unknown): Promise<MCPResult> {
    const typedArgs = (args || {}) as Partial<AssistantToolArgs>;
    try {
      const service = await getService();

      switch (name) {
        case 'listAssistants': {
          const page = typedArgs.page ?? 1;
          const pageSize = typedArgs.pageSize ?? 20;
          const result = await service.getList({ page, pageSize });

          return createMCPStructuredToolResult(
            `Found ${result.items.length} assistants (Page ${page}/${result.totalPages}, Total: ${result.totalItems})`,
            result,
          );
        }

        case 'getAssistant': {
          const { id } = typedArgs;
          if (!id) {
            return createMCPErrorToolResult('ID is required');
          }
          const assistant = await service.getById(id);
          if (!assistant) {
            const suggestions: string[] = [
              'Use listAssistants to see all available assistants',
              'Use searchAssistant to find assistants by name',
              'Check the assistant ID spelling',
            ];

            return new MCPResponseBuilder({
              requestedId: id,
              suggestions,
            })
              .withMessage(`Assistant with ID ${id} not found.`)
              .withSuggestions(suggestions)
              .asError(WebMCPErrorCodes.ASSISTANT.NOT_FOUND);
          }
          return createMCPStructuredToolResult(
            `Found assistant: ${assistant.name}`,
            assistant,
          );
        }

        case 'createAssistant': {
          const {
            name,
            systemPrompt,
            description,
            mcpServerIds,
            allowedBuiltInServiceAliases,
          } = typedArgs;
          if (!name) {
            return createMCPErrorToolResult('Name is required');
          }
          if (!systemPrompt) {
            return createMCPErrorToolResult('System prompt is required');
          }

          const newAssistant: Assistant = {
            id: createId(),
            name,
            systemPrompt,
            description,
            mcpServerIds: mcpServerIds || [],
            allowedBuiltInServiceAliases: allowedBuiltInServiceAliases,
            deletionProtected: false,
            createdAt: new Date(),
            updatedAt: new Date(),
          };
          const saved = await service.save(newAssistant);

          const nextActions: string[] = [
            'Use updateAssistant to modify settings',
            'Connect MCP servers with connect_server tool',
          ];
          if ((mcpServerIds || []).length === 0) {
            nextActions.push(
              'Add MCP server connections to extend capabilities',
            );
          }

          let message = `Assistant created: "${saved.name}" (ID: ${saved.id})`;
          if (description) {
            message += `\nDescription: ${description}`;
          }
          message += `\nMCP servers: ${(mcpServerIds || []).length}`;

          return new MCPResponseBuilder({
            assistant: saved,
            id: saved.id,
          })
            .withMessage(message)
            .withNextActions(nextActions)
            .asSuccess();
        }

        case 'updateAssistant': {
          const { id, ...updates } = typedArgs;
          if (!id) {
            return createMCPErrorToolResult('ID is required');
          }

          const existing = await service.getById(id);
          if (!existing) {
            const suggestions: string[] = [
              'Use listAssistants to see all available assistants',
              'Use searchAssistant to find assistants by name',
              'Check the assistant ID spelling',
            ];

            return new MCPResponseBuilder({
              requestedId: id,
              suggestions,
            })
              .withMessage(`Assistant with ID ${id} not found.`)
              .withSuggestions(suggestions)
              .asError(WebMCPErrorCodes.ASSISTANT.NOT_FOUND);
          }
          const updated: Assistant = {
            ...existing,
            ...updates,
            updatedAt: new Date(),
          };
          const saved = await service.save(updated);

          // Detect what was changed
          const changes: string[] = [];
          if (updates.name && updates.name !== existing.name) {
            changes.push(`name: "${existing.name}" → "${updates.name}"`);
          }
          if (
            updates.systemPrompt &&
            updates.systemPrompt !== existing.systemPrompt
          ) {
            changes.push('system prompt updated');
          }
          if (
            updates.description !== undefined &&
            updates.description !== existing.description
          ) {
            changes.push('description updated');
          }
          if (updates.mcpServerIds) {
            changes.push(
              `MCP servers: ${existing.mcpServerIds?.length || 0} → ${updates.mcpServerIds.length}`,
            );
          }

          let message = `Assistant updated: "${saved.name}" (ID: ${saved.id})`;
          if (changes.length > 0) {
            message += `\n\nChanges:\n${changes.map((c) => `  - ${c}`).join('\n')}`;
          }

          return new MCPResponseBuilder({
            assistant: saved,
            id: saved.id,
            changes,
          })
            .withMessage(message)
            .withNextActions(['Use getAssistant to verify changes'])
            .asSuccess();
        }

        case 'deleteAssistant': {
          const { id } = typedArgs;
          if (!id) {
            return createMCPErrorToolResult('ID is required');
          }
          await service.delete(id);
          return createMCPStructuredToolResult(
            `Deleted assistant with ID ${id}`,
            { success: true, id },
          );
        }

        case 'searchAssistant': {
          const { query, limit = 10 } = typedArgs;
          if (!query) {
            return createMCPErrorToolResult('Query is required');
          }

          const results = await service.search(query, limit);

          // Handle no results with guidance
          if (results.length === 0) {
            const allAssistants = await service.getAll();
            const totalCount = allAssistants.length;

            const suggestions: string[] = [
              'Use listAssistants to browse all assistants',
              'Try different or shorter keywords',
              'Check spelling of assistant name',
            ];

            return new MCPResponseBuilder({
              results: [],
              query,
              totalAssistants: totalCount,
              suggestions,
            })
              .withMessage(
                `No assistants found matching "${query}".\n\n` +
                  `Total assistants: ${totalCount}`,
              )
              .withSuggestions(suggestions)
              .asSuccess();
          }

          // Success with results
          return new MCPResponseBuilder({
            results,
            query,
            count: results.length,
          })
            .withMessage(
              `Found ${results.length} assistant(s) matching "${query}"`,
            )
            .withNextActions(['Use getAssistant to view full details'])
            .asSuccess();
        }

        default:
          return createMCPErrorToolResult(`Unknown tool: ${name}`);
      }
    } catch (error) {
      return createMCPErrorToolResult(
        error instanceof Error ? error.message : String(error),
      );
    }
  },
};
