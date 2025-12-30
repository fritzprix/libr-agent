# Agent V2 Frontend Integration Guide

This guide provides production-ready example code for integrating frontend components with the Rust backend's `AgentSessionManager` and Agent V2 architecture.

## Table of Contents

1. [Context Provider Setup](#context-provider-setup)
2. [Session Creation & Management](#session-creation--management)
3. [Chat Interface Implementation](#chat-interface-implementation)
4. [Tool Execution Visualization](#tool-execution-visualization)
5. [Error Handling & Recovery](#error-handling--recovery)
6. [State Synchronization](#state-synchronization)
7. [Advanced Patterns](#advanced-patterns)

---

## 1. Context Provider Setup

### App-Level Provider Structure

```tsx
// src/app/App.tsx
import { AgentSessionProvider } from '@/context/AgentSessionContext';
import { AgentChatProvider } from '@/context/AgentChatContext';
import { LLMServiceProvider } from '@/context/LLMServiceContext';

function App() {
  return (
    <LLMServiceProvider>
      {/* LLM service lives at app level, never unmounts */}
      <AgentSessionProvider>
        {/* Session provider wraps the entire app */}
        <Router>
          <Routes>
            <Route path="/agent/:sessionId" element={<AgentSessionLayout />} />
            <Route path="/agent/new" element={<NewAgentSession />} />
          </Routes>
        </Router>
      </AgentSessionProvider>
    </LLMServiceProvider>
  );
}

// Agent session layout with chat context
function AgentSessionLayout() {
  return (
    <AgentChatProvider>
      {/* Chat provider wraps only the active session UI */}
      <AgentChatView />
    </AgentChatProvider>
  );
}
```

---

## 2. Session Creation & Management

### Creating a New Session

```tsx
// src/features/agent/NewAgentSession.tsx
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useAgentSessionActions } from '@/context/AgentSessionContext';
import { useAssistants } from '@/context/AssistantsContext';
import { Button, Select, Input, Card } from '@/components/ui';

export function NewAgentSession() {
  const navigate = useNavigate();
  const { createSession } = useAgentSessionActions();
  const { assistants } = useAssistants();

  const [selectedAssistant, setSelectedAssistant] = useState<string>('');
  const [sessionName, setSessionName] = useState<string>('');
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCreate = async () => {
    if (!selectedAssistant) {
      setError('Please select an assistant');
      return;
    }

    setIsCreating(true);
    setError(null);

    try {
      const assistant = assistants.find((a) => a.id === selectedAssistant);
      if (!assistant) throw new Error('Assistant not found');

      // Create session with Rust backend
      const session = await createSession({
        assistant,
        name: sessionName || `Chat with ${assistant.name}`,
      });

      // Navigate to the new session
      navigate(`/agent/${session.id}`);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create session');
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <div className="container max-w-2xl mx-auto p-6">
      <Card className="p-6">
        <h1 className="text-2xl font-bold mb-6">Create New Agent Session</h1>

        <div className="space-y-4">
          {/* Assistant Selection */}
          <div>
            <label className="block text-sm font-medium mb-2">
              Select Assistant
            </label>
            <Select
              value={selectedAssistant}
              onValueChange={setSelectedAssistant}
              disabled={isCreating}
            >
              <option value="">Choose an assistant...</option>
              {assistants.map((assistant) => (
                <option key={assistant.id} value={assistant.id}>
                  {assistant.name} - {assistant.description}
                </option>
              ))}
            </Select>
          </div>

          {/* Session Name */}
          <div>
            <label className="block text-sm font-medium mb-2">
              Session Name (Optional)
            </label>
            <Input
              value={sessionName}
              onChange={(e) => setSessionName(e.target.value)}
              placeholder="e.g., Code Review Session"
              disabled={isCreating}
            />
          </div>

          {/* Error Display */}
          {error && (
            <div className="p-3 bg-destructive/10 border border-destructive rounded-md">
              <p className="text-destructive text-sm">{error}</p>
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-end space-x-2 pt-4">
            <Button
              variant="outline"
              onClick={() => navigate('/sessions')}
              disabled={isCreating}
            >
              Cancel
            </Button>
            <Button
              onClick={handleCreate}
              disabled={!selectedAssistant || isCreating}
            >
              {isCreating ? 'Creating...' : 'Create Session'}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
```

### Resuming an Existing Session

```tsx
// src/features/agent/SessionList.tsx
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { invoke } from '@tauri-apps/api/core';
import { Button, Card } from '@/components/ui';
import { formatDistanceToNow } from 'date-fns';

interface SessionMetadata {
  id: string;
  name?: string;
  status: 'idle' | 'busy' | 'paused' | 'error';
  created_at: number;
}

export function SessionList() {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<SessionMetadata[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadSessions();
  }, []);

  const loadSessions = async () => {
    setIsLoading(true);
    try {
      const allSessions = await invoke<SessionMetadata[]>(
        'agent_get_all_sessions',
      );
      // Sort by most recent
      allSessions.sort((a, b) => b.created_at - a.created_at);
      setSessions(allSessions);
    } catch (err) {
      console.error('Failed to load sessions:', err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleResume = (sessionId: string) => {
    navigate(`/agent/${sessionId}`);
  };

  return (
    <div className="container max-w-4xl mx-auto p-6">
      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold">Agent Sessions</h1>
        <Button onClick={() => navigate('/agent/new')}>New Session</Button>
      </div>

      {isLoading ? (
        <div className="text-center py-12">
          <p className="text-muted-foreground">Loading sessions...</p>
        </div>
      ) : sessions.length === 0 ? (
        <Card className="p-12 text-center">
          <p className="text-muted-foreground mb-4">No sessions yet</p>
          <Button onClick={() => navigate('/agent/new')}>
            Create Your First Session
          </Button>
        </Card>
      ) : (
        <div className="space-y-3">
          {sessions.map((session) => (
            <Card
              key={session.id}
              className="p-4 hover:bg-accent/50 transition-colors cursor-pointer"
              onClick={() => handleResume(session.id)}
            >
              <div className="flex justify-between items-start">
                <div className="flex-1">
                  <h3 className="font-semibold">
                    {session.name || `Session ${session.id.slice(0, 8)}`}
                  </h3>
                  <p className="text-sm text-muted-foreground">
                    Created {formatDistanceToNow(new Date(session.created_at))}{' '}
                    ago
                  </p>
                </div>
                <div className="flex items-center space-x-3">
                  <StatusBadge status={session.status} />
                  <Button size="sm" variant="ghost">
                    Resume →
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const config = {
    idle: { label: 'Idle', className: 'bg-gray-100 text-gray-700' },
    busy: { label: 'Running', className: 'bg-yellow-100 text-yellow-700' },
    paused: { label: 'Paused', className: 'bg-blue-100 text-blue-700' },
    error: { label: 'Error', className: 'bg-red-100 text-red-700' },
  }[status] || { label: status, className: 'bg-gray-100 text-gray-700' };

  return (
    <span
      className={`px-2 py-1 rounded-full text-xs font-medium ${config.className}`}
    >
      {config.label}
    </span>
  );
}
```

---

## 3. Chat Interface Implementation

### Production-Ready Chat Component

```tsx
// src/features/agent/components/AgentChat.tsx
import { useState, useEffect, useRef, useCallback } from 'react';
import { createId } from '@paralleldrive/cuid2';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Button, Textarea } from '@/components/ui';
import { MessageList } from './MessageList';
import { WorkflowControls } from './WorkflowControls';
import { SessionHeader } from './SessionHeader';
import type { Message } from '@/models/chat';

export function AgentChat() {
  const { currentSession } = useAgentSessionState();
  const {
    messages,
    isLoading,
    error,
    llmError,
    workflowStatus,
    submit,
    cancel,
    retryMessage,
  } = useAgentChat();

  const [input, setInput] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages.length]);

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  // Auto-resize textarea
  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.style.height = 'auto';
      textarea.style.height = `${textarea.scrollHeight}px`;
    }
  }, [input]);

  const handleSubmit = useCallback(async () => {
    if (!input.trim() || !currentSession?.id || isSubmitting) return;

    const userMessage: Message = {
      id: createId(),
      sessionId: currentSession.id,
      threadId: currentSession.id,
      role: 'user',
      content: [{ type: 'text', text: input.trim() }],
      createdAt: new Date(),
      updatedAt: new Date(),
    };

    setIsSubmitting(true);
    setInput(''); // Clear input immediately for better UX

    try {
      await submit(userMessage);
    } catch (err) {
      // Restore input on error
      setInput(input);
      console.error('Failed to submit message:', err);
    } finally {
      setIsSubmitting(false);
      // Refocus textarea
      textareaRef.current?.focus();
    }
  }, [input, currentSession?.id, isSubmitting, submit]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleRetry = useCallback(async () => {
    setIsSubmitting(true);
    try {
      await retryMessage();
    } catch (err) {
      console.error('Failed to retry message:', err);
    } finally {
      setIsSubmitting(false);
    }
  }, [retryMessage]);

  if (!currentSession) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-muted-foreground">No active session</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header with session info and controls */}
      <SessionHeader
        session={currentSession}
        status={workflowStatus}
        onCancel={cancel}
      />

      {/* Messages */}
      <div className="flex-1 overflow-y-auto">
        <MessageList
          messages={messages}
          isLoading={isLoading}
          error={error}
          llmError={llmError}
        />
        <div ref={messagesEndRef} />
      </div>

      {/* Error Actions */}
      {error && (
        <div className="px-4 py-3 bg-destructive/10 border-t border-destructive flex justify-between items-center">
          <p className="text-sm text-destructive">
            <strong>Workflow Error:</strong> {error}
          </p>
          <Button size="sm" variant="outline" onClick={handleRetry}>
            Retry
          </Button>
        </div>
      )}

      {/* Workflow Controls (Pause/Resume) */}
      <WorkflowControls status={workflowStatus} sessionId={currentSession.id} />

      {/* Input Area */}
      <div className="border-t border-border p-4">
        <div className="flex items-end space-x-2">
          <div className="flex-1 relative">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Type your message... (Shift+Enter for new line)"
              disabled={isSubmitting || workflowStatus === 'busy'}
              className="min-h-[60px] max-h-[200px] resize-none"
              rows={1}
            />
            <div className="absolute bottom-2 right-2 text-xs text-muted-foreground">
              {input.length} chars
            </div>
          </div>
          <Button
            onClick={handleSubmit}
            disabled={
              !input.trim() || isSubmitting || workflowStatus === 'busy'
            }
            className="h-[60px]"
          >
            {isSubmitting ? 'Sending...' : 'Send'}
          </Button>
        </div>
      </div>
    </div>
  );
}
```

### Session Header Component

```tsx
// src/features/agent/components/SessionHeader.tsx
import { invoke } from '@tauri-apps/api/core';
import { Button } from '@/components/ui';
import { MoreVertical, Pause, Square } from 'lucide-react';
import type { AgentSession } from '@/context/AgentSessionContext';

interface SessionHeaderProps {
  session: AgentSession;
  status: 'idle' | 'busy' | 'paused' | 'error';
  onCancel: () => Promise<void>;
}

export function SessionHeader({
  session,
  status,
  onCancel,
}: SessionHeaderProps) {
  const handlePause = async () => {
    try {
      await invoke('agent_pause_workflow', { sessionId: session.id });
    } catch (err) {
      console.error('Failed to pause workflow:', err);
    }
  };

  const handleResume = async () => {
    try {
      await invoke('agent_resume_workflow', { sessionId: session.id });
    } catch (err) {
      console.error('Failed to resume workflow:', err);
    }
  };

  return (
    <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-background">
      <div className="flex-1">
        <h2 className="text-lg font-semibold truncate">
          {session.name || 'Agent Session'}
        </h2>
        <div className="flex items-center space-x-2 text-sm text-muted-foreground">
          <StatusIndicator status={status} />
          <span>•</span>
          <span>{session.id.slice(0, 8)}</span>
        </div>
      </div>

      <div className="flex items-center space-x-2">
        {status === 'busy' && (
          <>
            <Button
              size="sm"
              variant="outline"
              onClick={handlePause}
              className="flex items-center space-x-1"
            >
              <Pause className="w-3 h-3" />
              <span>Pause</span>
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={onCancel}
              className="flex items-center space-x-1"
            >
              <Square className="w-3 h-3" />
              <span>Stop</span>
            </Button>
          </>
        )}

        {status === 'paused' && (
          <Button size="sm" variant="default" onClick={handleResume}>
            Resume
          </Button>
        )}

        <Button size="sm" variant="ghost">
          <MoreVertical className="w-4 h-4" />
        </Button>
      </div>
    </div>
  );
}

function StatusIndicator({ status }: { status: string }) {
  const config = {
    idle: { dot: 'bg-gray-400', text: 'Idle' },
    busy: { dot: 'bg-yellow-500 animate-pulse', text: 'Processing' },
    paused: { dot: 'bg-blue-500', text: 'Paused' },
    error: { dot: 'bg-red-500', text: 'Error' },
  }[status] || { dot: 'bg-gray-400', text: status };

  return (
    <div className="flex items-center space-x-2">
      <div className={`w-2 h-2 rounded-full ${config.dot}`} />
      <span className="font-medium">{config.text}</span>
    </div>
  );
}
```

---

## 4. Tool Execution Visualization

### Enhanced Tool Call Display

```tsx
// src/features/agent/components/ToolExecutionCard.tsx
import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Card } from '@/components/ui';
import {
  CheckCircle,
  XCircle,
  Loader2,
  ChevronDown,
  ChevronUp,
} from 'lucide-react';
import type { ToolCall } from '@/models/chat';

interface ToolExecutionCardProps {
  toolCall: ToolCall;
  sessionId: string;
  result?: string;
  isError?: boolean;
}

export function ToolExecutionCard({
  toolCall,
  sessionId,
  result,
  isError,
}: ToolExecutionCardProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [executionState, setExecutionState] = useState<
    'pending' | 'running' | 'completed' | 'failed'
  >('pending');
  const [startTime] = useState(Date.now());
  const [duration, setDuration] = useState<number | null>(null);

  useEffect(() => {
    // Listen for tool execution events
    const unlisten = listen<{
      type: string;
      session_id: string;
      tool_name: string;
      success?: boolean;
    }>('agent:event', (event) => {
      if (event.payload.session_id !== sessionId) return;

      const toolName = toolCall.function.name;

      if (
        event.payload.type === 'ToolExecutionStarted' &&
        event.payload.tool_name === toolName
      ) {
        setExecutionState('running');
      } else if (
        event.payload.type === 'ToolExecutionCompleted' &&
        event.payload.tool_name === toolName
      ) {
        setExecutionState(event.payload.success ? 'completed' : 'failed');
        setDuration(Date.now() - startTime);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [sessionId, toolCall.function.name, startTime]);

  const getStatusIcon = () => {
    switch (executionState) {
      case 'pending':
        return <Loader2 className="w-4 h-4 text-gray-400" />;
      case 'running':
        return <Loader2 className="w-4 h-4 text-blue-500 animate-spin" />;
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-500" />;
      case 'failed':
        return <XCircle className="w-4 h-4 text-red-500" />;
    }
  };

  const getStatusColor = () => {
    switch (executionState) {
      case 'pending':
        return 'border-gray-200';
      case 'running':
        return 'border-blue-300 bg-blue-50/50';
      case 'completed':
        return 'border-green-300 bg-green-50/50';
      case 'failed':
        return 'border-red-300 bg-red-50/50';
    }
  };

  // Parse tool name (remove server prefix)
  const displayName =
    toolCall.function.name.split('__').pop() || toolCall.function.name;
  const serverName = toolCall.function.name.split('__')[0];

  return (
    <Card className={`p-3 border-2 transition-all ${getStatusColor()}`}>
      <div
        className="flex items-start justify-between cursor-pointer"
        onClick={() => setIsExpanded(!isExpanded)}
      >
        <div className="flex items-start space-x-3 flex-1">
          {getStatusIcon()}
          <div className="flex-1 min-w-0">
            <div className="flex items-center space-x-2">
              <span className="font-medium text-sm">{displayName}</span>
              <span className="text-xs text-muted-foreground bg-muted px-2 py-0.5 rounded">
                {serverName}
              </span>
            </div>
            {duration !== null && (
              <span className="text-xs text-muted-foreground">
                {duration}ms
              </span>
            )}
          </div>
        </div>
        {isExpanded ? (
          <ChevronUp className="w-4 h-4 text-muted-foreground" />
        ) : (
          <ChevronDown className="w-4 h-4 text-muted-foreground" />
        )}
      </div>

      {isExpanded && (
        <div className="mt-3 space-y-2 pl-7">
          {/* Arguments */}
          <div>
            <div className="text-xs font-semibold text-muted-foreground mb-1">
              Arguments:
            </div>
            <pre className="text-xs bg-muted p-2 rounded overflow-x-auto">
              {JSON.stringify(JSON.parse(toolCall.function.arguments), null, 2)}
            </pre>
          </div>

          {/* Result */}
          {result && (
            <div>
              <div className="text-xs font-semibold text-muted-foreground mb-1">
                {isError ? 'Error:' : 'Result:'}
              </div>
              <pre
                className={`text-xs p-2 rounded overflow-x-auto ${
                  isError ? 'bg-red-50 text-red-900' : 'bg-muted'
                }`}
              >
                {result}
              </pre>
            </div>
          )}
        </div>
      )}
    </Card>
  );
}
```

---

## 5. Error Handling & Recovery

### Comprehensive Error Handler

```tsx
// src/features/agent/components/ErrorBoundary.tsx
import { Component, ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button, Card } from '@/components/ui';
import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  children: ReactNode;
  sessionId?: string;
}

interface State {
  hasError: boolean;
  error: Error | null;
  errorInfo: React.ErrorInfo | null;
}

export class AgentErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false, error: null, errorInfo: null };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, errorInfo: null };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('Agent error boundary caught error:', error, errorInfo);
    this.setState({ errorInfo });

    // Report to backend if session is active
    if (this.props.sessionId) {
      invoke('agent_handle_llm_error', {
        sessionId: this.props.sessionId,
        error: `Frontend error: ${error.message}\n${errorInfo.componentStack}`,
      }).catch(console.error);
    }
  }

  handleReset = () => {
    this.setState({ hasError: false, error: null, errorInfo: null });
  };

  handleTerminate = async () => {
    if (this.props.sessionId) {
      try {
        await invoke('agent_terminate_workflow', {
          sessionId: this.props.sessionId,
        });
        this.handleReset();
      } catch (err) {
        console.error('Failed to terminate workflow:', err);
      }
    }
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex items-center justify-center h-full p-6">
          <Card className="max-w-2xl w-full p-6">
            <div className="flex items-start space-x-4">
              <AlertTriangle className="w-8 h-8 text-destructive flex-shrink-0" />
              <div className="flex-1">
                <h2 className="text-xl font-bold mb-2">Something went wrong</h2>
                <p className="text-muted-foreground mb-4">
                  The agent session encountered an unexpected error. You can try
                  to recover or restart the session.
                </p>

                {/* Error Details (Collapsible) */}
                <details className="mb-4">
                  <summary className="cursor-pointer text-sm font-medium mb-2">
                    Error Details
                  </summary>
                  <div className="space-y-2">
                    <div>
                      <div className="text-xs font-semibold text-muted-foreground mb-1">
                        Error Message:
                      </div>
                      <pre className="text-xs bg-muted p-2 rounded overflow-x-auto">
                        {this.state.error?.message}
                      </pre>
                    </div>
                    {this.state.errorInfo && (
                      <div>
                        <div className="text-xs font-semibold text-muted-foreground mb-1">
                          Component Stack:
                        </div>
                        <pre className="text-xs bg-muted p-2 rounded overflow-x-auto max-h-40">
                          {this.state.errorInfo.componentStack}
                        </pre>
                      </div>
                    )}
                  </div>
                </details>

                {/* Actions */}
                <div className="flex space-x-2">
                  <Button
                    onClick={this.handleReset}
                    className="flex items-center space-x-2"
                  >
                    <RefreshCw className="w-4 h-4" />
                    <span>Try Again</span>
                  </Button>
                  {this.props.sessionId && (
                    <Button variant="outline" onClick={this.handleTerminate}>
                      Terminate & Reset Session
                    </Button>
                  )}
                </div>
              </div>
            </div>
          </Card>
        </div>
      );
    }

    return this.props.children;
  }
}
```

### Usage in App

```tsx
// src/app/Router.tsx
import { AgentErrorBoundary } from '@/features/agent/components/ErrorBoundary';

function AgentSessionRoute() {
  const { sessionId } = useParams();

  return (
    <AgentErrorBoundary sessionId={sessionId}>
      <AgentSessionLayout />
    </AgentErrorBoundary>
  );
}
```

---

## 6. State Synchronization

### Real-time Event Listener Hook

```tsx
// src/hooks/use-agent-events.ts
import { useEffect, useRef, useCallback } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { getLogger } from '@/lib/logger';

const logger = getLogger('useAgentEvents');

interface AgentEventHandler {
  onWorkflowStarted?: (sessionId: string) => void;
  onWorkflowCompleted?: (sessionId: string) => void;
  onWorkflowError?: (sessionId: string, error: string) => void;
  onStatusChanged?: (sessionId: string, status: string) => void;
  onMessageAdded?: (sessionId: string, message: unknown) => void;
  onToolExecutionStarted?: (sessionId: string, toolName: string) => void;
  onToolExecutionCompleted?: (
    sessionId: string,
    toolName: string,
    success: boolean,
  ) => void;
}

export function useAgentEvents(
  sessionId: string | undefined,
  handlers: AgentEventHandler,
) {
  const handlersRef = useRef(handlers);

  // Update handlers ref on changes
  useEffect(() => {
    handlersRef.current = handlers;
  }, [handlers]);

  useEffect(() => {
    if (!sessionId) return;

    let unlisten: UnlistenFn | undefined;
    let isMounted = true;

    const setupListener = async () => {
      logger.info('Setting up agent event listener', { sessionId });

      unlisten = await listen<Record<string, unknown>>(
        'agent:event',
        (event) => {
          if (!isMounted) return;

          const payload = event.payload;
          const eventSessionId = payload.session_id as string;

          // Filter events for this session only
          if (eventSessionId !== sessionId) return;

          const eventType = payload.type as string;
          logger.debug('Received agent event', {
            sessionId,
            eventType,
            payload,
          });

          // Route to appropriate handler
          const h = handlersRef.current;
          switch (eventType) {
            case 'WorkflowStarted':
              h.onWorkflowStarted?.(sessionId);
              break;
            case 'WorkflowCompleted':
              h.onWorkflowCompleted?.(sessionId);
              break;
            case 'WorkflowError':
              h.onWorkflowError?.(sessionId, payload.error as string);
              break;
            case 'StatusChanged':
              h.onStatusChanged?.(sessionId, payload.status as string);
              break;
            case 'MessageAdded':
              h.onMessageAdded?.(sessionId, payload.message);
              break;
            case 'ToolExecutionStarted':
              h.onToolExecutionStarted?.(
                sessionId,
                payload.tool_name as string,
              );
              break;
            case 'ToolExecutionCompleted':
              h.onToolExecutionCompleted?.(
                sessionId,
                payload.tool_name as string,
                payload.success as boolean,
              );
              break;
            default:
              logger.warn('Unknown agent event type', { eventType, payload });
          }
        },
      );

      logger.info('Agent event listener setup complete', { sessionId });
    };

    setupListener();

    return () => {
      isMounted = false;
      if (unlisten) {
        logger.info('Cleaning up agent event listener', { sessionId });
        unlisten();
      }
    };
  }, [sessionId]);
}
```

### Usage Example

```tsx
// src/features/agent/AgentChatView.tsx
import { useAgentEvents } from '@/hooks/use-agent-events';
import { toast } from '@/components/ui/use-toast';

export function AgentChatView() {
  const { currentSession } = useAgentSessionState();
  const [toolExecutions, setToolExecutions] = useState<
    Record<string, 'running' | 'completed' | 'failed'>
  >({});

  useAgentEvents(currentSession?.id, {
    onWorkflowStarted: (sessionId) => {
      console.log('Workflow started:', sessionId);
      toast({
        title: 'Workflow Started',
        description: 'Agent is processing your request',
      });
    },

    onWorkflowCompleted: (sessionId) => {
      console.log('Workflow completed:', sessionId);
      toast({
        title: 'Workflow Completed',
        description: 'Agent has finished processing',
        variant: 'success',
      });
    },

    onWorkflowError: (sessionId, error) => {
      console.error('Workflow error:', sessionId, error);
      toast({
        title: 'Workflow Error',
        description: error,
        variant: 'destructive',
      });
    },

    onToolExecutionStarted: (sessionId, toolName) => {
      setToolExecutions((prev) => ({ ...prev, [toolName]: 'running' }));
    },

    onToolExecutionCompleted: (sessionId, toolName, success) => {
      setToolExecutions((prev) => ({
        ...prev,
        [toolName]: success ? 'completed' : 'failed',
      }));
    },
  });

  // ... rest of component
}
```

---

## 7. Advanced Patterns

### Custom Hook for Session Lifecycle

```tsx
// src/hooks/use-agent-session-lifecycle.ts
import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAgentSessionActions } from '@/context/AgentSessionContext';

export function useAgentSessionLifecycle(sessionId: string | undefined) {
  const [isInitialized, setIsInitialized] = useState(false);
  const [initError, setInitError] = useState<string | null>(null);
  const { resumeSession } = useAgentSessionActions();

  // Initialize session on mount
  useEffect(() => {
    if (!sessionId) return;

    const initSession = async () => {
      try {
        // Initialize cache from DB
        await invoke('agent_init_session_with_messages', { sessionId });

        // Resume session context
        await resumeSession(sessionId);

        setIsInitialized(true);
      } catch (err) {
        const error = err instanceof Error ? err.message : String(err);
        setInitError(error);
        console.error('Failed to initialize session:', error);
      }
    };

    initSession();
  }, [sessionId, resumeSession]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      // Note: Session continues running in Rust backend
      // This just cleans up frontend state
      console.log('Component unmounted, session still active in backend');
    };
  }, []);

  const terminateSession = useCallback(async () => {
    if (!sessionId) return;

    try {
      await invoke('agent_terminate_workflow', { sessionId });
    } catch (err) {
      console.error('Failed to terminate session:', err);
      throw err;
    }
  }, [sessionId]);

  return {
    isInitialized,
    initError,
    terminateSession,
  };
}
```

### Multi-Session Manager Component

```tsx
// src/features/agent/MultiSessionManager.tsx
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Card,
  Button,
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui';
import { AgentChatProvider } from '@/context/AgentChatContext';
import { AgentChat } from './components/AgentChat';

interface SessionTab {
  id: string;
  name: string;
  status: 'idle' | 'busy' | 'paused';
}

export function MultiSessionManager() {
  const [sessions, setSessions] = useState<SessionTab[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);

  useEffect(() => {
    loadActiveSessions();
  }, []);

  const loadActiveSessions = async () => {
    try {
      const allSessions = await invoke<SessionTab[]>('agent_get_all_sessions');
      // Filter to only active or paused sessions
      const active = allSessions.filter((s) => s.status !== 'idle');
      setSessions(active);

      if (active.length > 0 && !activeSessionId) {
        setActiveSessionId(active[0].id);
      }
    } catch (err) {
      console.error('Failed to load sessions:', err);
    }
  };

  const handleNewSession = () => {
    // Navigate to new session creation
    // (Implementation depends on your routing setup)
  };

  return (
    <div className="flex flex-col h-full">
      <div className="border-b border-border">
        <Tabs value={activeSessionId || ''} onValueChange={setActiveSessionId}>
          <div className="flex items-center justify-between px-4 py-2">
            <TabsList>
              {sessions.map((session) => (
                <TabsTrigger key={session.id} value={session.id}>
                  <div className="flex items-center space-x-2">
                    <StatusDot status={session.status} />
                    <span>{session.name || session.id.slice(0, 8)}</span>
                  </div>
                </TabsTrigger>
              ))}
            </TabsList>
            <Button size="sm" onClick={handleNewSession}>
              + New Session
            </Button>
          </div>

          {sessions.map((session) => (
            <TabsContent
              key={session.id}
              value={session.id}
              className="h-full mt-0"
            >
              <AgentChatProvider key={session.id}>
                <AgentChat />
              </AgentChatProvider>
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </div>
  );
}

function StatusDot({ status }: { status: string }) {
  const color =
    {
      idle: 'bg-gray-400',
      busy: 'bg-yellow-500 animate-pulse',
      paused: 'bg-blue-500',
    }[status] || 'bg-gray-400';

  return <div className={`w-2 h-2 rounded-full ${color}`} />;
}
```

---

## Summary

These examples demonstrate:

1. **✅ Proper Context Usage**: Correct provider nesting and state management
2. **✅ Session Lifecycle**: Creation, resumption, and termination patterns
3. **✅ Real-time Events**: Listening and responding to Rust backend events
4. **✅ Error Handling**: Comprehensive error boundaries and recovery mechanisms
5. **✅ Tool Visualization**: Advanced UI for tool execution tracking
6. **✅ State Synchronization**: Keeping frontend and backend in sync
7. **✅ Multi-Session Support**: Managing multiple concurrent agent sessions

All patterns follow the Agent V2 architecture and integrate seamlessly with the Rust `AgentSessionManager`.
