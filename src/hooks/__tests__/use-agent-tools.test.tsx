import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAgentTools } from '../use-agent-tools';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import { validateMCPTools } from '@/lib/schemas/mcp-tool';
import { isBuiltinTool } from '@/lib/tool-call-utils';
import type { MCPTool } from '@/lib/mcp/protocol/tool';

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

describe('useAgentTools', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should return initial state when sessionId is undefined', () => {
    const { result } = renderHook(() => useAgentTools(undefined));

    expect(result.current.availableTools).toEqual([]);
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeUndefined();
    expect(getAgentAvailableTools).not.toHaveBeenCalled();
  });

  it('should fetch and validate tools successfully', async () => {
    const mockSessionId = 'test-session-123';
    const mockBackendResponse: MCPTool[] = [
      { name: 'tool1', description: 'test1', inputSchema: { type: 'object', properties: {} } },
      { name: 'tool2', description: 'test2', inputSchema: { type: 'object', properties: {} } },
    ];
    const mockValidatedTools: MCPTool[] = [
      { name: 'tool1', description: 'test1', inputSchema: { type: 'object', properties: {} } },
    ];

    vi.mocked(getAgentAvailableTools).mockResolvedValue(mockBackendResponse);
    vi.mocked(validateMCPTools).mockReturnValue(mockValidatedTools);
    vi.mocked(isBuiltinTool).mockReturnValue(false);

    const { result } = renderHook(() => useAgentTools(mockSessionId));

    // Initial state while fetching
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

  it('should handle API errors', async () => {
    const mockSessionId = 'test-session-123';
    const errorMessage = 'Network error';

    vi.mocked(getAgentAvailableTools).mockRejectedValue(new Error(errorMessage));

    const { result } = renderHook(() => useAgentTools(mockSessionId));

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

    const { result } = renderHook(() => useAgentTools(mockSessionId));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe(errorMessage);
    expect(result.current.availableTools).toEqual([]);
  });

  it('should throw and handle error if response is not an array', async () => {
    const mockSessionId = 'test-session-123';
    const invalidResponse = { name: 'tool1' }; // Not an array

    vi.mocked(getAgentAvailableTools).mockResolvedValue(invalidResponse as unknown as MCPTool[]);

    const { result } = renderHook(() => useAgentTools(mockSessionId));

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

    const { result, unmount } = renderHook(() => useAgentTools(mockSessionId));

    expect(result.current.isLoading).toBe(true);
    expect(result.current.availableTools).toEqual([]);

    unmount();

    // Resolve after unmount
    resolvePromise!([]);

    // Wait for the promise microtask to flush; state should not change after unmount.
    await new Promise((resolve) => setTimeout(resolve, 0));

    // React 18 keeps the last snapshot; verify it did not update post-unmount.
    expect(result.current.isLoading).toBe(true);
    expect(result.current.availableTools).toEqual([]);
  });
});
