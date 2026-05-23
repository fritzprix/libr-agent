import { beforeEach, describe, expect, it, vi } from 'vitest';

import { saveAgentFile } from './agent-backend';
import { safeInvoke } from '@/lib/backend/core';
import type { MCPResult } from '@/lib/mcp/protocol/response';
import type { AttachmentItem } from '@/models/attachments';

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: vi.fn(),
}));

describe('agent-backend attachment APIs', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns the attachment item when agent_add_attachment succeeds', async () => {
    const attachment: AttachmentItem = {
      sessionId: 'session-1',
      contentId: 'content-1',
      filename: 'note.md',
      mimeType: 'text/markdown',
      size: 12,
      lineCount: 2,
      preview: 'hello',
      uploadedAt: '2026-05-21T05:00:00.000Z',
      chunkCount: 1,
    };

    vi.mocked(safeInvoke).mockResolvedValueOnce({
      structuredContent: attachment,
      isError: false,
    } satisfies MCPResult);

    await expect(
      saveAgentFile('session-1', 'note.md', {
        fileUrl: 'file:///C:/workspace/note.md',
      }),
    ).resolves.toEqual(attachment);
  });

  it('surfaces backend tool errors from MCP text content', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({
      isError: true,
      content: [
        {
          type: 'text',
          text: 'Invalid file URL: URL cannot be converted to a local file path',
        },
      ],
    } satisfies MCPResult);

    await expect(
      saveAgentFile('session-1', 'note.md', {
        fileUrl: 'file:///?bad-path',
      }),
    ).rejects.toThrow(
      'Invalid file URL: URL cannot be converted to a local file path',
    );
  });

  it('includes MCP text content when payload shape is invalid', async () => {
    vi.mocked(safeInvoke).mockResolvedValueOnce({
      isError: false,
      content: [
        {
          type: 'text',
          text: 'Attachment saved successfully but no structured attachment data was returned',
        },
      ],
      structuredContent: {
        ok: true,
      },
    } satisfies MCPResult);

    await expect(
      saveAgentFile('session-1', 'note.md', {
        fileUrl: 'file:///C:/workspace/note.md',
      }),
    ).rejects.toThrow(
      'agent_add_attachment returned an invalid attachment payload: Attachment saved successfully but no structured attachment data was returned',
    );
  });
});
