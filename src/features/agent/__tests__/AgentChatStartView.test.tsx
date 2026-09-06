import { fireEvent, render, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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

function createMemoryStorage() {
  const store = new Map<string, string>();
  return {
    getItem: vi.fn((key: string) => store.get(key) ?? null),
    setItem: vi.fn((key: string, value: string) => {
      store.set(key, value);
    }),
    removeItem: vi.fn((key: string) => {
      store.delete(key);
    }),
    clear: vi.fn(() => {
      store.clear();
    }),
  };
}

describe('AgentChatStartView', () => {
  let storage: ReturnType<typeof createMemoryStorage>;

  beforeEach(() => {
    vi.clearAllMocks();
    storage = createMemoryStorage();
    vi.stubGlobal('localStorage', storage);
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

  afterEach(() => {
    vi.unstubAllGlobals();
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
    expect(mocks.navigate).toHaveBeenCalledWith('/settings?tab=ai-models');
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

  it('renders featured recipe card and clicking button opens walkthrough dialog', async () => {
    const { getByTestId, findByRole, queryByRole, getByRole } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    const card = getByTestId('featured-recipe-card');
    expect(card).toBeInTheDocument();
    expect(card).toHaveTextContent('모닝 브리핑');

    const startButton = getByRole('button', {
      name: /가이드 워크스루 시작/i,
    });
    expect(startButton).toBeInTheDocument();

    expect(queryByRole('dialog')).not.toBeInTheDocument();

    fireEvent.click(startButton);

    const dialog = await findByRole('dialog');
    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveTextContent('모닝 테크 & 금융 브리핑 세팅');
  });

  it('dismisses featured recipe card when dismiss button is clicked', () => {
    const { getByTestId, queryByTestId } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    expect(getByTestId('featured-recipe-card')).toBeInTheDocument();

    const dismissButton = getByTestId('dismiss-recipe-button');
    fireEvent.click(dismissButton);

    expect(queryByTestId('featured-recipe-card')).not.toBeInTheDocument();
    expect(
      storage.getItem('libragent:morning-briefing:dismissed'),
    ).toBe('true');
  });

  it('does not render featured recipe card when already dismissed in localStorage', () => {
    storage.setItem('libragent:morning-briefing:dismissed', 'true');

    const { queryByTestId } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    expect(queryByTestId('featured-recipe-card')).not.toBeInTheDocument();
  });

  it('does not render featured recipe card when completed in localStorage', () => {
    storage.setItem('libragent:morning-briefing:completed', 'true');

    const { queryByTestId } = render(
      <MemoryRouter initialEntries={['/agent']}>
        <AgentChatStartView />
      </MemoryRouter>,
    );

    expect(queryByTestId('featured-recipe-card')).not.toBeInTheDocument();
  });
});
