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

  it('toasts newly failed MCP servers when phase becomes degraded', () => {
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

    expect(toast.error).toHaveBeenCalledTimes(1);
    expect(toast.error).toHaveBeenCalledWith(
      "MCP Server 'harbor-mcp' failed to load",
      expect.objectContaining({
        description: 'connection closed: initialize response',
      }),
    );
  });

  it('does not re-toast servers that were already failed', () => {
    const prev = baseState({
      phase: 'degraded',
      proxy: { exists: true, mode: 'configured', ready: true },
      servers: [
        {
          name: 'harbor-mcp',
          transport: 'stdio',
          status: 'failed',
          toolCount: 0,
          error: 'connection closed',
        },
      ],
    });
    const next = {
      ...prev,
      sequence: prev.sequence + 1,
    };

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).not.toHaveBeenCalled();
  });

  it('toasts each newly failed server when phase is failed', () => {
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
        {
          name: 'broken-http',
          transport: 'http',
          status: 'failed',
          toolCount: 0,
          error: 'connection refused',
        },
      ],
    });

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).toHaveBeenCalledTimes(2);
    expect(toast.error).toHaveBeenCalledWith(
      "MCP Server 'harbor-mcp' failed to load",
      expect.objectContaining({ description: 'No such file or directory' }),
    );
    expect(toast.error).toHaveBeenCalledWith(
      "MCP Server 'broken-http' failed to load",
      expect.objectContaining({ description: 'connection refused' }),
    );
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

  it('toasts generic init failure when phase is failed without server list', () => {
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

    notifyRuntimeStateErrors(prev, next);

    expect(toast.error).toHaveBeenCalledWith(
      'MCP Server initialization failed',
      expect.objectContaining({
        description: 'Loading tool configurations failed',
      }),
    );
  });
});
