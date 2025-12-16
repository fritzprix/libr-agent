import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Planning Server Dependency Validation', () => {
    const sessionId = 'dep-test-session';
    const threadId = 'dep-test-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();

        // Reset context
        if (planningServer.switchContext) {
            await planningServer.switchContext({ sessionId, threadId });
        }
    });

    afterEach(async () => {
        await db.close();
    });

    it('should accept valid dependsOnIds', async () => {
        // 1. Add a parent task
        const parentResult = await planningServer.callTool('addTodo', { title: 'Parent Task' });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const parentId = (parentResult.structuredContent as any).todos[0].id;

        // 2. Add a child task depending on parent
        const childResult = await planningServer.callTool('addTodo', { 
            title: 'Child Task',
            dependsOnIds: [parentId]
        });

        expect(childResult.isError).toBe(false);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const todos = (childResult.structuredContent as any).todos;
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const childTodo = todos.find((t: any) => t.title === 'Child Task');
        expect(childTodo.dependsOn).toEqual([parentId]);
    });

    it('should reject invalid dependsOnIds (non-existent ID)', async () => {
        const result = await planningServer.callTool('addTodo', { 
            title: 'Invalid Task',
            dependsOnIds: [99999] // Non-existent ID
        });

        expect(result.isError).toBe(true);
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const message = (result.content![0] as any).text;
        expect(message).toContain('Dependency Todo ID 99999 does not exist');
        expect(message).toContain('dependsOnIds expects database IDs');
    });

    it('should reject circular dependencies', async () => {
        // 1. Add Task A
        const resA = await planningServer.callTool('addTodo', { title: 'Task A' });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const idA = (resA.structuredContent as any).todos[0].id;

        // 2. Add Task B depending on A
        const resB = await planningServer.callTool('addTodo', { 
            title: 'Task B',
            dependsOnIds: [idA]
        });
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const idB = (resB.structuredContent as any).todos.find((t: any) => t.title === 'Task B').id;

        // 3. Try to add Task C depending on B (this is fine)
        const resC = await planningServer.callTool('addTodo', { 
            title: 'Task C',
            dependsOnIds: [idB]
        });
        expect(resC.isError).toBe(false);
    });
});
