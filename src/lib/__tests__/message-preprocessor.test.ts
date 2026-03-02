import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import { prepareMessageForLLM, prepareMessagesForLLM } from '../message-preprocessor';
import type { Message, AttachmentReference } from '@/models/chat';
import type { MCPTextContent } from '@/lib/mcp/protocol/content';
import * as loggerModule from '../logger';

// Mock the logger
vi.mock('../logger', () => {
  const debug = vi.fn();
  const info = vi.fn();
  const error = vi.fn();
  const warn = vi.fn();
  return {
    getLogger: vi.fn(() => ({
      debug,
      info,
      error,
      warn,
    })),
    // Also export the mock functions directly so we can inspect them
    _mockDebug: debug,
    _mockInfo: info,
    _mockError: error,
    _mockWarn: warn,
  };
});

describe('message-preprocessor', () => {
  let mockLogger: { debug: Mock; info: Mock; error: Mock; warn: Mock };

  beforeEach(() => {
    vi.clearAllMocks();
    // Re-acquire the mock logger - cast through unknown for test mock module access
    mockLogger = (loggerModule as unknown as { getLogger: () => typeof mockLogger }).getLogger();
  });

  const createMessage = (overrides: Partial<Message> = {}): Message => ({
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'user',
    content: [{ type: 'text', text: 'Hello' }],
    createdAt: new Date(),
    updatedAt: new Date(),
    ...overrides,
  });

  describe('prepareMessageForLLM', () => {
    it('should return the message unchanged if there are no attachments', async () => {
      const message = createMessage({ attachments: [] });
      const processed = await prepareMessageForLLM(message);

      expect(processed).toEqual(message);
      expect(mockLogger.debug).not.toHaveBeenCalled();
    });

    it('should append attachment hints for Content Store files', async () => {
      const message = createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            contentId: 'content-1',
            filename: 'test.txt',
            mimeType: 'text/plain',
            size: 100,
            lineCount: 10,
            preview: 'preview',
            uploadedAt: new Date().toISOString(),
            status: 'committed',
          },
        ],
      });

      const processed = await prepareMessageForLLM(message);

      expect(processed.content).toHaveLength(2);
      expect(processed.content[1].type).toBe('text');
      const text = (processed.content[1] as MCPTextContent).text;

      expect(text).toContain('<attachment_0>');
      expect(text).toContain('"filename": "test.txt"');
      expect(text).toContain('readContent(sessionId: "session-1", contentId: "content-1"');
      expect(text).toContain('searchContent');
    });

    it('should append attachment hints for Workspace files', async () => {
      const message = createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            workspacePath: '/path/to/file.txt',
            filename: 'file.txt',
            mimeType: 'text/plain',
            size: 100,
            lineCount: 10,
            preview: 'preview',
            uploadedAt: new Date().toISOString(),
            status: 'workspace-only',
          },
        ],
      });

      const processed = await prepareMessageForLLM(message);

      expect(processed.content).toHaveLength(2);
      const text = (processed.content[1] as MCPTextContent).text;

      expect(text).toContain('builtin_workspace__readFile(path: "/path/to/file.txt")');
      expect(text).toContain('listContent(sessionId: "session-1")');
    });

    it('should append attachment hints for metadata-only files', async () => {
      const message = createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            filename: 'unknown.txt',
            mimeType: 'text/plain',
            size: 100,
            lineCount: 10,
            preview: 'preview',
            uploadedAt: new Date().toISOString(),
            status: 'pending', // No contentId or workspacePath
          },
        ],
      });

      const processed = await prepareMessageForLLM(message);

      expect(processed.content).toHaveLength(2);
      const text = (processed.content[1] as MCPTextContent).text;

      expect(text).toContain('File metadata only');
      expect(text).toContain('listContent(sessionId: "session-1")');
    });

    it('should handle multiple attachments', async () => {
      const message = createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            contentId: 'c1',
            filename: 'f1.txt',
            mimeType: 'text/plain',
            size: 10,
            lineCount: 1,
            preview: '',
            uploadedAt: '',
            status: 'committed',
          },
          {
            sessionId: 'session-1',
            contentId: 'c2',
            filename: 'f2.txt',
            mimeType: 'text/plain',
            size: 10,
            lineCount: 1,
            preview: '',
            uploadedAt: '',
            status: 'committed',
          },
        ],
      });

      const processed = await prepareMessageForLLM(message);

      expect(processed.content).toHaveLength(2);
      const text = (processed.content[1] as MCPTextContent).text;

      expect(text).toContain('<attachment_0>');
      expect(text).toContain('<attachment_1>');
    });

    it('should return original message on error', async () => {
      const message = createMessage({
        attachments: [
            // Circular reference or something that breaks JSON.stringify inside prepareMessageForLLM?
            // Actually prepareMessageForLLM uses JSON.stringify(attachment).
            // Let's create an object that throws on access to a property or something.
            // But AttachmentReference is an interface, so we pass a plain object.
            // A circular structure will cause JSON.stringify to throw.
        ]
      });

      // Create a circular structure to force JSON.stringify to throw
      const attachment: Record<string, unknown> = {
        sessionId: 's1',
        filename: 'f1',
        status: 'committed',
      };
      attachment.self = attachment;

      message.attachments = [attachment as unknown as AttachmentReference];

      const processed = await prepareMessageForLLM(message);

      expect(processed).toBe(message); // Should return the exact same object reference
      expect(mockLogger.error).toHaveBeenCalledWith(
        'Failed to preprocess message',
        expect.objectContaining({
            messageId: message.id,
            error: expect.stringContaining('Converting circular structure to JSON')
        })
      );
    });
  });

  describe('prepareMessagesForLLM', () => {
    it('should process an array of messages', async () => {
      const messages = [
        createMessage({ id: '1', content: [{ type: 'text', text: 'hi' }] }),
        createMessage({
            id: '2',
            attachments: [{
                sessionId: 's1',
                contentId: 'c1',
                filename: 'f1',
                mimeType: 'text',
                size: 0,
                lineCount: 0,
                preview: '',
                uploadedAt: '',
                status: 'committed'
            }]
        }),
      ];

      const processed = await prepareMessagesForLLM(messages);

      expect(processed).toHaveLength(2);
      expect(processed[0]).toEqual(messages[0]); // No attachments, same ref
      expect(processed[1].content).toHaveLength(2); // Attachments processed
    });

    it('should log statistics when attachments are present', async () => {
      const messages = [
        createMessage({
            id: '1',
            attachments: [{
                sessionId: 's1',
                contentId: 'c1',
                filename: 'f1',
                mimeType: 'text',
                size: 0,
                lineCount: 0,
                preview: '',
                uploadedAt: '',
                status: 'committed'
            }]
        }),
      ];

      await prepareMessagesForLLM(messages);

      expect(mockLogger.info).toHaveBeenCalledWith(
        'Processed messages for LLM',
        expect.objectContaining({
            totalMessages: 1,
            totalAttachments: 1,
            errorMessages: 0
        })
      );
    });

    it('should NOT log statistics when no attachments or errors', async () => {
        const messages = [
          createMessage({ id: '1', content: [{ type: 'text', text: 'hi' }] }),
        ];

        await prepareMessagesForLLM(messages);

        expect(mockLogger.info).not.toHaveBeenCalled();
      });

    it('should count error messages correctly', async () => {
        const messages = [
            createMessage({
                id: '1',
                error: {
                    displayMessage: 'Error',
                    type: 'AI_SERVICE_ERROR',
                    recoverable: true
                }
            }),
        ];

        await prepareMessagesForLLM(messages);

        expect(mockLogger.info).toHaveBeenCalledWith(
            'Processed messages for LLM',
            expect.objectContaining({
                totalMessages: 1,
                totalAttachments: 0,
                errorMessages: 1
            })
        );
    });
  });
});
