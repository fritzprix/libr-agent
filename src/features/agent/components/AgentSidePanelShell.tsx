import { Card } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  useAgentPanels,
  type AgentPanelId,
} from '@/context/AgentPanelsContext';
import { cn } from '@/lib/utils';
import { FolderOpen, ListChecks, Terminal } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { AgentPlanningPanel } from './AgentPlanningPanel';
import { AgentProcessPanel } from './AgentProcessPanel';
import { AgentWorkspacePanel } from './AgentWorkspacePanel';
import { PanelAttentionDot } from './PanelAttentionDot';

interface AgentSidePanelShellProps {
  isVisible?: boolean;
  variant?: 'rail' | 'sheet';
}

const TAB_ORDER: AgentPanelId[] = ['workspace', 'processes', 'planning'];

/**
 * Single right-side shell hosting Files / Processes / Planning as tabs.
 */
export function AgentSidePanelShell({
  isVisible = true,
  variant = 'rail',
}: AgentSidePanelShellProps) {
  const { t } = useTranslation();
  const { activeTab, setActiveTab, hasPanelAttention, isPanelOpen } =
    useAgentPanels();

  return (
    <Card
      id="agent-side-panel-shell"
      role="region"
      aria-label={t('agent.panels.shellTitle', 'Agent panels')}
      aria-hidden={!isVisible}
      className={cn(
        'flex h-full w-full flex-col overflow-hidden rounded-none bg-background py-0 shadow-none gap-0',
        variant === 'rail'
          ? 'border-y-0 border-r-0 border-l border-border/40'
          : 'border-0',
        !isVisible && 'pointer-events-none',
      )}
    >
      <Tabs
        value={activeTab}
        onValueChange={(value) => {
          setActiveTab(value as AgentPanelId);
        }}
        className="flex min-h-0 flex-1 flex-col gap-0"
      >
        <div className="shrink-0 border-b border-border/40 px-3 py-2">
          <TabsList className="grid h-9 w-full grid-cols-3">
            {TAB_ORDER.map((tabId) => {
              const attention = hasPanelAttention(tabId);
              const label =
                tabId === 'workspace'
                  ? t('agent.workspace.title', 'Workspace')
                  : tabId === 'processes'
                    ? t('agent.processes.title', 'Processes')
                    : t('agent.planning.title', 'Planning');
              const Icon =
                tabId === 'workspace'
                  ? FolderOpen
                  : tabId === 'processes'
                    ? Terminal
                    : ListChecks;

              return (
                <TabsTrigger
                  key={tabId}
                  value={tabId}
                  className="relative gap-1.5 px-2 text-xs"
                  aria-label={
                    attention
                      ? t('agent.panels.tabHasUpdatesAria', {
                          tab: label,
                          defaultValue: '{{tab}} (has updates)',
                        })
                      : label
                  }
                >
                  <Icon className="size-3.5 shrink-0" />
                  <span className="truncate">{label}</span>
                  <PanelAttentionDot visible={attention} />
                </TabsTrigger>
              );
            })}
          </TabsList>
        </div>

        {TAB_ORDER.map((tabId) => (
          <TabsContent
            key={tabId}
            value={tabId}
            forceMount
            className={cn(
              'mt-0 min-h-0 flex-1 overflow-hidden data-[state=inactive]:hidden',
            )}
          >
            {tabId === 'workspace' ? (
              <AgentWorkspacePanel
                isVisible={isVisible && isPanelOpen('workspace')}
                variant="tab"
              />
            ) : null}
            {tabId === 'processes' ? (
              <AgentProcessPanel
                isVisible={isVisible && isPanelOpen('processes')}
                variant="tab"
              />
            ) : null}
            {tabId === 'planning' ? (
              <AgentPlanningPanel
                isVisible={isVisible && isPanelOpen('planning')}
                variant="tab"
              />
            ) : null}
          </TabsContent>
        ))}
      </Tabs>
    </Card>
  );
}
