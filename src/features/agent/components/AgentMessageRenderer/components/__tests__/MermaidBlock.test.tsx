import { render, screen, waitFor, fireEvent, act } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { MermaidBlock } from '../MermaidBlock';
import { CodeBlock } from '../CodeBlock';
import { resetMermaidLoaderForTests } from '@/lib/mermaid/loader';

const renderMock = vi.fn();
const initializeMock = vi.fn();

vi.mock('mermaid', () => ({
  default: {
    initialize: (...args: unknown[]) => initializeMock(...args),
    render: (...args: unknown[]) => renderMock(...args),
  },
}));

describe('MermaidBlock', () => {
  beforeEach(() => {
    resetMermaidLoaderForTests();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    initializeMock.mockClear();
    renderMock.mockReset();
    renderMock.mockResolvedValue({ svg: '<svg data-testid="fake-svg"></svg>' });
  });

  afterEach(() => {
    vi.useRealTimers();
    resetMermaidLoaderForTests();
  });

  it('renders valid mermaid code as SVG', async () => {
    render(<MermaidBlock code={'flowchart TD\n  A-->B'} isDark={false} />);

    expect(screen.getByTestId('mermaid-skeleton')).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(150);
    });

    await waitFor(() => {
      expect(screen.getByTestId('mermaid-diagram')).toBeInTheDocument();
    });
    expect(initializeMock).toHaveBeenCalledWith(
      expect.objectContaining({
        securityLevel: 'strict',
        theme: 'default',
      }),
    );
    expect(renderMock).toHaveBeenCalled();
  });

  it('re-inits with dark theme', async () => {
    render(<MermaidBlock code={'flowchart TD\n  A-->B'} isDark />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(150);
    });

    await waitFor(() => {
      expect(initializeMock).toHaveBeenCalledWith(
        expect.objectContaining({ theme: 'dark' }),
      );
    });
  });

  it('shows error UI with expandable source for invalid code', async () => {
    renderMock.mockRejectedValueOnce(new Error('Parse error'));

    render(<MermaidBlock code="not a diagram" isDark={false} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(150);
    });

    await waitFor(() => {
      expect(screen.getByTestId('mermaid-error')).toBeInTheDocument();
    });
    expect(screen.getByText(/Parse error/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: /Show source/i }));
    expect(screen.getByText('not a diagram')).toBeInTheDocument();
  });
});

describe('CodeBlock mermaid routing', () => {
  beforeEach(() => {
    resetMermaidLoaderForTests();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    initializeMock.mockClear();
    renderMock.mockReset();
    renderMock.mockResolvedValue({ svg: '<svg></svg>' });
  });

  afterEach(() => {
    vi.useRealTimers();
    resetMermaidLoaderForTests();
  });

  it('routes language-mermaid to MermaidBlock', async () => {
    render(
      <CodeBlock className="language-mermaid" isDark={false}>
        {'flowchart TD\n  A-->B'}
      </CodeBlock>,
    );

    expect(screen.getByTestId('mermaid-skeleton')).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(150);
    });

    await waitFor(() => {
      expect(screen.getByTestId('mermaid-diagram')).toBeInTheDocument();
    });
  });

  it('keeps non-mermaid languages on prism path', () => {
    const { container } = render(
      <CodeBlock className="language-javascript" isDark={false}>
        {'const x = 1;'}
      </CodeBlock>,
    );

    expect(screen.queryByTestId('mermaid-skeleton')).not.toBeInTheDocument();
    expect(container.querySelector('code.language-javascript')).toBeTruthy();
  });
});
