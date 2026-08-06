/**
 * Watches background processes while the processes panel/tab is not active and
 * marks an attention (notification) dot — never auto-opens the shell.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useAgentPanels } from '@/context/AgentPanelsContext';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { getLogger } from '@/lib/logger';
import {
  filterVisibleProcesses,
  PROCESS_LIST_POLL_INTERVAL_MS,
  PROCESS_MESSAGE_REFRESH_DEBOUNCE_MS,
  processListFingerprint,
} from './process-panel/listProcessesShared';
import {
  isActiveProcessStatus,
  parseListProcessesResult,
} from './process-panel/types';

const logger = getLogger('AgentProcessAttentionUpdates');

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

  // While the processes tab is active the user sees updates live — clear the
  // baseline so leaving the tab does not immediately re-mark viewed changes.
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

      const visible = filterVisibleProcesses(parsed.processes);
      const nextFingerprint = processListFingerprint(visible);
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
      debounceMs: PROCESS_MESSAGE_REFRESH_DEBOUNCE_MS,
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
    }, PROCESS_LIST_POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [hasActive, panelOpen, refresh, session?.id]);

  return null;
}
