import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { safeInvoke } from './core';
import {
  getMessagesPageForSession,
  upsertMessages,
  upsertMessage,
  deleteMessage,
  deleteAllMessagesForSession,
  searchMessages,
} from './messages';
import type { Message, RustMessage, ToolCall } from '@/models/chat';

vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

describe('Message Management Service', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  describe('getMessagesPageForSession', () => {
    it('should throw if sessionId is missing', async () => {
      await expect(
        getMessagesPageForSession('', 'thread-1', 1, 10),
      ).rejects.toThrow('sessionId and threadId are required');
    });

    it('should throw if threadId is missing', async () => {
      await expect(
        getMessagesPageForSession('session-1', '', 1, 10),
      ).rejects.toThrow('sessionId and threadId are required');
    });

    it('should fetch and deserialize messages successfully', async () => {
      const mockTimestamp = Date.now();

      const mockRustResponse = {
        items: [
          {
            id: 'msg-1',
            sessionId: 'session-1',
            threadId: 'thread-1',
            role: 'assistant',
            content: [{ type: 'text', text: 'Hello World' }],
            createdAt: mockTimestamp,
            updatedAt: mockTimestamp,
            isStreaming: false,
            thinking: 'A thinking process',
            thinkingSignature: 'sig',
            assistantId: 'assist-1',
            source: 'assistant',
            error: undefined,
            usage: {
              promptTokens: 10,
              completionTokens: 20,
              totalTokens: 30,
            },
            toolCalls: [
              {
                id: 'tc-1',
                type: 'function',
                function: {
                  name: 'getWeather',
                  arguments: '{"location": "Tokyo"}',
                },
              },
            ],
          } satisfies RustMessage,
          {
            id: 'msg-2',
            sessionId: 'session-1',
            threadId: (null as unknown) as string, // should fall back to sessionId
            role: 'user',
            content: [{ type: 'text', text: 'How are you?' }],
            createdAt: mockTimestamp,
            updatedAt: mockTimestamp,
            toolCallId: 'tc-1',
          } as unknown as RustMessage,
        ],
        totalItems: 2,
        page: 1,
        pageSize: 10,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await getMessagesPageForSession(
        'session-1',
        'thread-1',
        1,
        10,
      );

      expect(safeInvoke).toHaveBeenCalledWith('messages_get_page', {
        sessionId: 'session-1',
        threadId: 'thread-1',
        page: 1,
        pageSize: 10,
      });

      expect(result.items).toHaveLength(2);
      expect(result.totalItems).toBe(2);

      // Verify deserialization mappings of msg-1
      expect(result.items[0]).toEqual(
        expect.objectContaining({
          id: 'msg-1',
          sessionId: 'session-1',
          threadId: 'thread-1',
          role: 'assistant',
          content: [{ type: 'text', text: 'Hello World' }],
          isStreaming: false,
          thinking: 'A thinking process',
          thinkingSignature: 'sig',
          assistantId: 'assist-1',
          source: 'assistant',
          createdAt: new Date(mockTimestamp),
          updatedAt: new Date(mockTimestamp),
          usage: mockRustResponse.items[0].usage,
        }),
      );
      expect(result.items[0].tool_calls).toEqual([
        {
          id: 'tc-1',
          type: 'function',
          function: {
            name: 'getWeather',
            arguments: '{"location": "Tokyo"}',
          },
        },
      ]);

      // Verify fallback and optional missing properties of msg-2
      expect(result.items[1]).toEqual(
        expect.objectContaining({
          id: 'msg-2',
          sessionId: 'session-1',
          threadId: 'session-1', // fallback logic
          role: 'user',
          content: [{ type: 'text', text: 'How are you?' }],
          tool_call_id: 'tc-1',
          createdAt: new Date(mockTimestamp),
          updatedAt: new Date(mockTimestamp),
        }),
      );
      expect(result.items[1].tool_calls).toBeUndefined();
      expect(result.items[1].isStreaming).toBeUndefined();
    });

    it('should map tool_calls without explicit type to type "function"', async () => {
      const mockRustResponse = {
        items: [
          {
            id: 'msg-3',
            sessionId: 'session-1',
            threadId: 'thread-1',
            role: 'assistant',
            content: [],
            createdAt: Date.now(),
            updatedAt: Date.now(),
            toolCalls: [
              ({
                id: 'tc-2',
                // missing type should default to 'function'
                function: {
                  name: 'test',
                  arguments: '{}',
                },
              } as unknown) as ToolCall,
            ],
          } as unknown as RustMessage,
        ],
        totalItems: 1,
        page: 1,
        pageSize: 10,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await getMessagesPageForSession(
        'session-1',
        'thread-1',
        1,
        10,
      );
      expect(result.items[0].tool_calls?.[0].type).toBe('function');
    });

    it('should preserve channel source and metadata when deserializing', async () => {
      const mockTimestamp = Date.now();
      const mockRustResponse = {
        items: [
          {
            id: 'msg-channel',
            sessionId: 'session-1',
            threadId: 'thread-1',
            role: 'user',
            content: [
              {
                type: 'text',
                text: '<channel source="webhook" severity="high">deploy failed</channel>',
              },
            ],
            createdAt: mockTimestamp,
            updatedAt: mockTimestamp,
            source: 'channel',
            metadata: {
              channel: {
                serverName: 'webhook',
                meta: {
                  severity: 'high',
                },
              },
            },
          } satisfies RustMessage,
        ],
        totalItems: 1,
        page: 1,
        pageSize: 10,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await getMessagesPageForSession(
        'session-1',
        'thread-1',
        1,
        10,
      );

      expect(result.items[0]).toEqual(
        expect.objectContaining({
          source: 'channel',
          metadata: mockRustResponse.items[0].metadata,
        }),
      );
    });

    it('should preserve backend-emitted source values recognized by the frontend', async () => {
      const mockTimestamp = Date.now();
      const mockRustResponse = {
        items: [
          {
            id: 'msg-scheduled',
            sessionId: 'session-1',
            threadId: 'thread-1',
            role: 'user',
            content: [{ type: 'text', text: 'Scheduled execution' }],
            createdAt: mockTimestamp,
            updatedAt: mockTimestamp,
            source: 'scheduled_task',
          } satisfies RustMessage,
        ],
        totalItems: 1,
        page: 1,
        pageSize: 10,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await getMessagesPageForSession(
        'session-1',
        'thread-1',
        1,
        10,
      );

      expect(result.items[0]?.source).toBe('scheduled_task');
    });

    it('should drop unknown source values when deserializing backend messages', async () => {
      const mockTimestamp = Date.now();
      const mockRustResponse = {
        items: [
          {
            id: 'msg-unknown-source',
            sessionId: 'session-1',
            threadId: 'thread-1',
            role: 'user',
            content: [{ type: 'text', text: 'Unknown source' }],
            createdAt: mockTimestamp,
            updatedAt: mockTimestamp,
            source: 'not-a-real-source',
          } as unknown as RustMessage,
        ],
        totalItems: 1,
        page: 1,
        pageSize: 10,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await getMessagesPageForSession(
        'session-1',
        'thread-1',
        1,
        10,
      );

      expect(result.items[0]?.source).toBeUndefined();
    });
  });

  describe('upsertMessages', () => {
    it('should throw if any message is missing a sessionId', async () => {
      const msgs = ([
        {
          id: '1',
          role: 'user',
          content: [],
          threadId: 'thread-1',
          createdAt: new Date(),
        },
        {
          id: '2',
          role: 'assistant',
          content: [],
          threadId: 'thread-1',
          createdAt: new Date(),
        },
      ] as unknown) as Message[];

      await expect(upsertMessages(msgs)).rejects.toThrow(
        'Cannot upsert message: missing or empty sessionId for message 1',
      );
    });

    it('should throw if any message has empty sessionId', async () => {
      const msgs = ([
        {
          id: '1',
          role: 'user',
          content: [],
          sessionId: ' ',
          threadId: 'thread-1',
          createdAt: new Date(),
        },
      ] as unknown) as Message[];

      await expect(upsertMessages(msgs)).rejects.toThrow(
        'Cannot upsert message: missing or empty sessionId for message 1',
      );
    });

    it('should throw if any message is missing a threadId', async () => {
      const msgs = ([
        {
          id: '2',
          role: 'user',
          content: [],
          sessionId: 'sess-1',
          createdAt: new Date(),
        },
      ] as unknown) as Message[];

      await expect(upsertMessages(msgs)).rejects.toThrow(
        'Cannot upsert message: missing or empty threadId for message 2',
      );
    });

    it('should properly map frontend message arrays to rust structure', async () => {
      const mockDate = new Date();
      const msgs: Message[] = [
        {
          id: '1',
          sessionId: 'sess-1',
          threadId: 'thread-1',
          role: 'user',
          content: [{ type: 'text', text: 'hello' }],
          tool_calls: [
            {
              id: 'call_1',
              type: 'function',
              function: { name: 'test', arguments: '{}' },
            },
          ],
          tool_call_id: 'call_1',
          isStreaming: true,
          thinking: 'hmmm',
          thinkingSignature: 'sig',
          assistantId: 'asst-1',
          attachments: [],
          tool_use: undefined,
          createdAt: mockDate,
          updatedAt: mockDate,
          source: 'ui',
          error: undefined,
        },
      ];

      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await upsertMessages(msgs);

      expect(safeInvoke).toHaveBeenCalledWith('messages_upsert_many', {
        messages: [
          {
            id: '1',
            sessionId: 'sess-1',
            role: 'user',
            content: [{ type: 'text', text: 'hello' }],
            toolCalls: [
              {
                id: 'call_1',
                type: 'function',
                function: { name: 'test', arguments: '{}' },
              },
            ],
            toolCallId: 'call_1',
            isStreaming: true,
            thinking: 'hmmm',
            thinkingSignature: 'sig',
            assistantId: 'asst-1',
            attachments: [],
            toolUse: null,
            createdAt: mockDate.getTime(),
            updatedAt: mockDate.getTime(),
            source: 'ui',
            error: null,
          },
        ],
      });
    });

    it('should map missing date properties to current time', async () => {
      vi.useFakeTimers();
      const now = new Date('2024-01-01T00:00:00Z').getTime();
      vi.setSystemTime(now);

      const msgs = ([
        {
          id: '1',
          sessionId: 'sess-1',
          threadId: 'thread-1',
          role: 'user',
          content: [],
        },
      ] as unknown) as Message[];

      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await upsertMessages(msgs);

      expect(safeInvoke).toHaveBeenCalledWith('messages_upsert_many', {
        messages: [
          expect.objectContaining({
            createdAt: now,
            updatedAt: now,
          }),
        ],
      });
    });
  });

  describe('upsertMessage', () => {
    it('should throw if message is missing a sessionId', async () => {
      const msg = ({
        id: '1',
        role: 'user',
        content: [],
        threadId: 'thread-1',
        createdAt: new Date(),
      } as unknown) as Message;

      await expect(upsertMessage(msg)).rejects.toThrow(
        'Cannot upsert message: missing or empty sessionId for message 1',
      );
    });

    it('should map frontend message to rust structure', async () => {
      const mockDate = new Date();
      const msg = ({
        id: '1',
        sessionId: 'sess-1',
        threadId: 'thread-1',
        role: 'user',
        content: [{ type: 'text', text: 'hello' }],
        tool_calls: [
          {
            id: 'call_1',
            type: 'function',
            function: { name: 'test', arguments: '{}' },
          },
        ],
        createdAt: mockDate,
        updatedAt: mockDate,
      } as unknown) as Message;

      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await upsertMessage(msg);

      expect(safeInvoke).toHaveBeenCalledWith('messages_upsert', {
        message: expect.objectContaining({
          id: '1',
          sessionId: 'sess-1',
          role: 'user',
          content: [{ type: 'text', text: 'hello' }],
          toolCalls: [
            {
              id: 'call_1',
              type: 'function',
              function: { name: 'test', arguments: '{}' },
            },
          ],
          createdAt: mockDate.getTime(),
          updatedAt: mockDate.getTime(),
        }),
      });
    });

    it('should map missing date properties to current time', async () => {
      vi.useFakeTimers();
      const now = new Date('2024-01-01T00:00:00Z').getTime();
      vi.setSystemTime(now);

      const msg = ({
        id: '1',
        sessionId: 'sess-1',
        threadId: 'thread-1',
        role: 'user',
        content: [],
      } as unknown) as Message;

      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await upsertMessage(msg);

      expect(safeInvoke).toHaveBeenCalledWith('messages_upsert', {
        message: expect.objectContaining({
          createdAt: now,
          updatedAt: now,
        }),
      });
    });
  });

  describe('deleteMessage', () => {
    it('should call backend to delete a single message', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await deleteMessage('msg-1');

      expect(safeInvoke).toHaveBeenCalledWith('messages_delete', {
        messageId: 'msg-1',
      });
    });
  });

  describe('deleteAllMessagesForSession', () => {
    it('should call backend to delete all messages for a session', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce(undefined);

      await deleteAllMessagesForSession('sess-1');

      expect(safeInvoke).toHaveBeenCalledWith('messages_delete_all_for_session', {
        sessionId: 'sess-1',
      });
    });
  });

  describe('searchMessages', () => {
    it('should pass default pagination and map valid timestamps', async () => {
      const mockTimestamp = Date.now();

      const mockRustResponse = {
        items: [
          {
            messageId: 'msg-1',
            sessionId: 'sess-1',
            score: 0.95,
            snippet: 'hello world',
            createdAt: mockTimestamp,
          },
        ],
        totalItems: 1,
        page: 1,
        pageSize: 25,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await searchMessages('hello');

      expect(safeInvoke).toHaveBeenCalledWith('messages_search', {
        query: 'hello',
        sessionId: null,
        page: 1,
        pageSize: 25,
      });

      expect(result.items[0]).toEqual({
        messageId: 'msg-1',
        sessionId: 'sess-1',
        score: 0.95,
        snippet: 'hello world',
        createdAt: new Date(mockTimestamp),
      });
    });

    it('should handle specific pagination and sessionId', async () => {
      vi.mocked(safeInvoke).mockResolvedValueOnce({
        items: [],
        totalItems: 0,
        page: 1,
        pageSize: 50,
        totalPages: 0,
        hasNextPage: false,
        hasPreviousPage: false,
      });

      await searchMessages('hello', 'sess-1', 2, 50);

      expect(safeInvoke).toHaveBeenCalledWith('messages_search', {
        query: 'hello',
        sessionId: 'sess-1',
        page: 2,
        pageSize: 50,
      });
    });

    it('should fall back to 0-epoch date if timestamp is invalid', async () => {
      const mockRustResponse = {
        items: [
          {
            messageId: 'msg-1',
            sessionId: 'sess-1',
            score: 0.5,
            snippet: null,
            createdAt: ('invalid-date' as unknown) as number, // string instead of number
          },
        ],
        totalItems: 1,
        page: 1,
        pageSize: 25,
        totalPages: 1,
        hasNextPage: false,
        hasPreviousPage: false,
      };

      vi.mocked(safeInvoke).mockResolvedValueOnce(mockRustResponse);

      const result = await searchMessages('invalid');

      expect(result.items[0].createdAt).toEqual(new Date(0));
      expect(result.items[0].snippet).toBeUndefined();
    });
  });
});
