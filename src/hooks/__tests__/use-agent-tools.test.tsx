import { renderHook, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useAgentTools } from '../use-agent-tools';
import { getAgentAvailableTools } from '@/lib/backend/agent-commands';
import { validateMCPTools } from '@/lib/schemas/mcp-tool';
import { isBuiltinTool } from '@/lib/tool-call-utils';

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
    const mockBackendResponse = [
      { name: 'tool1', description: 'test1' },
      { name: 'tool2', description: 'test2' },
    ];
    const mockValidatedTools = [
      { name: 'tool1', description: 'test1' },
    ];

    vi.mocked(getAgentAvailableTools).mockResolvedValue(mockBackendResponse);
    vi.mocked(validateMCPTools).mockReturnValue(mockValidatedTools as any);
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

    vi.mocked(getAgentAvailableTools).mockResolvedValue(invalidResponse as any);

    const { result } = renderHook(() => useAgentTools(mockSessionId));

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe('Expected array of tools from backend');
    expect(result.current.availableTools).toEqual([]);
  });

  it('should handle unmounting before fetch completes', async () => {
    const mockSessionId = 'test-session-123';
    let resolvePromise: (val: any) => void;
    const promise = new Promise((resolve) => {
      resolvePromise = resolve;
    });

    vi.mocked(getAgentAvailableTools).mockReturnValue(promise as any);

    const { result, unmount } = renderHook(() => useAgentTools(mockSessionId));

    expect(result.current.isLoading).toBe(true);

    unmount();

    // Resolve after unmount
    resolvePromise!([]);

    // We can't wait for the state update, but we can verify it doesn't crash
    // and ideally the loading state wouldn't be updated (though React 18 handles this better)
  });
});
