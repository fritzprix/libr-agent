import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { useSessionAgentTools } from '@/features/agent/hooks/useSessionAgentTools';
import { isBuiltinTool } from '@/lib/tool-call-utils';
import { cn } from '@/lib/utils';
import { AlertTriangle, Wrench } from 'lucide-react';
import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import AgentToolsModal from '../AgentToolsModal';

export function StatusBarToolsControl() {
  const { t } = useTranslation();
  const [showToolsModal, setShowToolsModal] = useState(false);

  // Single Source of Truth: Fetch filtered tools from Rust backend
  const {
    availableTools,
    isLoading: toolsLoading,
    error: toolsError,
  } = useSessionAgentTools();

  // Categorize tools by type
  const { builtinTools, externalTools } = useMemo(() => {
    const builtin = availableTools.filter((tool) => isBuiltinTool(tool.name));
    const external = availableTools.filter((tool) => !isBuiltinTool(tool.name));
    return { builtinTools: builtin, externalTools: external };
  }, [availableTools]);

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

  return (
    <>
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
                type="button"
                onClick={() => setShowToolsModal(true)}
                className={cn(
                  'text-xs flex items-center gap-1 cursor-pointer rounded-sm transition-colors hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none',
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

      <AgentToolsModal
        isOpen={showToolsModal}
        onClose={() => setShowToolsModal(false)}
      />
    </>
  );
}
