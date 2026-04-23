import { act, fireEvent, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useListNavigation } from '../use-list-navigation';

describe('useListNavigation', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('clamps the active index when the list shrinks and enters using the clamped item', () => {
    const onEnter = vi.fn();
    const { result, rerender } = renderHook(
      ({ itemCount }) =>
        useListNavigation({
          itemCount,
          onEnter,
        }),
      {
        initialProps: { itemCount: 3 },
      },
    );

    act(() => {
      result.current.setActiveIndex(2);
    });

    expect(result.current.activeIndex).toBe(2);

    rerender({ itemCount: 1 });

    expect(result.current.activeIndex).toBe(0);

    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'Enter' });

    expect(result.current.activeIndex).toBe(0);
    expect(onEnter).toHaveBeenCalledWith(0);
  });

  it('only prevents Escape when the hook handles it', () => {
    const onEscape = vi.fn();
    const handled = renderHook(() =>
      useListNavigation({
        itemCount: 2,
        onEnter: vi.fn(),
        onEscape,
      }),
    );

    const handledEscape = new KeyboardEvent('keydown', {
      key: 'Escape',
      cancelable: true,
    });

    window.dispatchEvent(handledEscape);

    expect(handledEscape.defaultPrevented).toBe(true);
    expect(onEscape).toHaveBeenCalledTimes(1);

    handled.unmount();

    renderHook(() =>
      useListNavigation({
        itemCount: 2,
        onEnter: vi.fn(),
      }),
    );

    const unhandledEscape = new KeyboardEvent('keydown', {
      key: 'Escape',
      cancelable: true,
    });

    window.dispatchEvent(unhandledEscape);

    expect(unhandledEscape.defaultPrevented).toBe(false);
  });
});
