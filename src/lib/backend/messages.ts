import { safeInvoke } from './core';
import type { Message } from '@/models/chat';
import type { Page } from '@/lib/db/types';
import type { MessageSearchResult } from './types';

// ========================================
// Message Management
// ========================================

/**
 * Deserialize a message from the Rust backend format to frontend format.
 * Parses JSON fields and converts timestamps to Date objects.
 */
function deserializeMessage(rustMsg: Record<string, unknown>): Message {
  return {
    id: rustMsg.id as string,
    sessionId: rustMsg.sessionId as string,
    threadId: (rustMsg.threadId as string) || (rustMsg.sessionId as string), // Fallback to sessionId for backward compatibility
    role: rustMsg.role as 'user' | 'assistant' | 'system' | 'tool',
    content: JSON.parse(rustMsg.content as string),
    tool_calls: rustMsg.toolCalls
      ? JSON.parse(rustMsg.toolCalls as string)
      : undefined,
    tool_call_id: rustMsg.toolCallId as string | undefined,
    isStreaming: rustMsg.isStreaming as boolean | undefined,
    thinking: rustMsg.thinking as string | undefined,
    thinkingSignature: rustMsg.thinkingSignature as string | undefined,
    assistantId: rustMsg.assistantId as string | undefined,
    attachments: rustMsg.attachments
      ? JSON.parse(rustMsg.attachments as string)
      : undefined,
    tool_use: rustMsg.toolUse
      ? JSON.parse(rustMsg.toolUse as string)
      : undefined,
    createdAt: new Date(rustMsg.createdAt as number),
    updatedAt: new Date(rustMsg.updatedAt as number),
    source: rustMsg.source as 'assistant' | 'ui' | undefined,
    error: rustMsg.error ? JSON.parse(rustMsg.error as string) : undefined,
  };
}

/**
 * Retrieves a paginated list of messages for a specific thread in a session.
 * @param sessionId The ID of the session
 * @param threadId The ID of the thread (defaults to sessionId for top thread)
 * @param page The page number to retrieve (1-indexed)
 * @param pageSize The number of messages per page
 * @returns A promise that resolves to a Page of messages
 */
export async function getMessagesPageForSession(
  sessionId: string,
  threadId: string,
  page: number,
  pageSize: number,
): Promise<Page<Message>> {
  if (!sessionId || !threadId) {
    throw new Error('sessionId and threadId are required');
  }

  const result = await safeInvoke<Page<Record<string, unknown>>>(
    'messages_get_page',
    {
      sessionId,
      threadId,
      page,
      pageSize,
    },
  );

  // Deserialize messages from Rust format
  return {
    ...result,
    items: result.items.map(deserializeMessage),
  };
}

/**
 * Inserts or updates multiple messages in a single transaction.
 * @param messages An array of messages to upsert
 * @returns A promise that resolves when the operation completes
 */
export async function upsertMessages(messages: Message[]): Promise<void> {
  // Validate all messages have sessionId and threadId
  for (const message of messages) {
    if (!message.sessionId || message.sessionId.trim() === '') {
      throw new Error(
        `Cannot upsert message: missing or empty sessionId for message ${message.id}`,
      );
    }
    if (!message.threadId || message.threadId.trim() === '') {
      throw new Error(
        `Cannot upsert message: missing or empty threadId for message ${message.id}`,
      );
    }
  }

  // Convert Message objects to match Rust expectations
  const rustMessages = messages.map((msg) => ({
    id: msg.id,
    sessionId: msg.sessionId,
    threadId: msg.threadId,
    role: msg.role,
    content: JSON.stringify(msg.content),
    toolCalls: msg.tool_calls ? JSON.stringify(msg.tool_calls) : null,
    toolCallId: msg.tool_call_id || null,
    isStreaming: msg.isStreaming || null,
    thinking: msg.thinking || null,
    thinkingSignature: msg.thinkingSignature || null,
    assistantId: msg.assistantId || null,
    attachments: msg.attachments ? JSON.stringify(msg.attachments) : null,
    toolUse: msg.tool_use ? JSON.stringify(msg.tool_use) : null,
    createdAt: msg.createdAt ? msg.createdAt.getTime() : Date.now(),
    updatedAt: msg.updatedAt ? msg.updatedAt.getTime() : Date.now(),
    source: msg.source || null,
    error: msg.error ? JSON.stringify(msg.error) : null,
  }));

  return safeInvoke<void>('messages_upsert_many', { messages: rustMessages });
}

