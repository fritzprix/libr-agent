import React, { useMemo } from 'react';
import type { ToolCall, Message } from '@/models/chat';
import { AgentMessageRenderer } from './AgentMessageRenderer';
import { AlertCircle, Loader2 } from 'lucide-react';
import {
  getToolStructuredContent,
  hasUIResource,
  parseToolArguments,
} from '@/lib/tool-call-utils';
import { cn } from '@/lib/utils';
import { useTranslation } from 'react-i18next';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import {
  canRenderStructuredToolResult,
  ToolStructuredResult,
} from './tool-structured/ToolStructuredResult';
import { AgentSessionToolWaiting } from './tool-structured/AgentSessionToolWaiting';
import { isCheckSessionWaitTool } from './tool-structured/types';

interface AgentToolCallDetailsProps {
  toolCall: ToolCall;
  toolResult?: Message;
  hasError?: boolean;
  isLoading?: boolean;
  /** When false, renders nothing. Used by Simple display mode to hide all detail. */
  showDetails?: boolean;
  /** Pre-parsed arguments to avoid redundant JSON.parse calls */
  parsedArgs?: Record<string, unknown>;
  /** When true, hides the parameters section. Useful for UI resources in simple mode. */
  hideParameters?: boolean;
}

/**
 * Agent V2 version of ToolCallDetails component.
 * Uses AgentMessageRenderer instead of legacy MessageRenderer.
 *
 * Displays tool call details (parameters and result/error).
 * showDetails=false collapses the panel entirely (Simple mode).
 */
export const AgentToolCallDetails: React.FC<AgentToolCallDetailsProps> = ({
  toolCall,
  toolResult,
  hasError = false,
  isLoading = false,
  showDetails = true,
  parsedArgs,
  hideParameters = false,
}) => {
  const { t } = useTranslation('common');
  const { session } = useAgentSessionState();
  const params = useMemo(
    () => parsedArgs || parseToolArguments(toolCall.function.arguments),
    [parsedArgs, toolCall.function.arguments],
  );
  const containsUIResource = hasUIResource(toolResult);
  const structuredContent = getToolStructuredContent(toolResult);
  const showStructuredResult =
    !hasError &&
    structuredContent !== undefined &&
    canRenderStructuredToolResult(toolCall.function.name, structuredContent);
  const showSessionWaiting =
    isLoading &&
    !toolResult &&
    isCheckSessionWaitTool(toolCall.function.name, params);
  const childSessionRef =
    typeof params.sessionId === 'string' ? params.sessionId : '';

  if (!showDetails) return null;

  const hasParams = !hideParameters && Object.keys(params).length > 0;

  return (
    <div className="space-y-3 w-full max-w-full">
      {/* Parameters section */}
      {hasParams && (
        <div>
          <div className="text-xs font-medium text-muted-foreground mb-2">
            {t('agent.toolDetails.parameters', 'Parameters')}
          </div>
          <div className="bg-muted/50 rounded p-2 w-full max-w-full min-w-0 max-h-96 overflow-y-auto">
            <pre className="text-xs font-mono w-full whitespace-pre-wrap break-all">
              {JSON.stringify(params, null, 2)}
            </pre>
          </div>
        </div>
      )}

      {/* In-flight delegated wait — before tool result exists */}
      {showSessionWaiting && session?.id && childSessionRef ? (
        <AgentSessionToolWaiting
          callerSessionId={session.id}
          childSessionRef={childSessionRef}
          displayName={childSessionRef}
        />
      ) : null}

      {/* Result or Error section */}
      {toolResult && (
        <div>
          <div className="text-xs font-medium text-muted-foreground mb-2">
            {hasError
              ? t('agent.toolDetails.errorDetails', 'Error Details')
              : t('agent.toolDetails.result', 'Result')}
          </div>
          {hasError ? (
            <div
              className={cn(
                'bg-destructive/10 border border-destructive/20 rounded p-3',
                !containsUIResource && 'max-h-96 overflow-y-auto',
              )}
            >
              <div className="flex items-start gap-2">
                <AlertCircle className="w-4 h-4 text-destructive flex-shrink-0 mt-0.5" />
                <div className="flex-1 min-w-0">
                  <AgentMessageRenderer
                    message={toolResult}
                    className="text-sm text-foreground"
                  />
                </div>
              </div>
            </div>
          ) : (
            <div
              data-testid="tool-call-result"
              className={cn(
                'bg-background rounded border p-2',
                !containsUIResource &&
                  !showStructuredResult &&
                  'max-h-96 overflow-y-auto',
              )}
            >
              {showStructuredResult ? (
                <ToolStructuredResult
                  toolName={toolCall.function.name}
                  data={structuredContent}
                />
              ) : (
                <AgentMessageRenderer
                  message={toolResult}
                  className="text-sm"
                  expandResources={true}
                />
              )}
            </div>
          )}
        </div>
      )}

      {/* Generic loading — skipped when specialized waiting chrome is shown */}
      {isLoading && !showSessionWaiting && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="w-4 h-4 animate-spin" />
          <span>{t('agent.toolDetails.executing', 'Executing tool...')}</span>
        </div>
      )}
    </div>
  );
};
