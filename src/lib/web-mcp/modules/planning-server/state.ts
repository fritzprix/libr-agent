import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import { db, type PlanningGoal, type PlanningTodo } from './db';
import type {
  SimpleTodo,
  ScratchpadItem,
  ThoughtData,
  PlanningState,
  BaseOutput,
  CreateGoalOutput,
  ClearGoalOutput,
  AddToDoOutput,
  CheckTodoOutput,
} from './types';
import { MCPResponseBuilder } from '@/lib/web-mcp/response-builder';
import { WebMCPErrorCodes } from '@/lib/web-mcp/error-codes';

const MAX_NOTES = 20;

/**
 * Formats a list of todos for error messages
 */
function formatTodosList(todos: SimpleTodo[]): string {
  if (todos.length === 0) {
    return '  (no todos)';
  }
  return todos
    .map((t) => `  - ID: ${t.id} [${t.status}] ${t.name}`)
    .join('\n');
}

/**
 * Manages the persistent state for the planning server using Dexie.js.
 * Goals, Todos, and Memos are persisted.
 * Sequential Thinking state remains ephemeral (in-memory) for now.
 * @internal
 */
export class PersistentState {
  private sessionId: string;
  private threadId: string;

  // Sequential thinking state (Ephemeral)
  private thoughtHistory: ThoughtData[] = [];
  private branches: Record<string, ThoughtData[]> = {};
  private disableThoughtLogging = false;

  constructor(sessionId: string, threadId: string) {
    this.sessionId = sessionId;
    this.threadId = threadId;
  }

  private async getActiveGoal(): Promise<PlanningGoal | undefined> {
    return db.goals
      .where({
        sessionId: this.sessionId,
        threadId: this.threadId,
        isActive: 1,
      })
      .last();
  }

  private async getLastClearedGoalRecord(): Promise<PlanningGoal | undefined> {
    return db.goals
      .where({
        sessionId: this.sessionId,
        threadId: this.threadId,
        isActive: 0,
      })
      .last();
  }

  private async getTodosList(): Promise<SimpleTodo[]> {
    const todos = await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('order');

    return todos.map((t) => ({
      id: t.id!,
      name: t.name,
      status: t.status,
      summary: t.summary,
      priority: t.priority,
      dependsOn: t.dependsOn,
    }));
  }

  /**
   * Helper method to resolve todo ID from either id or index parameter.
   * @param params Object containing either 'id' or 'index'
   * @returns Resolved todo ID or undefined if not found
   */
  private async resolveTodoId(params: {
    id?: number;
    index?: number;
  }): Promise<{ id: number | undefined; todos: SimpleTodo[] }> {
    const todos = await this.getTodosList();

    if (params.id !== undefined) {
      return { id: params.id, todos };
    }

    if (params.index !== undefined) {
      if (params.index >= 0 && params.index < todos.length) {
        return { id: todos[params.index].id, todos };
      }
      return { id: undefined, todos };
    }

    return { id: undefined, todos };
  }

  async getScratchpadList(): Promise<ScratchpadItem[]> {
    const items = await db.scratchpad
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('id');

    return items.map((m) => ({
      id: m.id!,
      content: m.content,
      source: m.source,
    }));
  }

  async createGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    const previousGoal = await this.getActiveGoal();

    // Deactivate previous goal if exists
    if (previousGoal && previousGoal.id) {
      await db.goals.update(previousGoal.id, { isActive: 0 });
    }

