import type { Tool } from 'ollama';

import type { Message } from '@/models/chat';
import type { MCPTool } from '@/lib/mcp';
import {
  formatToolResultForLlm,
  tryParse,
  generateToolCallId,
  processMultiModalContent,
} from './utils';
import {
  noopLogger,
  type Logger,
  type SimpleOllamaMessage,
} from './ollama-core-types';

export function convertMCPToolsToOllamaTools(
  mcpTools?: MCPTool[],
  logger: Logger = noopLogger,
): Tool[] {
  if (!mcpTools || mcpTools.length === 0) {
    return [];
  }

  return mcpTools.map((tool) => {
    const schema = tool.inputSchema || { type: 'object', properties: {} };
    const parameters: {
      type: 'object';
      properties: Record<
        string,
        {
          type?: string | string[];
          items?: unknown;
          description?: string;
          enum?: unknown[];
        }
      >;
      required?: string[];
    } = {
      type: 'object',
      properties: (schema.properties || {}) as Record<
        string,
        {
          type?: string | string[];
          items?: unknown;
          description?: string;
          enum?: unknown[];
        }
      >,
    };

    if (schema.required && schema.required.length > 0) {
      parameters.required = schema.required;
    }

    logger.debug('Converting MCP tool to Ollama format', {
      name: tool.name,
      hasRequired: !!schema.required,
      requiredCount: schema.required?.length || 0,
    });

    return {
      type: 'function',
      function: {
        name: tool.name,
        description: tool.description || '',
        parameters,
      },
    };
  });
}

export function processMessageContent(content: Message['content']): string {
  if (!content) return '';
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .map((item) => {
        if ('text' in item) return item.text;
        if ('image_url' in item) return `[Image: ${item.image_url}]`;
        return '';
      })
      .join('\n');
  }
  return '';
}

export function convertUserMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  const multimodal = processMultiModalContent(message.content);
  const textParts = multimodal.filter((part) => part.type === 'text');
  const imageParts = multimodal.filter((part) => part.type === 'image');
  const content = textParts.map((part) => part.text ?? '').join('\n');
  const images = imageParts
    .map((part) => part.image)
    .filter(
      (image): image is string => typeof image === 'string' && image.length > 0,
    );

  logger.debug('Converting user message', {
    messageId: message.id,
    contentLength: content.length,
    imageCount: images.length,
  });

  const result: SimpleOllamaMessage = { role: 'user', content };
  if (images.length > 0) {
    result.images = images;
  }
  return result;
}

export function convertAssistantMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  const result: SimpleOllamaMessage = {
    role: 'assistant',
    content: processMessageContent(message.content) || '',
  };

  if (message.tool_calls && message.tool_calls.length > 0) {
    result.tool_calls = message.tool_calls.map((toolCall) => {
      const callId = toolCall.id || generateToolCallId();
      const args =
        tryParse<Record<string, unknown>>(toolCall.function.arguments) ?? {};

      logger.debug('Converting assistant tool call', {
        id: callId,
        name: toolCall.function.name,
        argsType: typeof toolCall.function.arguments,
      });

      return {
        id: callId,
        type: 'function' as const,
        function: {
          name: toolCall.function.name,
          arguments: args,
        },
      };
    });
  }

  return result;
}

export function convertMessage(
  message: Message,
  logger: Logger = noopLogger,
): SimpleOllamaMessage | null {
  if (!message?.role) {
    logger.warn('Invalid message structure', { message });
    return null;
  }

  logger.debug(`🔄 Converting message: role=${message.role}, id=${message.id}`);

  switch (message.role) {
    case 'user': {
      const userResult = convertUserMessage(message, logger);
      logger.debug('User message converted', {
        messageId: message.id,
        hasResult: !!userResult,
        contentLength: userResult?.content?.length ?? 0,
      });
      return userResult;
    }
    case 'assistant': {
      const assistantResult = convertAssistantMessage(message, logger);
      logger.debug('Assistant message converted', {
        messageId: message.id,
        hasResult: !!assistantResult,
        hasToolCalls: !!assistantResult?.tool_calls,
        toolCallCount: assistantResult?.tool_calls?.length ?? 0,
      });
      return assistantResult;
    }
    case 'system': {
      const systemContent = processMessageContent(message.content) || '';
      logger.debug('System message converted', {
        messageId: message.id,
        contentLength: systemContent.length,
      });
      return {
        role: 'system',
        content: systemContent,
      };
    }
    case 'tool': {
      const toolContent = formatToolResultForLlm(message);
      logger.debug('🔧 Tool message converted', {
        messageId: message.id,
        toolCallId: message.tool_call_id,
        contentLength: toolContent.length,
        contentPreview: toolContent.substring(0, 100),
      });
      const result: SimpleOllamaMessage = {
        role: 'tool',
        content: toolContent,
        tool_call_id: message.tool_call_id,
      };

      const multimodal = processMultiModalContent(message.content);
      const images = multimodal
        .filter((part) => part.type === 'image')
        .map((part) => part.image)
        .filter(
          (image): image is string =>
            typeof image === 'string' && image.length > 0,
        );
      if (images.length > 0) {
        result.images = images;
      }
      return result;
    }
    default:
      logger.warn(`Unsupported message role: ${message.role}`);
      return null;
  }
}

export function convertToOllamaMessages(
  messages: Message[],
  systemPrompt?: string,
  logger: Logger = noopLogger,
): SimpleOllamaMessage[] {
  if (!Array.isArray(messages) || messages.length === 0) {
    throw new Error('Messages must be a non-empty array');
  }

  const ollamaMessages: SimpleOllamaMessage[] = [];

  if (systemPrompt?.trim()) {
    ollamaMessages.push({
      role: 'system',
      content: systemPrompt.trim(),
    });
    logger.debug('Added system prompt', {
      length: systemPrompt.trim().length,
    });
  }

  let skippedCount = 0;
  for (const message of messages) {
    const converted = convertMessage(message, logger);
    if (converted) {
      ollamaMessages.push(converted);
    } else {
      skippedCount++;
      logger.warn('⚠️ Message conversion returned null - SKIPPED', {
        messageId: message.id,
        role: message.role,
        hasContent: !!message.content,
        contentLength: message.content?.length ?? 0,
        hasToolCalls: !!message.tool_calls,
        toolCallId: message.tool_call_id,
      });
    }
  }

  logger.info('Converted messages to Ollama format', {
    inputCount: messages.length,
    outputCount: ollamaMessages.length,
    skippedCount,
  });

  return ollamaMessages;
}
