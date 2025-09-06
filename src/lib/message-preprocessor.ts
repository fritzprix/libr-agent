import { Message } from '@/models/chat';
import { getLogger } from './logger';
import { stringToMCPContentArray } from './utils';

const logger = getLogger('message-preprocessor');

/**
 * Type guard to check if an MCPContent item has text property
 */


/**
 * Prepares a message for LLM by including attachment metadata and content-store tool usage guides
 */
export async function prepareMessageForLLM(message: Message): Promise<Message> {
  // If no attachments, just normalize content
  if (!message.attachments || message.attachments.length === 0) {
    return message
  }

  logger.debug('Preprocessing message with attachments', {
    messageId: message.id,
    attachmentCount: message.attachments.length,
  });

  try {
    // Generate attachment content blocks
    const attachmentContents = message.attachments.map((attachment, i) => {
      return `<attachment_${i}>
${JSON.stringify(attachment, null, 2)}
<!-- 
To read the full content of this file, use:
- readContent(storeId: "${attachment.storeId}", contentId: "${attachment.contentId}", lineRange: {fromLine: 1, toLine: 200})
- For keyword-based similarity search: keywordSimilaritySearch(storeId: "${attachment.storeId}", query: "your search query")
- For file list: listContent(storeId: "${attachment.storeId}")
-->
</attachment_${i}>`;
    });

    // Normalize content for LLM and combine with attachment information
    const processedMessage: Message = {
      ...message,
      content: [
        ...message.content,
        ...stringToMCPContentArray(attachmentContents.join("\n\n"))
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
 * Processes multiple messages for LLM consumption
 */
export async function prepareMessagesForLLM(
  messages: Message[],
): Promise<Message[]> {
  const processedMessages = await Promise.all(
    messages.map((message) => prepareMessageForLLM(message)),
  );

  const attachmentCount = messages.reduce(
    (total, msg) => total + (msg.attachments?.length || 0),
    0,
  );

  if (attachmentCount > 0) {
    logger.info('Processed messages with attachments', {
      totalMessages: messages.length,
      totalAttachments: attachmentCount,
    });
  }

  return processedMessages;
}
