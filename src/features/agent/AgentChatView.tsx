import { useState, useEffect, useRef } from 'react';
import { Button } from '@/components/ui';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { getLogger } from '@/lib/logger';
import { createId } from '@paralleldrive/cuid2';
import { cn } from '@/lib/utils';
import type { Message } from '@/models/chat';
import type { MCPContent } from '@/lib/mcp/protocol/content';

const logger = getLogger('AgentChatView');

/**
 * Agent Chat View (Simple MVP)
 *
 * Minimal UI for agent chat interaction.
 * Focuses on core functionality testing without complex UI.
 *
 * Pattern: Simplified version of V1's Chat compound component
 *
 * Key Simplifications for MVP:
 * - No tool call grouping (simple text display)
 * - No side panels (planning, workspace)
 * - No file attachments
 * - Basic message rendering (text, tool_use, tool_result)
 */
export default function AgentChatView() {
  const { currentSession } = useAgentSessionState();
  const { messages, isLoading, error, workflowStatus, submit, cancel } =
    useAgentChat();
  const [input, setInput] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  const handleSubmit = async () => {
    if (!input.trim() || !currentSession?.id) return;

    const message: Message = {
      id: createId(),
      sessionId: currentSession.id,
      threadId: currentSession.id,
      role: 'user',
      content: [{ type: 'text', text: input }],
      createdAt: new Date(),
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
        {messages.map((msg) => (
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
                {renderMessageContent(msg.content[0])}
              </div>
            </div>
          </div>
        ))}

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
          <div className="text-destructive text-sm">Error: {error}</div>
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
 * Render message content based on type
 * MVP version: Simple text display, JSON for non-text content
 */
function renderMessageContent(content: MCPContent | undefined) {
  if (!content) return null;

  if (content.type === 'text') {
    return content.text;
  }

  // For MVP, show non-text content as formatted JSON
  return (
    <pre className="text-xs overflow-auto">
      {JSON.stringify(content, null, 2)}
    </pre>
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
