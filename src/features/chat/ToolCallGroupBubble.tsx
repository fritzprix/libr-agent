import React, { useState, useMemo } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import {
  Wrench,
  ChevronDown,
  CheckCircle,
  XCircle,
  Loader2,
  AlertCircle,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { MessageRenderer } from '@/components/MessageRenderer';

interface ToolCallGroupBubbleProps {
  message: Message;
  toolGroup: {
    calls: Array<{
      toolCall: ToolCall;
      toolResult?: Message;
    }>;
  };
}

const VISIBLE_COUNT = 4;

/**
 * Groups multiple tool calls into a single collapsible bubble.
 * Shows bottom 4 by default with gradient overlay for hidden items.
 * Supports multipart messages (text + tool calls).
 */
export const ToolCallGroupBubble: React.FC<ToolCallGroupBubbleProps> = ({
  message,
  toolGroup,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  // Check if this is a multipart message (has text content)
  const hasTextContent =
    message.content &&
    message.content.length > 0 &&
    message.content.some(
      (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
    );

  // Calculate status summary
  const { successCount, errorCount, runningCount } = useMemo(() => {
    let success = 0;
    let error = 0;
    let running = 0;

    toolGroup.calls.forEach(({ toolResult }) => {
      if (!toolResult) {
        running++;
      } else {
        const hasError = toolResult.content?.some(
          (c) =>
            c.type === 'text' &&
            (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
        );
        if (hasError) {
          error++;
        } else {
          success++;
        }
      }
    });

    return { successCount: success, errorCount: error, runningCount: running };
  }, [toolGroup.calls]);

  // Determine visible items
  const visibleCalls = isExpanded
    ? toolGroup.calls
    : toolGroup.calls.slice(-VISIBLE_COUNT);

  const hiddenCount = Math.max(0, toolGroup.calls.length - VISIBLE_COUNT);

  // Container styling - match ToolCallResultBubble's color coding
  const hasAnyError = errorCount > 0;
  const isAnyRunning = runningCount > 0;

  const containerClass = cn(
    'rounded-lg border transition-all mb-2 hover:bg-black/5 dark:hover:bg-white/5',
    isAnyRunning &&
      'border-l-4 border-blue-500 bg-blue-50/30 dark:bg-blue-950/30',
    !isAnyRunning &&
      hasAnyError &&
      'border-l-4 border-red-500 bg-red-50/50 dark:bg-red-950/30',
    !isAnyRunning &&
      !hasAnyError &&
      'border-l-4 border-green-500 bg-green-50/30 dark:bg-green-950/30',
  );

  return (
    <div className={containerClass}>
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-muted/20">
        <div className="flex items-center gap-2">
          <Wrench className="w-4 h-4 text-muted-foreground" />
          <span className="font-medium text-sm">
            Tool Executions ({toolGroup.calls.length}{' '}
            {toolGroup.calls.length === 1 ? 'call' : 'calls'})
          </span>
        </div>
        <div className="flex items-center gap-2">
          {runningCount > 0 && (
            <Badge
              variant="outline"
              className="gap-1 border-blue-500 text-blue-700 bg-blue-50 dark:bg-blue-950 dark:text-blue-300"
            >
              <Loader2 className="w-3 h-3 animate-spin" />
              {runningCount}
            </Badge>
          )}
          {successCount > 0 && (
            <Badge
              variant="outline"
              className="gap-1 border-green-500 text-green-700 bg-green-50 dark:bg-green-950 dark:text-green-300"
            >
              <CheckCircle className="w-3 h-3" />
              {successCount}
            </Badge>
          )}
          {errorCount > 0 && (
            <Badge variant="destructive" className="gap-1">
              <XCircle className="w-3 h-3" />
              {errorCount}
            </Badge>
          )}
        </div>
      </div>

      {/* Text Content - for multipart messages */}
      {hasTextContent && (
        <div className="px-4 py-3 border-b border-muted/20">
          <MessageRenderer content={message.content || []} />
        </div>
      )}

      {/* Gradient Overlay - move to bottom and match container color */}
      {!isExpanded && hiddenCount > 0 && (
        <div className="relative">
          <div
            className={cn(
              'h-10 bg-gradient-to-b border-b border-dashed border-muted-foreground/20',
              isAnyRunning &&
                'from-blue-50/80 via-blue-50/40 dark:from-blue-950/50 dark:via-blue-950/20',
              !isAnyRunning &&
                hasAnyError &&
                'from-red-50/80 via-red-50/40 dark:from-red-950/50 dark:via-red-950/20',
              !isAnyRunning &&
                !hasAnyError &&
                'from-green-50/80 via-green-50/40 dark:from-green-950/50 dark:via-green-950/20',
            )}
          >
            <div className="absolute inset-0 flex items-center justify-center">
              <span className="text-xs text-muted-foreground font-medium">
                {hiddenCount} older {hiddenCount === 1 ? 'call' : 'calls'} hidden
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Tool Call List - Compact items without individual borders */}
      <div className="px-2 py-2 space-y-0.5">
        {visibleCalls.map(({ toolCall, toolResult }) => (
          <ToolCallCompactItem
            key={toolCall.id}
            toolCall={toolCall}
            toolResult={toolResult}
          />
        ))}
      </div>

      {/* Expand/Collapse Toggle */}
      {toolGroup.calls.length > VISIBLE_COUNT && (
        <div
          className="flex items-center justify-center p-2 border-t border-muted cursor-pointer hover:bg-muted/50 transition-colors"
          onClick={() => setIsExpanded(!isExpanded)}
        >
          <span className="text-xs text-muted-foreground font-medium">
            {isExpanded
              ? 'Show Less'
              : `Show All (${toolGroup.calls.length} calls)`}
          </span>
          <ChevronDown
            className={cn(
              'w-3 h-3 ml-1 transition-transform text-muted-foreground',
              isExpanded && 'rotate-180',
            )}
          />
        </div>
      )}
    </div>
  );
};

/**
 * Compact tool call item - no individual border, tight spacing
 */
interface ToolCallCompactItemProps {
  toolCall: ToolCall;
  toolResult?: Message;
}

const ToolCallCompactItem: React.FC<ToolCallCompactItemProps> = ({
  toolCall,
  toolResult,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  // Parse tool name (remove server prefix)
  const toolName =
    toolCall.function.name.split('__').pop() || toolCall.function.name;

  // Check for error
  const hasError = toolResult?.content?.some(
    (c) =>
      c.type === 'text' &&
      (c.text?.startsWith('❌') || c.text?.startsWith('Error:')),
  );

  // Get execution time
  const executionTime = toolResult?.metadata?.executionTime;

  // Format execution time
  const formatTime = (ms: number) =>
    ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`;

  // Auto-expand on error
  React.useEffect(() => {
    if (hasError && !isExpanded) {
      setIsExpanded(true);
    }
  }, [hasError, isExpanded]);

  return (
    <div
      className={cn(
        'rounded px-3 py-2 text-sm transition-colors cursor-pointer',
        hasError
          ? 'bg-red-50 dark:bg-red-950/30 hover:bg-red-100 dark:hover:bg-red-950/50'
          : 'bg-background hover:bg-muted/50',
      )}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      {/* Collapsed header line */}
      <div className="flex items-center gap-2">
        {/* Status icon */}
        {!toolResult ? (
          <Loader2 className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400 animate-spin flex-shrink-0" />
        ) : hasError ? (
          <XCircle className="w-3.5 h-3.5 text-red-600 dark:text-red-400 flex-shrink-0" />
        ) : (
          <CheckCircle className="w-3.5 h-3.5 text-green-600 dark:text-green-400 flex-shrink-0" />
        )}

        {/* Tool name */}
        <span className="flex-1 truncate font-medium">{toolName}</span>

        {/* Execution time */}
        {executionTime !== undefined && (
          <span className="text-xs text-muted-foreground">
            {formatTime(executionTime)}
          </span>
        )}

        {/* Expand indicator */}
        <ChevronDown
          className={cn(
            'w-3.5 h-3.5 transition-transform flex-shrink-0 text-muted-foreground',
            isExpanded && 'rotate-180',
          )}
        />
      </div>

      {/* Expanded details */}
      {isExpanded && toolResult && (
        <div className="mt-3 pt-3 border-t border-muted/50">
          {hasError ? (
            <div className="flex items-start gap-2">
              <AlertCircle className="w-4 h-4 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" />
              <div className="flex-1 min-w-0">
                <div className="text-xs font-medium text-red-900 dark:text-red-100 mb-1">
                  Error Details
                </div>
                <MessageRenderer
                  content={toolResult.content}
                  className="text-sm text-red-900 dark:text-red-100"
                />
              </div>
            </div>
          ) : (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-1">
                Result
              </div>
              <MessageRenderer
                content={toolResult.content}
                className="text-sm"
                expandResources={true}
              />
            </div>
          )}
        </div>
      )}

      {/* Loading state */}
      {isExpanded && !toolResult && (
        <div className="mt-3 pt-3 border-t border-muted/50">
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>Executing tool...</span>
          </div>
        </div>
      )}
    </div>
  );
};

export default ToolCallGroupBubble;
