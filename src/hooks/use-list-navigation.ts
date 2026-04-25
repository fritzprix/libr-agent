import {
  useState,
  useEffect,
  useCallback,
  type DependencyList,
  type SetStateAction,
} from 'react';

export interface UseListNavigationProps {
  itemCount: number;
  onEnter: (index: number) => void;
  onEscape?: () => void;
  resetDependencies?: DependencyList;
}

function clampIndex(index: number, itemCount: number) {
  if (itemCount <= 0) {
    return 0;
  }

  return Math.min(Math.max(index, 0), itemCount - 1);
}

export function useListNavigation({
  itemCount,
  onEnter,
  onEscape,
  resetDependencies = [],
}: UseListNavigationProps) {
  const [internalActiveIndex, setInternalActiveIndex] = useState(0);
  const activeIndex = clampIndex(internalActiveIndex, itemCount);

  const setActiveIndex = useCallback(
    (value: SetStateAction<number>) => {
      setInternalActiveIndex((currentIndex) =>
        clampIndex(
          typeof value === 'function' ? value(currentIndex) : value,
          itemCount,
        ),
      );
    },
    [itemCount],
  );

  // Reset active index when items array length changes (if enabled)
  useEffect(() => {
    setInternalActiveIndex(0);
  }, resetDependencies);

  useEffect(() => {
    setInternalActiveIndex((currentIndex) =>
      clampIndex(currentIndex, itemCount),
    );
  }, [itemCount]);

  useEffect(() => {
    if (itemCount === 0) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setActiveIndex((index) => index + 1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setActiveIndex((index) => index - 1);
      } else if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        onEnter(activeIndex);
      } else if (e.key === 'Escape' && onEscape) {
        e.preventDefault();
        onEscape();
      }
    };

    window.addEventListener('keydown', handler, { capture: true });
    return () =>
      window.removeEventListener('keydown', handler, { capture: true });
  }, [itemCount, activeIndex, onEnter, onEscape, setActiveIndex]);

  return { activeIndex, setActiveIndex };
}
