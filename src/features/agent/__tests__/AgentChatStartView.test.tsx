import { fireEvent, render, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import AgentChatStartView from '../AgentChatStartView';
import type { Assistant } from '@/models/chat';
import type { AssistantSummary } from '@/lib/backend/assistants';

const mocks = vi.hoisted(() => ({
  assistants: [] as AssistantSummary[],
  fullAssistantsById: {} as Record<string, Assistant>,
  navigate: vi.fn(),
  createSession: vi.fn(),
  getPlaybook: vi.fn(),
  toastLoading: vi.fn(),
  toastDismiss: vi.fn(),
  toastError: vi.fn(),
  loggerInfo: vi.fn(),
  loggerError: vi.fn(),
  settings: {
    serviceConfigs: {
      openai: { apiKey: 'test-key' },
    } as Record<string, unknown>,
    customProviders: [] as unknown[],
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: string | { defaultValue?: string }) => {
      if (typeof options === 'string') return options;
      if (options && typeof options === 'object' && 'defaultValue' in options) {
        return options.defaultValue ?? key;
      }
      return key;
    },
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

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: mocks.settings,
  }),
}));

vi.mock('../hooks/useAssistantSummaries', () => ({
  useAssistantSummaries: () => ({
    assistants: mocks.assistants,
    loading: false,
    error: null,
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

vi.mock('@/lib/backend/assistants', () => ({
  getAssistant: (id: string) => Promise.resolve(mocks.fullAssistantsById[id]),
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
    ];
    mocks.fullAssistantsById = {
      'assistant-1': {
        id: 'assistant-1',
        name: 'Assistant One',
        description: 'First assistant',
        systemPrompt: 'Prompt one',
        deletionProtected: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      'assistant-2': {
        id: 'assistant-2',
        name: 'Assistant Two',
        description: 'Second assistant',
        systemPrompt: 'Prompt two',
        deletionProtected: false,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
    };

    mocks.toastLoading.mockReturnValue('toast-1');
    mocks.createSession.mockResolvedValue({ id: 'session-123' });
    mocks.settings = {
      serviceConfigs: {
        openai: { apiKey: 'test-key' },
      },
      customProviders: [],
    };
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
        assistant: mocks.fullAssistantsById['assistant-2'],
        name: 'Launch workflow',
      });
    });

    expect(mocks.navigate).toHaveBeenCalledWith(
      '/agent/session-123?playbookId=playbook-1',
    );
  });

  it('renders onboarding banner when no AI providers are configured and clicking action navigates to /settings', () => {
    mocks.settings = {
      serviceConfigs: {},
      customProviders: [],
    };

    const { getByTestId, getByRole } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    const banner = getByTestId('onboarding-banner');
    expect(banner).toBeInTheDocument();

    const settingsButton = getByRole('button', {
      name: /Configure AI Model/i,
    });
    expect(settingsButton).toBeInTheDocument();

    fireEvent.click(settingsButton);
    expect(mocks.navigate).toHaveBeenCalledWith('/settings');
  });

  it('does not render onboarding banner when AI providers are configured', () => {
    mocks.settings = {
      serviceConfigs: {
        openai: { apiKey: 'test-key' },
      },
      customProviders: [],
    };

    const { queryByTestId } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    expect(queryByTestId('onboarding-banner')).not.toBeInTheDocument();
  });
});
