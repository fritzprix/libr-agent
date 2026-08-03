import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { SessionRuntimeServerState } from '@/models/agent-ipc';
import { useMcpDiscoveryBanner } from '../useMcpDiscoveryBanner';

const servers: SessionRuntimeServerState[] = [
  {
    name: 'exa',
    transport: 'http',
    status: 'ready',
    toolCount: 2,
  },
];

describe('useMcpDiscoveryBanner', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows loading while proxy is not ready', () => {
    const { result } = renderHook(() =>
      useMcpDiscoveryBanner({
        hasSession: true,
        isProxyReady: false,
        phase: 'initializing',
        initResult: 'pending',
        servers,
        sessionId: 's1',
      }),
    );

    expect(result.current.bannerKind).toBe('loading');
  });

  it('shows success result then auto-dismisses', () => {
    const { result } = renderHook(() =>
      useMcpDiscoveryBanner({
        hasSession: true,
        isProxyReady: true,
        phase: 'ready',
        initResult: 'success',
        servers,
        sessionId: 's1',
      }),
    );

    expect(result.current.bannerKind).toBe('result');

    act(() => {
      vi.advanceTimersByTime(2500);
    });

    expect(result.current.bannerKind).toBeNull();
  });

  it('keeps partial result until dismissed', () => {
    const { result } = renderHook(() =>
      useMcpDiscoveryBanner({
        hasSession: true,
        isProxyReady: true,
        phase: 'degraded',
        initResult: 'partial',
        servers,
        sessionId: 's1',
      }),
    );

    expect(result.current.bannerKind).toBe('result');

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(result.current.bannerKind).toBe('result');

    act(() => {
      result.current.dismissResultBanner();
    });

    expect(result.current.bannerKind).toBeNull();
  });

  it('shows result banner for timed_out servers after Session Ready', () => {
    const timedOutServers: SessionRuntimeServerState[] = [
      {
        name: 'slow-stdio',
        transport: 'stdio',
        status: 'timed_out',
        toolCount: 0,
        error: 'Tool discovery timed out',
      },
      {
        name: 'http-ok',
        transport: 'http',
        status: 'ready',
        toolCount: 3,
      },
    ];

    const { result } = renderHook(() =>
      useMcpDiscoveryBanner({
        hasSession: true,
        isProxyReady: true,
        phase: 'degraded',
        initResult: 'partial',
        servers: timedOutServers,
        sessionId: 's-timeout',
      }),
    );

    expect(result.current.bannerKind).toBe('result');
  });
});
