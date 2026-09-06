import React from 'react';
import { PhosphorDotMatrix } from './PhosphorDotMatrix';

interface LoadingIndicatorProps {
  /** Optional className for custom styling */
  className?: string;
  /** Size of the indicator */
  size?: 'sm' | 'md' | 'lg';
}

/**
 * LoadingIndicator - Canvas-based retro-futuristic phosphor dot matrix indicator.
 * Used across AgentMessageBubble, ThinkingBubble, and AnalysisLoader.
 */
export const LoadingIndicator: React.FC<LoadingIndicatorProps> = ({
  className = '',
  size = 'md',
}) => {
  return <PhosphorDotMatrix size={size} className={className} />;
};
