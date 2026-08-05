import { describe, expect, it } from 'vitest';
import type { Message, ToolCall } from '@/models/chat';
import {
  buildMessageExportFilename,
  serializeMessageForClipboard,
  serializeMessageForDownload,
  serializeMessageTextOnly,
  serializeToolCallsForClipboard,
} from '../message-serialization';

function createMessage(overrides: Partial<Message> = {}): Message {
  return {
    id: 'msg-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'assistant',
    content: [{ type: 'text', text: 'Hello world' }],
    ...overrides,
  };
}

describe('message-serialization', () => {
  describe('serializeMessageTextOnly', () => {
    it('returns joined text content', () => {
      const message = createMessage({
        content: [
          { type: 'text', text: 'First' },
          { type: 'image', data: 'abc', mimeType: 'image/png' },
          { type: 'text', text: 'Second' },
        ],
      });

      expect(serializeMessageTextOnly(message)).toBe('First\n\nSecond');
    });

    it('prefers displayContent when provided', () => {
      const message = createMessage({
        content: [{ type: 'text', text: 'Original' }],
      });

      expect(
        serializeMessageTextOnly(message, [
          { type: 'text', text: 'From display' },
        ]),
      ).toBe('From display');
    });
  });

  describe('serializeMessageForClipboard', () => {
    it('includes thinking, text, attachments, and tool results in full mode', () => {
      const toolCalls: ToolCall[] = [
        {
          id: 'call-1',
          type: 'function',
          function: {
            name: 'workspace_read',
            arguments: '{"path":"README.md"}',
          },
        },
      ];
      const message = createMessage({
        thinking: 'Plan the read',
        tool_calls: toolCalls,
        attachments: [
          {
            sessionId: 'session-1',
            filename: 'notes.txt',
            mimeType: 'text/plain',
            size: 12,
            lineCount: 1,
            preview: 'note',
            uploadedAt: '2026-08-02T00:00:00.000Z',
            status: 'committed',
          },
        ],
      });
      const toolResultsMap = new Map<string, Message>([
        [
          'call-1',
          createMessage({
            id: 'result-1',
            role: 'tool',
            tool_call_id: 'call-1',
            content: [{ type: 'text', text: 'file contents' }],
          }),
        ],
      ]);

      const result = serializeMessageForClipboard(message, {
        mode: 'full',
        toolResultsMap,
      });

      expect(result).toContain('## Assistant');
      expect(result).toContain('Hello world');
      expect(result).toContain('Thinking');
      expect(result).toContain('Plan the read');
      expect(result).toContain('workspace_read');
      expect(result).toContain('notes.txt');
      expect(result).toContain('Tool Result: workspace_read');
      expect(result).toContain('file contents');
    });

    it('supports text-only and tools modes', () => {
      const message = createMessage({
        thinking: 'secret',
        content: [{ type: 'text', text: 'Visible' }],
        tool_calls: [
          {
            id: 'call-1',
            type: 'function',
            function: { name: 'demo', arguments: '{}' },
          },
        ],
      });

      expect(
        serializeMessageForClipboard(message, { mode: 'text' }),
      ).toBe('Visible');
      expect(
        serializeMessageForClipboard(message, { mode: 'tools' }),
      ).toContain('"name": "demo"');
    });

    it('download serialization keeps markdown body without role header, thinking, or tools', () => {
      const message = createMessage({
        thinking: 'secret plan',
        content: [
          { type: 'text', text: '## Answer\n\n- point one\n- point two' },
          {
            type: 'tool_call',
            id: 'call-1',
            name: 'workspace_read',
            arguments: '{}',
          },
        ],
        tool_calls: [
          {
            id: 'call-1',
            type: 'function',
            function: { name: 'workspace_read', arguments: '{}' },
          },
        ],
      });

      const result = serializeMessageForDownload(message);

      expect(result).toBe('## Answer\n\n- point one\n- point two');
      expect(result).not.toContain('## Assistant');
      expect(result).not.toContain('secret plan');
      expect(result).not.toContain('workspace_read');
    });

    it('serializes streaming messages (not skipped)', () => {
      const message = createMessage({
        isStreaming: true,
        content: [{ type: 'text', text: 'Partial' }],
      });

      expect(serializeMessageForClipboard(message)).toContain('Partial');
    });
  });

  describe('serializeToolCallsForClipboard', () => {
    it('returns formatted JSON with parsed arguments and results', () => {
      const toolCalls: ToolCall[] = [
        {
          id: 'call-1',
          type: 'function',
          function: {
            name: 'demo',
            arguments: '{"ok":true}',
          },
        },
      ];
      const toolResultsMap = new Map<string, Message>([
        [
          'call-1',
          createMessage({
            id: 'result-1',
            role: 'tool',
            content: [{ type: 'text', text: 'done' }],
          }),
        ],
      ]);

      const parsed = JSON.parse(
        serializeToolCallsForClipboard(toolCalls, toolResultsMap),
      ) as unknown;

      expect(parsed).toEqual([
        {
          id: 'call-1',
          name: 'demo',
          arguments: { ok: true },
          result: {
            content: [{ type: 'text', text: 'done' }],
          },
        },
      ]);
    });

    it('propagates tool failure via top-level isError from metadata.toolError', () => {
      const toolCalls: ToolCall[] = [
        {
          id: 'call-fail',
          type: 'function',
          function: {
            name: 'workspace__runPowerShell',
            arguments: '{"command":"exit 1","timeout":120}',
          },
        },
      ];
      const failureText =
        '✗ Command failed with exit code: 1\n\nError output:\n...';
      const toolResultsMap = new Map<string, Message>([
        [
          'call-fail',
          createMessage({
            id: 'result-fail',
            role: 'tool',
            tool_call_id: 'call-fail',
            content: [{ type: 'text', text: failureText }],
            metadata: { toolError: true },
          }),
        ],
      ]);

      const parsed = JSON.parse(
        serializeToolCallsForClipboard(toolCalls, toolResultsMap),
      ) as unknown;

      expect(parsed).toEqual([
        {
          id: 'call-fail',
          name: 'workspace__runPowerShell',
          arguments: { command: 'exit 1', timeout: 120 },
          result: {
            content: [{ type: 'text', text: failureText }],
            isError: true,
          },
        },
      ]);
    });

    it('detects failure from metadata.toolError alone', () => {
      const toolCalls: ToolCall[] = [
        {
          id: 'call-2',
          type: 'function',
          function: { name: 'demo', arguments: '{}' },
        },
      ];
      const toolResultsMap = new Map<string, Message>([
        [
          'call-2',
          createMessage({
            id: 'result-2',
            role: 'tool',
            content: [{ type: 'text', text: 'failed' }],
            metadata: { toolError: true },
          }),
        ],
      ]);

      const parsed = JSON.parse(
        serializeToolCallsForClipboard(toolCalls, toolResultsMap),
      ) as Array<{ result?: { isError?: boolean; content: unknown[] } }>;

      expect(parsed[0]?.result?.isError).toBe(true);
      expect(parsed[0]?.result?.content).toEqual([
        { type: 'text', text: 'failed' },
      ]);
    });
  });

  describe('buildMessageExportFilename', () => {
    it('uses a simple message.<ext> name', () => {
      const message = createMessage({
        role: 'user',
        createdAt: new Date('2026-08-02T01:02:03.000Z'),
      });

      expect(buildMessageExportFilename(message, 'md')).toBe('message.md');
      expect(buildMessageExportFilename(message, 'pdf')).toBe('message.pdf');
    });
  });
});
