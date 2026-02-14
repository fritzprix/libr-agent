import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    AgentSessionListProvider,
    useAgentSessionListState,
    useAgentSessionListActions,
} from '../AgentSessionListContext';
import { invoke } from '@tauri-apps/api/core';
import type { Assistant } from '@/models/chat';

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
vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
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

function TestWrapper({ children }: { children: React.ReactNode }) {
    return <AgentSessionListProvider>{children}</AgentSessionListProvider>;
}

describe('AgentSessionListContext', () => {
    beforeEach(() => {
        vi.clearAllMocks();
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
        (invoke as ReturnType<typeof vi.fn>).mockResolvedValue([
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

        expect(invoke).toHaveBeenCalledWith('agent_get_all_sessions');
    });

    it('should create a new session', async () => {
        // Mock create response
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd, args) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([]);
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

        expect(invoke).toHaveBeenCalledWith('agent_create_session', expect.anything());
    });

    it('should delete a session', async () => {
        // Setup: Load one session
        (invoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions') return Promise.resolve([
                {
                    id: 'session-1',
                    name: 'Test Session',
                    status: 'idle',
                    createdAt: Date.now(),
                }
            ]);
            if (cmd === 'agent_delete_session') return Promise.resolve();
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

        expect(invoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'session-1' });
    });
});
