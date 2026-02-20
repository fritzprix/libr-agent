import React, { useState, useEffect, memo } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { ChevronDown, CheckCircle, XCircle, Loader2 } from 'lucide-react';
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

export interface ToolCallCompactItemProps {
  toolCall: ToolCall;
  toolResult?: Message;
  isLast?: boolean;
}

export interface ToolStatusIconProps {
  hasResult: boolean;
  hasError: boolean;
}

/**
 * Status icon showing loading/error/success state
 */
export const ToolStatusIcon: React.FC<ToolStatusIconProps> = ({
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
const ToolCallCompactItemImpl: React.FC<ToolCallCompactItemProps> = ({
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

// Custom comparison for React.memo
function arePropsEqual(
  prev: ToolCallCompactItemProps,
  next: ToolCallCompactItemProps,
) {
  // Check primitive props
  if (prev.isLast !== next.isLast) return false;

  // Check toolResult reference equality (assuming stability from useMessageGrouping)
  if (prev.toolResult !== next.toolResult) return false;

  // Check toolCall content deeply (id, function name, arguments)
  if (prev.toolCall.id !== next.toolCall.id) return false;
  if (prev.toolCall.function.name !== next.toolCall.function.name) return false;
  if (prev.toolCall.function.arguments !== next.toolCall.function.arguments)
    return false;

  return true;
}

export const ToolCallCompactItem = memo(ToolCallCompactItemImpl, arePropsEqual);
