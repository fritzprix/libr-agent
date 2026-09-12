import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { FileWriteActions } from '../FileWriteActions';
import type { WriteFileResult } from '../types';
import {
  AgentFilePreviewProvider,
  useAgentFilePreview,
} from '@/context/AgentFilePreviewContext';
import { PREVIEW_MAX_BYTES } from '../../workspace-panel/filePreview';

const { openPathWithDefaultApp } = vi.hoisted(() => ({
  openPathWithDefaultApp: vi.fn(),
}));

vi.mock('@/lib/backend', () => ({
  openPathWithDefaultApp,
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

function PreviewProbe() {
  const { previewFile } = useAgentFilePreview();
  if (!previewFile) return null;
  return (
    <div
      data-testid="preview-probe"
      data-session-id={previewFile.sessionId ?? ''}
    >
      {previewFile.path}
    </div>
  );
}

function renderWithPreview(ui: ReactNode) {
  return render(
    <AgentFilePreviewProvider>
      {ui}
      <PreviewProbe />
    </AgentFilePreviewProvider>,
  );
}

function writeResult(
  overrides: Partial<WriteFileResult> = {},
): WriteFileResult {
  return {
    path: 'src/notes.md',
    absolute_path: '/home/user/project/src/notes.md',
    action: 'created',
    bytes_written: 42,
    lines: 3,
    ...overrides,
  };
}

describe('FileWriteActions', () => {
  beforeEach(() => {
    openPathWithDefaultApp.mockReset();
    openPathWithDefaultApp.mockResolvedValue(undefined);
  });

  it('opens previewable workspace files in the shared preview sheet', () => {
    renderWithPreview(<FileWriteActions data={writeResult()} />);

    fireEvent.click(screen.getByTestId('tool-structured-preview-file'));

    expect(screen.getByTestId('preview-probe')).toHaveTextContent('src/notes.md');
    expect(openPathWithDefaultApp).not.toHaveBeenCalled();
  });

  it('passes sessionId to preview context when provided', () => {
    renderWithPreview(
      <FileWriteActions data={writeResult()} sessionId="session-xyz" />,
    );

    fireEvent.click(screen.getByTestId('tool-structured-preview-file'));

    const probe = screen.getByTestId('preview-probe');
    expect(probe).toHaveTextContent('src/notes.md');
    expect(probe).toHaveAttribute('data-session-id', 'session-xyz');
  });

  it('falls back to the OS app for non-previewable files', async () => {
    renderWithPreview(
      <FileWriteActions
        data={writeResult({
          path: 'report.docx',
          absolute_path: '/home/user/project/report.docx',
        })}
      />,
    );

    expect(
      screen.queryByTestId('tool-structured-preview-file'),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByTestId('tool-structured-open-file'));

    await waitFor(() => {
      expect(openPathWithDefaultApp).toHaveBeenCalledWith(
        '/home/user/project/report.docx',
      );
    });
  });

  it('falls back to the OS app for files outside the workspace root', () => {
    renderWithPreview(
      <FileWriteActions
        data={writeResult({
          path: '/tmp/out.md',
          absolute_path: '/tmp/out.md',
        })}
      />,
    );

    expect(
      screen.queryByTestId('tool-structured-preview-file'),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('tool-structured-open-file')).toBeInTheDocument();
  });

  it('falls back to the OS app for oversized previewable files', () => {
    renderWithPreview(
      <FileWriteActions
        data={writeResult({
          path: 'large.ts',
          bytes_written: PREVIEW_MAX_BYTES + 1,
        })}
      />,
    );

    expect(
      screen.queryByTestId('tool-structured-preview-file'),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('tool-structured-open-file')).toBeInTheDocument();
  });

  it('opens the OS app from the secondary action on previewable files', async () => {
    renderWithPreview(<FileWriteActions data={writeResult()} />);

    fireEvent.click(screen.getByTestId('tool-structured-open-default-app'));

    await waitFor(() => {
      expect(openPathWithDefaultApp).toHaveBeenCalledWith(
        '/home/user/project/src/notes.md',
      );
    });
  });

  it('falls back to the OS app when preview context is missing', async () => {
    render(<FileWriteActions data={writeResult()} />);

    expect(
      screen.queryByTestId('tool-structured-preview-file'),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /open file/i }));
    await waitFor(() => {
      expect(openPathWithDefaultApp).toHaveBeenCalledWith(
        '/home/user/project/src/notes.md',
      );
    });
  });
});
