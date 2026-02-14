import React from 'react';

interface LoadingIndicatorProps {
  /** Optional className for custom styling */
  className?: string;
  /** Size of the dots (text size class) */
  size?: 'sm' | 'md' | 'lg';
  /** Accessible label */
  label?: string;
}

/**
 * LoadingIndicator - Reusable bouncing dots animation
 * Used across AgentMessageBubble, AgentToolCallGroup, and AgentChatMessages
 * to indicate loading/streaming/processing states.
 */
export const LoadingIndicator: React.FC<LoadingIndicatorProps> = ({
  className = '',
  size = 'md',
  label = 'Loading',
}) => {
  const sizeClass = {
    sm: 'text-xs',
    md: 'text-sm',
    lg: 'text-base',
  }[size];

  return (
    <span
      role="status"
      aria-label={label}
      className={`flex gap-1 ${sizeClass} ${className}`}
    >
      <span className="sr-only">{label}</span>
      <span
        aria-hidden="true"
        className="animate-bounce"
        style={{ animationDelay: '0ms' }}
      >
        ●
      </span>
      <span
        aria-hidden="true"
        className="animate-bounce"
        style={{ animationDelay: '150ms' }}
      >
        ●
      </span>
      <span
        aria-hidden="true"
        className="animate-bounce"
        style={{ animationDelay: '300ms' }}
      >
        ●
      </span>
    </span>
  );
};
