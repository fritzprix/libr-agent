import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Planning Server Persistence', () => {
    const sessionId = 'test-session';
    const threadId = 'test-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();
        await db.memos.clear();

        // Reset context
        if (planningServer.switchContext) {
            await planningServer.switchContext({ sessionId, threadId });
        }
    });

    afterEach(async () => {
        await db.close();
    });

    it('should persist a goal', async () => {
        const goal = 'Learn Rust';
        const result = await planningServer.callTool('create_goal', { goal });
        expect(result.structuredContent).toBeDefined();

        const state = await planningServer.callTool('get_current_state', {});
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const structuredState = (state as any).structuredContent;
        expect(structuredState.goal).toBe(goal);

        // Verify DB directly
        const dbGoal = await db.goals.where({ sessionId, threadId }).last();
        expect(dbGoal?.content).toBe(goal);
        expect(dbGoal?.isActive).toBe(1);
    });

    it('should persist todos', async () => {
        await planningServer.callTool('add_todo', { name: 'Task 1' });
        await planningServer.callTool('add_todo', { name: 'Task 2' });

        const state = await planningServer.callTool('get_current_state', {});
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const structuredState = (state as any).structuredContent;
        expect(structuredState.todos).toHaveLength(2);
        expect(structuredState.todos[0].name).toBe('Task 1');
        expect(structuredState.todos[1].name).toBe('Task 2');

        // Verify DB directly
        const dbTodos = await db.todos.where({ sessionId, threadId }).toArray();
        expect(dbTodos).toHaveLength(2);
    });

    it('should update todo status', async () => {
        const addResult = await planningServer.callTool('add_todo', { name: 'Task 1' });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const todoId = (addResult as any).structuredContent.todos[0].id;

        await planningServer.callTool('mark_todo', { id: todoId, completed: true });

        const state = await planningServer.callTool('get_current_state', {});
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const structuredState = (state as any).structuredContent;
        expect(structuredState.todos[0].status).toBe('completed');

        const dbTodo = await db.todos.get(todoId);
        expect(dbTodo?.status).toBe('completed');
    });

    it('should isolate sessions', async () => {
        // Session 1
        await planningServer.switchContext!({ sessionId: 'session1' });
        await planningServer.callTool('create_goal', { goal: 'Goal 1' });

        // Session 2
        await planningServer.switchContext!({ sessionId: 'session2' });
        await planningServer.callTool('create_goal', { goal: 'Goal 2' });

        // Verify Session 1
        await planningServer.switchContext!({ sessionId: 'session1' });
        const state1 = await planningServer.callTool('get_current_state', {});
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        expect((state1 as any).structuredContent.goal).toBe('Goal 1');

        // Verify Session 2
        await planningServer.switchContext!({ sessionId: 'session2' });
        const state2 = await planningServer.callTool('get_current_state', {});
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        expect((state2 as any).structuredContent.goal).toBe('Goal 2');
    });
});
