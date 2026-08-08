/**
 * Drag handle for the desktop agent side panel (right rail).
 * Dragging left widens the panel; dragging right narrows it.
 */

import { cn } from '@/lib/utils';
import {
  useCallback,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from 'react';
import { useTranslation } from 'react-i18next';

interface PanelResizeHandleProps {
  panelWidth: number;
  minWidth: number;
  maxWidth: number;
  onResize: (width: number) => void;
  onResizeEnd: (width: number) => void;
  onReset: () => void;
  disabled?: boolean;
}

export function PanelResizeHandle({
  panelWidth,
  minWidth,
  maxWidth,
  onResize,
  onResizeEnd,
  onReset,
  disabled = false,
}: PanelResizeHandleProps) {
  const { t } = useTranslation();
  const [dragging, setDragging] = useState(false);
  const dragRef = useRef<{
    pointerId: number;
    startX: number;
    startWidth: number;
  } | null>(null);

  const handlePointerDown = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      if (disabled) {
        return;
      }
      // Some test environments omit `button`; treat missing as primary click.
      if (typeof event.button === 'number' && event.button !== 0) {
        return;
      }

      event.preventDefault();
      dragRef.current = {
        pointerId: event.pointerId,
        startX: event.clientX,
        startWidth: panelWidth,
      };
      setDragging(true);

      try {
        event.currentTarget.setPointerCapture(event.pointerId);
      } catch {
        // jsdom / older browsers may not support pointer capture.
      }
    },
    [disabled, panelWidth],
  );

  const handlePointerMove = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) {
        return;
      }

      // Right rail: move left → wider, move right → narrower.
      const delta = event.clientX - drag.startX;
      const next = Math.min(
        Math.max(drag.startWidth - delta, minWidth),
        maxWidth,
      );
      onResize(next);
    },
    [maxWidth, minWidth, onResize],
  );

  const endDrag = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      const drag = dragRef.current;
      if (!drag || drag.pointerId !== event.pointerId) {
        return;
      }

      dragRef.current = null;
      setDragging(false);

      if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
        try {
          event.currentTarget.releasePointerCapture(event.pointerId);
        } catch {
          // Ignore release failures in test environments.
        }
      }

      const delta = event.clientX - drag.startX;
      const next = Math.min(
        Math.max(drag.startWidth - delta, minWidth),
        maxWidth,
      );
      onResizeEnd(next);
    },
    [maxWidth, minWidth, onResizeEnd],
  );

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-valuenow={panelWidth}
      aria-valuemin={minWidth}
      aria-valuemax={maxWidth}
      aria-label={t('agent.panels.resizeHandleAria', 'Resize agent panels')}
      aria-disabled={disabled || undefined}
      data-testid="panel-resize-handle"
      data-dragging={dragging || undefined}
      tabIndex={disabled ? -1 : 0}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
      onDoubleClick={() => {
        if (!disabled) {
          onReset();
        }
      }}
      onKeyDown={(event) => {
        if (disabled) {
          return;
        }
        const step = event.shiftKey ? 32 : 16;
        if (event.key === 'ArrowLeft') {
          event.preventDefault();
          const next = Math.min(panelWidth + step, maxWidth);
          onResize(next);
          onResizeEnd(next);
        } else if (event.key === 'ArrowRight') {
          event.preventDefault();
          const next = Math.max(panelWidth - step, minWidth);
          onResize(next);
          onResizeEnd(next);
        } else if (event.key === 'Home') {
          event.preventDefault();
          onReset();
        }
      }}
      className={cn(
        'absolute inset-y-0 left-0 z-30 w-1.5 -translate-x-1/2 cursor-col-resize touch-none',
        'bg-transparent hover:bg-primary/20 focus-visible:bg-primary/30',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        dragging && 'bg-primary/30',
        disabled && 'pointer-events-none opacity-0',
      )}
    />
  );
}
