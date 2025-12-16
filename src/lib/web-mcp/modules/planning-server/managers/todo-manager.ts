import { createMCPStructuredToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { db, type PlanningTodo } from '../db';
import type { SimpleTodo, AddToDoOutput, BaseOutput } from '../types';
import { resolveTodoId } from '../utils/todo-resolvers';
import {
  checkDuplicateTodo,
  checkCorruptedTodos,
  validateTodoExists,
} from '../utils/todo-validators';
import {
  buildTodoNotFoundError,
  buildDuplicateTodoError,
  buildCorruptedTodosError,
  buildEmptyTitleError,
  buildInvalidDependencyError,
  buildCircularDependencyError,
} from '../utils/response-builders';
import { detectCircularDependency } from '../utils/dependency-validator';

// Helper interface for backward compatibility
interface LegacyTodo extends PlanningTodo {
  name?: string;
}

/**
 * Manages todo operations including CRUD, validation, dependency tracking, and progress calculation.
 * Handles complex logic like duplicate detection, unblocked todo calculation, and ID/index resolution.
 *
 * @internal
 */
export class TodoManager {
  constructor(
    private sessionId: string,
    private threadId: string,
  ) {}

  /**
   * Retrieves all todos for the current session/thread, sorted by order field.
   * Converts database records to SimpleTodo format.
   *
   * @returns Array of SimpleTodo objects
   */
  async getTodosList(): Promise<SimpleTodo[]> {
    const todos = await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('order');

    return todos.map((t) => ({
      id: t.id!,
      title:
        typeof t.title === 'string' && t.title
          ? t.title
          : (t as unknown as LegacyTodo).name || '(Untitled)', // Fallback to name for legacy data
      description: t.description,
      checked: t.checked,
      summary: t.summary,
      priority: t.priority,
      dependsOn: t.dependsOn,
    }));
  }

  /**
   * Adds a new todo with duplicate detection and validation.
   * Checks for corrupted todos and duplicate names before adding.
   *
   * @param title - The title/summary of the todo
   * @param description - Optional detailed description
   * @param priority - Optional priority level (low, medium, high)
   * @param dependsOn - Optional array of todo IDs this todo depends on
   * @param activeGoalContent - Optional active goal content for context in response
   * @returns MCPResult with the new todo and updated list
   */
  async addTodo(
    title: string,
    description?: string,
    priority?: 'low' | 'medium' | 'high',
    dependsOn?: number[],
    activeGoalContent?: string | null,
  ): Promise<MCPResult<AddToDoOutput>> {
    // Validation: Title cannot be empty or whitespace-only
    if (!title || title.trim() === '') {
      return buildEmptyTitleError('todo') as MCPResult<AddToDoOutput>;
    }

    const todos = await this.getTodosList();

    // Check for corrupted todos (missing title)
    const corruptedTodos = checkCorruptedTodos(todos);
    if (corruptedTodos) {
      return buildCorruptedTodosError(
        corruptedTodos,
      ) as MCPResult<AddToDoOutput>;
    }

    // Check for duplicate todos (case-insensitive, trimmed)
    const duplicate = checkDuplicateTodo(todos, title);
    if (duplicate) {
      return buildDuplicateTodoError(
        duplicate,
        todos,
      ) as MCPResult<AddToDoOutput>;
    }

    // Validation: Dependency IDs must exist and not create cycles
    if (dependsOn && dependsOn.length > 0) {
      // Check all dependency IDs exist
      for (const depId of dependsOn) {
        const exists = todos.some((t) => t.id === depId);
        if (!exists) {
          return buildInvalidDependencyError(
            depId,
            todos,
          ) as MCPResult<AddToDoOutput>;
        }
      }

      // Check for circular dependencies
      // Use the next available ID for validation (the ID that will be assigned)
      const nextId = await this.getNextTodoId();
      const cycleError = detectCircularDependency(todos, nextId, dependsOn);
      if (cycleError) {
        return buildCircularDependencyError(
          cycleError,
        ) as MCPResult<AddToDoOutput>;
      }
    }

    const order = todos.length;

    const id = await db.todos.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      title,
      description,
      checked: false,
      priority,
      dependsOn,
      order,
      createdAt: Date.now(),
    });

    const newTodos = await this.getTodosList();
    const uncheckedCount = newTodos.filter((t) => !t.checked).length;
    const checkedCount = newTodos.filter((t) => t.checked).length;

    let message = `Todo added: "${title}" (ID: ${id})`;
    if (activeGoalContent) {
      message += `\n\nGoal: "${activeGoalContent}"`;
    }
    message += `\n\nCurrent progress:\n  - Total: ${newTodos.length} todos\n  - Unchecked: ${uncheckedCount}\n  - Checked: ${checkedCount}`;

    return new MCPResponseBuilder({
      success: true,
      id,
      todo: { id, title, description, checked: false, priority, dependsOn },
      todos: newTodos,
      summary: {
        total: newTodos.length,
        unchecked: uncheckedCount,
        checked: checkedCount,
      },
    })
      .withMessage(message)
      .withNextActions(['Use checkTodo when this task is done'])
      .asSuccess();
  }

  /**
   * Marks a todo as checked or unchecked. Automatically detects and reports unblocked todos.
   * Calculates progress and suggests next actions.
   *
   * @param params - Object containing either 'id' or 'index' to identify the todo
   * @param checked - true to mark as checked, false to mark as unchecked
   * @param summary - Optional summary of what was accomplished
   * @returns MCPResult with updated todo, progress, and next action suggestions
   */
  async checkTodo(
    params: { id?: number; index?: number },
    checked: boolean = true,
    summary?: string,
  ): Promise<MCPResult<unknown>> {
    // Resolve id from either id or index
    const { id: resolvedId, todos: allTodos } = await resolveTodoId(
      this.sessionId,
      this.threadId,
      params,
    );

    if (resolvedId === undefined) {
      return buildTodoNotFoundError(params, allTodos);
    }

    const todo = await db.todos.get(resolvedId);
    if (!validateTodoExists(todo, this.sessionId, this.threadId)) {
      return buildTodoNotFoundError(params, allTodos);
    }

    const updates: { checked: boolean; summary?: string } = {
      checked,
    };
    if (summary !== undefined) {
      updates.summary = summary || undefined;
    }

    await db.todos.update(resolvedId, updates);
    const updatedTodoRecord = await db.todos.get(resolvedId);
    const todos = await this.getTodosList();

    const simpleTodo: SimpleTodo = {
      id: updatedTodoRecord!.id!,
      title:
        updatedTodoRecord!.title ||
        (updatedTodoRecord as unknown as LegacyTodo).name ||
        '(Untitled)',
      description: updatedTodoRecord!.description,
      checked: updatedTodoRecord!.checked,
      summary: updatedTodoRecord!.summary,
      priority: updatedTodoRecord!.priority,
      dependsOn: updatedTodoRecord!.dependsOn,
    };

    // Find unblocked todos if this was checked
    const unblockedTodos: SimpleTodo[] = [];
    if (checked) {
      for (const t of todos) {
        // Check if this todo has dependencies including the one just checked
        if (t.dependsOn && t.dependsOn.includes(resolvedId)) {
          // Check if all dependencies are now checked
          const allDepsChecked = t.dependsOn.every((depId) => {
            const dep = todos.find((d) => d.id === depId);
            return dep?.checked === true;
          });

          // If all deps are checked and this todo is not checked, it's unblocked
          if (allDepsChecked && !t.checked) {
            unblockedTodos.push(t);
          }
        }
      }
    }

    // Calculate progress
    const checkedCount = todos.filter((t) => t.checked).length;
    const progress = Math.round((checkedCount / todos.length) * 100);

    const identifier =
      params.index !== undefined
        ? `index ${params.index} (ID: ${resolvedId})`
        : `ID ${resolvedId}`;

    let message = `Todo ${identifier} marked as ${checked ? 'checked' : 'unchecked'}.\n\n`;
    message += `Progress: ${checkedCount}/${todos.length} (${progress}%)`;

    if (simpleTodo.summary) {
      message += `\n\nSummary: "${simpleTodo.summary}"`;
    }

    if (unblockedTodos.length > 0) {
      message += `\n\nUnblocked todos:\n${unblockedTodos.map((t) => `  - [${t.id}] ${t.title}`).join('\n')}`;
    }

    const builder = new MCPResponseBuilder({
      success: true,
      id: resolvedId,
      checked,
      todo: simpleTodo,
      todos,
      progress: {
        checked: checkedCount,
        total: todos.length,
        percentage: progress,
      },
      unblockedTodos: unblockedTodos.map((t) => ({ id: t.id, title: t.title })),
    });

    // Identify next action guidance
    const nextActions: string[] = [];

    if (unblockedTodos.length > 0) {
      nextActions.push(
        `Begin work on unblocked todo: "${unblockedTodos[0].title}" (ID: ${unblockedTodos[0].id})`,
      );
    } else {
      const nextUnchecked = todos.find(
        (t) => !t.checked && t.id !== resolvedId,
      );
      if (nextUnchecked) {
        nextActions.push(
          `Proceed to next todo: "${nextUnchecked.title}" (ID: ${nextUnchecked.id})`,
        );
      } else if (checkedCount === todos.length) {
        nextActions.push(
          "All todos checked! Consider using 'critiqueAndReflection' to review your work before finishing.",
        );
      } else {
        nextActions.push(
          'Check the todo list for remaining blocked or available items.',
        );
      }
    }

    return builder
      .withMessage(message)
      .withNextActions(nextActions)
      .asSuccess();
  }

  /**
   * Clears todos by ID or clears all todos if no IDs provided.
   *
   * @param ids - Optional array of todo IDs to clear. If omitted, clears all todos.
   * @param activeGoalContent - Optional active goal content for context in response
   * @returns MCPResult with clear status and remaining todos count
   */
  async clearTodos(
    ids?: number[],
    activeGoalContent?: string | null,
  ): Promise<MCPResult<BaseOutput>> {
    const todos = await this.getTodosList();

    if (!ids || ids.length === 0) {
      const clearedCount = todos.length;
      await db.todos
        .where({ sessionId: this.sessionId, threadId: this.threadId })
        .delete();

      const msg =
        clearedCount > 0
          ? `All ${clearedCount} todo(s) cleared`
          : 'No todos to clear.';

      const nextActions: string[] = [];
      if (activeGoalContent) {
        nextActions.push(
          'Review the current goal and add new todos to proceed.',
        );
      } else {
        nextActions.push('Create a new goal to start a new planning session.');
      }

      return new MCPResponseBuilder({ success: true })
        .withMessage(
          `${msg}\nSession todos reset. Current goal: ${
            activeGoalContent ? activeGoalContent : '(none)'
          }`,
        )
        .withNextActions(nextActions)
        .asSuccess();
    }

    const initialCount = todos.length;
    // Verify IDs belong to session
    const validIds = todos.map((t) => t.id);
    const idsToDelete = ids.filter((id) => validIds.includes(id));

    if (idsToDelete.length === 0) {
      return createMCPStructuredToolResult<BaseOutput>(
        `No todos found with the specified IDs: ${ids.join(
          ', ',
        )}\nAvailable IDs: ${
          initialCount > 0 ? validIds.join(', ') : '(none)'
        }`,
        { success: false },
      );
    }

    const todosToDelete = todos.filter((t) => idsToDelete.includes(t.id));
    await db.todos.bulkDelete(idsToDelete);

    const remainingTodos = await this.getTodosList();
    const removedCount = idsToDelete.length;
    const clearedNames = todosToDelete.map((t) => t.title).join(', ');

    const nextActions: string[] = [];
    const hasUnchecked = remainingTodos.some((t) => !t.checked);

    if (!hasUnchecked) {
      nextActions.push(
        "No unchecked todos remain! Consider using 'critiqueAndReflection' to review your work before finishing.",
      );
    } else {
      nextActions.push('Review and prioritize the remaining todos.');
    }

    return new MCPResponseBuilder({ success: true })
      .withMessage(
        `Cleared ${removedCount} todo(s): ${clearedNames}\nRemaining todos: ${remainingTodos.length}`,
      )
      .withNextActions(nextActions)
      .asSuccess();
  }

  /**
   * Gets the next available todo ID by finding the maximum ID + 1.
   * Used for circular dependency detection before the todo is actually created.
   *
   * @returns The next available todo ID
   * @private
   */
  private async getNextTodoId(): Promise<number> {
    const allTodos = await db.todos.toArray();
    const maxId = allTodos.reduce(
      (max, todo) => (todo.id && todo.id > max ? todo.id : max),
      0,
    );
    return maxId + 1;
  }

  /**
   * Retrieves all todos for the current session/thread.
   * Public-facing method that delegates to getTodosList.
   *
   * @returns Array of SimpleTodo objects
   */
  async getTodos(): Promise<SimpleTodo[]> {
    return this.getTodosList();
  }

  /**
   * Clears all todos for the current session/thread.
   * Used when performing a complete session reset.
   *
   * @returns The number of todos that were deleted
   */
  async clearAllTodos(): Promise<number> {
    const todos = await this.getTodosList();
    const count = todos.length;

    await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();

    return count;
  }
}
