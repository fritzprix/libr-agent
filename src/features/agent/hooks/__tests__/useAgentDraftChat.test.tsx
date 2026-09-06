import { renderHook, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useAgentDraftChat } from '../useAgentDraftChat';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@/context/SettingsContext', () => ({
  useSettings: () => ({
    value: {
      advanced: { defaultMaxOutputTokens: 4096 },
    },
  }),
}));

vi.mock('@/context/DnDContext', () => ({
  useDnDContext: () => ({
    subscribe: vi.fn().mockReturnValue(() => {}),
  }),
}));

vi.mock('@/hooks/use-rust-backend', () => ({
  useRustBackend: () => true,
}));

vi.mock('../useScopedSkills', () => ({
  useScopedSkills: () => ({
    skills: [],
    loading: false,
    reload: vi.fn(),
  }),
}));

vi.mock('../useInputToken', () => ({
  useInputToken: () => ({
    stage: { kind: 'none' },
    typeResults: [],
    skillResults: [],
    onInputChange: vi.fn(),
    onTypeSelect: vi.fn(),
    onArgSelect: vi.fn(),
    onDismiss: vi.fn(),
  }),
}));

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  safeInvoke: vi.fn(),
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

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: (...args: unknown[]) => mocks.safeInvoke(...args),
}));

vi.mock('sonner', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
    loading: vi.fn(),
  },
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

describe('useAgentDraftChat prompt query param initialization', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.safeInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_assistant') {
        return {
          id: 'assistant-test-1',
          name: 'Test Assistant',
          config: {},
          createdAt: Date.now(),
          updatedAt: Date.now(),
        };
      }
      if (command === 'list_available_builtin_server_definitions') return [];
      if (command === 'list_mcp_server_configs') return [];
      if (command === 'agent_create_session') {
        return { sessionId: 'session-test-123' };
      }
      if (command === 'agent_submit_prompt') {
        return { status: 'submitted' };
      }
      return null;
    });
  });

  it('automatically triggers submitDraft when autoSubmit=true and assistant/prompt are loaded', async () => {
    const testPrompt = 'Write today morning briefing';
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <MemoryRouter
        initialEntries={[
          `/agent/draft?assistantId=assistant-test-1&prompt=${encodeURIComponent(testPrompt)}&autoSubmit=true`,
        ]}
      >
        {children}
      </MemoryRouter>
    );

    renderHook(() => useAgentDraftChat(), { wrapper });

    await waitFor(() => {
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringMatching(/^\/agent\/[a-zA-Z0-9_-]+$/),
      );
    });
  });

  it('initializes input from prompt query parameter', async () => {
    const testPrompt = 'Write today morning briefing';
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <MemoryRouter
        initialEntries={[
          `/agent/draft?assistantId=assistant-test-1&prompt=${encodeURIComponent(testPrompt)}`,
        ]}
      >
        {children}
      </MemoryRouter>
    );

    const { result } = renderHook(() => useAgentDraftChat(), { wrapper });

    expect(result.current.input).toBe(testPrompt);

    await waitFor(() => {
      expect(result.current.assistant?.id).toBe('assistant-test-1');
    });
  });

  it('initializes input from initialInput query parameter', async () => {
    const testInput = 'Hello from initialInput';
    const wrapper = ({ children }: { children: React.ReactNode }) => (
      <MemoryRouter
        initialEntries={[
          `/agent/draft?assistantId=assistant-test-1&initialInput=${encodeURIComponent(testInput)}`,
        ]}
      >
        {children}
      </MemoryRouter>
    );

    const { result } = renderHook(() => useAgentDraftChat(), { wrapper });

    expect(result.current.input).toBe(testInput);

    await waitFor(() => {
      expect(result.current.assistant?.id).toBe('assistant-test-1');
    });
  });
});
