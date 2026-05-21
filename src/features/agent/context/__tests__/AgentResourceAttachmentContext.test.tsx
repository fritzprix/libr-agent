import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  AgentResourceAttachmentProvider,
  useAgentResourceAttachment,
} from '../AgentResourceAttachmentContext';

const mockMutate = vi.fn();

vi.mock('swr', () => ({
  default: vi.fn(() => ({
    data: [],
    mutate: mockMutate,
  })),
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: {
      system: {
        maxFileUploadSizeMB: 50,
      },
      experimental: {
        inlineAudioAttachment: true,
      },
    },
  }),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('@/features/agent/api/agent-backend', () => ({
  agentCallBuiltinTool: vi.fn(),
  deleteAgentFile: vi.fn(),
}));

vi.mock('@/features/agent/lib/resource-attachment-operations', () => ({
  addAgentAttachment: vi.fn(),
}));

describe('AgentResourceAttachmentContext', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('clears pending files when the session id changes', async () => {
    const cleanupBlob = vi.fn();
    let activeSessionId = 'session-1';

    const Wrapper = ({ children }: { children: ReactNode }) => (
      <AgentResourceAttachmentProvider sessionId={activeSessionId}>
        {children}
      </AgentResourceAttachmentProvider>
    );

    const { result, rerender } = renderHook(() => useAgentResourceAttachment(), {
      wrapper: Wrapper,
    });

    act(() => {
      result.current.addPendingFiles([
        {
          url: 'file:///tmp/example.txt',
          filename: 'example.txt',
          mimeType: 'text/plain',
          blobCleanup: cleanupBlob,
        },
      ]);
    });

    expect(result.current.pendingFiles).toHaveLength(1);
    expect(result.current.pendingFiles[0]?.sessionId).toBe('session-1');

    activeSessionId = 'session-2';
    rerender();

    await waitFor(() => {
      expect(result.current.pendingFiles).toEqual([]);
    });

    expect(cleanupBlob).toHaveBeenCalledTimes(1);
  });
});
