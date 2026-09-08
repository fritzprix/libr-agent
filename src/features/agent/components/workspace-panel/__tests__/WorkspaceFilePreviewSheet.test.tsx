import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import '@testing-library/jest-dom';
import { WorkspaceFilePreviewSheet } from '../WorkspaceFilePreviewSheet';
import type { FileNode } from '../types';

const mockReadWorkspaceFileContent = vi.fn();
const mockOpenWorkspaceFileWithDefaultApp = vi.fn();

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => ({
    readWorkspaceFileContent: mockReadWorkspaceFileContent,
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultVal?: string) => defaultVal ?? key,
  }),
}));

// Mock CodeBlock
vi.mock('@/features/agent/components/AgentMessageRenderer/components/CodeBlock', () => ({
  CodeBlock: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="code-block" className={className}>
      {children}
    </div>
  ),
}));

describe('WorkspaceFilePreviewSheet', () => {
  const defaultFile: FileNode = {
    id: 'test.md',
    name: 'test.md',
    path: 'test.md',
    isDirectory: false,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing when file is null or isOpen is false', () => {
    const { container } = render(
      <WorkspaceFilePreviewSheet
        file={null}
        isOpen={false}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it('loads and displays markdown content', async () => {
    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: '# Hello Markdown',
      isBinary: false,
      size: 16,
      mimeType: 'text/markdown',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={defaultFile}
        sessionId="session-123"
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText('Hello Markdown')).toBeInTheDocument();
    });

    expect(mockReadWorkspaceFileContent).toHaveBeenCalledWith('test.md', 'session-123');
  });

  it('loads and displays code file using CodeBlock', async () => {
    const codeFile: FileNode = {
      id: 'app.ts',
      name: 'app.ts',
      path: 'app.ts',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: 'const x: number = 42;',
      isBinary: false,
      size: 21,
      mimeType: 'text/typescript',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={codeFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('code-block')).toBeInTheDocument();
      expect(screen.getByText('const x: number = 42;')).toBeInTheDocument();
    });
  });

  it('renders plain pre for code files exceeding 200KB', async () => {
    const codeFile: FileNode = {
      id: 'big.ts',
      name: 'big.ts',
      path: 'big.ts',
      isDirectory: false,
    };

    const bigContent = 'x'.repeat(201 * 1024);
    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: bigContent,
      isBinary: false,
      size: bigContent.length,
      mimeType: 'text/typescript',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={codeFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText(/syntax highlighting disabled/i)).toBeInTheDocument();
      expect(screen.queryByTestId('code-block')).not.toBeInTheDocument();
    });
  });

  it('renders HTML in preview mode by default and toggles to source mode', async () => {
    const htmlFile: FileNode = {
      id: 'index.html',
      name: 'index.html',
      path: 'index.html',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: '<h1>Interactive Chart</h1>',
      isBinary: false,
      size: 26,
      mimeType: 'text/html',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={htmlFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      const iframe = screen.getByTestId('html-preview-iframe');
      expect(iframe).toBeInTheDocument();
      expect(iframe).toHaveAttribute('srcDoc', '<h1>Interactive Chart</h1>');
      expect(iframe).toHaveAttribute('sandbox', 'allow-scripts allow-modals');
    });

    // Toggle to Source mode
    const sourceBtn = screen.getByRole('button', { name: /Source/i });
    fireEvent.click(sourceBtn);

    await waitFor(() => {
      expect(screen.getByTestId('code-block')).toBeInTheDocument();
      expect(screen.getByText('<h1>Interactive Chart</h1>')).toBeInTheDocument();
    });
  });

  it('shows unsupported binary message for non-image binary files and allows opening in default app', async () => {
    const binFile: FileNode = {
      id: 'corrupt.txt',
      name: 'corrupt.txt',
      path: 'corrupt.txt',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: '',
      isBinary: true,
      size: 50,
      mimeType: 'text/plain',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={binFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByText(/Binary or unsupported encoding/i),
      ).toBeInTheDocument();
    });

    const openBtns = screen.getAllByRole('button', { name: /Open in Default App/i });
    fireEvent.click(openBtns[0]);

    expect(mockOpenWorkspaceFileWithDefaultApp).toHaveBeenCalledWith('corrupt.txt');
  });
});
