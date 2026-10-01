import { useEffect, useState } from 'react';
import { useSettings } from '@/hooks/use-settings';
import {
  loadSpotlightState,
  pickWaitTip,
  recordWaitTipShown,
} from './spotlight-state';
import { WAIT_TIP_ROTATE_MS, type Spotlight } from './types';

/**
 * Rotating feature tip while the agent workflow is busy.
 * Shown in the composer strip (above chat input) — not inside AnalysisLoader.
 */
export function useBusyComposerTip(active: boolean): Spotlight | null {
  const { value: settings } = useSettings();
  const tipsEnabled = settings.display?.showFeatureTips !== false;
  const [tip, setTip] = useState<Spotlight | null>(null);

  useEffect(() => {
    if (!tipsEnabled || !active) {
      setTip(null);
      return;
    }

    const advance = () => {
      const state = loadSpotlightState();
      const next = pickWaitTip(state);
      if (!next) {
        setTip(null);
        return;
      }
      recordWaitTipShown(state);
      setTip(next);
    };

    advance();
    const timer = setInterval(advance, WAIT_TIP_ROTATE_MS);
    return () => clearInterval(timer);
  }, [active, tipsEnabled]);

  return tip;
}
