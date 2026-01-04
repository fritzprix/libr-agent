import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import 'fake-indexeddb/auto';
import { db } from '../db';
import planningServer from '../server';

describe('Subtask Validation', () => {
    const sessionId = 'subtask-validation-session';
    const threadId = 'subtask-validation-thread';

    beforeEach(async () => {
        await db.open();
        await db.goals.clear();
        await db.todos.clear();

        await planningServer.switchContext!({ sessionId, threadId });
    });

    afterEach(async () => {
        await db.close();
    });

    it('should reject addTodo when a subtask has an empty title', async () => {
        const result = await planningServer.callTool('addTodo', {
            title: 'Valid Parent',
            subtasks: [
                { title: 'Valid Subtask 1' },
                { title: '' }, // Invalid
            ]
        });

        expect(result.isError).toBe(true);
        expect(result.content).toBeDefined();
        interface ContentItem {
            text: string;
        }
        const text = (result.content![0] as ContentItem).text;
        expect(text).toContain('Subtask at index 1 has an empty title');
        expect(text).toContain('Please provide a valid title');

        // Verify nothing was added
        const state = await planningServer.callTool('getCurrentState', {});
        interface StateStructure {
            state: {
                todos: unknown[];
            };
        }
        const todos = (state.structuredContent as StateStructure).state.todos;
        expect(todos.length).toBe(0);
    });

    it('should reject addTodo when a subtask has a whitespace-only title', async () => {
        const result = await planningServer.callTool('addTodo', {
            title: 'Valid Parent',
            subtasks: [
                { title: '   ' }, // Invalid
            ]
        });

        expect(result.isError).toBe(true);
        expect(result.content).toBeDefined();
        interface ContentItem {
            text: string;
        }
        const text = (result.content![0] as ContentItem).text;
        expect(text).toContain('Subtask at index 0 has an empty title');
    });

    it('should accept addTodo when all subtasks have valid titles', async () => {
        const result = await planningServer.callTool('addTodo', {
            title: 'Valid Parent',
            subtasks: [
                { title: 'Valid Subtask 1' },
                { title: 'Valid Subtask 2' },
            ]
        });

        expect(result.isError).toBeFalsy();

        // Verify added
        const state = await planningServer.callTool('getCurrentState', {});
        interface Subtask {
            title: string;
        }
        interface Todo {
            subtasks: Subtask[];
        }
        interface StateStructure {
            state: {
                todos: Todo[];
            };
        }
        const todos = (state.structuredContent as StateStructure).state.todos;
        expect(todos.length).toBe(1);
        expect(todos[0].subtasks.length).toBe(2);
        expect(todos[0].subtasks[0].title).toBe('Valid Subtask 1');
        expect(todos[0].subtasks[1].title).toBe('Valid Subtask 2');
    });
});
