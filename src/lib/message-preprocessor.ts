import { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';
import { getLogger } from './logger';
import { stringToMCPContentArray } from './utils';
import type { AttachmentReference } from '@/models/chat';
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';

const logger = getLogger('message-preprocessor');
const ATTACHMENT_PREVIEW_CHAR_LIMIT = 500;
const MEDIA_CACHE_MAX_BYTES = 64 * 1024 * 1024;

type MediaContentLike = Extract<MCPContent, { type: 'image' | 'audio' }>;

const mediaBase64Cache = new Map<
  string,
  {
    data: string;
    size: number;
  }
>();
let mediaBase64CacheBytes = 0;

function truncateText(value: string, maxLength: number): string {
  if (value.length <= maxLength) {
    return value;
  }

  return `${value.slice(0, maxLength)}…`;
}

function createAttachmentHintPayload(
  attachment: AttachmentReference,
): AttachmentReference {
  if (
    typeof attachment.preview === 'string' &&
    attachment.preview.length > ATTACHMENT_PREVIEW_CHAR_LIMIT
  ) {
    return {
      ...attachment,
      preview: truncateText(attachment.preview, ATTACHMENT_PREVIEW_CHAR_LIMIT),
    };
  }

  return attachment;
}

function estimateTextTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

function estimateBase64Bytes(value: string): number {
  return Math.floor((value.length * 3) / 4);
}

function pruneMediaCache(maxBytes: number): void {
  while (mediaBase64CacheBytes > maxBytes && mediaBase64Cache.size > 0) {
    const oldestKey = mediaBase64Cache.keys().next().value;
    if (!oldestKey) {
      break;
    }
    const entry = mediaBase64Cache.get(oldestKey);
    if (!entry) {
      mediaBase64Cache.delete(oldestKey);
      continue;
    }
    mediaBase64CacheBytes -= entry.size;
    mediaBase64Cache.delete(oldestKey);
  }
}

function updateMediaCache(key: string, data: string): void {
  const size = estimateBase64Bytes(data);
  const existing = mediaBase64Cache.get(key);
  if (existing) {
    mediaBase64CacheBytes -= existing.size;
    mediaBase64Cache.delete(key);
  }
  mediaBase64Cache.set(key, { data, size });
  mediaBase64CacheBytes += size;
  pruneMediaCache(MEDIA_CACHE_MAX_BYTES);
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== 'string') {
        reject(new Error('Unexpected FileReader result type'));
        return;
      }
      const [, base64 = ''] = result.split(',', 2);
      resolve(base64);
    };
    reader.onerror = () => {
      reject(reader.error ?? new Error('Failed to read media blob'));
    };
    reader.readAsDataURL(blob);
  });
}

async function loadBase64FromUri(
  uri: string,
  sessionId: string,
): Promise<string> {
  if (uri.startsWith('data:')) {
    const [, base64 = ''] = uri.split(',', 2);
    return base64;
  }

  const cached = mediaBase64Cache.get(uri);
  if (cached) {
    mediaBase64Cache.delete(uri);
    mediaBase64Cache.set(uri, cached);
    return cached.data;
  }

  if (uri.startsWith('file://')) {
    const base64 = await readLocalFileAsBase64(sessionId, uri);
    updateMediaCache(uri, base64);
    return base64;
  }

  const response = await fetch(uri);
  if (!response.ok) {
    throw new Error(
      `Failed to fetch media URI "${uri}": ${response.status} ${response.statusText}`,
    );
  }

  const base64 = await blobToBase64(await response.blob());
  updateMediaCache(uri, base64);
  return base64;
}

function isMediaContentItem(content: MCPContent): content is MediaContentLike {
  return content.type === 'image' || content.type === 'audio';
}

function messageHasMedia(message: Message): boolean {
  if (message.content.some((item) => isMediaContentItem(item))) {
    return true;
  }

  return (
    message.attachments?.some(
      (attachment) =>
        attachment.status === 'inline' && !!attachment.inlineContent,
    ) ?? false
  );
}

function buildHistoricalMediaSummary(
  index: number,
  payload: Record<string, unknown>,
): string {
  return `<historical_media_${index}>
${JSON.stringify(payload, null, 2)}
</historical_media_${index}>`;
}

function summarizeInlineAttachment(
  attachment: AttachmentReference,
  index: number,
): string {
  const source = attachment.inlineContent;
  return buildHistoricalMediaSummary(index, {
    kind: source?.type ?? 'unknown',
    filename: attachment.filename,
    mimeType: attachment.mimeType,
    size: attachment.size,
    uploadedAt: attachment.uploadedAt,
    uri: source?.uri,
    hasInlineBytes: typeof source?.data === 'string' && source.data.length > 0,
  });
}

function summarizeHistoricalMediaItem(
  item: MediaContentLike,
  index: number,
): string {
  const source = item.source;
  const rawData = item.data ?? source?.data;
  const uri = item.uri ?? source?.uri;

  return buildHistoricalMediaSummary(index, {
    kind: item.type,
    mimeType: item.mimeType ?? source?.mimeType,
    uri,
    embeddedBytes: rawData ? estimateBase64Bytes(rawData) : undefined,
  });
}

