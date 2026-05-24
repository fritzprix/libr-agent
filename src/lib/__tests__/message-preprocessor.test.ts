import { describe, it, expect, vi, beforeEach, type Mock } from 'vitest';
import {
  calculateContextSafetyMargin,
  estimatePayloadTokens,
  prepareMessageForLLM,
  prepareMessagesForLLM,
} from '../message-preprocessor';
import type { Message, AttachmentReference } from '@/models/chat';
import type { MCPTextContent } from '@/lib/mcp/protocol/content';
import * as loggerModule from '../logger';
import { readLocalFileAsBase64 } from '@/lib/backend/workspace';

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

vi.mock('@/lib/backend/workspace', () => ({
  readLocalFileAsBase64: vi.fn(),
}));

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
      expect(text).toContain('"mode": "indexed"');
      expect(text).toContain(
        'attachments__read(contentId: "content-1", fromLine: 1, toLine: 200)',
      );
      expect(text).toContain(
        'attachments__search(query: "your search query")',
      );
      expect(text).toContain('attachments__list()');
    });

    it('logs the effective default for includeLatestMediaPayload', async () => {
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

      await prepareMessageForLLM(message);

      expect(mockLogger.debug).toHaveBeenCalledWith(
        'Preprocessing message with attachments',
        expect.objectContaining({
          messageId: message.id,
          attachmentCount: 1,
          includeLatestMediaPayload: true,
        }),
      );
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

      expect(text).toContain('workspace__readFile(path: "/path/to/file.txt")');
      expect(text).toContain('"mode": "workspace-text"');
      expect(text).toContain('attachments tools: do not use them for this file');
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
      expect(text).toContain('"mode": "metadata-only"');
    });

    it('marks workspace-only binary attachments as not readable through text tools', async () => {
      const message = createMessage({
        attachments: [
          {
            sessionId: 'session-1',
            workspacePath: '/path/to/video.mp4',
            filename: 'video.mp4',
            mimeType: 'video/mp4',
            size: 1024,
            lineCount: 0,
            preview: 'video.mp4',
            uploadedAt: new Date().toISOString(),
            status: 'workspace-only',
          },
        ],
      });

      const processed = await prepareMessageForLLM(message);
      const text = (processed.content[1] as MCPTextContent).text;

      expect(text).toContain('"mode": "workspace-binary"');
      expect(text).toContain('workspace__readFile: do not use it; this is binary/non-text');
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

  describe('token estimation helpers', () => {
    it('calculates a bounded context safety margin', () => {
      expect(calculateContextSafetyMargin(10_000)).toBe(1024);
      expect(calculateContextSafetyMargin(100_000)).toBe(5000);
      expect(calculateContextSafetyMargin(500_000)).toBe(8192);
    });

    it('estimates payload tokens including system prompt and tools', () => {
      const messages = [
        createMessage({ content: [{ type: 'text', text: 'hello world' }] }),
      ];

      const estimate = estimatePayloadTokens('system prompt', messages, [
        { name: 'toolA', description: 'Test tool' },
      ]);

      expect(estimate).toBeGreaterThan(0);
    });
  });

  // ─── Regression: inline image/audio injection (multimodal pipeline fix) ──────
  //
  // In the agent V2 path (Rust-orchestrated), inlineContent lives only in the
  // attachments[] field — it is NOT pre-populated into message.content.
  // The preprocessor must inject MCPImageContent/MCPAudioContent blocks.
  describe('inline multimodal attachments (agent V2 regression)', () => {
    const makeInlineImageAttachment = (overrides = {}): AttachmentReference => ({
      sessionId: 'session-1',
      filename: 'photo.jpg',
      mimeType: 'image/jpeg',
      size: 12345,
      lineCount: 0,
      preview: '',
      uploadedAt: new Date().toISOString(),
      status: 'inline',
      inlineContent: {
        type: 'image',
        data: 'base64encodedimagedata==',
        mimeType: 'image/jpeg',
      },
      ...overrides,
    });

    it('injects an MCPImageContent block into message.content for an inline image', async () => {
      const message = createMessage({
        attachments: [makeInlineImageAttachment()],
      });

      const result = await prepareMessageForLLM(message);

      const imageBlocks = result.content.filter((c) => c.type === 'image');
      expect(imageBlocks).toHaveLength(1);
      expect(imageBlocks[0]).toMatchObject({
        type: 'image',
        data: 'base64encodedimagedata==',
        mimeType: 'image/jpeg',
      });
    });

    it('injects an MCPAudioContent block into message.content for an inline audio', async () => {
      const attachment: AttachmentReference = {
        sessionId: 'session-1',
        filename: 'clip.wav',
        mimeType: 'audio/wav',
        size: 4096,
        lineCount: 0,
        preview: '',
        uploadedAt: new Date().toISOString(),
        status: 'inline',
        inlineContent: {
          type: 'audio',
          data: 'base64audiodata==',
          mimeType: 'audio/wav',
        },
      };
      const message = createMessage({ attachments: [attachment] });

      const result = await prepareMessageForLLM(message);

      const audioBlocks = result.content.filter((c) => c.type === 'audio');
      expect(audioBlocks).toHaveLength(1);
      expect(audioBlocks[0]).toMatchObject({
        type: 'audio',
        data: 'base64audiodata==',
        mimeType: 'audio/wav',
      });
    });

    it('preserves existing message.content text alongside injected image blocks', async () => {
      const message = createMessage({
        content: [{ type: 'text', text: 'Look at this image' }],
        attachments: [makeInlineImageAttachment()],
      });

      const result = await prepareMessageForLLM(message);

      expect(result.content[0]).toMatchObject({ type: 'text', text: 'Look at this image' });
      const imageBlocks = result.content.filter((c) => c.type === 'image');
      expect(imageBlocks).toHaveLength(1);
    });

    it('handles multiple inline images in one message', async () => {
      const message = createMessage({
        attachments: [
          makeInlineImageAttachment({ filename: 'a.png', inlineContent: { type: 'image', data: 'aaa', mimeType: 'image/png' } }),
          makeInlineImageAttachment({ filename: 'b.jpg', inlineContent: { type: 'image', data: 'bbb', mimeType: 'image/jpeg' } }),
        ],
      });

      const result = await prepareMessageForLLM(message);

      const imageBlocks = result.content.filter((c) => c.type === 'image');
      expect(imageBlocks).toHaveLength(2);
      expect(imageBlocks[0]).toMatchObject({ data: 'aaa', mimeType: 'image/png' });
      expect(imageBlocks[1]).toMatchObject({ data: 'bbb', mimeType: 'image/jpeg' });
    });

    it('ignores inline attachments that have no inlineContent', async () => {
      const message = createMessage({
        attachments: [{
          sessionId: 'session-1',
          filename: 'empty.jpg',
          mimeType: 'image/jpeg',
          size: 0,
          lineCount: 0,
          preview: '',
          uploadedAt: new Date().toISOString(),
          status: 'inline',
          // no inlineContent
        }],
      });

      const result = await prepareMessageForLLM(message);
      const imageBlocks = result.content.filter((c) => c.type === 'image');
      expect(imageBlocks).toHaveLength(0);
    });

    it('does not inject text hint blocks for inline attachments (no tool-call hints)', async () => {
      const message = createMessage({
        attachments: [makeInlineImageAttachment()],
      });

      const result = await prepareMessageForLLM(message);

      // Should have original text + image block, but no attachment_0 XML hint
      const textBlocks = result.content.filter((c) => c.type === 'text') as MCPTextContent[];
      const hasHintBlock = textBlocks.some((c) => c.text.includes('<attachment_'));
      expect(hasHintBlock).toBe(false);
    });

    it('mixes inline and workspace attachments correctly', async () => {
      const workspaceAttachment: AttachmentReference = {
        sessionId: 'session-1',
        filename: 'notes.md',
        mimeType: 'text/markdown',
        size: 500,
        lineCount: 20,
        preview: '# Notes',
        uploadedAt: new Date().toISOString(),
        status: 'workspace-only',
        workspacePath: '/workspace/notes.md',
      };

      const message = createMessage({
        attachments: [makeInlineImageAttachment(), workspaceAttachment],
      });

      const result = await prepareMessageForLLM(message);

      // Image block present
      const imageBlocks = result.content.filter((c) => c.type === 'image');
      expect(imageBlocks).toHaveLength(1);
      // Workspace attachment hint present
      const textBlocks = result.content.filter((c) => c.type === 'text') as MCPTextContent[];
      const hasHintBlock = textBlocks.some((c) => c.text.includes('<attachment_'));
      expect(hasHintBlock).toBe(true);
    });

    it('materializes the latest inline media URI and summarizes older media messages', async () => {
      const olderMessage = createMessage({
        content: [
          { type: 'text', text: 'Earlier image' },
          {
            type: 'image',
            uri: 'data:image/png;base64,b2xkZXI=',
            mimeType: 'image/png',
          },
        ],
      });
      const latestMessage = createMessage({
        content: [
          { type: 'text', text: 'Latest image' },
          {
            type: 'image',
            uri: 'data:image/png;base64,bGF0ZXN0',
            mimeType: 'image/png',
          },
        ],
      });

      const [processedOlder, processedLatest] = await prepareMessagesForLLM([
        olderMessage,
        latestMessage,
      ]);

      expect(processedOlder.content.filter((c) => c.type === 'image')).toHaveLength(0);
      const olderTextBlocks = processedOlder.content.filter(
        (c) => c.type === 'text',
      ) as MCPTextContent[];
      expect(
        olderTextBlocks.some((c) => c.text.includes('<historical_media_0>')),
      ).toBe(true);
      expect(
        olderTextBlocks.some((c) =>
          c.text.includes('Do not call attachments tools or workspace__readFile'),
        ),
      ).toBe(true);

      const latestImageBlocks = processedLatest.content.filter(
        (c) => c.type === 'image',
      );
      expect(latestImageBlocks).toHaveLength(1);
      expect(latestImageBlocks[0]).toMatchObject({
        data: 'bGF0ZXN0',
        mimeType: 'image/png',
      });
    });

    it('materializes latest file URIs through the backend instead of fetch(file://)', async () => {
      vi.mocked(readLocalFileAsBase64).mockResolvedValue('ZmlsZS1ieXRlcw==');
      const fetchSpy = vi.spyOn(globalThis, 'fetch');

      const latestMessage = createMessage({
        content: [
          { type: 'text', text: 'Latest image' },
          {
            type: 'image',
            uri: 'file:///tmp/example.png',
            mimeType: 'image/png',
          },
        ],
      });

      const [processedLatest] = await prepareMessagesForLLM([latestMessage]);

      expect(readLocalFileAsBase64).toHaveBeenCalledWith(
        'session-1',
        'file:///tmp/example.png',
      );
      expect(fetchSpy).not.toHaveBeenCalled();
      expect(processedLatest.content.filter((c) => c.type === 'image')[0]).toMatchObject({
        data: 'ZmlsZS1ieXRlcw==',
        mimeType: 'image/png',
      });
    });

    it('materializes inline attachment URIs only for the latest media message', async () => {
      const olderMessage = createMessage({
        attachments: [
          makeInlineImageAttachment({
            filename: 'older.png',
            inlineContent: {
              type: 'image',
              uri: 'data:image/png;base64,b2xk',
              mimeType: 'image/png',
            },
          }),
        ],
      });
      const latestMessage = createMessage({
        attachments: [
          makeInlineImageAttachment({
            filename: 'latest.png',
            inlineContent: {
              type: 'image',
              uri: 'data:image/png;base64,bmV3',
              mimeType: 'image/png',
            },
          }),
        ],
      });

      const [processedOlder, processedLatest] = await prepareMessagesForLLM([
        olderMessage,
        latestMessage,
      ]);

      expect(processedOlder.content.filter((c) => c.type === 'image')).toHaveLength(0);
      const processedLatestImages = processedLatest.content.filter(
        (c) => c.type === 'image',
      );
      expect(processedLatestImages).toHaveLength(1);
      expect(processedLatestImages[0]).toMatchObject({
        data: 'bmV3',
        mimeType: 'image/png',
      });
    });
  });
});
