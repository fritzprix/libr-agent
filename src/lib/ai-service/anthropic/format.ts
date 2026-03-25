import type {
  ImageBlockParam,
  MessageParam as AnthropicMessageParam,
  TextBlockParam,
} from '@anthropic-ai/sdk/resources/messages.mjs';
import type { MCPContent, MCPImageContent } from '@/lib/mcp';
import { processMessageContent, processMultiModalContent } from '../utils';

export type AnthropicImageMediaType =
  | 'image/jpeg'
  | 'image/png'
  | 'image/gif'
  | 'image/webp';

export function normalizeAnthropicImageMediaType(
  rawMimeType: string | undefined,
): AnthropicImageMediaType | null {
  if (!rawMimeType) {
    return null;
  }

  const mimeType = rawMimeType.toLowerCase();
  if (mimeType === 'image/jpg') {
    return 'image/jpeg';
  }

  if (
    mimeType === 'image/jpeg' ||
    mimeType === 'image/png' ||
    mimeType === 'image/gif' ||
    mimeType === 'image/webp'
  ) {
    return mimeType;
  }

  return null;
}

export function formatAnthropicContent(
  content: MCPContent[],
): AnthropicMessageParam['content'] {
  const multimodal = processMultiModalContent(content);
  if (multimodal.every((part) => part.type === 'text')) {
    return processMessageContent(content);
  }

  return multimodal.map((part) => {
    if (part.type === 'text') {
      return { type: 'text', text: part.text || '' };
    }

    if (part.type === 'image') {
      return {
        type: 'image',
        source: {
          type: 'base64',
          media_type: part.mimeType || 'image/jpeg',
          data: part.image || '',
        },
      };
    }

    return {
      type: 'text',
      text: `[Unsupported content format for Anthropic: ${part.type}]`,
    };
  }) as AnthropicMessageParam['content'];
}

export function buildAnthropicToolResultBlocks(
  content: MCPContent[],
  toolCallId: string,
  messageId: string,
  logger: {
    warn: (message: string, ...args: unknown[]) => void;
  },
): {
  content: Array<{
    type: 'tool_result';
    tool_use_id: string;
    content: string | Array<TextBlockParam | ImageBlockParam>;
  }>;
} {
  const textContent = processMessageContent(content);
  const images = content.filter(
    (item): item is MCPImageContent => item.type === 'image',
  );

  if (images.length === 0) {
    return {
      content: [
        {
          type: 'tool_result',
          tool_use_id: toolCallId,
          content: textContent,
        },
      ],
    };
  }

  const imageBlocks = images
    .map((image): ImageBlockParam | null => {
      const legacySource =
        'source' in image &&
        typeof image.source === 'object' &&
        image.source !== null
          ? (image.source as { data?: string; mimeType?: string })
          : undefined;
      const data = image.data ?? legacySource?.data;
      const mediaType = normalizeAnthropicImageMediaType(
        image.mimeType ?? legacySource?.mimeType,
      );

      if (!data || !mediaType) {
        logger.warn(
          'Skipping Anthropic tool-result image with missing data or unsupported MIME type',
          {
            messageId,
            toolCallId,
            hasData: Boolean(data),
            mimeType: image.mimeType ?? legacySource?.mimeType,
          },
        );
        return null;
      }

      return {
        type: 'image',
        source: {
          type: 'base64',
          media_type: mediaType,
          data,
        },
      };
    })
    .filter((block): block is ImageBlockParam => block !== null);

  if (imageBlocks.length === 0) {
    const placeholderText = textContent
      ? `${textContent}\n\n[Tool returned image(s) that could not be displayed due to unsupported format or missing data.]`
      : '[Tool returned image(s) that could not be displayed due to unsupported format or missing data.]';

    return {
      content: [
        {
          type: 'tool_result',
          tool_use_id: toolCallId,
          content: placeholderText,
        },
      ],
    };
  }

  const blocks: Array<TextBlockParam | ImageBlockParam> = [];
  if (textContent) {
    blocks.push({ type: 'text', text: textContent });
  }
  blocks.push(...imageBlocks);

  return {
    content: [
      {
        type: 'tool_result',
        tool_use_id: toolCallId,
        content: blocks,
      },
    ],
  };
}
