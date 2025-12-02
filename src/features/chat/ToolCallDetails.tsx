import React from 'react';
import type { ToolCall, Message } from '@/models/chat';
import { MessageRenderer } from '@/components/MessageRenderer';
import { AlertCircle, Loader2 } from 'lucide-react';
import { parseToolArguments } from '@/lib/tool-call-utils';

interface ToolCallDetailsProps {
  toolCall: ToolCall;
  toolResult?: Message;
  hasError?: boolean;
  isLoading?: boolean;
}

/**
 * Shared component for displaying tool call details (parameters and result/error).
 * Used by both ToolCallResultBubble and ToolCallGroupBubble for consistency.
 */
export const ToolCallDetails: React.FC<ToolCallDetailsProps> = ({
  toolCall,
  toolResult,
  hasError = false,
  isLoading = false,
}) => {
  const params = parseToolArguments(toolCall.function.arguments);
  const hasParams = Object.keys(params).length > 0;

  return (
    <div className="space-y-3 w-full">
      {/* Parameters section */}
      {hasParams && (
        <div>
          <div className="text-xs font-medium text-muted-foreground mb-2">
            Parameters
          </div>
          <div className="bg-muted/50 rounded p-2 w-full overflow-hidden">
            <pre className="text-xs overflow-x-auto font-mono">
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
  );
};
