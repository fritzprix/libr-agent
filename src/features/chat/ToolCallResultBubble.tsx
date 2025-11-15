import React, { useState, useEffect } from 'react';
import type { ToolCall, Message } from '@/models/chat';
import { Badge } from '@/components/ui/badge';
import {
  Wrench,
  CheckCircle,
  XCircle,
  Loader2,
  ChevronDown,
  AlertCircle,
} from 'lucide-react';
import { cn } from '@/lib/utils';
import { MessageRenderer } from '@/components/MessageRenderer';
import {
  hasToolCallError,
  hasUIResource,
  parseToolName,
  parseToolArguments,
  formatExecutionTime,
} from '@/lib/tool-call-utils';

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
  const paramCount = Object.keys(params).length;

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
    'rounded-lg border transition-all mb-2',
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
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <Wrench className="w-4 h-4 flex-shrink-0 text-muted-foreground" />
          <span className="font-medium truncate">{toolName}</span>
          <span className="text-xs text-muted-foreground">
            • {paramCount} {paramCount === 1 ? 'param' : 'params'}
          </span>
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
        <div className="border-t px-3 pb-3 space-y-3">
          {/* Parameters section */}
          <div className="pt-3">
            <div className="text-xs font-medium text-muted-foreground mb-2">
              Parameters
            </div>
            <div className="bg-muted/50 rounded p-2">
              <pre className="text-xs overflow-x-auto font-mono">
                {JSON.stringify(params, null, 2)}
              </pre>
            </div>
          </div>

          {/* Result or Error section */}
          {toolResult && (
            <div>
              <div className="text-xs font-medium text-muted-foreground mb-2">
                {hasError ? 'Error Details' : 'Result'}
              </div>
              {hasError ? (
                <div className="bg-red-50 dark:bg-red-950/30 border border-red-200 dark:border-red-800 rounded p-3">
                  <div className="flex items-start gap-2">
                    <AlertCircle className="w-4 h-4 text-red-600 dark:text-red-400 flex-shrink-0 mt-0.5" />
                    <div className="flex-1 min-w-0">
                      <MessageRenderer
                        content={toolResult.content}
                        className="text-sm text-red-900 dark:text-red-100"
                      />
                    </div>
                  </div>
                </div>
              ) : (
                <div className="bg-background rounded border p-2">
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
          {isLoading && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span>Executing tool...</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
};

export default ToolCallResultBubble;
