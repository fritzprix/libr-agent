/**
 * Scroll-behavior tests for ProcessOutputDialog (issue #2615).
 *
 * jsdom has no layout, so the Radix viewport metrics (scrollHeight,
 * clientHeight, scrollTop) are stubbed directly on the viewport element —
 * same approach as AgentChatMessages scroll-follow tests.
 */
import { act, fireEvent, render } from '@testing-library/react';
import '@testing-library/jest-dom';
import { describe, expect, it, vi } from 'vitest';
import { ProcessOutputDialog } from '../ProcessOutputDialog';
import type { ReadProcessOutputResult } from '../types';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (
      key: string,
      defaultOrOptions?: string | Record<string, unknown>,
    ) => (typeof defaultOrOptions === 'string' ? defaultOrOptions : key),
  }),
}));

function makeResult(label: string): ReadProcessOutputResult {
  return {
    process_id: 'proc-1',
    stream: 'both',
    mode: 'tail',
    status: 'running',
    outputs: {
      stdout: { content: [`${label}-line-1`, `${label}-line-2`] },
      stderr: { content: [] },
    },
  };
}

function getViewport(): HTMLElement {
  const viewport = document.querySelector(
    '[data-radix-scroll-area-viewport]',
  ) as HTMLElement | null;
  expect(viewport).not.toBeNull();
  return viewport as HTMLElement;
}

function setViewportMetrics(
  viewport: HTMLElement,
  metrics: { scrollHeight: number; clientHeight: number },
): void {
  Object.defineProperty(viewport, 'scrollHeight', {
    value: metrics.scrollHeight,
    configurable: true,
  });
  Object.defineProperty(viewport, 'clientHeight', {
    value: metrics.clientHeight,
    configurable: true,
  });
  // jsdom has no layout, so scrollTop is not clamped like a real scroll box.
  // Emulate the browser clamping to [0, scrollHeight - clientHeight] so that
  // asserting "pinned to the bottom" matches what a user would see.
  let top = 0;
  Object.defineProperty(viewport, 'scrollTop', {
    get: () => top,
    set: (value: number) => {
      top = Math.max(
        0,
        Math.min(value, metrics.scrollHeight - metrics.clientHeight),
      );
    },
    configurable: true,
  });
}

describe('ProcessOutputDialog scroll behavior', () => {
  it('pins to the bottom when output loads and keeps pinning on new results', () => {
    const { rerender } = render(
      <ProcessOutputDialog
        open
        loading={false}
        result={null}
        onOpenChange={() => {}}
      />,
    );

    // No output yet: viewport is mounted, nothing to pin to.
    const viewport = getViewport();
    setViewportMetrics(viewport, { scrollHeight: 1000, clientHeight: 200 });
    viewport.scrollTop = 0;

    act(() => {
      rerender(
        <ProcessOutputDialog
          open
          loading={false}
          result={makeResult('first')}
          onOpenChange={() => {}}
        />,
      );
    });

    expect(viewport.scrollTop).toBe(800);

    // A new result while pinned keeps the view at the (grown) bottom.
    setViewportMetrics(viewport, { scrollHeight: 2000, clientHeight: 200 });

    act(() => {
      rerender(
        <ProcessOutputDialog
          open
          loading={false}
          result={makeResult('second')}
          onOpenChange={() => {}}
        />,
      );
    });

    expect(viewport.scrollTop).toBe(1800);
  });

  it('stops pinning after the user scrolls more than 48px above the bottom', () => {
    const { rerender } = render(
      <ProcessOutputDialog
        open
        loading={false}
        result={makeResult('first')}
        onOpenChange={() => {}}
      />,
    );

    const viewport = getViewport();
    setViewportMetrics(viewport, { scrollHeight: 2000, clientHeight: 200 });
    viewport.scrollTop = 1800;

    // Scroll up far enough (1300px above bottom) to release the pin.
    viewport.scrollTop = 500;
    fireEvent.scroll(viewport);

    act(() => {
      rerender(
        <ProcessOutputDialog
          open
          loading={false}
          result={makeResult('third')}
          onOpenChange={() => {}}
        />,
      );
    });

    expect(viewport.scrollTop).toBe(500);
  });

  it('re-pins when the dialog is closed and reopened', () => {
    const result = makeResult('stable');
    const { rerender } = render(
      <ProcessOutputDialog
        open
        loading={false}
        result={result}
        onOpenChange={() => {}}
      />,
    );

    const firstViewport = getViewport();
    setViewportMetrics(firstViewport, {
      scrollHeight: 2000,
      clientHeight: 200,
    });
    firstViewport.scrollTop = 500;
    fireEvent.scroll(firstViewport);

    act(() => {
      rerender(
        <ProcessOutputDialog
          open={false}
          loading={false}
          result={result}
          onOpenChange={() => {}}
        />,
      );
    });

    act(() => {
      rerender(
        <ProcessOutputDialog
          open
          loading={false}
          result={result}
          onOpenChange={() => {}}
        />,
      );
    });

    const reopenedViewport = getViewport();
    setViewportMetrics(reopenedViewport, {
      scrollHeight: 2000,
      clientHeight: 200,
    });

    act(() => {
      rerender(
        <ProcessOutputDialog
          open
          loading={false}
          result={makeResult('after-reopen')}
          onOpenChange={() => {}}
        />,
      );
    });

    expect(reopenedViewport.scrollTop).toBe(1800);
  });
});
