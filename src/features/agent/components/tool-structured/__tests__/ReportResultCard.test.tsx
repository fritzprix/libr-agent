import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { ReportResultCard } from '../ReportResultCard';
import type { ReportResultData } from '../types';

const openPathWithDefaultAppMock = vi.fn();
const downloadWorkspaceFileMock = vi.fn();
const openExternalUrlMock = vi.fn();
const openWorkspaceFileWithDefaultAppMock = vi.fn();

vi.mock('@/lib/backend', () => ({
  openPathWithDefaultApp: (...args: unknown[]) => openPathWithDefaultAppMock(...args),
  downloadWorkspaceFile: (...args: unknown[]) => downloadWorkspaceFileMock(...args),
  openExternalUrl: (...args: unknown[]) => openExternalUrlMock(...args),
  openWorkspaceFileWithDefaultApp: (...args: unknown[]) =>
    openWorkspaceFileWithDefaultAppMock(...args),
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
  useOptionalAgentSessionState: () => ({
    session: { id: 'session-test-123' },
  }),
}));

vi.mock('@/hooks/use-is-dark-mode', () => ({
  useIsDarkMode: () => false,
}));

vi.mock('@/lib/mermaid/loader', () => ({
  renderMermaidSvg: vi.fn(async () =>
    '<svg xmlns="http://www.w3.org/2000/svg" data-testid="mermaid-svg" width="10" height="10"></svg>',
  ),
  resetMermaidLoaderForTests: vi.fn(),
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

  it('opens workspace markdown links via preview instead of navigating', async () => {
    render(
      <ReportResultCard
        data={{
          ...sampleData,
          result: 'See [notes](notes.md) for details.',
          deliverables: [],
        }}
        sessionId="session-test-123"
      />,
    );

    const link = screen.getByTestId('report-result-markdown-link');
    fireEvent.click(link);

    await waitFor(() => {
      expect(openFilePreviewMock).toHaveBeenCalledWith({
        path: 'notes.md',
        name: 'notes.md',
        sessionId: 'session-test-123',
      });
    });
    expect(openWorkspaceFileWithDefaultAppMock).not.toHaveBeenCalled();
  });

  it('opens https markdown links externally', async () => {
    openExternalUrlMock.mockResolvedValueOnce(undefined);
    render(
      <ReportResultCard
        data={{
          ...sampleData,
          result: 'Docs: [site](https://example.com/docs)',
          deliverables: [],
        }}
        sessionId="session-test-123"
      />,
    );

    fireEvent.click(screen.getByTestId('report-result-markdown-link'));

    await waitFor(() => {
      expect(openExternalUrlMock).toHaveBeenCalledWith('https://example.com/docs');
    });
  });

  it('renders criteria and proof as markdown', () => {
    render(
      <ReportResultCard
        data={{
          ...sampleData,
          criteria: '- **pass** unit tests',
          proof: 'Ran `cargo test`',
          deliverables: [],
        }}
      />,
    );

    const criteria = screen.getByTestId('report-result-criteria');
    expect(criteria.querySelector('strong')).toHaveTextContent('pass');
    expect(criteria.querySelector('li')).toBeInTheDocument();

    const proof = screen.getByTestId('report-result-proof');
    expect(proof.querySelector('code')).toHaveTextContent('cargo test');
  });

  it('uses a single-column criteria/proof grid when only one field is present', () => {
    render(
      <ReportResultCard
        data={{
          ...sampleData,
          proof: undefined,
          deliverables: [],
        }}
      />,
    );

    const grid = screen.getByTestId('report-result-criteria-proof-grid');
    expect(grid.className).toContain('grid-cols-1');
    expect(grid.className).not.toContain('md:grid-cols-2');
  });

  it('uses a two-column criteria/proof grid when both fields are present', () => {
    render(<ReportResultCard data={{ ...sampleData, deliverables: [] }} />);

    const grid = screen.getByTestId('report-result-criteria-proof-grid');
    expect(grid.className).toContain('md:grid-cols-2');
  });

  it('copies the raw result body to the clipboard', async () => {
    const { toast } = await import('sonner');
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: { writeText },
    });

    render(
      <ReportResultCard
        data={{ ...sampleData, deliverables: [] }}
        sessionId="session-test-123"
      />,
    );

    fireEvent.click(screen.getByTestId('report-result-copy-button'));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(sampleData.result);
    });
    expect(toast.success).toHaveBeenCalled();
  });

  it('toasts an error when clipboard copy fails', async () => {
    const { toast } = await import('sonner');
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: vi.fn().mockRejectedValue(new Error('denied')),
      },
    });

    render(
      <ReportResultCard
        data={{ ...sampleData, deliverables: [] }}
        sessionId="session-test-123"
      />,
    );

    fireEvent.click(screen.getByTestId('report-result-copy-button'));

    await waitFor(() => {
      expect(toast.error).toHaveBeenCalled();
    });
  });

  it('shows a not-found tooltip with explanation for missing deliverables', () => {
    render(<ReportResultCard data={sampleData} sessionId="session-test-123" />);

    const badge = screen.getByTestId('deliverable-not-found');
    expect(badge).toBeInTheDocument();
    expect(badge).toHaveAttribute(
      'title',
      expect.stringMatching(/could not be found in the workspace/i),
    );
  });

  it('renders Mermaid fences via MermaidBlock like chat messages', async () => {
    render(
      <ReportResultCard
        data={{
          ...sampleData,
          criteria: undefined,
          proof: undefined,
          deliverables: [],
          result: ['```mermaid', 'flowchart TD', '  A-->B', '```'].join('\n'),
        }}
        sessionId="session-test-123"
      />,
    );

    expect(
      await screen.findByTestId('mermaid-diagram', {}, { timeout: 3000 }),
    ).toBeInTheDocument();
  });

  it('renders LaTeX math via KaTeX like chat messages', () => {
    const { container } = render(
      <ReportResultCard
        data={{
          ...sampleData,
          criteria: undefined,
          proof: undefined,
          deliverables: [],
          result: 'Energy is $E=mc^2$.',
        }}
        sessionId="session-test-123"
      />,
    );

    expect(container.querySelector('.katex')).toBeTruthy();
  });
});
