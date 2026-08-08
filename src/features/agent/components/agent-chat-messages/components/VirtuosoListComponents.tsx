import { forwardRef } from 'react';
import { type ListProps } from 'react-virtuoso';
import { Bot } from 'lucide-react';
import { ErrorBubble } from '@/components/shared/ErrorBubble';
import { AnalysisLoader } from '@/features/agent/components/shared';
import { PendingApprovalWidget } from '@/features/agent/components/PendingApprovalWidget';
import { shouldShowAnalysisLoader } from '../utils';
import {
  CHAT_COMPOSER_CLEARANCE,
  CHAT_LIST_HEADER_MIN_HEIGHT_PX,
  type AgentChatVirtuosoContextProps,
} from '../types';

export const AgentChatMessagesList = forwardRef<
  HTMLDivElement,
  ListProps & AgentChatVirtuosoContextProps
>(function AgentChatMessagesList(props, ref) {
  const { children, style, context, ...domProps } = props;
  void context;

  return (
    <div
      {...domProps}
      ref={ref}
      style={{
        // Preserve Virtuoso's paddingTop/paddingBottom — they are the
        // virtualization offsets for off-screen items, not visual chrome.
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
  const showOlderMessagesHint =
    context.hasOlderMessages || context.isLoadingOlderMessages;

  return (
    <div
      className="box-border flex shrink-0 items-center justify-center overflow-hidden px-4"
      style={{
        height: CHAT_LIST_HEADER_MIN_HEIGHT_PX,
        minHeight: CHAT_LIST_HEADER_MIN_HEIGHT_PX,
      }}
      aria-hidden={!showOlderMessagesHint}
      data-testid="agent-chat-messages-header"
    >
      {showOlderMessagesHint ? (
        <div className="rounded-full border border-border/60 bg-background/80 px-3 py-1 text-xs text-muted-foreground shadow-sm">
          {context.isLoadingOlderMessages
            ? context.loadingOlderLabel
            : context.scrollToLoadOlderLabel}
        </div>
      ) : null}
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
          <div className="w-full max-w-3xl rounded-lg bg-secondary/30 px-6 py-5">
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
            executionMode={context.executionMode}
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
