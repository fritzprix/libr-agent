import '@testing-library/jest-dom';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { ThinkingBubble } from '../ThinkingBubble';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, defaultValueOrOptions?: string | { time?: string }) => {
      if (key === 'agent.bubble.thinkingProcessWithTime') {
        const options = defaultValueOrOptions as { time?: string } | undefined;
        return `Thinking Process ${options?.time ?? ''}`.trim();
      }
      if (typeof defaultValueOrOptions === 'string') {
        return defaultValueOrOptions;
      }
      const defaults: Record<string, string> = {
        'agent.bubble.thinkingProcess': 'Thinking Process',
        'agent.bubble.thinking': 'Thinking…',
        'agent.bubble.expandThinking': 'Expand',
      };
      return defaults[key] ?? key;
    },
  }),
}));

function expandThinking() {
  fireEvent.click(screen.getByRole('button', { name: /Thinking Process/i }));
}

function getScrollContainer(container: HTMLElement): HTMLDivElement {
  const scrollContainer = container.querySelector(
    '.overflow-y-auto',
  ) as HTMLDivElement | null;

  if (!scrollContainer) {
    throw new Error('Expected thinking scroll container');
  }

  return scrollContainer;
}

function mockScrollMetrics(
  element: HTMLDivElement,
  metrics: { scrollHeight: number; clientHeight: number; scrollTop: number },
) {
  Object.defineProperty(element, 'scrollHeight', {
    configurable: true,
    value: metrics.scrollHeight,
  });
  Object.defineProperty(element, 'clientHeight', {
    configurable: true,
    value: metrics.clientHeight,
  });
  element.scrollTop = metrics.scrollTop;
}

describe('ThinkingBubble', () => {
  it('is collapsed by default and shows a truncated preview', () => {
    const longThinking = 'a'.repeat(120);
    render(<ThinkingBubble thinking={longThinking} isStreaming={false} />);

    expect(screen.getByText(/a{80}…/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Expand' })).toBeInTheDocument();
    expect(screen.queryByText(longThinking)).not.toBeInTheDocument();
  });

  it('expands to show full thinking content', () => {
    const longThinking = 'a'.repeat(120);
    render(<ThinkingBubble thinking={longThinking} isStreaming={false} />);

    expandThinking();

    expect(screen.getByText(longThinking)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Expand' })).not.toBeInTheDocument();
  });

  it('keeps its internal scroll pinned to the bottom while streaming', () => {
    const { container, rerender } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    const scrollContainer = getScrollContainer(container);
    mockScrollMetrics(scrollContainer, {
      scrollHeight: 240,
      clientHeight: 128,
      scrollTop: 0,
    });

    rerender(
      <ThinkingBubble
        thinking={'first line\nsecond line\nthird line'}
        isStreaming={true}
      />,
    );

    expect(scrollContainer.scrollTop).toBe(240);
  });

  it('does not auto-pin after the user scrolls upward inside the panel', () => {
    const { container, rerender } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    const scrollContainer = getScrollContainer(container);
    mockScrollMetrics(scrollContainer, {
      scrollHeight: 240,
      clientHeight: 128,
      scrollTop: 0,
    });

    scrollContainer.scrollTop = 40;
    fireEvent.scroll(scrollContainer);

    rerender(
      <ThinkingBubble
        thinking={'first line\nsecond line\nthird line'}
        isStreaming={true}
      />,
    );

    expect(scrollContainer.scrollTop).toBe(40);
  });

  it('resumes auto-pin after the user scrolls back to the bottom', () => {
    const { container, rerender } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    const scrollContainer = getScrollContainer(container);
    mockScrollMetrics(scrollContainer, {
      scrollHeight: 240,
      clientHeight: 128,
      scrollTop: 0,
    });

    scrollContainer.scrollTop = 40;
    fireEvent.scroll(scrollContainer);

    scrollContainer.scrollTop = 112;
    fireEvent.scroll(scrollContainer);

    rerender(
      <ThinkingBubble
        thinking={'first line\nsecond line\nthird line'}
        isStreaming={true}
      />,
    );

    expect(scrollContainer.scrollTop).toBe(240);
  });

  it('does not auto-pin when chat scroll follow is disabled', () => {
    const { container, rerender } = render(
      <ThinkingBubble
        thinking="first line"
        isStreaming={true}
        followChatScroll={false}
      />,
    );

    const scrollContainer = getScrollContainer(container);
    mockScrollMetrics(scrollContainer, {
      scrollHeight: 240,
      clientHeight: 128,
      scrollTop: 0,
    });

    rerender(
      <ThinkingBubble
        thinking={'first line\nsecond line\nthird line'}
        isStreaming={true}
        followChatScroll={false}
      />,
    );

    expect(scrollContainer.scrollTop).toBe(0);
  });

  it('auto-expands while streaming', () => {
    const { container } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    expect(container.querySelector('.overflow-y-auto')).not.toBeNull();
    expect(screen.getByRole('button', { name: /Thinking Process/i })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
  });

  it('allows collapsing while streaming', () => {
    const { container } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    fireEvent.click(screen.getByRole('button', { name: /Thinking Process/i }));

    expect(container.querySelector('.overflow-y-auto')).toBeNull();
  });
});
