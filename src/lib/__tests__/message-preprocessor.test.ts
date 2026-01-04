import { describe, it, expect, vi } from 'vitest';
import { prepareMessagesForLLM, prepareMessageForLLM } from '../message-preprocessor';
import type { Message } from '@/models/chat';

// Mock logger to avoid cluttering test output
vi.mock('../logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    error: vi.fn(),
    warn: vi.fn(),
  }),
}));

describe('message-preprocessor', () => {
  describe('prepareMessageForLLM', () => {
    it('should return the message as-is if there are no attachments', async () => {
      const message: Message = {
        id: '1',
        role: 'user',
        content: [{ type: 'text', text: 'Hello' }],
        sessionId: 'session-1',
        threadId: 'thread-1',
      };

      const result = await prepareMessageForLLM(message);
      expect(result).toEqual(message);
    });

    it('should append attachment information to the content', async () => {
      const message: Message = {
        id: '1',
        role: 'user',
        content: [{ type: 'text', text: 'Analyze this file' }],
        sessionId: 'session-1',
        threadId: 'thread-1',
        attachments: [
          {
            sessionId: 'session-1',
            contentId: 'content-1',
            filename: 'data.csv',
            mimeType: 'text/csv',
            size: 1024,
            lineCount: 10,
            preview: 'col1,col2\nval1,val2',
            uploadedAt: '2024-01-01T00:00:00Z',
          },
        ],
      };

      const result = await prepareMessageForLLM(message);

      expect(result.content).toHaveLength(2);
      expect(result.content[0]).toEqual({ type: 'text', text: 'Analyze this file' });
      
      const attachmentContent = result.content[1];
      expect(attachmentContent.type).toBe('text');
      if (attachmentContent.type === 'text') {
        expect(attachmentContent.text).toContain('<attachment_0>');
        expect(attachmentContent.text).toContain('data.csv');
        expect(attachmentContent.text).toContain('readContent');
      }
    });

    it('should handle multiple attachments', async () => {
      const message: Message = {
        id: '1',
        role: 'user',
        content: [{ type: 'text', text: 'Files:' }],
        sessionId: 'session-1',
        threadId: 'thread-1',
        attachments: [
          {
            sessionId: 'session-1',
            contentId: 'c1',
            filename: 'file1.txt',
            mimeType: 'text/plain',
            size: 100,
            lineCount: 5,
            preview: 'line1\nline2',
            uploadedAt: '2024-01-01T00:00:00Z',
          },
          {
            sessionId: 'session-1',
            contentId: 'c2',
            filename: 'file2.txt',
            mimeType: 'text/plain',
            size: 200,
            lineCount: 10,
            preview: 'content2',
            uploadedAt: '2024-01-01T00:00:00Z',
          },
        ],
      };

      const result = await prepareMessageForLLM(message);
      
      const attachmentContent = result.content[1];
      expect(attachmentContent.type).toBe('text');
      if (attachmentContent.type === 'text') {
        expect(attachmentContent.text).toContain('<attachment_0>');
        expect(attachmentContent.text).toContain('file1.txt');
        expect(attachmentContent.text).toContain('<attachment_1>');
        expect(attachmentContent.text).toContain('file2.txt');
      }
    });
  });

  describe('prepareMessagesForLLM', () => {
    it('should process multiple messages', async () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'user',
          content: [{ type: 'text', text: 'Hello' }],
          sessionId: 's1',
          threadId: 't1',
        },
        {
          id: '2',
          role: 'user',
          content: [{ type: 'text', text: 'Look at this' }],
          sessionId: 's1',
          threadId: 't1',
          attachments: [
            {
              sessionId: 's1',
              contentId: 'c1',
              filename: 'test.txt',
              mimeType: 'text/plain',
              size: 10,
              lineCount: 1,
              preview: 'test',
              uploadedAt: '2024-01-01T00:00:00Z',
            },
          ],
        },
      ];

      const results = await prepareMessagesForLLM(messages);

      expect(results).toHaveLength(2);
      expect(results[0].content).toHaveLength(1);
      expect(results[1].content).toHaveLength(2);
      
      const attachmentText = results[1].content[1];
      if (attachmentText.type === 'text') {
        expect(attachmentText.text).toContain('test.txt');
      }
    });

    it('should preserve error messages', async () => {
      const messages: Message[] = [
        {
          id: '1',
          role: 'assistant',
          content: [{ type: 'text', text: 'Error occurred' }],
          sessionId: 's1',
          threadId: 't1',
          error: {
            displayMessage: 'Something went wrong',
            type: 'AI_SERVICE_ERROR',
            recoverable: false,
          },
        },
      ];

      const results = await prepareMessagesForLLM(messages);
      expect(results[0].error).toBeDefined();
      expect(results[0].error?.type).toBe('AI_SERVICE_ERROR');
    });
  });
});
