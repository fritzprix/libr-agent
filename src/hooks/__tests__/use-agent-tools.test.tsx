import { SWRConfig } from 'swr';
import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  buildMcpToolsDiscoveryRevision,
  useAgentTools,
} from '../use-agent-tools';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import { validateMCPTools } from '@/lib/schemas/mcp-tool';
import { isBuiltinTool } from '@/lib/tool-call-utils';
import type { MCPTool } from '@/lib/mcp/protocol/tool';
import type { ReactNode } from 'react';

// Mock dependencies
vi.mock('@/lib/backend/agent-commands', () => ({
  getAgentAvailableTools: vi.fn(),
}));

vi.mock('@/lib/schemas/mcp-tool', () => ({
  validateMCPTools: vi.fn(),
}));

vi.mock('@/lib/tool-call-utils', () => ({
  isBuiltinTool: vi.fn(),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  }),
}));

function createWrapper() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <SWRConfig value={{ provider: () => new Map(), dedupingInterval: 0 }}>
        {children}
      </SWRConfig>
    );
  };
}

describe('buildMcpToolsDiscoveryRevision', () => {
  it('includes proxy readiness and sorted server fingerprints', () => {
    expect(
      buildMcpToolsDiscoveryRevision(
        [
          { name: 'arxiv', status: 'ready', toolCount: 5 },
          { name: 'exa', status: 'ready', toolCount: 2 },
        ],
        true,
      ),
    ).toBe('ready:arxiv:ready:5|exa:ready:2');
  });

  it('changes when a slow stdio server becomes ready', () => {
    const pending = buildMcpToolsDiscoveryRevision(
      [
        { name: 'arxiv', status: 'discovering', toolCount: 0 },
        { name: 'exa', status: 'ready', toolCount: 2 },
      ],
      false,
    );
    const ready = buildMcpToolsDiscoveryRevision(
      [
        { name: 'arxiv', status: 'ready', toolCount: 5 },
        { name: 'exa', status: 'ready', toolCount: 2 },
      ],
      true,
    );

    expect(pending).toBe('pending:arxiv:discovering:0|exa:ready:2');
    expect(ready).toBe('ready:arxiv:ready:5|exa:ready:2');
    expect(pending).not.toBe(ready);
  });
});

