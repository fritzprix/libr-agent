import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadingIndicator } from './LoadingIndicator';

interface AnalysisLoaderProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const DEFAULT_INITIAL = 'Preparing response...';
const DEFAULT_WITTY = [
  'Sipping digital coffee...',
  'Neurons are stretching...',
  'Picking the most intellectual emojis...',
  'Navigating through 0s and 1s...',
  'Tabs or spaces? Debating the eternal question...',
  'Tuning quantum entanglement...',
  'Hunting for missing semicolons...',
  'Dusting off virtual bookshelves...',
  'GPU fans spinning at full speed 🌪️',
  'Baking a fresh, crispy response 🥐',
  'Briefly admiring a cat picture 🐾',
  'Searching every corner of cache memory...',
];
const DEFAULT_LATE = [
  'Great answers require proper aging, like fine wine 🍷',
  'No progress bar, so cycling witty text instead...',
  'Waiting 3 more seconds might unleash pure genius...',
  'Almost there, hang tight!',
  'Taking a moment? Time for a quick stretch 🧘',
  'Thanks for your patience, almost ready...',
];

function isStringArray(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.every((item) => typeof item === 'string')
  );
}

function shuffleArray<T>(array: readonly T[]): T[] {
  const result = [...array];
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

export const AnalysisLoader: React.FC<AnalysisLoaderProps> = ({
  size = 'md',
  className = '',
}) => {
  const { t } = useTranslation('common');
  const [step, setStep] = useState(0);

  const shuffledPool = useMemo(() => {
    const raw = t('agent.analysisLoader.wittyMessages', {
      returnObjects: true,
      defaultValue: DEFAULT_WITTY,
    });
    const list = isStringArray(raw) ? raw : DEFAULT_WITTY;
    const initial = t('agent.analysisLoader.initial', DEFAULT_INITIAL);
    return shuffleArray([initial, ...list]);
  }, [t]);

  const lateList = useMemo(() => {
    const raw = t('agent.analysisLoader.lateMessages', {
      returnObjects: true,
      defaultValue: DEFAULT_LATE,
    });
    return isStringArray(raw) ? raw : DEFAULT_LATE;
  }, [t]);

  useEffect(() => {
    // Adaptive cognitive pacing: 1.8s for initial eye-landing, then 1.4s for brisk dopamine cycle
    const delay = step === 0 ? 1800 : 1400;
    const timer = setTimeout(() => {
      setStep((prev) => prev + 1);
    }, delay);

    return () => clearTimeout(timer);
  }, [step]);

  // Determine current message based on progression step:
  // Steps 0..shuffledPool.length - 1: Immediately randomized diverse messages
  // Steps beyond: Progressive late messages (clamped at final reassurance)
  const currentMessage = useMemo(() => {
    if (step < shuffledPool.length) {
      return shuffledPool[step];
    }

    const lateIndex = step - shuffledPool.length;
    if (lateIndex < lateList.length) {
      return lateList[lateIndex];
    }

    // Clamped at the last reassuring message
    return lateList[lateList.length - 1] || shuffledPool[0];
  }, [step, shuffledPool, lateList]);

  return (
    <div
      className={`inline-flex items-center gap-2.5 text-xs text-muted-foreground font-mono select-none ${className}`}
    >
      <LoadingIndicator size={size} />
      <span
        key={currentMessage}
        className="animate-in fade-in duration-300 transition-all truncate"
      >
        {currentMessage}
      </span>
    </div>
  );
};
