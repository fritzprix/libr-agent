import { render, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AgentChatStartView from '../AgentChatStartView';
import type { Assistant } from '@/models/chat';

const mocks = vi.hoisted(() => ({
  assistants: [] as Assistant[],
  navigate: vi.fn(),
  createSession: vi.fn(),
  getPlaybook: vi.fn(),
  toastLoading: vi.fn(),
  toastDismiss: vi.fn(),
  toastError: vi.fn(),
  loggerInfo: vi.fn(),
  loggerError: vi.fn(),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
  }),
}));

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>(
    'react-router-dom',
  );

  return {
    ...actual,
    useNavigate: () => mocks.navigate,
  };
});

vi.mock('@/context/AssistantContext', () => ({
  useAssistantContext: () => ({
    assistants: mocks.assistants,
  }),
}));

vi.mock('@/context/AgentSessionListContext', () => ({
  useAgentSessionListActions: () => ({
    createSession: mocks.createSession,
  }),
}));

vi.mock('@/lib/backend/playbooks', () => ({
  getPlaybook: (...args: unknown[]) => mocks.getPlaybook(...args),
}));

vi.mock('@/lib/logger', () => ({
  getLogger: () => ({
    info: mocks.loggerInfo,
    error: mocks.loggerError,
    warn: vi.fn(),
    debug: vi.fn(),
  }),
}));

vi.mock('sonner', () => ({
  toast: {
    loading: mocks.toastLoading,
    dismiss: mocks.toastDismiss,
    error: mocks.toastError,
  },
}));

describe('AgentChatStartView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.assistants = [
      {
        id: 'assistant-1',
        name: 'Assistant One',
        description: 'First assistant',
        deletionProtected: true,
      },
      {
        id: 'assistant-2',
        name: 'Assistant Two',
        description: 'Second assistant',
        deletionProtected: false,
      },
    ] as Assistant[];

    mocks.toastLoading.mockReturnValue('toast-1');
    mocks.createSession.mockResolvedValue({ id: 'session-123' });
  });

  it('starts playbook lookups for all assistants without waiting for earlier results', async () => {
    mocks.getPlaybook.mockImplementation(() => new Promise(() => {}));

    render(
      <MemoryRouter initialEntries={['/agent?playbookId=playbook-1']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(mocks.getPlaybook).toHaveBeenCalledTimes(2);
    });

    expect(mocks.getPlaybook).toHaveBeenNthCalledWith(
      1,
      'playbook-1',
      'assistant-1',
    );
    expect(mocks.getPlaybook).toHaveBeenNthCalledWith(
      2,
      'playbook-1',
      'assistant-2',
    );
  });

  it('creates a session with the assistant that owns the playbook', async () => {
    mocks.getPlaybook.mockImplementation(
      async (_playbookId: string, assistantId: string) => {
        if (assistantId === 'assistant-2') {
          return {
            id: 'playbook-1',
            goal: 'Launch workflow',
          };
        }

        return null;
      },
    );

    render(
      <MemoryRouter initialEntries={['/agent?playbookId=playbook-1']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    await waitFor(() => {
      expect(mocks.createSession).toHaveBeenCalledWith({
        assistant: mocks.assistants[1],
        name: 'Launch workflow',
      });
    });

    expect(mocks.navigate).toHaveBeenCalledWith(
      '/agent/session-123?playbookId=playbook-1',
    );
  });
});
