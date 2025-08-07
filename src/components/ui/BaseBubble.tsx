import React, { useState } from 'react';
import { ChevronDown, ChevronRight, Copy, Check } from 'lucide-react';

interface BaseBubbleProps {
  title: string;
  icon?: React.ReactNode;
  badge?: React.ReactNode;
  defaultExpanded?: boolean;
  copyData?: string;
  collapsedSummary?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

export const BaseBubble: React.FC<BaseBubbleProps> = ({
  title,
  icon,
  badge,
  defaultExpanded = false,
  copyData,
  collapsedSummary,
  children,
  className = '',
}) => {
  const [copied, setCopied] = useState(false);
  const [isExpanded, setIsExpanded] = useState(defaultExpanded);

  const copyToClipboard = async () => {
    if (!copyData) return;
    
    try {
      await navigator.clipboard.writeText(copyData);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy data: ', err);
    }
  };

  return (
    <div className={`mt-4 bg-background rounded-lg border border-border overflow-hidden shadow-lg ${className}`}>
      <div className="px-4 py-3 bg-muted border-b border-border flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="flex gap-1.5">
            <div className="w-3 h-3 bg-red-500 rounded-full"></div>
            <div className="w-3 h-3 bg-yellow-500 rounded-full"></div>
            <div className="w-3 h-3 bg-green-500 rounded-full"></div>
          </div>
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="flex items-center gap-2 text-muted-foreground hover:text-foreground transition-colors"
          >
            {isExpanded ? (
              <ChevronDown size={16} />
            ) : (
              <ChevronRight size={16} />
            )}
            {icon}
            <span className="font-mono text-sm flex items-center gap-2">
              {title}
              {badge}
            </span>
          </button>
        </div>
        {copyData && (
          <button
            onClick={copyToClipboard}
            className="flex items-center gap-2 px-3 py-1.5 bg-secondary hover:bg-secondary/80 text-secondary-foreground text-xs rounded transition-colors"
          >
            {copied ? <Check size={12} /> : <Copy size={12} />}
            {copied ? 'Copied!' : 'Copy'}
          </button>
        )}
      </div>

      {!isExpanded && collapsedSummary && (
        <div className="px-4 py-3 bg-background/50 text-muted-foreground text-sm">
          {collapsedSummary}
        </div>
      )}

      {isExpanded && (
        <div className="p-4 max-h-96 overflow-auto bg-background">
          {children}
        </div>
      )}
    </div>
  );
};
