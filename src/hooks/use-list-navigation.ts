import { useState, useEffect, type DependencyList } from 'react';

export interface UseListNavigationProps {
  itemCount: number;
  onEnter: (index: number) => void;
  onEscape?: () => void;
  resetDependencies?: DependencyList;
}

export function useListNavigation({
  itemCount,
  onEnter,
  onEscape,
  resetDependencies = [],
}: UseListNavigationProps) {
  const [internalActiveIndex, setInternalActiveIndex] = useState(0);

  // Ensure activeIndex is bounded by the new count during render to avoid action-effect chains
  const activeIndex =
    itemCount > 0 ? Math.min(internalActiveIndex, itemCount - 1) : 0;

  // Reset active index when items array length changes (if enabled)
  useEffect(() => {
    setInternalActiveIndex(0);
  }, resetDependencies);

  useEffect(() => {
    if (itemCount === 0) return;

    const handler = (e: KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setInternalActiveIndex((i) => Math.min(i + 1, itemCount - 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setInternalActiveIndex((i) => Math.max(i - 1, 0));
      } else if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        onEnter(activeIndex);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        if (onEscape) {
          onEscape();
        }
      }
    };

    window.addEventListener('keydown', handler, { capture: true });
    return () =>
      window.removeEventListener('keydown', handler, { capture: true });
  }, [itemCount, activeIndex, onEnter, onEscape]);

  return { activeIndex, setActiveIndex: setInternalActiveIndex };
}
