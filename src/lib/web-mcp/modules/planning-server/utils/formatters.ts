import type { SimpleTodo } from '../types';

/**
 * Formats a list of todos for display in error messages and responses.
 * Returns a formatted string with each todo on a new line showing ID, checked status, and name.
 * If the list is empty, returns a placeholder message.
 *
 * @param todos - Array of SimpleTodo objects to format
 * @returns Formatted string representation of the todo list
 *
 * @example
 * ```typescript
 * const todos = [
 *   { id: 1, name: 'Task 1', checked: false },
 *   { id: 2, name: 'Task 2', checked: true }
 * ];
 * formatTodosList(todos);
 * // Returns:
 * //   - ID: 1 [unchecked] Task 1
 * //   - ID: 2 [checked] Task 2
 * ```
 */
export function formatTodosList(todos: SimpleTodo[]): string {
  if (todos.length === 0) {
    return '  (no todos)';
  }
  return todos
    .map(
      (t) =>
        `  - ID: ${t.id} [${t.checked ? 'checked' : 'unchecked'}] ${t.name}`,
    )
    .join('\n');
}
