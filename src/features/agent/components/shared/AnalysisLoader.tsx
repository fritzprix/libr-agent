import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadingIndicator } from './LoadingIndicator';
import {
  DEFAULT_COMBINATORIAL,
  DEFAULT_INITIAL,
  DEFAULT_LATE,
  DEFAULT_WITTY,
  buildEarlyPool,
  buildEndlessPool,
  isStringArray,
  nextPhase,
  parseCombinatorialParts,
  type CombinatorialParts,
  type LoaderPhase,
} from './analysisLoaderMessages';

interface AnalysisLoaderProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

export const AnalysisLoader: React.FC<AnalysisLoaderProps> = ({
  size = 'md',
  className = '',
}) => {
  const { t, i18n } = useTranslation('common');

  const wittyList = useMemo(() => {
    const raw = t('agent.analysisLoader.wittyMessages', {
      returnObjects: true,
      defaultValue: DEFAULT_WITTY,
    });
    return isStringArray(raw) ? raw : DEFAULT_WITTY;
  }, [t]);

  const lateList = useMemo(() => {
    const raw = t('agent.analysisLoader.lateMessages', {
      returnObjects: true,
      defaultValue: DEFAULT_LATE,
    });
    return isStringArray(raw) ? raw : DEFAULT_LATE;
  }, [t]);

  const combinatorialParts = useMemo((): CombinatorialParts => {
    const raw = t('agent.analysisLoader.combinatorial', {
      returnObjects: true,
      defaultValue: DEFAULT_COMBINATORIAL,
    });
    return parseCombinatorialParts(raw) ?? DEFAULT_COMBINATORIAL;
  }, [t]);

  const initialMessage = t('agent.analysisLoader.initial', DEFAULT_INITIAL);

  const [phase, setPhase] = useState<LoaderPhase>('early');
  const [index, setIndex] = useState(0);
  const [queue, setQueue] = useState(() =>
    buildEarlyPool(initialMessage, wittyList, combinatorialParts),
  );

  // Rebuild only when the active language changes (translations are sync on mount).
  const languageRef = useRef(i18n.language);
  useEffect(() => {
    if (languageRef.current === i18n.language) {
      return;
    }
    languageRef.current = i18n.language;
    setPhase('early');
    setIndex(0);
    setQueue(buildEarlyPool(initialMessage, wittyList, combinatorialParts));
  }, [i18n.language, initialMessage, wittyList, combinatorialParts]);

  useEffect(() => {
    // Adaptive pacing: longer first beat on the pinned initial message, then brisk cycle.
    const delay = phase === 'early' && index === 0 ? 1800 : 1400;
    const timer = setTimeout(() => {
      if (index + 1 < queue.length) {
        setIndex(index + 1);
        return;
      }

      const lastMessage = queue[index];
      const upcoming = nextPhase(phase);

      if (upcoming === 'late' && lateList.length > 0) {
        setPhase('late');
        setQueue([...lateList]);
        setIndex(0);
        return;
      }

      // late → endless, empty late list, or endless wrap: reshuffle + fresh combos
      setPhase('endless');
      setQueue(
        buildEndlessPool(wittyList, combinatorialParts, 24, lastMessage),
      );
      setIndex(0);
    }, delay);

    return () => clearTimeout(timer);
  }, [phase, index, queue, lateList, wittyList, combinatorialParts]);

  const currentMessage = queue[index] ?? initialMessage;

  return (
    <div
      className={`inline-flex items-center gap-2.5 text-xs text-muted-foreground font-mono select-none ${className}`}
    >
      <LoadingIndicator size={size} />
      <span
        key={`${phase}-${index}-${currentMessage}`}
        className="animate-in fade-in duration-300 transition-all truncate"
      >
        {currentMessage}
      </span>
    </div>
  );
};
