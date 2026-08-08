import { act, fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { PanelResizeHandle } from '../PanelResizeHandle';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (_key: string, defaultValue?: string) => defaultValue ?? _key,
  }),
}));

function dispatchPointer(
  target: Element,
  type: 'pointerdown' | 'pointermove' | 'pointerup',
  clientX: number,
  pointerId = 1,
  button = 0,
) {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    clientX: { value: clientX },
    pointerId: { value: pointerId },
    button: { value: button },
  });
  target.dispatchEvent(event);
}

describe('PanelResizeHandle', () => {
  it('widens the right rail when dragging left', () => {
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    render(
      <PanelResizeHandle
        panelWidth={320}
        minWidth={320}
        maxWidth={640}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
        onReset={vi.fn()}
      />,
    );

    const handle = screen.getByTestId('panel-resize-handle');
    act(() => {
      dispatchPointer(handle, 'pointerdown', 1000);
      dispatchPointer(handle, 'pointermove', 900);
      dispatchPointer(handle, 'pointerup', 900);
    });

    expect(onResize).toHaveBeenCalledWith(420);
    expect(onResizeEnd).toHaveBeenCalledWith(420);
  });

  it('resets on double-click', () => {
    const onReset = vi.fn();
    render(
      <PanelResizeHandle
        panelWidth={480}
        minWidth={320}
        maxWidth={640}
        onResize={vi.fn()}
        onResizeEnd={vi.fn()}
        onReset={onReset}
      />,
    );

    fireEvent.doubleClick(screen.getByTestId('panel-resize-handle'));
    expect(onReset).toHaveBeenCalledTimes(1);
  });

  it('supports keyboard resize for a11y', () => {
    const onResize = vi.fn();
    const onResizeEnd = vi.fn();

    render(
      <PanelResizeHandle
        panelWidth={320}
        minWidth={320}
        maxWidth={640}
        onResize={onResize}
        onResizeEnd={onResizeEnd}
        onReset={vi.fn()}
      />,
    );

    fireEvent.keyDown(screen.getByTestId('panel-resize-handle'), {
      key: 'ArrowLeft',
    });

    expect(onResize).toHaveBeenCalledWith(336);
    expect(onResizeEnd).toHaveBeenCalledWith(336);
  });
});
