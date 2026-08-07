import { beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'sonner';
import type { SessionRuntimeState } from '@/models/agent-ipc';
import { notifyRuntimeStateErrors } from '../notifyRuntimeStateErrors';

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
  },
}));

function baseState(
  overrides: Partial<SessionRuntimeState> = {},
): SessionRuntimeState {
  return {
    sequence: 1,
    phase: 'initializing',
    proxy: { exists: true, mode: 'configured', ready: false },
    initialization: { result: 'pending' },
    servers: [],
    ...overrides,
  };
}

describe('notifyRuntimeStateErrors', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('does not toast per-server failures (handled by useMcpServerFailureToasts)', () => {
    const prev = baseState();
    const next = baseState({
      sequence: 2,
      phase: 'degraded',
      proxy: { exists: true, mode: 'configured', ready: true },
      initialization: {
        result: 'partial',
        error: '1 of 2 external servers failed during initialization',
      },
      servers: [
        {
          name: 'harbor-mcp',
          transport: 'stdio',
          status: 'failed',
          toolCount: 0,
          error: 'connection closed: initialize response',
        },
        {
          name: 'exa',
          transport: 'http',
          status: 'ready',
          toolCount: 2,
        },
      ],
    });

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).not.toHaveBeenCalled();
  });

  it('does not toast while still initializing', () => {
    const prev = baseState({ phase: 'hydrating' });
    const next = baseState({
      phase: 'initializing',
      servers: [
        {
          name: 'harbor-mcp',
          transport: 'stdio',
          status: 'failed',
          toolCount: 0,
          error: 'boom',
        },
      ],
    });

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).not.toHaveBeenCalled();
  });

  it('toasts generic init failure with deterministic ID when phase is failed without server list', () => {
    const prev = baseState();
    const next = baseState({
      phase: 'failed',
      proxy: { exists: false, mode: 'configured', ready: false },
      initialization: {
        result: 'failed',
        error: 'Loading tool configurations failed',
      },
      servers: [],
    });

    notifyRuntimeStateErrors(prev, next, 's1');

    expect(toast.error).toHaveBeenCalledWith(
      'MCP Server initialization failed',
      expect.objectContaining({
        id: 'mcp-runtime-error:s1',
        description: 'Loading tool configurations failed',
      }),
    );
  });

  it('does not toast generic failure when failed servers are present', () => {
    const prev = baseState();
    const next = baseState({
      phase: 'failed',
      proxy: { exists: true, mode: 'configured', ready: true },
      initialization: {
        result: 'failed',
        error: 'All external servers failed during initialization',
      },
      servers: [
        {
          name: 'harbor-mcp',
          transport: 'stdio',
          status: 'failed',
          toolCount: 0,
          error: 'No such file or directory',
        },
      ],
    });

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).not.toHaveBeenCalled();
  });

  it('does not toast generic failure when timed_out servers are present', () => {
    const prev = baseState();
    const next = baseState({
      phase: 'failed',
      proxy: { exists: true, mode: 'configured', ready: true },
      initialization: {
        result: 'failed',
        error:
          'All external servers failed or timed out during initialization',
      },
      servers: [
        {
          name: 'slow-stdio',
          transport: 'stdio',
          status: 'timed_out',
          toolCount: 0,
          error: 'Tool discovery timed out after 30s',
        },
      ],
    });

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).not.toHaveBeenCalled();
  });
});
