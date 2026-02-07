import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createSession, upsertSession } from './session-crud';
import { Session, Assistant, Thread } from '@/models/chat';

// Mock the core module
vi.mock('./core', () => ({
  safeInvoke: vi.fn(),
}));

import { safeInvoke } from './core';

describe('session-crud', () => {
  const mockAssistant1: Assistant = {
    id: 'asst-1',
    name: 'Assistant 1',
    systemPrompt: 'Prompt 1',
    mcpServerIds: ['server-1', 'server-2'],
    createdAt: new Date(),
    updatedAt: new Date(),
    deletionProtected: false,
  };

  const mockAssistant2: Assistant = {
    id: 'asst-2',
    name: 'Assistant 2',
    systemPrompt: 'Prompt 2',
    mcpServerIds: ['server-2', 'server-3'], // Overlap with server-2
    createdAt: new Date(),
    updatedAt: new Date(),
    deletionProtected: false,
  };

  const mockSession: Session = {
    id: 'session-1',
    type: 'group',
    name: 'Test Session',
    assistants: [mockAssistant1, mockAssistant2],
    createdAt: new Date(),
    updatedAt: new Date(),
    sessionThread: {
      id: 'session-1',
      sessionId: 'session-1',
      createdAt: new Date(),
    } as Thread,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('createSession should extract unique mcpServerIds from assistants', async () => {
    (safeInvoke as any).mockResolvedValue({});

    await createSession(mockSession);

    expect(safeInvoke).toHaveBeenCalledWith('agent_create_session', {
      request: {
        sessionId: mockSession.id,
        name: mockSession.name,
        agentConfig: expect.objectContaining({
          // The fix should populate this field.
          // Currently it's expecting mcpServers: [], but we will change it to mcpServerIds: [...]
          // For now, let's verify what we expect AFTER the fix.
          mcpServerIds: expect.arrayContaining(['server-1', 'server-2', 'server-3']),
        }),
        isEphemeral: false,
      },
    });

    const callArgs = (safeInvoke as any).mock.calls[0][1];
    const passedIds = callArgs.request.agentConfig.mcpServerIds;
    expect(passedIds).toHaveLength(3); // Unique IDs
  });

  it('upsertSession should extract unique mcpServerIds from assistants', async () => {
    // Mock getSession to return existing session so upsert path is taken
    // BUT getSession calls safeInvoke('agent_get_session').
    // We need to handle sequential calls to safeInvoke.

    // First call: getSession -> agent_get_session
    // Second call: agent_update_session_config

    (safeInvoke as any)
      .mockResolvedValueOnce({
        id: 'session-1',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        config: {},
        activeThreadId: null
      }) // existing session found
      .mockResolvedValueOnce({}); // update result

    await upsertSession(mockSession);

    expect(safeInvoke).toHaveBeenLastCalledWith('agent_update_session_config', {
      request: {
        sessionId: mockSession.id,
        agentConfig: expect.objectContaining({
          mcpServerIds: expect.arrayContaining(['server-1', 'server-2', 'server-3']),
        }),
      },
    });
  });
});
