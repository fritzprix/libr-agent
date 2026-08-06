import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRustBackend } from '@/hooks/use-rust-backend';
import { useAgentSessionState } from '@/context/AgentSessionContext';
import { useAgentMessageTrigger } from '@/hooks/use-agent-message-trigger';
import { getLogger } from '@/lib/logger';
import { toast } from 'sonner';
import { useTranslation } from 'react-i18next';
import type { MCPContent } from '@/lib/mcp/protocol/content';
import type { MCPResult } from '@/lib/mcp/protocol/response';
import {
  filterVisibleProcesses,
  PROCESS_LIST_POLL_INTERVAL_MS,
  PROCESS_MESSAGE_REFRESH_DEBOUNCE_MS,
  summarizeVisibleProcesses,
} from './listProcessesShared';
import {
  isActiveProcessStatus,
  parseListProcessesResult,
  parseReadProcessOutputResult,
  type ListProcessesResult,
  type ProcessEntry,
  type ReadProcessOutputResult,
} from './types';

const logger = getLogger('useProcesses');

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isTextContent(
  value: unknown,
): value is Extract<MCPContent, { type: 'text' }> {
  return (
    isRecord(value) && value.type === 'text' && typeof value.text === 'string'
  );
}

function extractMcpText(result: MCPResult): string | null {
  if (!Array.isArray(result.content)) {
    return null;
  }

  const messages = result.content
    .filter(isTextContent)
    .map((item) => item.text.trim())
    .filter((text) => text.length > 0);

  return messages.length > 0 ? messages.join('\n\n') : null;
}

function emptyListResult(): ListProcessesResult {
  return {
    processes: [],
    total: 0,
    running: 0,
    finished: 0,
  };
}

