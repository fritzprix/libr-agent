import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import { getLogger } from '@/lib/logger';
import { assistantManagerTools } from './tools';
import { AssistantService } from '@/lib/services/assistant-service';
import { dbService } from '@/lib/db';
import { createId } from '@paralleldrive/cuid2';
import { Assistant } from '@/models/chat';

const logger = getLogger('AssistantManagerServer');

interface AssistantToolArgs {
  id: string;
  name: string;
  systemPrompt: string;
  description?: string;
  mcpServerIds?: string[];
  allowedBuiltInServiceAliases?: string[];
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
      // Initialize service with agentHubUrl from DB
      const agentHubUrlObj = await dbService.objects.read('agentHubUrl');
      const agentHubUrl = agentHubUrlObj?.value as string;
      const service = new AssistantService(agentHubUrl);

      switch (name) {
        case 'list_assistants': {
          const assistants = await service.getAll();
          return createMCPStructuredToolResult(
            `Found ${assistants.length} assistants`,
            assistants,
          );
        }

        case 'get_assistant': {
          const { id } = typedArgs;
          if (!id) throw new Error('ID is required');
          const assistant = await service.getById(id);
          if (!assistant) {
            return createMCPErrorToolResult(
              `Assistant with ID ${id} not found`,
            );
          }
          return createMCPStructuredToolResult(
            `Found assistant: ${assistant.name}`,
            assistant,
          );
        }

        case 'create_assistant': {
          const {
            name,
            systemPrompt,
            description,
            mcpServerIds,
            allowedBuiltInServiceAliases,
          } = typedArgs;
          if (!name) throw new Error('Name is required');
          if (!systemPrompt) throw new Error('System prompt is required');

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
          return createMCPStructuredToolResult(
            `Created assistant: ${saved.name}`,
            saved,
          );
        }

        case 'update_assistant': {
          const { id, ...updates } = typedArgs;
          if (!id) throw new Error('ID is required');

          const existing = await service.getById(id);
          if (!existing) {
            return createMCPErrorToolResult(
              `Assistant with ID ${id} not found`,
            );
          }
          const updated: Assistant = {
            ...existing,
            ...updates,
            updatedAt: new Date(),
          };
          const saved = await service.save(updated);
          return createMCPStructuredToolResult(
            `Updated assistant: ${saved.name}`,
            saved,
          );
        }

        case 'delete_assistant': {
          const { id } = typedArgs;
          if (!id) throw new Error('ID is required');
          await service.delete(id);
          return createMCPStructuredToolResult(
            `Deleted assistant with ID ${id}`,
            { success: true, id },
          );
        }

        default:
          return createMCPErrorToolResult(`Unknown tool: ${name}`);
      }
    } catch (error) {
      logger.error(`Error executing tool ${name}`, error);
      return createMCPErrorToolResult(
        error instanceof Error ? error.message : String(error),
      );
    }
  },
};
