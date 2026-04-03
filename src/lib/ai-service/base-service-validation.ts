import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import { AIServiceError, type AIServiceProvider } from './types';

const VALID_MESSAGE_ROLES = new Set(['user', 'assistant', 'system', 'tool']);

export function validateApiKey(
  apiKey: string,
  provider: AIServiceProvider,
): void {
  if (!apiKey || typeof apiKey !== 'string' || apiKey.trim().length === 0) {
    throw new AIServiceError('Invalid API key provided', provider);
  }
}

export function validateMessages(
  messages: Message[],
  provider: AIServiceProvider,
): void {
  if (!messages || !Array.isArray(messages) || messages.length === 0) {
    throw new AIServiceError('Messages array is empty or undefined', provider);
  }

  messages.forEach((message) => {
    if (!message.id || typeof message.id !== 'string') {
      throw new Error('Message must have a valid id');
    }

    if (
      (!message.content &&
        (message.role === 'user' || message.role === 'system')) ||
      (typeof message.content !== 'string' && !Array.isArray(message.content))
    ) {
      throw new Error('Message must have valid content');
    }

    if (!VALID_MESSAGE_ROLES.has(message.role)) {
      throw new Error('Message must have a valid role');
    }
  });
}

export function validateToolDefinition(
  tool: MCPTool,
  provider: AIServiceProvider,
): void {
  if (!tool.name || typeof tool.name !== 'string') {
    throw new Error(`Tool must have a valid name (provider: ${provider})`);
  }

  if (!tool.description || typeof tool.description !== 'string') {
    throw new Error(`Tool must have a valid description (tool: ${tool.name})`);
  }

  if (!tool.inputSchema || typeof tool.inputSchema !== 'object') {
    throw new Error(`Tool must have a valid inputSchema (tool: ${tool.name})`);
  }

  if (tool.inputSchema.type !== 'object') {
    throw new Error(
      `Tool inputSchema must be of type "object" (tool: ${tool.name})`,
    );
  }
}
