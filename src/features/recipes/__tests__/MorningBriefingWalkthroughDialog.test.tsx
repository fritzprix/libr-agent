import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom/vitest';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { MorningBriefingWalkthroughDialog } from '../components/MorningBriefingWalkthroughDialog';
import { MORNING_BRIEFING_RECIPE } from '../builtin-recipes';

const mocks = vi.hoisted(() => ({
  navigate: vi.fn(),
  toastSuccess: vi.fn(),
  toastError: vi.fn(),
  saveServer: vi.fn(),
  createAssistant: vi.fn(),
  updateAssistant: vi.fn(),
  createScheduledTask: vi.fn(),
  updateScheduledTask: vi.fn(),
  safeInvoke: vi.fn(),
  listMCPServerPresets: vi.fn(),
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

vi.mock('sonner', () => ({
  toast: {
    success: (...args: unknown[]) => mocks.toastSuccess(...args),
    error: (...args: unknown[]) => mocks.toastError(...args),
  },
}));

vi.mock('@/hooks/use-settings', () => ({
  useSettings: () => ({
    value: mocks.settings,
  }),
}));

vi.mock('../hooks/useMCPServerActions', () => ({
  useMCPServerActions: () => ({
    saveServer: mocks.saveServer,
  }),
}));

vi.mock('@/lib/backend/assistants', () => ({
  createAssistant: (...args: unknown[]) => mocks.createAssistant(...args),
  updateAssistant: (...args: unknown[]) => mocks.updateAssistant(...args),
}));

vi.mock('@/lib/backend/scheduled-tasks', () => ({
  createScheduledTask: (...args: unknown[]) =>
    mocks.createScheduledTask(...args),
  updateScheduledTask: (...args: unknown[]) =>
    mocks.updateScheduledTask(...args),
}));

vi.mock('@/lib/backend/core', () => ({
  safeInvoke: (...args: unknown[]) => mocks.safeInvoke(...args),
}));

vi.mock('@/lib/backend/mcp-server-config', () => ({
  listMCPServerPresets: () => mocks.listMCPServerPresets(),
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

describe('MorningBriefingWalkthroughDialog', () => {
  let storage: ReturnType<typeof createMemoryStorage>;

  beforeEach(() => {
    vi.clearAllMocks();
    storage = createMemoryStorage();
    vi.stubGlobal('localStorage', storage);
    mocks.settings = {
      serviceConfigs: {
        openai: { apiKey: 'test-key' },
      },
      customProviders: [],
    };
    mocks.saveServer.mockImplementation(async (server) => ({
      ...server,
      id: `saved-${server.name}`,
    }));
    mocks.createAssistant.mockImplementation(async (assistant) => ({
      ...assistant,
      id: 'assistant-briefing-1',
    }));
    mocks.updateAssistant.mockImplementation(async (assistant) => ({
      ...assistant,
    }));
    mocks.createScheduledTask.mockResolvedValue({ id: 'task-1' });
    mocks.updateScheduledTask.mockResolvedValue({ id: 'task-1' });
    mocks.safeInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_mcp_server_configs') return [];
      if (cmd === 'list_assistants') return [];
      if (cmd === 'list_scheduled_tasks') return [];
      return null;
    });
    mocks.listMCPServerPresets.mockResolvedValue([
      {
        name: 'hn',
        category: 'search',
        description: 'Read Hacker News',
        transportType: 'stdio',
        command: 'npx',
        args: ['-y', '@fre4x/hn'],
      },
      {
        name: 'yahoo-finance',
        category: 'data',
        description: 'Get real-time stock prices',
        transportType: 'stdio',
        command: 'npx',
        args: ['-y', '@fre4x/yahoo-finance'],
      },
    ]);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders modal with header and all sections when open', () => {
    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    expect(
      screen.getByText('🌅 모닝 테크 & 금융 브리핑 세팅 (Morning Briefing Setup)'),
    ).toBeInTheDocument();
    expect(screen.getByText('Step 1 · AI 모델 설정 확인')).toBeInTheDocument();
    expect(screen.getByText('Step 2 · 무료 확장 도구 설치')).toBeInTheDocument();
    expect(
      screen.getByText('Step 3 · 비서 및 자동 실행 스케줄'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', {
        name: /자동 설정 & 지금 브리핑 받아보기/i,
      }),
    ).toBeInTheDocument();
  });

  it('shows configured status badge when AI providers exist', () => {
    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    expect(screen.getByText('연동 완료')).toBeInTheDocument();
  });

  it('shows alert banner and "설정으로 이동" button when no AI provider is configured', () => {
    mocks.settings = {
      serviceConfigs: {},
      customProviders: [],
    };
    const onOpenChange = vi.fn();

    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={onOpenChange}
        />
      </MemoryRouter>,
    );

    expect(screen.getByText('설정 필요')).toBeInTheDocument();
    const settingsButton = screen.getByRole('button', {
      name: /설정으로 이동/i,
    });
    expect(settingsButton).toBeInTheDocument();

    fireEvent.click(settingsButton);
    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(mocks.navigate).toHaveBeenCalledWith('/settings?tab=ai-models');
  });

  it('shows checkboxes for hn and yahoo-finance default checked', () => {
    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    const hnCheckbox = screen.getByLabelText(/Hacker News \(hn\)/i);
    const yfCheckbox = screen.getByLabelText(/Yahoo Finance \(yahoo-finance\)/i);

    expect(hnCheckbox).toBeChecked();
    expect(yfCheckbox).toBeChecked();
    expect(
      screen.getByText(/100% 무료 · API 키 불필요/i),
    ).toBeInTheDocument();
  });

  it('displays assistant and scheduled task preview in Section 3', () => {
    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    expect(
      screen.getByText(MORNING_BRIEFING_RECIPE.assistantTemplate.name),
    ).toBeInTheDocument();
    expect(
      screen.getByText(MORNING_BRIEFING_RECIPE.assistantTemplate.description),
    ).toBeInTheDocument();
    expect(
      screen.getByText(MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/매일 09:00 AM \(YOLO 모드\)/i),
    ).toBeInTheDocument();
  });

  it('performs full automatic setup on action button click and navigates to draft', async () => {
    const onOpenChange = vi.fn();

    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={onOpenChange}
        />
      </MemoryRouter>,
    );

    const submitButton = screen.getByRole('button', {
      name: /자동 설정 & 지금 브리핑 받아보기/i,
    });

    fireEvent.click(submitButton);

    await waitFor(() => {
      // 1. Installs both MCP servers
      expect(mocks.saveServer).toHaveBeenCalledTimes(2);
      expect(mocks.saveServer).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'hn' }),
      );
      expect(mocks.saveServer).toHaveBeenCalledWith(
        expect.objectContaining({ name: 'yahoo-finance' }),
      );

      // 2. Creates assistant with installed server ids
      expect(mocks.createAssistant).toHaveBeenCalledWith(
        expect.objectContaining({
          name: '모닝 브리핑 비서',
          mcpServerIds: ['saved-hn', 'saved-yahoo-finance'],
          allowedBuiltInServiceAliases: ['browser', 'workspace'],
        }),
      );

      // 3. Creates scheduled task
      expect(mocks.createScheduledTask).toHaveBeenCalledWith({
        name: '매일 아침 9시 모닝 브리핑',
        cronExpression: '0 9 * * *',
        executionMode: 'yolo',
        assistantId: 'assistant-briefing-1',
        message:
          '오늘의 Hacker News 주요 인기 글 3개와 S&P 500, 나스닥 등 핵심 금융 지표를 종합하여 오늘의 모닝 브리핑 리포트를 작성해줘.',
      });

      // 4. Toast success and close dialog
      expect(mocks.toastSuccess).toHaveBeenCalled();
      expect(onOpenChange).toHaveBeenCalledWith(false);

      // 5. Marks completed in localStorage
      expect(
        localStorage.getItem('libragent:morning-briefing:completed'),
      ).toBe('true');

      // 6. Navigates to draft view with prompt ready and autoSubmit=true
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringContaining(
          '/agent/draft?assistantId=assistant-briefing-1&prompt=',
        ),
      );
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringContaining('&autoSubmit=true'),
      );
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringContaining(
          encodeURIComponent(MORNING_BRIEFING_RECIPE.testRunPrompt),
        ),
      );
    });
  });

  it('disables the action button and prevents setup when no AI provider is configured', async () => {
    mocks.settings = {
      serviceConfigs: {},
      customProviders: [],
    };
    const onOpenChange = vi.fn();

    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={onOpenChange}
        />
      </MemoryRouter>,
    );

    const submitButton = screen.getByRole('button', {
      name: /자동 설정 & 지금 브리핑 받아보기/i,
    });
    expect(submitButton).toBeDisabled();

    fireEvent.click(submitButton);
    expect(mocks.createAssistant).not.toHaveBeenCalled();
    expect(mocks.createScheduledTask).not.toHaveBeenCalled();
  });

  it('reuses existing MCP servers, assistant, and scheduled task on re-run (idempotency)', async () => {
    mocks.safeInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_mcp_server_configs') {
        return [
          { id: 'existing-hn-id', name: 'hn' },
          { id: 'existing-yf-id', name: 'yahoo-finance' },
        ];
      }
      if (cmd === 'list_assistants') {
        return [
          {
            id: 'existing-assistant-id',
            name: MORNING_BRIEFING_RECIPE.assistantTemplate.name,
          },
        ];
      }
      if (cmd === 'list_scheduled_tasks') {
        return [
          {
            id: 'existing-task-id',
            name: MORNING_BRIEFING_RECIPE.scheduledTaskTemplate.name,
          },
        ];
      }
      return null;
    });

    const onOpenChange = vi.fn();

    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={onOpenChange}
        />
      </MemoryRouter>,
    );

    const submitButton = screen.getByRole('button', {
      name: /자동 설정 & 지금 브리핑 받아보기/i,
    });

    fireEvent.click(submitButton);

    await waitFor(() => {
      // 1. Should NOT save duplicate servers
      expect(mocks.saveServer).not.toHaveBeenCalled();

      // 2. Should NOT create assistant, but update existing assistant
      expect(mocks.createAssistant).not.toHaveBeenCalled();
      expect(mocks.updateAssistant).toHaveBeenCalledWith(
        expect.objectContaining({
          id: 'existing-assistant-id',
          mcpServerIds: ['existing-hn-id', 'existing-yf-id'],
        }),
      );

      // 3. Should NOT create scheduled task, but update existing task
      expect(mocks.createScheduledTask).not.toHaveBeenCalled();
      expect(mocks.updateScheduledTask).toHaveBeenCalledWith(
        'existing-task-id',
        expect.objectContaining({
          assistantId: 'existing-assistant-id',
        }),
      );

      // 4. Navigates to draft with reused assistant id and autoSubmit=true
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringContaining(
          '/agent/draft?assistantId=existing-assistant-id&prompt=',
        ),
      );
      expect(mocks.navigate).toHaveBeenCalledWith(
        expect.stringContaining('&autoSubmit=true'),
      );
      expect(
        localStorage.getItem('libragent:morning-briefing:completed'),
      ).toBe('true');
    });
  });

  it('handles error gracefully if assistant or task creation fails', async () => {
    mocks.createAssistant.mockRejectedValueOnce(new Error('Backend error'));

    render(
      <MemoryRouter>
        <MorningBriefingWalkthroughDialog
          open={true}
          onOpenChange={vi.fn()}
        />
      </MemoryRouter>,
    );

    const submitButton = screen.getByRole('button', {
      name: /자동 설정 & 지금 브리핑 받아보기/i,
    });

    fireEvent.click(submitButton);

    await waitFor(() => {
      expect(mocks.toastError).toHaveBeenCalled();
    });
  });
});
