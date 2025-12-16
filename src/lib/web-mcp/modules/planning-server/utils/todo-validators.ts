import type { SimpleTodo } from '../types';
import type { PlanningTodo } from '../db';

/**
 * Checks if a todo with the same title already exists in the list.
 * Comparison is case-insensitive and ignores leading/trailing whitespace.
 *
 * @param todos - Array of existing todos to check against
 * @param title - The title of the new todo to validate
 * @returns The duplicate todo if found, otherwise null
 *
 * @example
 * ```typescript
 * const todos = [{ id: 1, title: 'Task 1', status: 'pending' }];
 * const duplicate = checkDuplicateTodo(todos, 'task 1'); // Returns the todo
 * const noDup = checkDuplicateTodo(todos, 'Task 2'); // Returns null
 * ```
 */
export function checkDuplicateTodo(
  todos: SimpleTodo[],
  title: string,
): SimpleTodo | null {
  const normalizedTitle = title.trim().toLowerCase();
  const duplicate = todos.find(
    (t) => t.title.trim().toLowerCase() === normalizedTitle,
  );
  return duplicate ?? null;
}

/**
 * Checks for corrupted todos that are missing the required 'title' property.
 * This can happen due to data corruption or migration issues.
 *
 * @param todos - Array of todos to validate
 * @returns Array of corrupted todos (those missing a valid title), or null if all are valid
 *
 * @example
 * ```typescript
 * const todos = [
 *   { id: 1, title: 'Valid', status: 'pending' },
 *   { id: 2, title: '', status: 'pending' }, // Corrupted
 * ];
 * const corrupted = checkCorruptedTodos(todos); // Returns [{ id: 2, ... }]
 * ```
 */
export function checkCorruptedTodos(todos: SimpleTodo[]): SimpleTodo[] | null {
  const corrupted = todos.filter(
    (t) => !t.title || typeof t.title !== 'string',
  );
  return corrupted.length > 0 ? corrupted : null;
}

/**
 * Validates that a todo exists and belongs to the specified session and thread.
 *
 * @param todo - The todo record to validate (can be undefined)
 * @param sessionId - The expected session ID
 * @param threadId - The expected thread ID
 * @returns true if the todo exists and belongs to the session/thread, false otherwise
 *
 * @example
 * ```typescript
 * const todo = await db.todos.get(42);
 * const isValid = validateTodoExists(todo, 'session1', 'thread1');
 * ```
 */
export function validateTodoExists(
  todo: PlanningTodo | undefined,
  sessionId: string,
  threadId: string,
): boolean {
  if (!todo) {
    return false;
  }
  return todo.sessionId === sessionId && todo.threadId === threadId;
}
