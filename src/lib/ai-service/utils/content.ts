import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';
import { safeJsonStringify, tryParse } from './general';

type MediaItem = {
  data?: string;
  uri?: string;
  mimeType?: string;
  source?: { data?: string; uri?: string; mimeType?: string };
};

export function processMessageContent(content: string | MCPContent[]): string {
  if (typeof content === 'string') {
    return content;
  }
  if (!Array.isArray(content)) {
    return '';
  }

  return content
    .filter((item) => item.type === 'text')
    .map((item) => (item as { text: string }).text)
    .join('\n');
}

export function extractStructuredToolResult(
  message: Message,
): Record<string, unknown> | null {
  const metadata = message.metadata;
  if (typeof metadata !== 'object' || metadata === null) {
    return null;
  }

  const structuredContent = metadata.structuredContent;
  if (
    typeof structuredContent !== 'object' ||
    structuredContent === null ||
    Array.isArray(structuredContent)
  ) {
    return null;
  }

  return structuredContent as Record<string, unknown>;
}

export function formatToolResultForLlm(message: Message): string {
  const structuredResult = extractStructuredToolResult(message);
  if (structuredResult) {
    return safeJsonStringify(structuredResult);
  }

  return processMessageContent(message.content);
}

export function parseToolResultForLlm(
  message: Message,
): Record<string, unknown> {
  const structuredResult = extractStructuredToolResult(message);
  if (structuredResult) {
    return structuredResult;
  }

  const text = processMessageContent(message.content);
  const parsed = tryParse<Record<string, unknown>>(text);
  if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
    return parsed;
  }

  return { result: text };
}

export function processMultiModalContent(content: MCPContent[]): Array<{
  type: string;
  text?: string;
  image?: string;
  audio?: string;
  mimeType?: string;
}> {
  return content.map((item) => {
    switch (item.type) {
      case 'text':
        return { type: 'text', text: (item as { text: string }).text };
      case 'image': {
        const mediaItem = item as MediaItem;
        const data = mediaItem.data || mediaItem.source?.data;
        if (data) {
          return {
            type: 'image',
            image: data,
            mimeType: mediaItem.mimeType || mediaItem.source?.mimeType,
          };
        }

        const uri = mediaItem.uri || mediaItem.source?.uri;
        return {
          type: 'text',
          text: `[unresolved image omitted from multimodal request: ${uri || 'missing-uri'}]`,
        };
      }
      case 'audio': {
        const mediaItem = item as MediaItem;
        const data = mediaItem.data || mediaItem.source?.data;
        if (data) {
          return {
            type: 'audio',
            audio: data,
            mimeType: mediaItem.mimeType || mediaItem.source?.mimeType,
          };
        }

        const uri = mediaItem.uri || mediaItem.source?.uri;
        return {
          type: 'text',
          text: `[unresolved audio omitted from multimodal request: ${uri || 'missing-uri'}]`,
        };
      }
      default:
        return { type: 'text', text: `[${item.type}]` };
    }
  });
}

export function extractMediaContent(content: MCPContent[]): MCPContent[] {
  return content.filter((c) => c.type === 'image' || c.type === 'audio');
}
