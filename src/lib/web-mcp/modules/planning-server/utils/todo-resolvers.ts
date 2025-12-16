import { db, type PlanningTodo } from '../db';
import type { SimpleTodo } from '../types';

interface LegacyTodo extends PlanningTodo {
  name?: string;
}

/**
 * Resolves a todo ID from either an explicit id or an index parameter.
 * When an index is provided, it looks up the todo at that position (0-based).
 * When an id is provided, it returns that id directly.
 *
 * @param sessionId - The session ID for scoping the query
 * @param threadId - The thread ID for scoping the query
 * @param params - Object containing either 'id' or 'index'
 * @returns Object containing the resolved todo ID (or undefined if not found) and the full todos list
 *
 * @example
 * ```typescript
 * // Resolve by index
 * const result = await resolveTodoId('session1', 'thread1', { index: 0 });
 * // result.id will be the ID of the first todo
 *
 * // Resolve by ID
 * const result = await resolveTodoId('session1', 'thread1', { id: 42 });
 * // result.id will be 42
 * ```
 */
export async function resolveTodoId(
  sessionId: string,
  threadId: string,
  params: { id?: number; index?: number },
): Promise<{ id: number | undefined; todos: SimpleTodo[] }> {
  const todos = await db.todos.where({ sessionId, threadId }).sortBy('order');

  const simpleTodos: SimpleTodo[] = todos.map((t) => ({
    id: t.id!,
    title:
      typeof t.title === 'string' && t.title
        ? t.title
        : (t as unknown as LegacyTodo).name || '(Untitled)',
    description: t.description,
    checked: t.checked,
    summary: t.summary,
    priority: t.priority,
    dependsOn: t.dependsOn,
  }));

  if (params.id !== undefined) {
    return { id: params.id, todos: simpleTodos };
  }

  if (params.index !== undefined) {
    if (params.index >= 0 && params.index < simpleTodos.length) {
      return { id: simpleTodos[params.index].id, todos: simpleTodos };
    }
    return { id: undefined, todos: simpleTodos };
  }

  return { id: undefined, todos: simpleTodos };
}
