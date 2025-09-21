import { Message } from '@/models/chat';
import { getLogger } from './logger';
import { stringToMCPContentArray } from './utils';

const logger = getLogger('message-preprocessor');

/**
 * Prepares a single message for consumption by an LLM.
 * If the message has attachments, it enriches the message content with metadata
 * about each attachment and provides a guide on how to use tools to access the
 * full content of the attachments. This helps the LLM understand what files are
 * available and how to interact with them.
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
        ...stringToMCPContentArray(attachmentContents.join('\n\n')),
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
