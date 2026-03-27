import { renderHook, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
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

// ---------------------------------------------------------------------------
// SP7: Session delete options – cascade delete vs. delete-only (orphan)
// ---------------------------------------------------------------------------
describe('AgentSessionListContext – SP7 session delete options', () => {
    const mockUnlisten = vi.fn();

    beforeEach(() => {
        vi.clearAllMocks();
        (listen as ReturnType<typeof vi.fn>).mockResolvedValue(mockUnlisten);
    });

    // ── deleteSession (cascade) ──────────────────────────────────────────────

    it('deleteSession: BFS removes parent AND direct child from UI', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'parent', name: 'Parent', status: 'idle', createdAt: Date.now() },
                    { id: 'child', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'parent' },
                ]);
            if (cmd === 'agent_delete_session') return Promise.resolve({ success: true, message: 'Deleted sessions', data: ['parent', 'child'] });
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        await act(async () => {
            await result.current.actions.deleteSession('parent');
        });

        // Both parent and child must be removed
        await waitFor(() => expect(result.current.state.sessions).toHaveLength(0));
        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session', { sessionId: 'parent' });
    });

    it('deleteSession: BFS removes entire 3-level tree (grandparent → parent → child)', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'gp', name: 'Grandparent', status: 'idle', createdAt: Date.now() },
                    { id: 'p', name: 'Parent', status: 'idle', createdAt: Date.now(), parentSessionId: 'gp' },
                    { id: 'c', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'p' },
                ]);
            if (cmd === 'agent_delete_session') return Promise.resolve({ success: true, message: 'ok', data: ['gp', 'p', 'c'] });
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(3));

        await act(async () => {
            await result.current.actions.deleteSession('gp');
        });

        // All three nodes must vanish
        await waitFor(() => expect(result.current.state.sessions).toHaveLength(0));
    });

    // ── deleteSessionOnly (orphan) ───────────────────────────────────────────

    it('deleteSessionOnly: removes parent, direct child becomes top-level (parentSessionId cleared)', async () => {
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'parent', name: 'Parent', status: 'idle', createdAt: Date.now() },
                    { id: 'child', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'parent' },
                ]);
            if (cmd === 'agent_delete_session_only') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        await act(async () => {
            await result.current.actions.deleteSessionOnly('parent');
        });

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(1));

        const remaining = result.current.state.sessions[0];
        expect(remaining.id).toBe('child');
        // Direct child must be orphaned (no parentSessionId)
        expect(remaining.parentSessionId).toBeUndefined();
        expect(safeInvoke).toHaveBeenCalledWith('agent_delete_session_only', { sessionId: 'parent' });
    });

    it('deleteSessionOnly: grandchild still linked to its own parent after orphan', async () => {
        // Tree: gp → p → c. Delete p with deleteSessionOnly.
        (safeInvoke as ReturnType<typeof vi.fn>).mockImplementation((cmd) => {
            if (cmd === 'agent_get_all_sessions')
                return Promise.resolve([
                    { id: 'gp', name: 'Grandparent', status: 'idle', createdAt: Date.now() },
                    { id: 'p', name: 'Parent', status: 'idle', createdAt: Date.now(), parentSessionId: 'gp' },
                    { id: 'c', name: 'Child', status: 'idle', createdAt: Date.now(), parentSessionId: 'p' },
                ]);
            if (cmd === 'agent_delete_session_only') return Promise.resolve();
            return Promise.resolve();
        });

        const { result } = renderHook(
            () => ({ state: useAgentSessionListState(), actions: useAgentSessionListActions() }),
            { wrapper: TestWrapper },
        );

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(3));

        await act(async () => {
            // Delete only the middle node – its direct child 'c' is orphaned at top level
            await result.current.actions.deleteSessionOnly('p');
        });

        await waitFor(() => expect(result.current.state.sessions).toHaveLength(2));

        const ids = result.current.state.sessions.map((s) => s.id);
        expect(ids).toContain('gp');
        expect(ids).toContain('c');
        expect(ids).not.toContain('p');

        // 'c' was a direct child of 'p' → parentSessionId cleared (now top-level)
        const orphanedChild = result.current.state.sessions.find((s) => s.id === 'c');
        expect(orphanedChild?.parentSessionId).toBeUndefined();

        // 'gp' parentSessionId should be untouched (was already undefined / unrelated)
        const gp = result.current.state.sessions.find((s) => s.id === 'gp');
        expect(gp?.parentSessionId).toBeUndefined();
    });
});
