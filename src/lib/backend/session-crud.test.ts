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
    vi.mocked(safeInvoke).mockResolvedValue({});

    await createSession(mockSession);

    // The new implementation passes the first assistant as agentConfig
    expect(safeInvoke).toHaveBeenCalledWith('agent_create_session', {
      request: {
        sessionId: mockSession.id,
        name: mockSession.name,
        agentConfig: expect.objectContaining({
          // agentConfig is the first assistant from the session
          id: mockAssistant1.id,
          name: mockAssistant1.name,
          systemPrompt: mockAssistant1.systemPrompt,
          mcpServerIds: expect.arrayContaining(['server-1', 'server-2']),
          deletionProtected: mockAssistant1.deletionProtected,
        }),
        isEphemeral: false,
      },
    });

    const callArgs = vi.mocked(safeInvoke).mock.calls[0][1] as {
      request: {
        agentConfig: Assistant;
      };
    };
    const passedConfig = callArgs.request.agentConfig as Assistant;
    expect(passedConfig.id).toBe(mockAssistant1.id);
    expect(passedConfig.mcpServerIds).toHaveLength(2); // From first assistant only
  });

  it('upsertSession should extract unique mcpServerIds from assistants', async () => {
    // Mock getSession to return existing session so upsert path is taken
    // First call: getSession -> agent_get_session
    // Second call: agent_update_session_config

    vi.mocked(safeInvoke)
      .mockResolvedValueOnce({
        id: 'session-1',
        createdAt: Date.now(),
        updatedAt: Date.now(),
        config: {},
        activeThreadId: null
      }) // existing session found
      .mockResolvedValueOnce({}); // update result

    await upsertSession(mockSession);

    // The new implementation passes the first assistant as agentConfig
    expect(safeInvoke).toHaveBeenLastCalledWith('agent_update_session_config', {
      request: {
        sessionId: mockSession.id,
        agentConfig: expect.objectContaining({
          // agentConfig is the first assistant from the session
          id: mockAssistant1.id,
          name: mockAssistant1.name,
          systemPrompt: mockAssistant1.systemPrompt,
          mcpServerIds: expect.arrayContaining(['server-1', 'server-2']),
          deletionProtected: mockAssistant1.deletionProtected,
        }),
      },
    });
  });
});
