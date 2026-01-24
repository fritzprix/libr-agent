import React from 'react';

interface LoadingIndicatorProps {
  /** Optional className for custom styling */
  className?: string;
  /** Size of the dots (text size class) */
  size?: 'sm' | 'md' | 'lg';
}

/**
 * LoadingIndicator - Reusable bouncing dots animation
 * Used across AgentMessageBubble, AgentToolCallGroup, and AgentChatMessages
 * to indicate loading/streaming/processing states.
 */
export const LoadingIndicator: React.FC<LoadingIndicatorProps> = ({
  className = '',
  size = 'md',
}) => {
  const sizeClass = {
    sm: 'text-xs',
    md: 'text-sm',
    lg: 'text-base',
  }[size];

  return (
    <span className={`flex gap-1 ${sizeClass} ${className}`}>
      <span className="animate-bounce" style={{ animationDelay: '0ms' }}>
        ●
      </span>
      <span className="animate-bounce" style={{ animationDelay: '150ms' }}>
        ●
      </span>
      <span className="animate-bounce" style={{ animationDelay: '300ms' }}>
        ●
      </span>
    </span>
  );
};
