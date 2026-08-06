/**
 * Watches background processes while the processes panel is closed and marks
 * an attention (notification) dot on the header toggle — never auto-opens.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useAgentPanels } from '@/context/AgentPanelsContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  isActiveProcessStatus,
  parseListProcessesResult,
} from './process-panel/types';

const logger = getLogger('AgentProcessAttentionUpdates');

const POLL_INTERVAL_MS = 2500;
const MESSAGE_REFRESH_DEBOUNCE_MS = 500;

function processFingerprint(
  processes: Array<{ process_id: string; status: string }>,
): string {
  return processes
    .map((process) => `${process.process_id}:${process.status}`)
    .sort()
    .join('|');
}

export function AgentProcessAttentionUpdates() {
  const { session } = useAgentSessionState();
  const { isPanelOpen, markPanelAttention } = useAgentPanels();
  const { agentCallBuiltinTool } = useRustBackend();
  const panelOpen = isPanelOpen('processes');

  const [hasActive, setHasActive] = useState(false);
  const hasHydratedRef = useRef(false);
  const fingerprintRef = useRef('');
  const panelOpenRef = useRef(panelOpen);
  panelOpenRef.current = panelOpen;

  useEffect(() => {
    if (!session?.id) {
      hasHydratedRef.current = false;
      fingerprintRef.current = '';
      setHasActive(false);
    }
  }, [session?.id]);

  // While the panel is open the user sees updates live — clear the closed-panel
  // baseline so closing does not immediately re-mark already-viewed changes.
  useEffect(() => {
    if (panelOpen) {
      hasHydratedRef.current = false;
      fingerprintRef.current = '';
      setHasActive(false);
    }
  }, [panelOpen]);

  const refresh = useCallback(async () => {
    const sessionId = session?.id;
    if (!sessionId || panelOpenRef.current) {
      return;
    }

    try {
      const response = await agentCallBuiltinTool(
        sessionId,
        'workspace__listProcesses',
        { statusFilter: 'all' },
      );

      if (response.isError) {
        return;
      }

      const parsed = parseListProcessesResult(response.structuredContent);
      if (!parsed) {
        return;
      }

      const visible = parsed.processes.filter(
        (process) => process.status !== 'killed',
      );
      const nextFingerprint = processFingerprint(visible);
      const nextHasActive = visible.some((process) =>
        isActiveProcessStatus(process.status),
      );
      setHasActive(nextHasActive);

      if (!hasHydratedRef.current) {
        fingerprintRef.current = nextFingerprint;
        hasHydratedRef.current = true;
        return;
      }

      if (nextFingerprint !== fingerprintRef.current) {
        fingerprintRef.current = nextFingerprint;
        markPanelAttention('processes');
      }
    } catch (error: unknown) {
      logger.warn('Failed to refresh process attention state', { error });
    }
  }, [agentCallBuiltinTool, markPanelAttention, session?.id]);

  useEffect(() => {
    if (!session?.id || panelOpen) {
      return;
    }
    void refresh();
  }, [session?.id, panelOpen, refresh]);

  const onMessageTrigger = useCallback(() => {
    if (!panelOpenRef.current) {
      void refresh();
    }
  }, [refresh]);

  const messageTriggerOptions = useMemo(
    () => ({
      enabled: Boolean(session?.id) && !panelOpen,
      debounceMs: MESSAGE_REFRESH_DEBOUNCE_MS,
    }),
    [panelOpen, session?.id],
  );

  useAgentMessageTrigger(onMessageTrigger, messageTriggerOptions);

  useEffect(() => {
    if (!session?.id || panelOpen || !hasActive) {
      return;
    }

    const timer = window.setInterval(() => {
      void refresh();
    }, POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [hasActive, panelOpen, refresh, session?.id]);

  return null;
}
