import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer, { ScratchpadItem } from '../server';

interface TodoItem {
    id: number;
    name: string;
    status: 'pending' | 'completed';
}

interface PlanningState {
    state: {
        goal: string | null;
        todos: TodoItem[];
        scratchpad: ScratchpadItem[];
    };
}

interface AddTodoResult {
    todos: TodoItem[];
}

describe('Planning Server Persistence', () => {
    const sessionId = 'test-session';
    const threadId = 'test-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();
        await db.scratchpad.clear();

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
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.goal).toBe(goal);

        // Verify DB directly
        const dbGoal = await db.goals.where({ sessionId, threadId }).last();
        expect(dbGoal?.content).toBe(goal);
        expect(dbGoal?.isActive).toBe(1);
    });

    it('should persist todos', async () => {
        await planningServer.callTool('add_todo', { name: 'Task 1' });
        await planningServer.callTool('add_todo', { name: 'Task 2' });

        const state = await planningServer.callTool('get_current_state', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.todos).toHaveLength(2);
        expect(structuredState.state.todos[0].name).toBe('Task 1');
        expect(structuredState.state.todos[1].name).toBe('Task 2');

        // Verify DB directly
        const dbTodos = await db.todos.where({ sessionId, threadId }).toArray();
        expect(dbTodos).toHaveLength(2);
    });

    it('should add and clear scratchpad items', async () => {
        const result = await planningServer.callTool('add_scratchpad', {
            note: 'Test Note',
        });
        expect(result.isError).toBe(false);
        expect(result.structuredContent).toHaveProperty('scratchpad');
        const { scratchpad } = result.structuredContent as { scratchpad: ScratchpadItem[] };
        expect(scratchpad).toHaveLength(1);
        expect(scratchpad[0].content).toBe('Test Note');

        const id = scratchpad[0].id;
        const clearResult = await planningServer.callTool('clear_scratchpad', { id });
        expect(clearResult.isError).toBe(false);
        const { scratchpad: remaining } = clearResult.structuredContent as { scratchpad: ScratchpadItem[] };
        expect(remaining).toHaveLength(0);
    });
    it('should update todo status', async () => {
        const addResult = await planningServer.callTool('add_todo', { name: 'Task 1' });
        const { todos } = addResult.structuredContent as AddTodoResult;
        const todoId = todos[0].id;

        await planningServer.callTool('mark_todo', { id: todoId, completed: true });

        const state = await planningServer.callTool('get_current_state', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.todos[0].status).toBe('completed');

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
        const content1 = state1.structuredContent as PlanningState;
        expect(content1.state.goal).toBe('Goal 1');

        // Verify Session 2
        await planningServer.switchContext!({ sessionId: 'session2' });
        const state2 = await planningServer.callTool('get_current_state', {});
        const content2 = state2.structuredContent as PlanningState;
        expect(content2.state.goal).toBe('Goal 2');
    });
});
