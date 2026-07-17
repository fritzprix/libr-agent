import { useEffect, useMemo, useRef } from 'react';
import { PanelRight } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useAgentChat } from '@/context/AgentChatContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { ScrollArea } from '@/components/ui/scroll-area';
import { getLogger } from '@/lib/logger';
import { parseScratchpadState } from '@/models/scratchpad';
import { cn } from '@/lib/utils';
import { SessionSchedulesSection } from './SessionSchedulesSection';

const logger = getLogger('AgentScratchpadPanel');

interface AgentScratchpadPanelProps {
  isVisible?: boolean;
  variant?: 'rail' | 'sheet';
}

export function AgentScratchpadPanel({
  isVisible = true,
  variant = 'rail',
}: AgentScratchpadPanelProps) {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { serviceContexts, updateServiceContexts } = useAgentChat();
  const wasVisibleRef = useRef(false);

  useEffect(() => {
    if (isVisible && !wasVisibleRef.current) {
      logger.info('AGENT_SCRATCHPAD_PANEL: Panel became visible');
      void updateServiceContexts();
    }

    wasVisibleRef.current = isVisible;
  }, [isVisible, updateServiceContexts]);

  const scratchpadState = useMemo(
    () => parseScratchpadState(serviceContexts.scratchpad?.structuredState),
    [serviceContexts.scratchpad?.structuredState],
  );

  const scratchpadCount = scratchpadState?.items.length ?? 0;

  if (!session) return null;

  return (
    <Card
      id="agent-scratchpad-panel"
      className={cn(
        'h-full overflow-hidden rounded-none bg-background py-0 shadow-none gap-0',
        variant === 'rail'
          ? 'w-80 flex-shrink-0 border-y-0 border-r-0 border-l border-border/40'
          : 'w-full border-0',
      )}
    >
      <CardHeader className="border-b border-border/40 px-4 py-3">
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
            <PanelRight className="h-3.5 w-3.5" />
            <span>{t('agent.schedules.title', 'Schedules')}</span>
          </div>

          <div className="flex flex-wrap gap-2">
            <div className="rounded-full border border-border/50 bg-background/70 px-2.5 py-1 text-[11px] text-muted-foreground">
              {scratchpadCount}{' '}
              {t('agent.schedules.scratchpadCountLabel', 'Scratchpad Notes')}
            </div>
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-5 overflow-hidden px-4 py-4">
        {/* Scratchpad Section */}
        <section className="flex min-h-0 flex-1 flex-col space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
              {t('agent.schedules.scratchpad', 'Scratchpad')}
            </h4>
            <span className="text-[11px] text-muted-foreground">
              {scratchpadCount}
            </span>
          </div>
          <div className="min-h-0 flex-1 overflow-hidden rounded-lg border border-border/40 bg-muted/[0.18]">
            <ScrollArea className="h-full">
              {scratchpadState?.items && scratchpadState.items.length > 0 ? (
                <div>
                  {scratchpadState.items.map((item) => (
                    <div
                      key={item.id}
                      className="border-b border-border/25 px-3 py-3 last:border-b-0"
                    >
                      <div className="flex items-center justify-between gap-2">
                        <div className="truncate text-sm font-medium text-foreground">
                          {item.title ||
                            t('agent.schedules.untitledNote', 'Untitled note')}
                        </div>
                        <Badge
                          variant="outline"
                          className="shrink-0 border-border/40 bg-background/80 text-[10px] text-muted-foreground"
                        >
                          #{item.id}
                        </Badge>
                      </div>
                      <p className="mt-1.5 whitespace-pre-wrap break-words text-xs leading-5 text-muted-foreground">
                        {item.content}
                      </p>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="px-3 py-4 text-sm text-muted-foreground">
                  {t('agent.schedules.noScratchpad', 'No scratchpad notes')}
                </div>
              )}
            </ScrollArea>
          </div>
        </section>

        {/* Schedules Section */}
        <SessionSchedulesSection sessionId={session.id} isVisible={isVisible} />
      </CardContent>
    </Card>
  );
}
