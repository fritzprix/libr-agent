import { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';
import { getLogger } from './logger';
import { stringToMCPContentArray } from './utils';
import type { AttachmentAgentAccess, AttachmentReference } from '@/models/chat';
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
  const preview =
    typeof attachment.preview === 'string' &&
    attachment.preview.length > ATTACHMENT_PREVIEW_CHAR_LIMIT
      ? truncateText(attachment.preview, ATTACHMENT_PREVIEW_CHAR_LIMIT)
      : attachment.preview;

  return {
    ...attachment,
    preview,
    agentAccess: deriveAttachmentAgentAccess(attachment),
  };
}

function isWorkspaceTextReadable(attachment: AttachmentReference): boolean {
  if (
    /^text\/|\/(json|xml|javascript|typescript)/.test(attachment.mimeType) ||
    /\.(txt|md|markdown|json|jsonc|json5|yaml|yml|toml|js|jsx|ts|tsx|mjs|cjs|py|rb|rs|go|java|c|cpp|h|hpp|css|scss|less|html|htm|svg|sh|bash|zsh|fish|ps1|sql|graphql|csv|log|xml|proto)$/i.test(
      attachment.filename,
    )
  ) {
    return true;
  }

  return false;
}

function deriveAttachmentAgentAccess(
  attachment: AttachmentReference,
): AttachmentAgentAccess {
  if (attachment.agentAccess) {
    return attachment.agentAccess;
  }

  if (attachment.contentId || attachment.status === 'committed') {
    return {
      mode: 'indexed',
      reason: 'indexed',
      note: 'Indexed in the attachments store. Use attachments tools such as list/read/search.',
    };
  }

  if (attachment.status === 'inline' || attachment.inlineContent) {
    return {
      mode: 'inline-media',
      reason: 'inline_media',
      note: 'Inline media attachment. Use the media payload already present in the message instead of attachments or workspace text tools.',
    };
  }

  if (attachment.workspacePath) {
    return isWorkspaceTextReadable(attachment)
      ? {
          mode: 'workspace-text',
          reason: 'workspace_only',
          note: 'Workspace-only attachment. attachments tools will not find it; use workspace__readFile if you need the text content.',
        }
      : {
          mode: 'workspace-binary',
          reason: 'workspace_only',
          note: 'Workspace-only binary/media attachment. attachments tools will not find it, and workspace__readFile is not appropriate.',
        };
  }

  return {
    mode: 'metadata-only',
    reason: 'metadata_only',
    note: 'Only metadata is available. Do not assume this attachment is readable until the storage mode is clarified.',
  };
}

function buildAttachmentGuidanceLines(
  attachment: AttachmentReference,
  access: AttachmentAgentAccess,
): string[] {
  const lines = [
    'Agent guidance:',
    `- Access mode: ${access.mode}`,
    `- Reason: ${access.reason}`,
    `- ${access.note}`,
  ];

  if (attachment.workspacePath) {
    lines.push(`- Workspace path: ${attachment.workspacePath}`);
  }

  switch (access.mode) {
    case 'indexed':
      if (attachment.contentId) {
        lines.push(
          '- Valid tools: attachments list/read/search',
          `- Read full content: read(contentId: "${attachment.contentId}", fromLine: 1, toLine: 200)`,
          '- Search indexed content: search(query: "your search query")',
          '- List indexed attachments: list()',
        );
      }
      break;
    case 'workspace-text':
      if (attachment.workspacePath) {
        lines.push(
          '- attachments tools: do not use them for this file',
          `- Read text via workspace: workspace__readFile(path: "${attachment.workspacePath}")`,
        );
      }
      break;
    case 'workspace-binary':
      lines.push(
        '- attachments tools: do not use them for this file',
        '- workspace__readFile: do not use it; this is binary/non-text',
        '- Refer to the file by filename/path or use a media/specialized tool if one exists',
      );
      break;
    case 'inline-media':
      lines.push(
        '- The media payload is already part of the message content',
        '- Do not call attachments tools or workspace__readFile for this attachment',
      );
      break;
    case 'metadata-only':
      lines.push(
        '- File metadata only — do not guess a read path',
        '- Clarify storage mode before attempting tool calls',
      );
      break;
  }

  return lines;
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
  const access = deriveAttachmentAgentAccess(attachment);
  return buildHistoricalMediaSummary(index, {
    kind: source?.type ?? 'unknown',
    filename: attachment.filename,
    mimeType: attachment.mimeType,
    size: attachment.size,
    uploadedAt: attachment.uploadedAt,
    uri: source?.uri,
    hasInlineBytes: typeof source?.data === 'string' && source.data.length > 0,
    agentAccess: access,
    note: 'Inline media history item. Do not call attachments tools or workspace__readFile for it.',
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
    note: 'Historical media item. Do not call attachments tools or workspace__readFile for it.',
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
  // ⚡ Bolt: Replace .reduce() with a loop to reduce per-element callback overhead on hot paths.
  let total = 0;
  for (const item of content) {
    switch (item.type) {
      case 'text':
        total += estimateTextTokens(item.text);
        break;
      case 'resource':
        total += estimateTextTokens(
          typeof item.resource?.text === 'string' ? item.resource.text : '',
        );
        break;
      case 'tool_call':
        total += estimateTextTokens(
          `${item.name ?? ''} ${item.arguments ?? ''}`,
        );
        break;
      case 'thinking':
        total += estimateTextTokens(item.thinking ?? '');
        break;
      case 'image':
      case 'audio':
        total += 1000;
        break;
      default:
        break;
    }
  }
  return total;
}

export function estimateMessageTokens(message: Message): number {
  let total = estimateTextTokens(message.role);
  total += estimateMCPContentTokens(message.content);

  if (message.tool_calls) {
    // ⚡ Bolt: Replace .reduce() with for-loop for better token estimation performance on hot paths
    for (const toolCall of message.tool_calls) {
      total += estimateTextTokens(
        `${toolCall.function?.name ?? ''} ${toolCall.function?.arguments ?? ''}`,
      );
    }
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

  // ⚡ Bolt: Replace .reduce() with for-loop for better token estimation performance on hot paths
  let messageTokens = 0;
  for (const message of messages) {
    messageTokens += estimateMessageTokens(message);
  }

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
      const access = deriveAttachmentAgentAccess(safeAttachment);
      const guidance = buildAttachmentGuidanceLines(
        safeAttachment,
        access,
      ).join('\n');

      return `<attachment_${i}>
${JSON.stringify(safeAttachment, null, 2)}
${guidance}
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
  // ⚡ Bolt: Single O(N) backwards pass to compute all metadata in one scan
  let latestMediaMessageIndex = -1;
  let attachmentCount = 0;
  let errorMessageCount = 0;

  for (let i = messages.length - 1; i >= 0; i--) {
    const msg = messages[i];

    if (latestMediaMessageIndex === -1 && messageHasMedia(msg)) {
      latestMediaMessageIndex = i;
    }

    attachmentCount += msg.attachments?.length || 0;

    if (msg.error) {
      errorMessageCount++;
    }
  }

  const processedMessages = await Promise.all(
    messages.map((message, index) =>
      prepareMessageForLLM(message, {
        includeLatestMediaPayload: index === latestMediaMessageIndex,
      }),
    ),
  );

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
