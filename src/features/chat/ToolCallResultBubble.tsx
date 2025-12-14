import React, { useState, useEffect } from 'react';
import type { ToolCall, Message } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import {
  Wrench,
  CheckCircle,
  XCircle,
  Loader2,
  ChevronDown,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import {
  hasToolCallError,
  hasUIResource,
  parseToolName,
  parseToolArguments,
  formatExecutionTime,
  formatToolArgumentsSummary,
} from '@/lib/tool-call-utils';
import { ToolCallDetails } from './ToolCallDetails';

interface ToolCallResultBubbleProps {
  toolCall: ToolCall;
  toolResult?: Message;
  isLoading?: boolean;
}

/**
 * Integrated bubble component that displays a tool call and its result together.
 * Replaces the previous separate ToolCallBubble and ToolOutputBubble components.
 *
 * Features:
 * - Collapsed state: Shows tool name, param count, status badge, and execution time
 * - Expanded state: Shows parameters (JSON) and result/error details
 * - Auto-expands on: Error OR UI Resources present
 * - Color coding: Green (success), Red (error), Blue (running)
 */
export const ToolCallResultBubble: React.FC<ToolCallResultBubbleProps> = ({
  toolCall,
  toolResult,
  isLoading = false,
}) => {
  // Use shared utility functions
  const hasError = hasToolCallError(toolResult);
  const hasResource = hasUIResource(toolResult);
  const executionTime = toolResult?.metadata?.executionTime;

  // Auto-expand on error or UI resources
  const [isExpanded, setIsExpanded] = useState(hasError || hasResource);

  useEffect(() => {
    if ((hasError || hasResource) && !isExpanded) {
      setIsExpanded(true);
    }
  }, [hasError, hasResource, isExpanded]);

  // Use shared utility functions for parsing
  const toolName = parseToolName(toolCall.function.name);
  const params = parseToolArguments(toolCall.function.arguments);
  const paramSummary = formatToolArgumentsSummary(params);

  // Status badge component
  const getStatusBadge = () => {
    if (isLoading) {
      return (
        <Badge
          variant="outline"
          className="gap-1 border-blue-500 text-blue-700 bg-blue-50 dark:bg-blue-950 dark:text-blue-300 dark:border-blue-700"
        >
          <Loader2 className="w-3 h-3 animate-spin" />
          Running
        </Badge>
      );
    }

    if (hasError) {
      return (
        <Badge variant="destructive" className="gap-1">
          <XCircle className="w-3 h-3" />
          Error
        </Badge>
      );
    }

    return (
      <Badge
        variant="outline"
        className="gap-1 border-green-500 text-green-700 bg-green-50 dark:bg-green-950 dark:text-green-300 dark:border-green-700"
      >
        <CheckCircle className="w-3 h-3" />
        Success
      </Badge>
    );
  };

  // Container styling based on status
  const containerClass = cn(
    'rounded-lg border transition-all mb-2 w-full max-w-full',
    isLoading && 'border-l-4 border-blue-500 bg-blue-50/30 dark:bg-blue-950/30',
    !isLoading &&
      hasError &&
      'border-l-4 border-red-500 bg-red-50/50 dark:bg-red-950/30',
    !isLoading &&
      !hasError &&
      'border-l-4 border-green-500 bg-green-50/30 dark:bg-green-950/30',
  );

  return (
    <div className={containerClass}>
      {/* Header: Collapsed state info */}
      <div
        className="flex items-center justify-between gap-2 p-3 cursor-pointer hover:bg-black/5 dark:hover:bg-white/5 transition-colors"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        {/* Left: Tool information */}
        <div className="flex items-center gap-2 flex-1 min-w-0 mr-4">
          <Wrench className="w-4 h-4 flex-shrink-0 text-muted-foreground" />
          <span className="font-medium flex-shrink-0">{toolName}</span>
          {paramSummary && (
            <span className="text-xs text-muted-foreground truncate font-mono opacity-70 min-w-0">
              {paramSummary}
            </span>
          )}
        </div>

        {/* Right: Status + Time + Expand icon */}
        <div className="flex items-center gap-2 flex-shrink-0">
          {getStatusBadge()}
          {executionTime !== undefined && !isLoading && (
            <span className="text-xs text-muted-foreground">
              ({formatExecutionTime(executionTime)})
            </span>
          )}
          <ChevronDown
            className={cn(
              'w-4 h-4 transition-transform text-muted-foreground',
              isExpanded && 'rotate-180',
            )}
          />
        </div>
      </div>

      {/* Expanded state: Details */}
      {isExpanded && (
        <div className="border-t px-3 pb-3 pt-3 min-w-0">
          <ToolCallDetails
            toolCall={toolCall}
            toolResult={toolResult}
            hasError={hasError}
            isLoading={isLoading}
          />
        </div>
      )}
    </div>
  );
};

export default ToolCallResultBubble;
