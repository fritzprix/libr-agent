import { db, type PlanningTodo } from '../db';
import type { SimpleTodo } from '../types';

interface LegacyTodo extends PlanningTodo {
  name?: string;
}

/**
 * Resolves a todo ID from either an explicit id or an index parameter.
 * When an index is provided, it looks up the todo at that position (0-based) in the top-level list.
 * When an id is provided, it returns that id directly.
 * Returns hierarchical todos with subtasks nested.
 *
 * @param sessionId - The session ID for scoping the query
 * @param threadId - The thread ID for scoping the query
 * @param params - Object containing either 'id' or 'index'
 * @returns Object containing the resolved todo ID (or undefined if not found) and the full todos list with hierarchy
 *
 * @example
 * ```typescript
 * // Resolve by index (top-level only)
 * const result = await resolveTodoId('session1', 'thread1', { index: 0 });
 * // result.id will be the ID of the first top-level todo
 *
 * // Resolve by ID (can be parent or child)
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

  const allTodos: SimpleTodo[] = todos.map((t) => ({
    id: t.id!,
    title:
      typeof t.title === 'string' && t.title
        ? t.title
        : (t as unknown as LegacyTodo).name || '(Untitled)',
    description: t.description,
    checked: t.checked,
    summary: t.summary,
    priority: t.priority,
    parentId: t.parentId,
  }));

  // Build hierarchy
  const topLevel = allTodos.filter((t) => !t.parentId);
  const children = allTodos.filter((t) => t.parentId);

  const hierarchicalTodos: SimpleTodo[] = topLevel.map((parent) => ({
    ...parent,
    subtasks: children.filter((c) => c.parentId === parent.id),
  }));

  if (params.id !== undefined) {
    return { id: params.id, todos: hierarchicalTodos };
  }

  if (params.index !== undefined) {
    // Index refers to top-level todos only
    if (params.index >= 0 && params.index < hierarchicalTodos.length) {
      return {
        id: hierarchicalTodos[params.index].id,
        todos: hierarchicalTodos,
      };
    }
    return { id: undefined, todos: hierarchicalTodos };
  }

  return { id: undefined, todos: hierarchicalTodos };
}
