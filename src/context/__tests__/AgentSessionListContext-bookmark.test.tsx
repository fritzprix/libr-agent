import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    __resetAgentSessionListStartupCacheForTests,
    AgentSessionListProvider,
    useAgentSessionListState,
    useAgentSessionListActions,
} from '../AgentSessionListContext';
import { safeInvoke } from '@/lib/backend/core';
import { listen } from '@tauri-apps/api/event';
import { MemoryRouter } from 'react-router-dom';

// Mock Tauri APIs
vi.mock('@/lib/backend/core', () => ({
    safeInvoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

// Mock logger
vi.mock('@/lib/logger', () => ({
    getLogger: () => ({
        info: vi.fn(),
        debug: vi.fn(),
        warn: vi.fn(),
        error: vi.fn(),
    }),
}));

// Mock ModelProvider
vi.mock('../ModelProvider', () => ({
    useModelOptions: () => ({
        modelId: 'test-model',
        provider: 'test-provider',
    }),
}));

// Mock GlobalEventContext
vi.mock('../GlobalEventContext', () => ({
    useBackendResource: vi.fn(),
}));

vi.mock('@/context/SettingsContext', () => ({
    useSettings: () => ({
        value: {
            advanced: {
                defaultSessionMaxDepth: 0,
                defaultSessionMaxFanout: 0,
            },
        },
    }),
}));

vi.mock('../LLMServiceContext', () => ({
    useLLMService: () => ({
        clearSessionState: vi.fn(),
        clearAllCompactState: vi.fn(),
    }),
}));

function TestWrapper({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

// ─── Regression: toggleBookmark stale-closure fix ───────────────────────────
describe('AgentSessionListContext – toggleBookmark', () => {
    const makeSession = (id: string, isBookmarked: boolean) => ({
        id,
        name: `Session ${id}`,
        status: 'idle' as const,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        isBookmarked,
    });

    beforeEach(() => {
        vi.clearAllMocks();
        __resetAgentSessionListStartupCacheForTests();
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(vi.fn());
    });

    function setupWithSessions(sessions: ReturnType<typeof makeSession>[]) {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve(sessions);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.resolve(undefined);
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });
    }

    it('toggles bookmark from false → true optimistically', async () => {
        setupWithSessions([makeSession('s1', false)]);

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        expect(result.current.state.sessions[0].isBookmarked).toBe(true);
        expect(safeInvoke).toHaveBeenCalledWith(
            'agent_toggle_session_bookmark',
            expect.objectContaining({ sessionId: 's1', bookmarked: true }),
        );
    });

    it('toggles bookmark from true → false', async () => {
        setupWithSessions([makeSession('s1', true)]);

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        expect(result.current.state.sessions[0].isBookmarked).toBe(false);
        expect(safeInvoke).toHaveBeenCalledWith(
            'agent_toggle_session_bookmark',
            expect.objectContaining({ sessionId: 's1', bookmarked: false }),
        );
    });

    it('reverts optimistic update when IPC call fails', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([makeSession('s1', false)]);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.reject(new Error('network error'));
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1').catch(() => {/* expected */});
        });

        // Should be reverted back to original (false)
        expect(result.current.state.sessions[0].isBookmarked).toBe(false);
    });

    it('sends correct newValue (not stale pre-flip value) to IPC', async () => {
        setupWithSessions([makeSession('s1', false)]);

        const invokeArgs: unknown[] = [];
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
            invokeArgs.push({ cmd, args });
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([makeSession('s1', false)]);
            if (cmd === 'agent_toggle_session_bookmark') return Promise.resolve(undefined);
            return Promise.reject(new Error(`Unexpected cmd: ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        await act(async () => {
            await result.current.actions.toggleBookmark('s1');
        });

        const toggleCall = (invokeArgs as Array<{ cmd: string; args: { bookmarked: boolean } }>).find(
            (c) => c.cmd === 'agent_toggle_session_bookmark',
        );
        expect(toggleCall?.args.bookmarked).toBe(true);
    });
});
