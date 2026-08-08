import { act, renderHook } from '@testing-library/react';
import { useRef } from 'react';
import type { VirtuosoHandle } from 'react-virtuoso';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { BottomAlignmentPhase } from '../../../types';
import { useScrollScheduler } from '../useScrollScheduler';

function mockVirtuoso(
  scrollToIndex: VirtuosoHandle['scrollToIndex'],
): VirtuosoHandle {
  return { scrollToIndex } as VirtuosoHandle;
}

function mockFooter(scrollIntoView: HTMLDivElement['scrollIntoView']) {
  return { scrollIntoView } as HTMLDivElement;
}

function useSchedulerHarness(overrides?: {
  upwardReleaseDistance?: number;
  visualBottom?: boolean;
  shouldFollowLatest?: boolean;
  itemCount?: number;
  virtuoso?: VirtuosoHandle | null;
  footerEnd?: HTMLDivElement | null;
}) {
  const virtuosoRef = useRef<VirtuosoHandle | null>(overrides?.virtuoso ?? null);
  const footerEndRef = useRef<HTMLDivElement | null>(
    overrides?.footerEnd ?? null,
  );
  const groupedMessageCountRef = useRef(overrides?.itemCount ?? 3);
  const selfScrollIgnoreUntilRef = useRef(0);
  const shouldFollowLatestRef = useRef(overrides?.shouldFollowLatest ?? true);
  const isPreservingPrependPositionRef = useRef(false);
  const isHistoryBrowsingRef = useRef(false);
  const upwardReleaseDistanceRef = useRef(
    overrides?.upwardReleaseDistance ?? 0,
  );
  const bottomAlignmentPhaseRef = useRef<BottomAlignmentPhase>('idle');
  const bottomAlignmentLayoutVersionRef = useRef(0);
  const bottomAlignmentRequestedVersionRef = useRef(0);
  const visualBottomRef = useRef(overrides?.visualBottom ?? true);
  const scrollTopRef = useRef(200);
  const logScrollState = vi.fn();

  const scheduler = useScrollScheduler({
    virtuosoRef,
    footerEndRef,
    groupedMessageCountRef,
    selfScrollIgnoreUntilRef,
    shouldFollowLatestRef,
    isPreservingPrependPositionRef,
    isHistoryBrowsingRef,
    upwardReleaseDistanceRef,
    bottomAlignmentPhaseRef,
    bottomAlignmentLayoutVersionRef,
    bottomAlignmentRequestedVersionRef,
    visualBottomRef,
    scrollTopRef,
    setBottomAlignmentPhase: vi.fn(),
    scheduleBottomAlignmentVerification: vi.fn(),
    logScrollState,
  });

  return { ...scheduler, logScrollState };
}