async function materializeInlineAttachment(
  attachment: AttachmentReference,
  sessionId: string,
): Promise<MCPContent | null> {
  const inlineContent = attachment.inlineContent;
  if (!inlineContent) {
    return null;
  }

  const base64 =
    inlineContent.data ??
    (inlineContent.uri
      ? await loadBase64FromUri(inlineContent.uri, sessionId)
      : undefined);

  if (!base64) {
    return null;
  }

  if (inlineContent.type === 'image') {
    return {
      type: 'image',
      data: base64,
      mimeType: inlineContent.mimeType,
    };
  }

  return {
    type: 'audio',
    data: base64,
    mimeType: inlineContent.mimeType,
  };
}

async function materializeMediaContentItem(
  item: MediaContentLike,
  sessionId: string,
): Promise<MCPContent | null> {
  const source = item.source;
  let base64 = item.data ?? source?.data;

  if (!base64 && item.uri) {
    base64 = await loadBase64FromUri(item.uri, sessionId);
  }

  if (!base64 && source?.uri) {
    base64 = await loadBase64FromUri(source.uri, sessionId);
  }

  if (!base64) {
    return null;
  }

  if (item.type === 'image') {
    return {
      type: 'image',
      data: base64,
      mimeType: item.mimeType ?? source?.mimeType ?? 'image/png',
    };
  }

  return {
    type: 'audio',
    data: base64,
    mimeType: item.mimeType ?? source?.mimeType ?? 'audio/mpeg',
  };
}

export function calculateContextSafetyMargin(effectiveLimit: number): number {
  const fivePercent = Math.ceil(effectiveLimit * 0.05);
  return Math.min(Math.max(fivePercent, 1024), 8192);
}

export function estimateMCPContentTokens(content: MCPContent[]): number {
  return content.reduce((total, item) => {
    switch (item.type) {
      case 'text':
        return total + estimateTextTokens(item.text);
      case 'resource':
        return (
          total +
          estimateTextTokens(
            typeof item.resource?.text === 'string' ? item.resource.text : '',
          )
        );
      case 'tool_call':
        return (
          total +
          estimateTextTokens(`${item.name ?? ''} ${item.arguments ?? ''}`)
        );
      case 'thinking':
        return total + estimateTextTokens(item.thinking ?? '');
      case 'image':
      case 'audio':
        return total + 1000;
      default:
        return total;
    }
  }, 0);
}

export function estimateMessageTokens(message: Message): number {
  let total = estimateTextTokens(message.role);
  total += estimateMCPContentTokens(message.content);

  if (message.tool_calls) {
    total += message.tool_calls.reduce(
      (sum, toolCall) =>
        sum +
        estimateTextTokens(
          `${toolCall.function?.name ?? ''} ${toolCall.function?.arguments ?? ''}`,
        ),
      0,
    );
  }

  if (message.tool_use) {
    total += estimateTextTokens(JSON.stringify(message.tool_use));
  }

  if (message.thinking) {
    total += estimateTextTokens(message.thinking);
  }

  return total;
}

export function estimatePayloadTokens(
  systemPrompt: string | undefined,
  messages: Message[],
  availableTools: unknown[] | undefined,
): number {
  const promptTokens = systemPrompt ? estimateTextTokens(systemPrompt) : 0;
  const messageTokens = messages.reduce(
    (sum, message) => sum + estimateMessageTokens(message),
    0,
  );
  const toolTokens = availableTools
    ? estimateTextTokens(JSON.stringify(availableTools))
    : 0;

  return promptTokens + messageTokens + toolTokens;
}

/**
 * Prepares a single message for consumption by an LLM.
 * If the message has attachments, it enriches the message content with metadata
 * about each attachment and provides a guide on how to use tools to access the
 * full content of the attachments. This helps the LLM understand what files are
 * available and how to interact with them.
 *
 * Inline attachments (image/audio with status='inline') are injected directly
 * into message.content as MCPImageContent/MCPAudioContent blocks so the LLM
 * receives them as multimodal input.
 *
 * @param message The message to preprocess.
 * @returns A promise that resolves to the processed message, ready for the LLM.
 *          If an error occurs, it returns the original message as a fallback.
 */
