import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';
import type { PlanningTodo } from '../db';

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

    it('should not crash when adding a todo if existing todos have missing titles', async () => {
        // 1. Manually insert a "corrupted" todo (missing title) into the DB
        // We use 'as unknown as PlanningTodo' to bypass type checking for intentional corruption simulation

        await db.todos.add({
            sessionId,
            threadId,
            status: 'pending',
            order: 0,
            createdAt: Date.now(),
            checked: false,
            // Intentionally omitting title to simulate corruption
            title: undefined
        } as unknown as PlanningTodo);

        // 2. Verify the corrupted item exists
        const todos = await db.todos.toArray();
        expect(todos).toHaveLength(1);
        expect(todos[0].title).toBeUndefined();

        // 3. Attempt to add a new todo using the server tool
        // This normally crashes because it tries to check for duplicates by trimming existing titles
        try {
            const result = await planningServer.callTool('addTodo', { title: 'New Task' });

            // 4. Assert success if fixed
            expect(result.isError).toBe(false);
            expect(result.structuredContent).toHaveProperty('todos');

            // 5. Verify the corrupted todo is handled gracefully (e.g. shows as "(Untitled)")
            // Note: This assertion depends on the specific fix implementation
            const state = await planningServer.callTool('getCurrentState', {});
            interface StateStructure {
                state: {
                    todos: Array<{ title: string }>;
                };
            }
            const structuredState = state.structuredContent as StateStructure;
            const firstTodo = structuredState.state.todos[0];
            expect(firstTodo.title).toBe('(Untitled)');

        } catch (error) {
            // Test fails if it crashes
            console.error('Crashed as expected (before fix):', error);
            throw error;
        }
    });
});
