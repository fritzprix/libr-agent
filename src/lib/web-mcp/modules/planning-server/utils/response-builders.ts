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
  const pendingCount = allTodos.filter((t) => t.status === 'pending').length;
  const completedCount = allTodos.filter(
    (t) => t.status === 'completed',
  ).length;

  const identifier =
    params.id !== undefined
      ? `ID ${params.id}`
      : params.index !== undefined
        ? `index ${params.index}`
        : 'no identifier';

  const suggestions = [
    'Use get_current_state to see all todos with their IDs and indexes',
    'Specify either "id" (database ID) or "index" (0-based position)',
  ];

  return new MCPResponseBuilder({
    requestedId: params.id,
    requestedIndex: params.index,
    validIds,
    validIndexRange: `0-${allTodos.length - 1}`,
    totalCount: allTodos.length,
    pending: pendingCount,
    completed: completedCount,
    todos: allTodos,
  })
    .withMessage(
      `Todo with ${identifier} not found.\n\n` +
        `Current todos (${allTodos.length} total):\n` +
        `  - Pending: ${pendingCount}\n` +
        `  - Completed: ${completedCount}\n` +
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
        `  - ID: ${duplicate.id} [${duplicate.status}] ${duplicate.name}\n\n` +
        `Current todos:\n${formatTodosList(todos)}`,
    )
    .withSuggestions([
      'Use a different name for the new todo',
      `Update the existing todo with update_todo(id=${duplicate.id})`,
      `Mark the existing todo as completed if needed`,
    ])
    .asError(WebMCPErrorCodes.PLANNING.DUPLICATE_TODO);
}

/**
 * Builds an error response for corrupted todos (missing name property).
 *
 * @param corruptedTodos - Array of todos missing the name property
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
        `The following todo items are missing a 'name' property and must be fixed or removed:\n` +
        corruptedTodos
          .map((t) => `  - ID: ${t.id} (Status: ${t.status})`)
          .join('\n') +
        `\n\nPlease use 'clear_todos' with these IDs to remove them.`,
    )
    .asError(WebMCPErrorCodes.INTERNAL_ERROR);
}