    await db.goals.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: goal,
      isActive: 1,
      createdAt: Date.now(),
    });

    const todos = await this.getTodosList();

    const nextActions = [
      'Break down goal into actionable todos with add_todo',
      'Set priorities and dependencies if needed',
      'Track progress with get_current_state',
    ];

    let message = `Goal set: "${goal}"`;
    if (previousGoal) {
      message += `\n\nPrevious goal: "${previousGoal.content}"\nTodos from previous goal: ${todos.length}`;
    }

    return new MCPResponseBuilder({
      goal,
      success: true,
      previousGoal: previousGoal?.content,
      existingTodos: todos.length,
    })
      .withMessage(message)
      .withNextActions(nextActions)
      .withSuggestions([
        'Start with 3-5 high-level todos, then refine as you go',
      ])
      .asSuccess();
  }

  async updateGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    const activeGoal = await this.getActiveGoal();
    if (!activeGoal || !activeGoal.id) {
      return createMCPStructuredToolResult(
        'No active goal to update. Use create_goal first.',
        {
          success: false,
          goal: '',
        },
      );
    }
    const oldGoalContent = activeGoal.content;
    await db.goals.update(activeGoal.id, { content: goal });

    return createMCPStructuredToolResult<CreateGoalOutput>(
      `Goal updated from "${oldGoalContent}" to "${goal}"`,
      {
        goal,
        success: true,
      },
    );
  }

  async clearGoal(): Promise<MCPResult<ClearGoalOutput>> {
    const activeGoal = await this.getActiveGoal();
    if (activeGoal && activeGoal.id) {
      const clearedGoalContent = activeGoal.content;
      await db.goals.update(activeGoal.id, { isActive: 0 });

      const todos = await this.getTodosList();
      const remainingTodos = todos.length;
      const todoSummary =
        remainingTodos > 0
          ? `Remaining todos: ${remainingTodos}`
          : 'All todos have been completed or cleared.';
      return createMCPStructuredToolResult<ClearGoalOutput>(
        `Goal cleared: "${clearedGoalContent}"\n${todoSummary}\nSession is now ready for a new goal.`,
        {
          success: true,
        },
      );
    }
    return createMCPStructuredToolResult('No active goal to clear', {
      success: false,
    });
  }

  async addTodo(
    name: string,
    priority?: 'low' | 'medium' | 'high',
    dependsOn?: number[],
  ): Promise<MCPResult<AddToDoOutput>> {
    const todos = await this.getTodosList();
    
    // Check for duplicate todos (case-insensitive, trimmed)
    const normalizedName = name.trim().toLowerCase();
    const duplicate = todos.find(
      (t) => t.name.trim().toLowerCase() === normalizedName,
    );
    
    if (duplicate) {
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

    const order = todos.length;

    const id = await db.todos.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      name,
      status: 'pending',
      priority,
      dependsOn,
      order,
      createdAt: Date.now(),
    });

    const newTodos = await this.getTodosList();
    const activeGoal = await this.getActiveGoal();
    const pendingCount = newTodos.filter((t) => t.status === 'pending').length;
    const completedCount = newTodos.filter(
      (t) => t.status === 'completed',
    ).length;

    let message = `Todo added: "${name}" (ID: ${id})`;
    if (activeGoal) {
      message += `\n\nGoal: "${activeGoal.content}"`;
    }
    message += `\n\nCurrent progress:\n  - Total: ${newTodos.length} todos\n  - Pending: ${pendingCount}\n  - Completed: ${completedCount}`;

    return new MCPResponseBuilder({
      success: true,
      id,
      todo: { id, name, status: 'pending' as const, priority, dependsOn },
      todos: newTodos,
      summary: {
        total: newTodos.length,
        pending: pendingCount,
        completed: completedCount,
      },
    })
      .withMessage(message)
      .withNextActions(['Use mark_todo when this task is done'])
      .asSuccess();
  }

  async updateTodo(
    params: { id?: number; index?: number },
    updates: {
      name?: string;
      status?: 'pending' | 'completed' | 'blocked';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): Promise<MCPResult<unknown>> {
    // Resolve id from either id or index
    const { id: resolvedId, todos: allTodos } =
      await this.resolveTodoId(params);

    if (resolvedId === undefined) {
      const validIds = allTodos.map((t) => t.id);
      const pendingCount = allTodos.filter(
        (t) => t.status === 'pending',
      ).length;
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

    const todo = await db.todos.get(resolvedId);
    // Ensure todo belongs to this session
    if (
      !todo ||
      todo.sessionId !== this.sessionId ||
      todo.threadId !== this.threadId
    ) {
      const validIds = allTodos.map((t) => t.id);
      const pendingCount = allTodos.filter(
        (t) => t.status === 'pending',
      ).length;
      const completedCount = allTodos.filter(
        (t) => t.status === 'completed',
      ).length;

      const suggestions = [
        'Use get_current_state to see all todos with their IDs and indexes',
      ];

      return new MCPResponseBuilder({
        requestedId: resolvedId,
        validIds,
        totalCount: allTodos.length,
        pending: pendingCount,
        completed: completedCount,
        todos: allTodos,
      })
        .withMessage(
          `Todo ${resolvedId} not found.\n\n` +
            `Current todos (${allTodos.length} total):\n` +
            formatTodosList(allTodos),
        )
        .withSuggestions(suggestions)
        .asError(WebMCPErrorCodes.PLANNING.TODO_NOT_FOUND);
    }

    await db.todos.update(resolvedId, updates);
    const updatedTodoRecord = await db.todos.get(resolvedId);
    const todos = await this.getTodosList();

    const simpleTodo: SimpleTodo = {
      id: updatedTodoRecord!.id!,
      name: updatedTodoRecord!.name,
      status: updatedTodoRecord!.status,
      summary: updatedTodoRecord!.summary,
      priority: updatedTodoRecord!.priority,
      dependsOn: updatedTodoRecord!.dependsOn,
    };

    const identifier =
      params.index !== undefined
        ? `index ${params.index} (ID: ${resolvedId})`
        : `ID ${resolvedId}`;

    return createMCPStructuredToolResult<CheckTodoOutput>(
      `Todo ${identifier} updated: "${simpleTodo.name}"`,
      {
        success: true,
        todo: simpleTodo,
        todos,
      },
    );
  }

  async checkTodo(
    params: { id?: number; index?: number },
    check: boolean = true,
    summary?: string,
  ): Promise<MCPResult<unknown>> {
    // Resolve id from either id or index
    const { id: resolvedId, todos: allTodos } =
      await this.resolveTodoId(params);

    if (resolvedId === undefined) {
      const validIds = allTodos.map((t) => t.id);
      const pendingCount = allTodos.filter(
        (t) => t.status === 'pending',
      ).length;
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

    const todo = await db.todos.get(resolvedId);
    if (
      !todo ||
      todo.sessionId !== this.sessionId ||
      todo.threadId !== this.threadId
    ) {
      const validIds = allTodos.map((t) => t.id);
      const pendingCount = allTodos.filter(
        (t) => t.status === 'pending',
      ).length;
      const completedCount = allTodos.filter(
        (t) => t.status === 'completed',
      ).length;

      const suggestions = [
        'Use get_current_state to see all todos with their IDs and indexes',
      ];

      return new MCPResponseBuilder({
        requestedId: resolvedId,
        validIds,
        totalCount: allTodos.length,
        pending: pendingCount,
        completed: completedCount,
        todos: allTodos,
      })
        .withMessage(
          `Todo ${resolvedId} not found.\n\n` +
            `Current todos (${allTodos.length} total):\n` +
            formatTodosList(allTodos),
        )
        .withSuggestions(suggestions)
        .asError(WebMCPErrorCodes.PLANNING.TODO_NOT_FOUND);
    }

    const updates: Partial<PlanningTodo> = {
      status: check ? 'completed' : 'pending',
    };
    if (summary !== undefined) {
      updates.summary = summary || undefined;
    }

    await db.todos.update(resolvedId, updates);
    const updatedTodoRecord = await db.todos.get(resolvedId);
    const todos = await this.getTodosList();

    const simpleTodo: SimpleTodo = {
      id: updatedTodoRecord!.id!,
      name: updatedTodoRecord!.name,
      status: updatedTodoRecord!.status,
      summary: updatedTodoRecord!.summary,
      priority: updatedTodoRecord!.priority,
      dependsOn: updatedTodoRecord!.dependsOn,
    };

    // Find unblocked todos if this was completed
    const unblockedTodos: SimpleTodo[] = [];
    if (check) {
      for (const t of todos) {
        if (
          t.status === 'blocked' &&
          t.dependsOn &&
          t.dependsOn.includes(resolvedId)
        ) {
          // Check if all dependencies are now completed
          const allDepsCompleted = t.dependsOn.every((depId) => {
            const dep = todos.find((d) => d.id === depId);
            return dep?.status === 'completed';
          });
          if (allDepsCompleted) {
            unblockedTodos.push(t);
          }
        }
      }
    }

    // Calculate progress
    const completedCount = todos.filter((t) => t.status === 'completed').length;
    const progress = Math.round((completedCount / todos.length) * 100);

    const identifier =
      params.index !== undefined
        ? `index ${params.index} (ID: ${resolvedId})`
        : `ID ${resolvedId}`;

    let message = `Todo ${identifier} marked as ${check ? 'completed' : 'pending'}.\n\n`;
    message += `Progress: ${completedCount}/${todos.length} (${progress}%)`;

    if (simpleTodo.summary) {
      message += `\n\nSummary: "${simpleTodo.summary}"`;
    }

    if (unblockedTodos.length > 0) {
      message += `\n\nUnblocked todos:\n${unblockedTodos.map((t) => `  - [${t.id}] ${t.name}`).join('\n')}`;
    }

    return new MCPResponseBuilder({
      success: true,
      id: resolvedId,
      completed: check,
      todo: simpleTodo,
      todos,
      progress: {
        completed: completedCount,
        total: todos.length,
        percentage: progress,
      },
      unblockedTodos: unblockedTodos.map((t) => ({ id: t.id, name: t.name })),
    })
      .withMessage(message)
      .asSuccess();
  }

  async clearTodos(ids?: number[]): Promise<MCPResult<BaseOutput>> {
    const todos = await this.getTodosList();

    if (!ids || ids.length === 0) {
      const clearedCount = todos.length;
      await db.todos
        .where({ sessionId: this.sessionId, threadId: this.threadId })
        .delete();

      const activeGoal = await this.getActiveGoal();
      const msg =
        clearedCount > 0
          ? `All ${clearedCount} todo(s) cleared`
          : 'No todos to clear.';
      return createMCPStructuredToolResult<BaseOutput>(
        `${msg}\nSession todos reset. Current goal: ${activeGoal ? activeGoal.content : '(none)'}`,
        {
          success: true,
        },
      );
    }

    const initialCount = todos.length;
    // Verify IDs belong to session
    const validIds = todos.map((t) => t.id);
    const idsToDelete = ids.filter((id) => validIds.includes(id));

    if (idsToDelete.length === 0) {
      return createMCPStructuredToolResult<BaseOutput>(
        `No todos found with the specified IDs: ${ids.join(', ')}\nAvailable IDs: ${initialCount > 0 ? validIds.join(', ') : '(none)'}`,
        { success: false },
      );
    }

    const todosToDelete = todos.filter((t) => idsToDelete.includes(t.id));
    await db.todos.bulkDelete(idsToDelete);

    const remainingTodos = await this.getTodosList();
    const removedCount = idsToDelete.length;
    const clearedNames = todosToDelete.map((t) => t.name).join(', ');

    return createMCPStructuredToolResult<BaseOutput>(
      `Cleared ${removedCount} todo(s): ${clearedNames}\nRemaining todos: ${remainingTodos.length}`,
      { success: true },
    );
  }

  async clear(): Promise<MCPResult<BaseOutput>> {
    const activeGoal = await this.getActiveGoal();
    const todos = await this.getTodosList();
    const scratchpad = await this.getScratchpadList();

    const clearedGoal = activeGoal ? activeGoal.content : null;
    const clearedTodos = todos.length;
    const clearedScratchpad = scratchpad.length;

    // Delete all data for this session/thread
    await db.goals
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();
    await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();
    await db.scratchpad
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();

    // Reset sequential thinking
    this.thoughtHistory = [];
    this.branches = {};

    const summaryText = `Session state cleared:\n- Goal: ${clearedGoal ? `"${clearedGoal}"` : '(none)'}\n- Todos cleared: ${clearedTodos}\n- Scratchpad items cleared: ${clearedScratchpad}\n\nSession is now completely reset.`;
    return createMCPStructuredToolResult(summaryText, {
      success: true,
    });
  }

  async getGoal(): Promise<string | null> {
    const goal = await this.getActiveGoal();
    return goal ? goal.content : null;
  }

  async getTodos(): Promise<SimpleTodo[]> {
    return this.getTodosList();
  }

  async addScratchpad(
    note: string,
    source?: string,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    await db.scratchpad.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: note,
      source,
      createdAt: Date.now(),
    });

    // Enforce MAX_NOTES
    const items = await this.getScratchpadList();
    if (items.length > MAX_NOTES) {
      // Remove oldest
      const oldest = items[0]; // Sorted by id (auto-inc)
      await db.scratchpad.delete(oldest.id);
    }

    const updatedItems = await this.getScratchpadList();
    const capacityWarning =
      updatedItems.length === MAX_NOTES
        ? `⚠️ At capacity (${MAX_NOTES}/${MAX_NOTES}) - oldest items will be removed`
        : `Scratchpad: ${updatedItems.length}/${MAX_NOTES}`;

    // Get the ID of the newly added item (last one)
    const newItemId = updatedItems[updatedItems.length - 1].id;

    let message = `Scratchpad ID:${newItemId} added\n${capacityWarning}`;
    if (source) {
      message += `\nSource: ${source}`;
    }

    return createMCPStructuredToolResult<
      BaseOutput & { scratchpad: ScratchpadItem[] }
    >(message, {
      success: true,
      scratchpad: updatedItems,
    });
  }

  async clearScratchpad(
    id: number,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    const item = await db.scratchpad.get(id);
    if (
      !item ||
      item.sessionId !== this.sessionId ||
      item.threadId !== this.threadId
    ) {
      const scratchpad = await this.getScratchpadList();
      const validIds = scratchpad.map((m) => m.id);
      return createMCPStructuredToolResult<
        BaseOutput & { scratchpad: ScratchpadItem[] }
      >(
        `Scratchpad ID:${id} not found.\nValid IDs: ${validIds.length > 0 ? validIds.join(', ') : '(no scratchpad items)'}`,
        { success: false, scratchpad },
      );
    }

    await db.scratchpad.delete(id);
    const scratchpad = await this.getScratchpadList();

    return createMCPStructuredToolResult<
      BaseOutput & { scratchpad: ScratchpadItem[] }
    >(
      `Scratchpad ID:${id} cleared: "${item.content}"\nRemaining scratchpad items: ${scratchpad.length}`,
      { success: true, scratchpad },
    );
  }

  async getLastClearedGoal(): Promise<string | null> {
    const goal = await this.getLastClearedGoalRecord();
    return goal ? goal.content : null;
  }

  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    try {
      const data = input as Record<string, unknown>;

      if (!data.thought || typeof data.thought !== 'string') {
        return createMCPErrorToolResult(
          'Invalid thought: must be a string',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (
        data.thoughtNumber === undefined ||
        typeof data.thoughtNumber !== 'number'
      ) {
        return createMCPErrorToolResult(
          'Invalid thoughtNumber: must be a number',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (
        data.totalThoughts === undefined ||
        typeof data.totalThoughts !== 'number'
      ) {
        return createMCPErrorToolResult(
          'Invalid totalThoughts: must be a number',
        ) as MCPResult<Record<string, unknown>>;
      }
      if (typeof data.nextThoughtNeeded !== 'boolean') {
        return createMCPErrorToolResult(
          'Invalid nextThoughtNeeded: must be a boolean',
        ) as MCPResult<Record<string, unknown>>;
      }

      const thought: ThoughtData = {
        thought: data.thought as string,
        thoughtNumber: data.thoughtNumber as number,
        totalThoughts: data.totalThoughts as number,
        nextThoughtNeeded: data.nextThoughtNeeded as boolean,
        isRevision: data.isRevision as boolean | undefined,
        revisesThought: data.revisesThought as number | undefined,
        branchFromThought: data.branchFromThought as number | undefined,
        branchId: data.branchId as string | undefined,
        needsMoreThoughts: data.needsMoreThoughts as boolean | undefined,
        category: data.category as string | undefined,
        relatedTodoId: data.relatedTodoId as number | undefined,
        nextAction: data.nextAction as string | undefined,
      };

      if (thought.thoughtNumber > thought.totalThoughts) {
        thought.totalThoughts = thought.thoughtNumber;
      }

      this.thoughtHistory.push(thought);

      if (thought.branchFromThought && thought.branchId) {
        if (!this.branches[thought.branchId]) {
          this.branches[thought.branchId] = [];
        }
        this.branches[thought.branchId].push(thought);
      }

      if (!this.disableThoughtLogging) {
        console.error(
          `SEQUENTIAL THOUGHT ${thought.thoughtNumber}/${thought.totalThoughts}: ${thought.thought}`,
        );
      }

      const summary = {
        thoughtNumber: thought.thoughtNumber,
        totalThoughts: thought.totalThoughts,
        nextThoughtNeeded: thought.nextThoughtNeeded,
        branches: Object.keys(this.branches),
        thoughtHistoryLength: this.thoughtHistory.length,
      } as Record<string, unknown>;

      return createMCPStructuredToolResult('Thought processed', summary);
    } catch (error) {
      return createMCPStructuredToolResult('Failed to process thought', {
        error: error instanceof Error ? error.message : String(error),
        status: 'failed',
      });
    }
  }
}

/**
 * Session-based state manager that maintains separate PersistentState instances
 * for each (sessionId, threadId) pair.
 * @internal
 */
export class SessionStateManager {
  private sessions = new Map<string, Map<string, PersistentState>>();
  private currentSessionId: string | null = null;
  private currentThreadId: string | null = null;

  setSession(sessionId: string, threadId?: string): void {
    const effectiveThreadId = threadId || sessionId;

    if (this.currentSessionId && this.currentSessionId !== sessionId) {
      // In persistent mode, we don't strictly need to clear memory,
      // but we can to keep memory usage low.
      // However, if we want to keep sequential thinking history for the session,
      // we should keep it until explicitly cleared or maybe LRU.
      // For now, we'll keep the same behavior: clear old session from memory.
      this.sessions.delete(this.currentSessionId);
      console.info(
        `[PlanningServer] Session cleanup: removed all threads from session "${this.currentSessionId}"`,
      );
    }

    this.currentSessionId = sessionId;
    this.currentThreadId = effectiveThreadId;
  }

  getCurrentSessionId(): string | null {
    return this.currentSessionId;
  }

  getCurrentThreadId(): string | null {
    return this.currentThreadId;
  }

  private getState(sessionId: string, threadId: string): PersistentState {
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, new Map());
    }
    const threadMap = this.sessions.get(sessionId)!;
    if (!threadMap.has(threadId)) {
      threadMap.set(threadId, new PersistentState(sessionId, threadId));
    }
    return threadMap.get(threadId)!;
  }

  getCurrentState(): PersistentState {
    if (!this.currentSessionId) {
      this.setSession('default');
    }
    const effectiveThreadId = this.currentThreadId || this.currentSessionId!;
    return this.getState(this.currentSessionId!, effectiveThreadId);
  }

  async createGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    return this.getCurrentState().createGoal(goal);
  }

  async updateGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    return this.getCurrentState().updateGoal(goal);
  }

  async clearGoal(): Promise<MCPResult<ClearGoalOutput>> {
    return this.getCurrentState().clearGoal();
  }

  async addTodo(
    name: string,
    priority?: 'low' | 'medium' | 'high',
    dependsOn?: number[],
  ): Promise<MCPResult<AddToDoOutput>> {
    return this.getCurrentState().addTodo(name, priority, dependsOn);
  }

  async updateTodo(
    params: { id?: number; index?: number },
    updates: {
      name?: string;
      status?: 'pending' | 'completed' | 'blocked';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): Promise<MCPResult<unknown>> {
    return this.getCurrentState().updateTodo(params, updates);
  }

  async clearTodos(ids?: number[]): Promise<MCPResult<BaseOutput>> {
    return this.getCurrentState().clearTodos(ids);
  }

  async clear(): Promise<MCPResult<BaseOutput>> {
    return this.getCurrentState().clear();
  }

  async getGoal(): Promise<string | null> {
    return this.getCurrentState().getGoal();
  }

  async getTodos(): Promise<SimpleTodo[]> {
    return this.getCurrentState().getTodos();
  }

  async addScratchpad(
    note: string,
    source?: string,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.getCurrentState().addScratchpad(note, source);
  }

  async clearScratchpad(
    id: number,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.getCurrentState().clearScratchpad(id);
  }

  async getScratchpad(): Promise<ScratchpadItem[]> {
    return this.getCurrentState().getScratchpadList();
  }

  async getLastClearedGoal(): Promise<string | null> {
    return this.getCurrentState().getLastClearedGoal();
  }

  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    return this.getCurrentState().processThought(input);
  }

  async checkTodo(
    params: { id?: number; index?: number },
    check: boolean = true,
    summary?: string,
  ): Promise<MCPResult<unknown>> {
    return this.getCurrentState().checkTodo(params, check, summary);
  }

  clearAllSessions(): void {
    for (const [sessionId, threadMap] of this.sessions.entries()) {
      // In persistent mode, this only clears in-memory instances.
      // It does NOT clear the DB.
      threadMap.clear();
      this.sessions.delete(sessionId);
    }
    this.currentSessionId = null;
    this.currentThreadId = null;
  }

  async getStateForSession(
    sessionId: string,
    threadId?: string,
  ): Promise<PlanningState | null> {
    const effectiveThreadId = threadId || sessionId;
    // We can create a temporary state to fetch data even if not in memory
    const state = new PersistentState(sessionId, effectiveThreadId);

    return {
      goal: await state.getGoal(),
      lastClearedGoal: await state.getLastClearedGoal(),
      todos: await state.getTodos(),
      scratchpad: await state.getScratchpadList(),
    };
  }
}
