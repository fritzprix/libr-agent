import React from 'react';
import type { ToolCall, Message } from '@/models/chat';
import { AgentMessageRenderer } from './AgentMessageRenderer';
import { AlertCircle, Loader2 } from 'lucide-react';
import { parseToolArguments } from '@/lib/tool-call-utils';

interface AgentToolCallDetailsProps {
  toolCall: ToolCall;
  toolResult?: Message;
  hasError?: boolean;
  isLoading?: boolean;
  isDark?: boolean;
}

/**
 * Agent V2 version of ToolCallDetails component.
 * Uses AgentMessageRenderer instead of legacy MessageRenderer.
 *
 * Displays tool call details (parameters and result/error).
 * Used by AgentToolCallGroup for consistency.
 */
export const AgentToolCallDetails: React.FC<AgentToolCallDetailsProps> = ({
  toolCall,
  toolResult,
  hasError = false,
  isLoading = false,
  isDark,
}) => {
  const params = parseToolArguments(toolCall.function.arguments);
  const hasParams = Object.keys(params).length > 0;

  return (
    <div className="space-y-3 w-full max-w-full">
      {/* Parameters section */}
      {hasParams && (
        <div>
          <div className="text-xs font-medium text-muted-foreground mb-2">
            Parameters
          </div>
          <div className="bg-muted/50 rounded p-2 w-full max-w-full min-w-0 max-h-96 overflow-y-auto">
            <pre className="text-xs font-mono w-full whitespace-pre-wrap break-all">
              {JSON.stringify(params, null, 2)}
            </pre>
          </div>
        </div>
      )}

      {/* Result or Error section */}
      {toolResult && (
        <div>
          <div className="text-xs font-medium text-muted-foreground mb-2">
            {hasError ? 'Error Details' : 'Result'}
          </div>
          {hasError ? (
            <div className="bg-destructive/10 border border-destructive/20 rounded p-3">
              <div className="flex items-start gap-2">
                <AlertCircle className="w-4 h-4 text-destructive flex-shrink-0 mt-0.5" />
                <div className="flex-1 min-w-0">
                  <AgentMessageRenderer
                    message={toolResult}
                    className="text-sm text-foreground"
                    isDark={isDark}
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-background rounded border p-2">
              <AgentMessageRenderer
                message={toolResult}
                className="text-sm"
                expandResources={true}
                isDark={isDark}
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
  );
};
