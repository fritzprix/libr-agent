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
import type { Assistant } from '@/models/chat';
import { MemoryRouter } from 'react-router-dom';
import React from 'react';

const { loggerInfo, loggerDebug, loggerWarn, loggerError } = vi.hoisted(() => ({
    loggerInfo: vi.fn(),
    loggerDebug: vi.fn(),
    loggerWarn: vi.fn(),
    loggerError: vi.fn(),
}));

// Mock Assistant for creating sessions
const mockAssistant: Assistant = {
    id: 'asst-1',
    name: 'Assistant',
    systemPrompt: 'sys prompt',
    deletionProtected: false,
    createdAt: new Date(),
    updatedAt: new Date(),
};

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
        info: loggerInfo,
        debug: loggerDebug,
        warn: loggerWarn,
        error: loggerError,
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

export function TestWrapper({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

function createDeferred<T>() {
    let resolve!: (value: T) => void;
    let reject!: (reason?: unknown) => void;
    const promise = new Promise<T>((res, rej) => {
        resolve = res;
        reject = rej;
    });

    return { promise, resolve, reject };
}

function DraftRouteWrapper({ children }: { children: React.ReactNode }) {
    return (
        <MemoryRouter initialEntries={['/agent/draft']}>
            <AgentSessionListProvider>{children}</AgentSessionListProvider>
        </MemoryRouter>
    );
}

describe('AgentSessionListContext', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        __resetAgentSessionListStartupCacheForTests();
        // Default: listen resolves to an unlisten function
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
    });

    it('should provide initial state', async () => {
        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapper,
        });

        expect(result.current.sessions).toEqual([]);
        // It might be loading immediately due to useEffect
        // So we wait for it to settle (mock returns empty array instantly)

        await waitFor(() => {
            expect(result.current.isSessionsListLoading).toBe(false);
        });
    });

    it('should load sessions on mount', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([
            {
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            }
        ]);

        const { result } = renderHook(() => useAgentSessionListState(), {
            wrapper: TestWrapper,
        });

        await waitFor(() => {
            expect(result.current.sessions).toHaveLength(1);
            expect(result.current.sessions[0].id).toBe('session-1');
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_list_sessions', {
            request: { limit: 100 },
        });
    });

    it('dedupes the initial session load across StrictMode remounts', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([
            {
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            }
        ]);

        function StrictWrapper({ children }: { children: React.ReactNode }) {
            return (
                <React.StrictMode>
                    <TestWrapper>{children}</TestWrapper>
                </React.StrictMode>
            );
        }

        renderHook(() => useAgentSessionListState(), {
            wrapper: StrictWrapper,
        });

        await waitFor(() => {
            expect(
                (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                    ([cmd]) => cmd === 'agent_list_sessions',
                ),
            ).toHaveLength(1);
            expect(
                (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                    ([cmd]) => cmd === 'agent_list_attention_sessions',
                ),
            ).toHaveLength(1);
        });

        expect(
            loggerInfo.mock.calls.filter(
                ([message]) => message === 'Loading recent agent sessions',
            )
        ).toHaveLength(1);
        expect(
            loggerInfo.mock.calls.filter(
                ([message]) => message === 'Loaded recent sessions',
            )
        ).toHaveLength(1);
    });

    it('does not mark the draft route as a viewed session', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockResolvedValue([]);

        renderHook(() => useAgentSessionListState(), {
            wrapper: DraftRouteWrapper,
        });

        await waitFor(() => {
            expect(safeInvoke).toHaveBeenCalledWith('agent_list_sessions', {
                request: { limit: 100 },
            });
        });

        expect(safeInvoke).not.toHaveBeenCalledWith(
            'agent_mark_session_viewed',
            expect.objectContaining({ sessionId: 'draft' }),
        );
    });

    it('keeps the loading flag and latest data when an older request resolves after refresh', async () => {
        const staleSessions = createDeferred<
            Array<{
                id: string;
                name: string;
                status: 'idle';
                createdAt: number;
                updatedAt: number;
            }>
        >();
        const refreshedSessions = createDeferred<
            Array<{
                id: string;
                name: string;
                status: 'idle';
                createdAt: number;
                updatedAt: number;
            }>
        >();

        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_list_attention_sessions') {
                return Promise.resolve([]);
            }
            if (cmd !== 'agent_list_sessions') {
                return Promise.resolve();
            }

            return (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                ([callCmd]) => callCmd === 'agent_list_sessions',
            ).length === 1
                ? staleSessions.promise
                : refreshedSessions.promise;
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper }
        );

        await waitFor(() => {
            expect(
                (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                    ([cmd]) => cmd === 'agent_list_sessions',
                ),
            ).toHaveLength(1);
            expect(result.current.state.isSessionsListLoading).toBe(true);
        });

        act(() => {
            void result.current.actions.loadSessions(true);
        });

        await waitFor(() => {
            expect(
                (safeInvoke as ReturnType<typeof vi.fn>).mock.calls.filter(
                    ([cmd]) => cmd === 'agent_list_sessions',
                ),
            ).toHaveLength(2);
        });

        staleSessions.resolve([
            {
                id: 'session-1',
                name: 'Stale Session',
                status: 'idle',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            }
        ]);
        await Promise.resolve();

        expect(result.current.state.isSessionsListLoading).toBe(true);
        expect(result.current.state.sessions).toEqual([]);

        refreshedSessions.resolve([
            {
                id: 'session-2',
                name: 'Fresh Session',
                status: 'idle',
                createdAt: Date.now(),
                updatedAt: Date.now(),
            }
        ]);

        await waitFor(() => {
            expect(result.current.state.isSessionsListLoading).toBe(false);
            expect(result.current.state.sessions).toHaveLength(1);
            expect(result.current.state.sessions[0].id).toBe('session-2');
        });
    });

    it('should create a new session', async () => {
        // Mock create response
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
            if (cmd === 'agent_list_sessions') {
                return Promise.resolve({ items: [], nextCursor: undefined });
            }
            if (cmd === 'agent_list_attention_sessions') return Promise.resolve([]);
            if (cmd === 'get_assistant') {
                return Promise.resolve({
                    id: args.id,
                    name: 'Test Assistant',
                    config: JSON.stringify({
                        systemPrompt: 'You are a test assistant.',
                        allowedBuiltInServiceAliases: ['planning', 'knowledge']
                    }),
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                });
            }
            if (cmd === 'agent_create_session') {
                return Promise.resolve({
                    id: args.request.sessionId,
                    name: args.request.name,
                    status: 'idle',
                    createdAt: Date.now(),
                    updatedAt: Date.now(),
                });
            }
            return Promise.reject(new Error(`Unknown command ${cmd}`));
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper }
        );

        await act(async () => {
            await result.current.actions.createSession({
                name: 'New Session',
                assistant: mockAssistant,
            });
        });

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
            expect(result.current.state.sessions[0].name).toBe('New Session');
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_create_session', expect.anything());
    });

    it('should delete a session', async () => {
        // Setup: Load one session
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_list_sessions') {
                return Promise.resolve({
                    items: [
                        {
                            id: 'session-1',
                            name: 'Test Session',
                            status: 'idle',
                            createdAt: Date.now(),
                        },
                    ],
                    nextCursor: undefined,
                });
            }
            if (cmd === 'agent_list_attention_sessions') return Promise.resolve([]);
            if (cmd === 'agent_delete_session') return Promise.resolve({ success: true, message: 'Session deleted', data: ['session-1'] });
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper }
        );

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(1);
        });

        await act(async () => {
            await result.current.actions.deleteSession('session-1');
        });

        await waitFor(() => {
            expect(result.current.state.sessions).toHaveLength(0);
        });

        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'session-1' });
    });
});
