import React, { useState, memo, useMemo, useRef } from 'react';
import type { Message, ToolCall } from '@/models/chat';
import { ChevronDown, CheckCircle, XCircle, Loader2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import {
  hasToolCallError,
  parseToolName,
  formatExecutionTime,
  parseToolArguments,
  formatToolArgumentsSummary,
} from '@/lib/tool-call-utils';
import { AgentToolCallDetails } from './AgentToolCallDetails';
import { useSettings } from '@/hooks/use-settings';
import { resolveToolResultUiOverride } from './tool-structured/presentation';

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

interface CompactHeaderRowProps {
  toolResult?: Message;
  hasError: boolean;
  displayToolName: string;
  paramSummary: string;
  executionTime?: number;
  showChevron?: boolean;
  isExpanded?: boolean;
}

const CompactHeaderRow: React.FC<CompactHeaderRowProps> = ({
  toolResult,
  hasError,
  displayToolName,
  paramSummary,
  executionTime,
  showChevron = false,
  isExpanded = false,
}) => (
  <div className="flex items-center gap-2">
    <ToolStatusIcon hasResult={!!toolResult} hasError={hasError} />
    <span className="flex-shrink-0 font-medium">{displayToolName}</span>
    {paramSummary ? (
      <span className="flex-1 text-xs text-muted-foreground truncate font-mono opacity-70 min-w-0">
        {paramSummary}
      </span>
    ) : (
      <span className="flex-1" />
    )}
    {executionTime !== undefined ? (
      <span className="text-xs text-muted-foreground flex-shrink-0">
        {formatExecutionTime(executionTime)}
      </span>
    ) : null}
    {showChevron ? (
      <ChevronDown
        className={cn(
          'w-3.5 h-3.5 transition-transform flex-shrink-0 text-muted-foreground',
          isExpanded && 'rotate-180',
        )}
      />
    ) : null}
  </div>
);

/**
 * Compact tool call item - no individual border, tight spacing.
 *
 * Renders differently based on the `toolDetailLevel` display setting:
 * - 'simple': tool name + status icon; expandable when the tool failed so
 *             users can read the error. Params stay hidden unless forced.
 * - 'developer': full detail view with params summary, execution time,
 *               error details, and expand/collapse.
 *
 * Results with a UI override (MCP UI resource or structured tool UI) always
 * show details and skip collapse — see {@link resolveToolResultUiOverride}.
 */
const ToolCallCompactItemImpl: React.FC<ToolCallCompactItemProps> = ({
  toolCall,
  toolResult,
}) => {
  const { t } = useTranslation('common');
  const {
    value: { display },
  } = useSettings();
  const isSimpleMode = (display?.toolDetailLevel ?? 'simple') === 'simple';
  const detailMode = isSimpleMode ? 'simple' : 'developer';

  const [isExpanded, setIsExpanded] = useState(false);
  const prevHasErrorRef = useRef(false);

  const toolName = useMemo(
    () => parseToolName(toolCall.function.name),
    [toolCall.function.name],
  );
  const displayToolName =
    toolName || t('agent.toolDetails.preparingTool', 'Preparing tool...');

  const parsedArgs = useMemo(
    () => parseToolArguments(toolCall.function.arguments),
    [toolCall.function.arguments],
  );

  const paramSummary = useMemo(
    () => formatToolArgumentsSummary(parsedArgs),
    [parsedArgs],
  );

  const hasError = hasToolCallError(toolResult);
  const uiOverride = resolveToolResultUiOverride(
    toolCall.function.name,
    toolResult,
    detailMode,
  );
  const forceVisible = uiOverride?.alwaysVisible === true;

  const executionTime = toolResult?.metadata?.executionTime;
  const detailsId = `tool-call-details-${toolCall.id}`;

  // Auto-expand on error transition (non-forced results only).
  // Forced-visible results already mount details without expand state.
  // In simple mode, keep collapsed by default; auto-expand in developer mode only.
  if (!forceVisible && hasError !== prevHasErrorRef.current) {
    const errorBecameVisible = !prevHasErrorRef.current && hasError;
    prevHasErrorRef.current = hasError;
    if (errorBecameVisible && !isSimpleMode) {
      setIsExpanded(true);
    }
  } else if (hasError !== prevHasErrorRef.current) {
    prevHasErrorRef.current = hasError;
  }

  const details = toolResult ? (
    <div id={detailsId} className="mt-2 pt-2 border-t border-muted/50 min-w-0">
      <AgentToolCallDetails
        toolCall={toolCall}
        toolResult={toolResult}
        hasError={hasError}
        isLoading={false}
        showDetails={true}
        parsedArgs={parsedArgs}
        hideParameters={uiOverride?.hideParameters ?? false}
      />
    </div>
  ) : null;

  // ── Simple Mode ─────────────────────────────────────────────────────────
  if (isSimpleMode) {
    // Failures stay inspectable; successful calls stay collapsed unless forced.
    if (!forceVisible && hasError) {
      return (
        <div
          className={cn(
            'rounded px-3 py-2 text-sm transition-colors',
            'bg-destructive/10 hover:bg-destructive/20',
          )}
          style={{ overflowAnchor: 'none' }}
        >
          <button
            type="button"
            className="w-full text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-md"
            aria-expanded={isExpanded}
            aria-controls={detailsId}
            aria-label={t(
              'agentChat.toolDetails.toggleAriaLabel',
              'Toggle {{toolName}} details',
              { toolName: displayToolName },
            )}
            onClick={() => setIsExpanded((prev) => !prev)}
          >
            <CompactHeaderRow
              toolResult={toolResult}
              hasError={hasError}
              displayToolName={displayToolName}
              paramSummary=""
              showChevron
              isExpanded={isExpanded}
            />
          </button>
          {isExpanded ? details : null}
        </div>
      );
    }

    return (
      <div
        className="rounded px-3 py-2 text-sm bg-background"
        style={{ overflowAnchor: 'none' }}
      >
        <CompactHeaderRow
          toolResult={toolResult}
          hasError={hasError}
          displayToolName={displayToolName}
          paramSummary={paramSummary}
        />
        {forceVisible ? details : null}
      </div>
    );
  }

  // ── Developer Mode — forced visible (static header, no collapse) ────────
  if (forceVisible) {
    return (
      <div
        className={cn(
          'rounded px-3 py-2 text-sm transition-colors',
          hasError
            ? 'bg-destructive/10 hover:bg-destructive/20'
            : 'bg-background hover:bg-muted/50',
        )}
        style={{ overflowAnchor: 'none' }}
      >
        <CompactHeaderRow
          toolResult={toolResult}
          hasError={hasError}
          displayToolName={displayToolName}
          paramSummary={paramSummary}
          executionTime={
            typeof executionTime === 'number' ? executionTime : undefined
          }
        />
        {details}
      </div>
    );
  }

  // ── Developer Mode — default expand/collapse ────────────────────────────
  return (
    <div
      className={cn(
        'rounded px-3 py-2 text-sm transition-colors',
        hasError
          ? 'bg-destructive/10 hover:bg-destructive/20'
          : 'bg-background hover:bg-muted/50',
      )}
      style={{ overflowAnchor: 'none' }}
    >
      <button
        type="button"
        className="w-full text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-md"
        aria-expanded={isExpanded}
        aria-controls={detailsId}
        aria-label={t(
          'agentChat.toolDetails.toggleAriaLabel',
          'Toggle {{toolName}} details',
          { toolName: displayToolName },
        )}
        onClick={() => setIsExpanded((prev) => !prev)}
      >
        <CompactHeaderRow
          toolResult={toolResult}
          hasError={hasError}
          displayToolName={displayToolName}
          paramSummary={paramSummary}
          executionTime={
            typeof executionTime === 'number' ? executionTime : undefined
          }
          showChevron
          isExpanded={isExpanded}
        />
      </button>

      {isExpanded ? (
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
            parsedArgs={parsedArgs}
          />
        </div>
      ) : null}
    </div>
  );
};

function arePropsEqual(
  prev: ToolCallCompactItemProps,
  next: ToolCallCompactItemProps,
) {
  if (prev.isLast !== next.isLast) return false;
  if (prev.toolResult !== next.toolResult) return false;
  if (prev.toolCall.id !== next.toolCall.id) return false;
  if (prev.toolCall.function.name !== next.toolCall.function.name) return false;
  if (prev.toolCall.function.arguments !== next.toolCall.function.arguments)
    return false;

  return true;
}

export { arePropsEqual };
export const ToolCallCompactItem = memo(ToolCallCompactItemImpl, arePropsEqual);
