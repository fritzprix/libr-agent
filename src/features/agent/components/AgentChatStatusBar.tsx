import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Button } from '@/components/ui/button';
import {
  AlertCircle,
  Info,
  Loader2,
  Pause,
  Play,
  RefreshCw,
  Wrench,
  AlertTriangle,
} from 'lucide-react';
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useEffect, useMemo, useState } from 'react';
import { getLogger } from '@/lib/logger';
import AgentToolsModal from './AgentToolsModal';
import { useTokenMetrics } from '@/hooks/use-token-metrics';
import { TokenMetricsBadge } from './TokenMetricsBadge';
import { TokenUsage } from '@/lib/ai-service/types';

const logger = getLogger('AgentChatStatusBar');

export function AgentChatStatusBar() {
  const { session } = useAgentSessionState();
  const { workflowStatus, error, llmError, retryMessage, resume } =
    useAgentChat();
  const [showToolsModal, setShowToolsModal] = useState(false);

  // ✅ Fetch real-time token metrics
  const { metrics } = useTokenMetrics(session?.id);

  // Persist last metrics to show after streaming ends
  const [lastMetrics, setLastMetrics] = useState<TokenUsage | null>(null);

  // Reset last metrics when session changes
  useEffect(() => {
    setLastMetrics(null);
  }, [session?.id]);

  // Update last metrics when active metrics are available
  useEffect(() => {
    if (metrics) {
      setLastMetrics(metrics);
    }
  }, [metrics]);

  const displayMetrics = metrics || lastMetrics;

  // ✅ Single Source of Truth: Fetch filtered tools from Rust backend
  const {
    availableTools,
    isLoading: toolsLoading,
    error: toolsError,
  } = useAgentTools(session?.id);

  // Categorize tools by type
  const { builtinTools, externalTools } = useMemo(() => {
    const builtin = availableTools.filter((t) => t.name.startsWith('builtin_'));
    const external = availableTools.filter(
      (t) => !t.name.startsWith('builtin_'),
    );
    return { builtinTools: builtin, externalTools: external };
  }, [availableTools]);

  const handleRetry = async () => {
    try {
      await retryMessage();
    } catch (err) {
      logger.error('Failed to retry message:', err);
    }
  };

  const handleResume = async () => {
    try {
      await resume();
    } catch (err) {
      logger.error('Failed to resume session:', err);
    }
  };

  const LoadingSpinner = () => (
    <svg
      className="animate-spin h-3 w-3 text-yellow-400"
      xmlns="http://www.w3.org/2000/svg"
      fill="none"
      viewBox="0 0 24 24"
    >
      <circle
        className="opacity-25"
        cx="12"
        cy="12"
        r="10"
        stroke="currentColor"
        strokeWidth="4"
      />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );

  const getToolsDisplayText = () => {
    if (toolsLoading) return 'Loading tools...';
    if (toolsError) return 'Tools error';

    const totalCount = availableTools.length;
    const mcpCount = externalTools.length;
    const builtinCount = builtinTools.length;

    return `${totalCount}(${mcpCount}) available • builtin ${builtinCount}`;
  };

  const getToolsColor = () => {
    if (toolsLoading) return 'text-yellow-400';
    if (toolsError) return 'text-red-400';
    return availableTools.length > 0 ? 'text-green-400' : 'text-gray-500';
  };

  const getToolsIcon = () => {
    if (toolsLoading) return <LoadingSpinner />;
    if (toolsError) return <AlertTriangle className="w-3.5 h-3.5" />;
    return <Wrench size={14} />;
  };

  const getStatusConfig = () => {
    if (error || llmError) {
      return {
        icon: <AlertCircle className="w-4 h-4" />,
        text: `An error occurred: ${error || llmError}`,
        className:
          'bg-red-50 dark:bg-red-950/20 border-red-200 dark:border-red-800 text-red-700 dark:text-red-400',
        showRetry: true,
        showResume: false,
      };
    }

    switch (workflowStatus) {
      case 'idle':
        return {
          icon: <Info className="w-4 h-4" />,
          text: 'Ready for input. Type a message to start.',
          className:
            'bg-blue-50 dark:bg-blue-950/20 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-400',
          showRetry: false,
          showResume: false,
        };
      case 'busy':
        return {
          icon: <Loader2 className="w-4 h-4 animate-spin" />,
          text: 'Processing your request... Agent is thinking and using tools.',
          className:
            'bg-yellow-50 dark:bg-yellow-950/20 border-yellow-200 dark:border-yellow-800 text-yellow-700 dark:text-yellow-400',
          showRetry: false,
          showResume: false,
        };
      case 'paused':
        return {
          icon: <Pause className="w-4 h-4" />,
          text: 'Workflow paused. Click Continue to resume processing.',
          className:
            'bg-blue-50 dark:bg-blue-950/20 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-400',
          showRetry: false,
          showResume: true,
        };
      case 'error':
        return {
          icon: <AlertCircle className="w-4 h-4" />,
          text: 'Workflow encountered an error.',
          className:
            'bg-red-50 dark:bg-red-950/20 border-red-200 dark:border-red-800 text-red-700 dark:text-red-400',
          showRetry: true,
          showResume: false,
        };
      default:
        return {
          icon: <Info className="w-4 h-4" />,
          text: `Status: ${workflowStatus}`,
          className:
            'bg-gray-50 dark:bg-gray-950/20 border-gray-200 dark:border-gray-800 text-gray-700 dark:text-gray-400',
          showRetry: false,
          showResume: false,
        };
    }
  };

  const config = getStatusConfig();

  return (
    <>
      {/* Workflow status bar (top) */}
      <div
        className={`px-4 py-2 border-b flex items-center justify-between ${config.className}`}
      >
        <div className="flex items-center gap-2">
          {config.icon}
          <span className="text-sm">{config.text}</span>
        </div>
        {config.showRetry && (
          <Button
            size="sm"
            variant="outline"
            onClick={handleRetry}
            className="h-7"
          >
            <RefreshCw className="w-3 h-3 mr-1" />
            Retry
          </Button>
        )}
        {config.showResume && (
          <Button
            size="sm"
            variant="outline"
            onClick={handleResume}
            className="h-7"
          >
            <Play className="w-3 h-3 mr-1" />
            Continue
          </Button>
        )}
      </div>

      {/* Model and tools status bar (matches ChatStatusBar) */}
      <div className="px-4 py-2 border-t flex items-center justify-between">
        <div>
          {session && (
            <AgentModelPicker
              currentModel={session.assistant?.model}
              currentProvider={session.assistant?.provider}
              onConfigUpdate={async (model, provider) => {
                if (!session.id || !session.assistant) return;

                // Optimistic update logging
                logger.info(`Updating session config to ${provider}/${model}`);

                try {
                  const updatedConfig = {
                    ...session.assistant,
                    provider,
                    model,
                    // Ensure required fields
                    temperature: session.assistant.temperature ?? 0.7,
                    name: session.assistant.name || 'Assistant',
                    systemPrompt:
                      session.assistant.systemPrompt ||
                      'You are a helpful assistant.',
                  };

                  // Dynamically import invoke to avoid circular dependencies if any (though invoke is from tauri-apps)
                  const { invoke } = await import('@tauri-apps/api/core');

                  await invoke('agent_update_session_config', {
                    request: {
                      sessionId: session.id,
                      agentConfig: updatedConfig,
                    },
                  });
                } catch (e) {
                  logger.error('Failed to update session config', e);
                }
              }}
            />
          )}
        </div>
        <div className="flex items-center gap-4">
          {/* Token Metrics Badge - Show if metrics exist */}
          {displayMetrics && (
            <div className="hidden md:block">
              <TokenMetricsBadge usage={displayMetrics} />
            </div>
          )}

          <div className="flex items-center gap-2">
            <span className="text-xs">Tools:</span>
            <button
              onClick={() => setShowToolsModal(true)}
              className={`text-xs flex items-center gap-1 cursor-pointer hover:underline transition-colors ${getToolsColor()}`}
              title={toolsError ? toolsError : 'Click to view available tools'}
              disabled={toolsLoading}
            >
              {getToolsIcon()} {getToolsDisplayText()}
            </button>
          </div>
        </div>
      </div>

      {/* Tools Modal */}
      <AgentToolsModal
        isOpen={showToolsModal}
        onClose={() => setShowToolsModal(false)}
      />
    </>
  );
}
