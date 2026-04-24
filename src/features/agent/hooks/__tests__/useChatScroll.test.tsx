import { act, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useChatScroll } from '../useChatScroll';
import type { Message } from '@/models/chat';

interface HookHarnessProps {
  messages: Message[];
  onReachTop?: () => void;
  canLoadOlder?: boolean;
  isLoadingOlder?: boolean;
}

function HookHarness({
  messages,
  onReachTop,
  canLoadOlder = false,
  isLoadingOlder = false,
}: HookHarnessProps) {
  const { scrollContainerRef, contentRef, messagesEndRef } = useChatScroll({
    messages,
    onReachTop,
    canLoadOlder,
    isLoadingOlder,
  });

  return (
    <div ref={scrollContainerRef} data-testid="scroll-container">
      <div ref={contentRef}>
        {messages.map((message) => (
          <div key={message.id}>{message.id}</div>
        ))}
        <div ref={messagesEndRef} />
      </div>
    </div>
  );
}

const messages: Message[] = [
  {
    id: 'message-1',
    sessionId: 'session-1',
    threadId: 'session-1',
    role: 'assistant',
    content: [{ type: 'text', text: 'hello' }],
    createdAt: new Date('2026-04-24T00:00:00Z'),
    updatedAt: new Date('2026-04-24T00:00:00Z'),
  },
];

class ResizeObserverMock {
  observe() {}
  disconnect() {}
  unobserve() {}
}

describe('useChatScroll', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('ResizeObserver', ResizeObserverMock);
    vi.stubGlobal('requestAnimationFrame', vi.fn(() => 0));
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
    Object.defineProperty(HTMLDivElement.prototype, 'scrollTo', {
      configurable: true,
      value: vi.fn(),
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('triggers older-message loading when scrolling upward into the top threshold', () => {
    const onReachTop = vi.fn();
    const { getByTestId } = render(
      <HookHarness
        messages={messages}
        onReachTop={onReachTop}
        canLoadOlder
      />,
    );
    const scrollContainer = getByTestId('scroll-container');

    Object.defineProperty(scrollContainer, 'scrollHeight', {
      configurable: true,
      value: 1000,
    });
    Object.defineProperty(scrollContainer, 'clientHeight', {
      configurable: true,
      value: 400,
    });

    act(() => {
      scrollContainer.scrollTop = 300;
      fireEvent.scroll(scrollContainer);
    });

    expect(onReachTop).not.toHaveBeenCalled();

    act(() => {
      scrollContainer.scrollTop = 120;
      fireEvent.scroll(scrollContainer);
      vi.advanceTimersByTime(80);
    });

    expect(onReachTop).toHaveBeenCalledTimes(1);
  });
});
