import { useState, useEffect, useRef, useMemo } from 'react';
import { Button } from '@/components/ui';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import { createId } from '@paralleldrive/cuid2';
import { cn } from '@/lib/utils';
import type { Message, ToolCall } from '@/models/chat';
import { AgentToolCallGroup } from './components/AgentToolCallGroup';
import { AgentMessageRenderer } from './components/AgentMessageRenderer';

const logger = getLogger('AgentChatView');

/**
 * Agent Chat View
 *
 * Enhanced UI for agent chat interaction with tool call visualization.
 *
 * Pattern: Uses AgentToolCallGroup for grouped tool call rendering
 *
 * Features:
 * - Message grouping (user, assistant, tool groups)
 * - Tool call visualization with status indicators
 * - Basic message rendering for text content
 */
export default function AgentChatView() {
  const { currentSession } = useAgentSessionState();
  const {
    messages,
    isLoading,
    error,
    llmError,
    workflowStatus,
    submit,
    cancel,
  } = useAgentChat();
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Group messages for rendering
  const groupedMessages = useMemo(() => {
    const result: Array<{
      type: 'single' | 'tool_group';
      message: Message;
      toolGroup?: { calls: ToolCall[] };
    }> = [];

    let i = 0;
    while (i < messages.length) {
      const msg = messages[i];

      // Skip standalone tool results (they're shown within tool groups)
      if (msg.role === 'tool') {
        i++;
        continue;
      }

      // Group assistant messages with tool_calls
      if (msg.role === 'assistant' && msg.tool_calls?.length) {
        const allToolCalls: ToolCall[] = [];
        let j = i;

        // Collect consecutive assistant messages with tool calls
        while (j < messages.length) {
          const currentMsg = messages[j];
          if (currentMsg.role !== 'assistant' || !currentMsg.tool_calls?.length)
            break;

          allToolCalls.push(...currentMsg.tool_calls);

          // Skip past associated tool results
          const toolCallIds = new Set(currentMsg.tool_calls.map((tc) => tc.id));
          j++;
          while (
            j < messages.length &&
            messages[j].role === 'tool' &&
            messages[j].tool_call_id &&
            toolCallIds.has(messages[j].tool_call_id!)
          ) {
            j++;
          }
        }

        result.push({
          type: 'tool_group',
          message: msg,
          toolGroup: { calls: allToolCalls },
        });
        i = j;
      } else {
        // Regular message (user or assistant without tool calls)
        result.push({ type: 'single', message: msg });
        i++;
      }
    }

    return result;
  }, [messages]);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, groupedMessages]);

  const handleSubmit = async () => {
    if (!input.trim() || !currentSession?.id) return;

    const now = new Date();
    const message: Message = {
      id: createId(),
      sessionId: currentSession.id,
      threadId: currentSession.id,
      role: 'user',
      content: [{ type: 'text', text: input }],
      createdAt: now,
      updatedAt: now,
    };

    logger.debug('Submitting message', { messageId: message.id });
    setInput('');
    await submit(message);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-border">
        <div>
          <h2 className="text-lg font-semibold">
            {currentSession?.name || 'Agent Session'}
          </h2>
          <div className="text-sm text-muted-foreground">
            Status:{' '}
            <span className={getStatusColor(workflowStatus)}>
              {workflowStatus}
            </span>
          </div>
        </div>
        <Button
          onClick={cancel}
          disabled={workflowStatus !== 'busy'}
          variant="outline"
          size="sm"
        >
          Cancel
        </Button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {groupedMessages.map((grouped, index) => {
          if (grouped.type === 'tool_group') {
            // Render tool call group
            return (
              <AgentToolCallGroup
                key={grouped.message.id}
                message={grouped.message}
                toolGroup={grouped.toolGroup!}
                isLast={index === groupedMessages.length - 1}
                visibleCount={3}
              />
            );
          }

          // Render regular message
          const msg = grouped.message;
          return (
            <div
              key={msg.id}
              className={cn(
                'flex',
                msg.role === 'user' ? 'justify-end' : 'justify-start',
              )}
            >
              <div
                className={cn(
                  'inline-block max-w-[70%] p-3 rounded-lg',
                  msg.role === 'user'
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-secondary text-secondary-foreground',
                )}
              >
                <div className="text-xs font-semibold mb-1 opacity-70">
                  {msg.role.toUpperCase()}
                </div>
                <div className="whitespace-pre-wrap">
                  {msg.content && msg.content.length > 0 ? (
                    <AgentMessageRenderer content={msg.content} message={msg} />
                  ) : (
                    <span className="text-muted-foreground italic">
                      No content
                    </span>
                  )}
                </div>
              </div>
            </div>
          );
        })}

        {/* Loading Indicator */}
        {isLoading && (
          <div className="flex justify-start">
            <div className="inline-block p-3 bg-secondary rounded-lg">
              <div className="flex items-center space-x-2">
                <div className="w-2 h-2 bg-muted-foreground rounded-full animate-pulse" />
                <div className="w-2 h-2 bg-muted-foreground rounded-full animate-pulse delay-75" />
                <div className="w-2 h-2 bg-muted-foreground rounded-full animate-pulse delay-150" />
              </div>
            </div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Error Display */}
      {error && (
        <div className="px-4 py-2 bg-destructive/20 border-t border-destructive">
          <div className="text-destructive text-sm">
            <span className="font-semibold">Workflow Error:</span> {error}
          </div>
        </div>
      )}
      {llmError && (
        <div className="px-4 py-2 bg-orange-500/20 border-t border-orange-500">
          <div className="text-orange-600 text-sm">
            <span className="font-semibold">LLM Completion Error:</span>{' '}
            {llmError}
          </div>
        </div>
      )}

      {/* Input */}
      <div className="p-4 border-t border-border">
        <div className="flex items-center space-x-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type your message..."
            disabled={isLoading}
            className="flex-1 px-4 py-2 bg-background border border-input rounded-lg focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50"
          />
          <Button onClick={handleSubmit} disabled={!input.trim() || isLoading}>
            Send
          </Button>
        </div>
      </div>
    </div>
  );
}

/**
 * Get color class for workflow status
 */
function getStatusColor(status: string): string {
  switch (status) {
    case 'idle':
      return 'text-muted-foreground';
    case 'busy':
      return 'text-yellow-500';
    case 'paused':
      return 'text-blue-500';
    case 'error':
      return 'text-destructive';
    default:
      return 'text-muted-foreground';
  }
}
