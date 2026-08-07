/**
 * Attention-driven header badges for agent panels.
 * Shows a badge when a panel has unread updates; click opens that panel.
 * No auto-open — notification only.
 */
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  AGENT_PANEL_IDS,
  useAgentPanels,
  type AgentPanelId,
} from '@/context/AgentPanelsContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { trackBadgeClicked } from '@/lib/analytics';
import { FolderOpen, ListChecks, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';

const PANEL_ICONS: Record<
  AgentPanelId,
  typeof Terminal | typeof ListChecks | typeof FolderOpen
> = {
  processes: Terminal,
  planning: ListChecks,
  workspace: FolderOpen,
};

function panelLabelKey(id: AgentPanelId): string {
  switch (id) {
    case 'workspace':
      return 'agent.workspace.title';
    case 'planning':
      return 'agent.planning.title';
    case 'processes':
      return 'agent.processes.title';
  }
}

function panelLabelFallback(id: AgentPanelId): string {
  switch (id) {
    case 'workspace':
      return 'Workspace';
    case 'planning':
      return 'Planning';
    case 'processes':
      return 'Processes';
  }
}

export function HeaderStatusBadges() {
  const { t } = useTranslation();
  const { session } = useAgentSessionState();
  const { hasPanelAttention, isPanelOpen, openPanel } = useAgentPanels();

  const badges = AGENT_PANEL_IDS.filter((id) => hasPanelAttention(id));

  if (badges.length === 0) {
    return null;
  }

  const liveAnnouncement = badges
    .map((id) =>
      t('agent.header.panelBadgeLive', '{{panel}} has updates', {
        panel: t(panelLabelKey(id), panelLabelFallback(id)),
      }),
    )
    .join('. ');

  return (
    <div
      className="flex items-center gap-1"
      data-testid="header-status-badges"
      role="group"
      aria-label={t('agent.header.statusBadgesAria', 'Panel update badges')}
    >
      <div className="sr-only" aria-live="polite">
        {liveAnnouncement}
      </div>
      {badges.map((panelId) => {
        const Icon = PANEL_ICONS[panelId];
        const label = t(panelLabelKey(panelId), panelLabelFallback(panelId));
        const expanded = isPanelOpen(panelId);
        const ariaLabel = t(
          'agent.header.panelBadgeAria',
          '{{panel}} has updates — click to open',
          { panel: label },
        );

        return (
          <Tooltip key={panelId}>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="sm"
                data-testid={`header-status-badge-${panelId}`}
                onClick={() => {
                  trackBadgeClicked(panelId, session?.id);
                  openPanel(panelId);
                }}
                aria-label={ariaLabel}
                aria-controls="agent-side-panel-shell"
                aria-expanded={expanded}
                className="h-6 gap-1.5 border-primary/30 px-2 text-primary hover:bg-primary/10 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Icon className="h-3 w-3 shrink-0" aria-hidden="true" />
                <span className="text-xs font-medium">{label}</span>
                <span className="sr-only">
                  {t('agent.header.panelBadgeSrOnly', 'has updates')}
                </span>
              </Button>
            </TooltipTrigger>
            <TooltipContent>{ariaLabel}</TooltipContent>
          </Tooltip>
        );
      })}
    </div>
  );
}
