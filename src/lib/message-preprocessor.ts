import { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';
import { getLogger } from './logger';
import { stringToMCPContentArray } from './utils';
import type { AttachmentReference } from '@/models/chat';

const logger = getLogger('message-preprocessor');
const ATTACHMENT_PREVIEW_CHAR_LIMIT = 500;

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
export async function prepareMessageForLLM(message: Message): Promise<Message> {
  // If there are no attachments, no preprocessing is needed.
  if (!message.attachments || message.attachments.length === 0) {
    return message;
  }

  logger.debug('Preprocessing message with attachments', {
    messageId: message.id,
    attachmentCount: message.attachments.length,
  });

  try {
    // Separate inline (image/audio) from text/workspace attachments.
    // Inline attachments must be injected into message.content; they are NOT
    // already there in the agent V2 path (Rust stores and returns them only in
    // the attachments field).
    const inlineAttachments = message.attachments.filter(
      (a) => a.status === 'inline' && !!a.inlineContent,
    );
    const textAttachments = message.attachments.filter(
      (a) => a.status !== 'inline',
    );

    // Build MCPContent blocks for inline image/audio attachments
    const inlineContentBlocks: MCPContent[] = inlineAttachments.map((a) => {
      if (a.inlineContent!.type === 'image') {
        return {
          type: 'image' as const,
          data: a.inlineContent!.data,
          mimeType: a.inlineContent!.mimeType,
        };
      }
      return {
        type: 'audio' as const,
        data: a.inlineContent!.data,
        mimeType: a.inlineContent!.mimeType,
      };
    });

    // Build text hint blocks for workspace/committed attachments
    const attachmentHintBlocks = textAttachments.map((attachment, i) => {
      const safeAttachment = createAttachmentHintPayload(attachment);
      const accessHints = attachment.contentId
        ? `To read the full content of this file, use:
- read(sessionId: "${attachment.sessionId}", contentId: "${attachment.contentId}", lineRange: {fromLine: 1, toLine: 200})
- For keyword search: search(sessionId: "${attachment.sessionId}", query: "your search query")
- For file list: list(sessionId: "${attachment.sessionId}")`
        : attachment.workspacePath
          ? `This file is in your workspace (may not be indexed in content store yet):
- To read it via workspace: workspace__readFile(path: "${attachment.workspacePath}")
- To check if it has been indexed: list(sessionId: "${attachment.sessionId}")
- If listed, use read(sessionId: "${attachment.sessionId}", contentId: <id from list>)`
          : `File metadata only — use list(sessionId: "${attachment.sessionId}") to find available files`;

      return `<attachment_${i}>
${JSON.stringify(safeAttachment, null, 2)}
<!--
${accessHints}
-->
</attachment_${i}>`;
    });

    const hasInline = inlineContentBlocks.length > 0;
    const hasText = attachmentHintBlocks.length > 0;

    if (!hasInline && !hasText) {
      return message;
    }

    const processedMessage: Message = {
      ...message,
      content: [
        ...message.content,
        ...inlineContentBlocks,
        ...(hasText
          ? stringToMCPContentArray(attachmentHintBlocks.join('\n\n'))
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
  const processedMessages = await Promise.all(
    messages.map((message) => prepareMessageForLLM(message)),
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
    });
  }

  return processedMessages;
}