export async function prepareMessageForLLM(
  message: Message,
  options?: {
    includeLatestMediaPayload?: boolean;
  },
): Promise<Message> {
  if (
    (!message.attachments || message.attachments.length === 0) &&
    !message.content.some((item) => isMediaContentItem(item))
  ) {
    return message;
  }

  logger.debug('Preprocessing message with attachments', {
    messageId: message.id,
    attachmentCount: message.attachments?.length ?? 0,
    includeLatestMediaPayload: options?.includeLatestMediaPayload ?? true,
  });

  try {
    // Separate inline (image/audio) from text/workspace attachments.
    // Inline attachments must be injected into message.content; they are NOT
    // already there in the agent V2 path (Rust stores and returns them only in
    // the attachments field).
    const inlineAttachments =
      message.attachments?.filter(
        (a) => a.status === 'inline' && !!a.inlineContent,
      ) ?? [];
    const textAttachments =
      message.attachments?.filter((a) => a.status !== 'inline') ?? [];

    const includeLatestMediaPayload =
      options?.includeLatestMediaPayload ?? true;

    const mediaSummaries: string[] = [];
    const processedContent: MCPContent[] = [];
    let historicalMediaIndex = 0;

    for (const contentItem of message.content) {
      if (!isMediaContentItem(contentItem)) {
        processedContent.push(contentItem);
        continue;
      }

      if (includeLatestMediaPayload) {
        const materializedItem = await materializeMediaContentItem(
          contentItem,
          message.sessionId,
        );
        if (materializedItem) {
          processedContent.push(materializedItem);
          continue;
        }
      }

      mediaSummaries.push(
        summarizeHistoricalMediaItem(contentItem, historicalMediaIndex++),
      );
    }

    const inlineContentBlocks: MCPContent[] = [];
    for (const attachment of inlineAttachments) {
      if (includeLatestMediaPayload) {
        const materializedAttachment = await materializeInlineAttachment(
          attachment,
          message.sessionId,
        );
        if (materializedAttachment) {
          inlineContentBlocks.push(materializedAttachment);
          continue;
        }
      }

      mediaSummaries.push(
        summarizeInlineAttachment(attachment, historicalMediaIndex++),
      );
    }

    // Build text hint blocks for workspace/committed attachments
    const attachmentHintBlocks = textAttachments.map((attachment, i) => {
      const safeAttachment = createAttachmentHintPayload(attachment);
      const accessHints = attachment.contentId
        ? `To read the full content of this file, use:
- read(contentId: "${attachment.contentId}", fromLine: 1, toLine: 200)
- For keyword search: search(query: "your search query")
- For file list: list()`
        : attachment.workspacePath
          ? `This file is in your workspace (may not be indexed in content store yet):
- To read it via workspace: workspace__readFile(path: "${attachment.workspacePath}")
- To check if it has been indexed: list()
- If listed, use read(contentId: <id from list>, fromLine: 1, toLine: 200)`
          : `File metadata only — use list() to find available files`;

      return `<attachment_${i}>
${JSON.stringify(safeAttachment, null, 2)}
<!--
${accessHints}
-->
</attachment_${i}>`;
    });

    const hasInline = inlineContentBlocks.length > 0;
    const hasText =
      attachmentHintBlocks.length > 0 || mediaSummaries.length > 0;
    const contentChanged =
      processedContent.length !== message.content.length ||
      processedContent.some((item, index) => item !== message.content[index]);

    if (!hasInline && !hasText && !contentChanged) {
      return message;
    }

    const processedMessage: Message = {
      ...message,
      content: [
        ...processedContent,
        ...inlineContentBlocks,
        ...(hasText
          ? stringToMCPContentArray(
              [...mediaSummaries, ...attachmentHintBlocks].join('\n\n'),
            )
          : []),
      ],
    };

    return processedMessage;
  } catch (error) {
    logger.error('Failed to preprocess message', {
      messageId: message.id,
      error: error instanceof Error ? error.message : String(error),
    });

    // Return original message as fallback
    return message;
  }
}

/**
 * Preprocesses an array of messages for consumption by an LLM.
 * It iterates through the messages and applies the `prepareMessageForLLM` function to each one.
 *
 * @param messages The array of messages to preprocess.
 * @returns A promise that resolves to an array of processed messages.
 */
export async function prepareMessagesForLLM(
  messages: Message[],
): Promise<Message[]> {
  // IMPORTANT: Do NOT filter out error messages!
  // Tool execution errors (Message.error) contain valuable context for the LLM
  // to understand what went wrong and how to recover.
  // The error field is metadata; the content field contains the actual error message
  // that should be sent to the LLM.
  let latestMediaMessageIndex = -1;
  for (let index = messages.length - 1; index >= 0; index--) {
    if (messageHasMedia(messages[index])) {
      latestMediaMessageIndex = index;
      break;
    }
  }

  const processedMessages = await Promise.all(
    messages.map((message, index) =>
      prepareMessageForLLM(message, {
        includeLatestMediaPayload: index === latestMediaMessageIndex,
      }),
    ),
  );

  const attachmentCount = messages.reduce(
    (total, msg) => total + (msg.attachments?.length || 0),
    0,
  );

  const errorMessageCount = messages.filter((msg) => !!msg.error).length;

  if (attachmentCount > 0 || errorMessageCount > 0) {
    logger.info('Processed messages for LLM', {
      totalMessages: messages.length,
      totalAttachments: attachmentCount,
      errorMessages: errorMessageCount,
      latestMediaMessageIndex,
    });
  }

  return processedMessages;
}
