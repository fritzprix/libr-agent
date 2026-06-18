import OpenAI from 'openai';

import type { MCPContent } from '@/lib/mcp';
import type { Message } from '@/models/chat';
import { isCompactSummaryMessage } from '../base-service-context';

import { formatToolResultForLlm } from '../utils';
import type { OpenAILoggerLike } from './types';

type OpenAIMultimodalItem = {
  type: string;
  text?: string;
  image?: string;
  audio?: string;
  mimeType?: string;
};

export function formatOpenAIContent(args: {
  content: MCPContent[];
  processMessageContent: (content: MCPContent[]) => string;
  processMultiModalContent: (content: MCPContent[]) => OpenAIMultimodalItem[];
}): string | OpenAI.Chat.Completions.ChatCompletionContentPart[] {
  const multimodal = args.processMultiModalContent(args.content);
  if (multimodal.every((part) => part.type === 'text')) {
    return args.processMessageContent(args.content);
  }
  return multimodal.map((part) => {
    if (part.type === 'text') {
      return { type: 'text', text: part.text || '' };
    } else if (part.type === 'image') {
      const mimeType = part.mimeType || 'image/jpeg';
      return {
        type: 'image_url',
        image_url: { url: `data:${mimeType};base64,${part.image}` },
      };
    } else if (part.type === 'audio') {
      const format = part.mimeType?.includes('wav') ? 'wav' : 'mp3';
      return {
        type: 'input_audio',
        input_audio: { data: part.audio || '', format },
      } as unknown as OpenAI.Chat.Completions.ChatCompletionContentPart;
    }
    return { type: 'text', text: `[Unsupported content: ${part.type}]` };
  });
}

export function convertToOpenAIMessages(args: {
  messages: Message[];
  systemPrompt?: string;
  logger: OpenAILoggerLike;
  processMessageContent: (content: MCPContent[]) => string;
  processMultiModalContent: (content: MCPContent[]) => OpenAIMultimodalItem[];
  extractMediaContent: (content: MCPContent[]) => MCPContent[];
}): OpenAI.Chat.Completions.ChatCompletionMessageParam[] {
  const openaiMessages: OpenAI.Chat.Completions.ChatCompletionMessageParam[] =
    [];

  let mergedSystemPrompt = args.systemPrompt ?? '';
  const compactSummaries = args.messages
    .filter(isCompactSummaryMessage)
    .map((m) => args.processMessageContent(m.content))
    .filter(Boolean);

  if (compactSummaries.length > 0) {
    mergedSystemPrompt = mergedSystemPrompt
      ? `${mergedSystemPrompt}\n\n${compactSummaries.join('\n\n')}`
      : compactSummaries.join('\n\n');
  }

  if (mergedSystemPrompt) {
    openaiMessages.push({ role: 'system', content: mergedSystemPrompt });
  }

  for (let index = 0; index < args.messages.length; index++) {
    const message = args.messages[index];
    if (isCompactSummaryMessage(message)) {
      continue;
    }
    const effectiveRole = message.source === 'ui' ? 'user' : message.role;

    if (effectiveRole === 'user') {
      openaiMessages.push({
        role: 'user',
        content: formatOpenAIContent({
          content: message.content,
          processMessageContent: args.processMessageContent,
          processMultiModalContent: args.processMultiModalContent,
        }),
      });
    } else if (effectiveRole === 'assistant') {
      if (message.tool_calls && message.tool_calls.length > 0) {
        openaiMessages.push({
          role: 'assistant',
          content: args.processMessageContent(message.content) || null,
          tool_calls: message.tool_calls,
        });
      } else {
        openaiMessages.push({
          role: 'assistant',
          content: args.processMessageContent(message.content),
        });
      }
    } else if (effectiveRole === 'tool') {
      if (message.tool_call_id) {
        const media = args.extractMediaContent(message.content as MCPContent[]);
        let toolContent = formatToolResultForLlm(message);
        if (media.length > 0) {
          const imageCount = media.filter((m) => m.type === 'image').length;
          const audioCount = media.filter((m) => m.type === 'audio').length;
          const parts: string[] = [];
          if (toolContent) parts.push(toolContent);
          if (imageCount > 0)
            parts.push(`[Image output: ${imageCount} file(s)]`);
          if (audioCount > 0)
            parts.push(`[Audio output: ${audioCount} file(s)]`);
          toolContent = parts.join('\n\n');
        }

        openaiMessages.push({
          role: 'tool',
          tool_call_id: message.tool_call_id,
          content: toolContent,
        });

        if (media.length > 0) {
          let toolName: string | undefined;
          const startIndex = index - 1;
          for (let i = startIndex; i >= 0; i--) {
            const m = args.messages[i];
            if (m.role === 'assistant' && m.tool_calls) {
              const tc = m.tool_calls.find(
                (t) => t.id === message.tool_call_id,
              );
              if (tc) {
                toolName = tc.function.name;
                break;
              }
            }
          }
          const annotationText = toolName
            ? `[Image/Audio output from tool "${toolName}" (ID: ${message.tool_call_id}). This is the result of your own tool execution, not a user request.]`
            : `[Image/Audio output from tool (ID: ${message.tool_call_id}). This is the result of your own tool execution, not a user request.]`;
          const annotatedMedia: MCPContent[] = [
            {
              type: 'text',
              text: annotationText,
            },
            ...media,
          ];
          openaiMessages.push({
            role: 'user',
            content: formatOpenAIContent({
              content: annotatedMedia,
              processMessageContent: args.processMessageContent,
              processMultiModalContent: args.processMultiModalContent,
            }),
          });
        }
      } else {
        args.logger.warn(
          `Tool message missing tool_call_id: ${JSON.stringify(message)}`,
        );
      }
    }
  }

  return openaiMessages;
}
