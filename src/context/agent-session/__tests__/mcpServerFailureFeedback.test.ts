import { describe, expect, it } from 'vitest';
import type { SessionRuntimeServerState } from '@/models/agent-ipc';
import { collectNewMcpServerFailures } from '../mcpServerFailureFeedback';

function server(
  partial: Partial<SessionRuntimeServerState> &
    Pick<SessionRuntimeServerState, 'name' | 'status'>,
): SessionRuntimeServerState {
  return {
    transport: 'stdio',
    toolCount: 0,
    ...partial,
  };
}

describe('collectNewMcpServerFailures', () => {
  it('returns timeout toasts for newly failed servers with timeout errors', () => {
    const toasts = collectNewMcpServerFailures(
      new Set(),
      [
        server({
          name: 'slow-stdio',
          status: 'failed',
          error: "stdio server 'slow-stdio' tool discovery timed out after 30s",
        }),
        server({
          name: 'http-ok',
          transport: 'http',
          status: 'ready',
          toolCount: 3,
        }),
      ],
    );

    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.kind).toBe('timeout');
    expect(toasts[0]?.serverName).toBe('slow-stdio');
  });

  it('does not re-emit toasts already shown', () => {
    const servers = [
      server({
        name: 'broken',
        transport: 'http',
        status: 'failed',
        error: 'connection refused',
      }),
    ];
    const first = collectNewMcpServerFailures(new Set(), servers);
    expect(first).toHaveLength(1);

    const second = collectNewMcpServerFailures(
      new Set(first.map((toast) => toast.key)),
      servers,
    );
    expect(second).toHaveLength(0);
  });

  it('isolates multiple server failures into separate toasts', () => {
    const toasts = collectNewMcpServerFailures(new Set(), [
      server({
        name: 'a',
        status: 'failed',
        error: 'timed out after 30s',
      }),
      server({
        name: 'b',
        transport: 'http',
        status: 'failed',
        error: 'HTTP 503',
      }),
    ]);

    expect(toasts).toHaveLength(2);
    expect(toasts.map((toast) => toast.serverName).sort()).toEqual(['a', 'b']);
    expect(toasts.find((toast) => toast.serverName === 'a')?.kind).toBe(
      'timeout',
    );
    expect(toasts.find((toast) => toast.serverName === 'b')?.kind).toBe(
      'failed',
    );
  });

  it('classifies session wait-budget degrade messages as timeout', () => {
    const toasts = collectNewMcpServerFailures(new Set(), [
      server({
        name: 'pending-stdio',
        status: 'failed',
        error:
          "MCP server startup/tool discovery timed out after 60s; session continues without this server's tools",
      }),
    ]);

    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.kind).toBe('timeout');
  });
});
