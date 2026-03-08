import React, { useState, useMemo, memo, useId } from 'react';
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
import { hasToolCallError } from '@/lib/tool-call-utils';
import { ToolCallCompactItem } from './ToolCallCompactItem';
import { useSettings } from '@/hooks/use-settings';
import { useTranslation } from 'react-i18next';

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
  ariaControls?: string;
}

/**
 * Header section showing tool execution count, title, and status badges
 */
const GroupHeader: React.FC<GroupHeaderProps & { isSimpleMode: boolean }> = ({
  totalCalls,
  statusSummary,
  isSimpleMode,
}) => {
  const { t } = useTranslation('common');
  return (
    <div className="flex items-center justify-between p-3 border-b border-muted/20">
      <div className="flex items-center gap-2">
        <Wrench className="w-4 h-4 text-muted-foreground" />
        <span className="font-medium text-sm">
          {t('agent.toolGroup.header', {
            count: totalCalls,
            defaultValue: `Tool Executions (${totalCalls} call${totalCalls === 1 ? '' : 's'})`,
          })}
        </span>
      </div>
      <StatusBadges {...statusSummary} isSimpleMode={isSimpleMode} />
    </div>
  );
};

/**
 * Status badges showing running/success/error counts.
 * In simple mode only the error badge is shown (if any).
 */
const StatusBadges: React.FC<StatusBadgesProps & { isSimpleMode: boolean }> = ({
  runningCount,
  successCount,
  errorCount,
  isSimpleMode,
}) => {
  return (
    <div className="flex items-center gap-2">
      {!isSimpleMode && runningCount > 0 && (
        <Badge
          variant="outline"
          className="gap-1 border-primary text-primary bg-primary/10"
        >
          <Loader2 className="w-3 h-3 animate-spin" />
          {runningCount}
        </Badge>
      )}
      {!isSimpleMode && successCount > 0 && (
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
  const { t } = useTranslation('common');
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
          {t('agent.toolGroup.hiddenCalls', {
            count: hiddenCount,
            defaultValue: `${hiddenCount} older calls hidden`,
          })}
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
  ariaControls,
}) => {
  const { t } = useTranslation('common');
  return (
    <button
      type="button"
      aria-expanded={isExpanded}
      aria-controls={ariaControls}
      className="flex items-center justify-center p-2 border-t border-muted w-full cursor-pointer hover:bg-muted/50 transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none rounded-b-lg"
      onClick={onToggle}
    >
      <span className="text-xs text-muted-foreground font-medium">
        {isExpanded
          ? t('agent.toolGroup.showLess', 'Show Less')
          : t('agent.toolGroup.showAllWithCount', {
              count: totalCalls,
              defaultValue: `Show All (${totalCalls} calls)`,
            })}
      </span>
      <ChevronDown
        className={cn(
          'w-3 h-3 ml-1 transition-transform text-muted-foreground',
          isExpanded && 'rotate-180',
        )}
      />
    </button>
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
  const listId = useId();
  const {
    value: { display },
  } = useSettings();
  const isSimpleMode = (display?.toolDetailLevel ?? 'simple') === 'simple';

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

  // Determine visible items and their corresponding results
  // We slice both arrays identically to ensure index alignment without O(N) searching
  const visibleCalls = isExpanded
    ? toolGroup.calls
    : toolGroup.calls.slice(-visibleCount);

  const visibleResults = isExpanded
    ? toolResults
    : toolResults.slice(-visibleCount);

  const hiddenCount = Math.max(0, toolGroup.calls.length - visibleCount);

  // Container styling — simple mode: always neutral, developer mode: colour-coded
  const hasAnyError = statusSummary.errorCount > 0;
  const isAnyRunning = statusSummary.runningCount > 0;

  const containerClass = cn(
    'rounded-lg border transition-all mb-2 hover:bg-accent/50 w-full max-w-full',
    isSimpleMode
      ? 'border-l-4 border-muted'
      : [
          isAnyRunning && 'border-l-4 border-primary bg-primary/10',
          !isAnyRunning &&
            hasAnyError &&
            'border-l-4 border-destructive bg-destructive/10',
          !isAnyRunning &&
            !hasAnyError &&
            'border-l-4 border-success bg-success/10',
        ],
  );

  return (
    <div className={containerClass}>
      <GroupHeader
        totalCalls={toolGroup.calls.length}
        statusSummary={statusSummary}
        isSimpleMode={isSimpleMode}
      />

      {!isExpanded && (
        <GradientOverlay
          hiddenCount={hiddenCount}
          hasError={hasAnyError}
          isRunning={isAnyRunning}
        />
      )}

      {/* Tool Call List - Compact items without individual borders */}
      <div id={listId} className="px-2 py-2 space-y-0.5">
        {visibleCalls.map((toolCall, index) => {
          // Optimization: Access result directly by parallel index instead of O(N) indexOf search
          const toolResult = visibleResults[index];
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
          ariaControls={listId}
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
  // Optimization: Check for reference equality first (for AgentToolGroupBlock support)
  if (
    prev.toolGroup !== next.toolGroup &&
    prev.toolGroup.calls !== next.toolGroup.calls
  ) {
    // AgentMessageRenderer recreates toolCall objects on every render, so we must check content deeply.
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
  // Optimization: Check for reference equality first
  if (prev.toolResults !== next.toolResults) {
    // We assume Message objects are stable (from useAgentChatState)
    if (prev.toolResults.length !== next.toolResults.length) return false;
    for (let i = 0; i < prev.toolResults.length; i++) {
      if (prev.toolResults[i] !== next.toolResults[i]) return false;
    }
  }

  return true;
}

export const AgentToolCallGroup = memo(AgentToolCallGroupImpl, arePropsEqual);
export default AgentToolCallGroup;
