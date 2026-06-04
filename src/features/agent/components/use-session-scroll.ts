import { useCallback, useEffect, useLayoutEffect, useRef } from 'react';

function findScrollParent(element: HTMLElement | null): HTMLElement | null {
  let current = element?.parentElement ?? null;

  while (current) {
    const { overflowY } = window.getComputedStyle(current);
    if (overflowY === 'auto' || overflowY === 'scroll') {
      return current;
    }
    current = current.parentElement;
  }

  return null;
}

interface UseInfiniteScrollProps {
  rootRef: React.RefObject<HTMLElement | null>;
  loadMoreSentinelRef: React.RefObject<HTMLElement | null>;
  hasMoreSessions: boolean;
  isLoadingMoreSessions: boolean;
  onLoadMore: () => void;
  displayRowsLength: number;
}

export function useInfiniteScroll({
  rootRef,
  loadMoreSentinelRef,
  hasMoreSessions,
  isLoadingMoreSessions,
  onLoadMore,
  displayRowsLength,
}: UseInfiniteScrollProps) {
  const scrollParentRef = useRef<HTMLElement | null>(null);

  useLayoutEffect(() => {
    scrollParentRef.current = findScrollParent(rootRef.current);
  }, [rootRef]);

  const checkShouldLoadMore = useCallback(() => {
    if (!hasMoreSessions || isLoadingMoreSessions) {
      return;
    }

    const scrollParent = scrollParentRef.current;
    const sentinel = loadMoreSentinelRef.current;
    if (!scrollParent || !sentinel) {
      return;
    }

    const sentinelBottom = sentinel.getBoundingClientRect().bottom;
    const scrollParentBottom = scrollParent.getBoundingClientRect().bottom;

    if (sentinelBottom - scrollParentBottom <= 240) {
      onLoadMore();
    }
  }, [hasMoreSessions, isLoadingMoreSessions, onLoadMore]);

  useEffect(() => {
    const scrollParent = scrollParentRef.current;
    if (!scrollParent) {
      return;
    }

    let frameId: number | null = null;
    const scheduleCheck = () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      frameId = window.requestAnimationFrame(() => {
        frameId = null;
        checkShouldLoadMore();
      });
    };

    scrollParent.addEventListener('scroll', scheduleCheck, { passive: true });
    window.addEventListener('resize', scheduleCheck);
    scheduleCheck();

    return () => {
      if (frameId !== null) {
        window.cancelAnimationFrame(frameId);
      }
      scrollParent.removeEventListener('scroll', scheduleCheck);
      window.removeEventListener('resize', scheduleCheck);
    };
  }, [checkShouldLoadMore]);

  useEffect(() => {
    const frameId = window.requestAnimationFrame(() => {
      checkShouldLoadMore();
    });

    return () => {
      window.cancelAnimationFrame(frameId);
    };
  }, [checkShouldLoadMore, displayRowsLength]);
}
