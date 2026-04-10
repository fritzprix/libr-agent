import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp';

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

export function formatToolResultForLlm(message: Message): string {
  return processMessageContent(message.content);
}

export function parseToolResultForLlm(
  message: Message,
): Record<string, unknown> {
  return { result: processMessageContent(message.content) };
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
