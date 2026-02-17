import React, { useState, useMemo, useEffect, memo } from 'react';
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
import {
  hasToolCallError,
  parseToolName,
  formatExecutionTime,
  parseToolArguments,
  formatToolArgumentsSummary,
  hasUIResource,
} from '@/lib/tool-call-utils';
import { AgentToolCallDetails } from './AgentToolCallDetails';

interface AgentToolCallGroupProps {
  message: Message;
  toolGroup: {
    calls: ToolCall[];
  };
  toolResults: (Message | undefined)[];
  isLast?: boolean;
  visibleCount?: number; // Default visible count if settings not available
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
      <div
        className={cn(
          'h-10 pointer-events-none',
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
        <div className="mt-3 pt-3 border-t border-muted/50 min-w-0">
          <AgentToolCallDetails
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
 * Groups multiple tool calls into a single collapsible bubble for Agent V2.
 * Optimized with React.memo to prevent unnecessary re-renders during streaming or history updates.
 */
const AgentToolCallGroupImpl: React.FC<AgentToolCallGroupProps> = ({
  toolGroup,
  toolResults,
  isLast = false,
  visibleCount = 3, // Default to 3 if not provided
}) => {
  const [isExpanded, setIsExpanded] = useState(false);

  // Calculate status summary using passed toolResults
  const statusSummary: StatusSummary = useMemo(() => {
    let success = 0;
    let error = 0;
    let running = 0;

    // toolResults corresponds 1:1 with toolGroup.calls
    toolResults.forEach((toolResult) => {
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
  }, [toolResults]);

  // Determine visible items
  const visibleCalls = isExpanded
    ? toolGroup.calls
    : toolGroup.calls.slice(-visibleCount);

  const hiddenCount = Math.max(0, toolGroup.calls.length - visibleCount);

  // Container styling - using shadcn semantic colors
  const hasAnyError = statusSummary.errorCount > 0;
  const isAnyRunning = statusSummary.runningCount > 0;

  const containerClass = cn(
    'rounded-lg border transition-all mb-2 hover:bg-accent/50 w-full max-w-full',
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
          // Find the corresponding result.
          // Since visibleCalls is a slice, we need the original index for toolResults lookup.
          // However, we can also just find it, but toolResults is 1:1.
          // A safer way if slicing is involved is to find index in original calls.
          const originalIndex = toolGroup.calls.indexOf(toolCall);
          const toolResult = toolResults[originalIndex];

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

      {toolGroup.calls.length > visibleCount && (
        <ExpandToggle
          isExpanded={isExpanded}
          totalCalls={toolGroup.calls.length}
          onToggle={() => setIsExpanded(!isExpanded)}
        />
      )}
    </div>
  );
};

// Custom comparison for React.memo
export function arePropsEqual(
  prev: AgentToolCallGroupProps,
  next: AgentToolCallGroupProps,
) {
  // Check message identity
  if (prev.message.id !== next.message.id) return false;

  // Check primitive props
  if (prev.isLast !== next.isLast) return false;
  if (prev.visibleCount !== next.visibleCount) return false;

  // Check calls array content (length and content of items)
  // AgentMessageRenderer recreates toolCall objects on every render, so we must check content deeply.
  // Optimization: If the array reference is identical (thanks to ToolGroupBlock), we skip deep checks.
  if (prev.toolGroup.calls !== next.toolGroup.calls) {
    if (prev.toolGroup.calls.length !== next.toolGroup.calls.length)
      return false;
    for (let i = 0; i < prev.toolGroup.calls.length; i++) {
      const prevCall = prev.toolGroup.calls[i];
      const nextCall = next.toolGroup.calls[i];

      if (prevCall.id !== nextCall.id) return false;
      if (prevCall.type !== nextCall.type) return false;
      if (prevCall.function.name !== nextCall.function.name) return false;
      if (prevCall.function.arguments !== nextCall.function.arguments)
        return false;
    }
  }

  // Check toolResults array content (shallow comparison of Message objects)
  // We assume Message objects are stable (from useAgentChatState)
  if (prev.toolResults.length !== next.toolResults.length) return false;
  for (let i = 0; i < prev.toolResults.length; i++) {
    if (prev.toolResults[i] !== next.toolResults[i]) return false;
  }

  return true;
}

export const AgentToolCallGroup = memo(AgentToolCallGroupImpl, arePropsEqual);
export default AgentToolCallGroup;
