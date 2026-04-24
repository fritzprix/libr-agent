import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from 'react';
import { useThrottle } from '@/hooks/useThrottle';
import type { Message } from '@/models/chat';

interface UseChatScrollProps {
  messages: Message[];
  onReachTop?: () => void;
  canLoadOlder?: boolean;
  isLoadingOlder?: boolean;
}

const BOTTOM_THRESHOLD_PX = 80;
const TOP_LOAD_THRESHOLD_PX = 160;

export function useChatScroll({
  messages,
  onReachTop,
  canLoadOlder = false,
  isLoadingOlder = false,
}: UseChatScrollProps) {
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const [autoScrollEnabled, setAutoScrollEnabled] = useState(true);
  const autoScrollEnabledRef = useRef(true);
  const isProgrammaticScrollRef = useRef(false);
  const lastScrollTopRef = useRef(0);
  const animationFrameRef = useRef<number | null>(null);
  const pendingPrependAdjustmentRef = useRef<{
    scrollHeight: number;
    scrollTop: number;
  } | null>(null);
  const programmaticScrollTimeoutRef = useRef<ReturnType<
    typeof setTimeout
  > | null>(null);
  const topLoadTriggeredRef = useRef(false);

  const setAutoScroll = useCallback((enabled: boolean) => {
    autoScrollEnabledRef.current = enabled;
    setAutoScrollEnabled((current) =>
      current === enabled ? current : enabled,
    );
  }, []);

  const clearProgrammaticScrollTimeout = useCallback(() => {
    if (programmaticScrollTimeoutRef.current) {
      clearTimeout(programmaticScrollTimeoutRef.current);
      programmaticScrollTimeoutRef.current = null;
    }
  }, []);

  const isNearBottom = useCallback((container: HTMLDivElement) => {
    return (
      container.scrollHeight - container.scrollTop - container.clientHeight <=
      BOTTOM_THRESHOLD_PX
    );
  }, []);

  const scrollToBottom = useCallback(
    (behavior: ScrollBehavior = 'auto') => {
      const container = scrollContainerRef.current;
      if (!container) {
        return;
      }

      isProgrammaticScrollRef.current = true;
      clearProgrammaticScrollTimeout();

      container.scrollTo({
        top: container.scrollHeight,
        behavior,
      });
      lastScrollTopRef.current = container.scrollTop;

      programmaticScrollTimeoutRef.current = setTimeout(() => {
        isProgrammaticScrollRef.current = false;
        programmaticScrollTimeoutRef.current = null;
      }, 150);
    },
    [clearProgrammaticScrollTimeout],
  );

  const scheduleScrollToBottom = useCallback(
    (behavior: ScrollBehavior = 'auto') => {
      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
      }

      animationFrameRef.current = requestAnimationFrame(() => {
        animationFrameRef.current = requestAnimationFrame(() => {
          scrollToBottom(behavior);
          animationFrameRef.current = null;
        });
      });
    },
    [scrollToBottom],
  );

  const prepareForPrepend = useCallback(() => {
    const container = scrollContainerRef.current;
    if (!container) {
      return;
    }

    pendingPrependAdjustmentRef.current = {
      scrollHeight: container.scrollHeight,
      scrollTop: container.scrollTop,
    };
  }, []);

  const handleScroll = useThrottle(() => {
    const container = scrollContainerRef.current;
    if (!container) {
      return;
    }

    if (isProgrammaticScrollRef.current) {
      lastScrollTopRef.current = container.scrollTop;
      return;
    }

    const currentScrollTop = container.scrollTop;
    const atBottom = isNearBottom(container);
    const isScrollingUp = currentScrollTop < lastScrollTopRef.current;

    lastScrollTopRef.current = currentScrollTop;

    if (
      onReachTop &&
      canLoadOlder &&
      !isLoadingOlder &&
      currentScrollTop <= TOP_LOAD_THRESHOLD_PX &&
      !topLoadTriggeredRef.current
    ) {
      topLoadTriggeredRef.current = true;
      onReachTop();
    }

    if (isScrollingUp && !atBottom) {
      setAutoScroll(false);
      return;
    }

    if (atBottom) {
      setAutoScroll(true);
    }

    if (currentScrollTop > TOP_LOAD_THRESHOLD_PX) {
      topLoadTriggeredRef.current = false;
    }
  }, 80);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (!container) {
      return;
    }

    lastScrollTopRef.current = container.scrollTop;
    container.addEventListener('scroll', handleScroll, { passive: true });

    return () => {
      container.removeEventListener('scroll', handleScroll);
    };
  }, [handleScroll]);

  useLayoutEffect(() => {
    const pendingPrependAdjustment = pendingPrependAdjustmentRef.current;
    const container = scrollContainerRef.current;
    if (!pendingPrependAdjustment || !container) {
      return;
    }

    const nextScrollTop =
      container.scrollHeight -
      pendingPrependAdjustment.scrollHeight +
      pendingPrependAdjustment.scrollTop;
    container.scrollTop = nextScrollTop;
    lastScrollTopRef.current = nextScrollTop;
    pendingPrependAdjustmentRef.current = null;
    topLoadTriggeredRef.current = false;
  }, [messages]);

  useLayoutEffect(() => {
    if (autoScrollEnabledRef.current) {
      scheduleScrollToBottom('auto');
    }
  }, [messages, scheduleScrollToBottom]);

  useEffect(() => {
    const content = contentRef.current;
    if (!content) {
      return;
    }

    const resizeObserver = new ResizeObserver(() => {
      if (autoScrollEnabledRef.current) {
        scheduleScrollToBottom('auto');
      }
    });

    resizeObserver.observe(content);

    return () => {
      resizeObserver.disconnect();
    };
  }, [messages.length, scheduleScrollToBottom]);

  useEffect(() => {
    const container = scrollContainerRef.current;
    if (container && isNearBottom(container)) {
      setAutoScroll(true);
    }
  }, [isNearBottom, setAutoScroll]);

  useEffect(() => {
    if (!isLoadingOlder) {
      topLoadTriggeredRef.current = false;
    }
  }, [isLoadingOlder]);

  useEffect(() => {
    return () => {
      clearProgrammaticScrollTimeout();

      if (animationFrameRef.current !== null) {
        cancelAnimationFrame(animationFrameRef.current);
        animationFrameRef.current = null;
      }
    };
  }, [clearProgrammaticScrollTimeout]);

  return {
    messagesEndRef,
    scrollContainerRef,
    contentRef,
    autoScrollEnabled,
    scrollToBottom,
    prepareForPrepend,
  };
}
