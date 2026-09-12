import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ReportResultCard } from '../ReportResultCard';
import type { ReportResultData } from '../types';

const openPathWithDefaultAppMock = vi.fn();
const downloadWorkspaceFileMock = vi.fn();

vi.mock('@/lib/backend', () => ({
  openPathWithDefaultApp: (...args: unknown[]) => openPathWithDefaultAppMock(...args),
  downloadWorkspaceFile: (...args: unknown[]) => downloadWorkspaceFileMock(...args),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

const openFilePreviewMock = vi.fn();
vi.mock('@/context/AgentFilePreviewContext', () => ({
  useOptionalAgentFilePreview: () => ({
    openFilePreview: openFilePreviewMock,
  }),
}));

vi.mock('@/context/AgentSessionContext', () => ({
  useAgentSessionState: () => ({
    session: { id: 'session-test-123' },
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    error: vi.fn(),
  },
}));

describe('ReportResultCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const sampleData: ReportResultData = {
    status: 'success',
    title: 'Deployment Complete',
    criteria: 'All unit tests pass and build succeeds',
    proof: 'cargo test returned 0 errors',
    result: 'The build was successful and artifacts were generated.',
    deliverables: [
      {
        path: 'dist/app.zip',
        absolute_path: '/workspace/dist/app.zip',
        name: 'app.zip',
        size_bytes: 1048576,
        extension: 'zip',
        exists: true,
      },
      {
        path: 'missing/output.txt',
        absolute_path: null,
        name: 'output.txt',
        size_bytes: null,
        extension: 'txt',
        exists: false,
      },
    ],
  };

  it('renders status header, criteria, proof, result, and deliverables count', () => {
    render(<ReportResultCard data={sampleData} sessionId="session-test-123" />);

    expect(screen.getByTestId('tool-structured-report-result')).toBeInTheDocument();
    expect(screen.getByText('Deployment Complete')).toBeInTheDocument();
    expect(screen.getByText('All unit tests pass and build succeeds')).toBeInTheDocument();
    expect(screen.getByText('cargo test returned 0 errors')).toBeInTheDocument();
    expect(
      screen.getByText('The build was successful and artifacts were generated.'),
    ).toBeInTheDocument();

    expect(screen.getByText('app.zip')).toBeInTheDocument();
    expect(screen.getByText('1.0 MB')).toBeInTheDocument();
    expect(screen.getByText('output.txt')).toBeInTheDocument();
    expect(screen.getByText(/not found/i)).toBeInTheDocument();
  });

  it('triggers openPathWithDefaultApp when Open button is clicked', async () => {
    openPathWithDefaultAppMock.mockResolvedValueOnce(undefined);
    render(<ReportResultCard data={sampleData} sessionId="session-test-123" />);

    const openButton = screen.getByTestId('deliverable-open-button');
    fireEvent.click(openButton);

    await waitFor(() => {
      expect(openPathWithDefaultAppMock).toHaveBeenCalledWith('/workspace/dist/app.zip');
    });
  });

  it('triggers downloadWorkspaceFile when Download button is clicked', async () => {
    downloadWorkspaceFileMock.mockResolvedValueOnce('/downloads/app.zip');
    render(<ReportResultCard data={sampleData} sessionId="session-test-123" />);

    const downloadButton = screen.getByTestId('deliverable-download-button');
    fireEvent.click(downloadButton);

    await waitFor(() => {
      expect(downloadWorkspaceFileMock).toHaveBeenCalledWith(
        'dist/app.zip',
        'session-test-123',
      );
    });
  });

  it('renders different badges for partial and blocked status', () => {
    const { rerender } = render(
      <ReportResultCard
        data={{
          ...sampleData,
          status: 'partial',
          title: undefined,
        }}
      />,
    );
    expect(screen.getByText('Partial result')).toBeInTheDocument();

    rerender(
      <ReportResultCard
        data={{
          ...sampleData,
          status: 'blocked',
          title: undefined,
        }}
      />,
    );
    expect(screen.getAllByText('Blocked').length).toBeGreaterThanOrEqual(1);
  });
});
