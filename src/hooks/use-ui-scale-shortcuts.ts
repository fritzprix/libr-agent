import { useEffect, useRef } from 'react';
import {
  RESET_UI_SCALE,
  resolveUiScaleShortcut,
  stepUiScale,
} from '@/lib/ui-scale';

/** Ctrl/Cmd + / - / 0 step the UI scale app-wide, like browser zoom. */
export function useUiScaleShortcuts(
  scale: number,
  onChange: (scale: number) => void,
) {
  const scaleRef = useRef(scale);
  scaleRef.current = scale;
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      const action = resolveUiScaleShortcut(event);
      if (!action) {
        return;
      }
      event.preventDefault();
      onChangeRef.current(
        action === 'reset'
          ? RESET_UI_SCALE
          : stepUiScale(scaleRef.current, action === 'up' ? 1 : -1),
      );
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);
}
