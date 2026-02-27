import React, { useState, useEffect, useRef, memo, useMemo } from 'react';
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
import { useSettings } from '@/hooks/use-settings';

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
 * Compact tool call item - no individual border, tight spacing.
 *
 * Renders differently based on the `toolDetailLevel` display setting:
 * - 'simple': shows only tool name + status icon. No params, no error text,
 *             no background colour change on error, no expand capability.
 * - 'developer': full detail view (current behaviour) with params summary,
 *               execution time, error details, and expand/collapse.
 */
const ToolCallCompactItemImpl: React.FC<ToolCallCompactItemProps> = ({
  toolCall,
  toolResult,
  isLast = false,
}) => {
  const {
    value: { display },
  } = useSettings();
  const isSimpleMode = (display?.toolDetailLevel ?? 'simple') === 'simple';

  const [isExpanded, setIsExpanded] = useState(false);
  const previousHasError = useRef(false);
  const previousHasResource = useRef(false);

  // Parse tool name (remove server prefix)
  const toolName = useMemo(
    () => parseToolName(toolCall.function.name),
    [toolCall.function.name],
  );

  // Parse arguments for summary (developer mode only)
  const params = useMemo(
    () => parseToolArguments(toolCall.function.arguments),
    [toolCall.function.arguments],
  );

  const paramSummary = useMemo(
    () => formatToolArgumentsSummary(params),
    [params],
  );

  // Check for error using utility function
  const hasError = hasToolCallError(toolResult);
  const hasResource = hasUIResource(toolResult);

  // Get execution time
  const executionTime = toolResult?.metadata?.executionTime;
  const detailsId = `tool-call-details-${toolCall.id}`;

  // Auto-expand only in developer mode when transitioning to error/resource state.
  useEffect(() => {
    if (isSimpleMode) return;

    const errorBecameVisible = !previousHasError.current && hasError;
    const resourceBecameVisible = !previousHasResource.current && hasResource;

    if (errorBecameVisible) {
      setIsExpanded(true);
    } else if (resourceBecameVisible && isLast) {
      setIsExpanded(true);
    }

    previousHasError.current = hasError;
    previousHasResource.current = hasResource;
  }, [hasError, hasResource, isLast, isSimpleMode]);

  // ── Simple Mode ─────────────────────────────────────────────────────────
  // Shows tool name + status + brief param summary. No expand, no execution
  // time. UI Resources (e.g. circuit break) are always rendered inline so
  // they are never hidden from the user.
  if (isSimpleMode) {
    return (
      <div className="rounded px-3 py-2 text-sm bg-background">
        <div className="flex items-center gap-2">
          <ToolStatusIcon hasResult={!!toolResult} hasError={hasError} />
          <span className="font-medium flex-shrink-0">{toolName}</span>
          {paramSummary && (
            <span className="flex-1 text-xs text-muted-foreground truncate font-mono opacity-70 min-w-0">
              {paramSummary}
            </span>
          )}
        </div>
        {/* Always show UI Resources inline (e.g. circuit break prompt) */}
        {hasResource && toolResult && (
          <div className="mt-2 pt-2 border-t border-muted/50">
            <AgentToolCallDetails
              toolCall={toolCall}
              toolResult={toolResult}
              hasError={hasError}
              isLoading={false}
              showDetails={true}
            />
          </div>
        )}
      </div>
    );
  }

  // ── Developer Mode ───────────────────────────────────────────────────────
  return (
    <div
      className={cn(
        'rounded px-3 py-2 text-sm transition-colors',
        hasError
          ? 'bg-destructive/10 hover:bg-destructive/20'
          : 'bg-background hover:bg-muted/50',
      )}
    >
      {/* Collapsed header line */}
      <button
        type="button"
        className="w-full text-left"
        aria-expanded={isExpanded}
        aria-controls={detailsId}
        onClick={() => setIsExpanded(!isExpanded)}
      >
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
      </button>

      {/* Expanded details */}
      {isExpanded && (
        <div
          id={detailsId}
          className="mt-3 pt-3 border-t border-muted/50 min-w-0"
        >
          <AgentToolCallDetails
            toolCall={toolCall}
            toolResult={toolResult}
            hasError={hasError}
            isLoading={!toolResult}
            showDetails={true}
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

export { arePropsEqual };
export const ToolCallCompactItem = memo(ToolCallCompactItemImpl, arePropsEqual);
