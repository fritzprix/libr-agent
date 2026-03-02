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
      // Generate tool-call hints based on whether the file is in the Content Store or workspace-only
      const accessHints = attachment.contentId
        ? `To read the full content of this file, use:
- readContent(sessionId: "${attachment.sessionId}", contentId: "${attachment.contentId}", lineRange: {fromLine: 1, toLine: 200})
- For keyword search: searchContent(sessionId: "${attachment.sessionId}", query: "your search query")
- For file list: listContent(sessionId: "${attachment.sessionId}")`
        : attachment.workspacePath
          ? `This file is in your workspace (may not be indexed in content store yet):
- To read it via workspace: builtin_workspace__readFile(path: "${attachment.workspacePath}")
- To check if it has been indexed: listContent(sessionId: "${attachment.sessionId}")
- If listed, use readContent(sessionId: "${attachment.sessionId}", contentId: <id from listContent>)`
          : `File metadata only — use listContent(sessionId: "${attachment.sessionId}") to find available files`;

      return `<attachment_${i}>
${JSON.stringify(attachment, null, 2)}
<!--
${accessHints}
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
