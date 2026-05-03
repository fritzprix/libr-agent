import '@testing-library/jest-dom';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ThinkingBubble } from '../ThinkingBubble';

describe('ThinkingBubble', () => {
  it('keeps its internal scroll pinned to the bottom while streaming', () => {
    const { container, rerender } = render(
      <ThinkingBubble thinking="first line" isStreaming={true} />,
    );

    const scrollContainer = container.querySelector(
      '.overflow-y-auto',
    ) as HTMLDivElement | null;

    expect(scrollContainer).not.toBeNull();

    if (!scrollContainer) {
      throw new Error('Expected thinking scroll container');
    }

    Object.defineProperty(scrollContainer, 'scrollHeight', {
      configurable: true,
      value: 240,
    });
    scrollContainer.scrollTop = 0;

    rerender(
      <ThinkingBubble
        thinking={'first line\nsecond line\nthird line'}
        isStreaming={true}
      />,
    );

    expect(scrollContainer.scrollTop).toBe(240);
  });
});
