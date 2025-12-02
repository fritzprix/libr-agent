import React, { useState, useMemo, useEffect } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import {
  Wrench,
  ChevronDown,
  CheckCircle,
  XCircle,
  Loader2,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { MessageRenderer } from '@/components/MessageRenderer';
import {
  hasToolCallError,
  parseToolName,
  formatExecutionTime,
  parseToolArguments,
  formatToolArgumentsSummary,
  hasUIResource,
} from '@/lib/tool-call-utils';
import { useSettings } from '@/hooks/use-settings';
import { useSessionHistory } from '@/context/SessionHistoryContext';
import { ToolCallDetails } from './ToolCallDetails';

interface ToolCallGroupBubbleProps {
  message: Message;
  toolGroup: {
    calls: ToolCall[]; // Simplified - just the tool calls, results looked up dynamically
  };
  isLast?: boolean;
}

interface StatusSummary {
  successCount: number;
  errorCount: number;
  runningCount: number;
}

interface GroupHeaderProps {
  totalCalls: number;
  statusSummary: StatusSummary;
}

interface StatusBadgesProps {
  runningCount: number;
  successCount: number;
  errorCount: number;
}

interface GradientOverlayProps {
  hiddenCount: number;
  hasError: boolean;
  isRunning: boolean;
}

interface ExpandToggleProps {
  isExpanded: boolean;
  totalCalls: number;
  onToggle: () => void;
}

interface ToolCallCompactItemProps {
  toolCall: ToolCall;
  toolResult?: Message;
  isLast?: boolean;
}

interface ToolStatusIconProps {
  hasResult: boolean;
  hasError: boolean;
}

/**
 * Header section showing tool execution count and title
 */
const GroupHeader: React.FC<GroupHeaderProps> = ({
  totalCalls,
  statusSummary,
}) => {
  return (
    <div className="flex items-center justify-between p-3 border-b border-muted/20">
      <div className="flex items-center gap-2">
        <Wrench className="w-4 h-4 text-muted-foreground" />
        <span className="font-medium text-sm">
          Tool Executions ({totalCalls} {totalCalls === 1 ? 'call' : 'calls'})
        </span>
      </div>
      <StatusBadges {...statusSummary} />
    </div>
  );
};

/**
 * Status badges showing running/success/error counts
 */
const StatusBadges: React.FC<StatusBadgesProps> = ({
  runningCount,
  successCount,
  errorCount,
}) => {
  return (
    <div className="flex items-center gap-2">
      {runningCount > 0 && (
        <Badge
          variant="outline"
          className="gap-1 border-primary text-primary bg-primary/10"
        >
          <Loader2 className="w-3 h-3 animate-spin" />
          {runningCount}
        </Badge>
      )}
      {successCount > 0 && (
        <Badge
          variant="outline"
          className="gap-1 border-success text-success bg-success/10"
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
  );
};

/**
 * Gradient overlay showing hidden items count
 */
const GradientOverlay: React.FC<GradientOverlayProps> = ({
  hiddenCount,
  hasError,
  isRunning,
}) => {
  if (hiddenCount === 0) return null;

  return (
    <div className="relative border-b border-dashed border-muted-foreground/20">
      {/* Gradient overlay that matches container background and fades to transparent */}
      <div
        className={cn(
          'h-10 pointer-events-none',
          // Gradient from container's background color (opaque) to transparent
          // This creates a natural fade effect that blends with the container
          isRunning && 'bg-gradient-to-b from-primary/10 to-transparent',
          !isRunning &&
            hasError &&
            'bg-gradient-to-b from-destructive/10 to-transparent',
          !isRunning &&
            !hasError &&
            'bg-gradient-to-b from-success/10 to-transparent',
        )}
      />
      <div className="absolute inset-0 flex items-center justify-center">
        <span className="text-xs text-muted-foreground font-medium">
          {hiddenCount} older {hiddenCount === 1 ? 'call' : 'calls'} hidden
        </span>
      </div>
    </div>
  );
};

/**
 * Expand/collapse toggle button
 */
const ExpandToggle: React.FC<ExpandToggleProps> = ({
  isExpanded,
  totalCalls,
  onToggle,
}) => {
  return (
    <div
      className="flex items-center justify-center p-2 border-t border-muted cursor-pointer hover:bg-muted/50 transition-colors"
      onClick={onToggle}
    >
      <span className="text-xs text-muted-foreground font-medium">
        {isExpanded ? 'Show Less' : `Show All (${totalCalls} calls)`}
      </span>
      <ChevronDown
        className={cn(
          'w-3 h-3 ml-1 transition-transform text-muted-foreground',
          isExpanded && 'rotate-180',
        )}
      />
    </div>
  );
};

/**
 * Status icon showing loading/error/success state
 */
const ToolStatusIcon: React.FC<ToolStatusIconProps> = ({
  hasResult,
  hasError,
}) => {
  if (!hasResult) {
    return (
      <Loader2 className="w-3.5 h-3.5 text-primary animate-spin flex-shrink-0" />
    );
  }

  if (hasError) {
    return <XCircle className="w-3.5 h-3.5 text-destructive flex-shrink-0" />;
  }

  return <CheckCircle className="w-3.5 h-3.5 text-success flex-shrink-0" />;
};

/**
 * Compact tool call item - no individual border, tight spacing
 */
const ToolCallCompactItem: React.FC<ToolCallCompactItemProps> = ({
  toolCall,
  toolResult,
  isLast = false,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  // Parse tool name (remove server prefix)
  const toolName = parseToolName(toolCall.function.name);

  // Parse arguments for summary
  const params = parseToolArguments(toolCall.function.arguments);
  const paramSummary = formatToolArgumentsSummary(params);

  // Check for error using utility function
  const hasError = hasToolCallError(toolResult);
  const hasResource = hasUIResource(toolResult);

  // Get execution time
  const executionTime = toolResult?.metadata?.executionTime;

  // Auto-expand on error or if it's the last message with a UI resource
  useEffect(() => {
    if (hasError && !isExpanded) {
      setIsExpanded(true);
    } else if (hasResource) {
      setIsExpanded(isLast);
    }
  }, [hasError, hasResource, isLast, isExpanded]);

  return (
    <div
      className={cn(
        'rounded px-3 py-2 text-sm transition-colors cursor-pointer',
        hasError
          ? 'bg-destructive/10 hover:bg-destructive/20'
          : 'bg-background hover:bg-muted/50',
      )}
      onClick={() => setIsExpanded(!isExpanded)}
    >
      {/* Collapsed header line */}
      <div className="flex items-center gap-2">
        <ToolStatusIcon hasResult={!!toolResult} hasError={hasError} />

        {/* Tool name */}
        <span className="flex-shrink-0 font-medium">{toolName}</span>

        {/* Params Summary */}
        {paramSummary && (
          <span className="flex-1 text-xs text-muted-foreground truncate font-mono opacity-70 min-w-0">
            {paramSummary}
          </span>
        )}
        {!paramSummary && <span className="flex-1" />}

        {/* Execution time */}
        {executionTime !== undefined && (
          <span className="text-xs text-muted-foreground flex-shrink-0">
            {formatExecutionTime(executionTime)}
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
      {isExpanded && (
        <div className="mt-3 pt-3 border-t border-muted/50">
          <ToolCallDetails
            toolCall={toolCall}
            toolResult={toolResult}
            hasError={hasError}
            isLoading={!toolResult}
          />
        </div>
      )}
    </div>
  );
};

/**
 * Groups multiple tool calls into a single collapsible bubble.
 * Shows bottom N (configurable) by default with gradient overlay for hidden items.
 * Supports multipart messages (text + tool calls).
 */
export const ToolCallGroupBubble: React.FC<ToolCallGroupBubbleProps> = ({
  message,
  toolGroup,
  isLast = false,
}) => {
  const [isExpanded, setIsExpanded] = useState(false);
  const {
    value: { toolCallGroupVisibleCount },
  } = useSettings();
  const { messages } = useSessionHistory();

  // Check if this is a multipart message (has text content)
  const hasTextContent =
    message.content &&
    message.content.length > 0 &&
    message.content.some(
      (c) => c.type === 'text' && c.text && c.text.trim().length > 0,
    );

  // Calculate status summary by looking up tool results dynamically
  const statusSummary: StatusSummary = useMemo(() => {
    let success = 0;
    let error = 0;
    let running = 0;

    toolGroup.calls.forEach((toolCall) => {
      const toolResult = messages.find(
        (m) => m.role === 'tool' && m.tool_call_id === toolCall.id,
      );

      if (!toolResult) {
        running++;
      } else {
        if (hasToolCallError(toolResult)) {
          error++;
        } else {
          success++;
        }
      }
    });

    return { successCount: success, errorCount: error, runningCount: running };
  }, [toolGroup.calls, messages]);

  // Determine visible items
  const visibleCalls = isExpanded
    ? toolGroup.calls
    : toolGroup.calls.slice(-toolCallGroupVisibleCount);

  const hiddenCount = Math.max(
    0,
    toolGroup.calls.length - toolCallGroupVisibleCount,
  );

  // Container styling - using shadcn semantic colors
  const hasAnyError = statusSummary.errorCount > 0;
  const isAnyRunning = statusSummary.runningCount > 0;

  const containerClass = cn(
    'rounded-lg border transition-all mb-2 hover:bg-accent/50 w-full',
    isAnyRunning && 'border-l-4 border-primary bg-primary/10',
    !isAnyRunning &&
      hasAnyError &&
      'border-l-4 border-destructive bg-destructive/10',
    !isAnyRunning && !hasAnyError && 'border-l-4 border-success bg-success/10',
  );

  return (
    <div className={containerClass}>
      <GroupHeader
        totalCalls={toolGroup.calls.length}
        statusSummary={statusSummary}
      />

      {/* Text Content - for multipart messages */}
      {hasTextContent && (
        <div className="px-4 py-3 border-b border-muted/20">
          <MessageRenderer content={message.content || []} />
        </div>
      )}

      {!isExpanded && (
        <GradientOverlay
          hiddenCount={hiddenCount}
          hasError={hasAnyError}
          isRunning={isAnyRunning}
        />
      )}

      {/* Tool Call List - Compact items without individual borders */}
      <div className="px-2 py-2 space-y-0.5">
        {visibleCalls.map((toolCall, index) => {
          const toolResult = messages.find(
            (m) => m.role === 'tool' && m.tool_call_id === toolCall.id,
          );

          const isLastItem = isLast && index === visibleCalls.length - 1;

          return (
            <ToolCallCompactItem
              key={toolCall.id}
              toolCall={toolCall}
              toolResult={toolResult}
              isLast={isLastItem}
            />
          );
        })}
      </div>

      {toolGroup.calls.length > toolCallGroupVisibleCount && (
        <ExpandToggle
          isExpanded={isExpanded}
          totalCalls={toolGroup.calls.length}
          onToggle={() => setIsExpanded(!isExpanded)}
        />
      )}
    </div>
  );
};

export default ToolCallGroupBubble;
