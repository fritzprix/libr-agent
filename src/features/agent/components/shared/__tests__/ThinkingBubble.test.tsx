import '@testing-library/jest-dom';
import { fireEvent, render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ThinkingBubble } from '../ThinkingBubble';

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
});
