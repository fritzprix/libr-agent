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
} from '../utils/response-builders';

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
   * Converts database records to SimpleTodo format with hierarchical structure.
   * Only returns top-level todos with their subtasks nested (1-level deep).
   *
   * @returns Array of SimpleTodo objects (top-level only, with subtasks)
   */
  async getTodosList(): Promise<SimpleTodo[]> {
    const todos = await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('order');

    const allTodos = todos.map((t) => ({
      id: t.id!,
      title:
        typeof t.title === 'string' && t.title
          ? t.title
          : (t as unknown as LegacyTodo).name || '(Untitled)', // Fallback to name for legacy data
      description: t.description,
      checked: t.checked,
      summary: t.summary,
      priority: t.priority,
      parentId: t.parentId,
    }));

    // Separate top-level and children
    const topLevel = allTodos.filter((t) => !t.parentId);
    const children = allTodos.filter((t) => t.parentId);

    // Build hierarchy (1-level only)
    return topLevel.map((parent) => ({
      ...parent,
      subtasks: children.filter((c) => c.parentId === parent.id),
    }));
  }

  /**
   * Adds a new todo with duplicate detection and validation.
   * Supports 1-level nesting:
   * - Can specify parentId to add as a child of an existing todo
   * - Can provide subtasks array to create children alongside the parent
   * Checks for corrupted todos and duplicate names before adding.
   *
   * @param title - The title/summary of the todo
   * @param description - Optional detailed description
   * @param priority - Optional priority level (low, medium, high)
   * @param parentId - Optional parent todo ID (must be top-level)
   * @param subtasks - Optional array of subtasks to create with this todo
   * @param activeGoalContent - Optional active goal content for context in response
   * @returns MCPResult with the new todo and updated list
   */
  async addTodo(
    title: string,
    description?: string,
    priority?: 'low' | 'medium' | 'high',
    parentId?: number,
    subtasks?: Array<{
      title: string;
      description?: string;
      priority?: 'low' | 'medium' | 'high';
    }>,
    activeGoalContent?: string | null,
  ): Promise<MCPResult<AddToDoOutput>> {
    // Validation: Title cannot be empty or whitespace-only
    if (!title || title.trim() === '') {
      return buildEmptyTitleError('todo') as MCPResult<AddToDoOutput>;
    }

    // Validation: Cannot have both parentId and subtasks
    if (parentId && subtasks && subtasks.length > 0) {
      return {
        content: [
          {
            type: 'text',
            text: 'Cannot specify both parentId and subtasks. Subtasks are only allowed when creating a top-level todo.',
          },
        ],
        isError: true,
      } as MCPResult<AddToDoOutput>;
    }

    // Validation: If parentId is provided, verify it exists and is top-level
    if (parentId) {
      const parent = await db.todos.get(parentId);
      if (
        !parent ||
        parent.sessionId !== this.sessionId ||
        parent.threadId !== this.threadId
      ) {
        return {
          content: [
            {
              type: 'text',
              text: `Parent todo with ID ${parentId} not found in current session.`,
            },
          ],
          isError: true,
        } as MCPResult<AddToDoOutput>;
      }

      if (parent.parentId) {
        return {
          content: [
            {
              type: 'text',
              text: `Cannot create subtask under ID ${parentId}: it is already a subtask. Only 1-level nesting is supported.`,
            },
          ],
          isError: true,
        } as MCPResult<AddToDoOutput>;
      }
    }

    const todos = await this.getTodosList();

    // Check for corrupted todos (missing title)
    const corruptedTodos = checkCorruptedTodos(
      todos.flatMap((t) => [t, ...(t.subtasks || [])]),
    );
    if (corruptedTodos) {
      return buildCorruptedTodosError(
        corruptedTodos,
      ) as MCPResult<AddToDoOutput>;
    }

    // Check for duplicate todos (case-insensitive, trimmed)
    const allTodosFlat = todos.flatMap((t) => [t, ...(t.subtasks || [])]);
    const duplicate = checkDuplicateTodo(allTodosFlat, title);
    if (duplicate) {
      return buildDuplicateTodoError(
        duplicate,
        allTodosFlat,
      ) as MCPResult<AddToDoOutput>;
    }

    const order = todos.length;

    const id = await db.todos.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      title,
      description,
      checked: false,
      priority,
      parentId,
      order,
      createdAt: Date.now(),
    });

    // Add subtasks if provided
    let subtaskIds: number[] = [];
    if (subtasks && subtasks.length > 0) {
      subtaskIds = await Promise.all(
        subtasks.map(async (sub, index) => {
          return await db.todos.add({
            sessionId: this.sessionId,
            threadId: this.threadId,
            title: sub.title,
            description: sub.description,
            checked: false,
            priority: sub.priority,
            parentId: id as number,
            order: order + index + 1,
            createdAt: Date.now(),
          });
        }),
      );
    }

    const newTodos = await this.getTodosList();
    const newTodosFlat = newTodos.flatMap((t) => [t, ...(t.subtasks || [])]);
    const uncheckedCount = newTodosFlat.filter((t) => !t.checked).length;
    const checkedCount = newTodosFlat.filter((t) => t.checked).length;

    let message = `Todo added: "${title}" (ID: ${id})`;
    if (parentId) {
      message = `Subtask added to parent ID ${parentId}: "${title}" (ID: ${id})`;
    }
    if (subtaskIds.length > 0) {
      message += `\n  with ${subtaskIds.length} subtask(s) (IDs: ${subtaskIds.join(', ')})`;
    }
    if (activeGoalContent) {
      message += `\n\nGoal: "${activeGoalContent}"`;
    }
    message += `\n\nCurrent progress:\n  - Total: ${newTodosFlat.length} todos (${newTodos.length} top-level, ${newTodosFlat.length - newTodos.length} subtasks)\n  - Unchecked: ${uncheckedCount}\n  - Checked: ${checkedCount}`;

    return new MCPResponseBuilder({
      success: true,
      id,
      todo: { id, title, description, checked: false, priority, parentId },
      todos: newTodos,
      summary: {
        total: newTodosFlat.length,
        topLevel: newTodos.length,
        unchecked: uncheckedCount,
        checked: checkedCount,
      },
    })
      .withMessage(message)
      .withNextActions(['Use checkTodo when this task is done'])
      .asSuccess();
  }

  /**
   * Marks a todo as checked or unchecked.
   * For parent todos with subtasks:
   * - Cannot be checked directly; must check all children first
   * - Auto-checks when all children are checked
   * For child todos:
   * - Can be checked normally
   * - Auto-checks parent when all siblings are checked
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

    // At this point, todo is guaranteed to exist and be valid
    const validTodo = todo!;

    // Check if this todo has subtasks
    const children = await db.todos
      .where({
        sessionId: this.sessionId,
        threadId: this.threadId,
        parentId: resolvedId,
      })
      .toArray();

    // If trying to check a parent with unchecked children, prevent it
    if (checked && children.length > 0) {
      const uncheckedChildren = children.filter((c) => !c.checked);
      if (uncheckedChildren.length > 0) {
        const childrenInfo = children
          .map(
            (c) =>
              `  - ID: ${c.id} [${c.checked ? 'checked' : 'unchecked'}] ${c.title}`,
          )
          .join('\n');

        return {
          content: [
            {
              type: 'text',
              text: `Cannot check parent todo "${validTodo.title}" (ID: ${resolvedId}) directly.\nThis todo has ${children.length} subtask(s), and ${uncheckedChildren.length} are still unchecked.\n\nPlease complete the subtasks first:\n${childrenInfo}\n\nThe parent will be automatically checked when all subtasks are completed.`,
            },
          ],
          isError: true,
        } as MCPResult<unknown>;
      }
    }

    const updates: { checked: boolean; summary?: string } = {
      checked,
    };
    if (summary !== undefined) {
      updates.summary = summary || undefined;
    }

    await db.todos.update(resolvedId, updates);

    // If this is a child todo and all siblings are now checked, auto-check parent
    let autoCheckedParent: PlanningTodo | undefined;
    if (checked && validTodo.parentId) {
      const siblings = await db.todos
        .where({
          sessionId: this.sessionId,
          threadId: this.threadId,
          parentId: validTodo.parentId,
        })
        .toArray();

      const allSiblingsChecked = siblings.every(
        (s) => s.id === resolvedId || s.checked,
      );

      if (allSiblingsChecked) {
        await db.todos.update(validTodo.parentId, { checked: true });
        autoCheckedParent = await db.todos.get(validTodo.parentId);
      }
    }

    const updatedTodoRecord = await db.todos.get(resolvedId);
    const todos = await this.getTodosList();
    const allTodosFlat = todos.flatMap((t) => [t, ...(t.subtasks || [])]);

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
      parentId: updatedTodoRecord!.parentId,
    };

    // Find unblocked todos if this was checked
    const unblockedTodos: SimpleTodo[] = [];

    // Calculate progress (count all todos including subtasks)
    const checkedCount = allTodosFlat.filter((t) => t.checked).length;
    const progress = Math.round((checkedCount / allTodosFlat.length) * 100);

    const identifier =
      params.index !== undefined
        ? `index ${params.index} (ID: ${resolvedId})`
        : `ID ${resolvedId}`;

    let message = `Todo ${identifier} marked as ${checked ? 'checked' : 'unchecked'}.\n\n`;
    message += `Progress: ${checkedCount}/${allTodosFlat.length} (${progress}%)`;

    if (autoCheckedParent) {
      message += `\n\n✓ Parent todo "${autoCheckedParent.title}" (ID: ${autoCheckedParent.id}) automatically checked (all subtasks completed)`;
    }

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
        total: allTodosFlat.length,
        percentage: progress,
      },
      unblockedTodos: unblockedTodos.map((t) => ({ id: t.id, title: t.title })),
      autoCheckedParent: autoCheckedParent
        ? { id: autoCheckedParent.id, title: autoCheckedParent.title }
        : undefined,
    });

    // Identify next action guidance
    const nextActions: string[] = [];

    if (unblockedTodos.length > 0) {
      nextActions.push(
        `Begin work on unblocked todo: "${unblockedTodos[0].title}" (ID: ${unblockedTodos[0].id})`,
      );
    } else {
      // Find next unchecked todo (consider subtasks)
      const nextUnchecked = allTodosFlat.find(
        (t) => !t.checked && t.id !== resolvedId,
      );
      if (nextUnchecked) {
        const taskType = nextUnchecked.parentId ? 'subtask' : 'todo';
        nextActions.push(
          `Proceed to next ${taskType}: "${nextUnchecked.title}" (ID: ${nextUnchecked.id})`,
        );
      } else if (checkedCount === allTodosFlat.length) {
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
   * Clears todos by ID or index, or clears all todos if no IDs/indices provided.
   * When clearing a parent todo, all subtasks are also deleted (cascade).
   *
   * @param ids - Optional array of todo IDs to clear.
   * @param indices - Optional array of todo indices to clear.
   * @param activeGoalContent - Optional active goal content for context in response
   * @returns MCPResult with clear status and remaining todos count
   */
  async clearTodos(
    ids?: number[],
    indices?: number[],
    activeGoalContent?: string | null,
  ): Promise<MCPResult<BaseOutput>> {
    const todos = await this.getTodosList();
    const allTodosFlat = todos.flatMap((t) => [t, ...(t.subtasks || [])]);

    const hasIds = ids && ids.length > 0;
    const hasIndices = indices && indices.length > 0;

    if (!hasIds && !hasIndices) {
      const clearedCount = allTodosFlat.length;
      await db.todos
        .where({ sessionId: this.sessionId, threadId: this.threadId })
        .delete();

      const msg =
        clearedCount > 0
          ? `All ${clearedCount} todo(s) cleared (including subtasks)`
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

    // Verify IDs belong to session (include both parents and children)
    const validIds = allTodosFlat.map((t) => t.id);
    let idsToDelete: number[] = [];

    if (hasIds) {
      idsToDelete = ids!.filter((id) => validIds.includes(id));
    }

    if (hasIndices) {
      indices!.forEach((index) => {
        if (index >= 0 && index < todos.length) {
          idsToDelete.push(todos[index].id);
        }
      });
    }

    // Deduplicate
    idsToDelete = [...new Set(idsToDelete)];

    if (idsToDelete.length === 0) {
      return createMCPStructuredToolResult<BaseOutput>(
        `No todos found with the specified IDs or indices.\nAvailable top-level IDs: ${
          todos.length > 0 ? todos.map((t) => t.id).join(', ') : '(none)'
        }`,
        { success: false },
      );
    }

    // For each parent being deleted, also delete all children
    const childrenToDelete: number[] = [];
    for (const id of idsToDelete) {
      const children = await db.todos
        .where({
          sessionId: this.sessionId,
          threadId: this.threadId,
          parentId: id,
        })
        .toArray();
      childrenToDelete.push(...children.map((c) => c.id!));
    }

    const allIdsToDelete = [...new Set([...idsToDelete, ...childrenToDelete])];
    const todosToDelete = allTodosFlat.filter((t) =>
      allIdsToDelete.includes(t.id),
    );

    await db.todos.bulkDelete(allIdsToDelete);

    const remainingTodos = await this.getTodosList();
    const remainingFlat = remainingTodos.flatMap((t) => [
      t,
      ...(t.subtasks || []),
    ]);
    const removedCount = allIdsToDelete.length;
    const clearedNames = todosToDelete.map((t) => t.title).join(', ');

    const nextActions: string[] = [];
    const hasUnchecked = remainingFlat.some((t) => !t.checked);

    if (!hasUnchecked && remainingFlat.length > 0) {
      nextActions.push(
        "No unchecked todos remain! Consider using 'critiqueAndReflection' to review your work before finishing.",
      );
    } else if (remainingFlat.length > 0) {
      nextActions.push('Review and prioritize the remaining todos.');
    }

    let message = `Cleared ${removedCount} todo(s)`;
    if (childrenToDelete.length > 0) {
      message += ` (${idsToDelete.length} parent(s) + ${childrenToDelete.length} subtask(s))`;
    }
    message += `: ${clearedNames}\nRemaining todos: ${remainingFlat.length}`;

    return new MCPResponseBuilder({ success: true })
      .withMessage(message)
      .withNextActions(nextActions)
      .asSuccess();
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
