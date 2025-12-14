import type { SimpleTodo } from '../types';

/**
 * Formats a list of todos for display in error messages and responses.
 * Returns a formatted string with each todo on a new line showing ID, status, and name.
 * If the list is empty, returns a placeholder message.
 *
 * @param todos - Array of SimpleTodo objects to format
 * @returns Formatted string representation of the todo list
 *
 * @example
 * ```typescript
 * const todos = [
 *   { id: 1, name: 'Task 1', status: 'pending' },
 *   { id: 2, name: 'Task 2', status: 'completed' }
 * ];
 * formatTodosList(todos);
 * // Returns:
 * //   - ID: 1 [pending] Task 1
 * //   - ID: 2 [completed] Task 2
 * ```
 */
export function formatTodosList(todos: SimpleTodo[]): string {
  if (todos.length === 0) {
    return '  (no todos)';
  }
  return todos.map((t) => `  - ID: ${t.id} [${t.status}] ${t.name}`).join('\n');
}
