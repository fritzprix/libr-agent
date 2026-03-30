import React, { useMemo } from 'react';
import { Blocks, Braces, Wrench } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from '@/components/ui/dialog';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui';
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
      <DialogContent className="grid max-h-[80vh] max-w-3xl grid-rows-[auto_minmax(0,1fr)_auto] gap-0 overflow-hidden border border-border/50 bg-background p-0 shadow-[0_28px_80px_-36px_rgba(0,0,0,0.45)]">
        <DialogHeader className="border-b border-border/40 px-6 py-5 text-left">
          <div className="space-y-4">
            <div className="flex items-center justify-between gap-3">
              <DialogTitle className="flex items-center gap-2 text-base font-semibold text-foreground">
                <div className="flex h-8 w-8 items-center justify-center rounded-full border border-border/50 bg-muted/[0.24] text-muted-foreground">
                  <Wrench className="h-4 w-4" />
                </div>
                <div className="flex items-baseline gap-2">
                  <span>{t('agent.toolsModal.title')}</span>
                  {totalCount > 0 && (
                    <span className="text-sm font-normal text-muted-foreground">
                      {totalCount}
                    </span>
                  )}
                </div>
              </DialogTitle>
            </div>

            <DialogDescription className="text-sm font-normal text-muted-foreground">
              {builtinCount > 0
                ? t('agent.toolsModal.subtitleWithCounts', {
                    builtinCount,
                    mcpCount,
                  })
                : t('agent.toolsModal.subtitleDefault')}
            </DialogDescription>

            <div className="flex flex-wrap gap-2">
              <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
                {totalCount} {t('agent.toolsModal.title')}
              </div>
              <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
                {builtinCount} {t('agent.toolsModal.badgeBuiltin')}
              </div>
              <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
                {mcpCount} {t('agent.toolsModal.badgeMcp')}
              </div>
            </div>
          </div>
        </DialogHeader>

        {/* Loading State */}
        {isLoading && (
          <div className="flex min-h-[320px] flex-col items-center justify-center gap-3 px-6 py-10 text-muted-foreground">
            <LoadingSpinner />
            <span className="text-sm">{t('agent.toolsModal.loading')}</span>
          </div>
        )}

        {/* Error State */}
        {error && (
          <div className="flex min-h-[320px] flex-col items-center justify-center gap-2 px-6 py-10 text-center">
            <div className="rounded-full border border-destructive/20 bg-destructive/10 p-3 text-destructive">
              <Braces className="h-4 w-4" />
            </div>
            <span className="font-semibold text-destructive">
              {t('agent.toolsModal.errorTitle')}
            </span>
            <span className="max-w-lg text-sm text-muted-foreground">{error}</span>
          </div>
        )}

        {/* Tools List */}
        {!isLoading && !error && (
          <div className="min-h-0 overflow-hidden">
            <ScrollArea className="h-full">
              <div className="px-6 py-5">
              {totalCount === 0 ? (
                <div className="flex min-h-[260px] flex-col items-center justify-center gap-3 rounded-xl border border-border/40 bg-muted/[0.16] px-6 py-10 text-center">
                  <div className="rounded-full border border-border/50 bg-background/80 p-3 text-muted-foreground">
                    <Blocks className="h-4 w-4" />
                  </div>
                  <div className="text-sm text-muted-foreground">
                    {t('agent.toolsModal.empty')}
                  </div>
                </div>
              ) : (
                <ul
                  className="space-y-3"
                  aria-label={t('agent.toolsModal.ariaLabel')}
                >
                  {availableTools.map((tool) => (
                    <li
                      key={tool.name}
                      className="overflow-hidden rounded-xl border border-border/40 bg-muted/[0.16]"
                    >
                      <div className="space-y-3 px-4 py-4">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 flex-1 space-y-1.5">
                            <div className="flex items-center gap-2">
                              <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-border/40 bg-background/80 text-muted-foreground">
                                <Wrench className="h-3.5 w-3.5" />
                              </div>
                              <span
                                className="break-words font-mono text-sm font-medium text-foreground"
                                title={parseToolName(tool.name)}
                              >
                                {parseToolName(tool.name)}
                              </span>
                            </div>
                            {tool.description && (
                              <p className="text-sm leading-6 text-muted-foreground">
                                {tool.description}
                              </p>
                            )}
                          </div>

                          <Badge
                            variant="outline"
                            className={
                              isBuiltinTool(tool.name)
                                ? 'shrink-0 border-border/40 bg-background/80 text-[10px] uppercase tracking-wide text-muted-foreground'
                                : 'shrink-0 border-border/40 bg-background/80 text-[10px] uppercase tracking-wide text-muted-foreground'
                            }
                            aria-hidden
                          >
                            {isBuiltinTool(tool.name)
                              ? t('agent.toolsModal.badgeBuiltin')
                              : t('agent.toolsModal.badgeMcp')}
                          </Badge>
                        </div>

                        {tool.inputSchema && (
                          <details className="group rounded-lg border border-border/35 bg-background/70">
                            <summary className="cursor-pointer list-none px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground">
                              <span className="inline-flex items-center gap-2">
                                <Braces className="h-3.5 w-3.5" />
                                {t('agent.toolsModal.viewSchema')}
                              </span>
                            </summary>
                            <div className="border-t border-border/35 px-3 py-3">
                              <pre className="overflow-x-auto rounded-md bg-muted/[0.35] p-3 text-xs leading-5 text-foreground">
                                {JSON.stringify(tool.inputSchema, null, 2)}
                              </pre>
                            </div>
                          </details>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              )}
              </div>
            </ScrollArea>
          </div>
        )}

        <div className="flex shrink-0 justify-end border-t border-border/40 px-6 py-4">
          <Button variant="secondary" onClick={onClose}>
            {t('common.close')}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
};

export default AgentToolsModal;
