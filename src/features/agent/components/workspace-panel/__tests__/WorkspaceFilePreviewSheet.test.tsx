import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
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

const mockUseIsDarkMode = vi.fn().mockReturnValue(true);

vi.mock('@/hooks/use-is-dark-mode', () => ({
  useIsDarkMode: () => mockUseIsDarkMode(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultVal?: string) => defaultVal ?? key,
  }),
}));

// Mock CodeBlock
vi.mock('@/features/agent/components/AgentMessageRenderer/components/CodeBlock', () => ({
  CodeBlock: ({
    children,
    className,
    isDark,
  }: {
    children: React.ReactNode;
    className?: string;
    isDark?: boolean;
  }) => (
    <div
      data-testid="code-block"
      className={className}
      data-is-dark={String(isDark)}
    >
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
    mockUseIsDarkMode.mockReturnValue(true);
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
      },
    });
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
      expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    });

    // Toggle to Source mode
    const sourceBtn = screen.getByRole('button', { name: /Source/i });
    fireEvent.click(sourceBtn);

    await waitFor(() => {
      expect(screen.getByTestId('code-block')).toBeInTheDocument();
      expect(screen.getByText('<h1>Interactive Chart</h1>')).toBeInTheDocument();
    });
  });

  it('passes isDark to CodeBlock', async () => {
    const codeFile: FileNode = {
      id: 'app.ts',
      name: 'app.ts',
      path: 'app.ts',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: 'const x = 1;',
      isBinary: false,
      size: 12,
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
      const codeBlock = screen.getByTestId('code-block');
      expect(codeBlock).toHaveAttribute('data-is-dark', 'true');
    });
  });

  it('allows copying empty 0-byte files', async () => {
    const emptyFile: FileNode = {
      id: 'empty.txt',
      name: 'empty.txt',
      path: 'empty.txt',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: '',
      isBinary: false,
      size: 0,
      mimeType: 'text/plain',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={emptyFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /Copy Content/i }),
      ).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole('button', { name: /Copy Content/i }));
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('');
  });

  it('allows copying file path', async () => {
    const nestedFile: FileNode = {
      id: 'src/utils/math.ts',
      name: 'math.ts',
      path: 'src/utils/math.ts',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: 'export const add = 1;',
      isBinary: false,
      size: 21,
      mimeType: 'text/typescript',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={nestedFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId('path-breadcrumb')).toBeInTheDocument();
    });

    const copyPathBtn = screen.getByRole('button', { name: /Copy file path/i });
    fireEvent.click(copyPathBtn);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
      'src/utils/math.ts',
    );
  });

  it('toggles maximized sheet width', async () => {
    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: 'const x = 1;',
      isBinary: false,
      size: 12,
      mimeType: 'text/typescript',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={defaultFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /Maximize/i }),
      ).toBeInTheDocument();
    });

    const maxBtn = screen.getByRole('button', { name: /Maximize/i });
    fireEvent.click(maxBtn);

    expect(
      screen.getByRole('button', { name: /Restore size/i }),
    ).toBeInTheDocument();
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

  it('renders safe markdown links as anchors and sanitizes unsafe links as spans', async () => {
    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: `
[Safe HTTPS](https://example.com/docs)
[Safe Mail](mailto:test@example.com)
[Unsafe JS](javascript:alert(1))
[Unsafe Data](data:text/html,<script>alert(1)</script>)
[Unsafe Scheme Relative](//evil.com/xss)
`,
      isBinary: false,
      size: 100,
      mimeType: 'text/markdown',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={defaultFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText('Safe HTTPS')).toBeInTheDocument();
    });

    const safeHttpsLink = screen.getByText('Safe HTTPS');
    expect(safeHttpsLink.tagName.toLowerCase()).toBe('a');
    expect(safeHttpsLink).toHaveAttribute('href', 'https://example.com/docs');
    expect(safeHttpsLink).toHaveAttribute('target', '_blank');

    const safeMailLink = screen.getByText('Safe Mail');
    expect(safeMailLink.tagName.toLowerCase()).toBe('a');
    expect(safeMailLink).toHaveAttribute('href', 'mailto:test@example.com');

    const unsafeJsLink = screen.getByText('Unsafe JS');
    expect(unsafeJsLink.tagName.toLowerCase()).toBe('span');
    expect(unsafeJsLink).not.toHaveAttribute('href');

    const unsafeDataLink = screen.getByText('Unsafe Data');
    expect(unsafeDataLink.tagName.toLowerCase()).toBe('span');
    expect(unsafeDataLink).not.toHaveAttribute('href');

    const unsafeSchemeRelativeLink = screen.getByText('Unsafe Scheme Relative');
    expect(unsafeSchemeRelativeLink.tagName.toLowerCase()).toBe('span');
    expect(unsafeSchemeRelativeLink).not.toHaveAttribute('href');
  });

  it('retains displayed content during exit transition when file becomes null and isOpen becomes false', async () => {
    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: '# Preserved Markdown Content',
      isBinary: false,
      size: 30,
      mimeType: 'text/markdown',
    });

    const { rerender } = render(
      <WorkspaceFilePreviewSheet
        file={defaultFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      expect(screen.getByText('Preserved Markdown Content')).toBeInTheDocument();
      expect(screen.getByRole('heading', { name: 'test.md' })).toBeInTheDocument();
    });

    // When caller clears file before/during sheet dismiss, activeFile retains displayedFile
    rerender(
      <WorkspaceFilePreviewSheet
        file={null}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    // activeFile retains displayedFile so sheet content does not abruptly vanish
    expect(screen.getByText('Preserved Markdown Content')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'test.md' })).toBeInTheDocument();
  });

  it('renders path as a segmented breadcrumb navigation', async () => {
    const deeplyNestedFile: FileNode = {
      id: 'src/components/ui/button.tsx',
      name: 'button.tsx',
      path: 'src/components/ui/button.tsx',
      isDirectory: false,
    };

    mockReadWorkspaceFileContent.mockResolvedValueOnce({
      content: 'export const Button = () => null;',
      isBinary: false,
      size: 33,
      mimeType: 'text/typescript',
    });

    render(
      <WorkspaceFilePreviewSheet
        file={deeplyNestedFile}
        isOpen={true}
        onClose={vi.fn()}
        onOpenInDefaultApp={mockOpenWorkspaceFileWithDefaultApp}
      />,
    );

    await waitFor(() => {
      const breadcrumb = screen.getByTestId('path-breadcrumb');
      expect(breadcrumb).toBeInTheDocument();
      expect(breadcrumb).toHaveAttribute('title', 'src/components/ui/button.tsx');
      expect(within(breadcrumb).getByText('src')).toBeInTheDocument();
      expect(within(breadcrumb).getByText('components')).toBeInTheDocument();
      expect(within(breadcrumb).getByText('ui')).toBeInTheDocument();
      expect(within(breadcrumb).getByText('button.tsx')).toBeInTheDocument();
    });
  });
});
