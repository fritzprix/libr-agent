import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';
import type { MCPResult } from '@/lib/mcp-types';
import type { SimpleTodo } from '../types';
import { formatTodosList } from './formatters';
import type { CircularDependencyError } from './dependency-validator';

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

/**
 * Builds an error response for empty or whitespace-only goal/todo names.
 *
 * @param type - Whether this is for a 'goal' or 'todo'
 * @returns MCPResult with error details and suggestions
 */
export function buildEmptyNameError(type: 'goal' | 'todo'): MCPResult<unknown> {
  const entityType = type === 'goal' ? 'Goal' : 'Todo';

  return new MCPResponseBuilder({})
    .withMessage(
      `${entityType} name cannot be empty or whitespace-only.\n\n` +
        `The provided name must contain at least one non-whitespace character.`,
    )
    .withSuggestions([
      `Provide a descriptive name that clearly identifies the ${type}`,
      'Names must contain at least one non-whitespace character',
      `Example: "${type === 'goal' ? 'Implement user authentication' : 'Write unit tests for auth module'}"`,
    ])
    .asError(WebMCPErrorCodes.PLANNING.EMPTY_NAME);
}

/**
 * Builds an error response for invalid dependency references.
 *
 * @param invalidId - The non-existent todo ID that was referenced
 * @param allTodos - The complete list of existing todos
 * @returns MCPResult with error details and suggestions
 */
export function buildInvalidDependencyError(
  invalidId: number,
  allTodos: SimpleTodo[],
): MCPResult<unknown> {
  const validIds = allTodos.map((t) => t.id);
  const validIdsStr = validIds.length > 0 ? validIds.join(', ') : 'none';

  return new MCPResponseBuilder({
    invalidId,
    validIds,
    todos: allTodos,
  })
    .withMessage(
      `Dependency Todo ID ${invalidId} does not exist.\n\n` +
        `Valid todo IDs in this session: [${validIdsStr}]\n` +
        `Total todos: ${allTodos.length}`,
    )
    .withSuggestions([
      'Use get_current_state to see all available todos with their IDs',
      'Ensure all dependency IDs reference existing todos in this session',
      'Remove the invalid dependency ID from the dependsOn array',
    ])
    .asError(WebMCPErrorCodes.PLANNING.INVALID_DEPENDENCY);
}

/**
 * Builds an error response for self-dependency (todo depending on itself).
 *
 * @param todoId - The ID of the todo that tried to depend on itself
 * @returns MCPResult with error details and suggestions
 */
export function buildSelfDependencyError(todoId: number): MCPResult<unknown> {
  return new MCPResponseBuilder({ todoId })
    .withMessage(
      `Todo cannot depend on itself (ID: ${todoId}).\n\n` +
        `A todo's dependencies must reference other todos, not itself.`,
    )
    .withSuggestions([
      'Remove the self-reference from the dependsOn array',
      'A todo can only depend on other todos, not itself',
      'Check that the dependency IDs are correct',
    ])
    .asError(WebMCPErrorCodes.PLANNING.SELF_DEPENDENCY);
}

/**
 * Builds an error response for circular dependency detection.
 *
 * @param error - The circular dependency error details including the cycle path
 * @returns MCPResult with error details and suggestions
 */
export function buildCircularDependencyError(
  error: CircularDependencyError,
): MCPResult<unknown> {
  const cycleDescription = error.cycle.join(' → ');

  return new MCPResponseBuilder({
    todoId: error.todoId,
    cycle: error.cycle,
  })
    .withMessage(
      `Circular dependency detected: ${cycleDescription}\n\n` +
        `This dependency creates a cycle in the task graph. ` +
        `Todo ${error.todoId} cannot transitively depend on itself.`,
    )
    .withSuggestions([
      'Remove one of the dependencies in the cycle to break the loop',
      'Reorganize your task dependencies to form a directed acyclic graph (DAG)',
      'A todo cannot directly or indirectly depend on itself',
      'Consider breaking the circular tasks into smaller, independent subtasks',
    ])
    .asError(WebMCPErrorCodes.PLANNING.CIRCULAR_DEPENDENCY);
}