/**
 * Inserts or updates a single message.
 * @param message The message to upsert
 * @returns A promise that resolves when the operation completes
 */
export async function upsertMessage(message: Message): Promise<void> {
  // Validate message has sessionId
  if (!message.sessionId || message.sessionId.trim() === '') {
    throw new Error(
      `Cannot upsert message: missing or empty sessionId for message ${message.id}`,
    );
  }

  const rustMessage = {
    id: message.id,
    sessionId: message.sessionId,
    role: message.role,
    content: JSON.stringify(message.content),
    toolCalls: message.tool_calls ? JSON.stringify(message.tool_calls) : null,
    toolCallId: message.tool_call_id || null,
    isStreaming: message.isStreaming || null,
    thinking: message.thinking || null,
    thinkingSignature: message.thinkingSignature || null,
    assistantId: message.assistantId || null,
    attachments: message.attachments
      ? JSON.stringify(message.attachments)
      : null,
    toolUse: message.tool_use ? JSON.stringify(message.tool_use) : null,
    createdAt: message.createdAt ? message.createdAt.getTime() : Date.now(),
    updatedAt: message.updatedAt ? message.updatedAt.getTime() : Date.now(),
    source: message.source || null,
    error: message.error ? JSON.stringify(message.error) : null,
  };

  return safeInvoke<void>('messages_upsert', { message: rustMessage });
}

/**
 * Deletes a single message by ID.
 * @param messageId The ID of the message to delete
 * @returns A promise that resolves when the operation completes
 */
export async function deleteMessage(messageId: string): Promise<void> {
  return safeInvoke<void>('messages_delete', { messageId });
}

/**
 * Deletes all messages for a specific session.
 * @param sessionId The ID of the session
 * @returns A promise that resolves when the operation completes
 */
export async function deleteAllMessagesForSession(
  sessionId: string,
): Promise<void> {
  return safeInvoke<void>('messages_delete_all_for_session', { sessionId });
}

// ========================================
// Message Search
// ========================================

/**
 * Searches messages using BM25 full-text search.
 * @param query Search query string
 * @param sessionId Session ID to search within (optional - omit for global search)
 * @param page Page number (1-indexed, default: 1)
 * @param pageSize Number of results per page (default: 25)
 * @returns A promise that resolves to a Page of search results
 */
export async function searchMessages(
  query: string,
  sessionId?: string,
  page = 1,
  pageSize = 25,
): Promise<Page<MessageSearchResult>> {
  const result = await safeInvoke<Page<Record<string, unknown>>>(
    'messages_search',
    {
      query,
      sessionId: sessionId || null,
      page,
      pageSize,
    },
  );

  // Deserialize search results from Rust format (serde rename_all = "camelCase")
  return {
    ...result,
    items: result.items.map((r) => {
      const timestamp = r.createdAt as number | undefined;
      // Validate timestamp before creating Date object
      const date =
        typeof timestamp === 'number' &&
        !isNaN(timestamp) &&
        isFinite(timestamp)
          ? new Date(timestamp)
          : new Date(0);

      return {
        messageId: r.messageId as string,
        sessionId: r.sessionId as string,
        score: r.score as number,
        snippet: r.snippet as string | undefined,
        createdAt: date,
      };
    }),
  };
}
