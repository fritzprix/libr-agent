import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Loader2, Square } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { cancelDelegatedWorkflow } from '@/lib/backend/agent-commands';
import { getLogger } from '@/lib/logger';

const logger = getLogger('AgentSessionToolWaiting');

export interface AgentSessionToolWaitingProps {
  /** Parent (caller) session storage id */
  callerSessionId: string;
  /** Child session ref from tool args (display token or storage id) */
  childSessionRef: string;
  /** Optional display label (assistant name or session id) */
  displayName?: string;
}

/**
 * In-flight UI for agent__checkSession(wait=true): live status + Stop.
 * Lives outside ToolStructuredResult — there is no structured_content yet.
 */
export const AgentSessionToolWaiting: React.FC<AgentSessionToolWaitingProps> = ({
  callerSessionId,
  childSessionRef,
  displayName,
}) => {
  const { t } = useTranslation('common');
  const [stopping, setStopping] = useState(false);
  const label = displayName?.trim() || childSessionRef;

  const handleStop = async () => {
    if (stopping) return;
    setStopping(true);
    try {
      await cancelDelegatedWorkflow(callerSessionId, childSessionRef);
      toast.message(
        t(
          'agent.toolStructured.sessionStopRequested',
          'Stop requested for "{{name}}"',
          { name: label },
        ),
      );
    } catch (error) {
      logger.error('Failed to stop delegated session', {
        callerSessionId,
        childSessionRef,
        error,
      });
      toast.error(
        t(
          'agent.toolStructured.sessionStopFailed',
          'Could not stop the subagent session',
        ),
      );
      setStopping(false);
    }
  };

  return (
    <div
      className="flex items-center justify-between gap-3 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3"
      data-testid="tool-structured-check-session-waiting"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Loader2 className="h-4 w-4 shrink-0 animate-spin text-amber-600 dark:text-amber-400" />
        <span className="truncate text-sm font-medium">
          {t(
            'agent.toolStructured.sessionWaitingTitle',
            'Running subagent: "{{name}}"',
            { name: label },
          )}
        </span>
      </div>
      <Button
        type="button"
        variant="destructive"
        size="sm"
        className="shrink-0"
        disabled={stopping}
        onClick={() => {
          void handleStop();
        }}
      >
        <Square className="mr-1 h-3 w-3 fill-current" />
        {stopping
          ? t('agent.toolStructured.sessionStopping', 'Stopping…')
          : t('agent.toolStructured.sessionStop', 'Stop')}
      </Button>
    </div>
  );
};
