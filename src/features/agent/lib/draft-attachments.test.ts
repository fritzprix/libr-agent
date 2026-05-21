import { beforeEach, describe, expect, it, vi } from 'vitest';

import { prepareDraftAttachments } from './draft-attachments';
import { workspacePathToFileUrl } from '@/lib/file-url';
import { getWorkspaceDir, workspaceWriteFile } from '@/lib/backend';
import { saveAgentFile } from '@/features/agent/api/agent-backend';
import { generateWorkspacePath } from '@/lib/workspace-sync-service';

vi.mock('@/lib/backend', () => ({
  getWorkspaceDir: vi.fn(),
  workspaceWriteFile: vi.fn(),
}));

vi.mock('@/features/agent/api/agent-backend', () => ({
  saveAgentFile: vi.fn(),
}));

vi.mock('@/lib/workspace-sync-service', () => ({
  generateWorkspacePath: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

describe('prepareDraftAttachments', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getWorkspaceDir).mockResolvedValue(
      '\\\\?\\C:\\Users\\SKTelecom\\workspace\\session-1',
    );
    vi.mocked(workspaceWriteFile).mockResolvedValue(undefined);
    vi.mocked(generateWorkspacePath).mockReturnValue(
      'attachments\\generated-report.pdf',
    );
  });

  it('uses workspacePathToFileUrl for binary indexable draft attachments', async () => {
    const file = new File(['pdf-content'], 'report.pdf', {
      type: 'application/pdf',
    });
    const onAttachmentError = vi.fn();

    Object.defineProperty(file, 'arrayBuffer', {
      value: vi.fn().mockResolvedValue(
        new TextEncoder().encode('pdf-content').buffer,
      ),
      configurable: true,
    });

    vi.mocked(saveAgentFile).mockResolvedValue({
      sessionId: 'session-1',
      contentId: 'content-1',
      filename: file.name,
      mimeType: file.type,
      size: file.size,
      lineCount: 0,
      preview: file.name,
      uploadedAt: '2026-05-21T07:00:00.000Z',
      chunkCount: 1,
    });

    const attachments = await prepareDraftAttachments({
      files: [file],
      sessionId: 'session-1',
      now: new Date('2026-05-21T07:00:00.000Z'),
      getMimeType: () => 'application/pdf',
      onAttachmentError,
    });

    expect(saveAgentFile).toHaveBeenCalledWith('session-1', 'report.pdf', {
      fileUrl: workspacePathToFileUrl(
        '\\\\?\\C:\\Users\\SKTelecom\\workspace\\session-1',
        'attachments\\generated-report.pdf',
      ),
      metadata: expect.objectContaining({
        mimeType: 'application/pdf',
        filename: 'report.pdf',
      }),
    });

    expect(attachments).toEqual([
      expect.objectContaining({
        status: 'committed',
        contentId: 'content-1',
        workspacePath: 'attachments\\generated-report.pdf',
      }),
    ]);
    expect(onAttachmentError).not.toHaveBeenCalled();
  });
});
