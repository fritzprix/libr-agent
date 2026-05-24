import { forwardRef } from 'react';
import { type ListProps } from 'react-virtuoso';
import { Bot } from 'lucide-react';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from '../../shared';
import { PendingApprovalWidget } from '../../PendingApprovalWidget';
import { shouldShowAnalysisLoader } from '../utils';
import {
  CHAT_COMPOSER_CLEARANCE,
  type AgentChatVirtuosoContextProps,
} from '../types';

export const AgentChatMessagesList = forwardRef<
  HTMLDivElement,
  ListProps & AgentChatVirtuosoContextProps
>(function AgentChatMessagesList(props, ref) {
  const { children, style, ...rest } = props;

  // Cleanly delete the custom 'context' property to prevent it from forwarding to the DOM element,
  // avoiding any unused variable ESLint warnings.
  const cleanedProps = rest as Record<string, unknown>;
  delete cleanedProps.context;

  return (
    <div
      {...cleanedProps}
      ref={ref}
      style={{
        ...style,
        paddingLeft: '16px',
        paddingRight: '16px',
      }}
    >
      {children}
    </div>
  );
});

export function AgentChatMessagesHeader({
  context,
}: AgentChatVirtuosoContextProps) {
  if (!context.hasOlderMessages && !context.isLoadingOlderMessages) {
    return null;
  }

  return (
    <div className="flex justify-center px-4">
      <div className="rounded-full border border-border/60 bg-background/80 px-3 py-1 text-xs text-muted-foreground shadow-sm">
        {context.isLoadingOlderMessages
          ? context.loadingOlderLabel
          : context.scrollToLoadOlderLabel}
      </div>
    </div>
  );
}

export function AgentChatMessagesFooter({
  context,
}: AgentChatVirtuosoContextProps) {
  const latestMessage = context.latestMessage;
  const showAnalysisLoader = shouldShowAnalysisLoader(
    latestMessage,
    context.workflowStatus,
  );

  return (
    <div className="px-4">
      {context.agentError && (
        <div className="self-start mt-2">
          <ErrorBubble
            error={context.agentError}
            onRetry={context.retryMessage}
          />
        </div>
      )}

      {context.agentLlmError && (
        <div className="self-start mt-2">
          <ErrorBubble
            error={context.agentLlmError}
            onRetry={context.retryMessage}
          />
        </div>
      )}

      {showAnalysisLoader && (
        <div className="flex justify-start mb-8 mt-3">
          <div className="w-full max-w-full bg-secondary/30 rounded-lg px-6 py-5">
            <div className="flex items-center gap-3 mb-2">
              <div className="w-7 h-7 bg-primary rounded-full flex items-center justify-center animate-pulse">
                <Bot size={16} className="text-primary-foreground" />
              </div>
              <span className="text-xs font-medium">
                {context.sessionAssistantName}
              </span>
            </div>
            <div className="text-sm">
              <AnalysisLoader size="md" />
            </div>
          </div>
        </div>
      )}

      {context.pendingApprovals && context.pendingApprovals.length > 0 && (
        <div className="flex justify-start mb-8 mt-3">
          <PendingApprovalWidget
            approvals={context.pendingApprovals}
            yoloModeEnabled={context.yoloModeEnabled}
            onRespond={context.respondToToolApproval}
          />
        </div>
      )}

      <div
        aria-hidden="true"
        style={{
          height: `calc(var(--agent-chat-composer-overlap, 64px) + ${CHAT_COMPOSER_CLEARANCE}px)`,
        }}
      />
      <div
        ref={(node) => {
          context.footerEndRef.current = node;
        }}
        aria-hidden="true"
        className="h-px w-full shrink-0"
      />
    </div>
  );
}
