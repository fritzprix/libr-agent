import type { SimpleTodo } from '../types';

/**
 * Formats a list of todos for display in error messages and responses.
 * Returns a formatted string with each todo on a new line showing ID, checked status, and title.
 * Supports 1-level nesting: subtasks are indented with a bullet point.
 * If the list is empty, returns a placeholder message.
 *
 * @param todos - Array of SimpleTodo objects to format (top-level only)
 * @returns Formatted string representation of the todo list with hierarchy
 *
 * @example
 * ```typescript
 * const todos = [
 *   { id: 1, title: 'Task 1', checked: false, subtasks: [
 *     { id: 2, title: 'Subtask 1.1', checked: true }
 *   ]},
 *   { id: 3, title: 'Task 2', checked: true }
 * ];
 * formatTodosList(todos);
 * // Returns:
 * //   - ID: 1 [unchecked] Task 1
 * //     • ID: 2 [checked] Subtask 1.1
 * //   - ID: 3 [checked] Task 2
 * ```
 */
export function formatTodosList(todos: SimpleTodo[]): string {
  if (todos.length === 0) {
    return '  (no todos)';
  }

  return todos
    .map((t) => {
      const main = `  - ID: ${t.id} [${t.checked ? 'checked' : 'unchecked'}] ${t.title}`;

      if (t.subtasks && t.subtasks.length > 0) {
        const subs = t.subtasks
          .map(
            (s) =>
              `    • ID: ${s.id} [${s.checked ? 'checked' : 'unchecked'}] ${s.title}`,
          )
          .join('\n');
        return `${main}\n${subs}`;
      }

      return main;
    })
    .join('\n');
}
