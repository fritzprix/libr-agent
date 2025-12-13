import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Planning Server Enhancements', () => {
    const sessionId = 'enhancement-test-session';
    const threadId = 'enhancement-test-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();

        await planningServer.switchContext!({ sessionId, threadId });
    });

    afterEach(async () => {
        await db.close();
    });

    it('should suggest next pending todo when marking a todo as completed', async () => {
        // 1. Add two todos
        await planningServer.callTool('add_todo', { name: 'Task 1' });
        const secondTodo = await planningServer.callTool('add_todo', { name: 'Task 2' });
        const secondTodoId = (secondTodo.structuredContent as any).id;

        // 2. Mark first todo as completed
        // We need to find the ID of the first todo. Since we just cleared DB, it should be the first one.
        const state = await planningServer.callTool('get_current_state', {});
        const todos = (state.structuredContent as any).state.todos;
        const firstTodoId = todos[0].id;

        const result = await planningServer.callTool('mark_todo', { id: firstTodoId, completed: true });

        // 3. Verify response contains "nextActions" suggesting Task 2
        // Note: The specific format of nextActions depends on implementation
        // But we expect it to contain the name of the second todo
        const structured = result.structuredContent as any;
        expect(structured.nextActions).toBeDefined();
        expect(structured.nextActions.length).toBeGreaterThan(0);
        expect(structured.nextActions[0]).toContain('Task 2');
        expect(structured.nextActions[0]).toContain(String(secondTodoId));
    });

    it('should suggest completion message when all todos are done', async () => {
        await planningServer.callTool('add_todo', { name: 'Only Task' });
        const state = await planningServer.callTool('get_current_state', {});
        const firstTodoId = (state.structuredContent as any).state.todos[0].id;

        const result = await planningServer.callTool('mark_todo', { id: firstTodoId, completed: true });

        const structured = result.structuredContent as any;
        expect(structured.nextActions).toBeDefined();
        expect(structured.nextActions[0]).toContain('All todos completed');
    });

    it('should suggest next actions when clearing all todos', async () => {
        await planningServer.callTool('add_todo', { name: 'To be cleared' });

        const result = await planningServer.callTool('clear_todos', {});

        const structured = result.structuredContent as any;
        expect(structured.nextActions).toBeDefined();
        // Since we have no goal, it should suggest creating one
        expect(structured.nextActions[0]).toContain('Create a new goal');
    });

    it('should suggest next actions when partially clearing todos', async () => {
        await planningServer.callTool('add_todo', { name: 'Task 1' });
        await planningServer.callTool('add_todo', { name: 'Task 2' });

        const state = await planningServer.callTool('get_current_state', {});
        const firstId = (state.structuredContent as any).state.todos[0].id;

        const result = await planningServer.callTool('clear_todos', { ids: [firstId] });

        const structured = result.structuredContent as any;
        expect(structured.nextActions).toBeDefined();
        // Should suggest reviewing remaining todos
        expect(structured.nextActions[0]).toContain('Review and prioritize');
    });
});
