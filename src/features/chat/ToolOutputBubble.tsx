
import React from 'react';

interface ToolOutputBubbleProps {
  content: string;
}

const ToolOutputBubble: React.FC<ToolOutputBubbleProps> = ({ content }) => {
  return (
    <div className="mt-4 bg-muted/70 rounded-lg border border-muted/40 overflow-hidden">
      <div className="px-3 py-2 bg-accent border-b border-accent/20 flex items-center gap-2">
        <div className="flex gap-1">
          <div className="w-2 h-2 bg-destructive rounded-full"></div>
          <div className="w-2 h-2 bg-warning rounded-full"></div>
          <div className="w-2 h-2 bg-success rounded-full"></div>
        </div>
        <span className="text-accent-foreground font-mono text-sm">
          Tool Output
        </span>
      </div>
      <div className="p-4 max-h-32 overflow-y-auto custom-scrollbar">
        <pre className="text-xs text-muted-foreground font-mono whitespace-pre-wrap break-words">
          {content}
        </pre>
      </div>
    </div>
  );
};

export default ToolOutputBubble;
