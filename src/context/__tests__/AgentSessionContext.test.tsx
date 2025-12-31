import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    AgentSessionProvider,
    useAgentSessionState,
    useAgentSessionActions,
} from '../AgentSessionContext';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import * as messagesBackend from '@/lib/backend/messages';
import type { Assistant } from '@/models/chat';
import type { Message } from '@/models/chat';

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
vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn(),
}));

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

// Mock backend session API (if called by provider on mount)
vi.mock('@/lib/backend/session', () => ({
    listSessions: vi.fn(),
    createSession: vi.fn(),
}));

// Mock backend messages API
vi.mock('@/lib/backend/messages', () => ({
    getMessagesPageForSession: vi.fn(),
}));

// Mock ModelProvider
vi.mock('../ModelProvider', () => ({
    useModelOptions: () => ({
        modelId: 'test-model',
        provider: 'test-provider',
    }),
}));

function TestWrapper({ children }: { children: React.ReactNode }) {
    return <AgentSessionProvider>{children}</AgentSessionProvider>;
}

describe('AgentSessionContext', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
        // Mock getMessagesPageForSession to return empty list by default
        (messagesBackend.getMessagesPageForSession as ReturnType<typeof vi.fn>).mockResolvedValue({
            items: [],
            total: 0,
            page: 1,
            pageSize: 50,
            totalPages: 0,
        });
    });

    it('should provide initial state', () => {
        const { result } = renderHook(() => useAgentSessionState(), {
            wrapper: TestWrapper,
        });

        expect(result.current.currentSession).toBeNull();
        expect(result.current.workflowStatus).toBe('idle');
        expect(result.current.messages).toEqual([]);
    });

    it('should register event listener when session is active', async () => {
        const { result } = renderHook(
            () => ({ state: useAgentSessionState(), actions: useAgentSessionActions() }),
            { wrapper: TestWrapper }
        );

        // Create a session to make it active
        (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
            id: 'session-1',
            name: 'Test Session',
            status: 'idle',
            created_at: Date.now(),
            updated_at: Date.now(),
        });

        await act(async () => {
            await result.current.actions.createSession({
                name: 'Test Session',
                assistant: mockAssistant,
            });
        });

        expect(result.current.state.currentSession?.id).toBe('session-1');

        await waitFor(() => {
            expect(listen).toHaveBeenCalledWith('agent:event', expect.any(Function));
        });
    });

    describe('Event Handling', () => {
        it('should update workflow status on statusChanged event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => ({ state: useAgentSessionState(), actions: useAgentSessionActions() }),
                { wrapper: TestWrapper }
            );

            // Setup active session
            (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
            });

            await act(async () => {
                await result.current.actions.createSession({
                    name: 'Test Session',
                    assistant: mockAssistant,
                });
            });

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit statusChanged event
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'statusChanged',
                        sessionId: 'session-1',
                        status: 'busy',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.state.workflowStatus).toBe('busy');
                // expect(result.current.state.isSessionLoading).toBe(true);
            });
        });

        it('should append message on messageAdded event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => ({ state: useAgentSessionState(), actions: useAgentSessionActions() }),
                { wrapper: TestWrapper }
            );

            // Setup active session
            (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
            });
            await act(async () => {
                await result.current.actions.createSession({
                    name: 'Test Session',
                    assistant: mockAssistant,
                });
            });

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit messageAdded event
            const newMessage: Message = {
                id: 'msg-1',
                sessionId: 'session-1',
                threadId: 'session-1',
                role: 'user',
                content: [{ type: 'text', text: 'Hello' }],
                createdAt: new Date(),
            } as unknown as Message; // Using any to bypass potential minor type mismatches if Model Message vs Chat Message types differ slightly, but imports are from same file so should be ok.
            // But let's check imports. Message is from '@/models/chat'.

            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'messageAdded',
                        sessionId: 'session-1',
                        message: newMessage,
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.state.messages).toHaveLength(1);
                expect(result.current.state.messages[0].id).toBe('msg-1');
            });
        });

        it('should ignore events from other sessions', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => ({ state: useAgentSessionState(), actions: useAgentSessionActions() }),
                { wrapper: TestWrapper }
            );

            // Setup active session
            (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
            });
            await act(async () => {
                await result.current.actions.createSession({
                    name: 'Test Session',
                    assistant: mockAssistant,
                });
            });

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit statusChanged for DIFFERENT session
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'statusChanged',
                        sessionId: 'other-session',
                        status: 'busy',
                    },
                });
            });

            // Status should remain idle
            expect(result.current.state.workflowStatus).toBe('idle');
        });

        it('should handle workflowError event', async () => {
            let eventHandler: ((event: unknown) => void) | undefined;
            (listen as ReturnType<typeof vi.fn>).mockImplementation(
                async (eventName, handler) => {
                    if (eventName === 'agent:event') {
                        eventHandler = handler as (event: unknown) => void;
                    }
                    return mockUnlisten;
                }
            );

            const { result } = renderHook(
                () => ({ state: useAgentSessionState(), actions: useAgentSessionActions() }),
                { wrapper: TestWrapper }
            );

            // Setup active session
            (invoke as ReturnType<typeof vi.fn>).mockResolvedValue({
                id: 'session-1',
                name: 'Test Session',
                status: 'idle',
            });
            await act(async () => {
                await result.current.actions.createSession({
                    name: 'Test Session',
                    assistant: mockAssistant,
                });
            });

            await waitFor(() => {
                expect(eventHandler).toBeDefined();
            });

            // Emit workflowError event
            act(() => {
                eventHandler?.({
                    payload: {
                        type: 'workflowError',
                        sessionId: 'session-1',
                        error: 'Something went wrong',
                    },
                });
            });

            await waitFor(() => {
                expect(result.current.state.workflowStatus).toBe('error');
                expect(result.current.state.error).toBe('Something went wrong');
            });
        });
    });
});
