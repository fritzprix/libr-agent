import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Planning Server Crash Reproduction', () => {
    const sessionId = 'crash-test-session';
    const threadId = 'crash-test-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();

        await planningServer.switchContext!({ sessionId, threadId });
    });

    afterEach(async () => {
        await db.close();
    });

    it('should not crash when adding a todo if existing todos have missing names', async () => {
        // 1. Manually insert a "corrupted" todo (missing name) into the DB
        // We use 'any' to bypass type checking for the purpose of simulating data corruption
        await db.todos.add({
            sessionId,
            threadId,
            status: 'pending',
            order: 0,
            createdAt: Date.now(),
            // Intentionally omitting name to simulate corruption
            name: undefined
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
        } as any);

        // 2. Verify the corrupted item exists
        const todos = await db.todos.toArray();
        expect(todos).toHaveLength(1);
        expect(todos[0].name).toBeUndefined();

        // 3. Attempt to add a new todo using the server tool
        // This normally crashes because it tries to check for duplicates by trimming existing names
        try {
            const result = await planningServer.callTool('add_todo', { name: 'New Task' });

            // 4. Assert success if fixed
            expect(result.isError).toBe(false);
            expect(result.structuredContent).toHaveProperty('todos');

            // 5. Verify the corrupted todo is handled gracefully (e.g. shows as "(Untitled)")
            // Note: This assertion depends on the specific fix implementation
            const state = await planningServer.callTool('get_current_state', {});
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            const structuredState = state.structuredContent as any;
            const firstTodo = structuredState.state.todos[0];
            expect(firstTodo.name).toBe('(Untitled)');

        } catch (error) {
            // Test fails if it crashes
            console.error('Crashed as expected (before fix):', error);
            throw error;
        }
    });
});
