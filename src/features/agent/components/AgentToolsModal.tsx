import React, { useMemo } from 'react';
import { Wrench } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';
import LoadingSpinner from '@/components/ui/LoadingSpinner';
import { useAgentTools } from '@/hooks/use-agent-tools';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Button } from '@/components/ui/button';
import { parseToolName, isBuiltinTool } from '@/lib/tool-call-utils';
import { useTranslation } from 'react-i18next';

interface AgentToolsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * AgentToolsModal - Tools list modal for Agent V2
 *
 * Refactored to use accessible Dialog component and semantic list structure.
 */
export const AgentToolsModal: React.FC<AgentToolsModalProps> = ({
  isOpen,
  onClose,
}) => {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();

  const { availableTools, isLoading, error } = useAgentTools(session?.id);

  const { builtinTools, mcpTools } = useMemo(() => {
    const builtin = availableTools.filter((t) => isBuiltinTool(t.name));
    const mcp = availableTools.filter((t) => !isBuiltinTool(t.name));
    return { builtinTools: builtin, mcpTools: mcp };
  }, [availableTools]);

  const totalCount = availableTools.length;
  const mcpCount = mcpTools.length;
  const builtinCount = builtinTools.length;

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-w-2xl max-h-[80vh] overflow-hidden flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {t('agent.toolsModal.title')}{' '}
            {totalCount > 0 && (
              <span className="text-muted-foreground font-normal text-sm ml-1">
                ({totalCount})
              </span>
            )}
          </DialogTitle>
          <DialogDescription className="text-sm text-muted-foreground mt-1 text-left font-normal">
            {builtinCount > 0
              ? t('agent.toolsModal.subtitleWithCounts', { builtinCount, mcpCount })
              : t('agent.toolsModal.subtitleDefault')}
          </DialogDescription>
        </DialogHeader>

        {/* Loading State */}
        {isLoading && (
          <div className="flex flex-col items-center justify-center py-8 text-muted-foreground gap-2">
            <LoadingSpinner />
            <span>{t('agent.toolsModal.loading')}</span>
          </div>
        )}

        {/* Error State */}
        {error && (
          <div className="text-center py-8 text-destructive flex flex-col items-center gap-2">
            <span className="font-semibold">{t('agent.toolsModal.errorTitle')}</span>
            <span className="text-sm opacity-90">{error}</span>
          </div>
        )}

        {/* Tools List */}
        {!isLoading && !error && (
          <div className="overflow-y-auto flex-1 min-h-0 pr-2">
            {totalCount === 0 ? (
              <div className="text-foreground text-center py-8">
                {t('agent.toolsModal.empty')}
              </div>
            ) : (
              <ul className="space-y-3" aria-label={t('agent.toolsModal.ariaLabel')}>
                {availableTools.map((tool) => (
                  <li
                    key={tool.name}
                    className="bg-muted border border-border rounded p-3"
                  >
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center gap-2">
                        <Wrench
                          size={16}
                          className="flex-shrink-0 text-muted-foreground"
                        />
                        <span
                          className="font-mono text-sm text-foreground break-words font-medium"
                          title={parseToolName(tool.name)}
                        >
                          {parseToolName(tool.name)}
                        </span>
                        <span
                          className={
                            isBuiltinTool(tool.name)
                              ? 'text-xs uppercase font-bold bg-success/20 text-success px-1.5 py-0.5 rounded-full'
                              : 'text-xs uppercase font-bold bg-primary/20 text-primary px-1.5 py-0.5 rounded-full'
                          }
                          aria-hidden
                        >
                          {isBuiltinTool(tool.name) ? t('agent.toolsModal.badgeBuiltin') : t('agent.toolsModal.badgeMcp')}
                        </span>
                      </div>
                    </div>
                    {tool.description && (
                      <p className="text-muted-foreground text-sm mb-2">
                        {tool.description}
                      </p>
                    )}
                    {tool.inputSchema && (
                      <details className="group">
                        <summary className="text-xs text-primary cursor-pointer hover:underline focus-visible:ring-2 rounded px-1 -ml-1 inline-flex items-center select-none">
                          {t('agent.toolsModal.viewSchema')}
                        </summary>
                        <pre className="text-xs text-foreground mt-2 bg-background p-3 rounded border border-border overflow-x-auto">
                          {JSON.stringify(tool.inputSchema, null, 2)}
                        </pre>
                      </details>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        <div className="mt-4 pt-4 border-t border-border flex justify-end">
          <Button variant="secondary" onClick={onClose}>
            {t('common.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
};

export default AgentToolsModal;
