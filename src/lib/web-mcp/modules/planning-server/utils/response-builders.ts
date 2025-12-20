import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';
import type { MCPResult } from '@/lib/mcp-types';
import type { SimpleTodo } from '../types';
import { formatTodosList } from './formatters';

/**
 * Builds an error response for when a todo is not found by ID or index.
 *
 * @param params - The parameters used to search for the todo (id or index)
 * @param allTodos - The complete list of todos for context
 * @returns MCPResult with error details and suggestions
 */
export function buildTodoNotFoundError(
  params: { id?: number; index?: number },
  allTodos: SimpleTodo[],
): MCPResult<unknown> {
  const validIds = allTodos.map((t) => t.id);
  const uncheckedCount = allTodos.filter((t) => !t.checked).length;
  const checkedCount = allTodos.filter((t) => t.checked).length;

  const identifier =
    params.id !== undefined
      ? `ID ${params.id}`
      : params.index !== undefined
        ? `index ${params.index}`
        : 'no identifier';

  const suggestions = [
    'Use getCurrentState to see all todos with their IDs and indexes',
    'Specify either "id" (database ID) or "index" (0-based position)',
  ];

  return new MCPResponseBuilder({
    requestedId: params.id,
    requestedIndex: params.index,
    validIds,
    validIndexRange: `0-${allTodos.length - 1}`,
    totalCount: allTodos.length,
    unchecked: uncheckedCount,
    checked: checkedCount,
    todos: allTodos,
  })
    .withMessage(
      `Todo with ${identifier} not found.\n\n` +
        `Current todos (${allTodos.length} total):\n` +
        `  - Unchecked: ${uncheckedCount}\n` +
        `  - Checked: ${checkedCount}\n` +
        `  - Valid IDs: [${validIds.join(', ') || 'none'}]\n` +
        `  - Valid indexes: ${allTodos.length > 0 ? `0-${allTodos.length - 1}` : 'none'}`,
    )
    .withSuggestions(suggestions)
    .asError(WebMCPErrorCodes.PLANNING.TODO_NOT_FOUND);
}

/**
 * Builds an error response for duplicate todo detection.
 *
 * @param duplicate - The existing todo that was found
 * @param todos - The complete list of todos
 * @returns MCPResult with error details and suggestions
 */
export function buildDuplicateTodoError(
  duplicate: SimpleTodo,
  todos: SimpleTodo[],
): MCPResult<unknown> {
  return new MCPResponseBuilder({
    success: false,
    duplicateId: duplicate.id,
    existingTodo: duplicate,
    todos,
  })
    .withMessage(
      `Duplicate todo detected.\n\n` +
        `A todo with similar content already exists:\n` +
        `  - ID: ${duplicate.id} [${duplicate.checked ? 'checked' : 'unchecked'}] ${duplicate.title}\n\n` +
        `Current todos:\n${formatTodosList(todos)}`,
    )
    .withSuggestions([
      'Use a different title for the new todo',
      `Check the existing todo with checkTodo(id=${duplicate.id})`,
      `Clear the existing todo if it's truly complete`,
    ])
    .asError(WebMCPErrorCodes.PLANNING.DUPLICATE_TODO);
}

/**
 * Builds an error response for corrupted todos (missing title property).
 *
 * @param corruptedTodos - Array of todos missing the title property
 * @returns MCPResult with error details
 */
export function buildCorruptedTodosError(
  corruptedTodos: SimpleTodo[],
): MCPResult<unknown> {
  return new MCPResponseBuilder({
    success: false,
    corruptedTodos,
  })
    .withMessage(
      `Data Corruption Detected.\n\n` +
        `The following todo items are missing a 'title' property and must be fixed or removed:\n` +
        corruptedTodos
          .map(
            (t) => `  - ID: ${t.id} (${t.checked ? 'Checked' : 'Unchecked'})`,
          )
          .join('\n') +
        `\n\nPlease use 'clearTodos' with these IDs to remove them.`,
    )
    .asError(WebMCPErrorCodes.INTERNAL_ERROR);
}

/**
 * Builds an error response for empty or whitespace-only goal/todo titles.
 *
 * @param type - Whether this is for a 'goal' or 'todo'
 * @returns MCPResult with error details and suggestions
 */
export function buildEmptyTitleError(
  type: 'goal' | 'todo',
): MCPResult<unknown> {
  const entityType = type === 'goal' ? 'Goal' : 'Todo';

  return new MCPResponseBuilder({})
    .withMessage(
      `${entityType} title cannot be empty or whitespace-only.\n\n` +
        `The provided title must contain at least one non-whitespace character.`,
    )
    .withSuggestions([
      `Provide a descriptive title that clearly identifies the ${type}`,
      'Titles must contain at least one non-whitespace character',
      `Example: "${type === 'goal' ? 'Implement user authentication' : 'Write unit tests for auth module'}"`,
    ])
    .asError(WebMCPErrorCodes.PLANNING.EMPTY_NAME);
}
