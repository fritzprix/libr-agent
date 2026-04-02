import type { AgentResponse } from '@/models/agent-ipc';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSession } from '@/context/AgentSessionContext';
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
  Zap,
  DatabaseZap,
} from 'lucide-react';
import { AgentModelPicker } from '@/features/agent/components/AgentModelPicker';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useLLMService } from '@/context/LLMServiceContext';
import { useMemo, useState, useEffect } from 'react';
import { getLogger } from '@/lib/logger';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import AgentToolsModal from './AgentToolsModal';
import { useTokenMetrics } from '@/hooks/use-token-metrics';
import { TokenMetricsBadge } from './TokenMetricsBadge';
import { TokenUsage } from '@/lib/ai-service/types';
import { toast } from 'sonner';
import { isBuiltinTool } from '@/lib/tool-call-utils';
import { useTranslation } from 'react-i18next';
import { mergeDisplayTokenUsage } from './token-metrics';
import { cn } from '@/lib/utils';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import { useSettings } from '@/context/SettingsContext';

const logger = getLogger('AgentChatStatusBar');

export function AgentChatStatusBar() {
  const { t } = useTranslation();
  const { value: settings } = useSettings();
  const { session, yoloModeEnabled, toggleYoloMode, updateSessionConfig } =
    useAgentSession();
  const { workflowStatus, error, llmError, retryMessage, resume } =
    useAgentChat();
  const { isCompacting, isAwaitingCompact, getContextUsage } = useLLMService();
  const isCompactStrategy = settings.contextStrategy === 'compact';
  // Rust is the source of truth for compact-strategy context occupancy.
  // The badge shows provider-reported usage, which may exclude cached prefixes.
  const usageInfo =
    isCompactStrategy && session?.id ? getContextUsage(session.id) : undefined;
  const contextWindow = usageInfo?.contextWindow;
  const modelMaxContext = usageInfo?.modelMaxContext;
  const [showToolsModal, setShowToolsModal] = useState(false);

  // ✅ Fetch real-time token metrics
  const { metrics } = useTokenMetrics(session?.id);

  // Persist last metrics to show after streaming ends
  const [lastMetrics, setLastMetrics] = useState<TokenUsage | null>(null);
  const [prevSessionId, setPrevSessionId] = useState<string | undefined>(
    session?.id,
  );

  // Adjusting State During Render: Reset metrics when session changes
  if (session?.id !== prevSessionId) {
    setPrevSessionId(session?.id);
    setLastMetrics(null);
  }

  useEffect(() => {
    // Update last metrics only when we have meaningful new data
    if (metrics) {
      const hasData =
        metrics.promptTokens > 0 ||
        metrics.completionTokens > 0 ||
        (metrics.cachedPromptTokens ?? 0) > 0;

      if (hasData) {
        setLastMetrics((prev) => {
          if (!prev) return metrics;
          // Smart merge: update counts but preserve metadata if new chunk lacks it
          return {
            ...prev,
            ...metrics,
            details: {
              ...prev.details,
              ...metrics.details,
              // Only overwrite metadata if new value is present and non-zero
              evalDuration:
                metrics.details?.evalDuration || prev.details?.evalDuration,
              timeToFirstToken:
                metrics.details?.timeToFirstToken ||
                prev.details?.timeToFirstToken,
              promptEvalDuration:
                metrics.details?.promptEvalDuration ||
                prev.details?.promptEvalDuration,
              loadDuration:
                metrics.details?.loadDuration || prev.details?.loadDuration,
            },
          };
        });
      }
    }
  }, [metrics]);

  // Derive displayMetrics during render to ensure UI reflects the absolute latest chunk
  // without waiting for the next paint cycle (Effect-less derivation)
  const displayMetrics = useMemo(
    () => mergeDisplayTokenUsage(lastMetrics, metrics),
    [metrics, lastMetrics],
  );

  // The context gauge intentionally uses Rust-estimated total context occupancy,
  // not provider-reported promptTokens shown in the badge.
  const contextUsage =
    isCompactStrategy && usageInfo && contextWindow
      ? {
          totalTokens: usageInfo.totalTokens,
          contextWindow,
          modelMaxContext,
        }
      : undefined;
  // ✅ Single Source of Truth: Fetch filtered tools from Rust backend
  const {
    availableTools,
    isLoading: toolsLoading,
    error: toolsError,
  } = useAgentTools(session?.id);

  // Categorize tools by type
  const { builtinTools, externalTools } = useMemo(() => {
    const builtin = availableTools.filter((t) => isBuiltinTool(t.name));
    const external = availableTools.filter((t) => !isBuiltinTool(t.name));
    return { builtinTools: builtin, externalTools: external };
  }, [availableTools]);

  const [isRetrying, setIsRetrying] = useState(false);
  const [isResuming, setIsResuming] = useState(false);

  const handleRetry = async () => {
    if (isRetrying) return;
    setIsRetrying(true);
    try {
      await retryMessage();
    } catch (err) {
      logger.error('Failed to retry message:', err);
      toast.error(t('agent.statusBar.retryError'));
    } finally {
      setIsRetrying(false);
    }
  };

  const handleResume = async () => {
    if (isResuming) return;
    setIsResuming(true);
    try {
      await resume();
    } catch (err) {
      logger.error('Failed to resume session:', err);
      toast.error(t('agent.statusBar.resumeError'));
    } finally {
      setIsResuming(false);
    }
  };

  const getToolsDisplayText = () => {
    if (toolsLoading) return t('agent.statusBar.loadingTools');
    if (toolsError) return t('agent.statusBar.toolsError');

    const totalCount = availableTools.length;
    const mcpCount = externalTools.length;
    const builtinCount = builtinTools.length;

    return t('agent.statusBar.toolsCount', {
      totalCount,
      mcpCount,
      builtinCount,
    });
  };

  const getToolsColor = () => {
    if (toolsLoading) return 'text-warning';
    if (toolsError) return 'text-destructive';
    return availableTools.length > 0 ? 'text-success' : 'text-muted-foreground';
  };

  const getToolsIcon = () => {
    if (toolsLoading) return <LoadingSpinner className="w-3 h-3" />;
    if (toolsError) return <AlertTriangle className="w-3.5 h-3.5" />;
    return <Wrench size={14} />;
  };

  const getStatusConfig = () => {
    if (error || llmError) {
      const activeError = error ?? llmError;
      return {
        icon: <AlertCircle className="w-4 h-4" />,
        text: t('agent.statusBar.statusError', {
          error: activeError?.displayMessage ?? '',
        }),
        className: 'bg-destructive/10 border-destructive/20 text-destructive',
        showRetry: true,
        showResume: false,
      };
    }

    // Compact-specific states take priority over generic 'busy'
    if (session?.id && isAwaitingCompact(session.id)) {
      return {
        icon: <Loader2 className="w-4 h-4 animate-spin" />,
        text: t('agent.statusBar.statusAwaitingCompact'),
        className:
          'bg-blue-500/10 border-blue-500/20 text-blue-700 dark:text-blue-400',
        showRetry: false,
        showResume: false,
      };
    }

    if (session?.id && isCompacting(session.id)) {
      return {
        icon: (
          <DatabaseZap className="w-4 h-4 animate-pulse text-muted-foreground" />
        ),
        text: t('agent.statusBar.statusCompacting'),
        className: 'bg-secondary/50 border-border text-muted-foreground',
        showRetry: false,
        showResume: false,
      };
    }

    switch (workflowStatus) {
      case 'idle':
        return {
          icon: <Info className="w-4 h-4" />,
          text: t('agent.statusBar.statusIdle'),
          className: 'bg-secondary/50 border-border text-foreground',
          showRetry: false,
          showResume: false,
        };
      case 'busy':
        return {
          icon: <Loader2 className="w-4 h-4 animate-spin" />,
          text: t('agent.statusBar.statusBusy'),
          className: 'bg-warning/10 border-warning/20 text-warning-foreground',
          showRetry: false,
          showResume: false,
        };
      case 'paused':
        return {
          icon: <Pause className="w-4 h-4" />,
          text: t('agent.statusBar.statusPaused'),
          className: 'bg-secondary/50 border-border text-foreground',
          showRetry: false,
          showResume: true,
        };
      case 'error':
        return {
          icon: <AlertCircle className="w-4 h-4" />,
          text: t('agent.statusBar.statusWorkflowError'),
          className: 'bg-destructive/10 border-destructive/20 text-destructive',
          showRetry: true,
          showResume: false,
        };
      default:
        return {
          icon: <Info className="w-4 h-4" />,
          text: t('agent.statusBar.statusUnknown', { status: workflowStatus }),
          className: 'bg-muted/50 border-border text-muted-foreground',
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
            disabled={isRetrying}
          >
            {isRetrying ? (
              <LoadingSpinner size="sm" className="mr-1" />
            ) : (
              <RefreshCw className="w-3 h-3 mr-1" />
            )}
            {isRetrying
              ? t('agent.statusBar.retrying')
              : t('agent.statusBar.retry')}
          </Button>
        )}
        {config.showResume && (
          <Button
            size="sm"
            variant="outline"
            onClick={handleResume}
            className="h-7"
            disabled={isResuming}
          >
            {isResuming ? (
              <LoadingSpinner size="sm" className="mr-1" />
            ) : (
              <Play className="w-3 h-3 mr-1" />
            )}
            {isResuming
              ? t('agent.statusBar.resuming')
              : t('agent.statusBar.continue')}
          </Button>
        )}
      </div>

      {/* Model and tools status bar (matches ChatStatusBar) */}
      <div className="px-4 py-2 border-t flex items-center justify-between">
        <div>
          {session && (
            <AgentModelPicker
              currentModel={session.model}
              currentProvider={session.provider}
              disabled={workflowStatus !== 'idle'}
              onConfigUpdate={async (model, provider) => {
                if (
                  !session.id ||
                  !session.assistant ||
                  workflowStatus !== 'idle'
                )
                  return;

                // Session config update logging
                logger.info(`Updating session config to ${provider}/${model}`);

                try {
                  const { enforceRuntimeBuiltinAliases } = await import(
                    '@/lib/assistant/runtime-builtins'
                  );

                  const updatedConfig = {
                    ...session.assistant,
                    allowedBuiltInServiceAliases: enforceRuntimeBuiltinAliases(
                      session.assistant.allowedBuiltInServiceAliases,
                    ),
                    // Note: We keep these for completeness but the backend will prioritize top-level model/provider
                    name: session.assistant.name || 'Assistant',
                    systemPrompt:
                      session.assistant.systemPrompt ||
                      'You are a helpful assistant.',
                  };

                  // Dynamically import safeInvoke to avoid circular dependencies if any (though it ultimately wraps Tauri invoke)
                  const { safeInvoke } = await import('@/lib/backend/core');

                  await safeInvoke<AgentResponse>(
                    'agent_update_session_config',
                    {
                      request: {
                        sessionId: session.id,
                        model,
                        provider,
                        agentConfig: updatedConfig,
                      },
                    },
                  );

                  // Update local session state
                  updateSessionConfig(model, provider);
                } catch (e) {
                  logger.error('Failed to update session config', e);
                }
              }}
            />
          )}
        </div>
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="sm"
            onClick={toggleYoloMode}
            className={`h-6 px-2 text-xs flex items-center gap-1 ${
              yoloModeEnabled
                ? 'text-primary bg-primary/10 hover:bg-primary/20'
                : 'text-muted-foreground hover:bg-muted'
            }`}
            title={
              yoloModeEnabled
                ? t(
                    'agent.statusBar.yoloModeOnTitle',
                    'YOLO Mode is ON. Tools will execute without asking for approval.',
                  )
                : t(
                    'agent.statusBar.yoloModeOffTitle',
                    'YOLO Mode is OFF. Sensitive tools require approval.',
                  )
            }
          >
            <Zap
              size={14}
              className={yoloModeEnabled ? 'fill-primary text-primary' : ''}
            />
            YOLO Mode
          </Button>

          {/* Token Metrics Badge - Show if metrics exist */}
          {displayMetrics && (
            <div className="hidden md:block">
              <TokenMetricsBadge
                usage={displayMetrics}
                contextUsage={contextUsage}
              />
            </div>
          )}

          <div className="flex items-center gap-2">
            <span className="text-xs">{t('agent.statusBar.toolsLabel')}</span>
            <Tooltip>
              <TooltipTrigger asChild>
                <span
                  className={cn(
                    'inline-block',
                    toolsLoading && 'cursor-not-allowed',
                  )}
                >
                  <button
                    onClick={() => setShowToolsModal(true)}
                    className={cn(
                      'text-xs flex items-center gap-1 cursor-pointer hover:underline transition-colors rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
                      getToolsColor(),
                      toolsLoading && 'opacity-50',
                    )}
                    disabled={toolsLoading}
                  >
                    {getToolsIcon()} {getToolsDisplayText()}
                  </button>
                </span>
              </TooltipTrigger>
              <TooltipContent>
                {toolsError ? toolsError : t('agent.statusBar.viewToolsTitle')}
              </TooltipContent>
            </Tooltip>
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
