import React, { useEffect, useState } from 'react';
import { LoadingIndicator } from './LoadingIndicator';

interface AnalysisLoaderProps {
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const MESSAGES = [
  'Analyzing Request...',
  'Thinking...',
  'Processing...',
  'Using Tools...',
];

export const AnalysisLoader: React.FC<AnalysisLoaderProps> = ({
  size = 'md',
  className = '',
}) => {
  const [index, setIndex] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setIndex((prev) => (prev + 1) % MESSAGES.length);
    }, 2000); // Rotate every 2 seconds

    return () => clearInterval(interval);
  }, []);

  return (
    <div
      className={`flex items-center gap-2 text-muted-foreground ${className}`}
    >
      <LoadingIndicator size={size} />
      <span className="animate-pulse">{MESSAGES[index]}</span>
    </div>
  );
};