describe('useScrollScheduler', () => {
  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      cb(0);
      return 1;
    });
    vi.stubGlobal('cancelAnimationFrame', vi.fn());
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it('aligns footer sentinel after Virtuoso scrollToIndex (#1647)', () => {
    const scrollToIndex = vi.fn();
    const scrollIntoView = vi.fn();

    const { result } = renderHook(() =>
      useSchedulerHarness({
        virtuoso: mockVirtuoso(scrollToIndex),
        footerEnd: mockFooter(scrollIntoView),
      }),
    );

    act(() => {
      result.current.scheduleScrollToBottom('manual-scroll-to-bottom');
    });

    expect(scrollToIndex).toHaveBeenCalledWith({
      index: 'LAST',
      align: 'end',
      behavior: 'auto',
    });
    expect(scrollIntoView).toHaveBeenCalledWith({
      block: 'end',
      inline: 'nearest',
      behavior: 'auto',
    });
  });

  it('falls back to footer sentinel when Virtuoso is unavailable', () => {
    const scrollIntoView = vi.fn();

    const { result } = renderHook(() =>
      useSchedulerHarness({
        virtuoso: null,
        footerEnd: mockFooter(scrollIntoView),
        itemCount: 2,
      }),
    );

    act(() => {
      result.current.scheduleScrollToBottom('manual-scroll-to-bottom');
    });

    expect(scrollIntoView).toHaveBeenCalledWith({
      block: 'end',
      inline: 'nearest',
      behavior: 'auto',
    });
  });

  it('suppresses new auto-scroll while upward release distance is accumulating', () => {
    const scrollToIndex = vi.fn();

    const { result } = renderHook(() =>
      useSchedulerHarness({
        virtuoso: mockVirtuoso(scrollToIndex),
        upwardReleaseDistance: 12,
        visualBottom: false,
        shouldFollowLatest: true,
      }),
    );

    act(() => {
      result.current.scheduleScrollToBottom('content-resize-observer');
    });

    expect(scrollToIndex).not.toHaveBeenCalled();
    expect(result.current.logScrollState).toHaveBeenCalledWith(
      'scheduleScrollToBottom:suppressed',
      expect.objectContaining({
        shouldSuppressForUpwardIntent: true,
        upwardReleaseDistance: 12,
      }),
    );
  });

  it('suppresses auto-scroll when follow is paused even if visualBottom is true', () => {
    const scrollToIndex = vi.fn();

    const { result } = renderHook(() =>
      useSchedulerHarness({
        virtuoso: mockVirtuoso(scrollToIndex),
        visualBottom: true,
        shouldFollowLatest: false,
      }),
    );

    act(() => {
      result.current.scheduleScrollToBottom('content-resize-observer');
    });

    expect(scrollToIndex).not.toHaveBeenCalled();
    expect(result.current.logScrollState).toHaveBeenCalledWith(
      'scheduleScrollToBottom:suppressed',
      expect.objectContaining({
        shouldSuppressForPausedFollow: true,
      }),
    );
  });

  it('suppresses auto-scroll while history browsing even if follow is still on', () => {
    const scrollToIndex = vi.fn();
    const isHistoryBrowsingRef = { current: true };

    const { result } = renderHook(() => {
      const virtuosoRef = useRef(mockVirtuoso(scrollToIndex));
      const footerEndRef = useRef<HTMLDivElement | null>(null);
      const groupedMessageCountRef = useRef(3);
      const selfScrollIgnoreUntilRef = useRef(0);
      const shouldFollowLatestRef = useRef(true);
      const isPreservingPrependPositionRef = useRef(false);
      const upwardReleaseDistanceRef = useRef(0);
      const bottomAlignmentPhaseRef = useRef<BottomAlignmentPhase>('idle');
      const bottomAlignmentLayoutVersionRef = useRef(0);
      const bottomAlignmentRequestedVersionRef = useRef(0);
      const visualBottomRef = useRef(true);
      const scrollTopRef = useRef(200);
      const logScrollState = vi.fn();

      return {
        ...useScrollScheduler({
          virtuosoRef,
          footerEndRef,
          groupedMessageCountRef,
          selfScrollIgnoreUntilRef,
          shouldFollowLatestRef,
          isPreservingPrependPositionRef,
          isHistoryBrowsingRef,
          upwardReleaseDistanceRef,
          bottomAlignmentPhaseRef,
          bottomAlignmentLayoutVersionRef,
          bottomAlignmentRequestedVersionRef,
          visualBottomRef,
          scrollTopRef,
          setBottomAlignmentPhase: vi.fn(),
          scheduleBottomAlignmentVerification: vi.fn(),
          logScrollState,
        }),
        logScrollState,
      };
    });

    act(() => {
      result.current.scheduleScrollToBottom('total-list-height-changed');
    });

    expect(scrollToIndex).not.toHaveBeenCalled();
    expect(result.current.logScrollState).toHaveBeenCalledWith(
      'scheduleScrollToBottom:suppressed',
      expect.objectContaining({
        shouldSuppressForHistoryBrowsing: true,
      }),
    );
  });
});
