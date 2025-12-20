import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';
import { ScratchpadItem } from '../types';

interface TodoItem {
    id: number;
    title: string;
    checked: boolean;
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
        const result = await planningServer.callTool('createGoal', { goal });
        expect(result.structuredContent).toBeDefined();

        const state = await planningServer.callTool('getCurrentState', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.goal).toBe(goal);

        // Verify DB directly
        const dbGoal = await db.goals.where({ sessionId, threadId }).last();
        expect(dbGoal?.content).toBe(goal);
        expect(dbGoal?.isActive).toBe(1);
    });

    it('should persist todos', async () => {
        await planningServer.callTool('addTodo', { title: 'Task 1' });
        await planningServer.callTool('addTodo', { title: 'Task 2' });

        const state = await planningServer.callTool('get_current_state', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.todos).toHaveLength(2);
        expect(structuredState.state.todos[0].title).toBe('Task 1');
        expect(structuredState.state.todos[1].title).toBe('Task 2');

        // Verify DB directly
        const dbTodos = await db.todos.where({ sessionId, threadId }).toArray();
        expect(dbTodos).toHaveLength(2);
    });

    it('should add and clear scratchpad items', async () => {
        const result = await planningServer.callTool('addScratchpad', {
            note: 'Test Note',
        });
        expect(result.isError).toBe(false);
        expect(result.structuredContent).toHaveProperty('scratchpad');
        const { scratchpad } = result.structuredContent as { scratchpad: ScratchpadItem[] };
        expect(scratchpad).toHaveLength(1);
        expect(scratchpad[0].content).toBe('Test Note');

        const id = scratchpad[0].id;
        const clearResult = await planningServer.callTool('clearScratchpad', { id });
        expect(clearResult.isError).toBe(false);
        const { scratchpad: remaining } = clearResult.structuredContent as { scratchpad: ScratchpadItem[] };
        expect(remaining).toHaveLength(0);
    });
    it('should update todo checked status', async () => {
        const addResult = await planningServer.callTool('addTodo', { title: 'Task 1' });
        const { todos } = addResult.structuredContent as AddTodoResult;
        const todoId = todos[0].id;

        await planningServer.callTool('checkTodo', { id: todoId, checked: true });

        const state = await planningServer.callTool('get_current_state', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.todos[0].checked).toBe(true);

        const dbTodo = await db.todos.get(todoId);
        expect(dbTodo?.checked).toBe(true);
    });

    it('should isolate sessions', async () => {
        // Session 1
        await planningServer.switchContext!({ sessionId: 'session1' });
        await planningServer.callTool('createGoal', { goal: 'Goal 1' });

        // Session 2
        await planningServer.switchContext!({ sessionId: 'session2' });
        await planningServer.callTool('createGoal', { goal: 'Goal 2' });

        // Verify Session 1
        await planningServer.switchContext!({ sessionId: 'session1' });
        const state1 = await planningServer.callTool('getCurrentState', {});
        const content1 = state1.structuredContent as PlanningState;
        expect(content1.state.goal).toBe('Goal 1');

        // Verify Session 2
        await planningServer.switchContext!({ sessionId: 'session2' });
        const state2 = await planningServer.callTool('getCurrentState', {});
        const content2 = state2.structuredContent as PlanningState;
        expect(content2.state.goal).toBe('Goal 2');
    });

    it('should support index-based todo operations', async () => {
        // Reset to test session
        await planningServer.switchContext!({ sessionId, threadId });

        // Add multiple todos
        await planningServer.callTool('addTodo', { title: 'First Task' });
        await planningServer.callTool('addTodo', { title: 'Second Task' });
        await planningServer.callTool('addTodo', { title: 'Third Task' });

        // Mark first todo (index 0) as checked using index
        const markResult = await planningServer.callTool('checkTodo', {
            index: 0,
            checked: true,
        });
        expect(markResult.isError).toBe(false);

        // Verify the checked status change
        const state = await planningServer.callTool('getCurrentState', {});
        const structuredState = state.structuredContent as PlanningState;
        expect(structuredState.state.todos[0].checked).toBe(true);
    });

    it('should handle invalid index gracefully', async () => {
        await planningServer.switchContext!({ sessionId, threadId });
        await planningServer.callTool('addTodo', { name: 'Only Task' });

        // Try to check with invalid index (out of range)
        const result = await planningServer.callTool('checkTodo', {
            index: 5,
            checked: true,
        });

        expect(result.isError).toBe(true);
        expect(result.content).toBeDefined();
        expect(result.content!.length).toBeGreaterThan(0);
        const firstContent = result.content![0];
        if (firstContent && firstContent.type === 'text') {
            const messageText = firstContent.text;
            expect(messageText).toContain('not found');
            expect(messageText).toContain('Valid indexes');
        }
    });

    it('should require either id or index', async () => {
        await planningServer.switchContext!({ sessionId, threadId });
        await planningServer.callTool('addTodo', { name: 'Test Task' });

        // Try to check without id or index
        const checkResult = await planningServer.callTool('checkTodo', {
            checked: true,
        });
        expect(checkResult.isError).toBe(true);
        expect(checkResult.content).toBeDefined();
        expect(checkResult.content!.length).toBeGreaterThan(0);
        const updateContent = checkResult.content![0];
        if (updateContent && updateContent.type === 'text') {
            const updateText = updateContent.text;
            expect(updateText).toContain('Either "id" or "index" must be provided');
        }

    });
});