export function useProcesses(options: { enabled?: boolean } = {}) {
  const { enabled = true } = options;
  const { t } = useTranslation();
  const { agentCallBuiltinTool } = useRustBackend();
  const { session } = useAgentSessionState();

  const [listResult, setListResult] =
    useState<ListProcessesResult>(emptyListResult);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [actionProcessId, setActionProcessId] = useState<string | null>(null);
  const [outputProcessId, setOutputProcessId] = useState<string | null>(null);
  const [outputResult, setOutputResult] =
    useState<ReadProcessOutputResult | null>(null);
  const [outputOpen, setOutputOpen] = useState(false);
  const [outputLoading, setOutputLoading] = useState(false);
  const [outputError, setOutputError] = useState<string | null>(null);

  const loadingRef = useRef(false);
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  // Keep translations out of callback deps — i18n `t` often changes identity.
  const tRef = useRef(t);
  tRef.current = t;

  const refresh = useCallback(
    async (opts?: { silent?: boolean }) => {
      const sessionId = session?.id;
      if (!sessionId || !enabledRef.current) {
        return;
      }

      if (loadingRef.current) {
        return;
      }

      loadingRef.current = true;
      const silent = Boolean(opts?.silent);
      if (!silent) {
        setLoading(true);
        setError(null);
      }

      try {
        const response = await agentCallBuiltinTool<unknown>(
          sessionId,
          'workspace__listProcesses',
          { statusFilter: 'all' },
        );

        if (response.isError) {
          const message =
            extractMcpText(response) ??
            tRef.current(
              'agent.processes.loadError',
              'Failed to load processes',
            );
          // Silent background polls must not wipe the list or halt polling.
          if (silent) {
            logger.warn('listProcesses silent refresh returned error', {
              message,
            });
            return;
          }
          setError(message);
          setListResult(emptyListResult());
          logger.warn('listProcesses returned error', { message });
          return;
        }

        const parsed = parseListProcessesResult(response.structuredContent);
        if (!parsed) {
          const message = tRef.current(
            'agent.processes.invalidPayload',
            'Process list response was invalid',
          );
          if (silent) {
            logger.warn(
              'listProcesses silent refresh failed validation; keeping previous list',
              { structuredContent: response.structuredContent },
            );
            return;
          }
          setError(message);
          setListResult(emptyListResult());
          logger.warn('listProcesses structured content failed validation', {
            structuredContent: response.structuredContent,
          });
          return;
        }

        setListResult(parsed);
        if (!silent) {
          setError(null);
        }
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : tRef.current(
                'agent.processes.loadError',
                'Failed to load processes',
              );
        if (silent) {
          logger.warn(
            'listProcesses silent refresh threw; keeping previous list',
            {
              error: message,
            },
          );
          return;
        }
        setError(message);
        logger.error('Failed to list processes', { error: message });
        toast.error(message);
      } finally {
        loadingRef.current = false;
        setLoading(false);
      }
    },
    [agentCallBuiltinTool, session?.id],
  );

  useEffect(() => {
    if (!enabled || !session?.id) {
      setListResult(emptyListResult());
      setError(null);
      return;
    }

    void refresh();
  }, [enabled, session?.id, refresh]);

  const onMessageTrigger = useCallback(() => {
    if (enabledRef.current) {
      void refresh({ silent: true });
    }
  }, [refresh]);

  const messageTriggerOptions = useMemo(
    () => ({
      enabled: enabled && Boolean(session?.id),
      debounceMs: PROCESS_MESSAGE_REFRESH_DEBOUNCE_MS,
    }),
    [enabled, session?.id],
  );

  useAgentMessageTrigger(onMessageTrigger, messageTriggerOptions);

  // Killed processes stay in the backend registry (for agent tools) but should
  // leave the panel once terminated — matching user expectation after stop.
  const processes = useMemo(
    () => filterVisibleProcesses(listResult.processes),
    [listResult.processes],
  );

  const summary = useMemo(
    () => summarizeVisibleProcesses(processes),
    [processes],
  );

  const hasActiveProcesses = summary.running > 0;

  useEffect(() => {
    if (!enabled || !session?.id || !hasActiveProcesses) {
      return;
    }

    const timer = window.setInterval(() => {
      void refresh({ silent: true });
    }, PROCESS_LIST_POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [enabled, session?.id, hasActiveProcesses, refresh]);

  const stopProcess = useCallback(
    async (process: ProcessEntry) => {
      const sessionId = session?.id;
      if (!sessionId) {
        return;
      }

      setActionProcessId(process.process_id);
      try {
        const response = await agentCallBuiltinTool(
          sessionId,
          'workspace__stopProcess',
          { processId: process.process_id },
        );

        if (response.isError) {
          const message =
            extractMcpText(response) ??
            tRef.current('agent.processes.stopError', 'Failed to stop process');
          toast.error(message);
          return;
        }

        // Optimistic removal so the row disappears before the next poll.
        setListResult((prev) => ({
          ...prev,
          processes: prev.processes.filter(
            (item) => item.process_id !== process.process_id,
          ),
        }));

        toast.success(
          tRef.current('agent.processes.stopSuccess', 'Process stop requested'),
        );
        await refresh({ silent: true });
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : tRef.current(
                'agent.processes.stopError',
                'Failed to stop process',
              );
        toast.error(message);
        logger.error('Failed to stop process', {
          processId: process.process_id,
          error: message,
        });
      } finally {
        setActionProcessId(null);
      }
    },
    [agentCallBuiltinTool, refresh, session?.id],
  );

  /** Non-blocking status check via waitForProcess(timeout: 0). */
  const pollProcess = useCallback(
    async (process: ProcessEntry) => {
      const sessionId = session?.id;
      if (!sessionId) {
        return;
      }

      setActionProcessId(process.process_id);
      try {
        const response = await agentCallBuiltinTool(
          sessionId,
          'workspace__waitForProcess',
          { processId: process.process_id, timeout: 0 },
        );

        if (response.isError) {
          const message =
            extractMcpText(response) ??
            tRef.current(
              'agent.processes.pollError',
              'Failed to poll process status',
            );
          toast.error(message);
          return;
        }

        await refresh({ silent: true });
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : tRef.current(
                'agent.processes.pollError',
                'Failed to poll process status',
              );
        toast.error(message);
        logger.error('Failed to poll process status', {
          processId: process.process_id,
          error: message,
        });
      } finally {
        setActionProcessId(null);
      }
    },
    [agentCallBuiltinTool, refresh, session?.id],
  );

  const fetchOutput = useCallback(
    async (
      processId: string,
      opts?: { silent?: boolean },
    ): Promise<boolean> => {
      const sessionId = session?.id;
      if (!sessionId) {
        return false;
      }

      const silent = Boolean(opts?.silent);
      if (!silent) {
        setOutputLoading(true);
        setOutputError(null);
      }

      try {
        const response = await agentCallBuiltinTool(
          sessionId,
          'workspace__readProcessOutput',
          {
            processId,
            stream: 'both',
            mode: 'tail',
            lines: 100,
          },
        );

        if (response.isError) {
          const message =
            extractMcpText(response) ??
            tRef.current(
              'agent.processes.readError',
              'Failed to read process output',
            );
          if (!silent) {
            setOutputError(message);
          } else {
            logger.warn('Silent process output refresh returned error', {
              processId,
              message,
            });
          }
          return false;
        }

        const parsed = parseReadProcessOutputResult(response.structuredContent);
        if (!parsed) {
          const message = tRef.current(
            'agent.processes.invalidOutputPayload',
            'Process output response was invalid',
          );
          if (!silent) {
            setOutputError(message);
          }
          return false;
        }

        setOutputResult(parsed);
        if (!silent) {
          setOutputError(null);
        }
        return true;
      } catch (err) {
        const message =
          err instanceof Error
            ? err.message
            : tRef.current(
                'agent.processes.readError',
                'Failed to read process output',
              );
        if (!silent) {
          setOutputError(message);
        }
        logger.error('Failed to read process output', {
          processId,
          error: message,
        });
        return false;
      } finally {
        if (!silent) {
          setOutputLoading(false);
        }
      }
    },
    [agentCallBuiltinTool, session?.id],
  );

  const readOutput = useCallback(
    async (process: ProcessEntry) => {
      setActionProcessId(process.process_id);
      setOutputProcessId(process.process_id);
      setOutputOpen(true);
      setOutputResult(null);
      try {
        await fetchOutput(process.process_id);
      } finally {
        setActionProcessId(null);
      }
    },
    [fetchOutput],
  );

  const closeOutput = useCallback(() => {
    setOutputOpen(false);
    setOutputProcessId(null);
    setOutputResult(null);
    setOutputError(null);
  }, []);

  const outputProcess = useMemo(
    () =>
      outputProcessId
        ? (processes.find((item) => item.process_id === outputProcessId) ??
          null)
        : null,
    [outputProcessId, processes],
  );

  const outputIsActive = outputProcess
    ? isActiveProcessStatus(outputProcess.status)
    : Boolean(outputResult?.is_process_running);

  // Live tail: re-fetch while the output dialog is open and the process is active.
  useEffect(() => {
    if (!outputOpen || !outputProcessId || !outputIsActive || !session?.id) {
      return;
    }

    const timer = window.setInterval(() => {
      void fetchOutput(outputProcessId, { silent: true });
    }, PROCESS_LIST_POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(timer);
    };
  }, [fetchOutput, outputIsActive, outputOpen, outputProcessId, session?.id]);

  return {
    processes,
    total: summary.total,
    running: summary.running,
    finished: summary.finished,
    loading,
    error,
    actionProcessId,
    outputOpen,
    outputLoading,
    outputResult,
    outputError,
    outputProcessId,
    refresh,
    stopProcess,
    pollProcess,
    readOutput,
    closeOutput,
    setOutputOpen,
  };
}
