import { useAgentChat } from '@/context/AgentChatContext';
import { Button } from '@/components/ui/button';
import {
  AlertCircle,
  Info,
  Loader2,
  Pause,
  RefreshCw,
  Wrench,
} from 'lucide-react';
import { CompactModelPicker } from '@/features/chat/ModelPicker';
import { useMCPServer } from '@/hooks/use-mcp-server';
import { useBuiltInTool } from '@/features/tools';
import { useMemo } from 'react';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentChatStatusBar');

export function AgentChatStatusBar() {
  const { workflowStatus, error, llmError, retryMessage } = useAgentChat();
  const {
    availableTools,
    isLoading: mcpLoading,
    error: mcpError,
  } = useMCPServer();
  const { availableTools: builtinAvailable } = useBuiltInTool();

  // Tool availability logic (matches ChatStatusBar)
  const { filteredBuiltin, totalBuiltin } = useMemo(() => {
    const builtinList = builtinAvailable ?? [];
    // Agent V2 doesn't filter builtin tools per assistant, show all
    return {
      filteredBuiltin: builtinList,
      totalBuiltin: builtinList.length,
    };
  }, [builtinAvailable]);

  const handleRetry = async () => {
    try {
      await retryMessage();
    } catch (err) {
      logger.error('Failed to retry message:', err);
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
    if (mcpLoading) return 'Loading tools...';
    if (mcpError) return 'Tools error';
    const mcpCount = availableTools.length;
    const totalCount = mcpCount + filteredBuiltin.length;
    const builtinSummary = totalBuiltin
      ? ` • builtin ${filteredBuiltin.length}/${totalBuiltin}`
      : '';
    return `${totalCount}(${mcpCount}) available${builtinSummary}`;
  };

  const getToolsColor = () => {
    if (mcpLoading) return 'text-yellow-400';
    if (mcpError) return 'text-red-400';
    const totalCount = availableTools.length + filteredBuiltin.length;
    return totalCount > 0 ? 'text-green-400' : 'text-gray-500';
  };

  const getToolsIcon = () => {
    if (mcpLoading) return <LoadingSpinner />;
    if (mcpError) return '⚠️';
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
        };
      case 'busy':
        return {
          icon: <Loader2 className="w-4 h-4 animate-spin" />,
          text: 'Processing your request... Agent is thinking and using tools.',
          className:
            'bg-yellow-50 dark:bg-yellow-950/20 border-yellow-200 dark:border-yellow-800 text-yellow-700 dark:text-yellow-400',
          showRetry: false,
        };
      case 'paused':
        return {
          icon: <Pause className="w-4 h-4" />,
          text: 'Workflow paused. Click Continue to resume processing.',
          className:
            'bg-blue-50 dark:bg-blue-950/20 border-blue-200 dark:border-blue-800 text-blue-700 dark:text-blue-400',
          showRetry: false,
        };
      case 'error':
        return {
          icon: <AlertCircle className="w-4 h-4" />,
          text: 'Workflow encountered an error.',
          className:
            'bg-red-50 dark:bg-red-950/20 border-red-200 dark:border-red-800 text-red-700 dark:text-red-400',
          showRetry: true,
        };
      default:
        return {
          icon: <Info className="w-4 h-4" />,
          text: `Status: ${workflowStatus}`,
          className:
            'bg-gray-50 dark:bg-gray-950/20 border-gray-200 dark:border-gray-800 text-gray-700 dark:text-gray-400',
          showRetry: false,
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
      </div>

      {/* Model and tools status bar (matches ChatStatusBar) */}
      <div className="px-4 py-2 border-t flex items-center justify-between">
        <div>
          <CompactModelPicker />
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs">Tools:</span>
          <div
            className={`text-xs flex items-center gap-1 ${getToolsColor()}`}
            title={mcpError ? mcpError : undefined}
          >
            {getToolsIcon()} {getToolsDisplayText()}
          </div>
        </div>
      </div>
    </>
  );
}