describe('useAgentTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should return initial state when sessionId is undefined', () => {
    const { result } = renderHook(() => useAgentTools(undefined), {
      wrapper: createWrapper(),
    });

    expect(result.current.availableTools).toEqual([]);
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeUndefined();
    expect(getAgentAvailableTools).not.toHaveBeenCalled();
  });

  it('should fetch and validate tools successfully', async () => {
    const mockSessionId = 'test-session-123';
    const mockBackendResponse: MCPTool[] = [
      {
        name: 'tool1',
        description: 'test1',
        inputSchema: { type: 'object', properties: {} },
      },
      {
        name: 'tool2',
        description: 'test2',
        inputSchema: { type: 'object', properties: {} },
      },
    ];
    const mockValidatedTools: MCPTool[] = [
      {
        name: 'tool1',
        description: 'test1',
        inputSchema: { type: 'object', properties: {} },
      },
    ];

    vi.mocked(getAgentAvailableTools).mockResolvedValue(mockBackendResponse);
    vi.mocked(validateMCPTools).mockReturnValue(mockValidatedTools);
    vi.mocked(isBuiltinTool).mockReturnValue(false);

    const { result } = renderHook(() => useAgentTools(mockSessionId), {
      wrapper: createWrapper(),
    });

    expect(result.current.isLoading).toBe(true);
    expect(result.current.error).toBeUndefined();
    expect(result.current.availableTools).toEqual([]);

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(getAgentAvailableTools).toHaveBeenCalledWith(mockSessionId);
    expect(validateMCPTools).toHaveBeenCalledWith(mockBackendResponse);
    expect(result.current.availableTools).toEqual(mockValidatedTools);
    expect(result.current.error).toBeUndefined();
  });

  it('should refetch when discovery revision advances after soft timeout', async () => {
    const mockSessionId = 'test-session-race';
    const partialTools: MCPTool[] = [
      {
        name: 'exa_search',
        description: 'http only',
        inputSchema: { type: 'object', properties: {} },
      },
    ];
    const fullTools: MCPTool[] = [
      ...partialTools,
      {
        name: 'arxiv_search',
        description: 'stdio arrived later',
        inputSchema: { type: 'object', properties: {} },
      },
    ];

    vi.mocked(getAgentAvailableTools)
      .mockResolvedValueOnce(partialTools)
      .mockResolvedValueOnce(fullTools);
    vi.mocked(validateMCPTools)
      .mockReturnValueOnce(partialTools)
      .mockReturnValueOnce(fullTools);
    vi.mocked(isBuiltinTool).mockReturnValue(false);

    const pendingRevision = buildMcpToolsDiscoveryRevision(
      [
        { name: 'arxiv', status: 'discovering', toolCount: 0 },
        { name: 'exa', status: 'ready', toolCount: 1 },
      ],
      false,
    );
    const readyRevision = buildMcpToolsDiscoveryRevision(
      [
        { name: 'arxiv', status: 'ready', toolCount: 1 },
        { name: 'exa', status: 'ready', toolCount: 1 },
      ],
      true,
    );

    const { result, rerender } = renderHook(
      ({ revision }) =>
        useAgentTools(mockSessionId, { discoveryRevision: revision }),
      {
        wrapper: createWrapper(),
        initialProps: { revision: pendingRevision },
      },
    );

    await waitFor(() => {
      expect(result.current.availableTools).toEqual(partialTools);
    });
    expect(getAgentAvailableTools).toHaveBeenCalledTimes(1);

    rerender({ revision: readyRevision });

    await waitFor(() => {
      expect(result.current.availableTools).toEqual(fullTools);
    });
    expect(getAgentAvailableTools).toHaveBeenCalledTimes(2);
  });

  it('should handle API errors', async () => {
    const mockSessionId = 'test-session-123';
    const errorMessage = 'Network error';

    vi.mocked(getAgentAvailableTools).mockRejectedValue(
      new Error(errorMessage),
    );

    const { result } = renderHook(() => useAgentTools(mockSessionId), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe(errorMessage);
    expect(result.current.availableTools).toEqual([]);
  });

  it('should handle non-Error throwables', async () => {
    const mockSessionId = 'test-session-123';
    const errorMessage = 'String error';

    vi.mocked(getAgentAvailableTools).mockRejectedValue(errorMessage);

    const { result } = renderHook(() => useAgentTools(mockSessionId), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe(errorMessage);
    expect(result.current.availableTools).toEqual([]);
  });

  it('should throw and handle error if response is not an array', async () => {
    const mockSessionId = 'test-session-123';
    const invalidResponse = { name: 'tool1' };

    vi.mocked(getAgentAvailableTools).mockResolvedValue(
      invalidResponse as unknown as MCPTool[],
    );

    const { result } = renderHook(() => useAgentTools(mockSessionId), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe('Expected array of tools from backend');
    expect(result.current.availableTools).toEqual([]);
  });

  it('should handle unmounting before fetch completes', async () => {
    const mockSessionId = 'test-session-123';
    let resolvePromise: (val: MCPTool[]) => void;
    const promise = new Promise<MCPTool[]>((resolve) => {
      resolvePromise = resolve;
    });

    vi.mocked(getAgentAvailableTools).mockReturnValue(promise);

    const { result, unmount } = renderHook(() => useAgentTools(mockSessionId), {
      wrapper: createWrapper(),
    });

    expect(result.current.isLoading).toBe(true);
    expect(result.current.availableTools).toEqual([]);

    unmount();

    resolvePromise!([]);

    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(result.current.isLoading).toBe(true);
    expect(result.current.availableTools).toEqual([]);
  });
});
