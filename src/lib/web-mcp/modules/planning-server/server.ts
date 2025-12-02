import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { WebMCPServerProxy } from '@/context/WebMCPContext';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { getLogger } from '@/lib/logger';
import { planningTools as tools } from './tools.ts';
import { db, type PlanningGoal, type PlanningTodo } from './db';

const logger = getLogger('PlanningServer');

/** Represents a single to-do item in the planning state. @internal */
interface SimpleTodo {
  id: number;
  name: string;
  status: 'pending' | 'completed' | 'blocked';
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  dependsOn?: number[];
}

export interface Memo {
  id: number;
  content: string;
}

/** Represents a single thought in the sequential-thinking tool. @internal */
interface ThoughtData {
  thought: string;
  thoughtNumber: number;
  totalThoughts: number;
  isRevision?: boolean;
  revisesThought?: number;
  branchFromThought?: number;
  branchId?: string;
  needsMoreThoughts?: boolean;
  nextThoughtNeeded: boolean;
  category?: string;
  relatedTodoId?: number;
  nextAction?: string;
}

/**
 * Represents the entire state of the planning server.
 */
export interface PlanningState {
  /** The current main goal. */
  goal: string | null;
  /** The most recently cleared goal, for context. */
  lastClearedGoal: string | null;
  /** The list of to-do items. */
  todos: SimpleTodo[];
  /** A list of recent notes or temporary records. */
  memos: Memo[];
}

/**
 * The base output structure for tool calls, indicating success.
 * @internal
 */
interface BaseOutput {
  success: boolean;
}

/**
 * The output for the `create_goal` tool call.
 * @internal
 */
interface CreateGoalOutput extends BaseOutput {
  goal: string;
}

/**
 * The output for the `clear_goal` tool call.
 * @internal
 */
type ClearGoalOutput = BaseOutput;

/**
 * The output for the `add_todo` tool call.
 * @internal
 */
interface AddToDoOutput extends BaseOutput {
  todos: SimpleTodo[];
}

/**
 * The output for the `check_todo` tool call.
 * @internal
 */
interface CheckTodoOutput extends BaseOutput {
  todo: SimpleTodo | null;
  todos: SimpleTodo[];
}

const MAX_NOTES = 20;

/**
 * Manages the persistent state for the planning server using Dexie.js.
 * Goals, Todos, and Memos are persisted.
 * Sequential Thinking state remains ephemeral (in-memory) for now.
 * @internal
 */
class PersistentState {
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

  private async getMemosList(): Promise<Memo[]> {
    const memos = await db.memos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .sortBy('id');

    return memos.map((m) => ({
      id: m.id!,
      content: m.content,
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
    const context = [];
    if (previousGoal) {
      context.push(`Previous goal: "${previousGoal.content}"`);
      context.push(`Todos from previous goal: ${todos.length}`);
    }
    const contextStr = context.length > 0 ? `\n${context.join('\n')}\n` : '';
    return createMCPStructuredToolResult<CreateGoalOutput>(
      `Goal created: "${goal}"${contextStr}New todos can be added to support this goal.`,
      {
        goal,
        success: true,
      },
    );
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
    const goalContext = activeGoal
      ? `Goal: "${activeGoal.content}"\n`
      : 'No active goal.\n';

    return createMCPStructuredToolResult<AddToDoOutput>(
      `Todo added: ID:${id} "${name}"\n${goalContext}Total todos: ${newTodos.length}`,
      {
        success: true,
        todos: newTodos,
      },
    );
  }

  async updateTodo(
    id: number,
    updates: {
      name?: string;
      status?: 'pending' | 'completed' | 'blocked';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): Promise<MCPResult<CheckTodoOutput>> {
    const todo = await db.todos.get(id);
    // Ensure todo belongs to this session
    if (
      !todo ||
      todo.sessionId !== this.sessionId ||
      todo.threadId !== this.threadId
    ) {
      const todos = await this.getTodosList();
      const availableIds = todos.map((t) => t.id);
      return createMCPStructuredToolResult<CheckTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos,
        },
      );
    }

    await db.todos.update(id, updates);
    const updatedTodoRecord = await db.todos.get(id);
    const todos = await this.getTodosList();

    const simpleTodo: SimpleTodo = {
      id: updatedTodoRecord!.id!,
      name: updatedTodoRecord!.name,
      status: updatedTodoRecord!.status,
      summary: updatedTodoRecord!.summary,
      priority: updatedTodoRecord!.priority,
      dependsOn: updatedTodoRecord!.dependsOn,
    };

    return createMCPStructuredToolResult<CheckTodoOutput>(
      `Todo ${id} updated: "${simpleTodo.name}"`,
      {
        success: true,
        todo: simpleTodo,
        todos,
      },
    );
  }

  async checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): Promise<MCPResult<CheckTodoOutput>> {
    const todo = await db.todos.get(id);
    if (
      !todo ||
      todo.sessionId !== this.sessionId ||
      todo.threadId !== this.threadId
    ) {
      const todos = await this.getTodosList();
      const availableIds = todos.map((t) => t.id);
      return createMCPStructuredToolResult<CheckTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos,
        },
      );
    }

    const updates: Partial<PlanningTodo> = {
      status: check ? 'completed' : 'pending',
    };
    if (summary !== undefined) {
      updates.summary = summary || undefined;
    }

    await db.todos.update(id, updates);
    const updatedTodoRecord = await db.todos.get(id);
    const todos = await this.getTodosList();

    const simpleTodo: SimpleTodo = {
      id: updatedTodoRecord!.id!,
      name: updatedTodoRecord!.name,
      status: updatedTodoRecord!.status,
      summary: updatedTodoRecord!.summary,
      priority: updatedTodoRecord!.priority,
      dependsOn: updatedTodoRecord!.dependsOn,
    };

    const summaryText = simpleTodo.summary
      ? ` (Summary: "${simpleTodo.summary}")`
      : '';
    return createMCPStructuredToolResult<CheckTodoOutput>(
      `Todo ${check ? 'checked' : 'unchecked'}: "${simpleTodo.name}"${summaryText}`,
      {
        success: true,
        todo: simpleTodo,
        todos,
      },
    );
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
    const memos = await this.getMemosList();

    const clearedGoal = activeGoal ? activeGoal.content : null;
    const clearedTodos = todos.length;
    const clearedMemos = memos.length;

    // Delete all data for this session/thread
    await db.goals
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();
    await db.todos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();
    await db.memos
      .where({ sessionId: this.sessionId, threadId: this.threadId })
      .delete();

    // Reset sequential thinking
    this.thoughtHistory = [];
    this.branches = {};

    const summaryText = `Session state cleared:\n- Goal: ${clearedGoal ? `"${clearedGoal}"` : '(none)'}\n- Todos cleared: ${clearedTodos}\n- Memos cleared: ${clearedMemos}\n\nSession is now completely reset.`;
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

  async addMemo(
    memo: string,
  ): Promise<MCPResult<BaseOutput & { memos: Memo[] }>> {
    await db.memos.add({
      sessionId: this.sessionId,
      threadId: this.threadId,
      content: memo,
      createdAt: Date.now(),
    });

    // Enforce MAX_NOTES
    const memos = await this.getMemosList();
    if (memos.length > MAX_NOTES) {
      // Remove oldest
      const oldest = memos[0]; // Sorted by id (auto-inc)
      await db.memos.delete(oldest.id);
    }

    const updatedMemos = await this.getMemosList();
    const capacityWarning =
      updatedMemos.length === MAX_NOTES
        ? `⚠️ At capacity (${MAX_NOTES}/${MAX_NOTES}) - oldest memos will be removed`
        : `Memos: ${updatedMemos.length}/${MAX_NOTES}`;

    // Get the ID of the newly added memo (last one)
    const newMemoId = updatedMemos[updatedMemos.length - 1].id;

    return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
      `Memo ID:${newMemoId} added\n${capacityWarning}`,
      { success: true, memos: updatedMemos },
    );
  }

  async clearMemo(
    id: number,
  ): Promise<MCPResult<BaseOutput & { memos: Memo[] }>> {
    const memo = await db.memos.get(id);
    if (
      !memo ||
      memo.sessionId !== this.sessionId ||
      memo.threadId !== this.threadId
    ) {
      const memos = await this.getMemosList();
      const validIds = memos.map((m) => m.id);
      return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
        `Memo ID:${id} not found.\nValid IDs: ${validIds.length > 0 ? validIds.join(', ') : '(no memos)'}`,
        { success: false, memos },
      );
    }

    await db.memos.delete(id);
    const memos = await this.getMemosList();

    return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
      `Memo ID:${id} cleared: "${memo.content}"\nRemaining memos: ${memos.length}`,
      { success: true, memos },
    );
  }

  async getMemos(): Promise<Memo[]> {
    return this.getMemosList();
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
class SessionStateManager {
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

  private getCurrentState(): PersistentState {
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
    id: number,
    updates: {
      name?: string;
      status?: 'pending' | 'completed' | 'blocked';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): Promise<MCPResult<CheckTodoOutput>> {
    return this.getCurrentState().updateTodo(id, updates);
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

  async addMemo(
    memo: string,
  ): Promise<MCPResult<BaseOutput & { memos: Memo[] }>> {
    return this.getCurrentState().addMemo(memo);
  }

  async removeMemo(
    id: number,
  ): Promise<MCPResult<BaseOutput & { memos: Memo[] }>> {
    return this.getCurrentState().clearMemo(id);
  }

  async getMemos(): Promise<Memo[]> {
    return this.getCurrentState().getMemos();
  }

  async getLastClearedGoal(): Promise<string | null> {
    return this.getCurrentState().getLastClearedGoal();
  }

  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    return this.getCurrentState().processThought(input);
  }

  async checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): Promise<MCPResult<CheckTodoOutput>> {
    return this.getCurrentState().checkTodo(id, check, summary);
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
      memos: await state.getMemos(),
    };
  }
}

const stateManager = new SessionStateManager();

const planningServer: WebMCPServer = {
  name: 'planning',
  displayName: 'Task Planning',
  description: 'Goal setting, task planning',
  version: '2.2.0',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResult<unknown>> {
    console.log(`[PlanningServer] callTool invoked: ${name}`, {
      args,
      currentSessionId: stateManager.getCurrentSessionId(),
      currentThreadId: stateManager.getCurrentThreadId(),
    });

    const typedArgs = (args as Record<string, unknown>) || {};

    if (typeof typedArgs.sessionId === 'string' && typedArgs.sessionId) {
      console.warn(
        `[PlanningServer] callTool: sessionId provided in args ("${String(
          typedArgs.sessionId,
        )}") - ignored. Use switchContext/setContext to change sessions.`,
      );
    }
    if (typeof typedArgs.threadId === 'string' && typedArgs.threadId) {
      console.warn(
        `[PlanningServer] callTool: threadId provided in args ("${String(
          typedArgs.threadId,
        )}") - ignored. Use switchContext/setContext to change threads.`,
      );
    }
    switch (name) {
      case 'create_goal': {
        return await stateManager.createGoal(typedArgs.goal as string);
      }
      case 'update_goal': {
        return await stateManager.updateGoal(typedArgs.goal as string);
      }
      case 'clear_goal': {
        return await stateManager.clearGoal();
      }
      case 'add_todo': {
        return await stateManager.addTodo(
          typedArgs.name as string,
          typedArgs.priority as 'low' | 'medium' | 'high' | undefined,
          typedArgs.dependsOn as number[] | undefined,
        );
      }
      case 'update_todo': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 1) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a positive integer.`,
          );
        }
        return await stateManager.updateTodo(id, {
          name: typedArgs.name as string | undefined,
          status: typedArgs.status as
            | 'pending'
            | 'completed'
            | 'blocked'
            | undefined,
          priority: typedArgs.priority as 'low' | 'medium' | 'high' | undefined,
          dependsOn: typedArgs.dependsOn as number[] | undefined,
        });
      }
      case 'mark_todo': {
        const id = typedArgs.id as number;
        const completed =
          typedArgs.completed !== undefined
            ? (typedArgs.completed as boolean)
            : true;
        const summary = typedArgs.summary as string | undefined;

        if (!Number.isInteger(id) || id < 1) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a positive integer.`,
          );
        }
        return await stateManager.checkTodo(id, completed, summary);
      }
      case 'clear_todos': {
        const ids = typedArgs.ids as number[] | undefined;
        return await stateManager.clearTodos(ids);
      }
      case 'clear_session':
        return await stateManager.clear();
      case 'add_memo': {
        return await stateManager.addMemo(typedArgs.memo as string);
      }
      case 'clear_memo': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 0) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a non-negative integer.`,
          );
        }
        return await stateManager.removeMemo(id);
      }
      case 'sequentialthinking': {
        return stateManager.processThought(typedArgs);
      }
      case 'get_current_state': {
        const includeCompleted = typedArgs.include_completed !== false; // Default true
        const includeMemos = typedArgs.include_memos !== false; // Default true
        const offset =
          typeof typedArgs.offset === 'number' ? typedArgs.offset : 0;
        const limit =
          typeof typedArgs.limit === 'number' ? typedArgs.limit : 50;

        const currentState: PlanningState = {
          goal: await stateManager.getGoal(),
          lastClearedGoal: await stateManager.getLastClearedGoal(),
          todos: await stateManager.getTodos(),
          memos: await stateManager.getMemos(),
        };

        const filteredTodos = includeCompleted
          ? currentState.todos
          : currentState.todos.filter((t) => t.status === 'pending');

        // Calculate metrics
        const totalTodos = currentState.todos.length;
        const pendingCount = currentState.todos.filter(
          (t) => t.status === 'pending',
        ).length;
        const completedCount = currentState.todos.filter(
          (t) => t.status === 'completed',
        ).length;
        const blockedCount = currentState.todos.filter(
          (t) => t.status === 'blocked',
        ).length;

        // Apply pagination to todos
        const paginatedTodos = filteredTodos.slice(offset, offset + limit);

        const todosText = paginatedTodos.length
          ? paginatedTodos
              .map((t) => {
                let checkbox = '[ ]';
                if (t.status === 'completed') checkbox = '[x]';
                else if (t.status === 'blocked') checkbox = '[!]';

                const summaryPart = t.summary ? ` - ${t.summary}` : '';
                const priorityPart = t.priority ? ` [${t.priority}]` : '';
                const dependsPart =
                  t.dependsOn && t.dependsOn.length > 0
                    ? ` (depends on: ${t.dependsOn.join(', ')})`
                    : '';
                return `- ID:${t.id} ${checkbox} ${t.name}${priorityPart}${dependsPart}${summaryPart}`;
              })
              .join('\n')
          : '- (none)';

        const notesText =
          includeMemos && currentState.memos.length
            ? currentState.memos
                .map((m) => `- [ID: ${m.id}] ${m.content.replace(/\n/g, ' ')}`)
                .join('\n')
            : '- (none)';

        const lines: string[] = [];
        lines.push('# Planning State', '');
        lines.push('**Summary**');
        lines.push(`- Total Todos: ${totalTodos}`);
        lines.push(`  - Pending: ${pendingCount}`);
        lines.push(`  - Completed: ${completedCount}`);
        if (blockedCount > 0) lines.push(`  - Blocked: ${blockedCount}`);
        lines.push(`- Scratchpad Items: ${currentState.memos.length}`, '');
        lines.push('**Goal**');
        lines.push(currentState.goal ? `- ${currentState.goal}` : '- (none)');
        if (currentState.lastClearedGoal) {
          lines.push(
            '',
            '**Last Cleared Goal**',
            `- ${currentState.lastClearedGoal}`,
          );
        }
        lines.push('', '**Todos**');
        if (offset > 0 || filteredTodos.length > limit) {
          lines.push(
            `(Showing ${offset + 1}-${offset + paginatedTodos.length} of ${filteredTodos.length})`,
          );
        }
        lines.push(todosText);

        if (includeMemos) {
          lines.push('', '**Scratchpad**');
          lines.push(notesText);
        }

        return createMCPStructuredToolResult(lines.join('\n'), currentState);
      }
    }

    return createMCPErrorToolResult(`Unknown tool: ${name}`);
  },

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<PlanningState>> {
    const sessionId = options?.sessionId || 'default';
    const threadId = options?.threadId || sessionId;

    // Ensure session is set
    stateManager.setSession(sessionId, threadId);

    // We can use getStateForSession to get the state even if we just switched
    const state = await stateManager.getStateForSession(sessionId, threadId);

    if (!state) {
      // Should not happen as setSession creates it
      return {
        contextPrompt: 'Error: Could not retrieve planning state.',
        structuredState: {
          goal: null,
          lastClearedGoal: null,
          todos: [],
          memos: [],
        },
      };
    }

    const { goal, todos, memos } = state;
    const pendingTodos = todos.filter((t) => t.status === 'pending');

    const contextParts = [];
    if (goal) {
      contextParts.push(`Current Goal: "${goal}"`);
    }
    if (pendingTodos.length > 0) {
      contextParts.push(`Pending Todos (${pendingTodos.length}):`);
      pendingTodos.slice(0, 5).forEach((t) => {
        contextParts.push(`- [ ] ${t.name}`);
      });
      if (pendingTodos.length > 5) {
        contextParts.push(`...and ${pendingTodos.length - 5} more`);
      }
    }
    if (memos.length > 0) {
      contextParts.push(`Scratchpad (${memos.length}):`);
      memos.slice(0, 3).forEach((m) => {
        contextParts.push(`- ${m.content}`);
      });
    }

    return {
      contextPrompt:
        contextParts.length > 0
          ? contextParts.join('\n')
          : 'No active plan or todos.',
      structuredState: state,
    };
  },

  async switchContext(options: ServiceContextOptions): Promise<void> {
    const sessionId = options.sessionId || 'default';
    const threadId = options.threadId || sessionId;
    stateManager.setSession(sessionId, threadId);
    logger.info(
      `Switched planning context to session: ${sessionId}, thread: ${threadId}`,
    );
  },
};

export interface PlanningServerProxy extends WebMCPServerProxy {
  create_goal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  update_goal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  clear_goal(): Promise<MCPResult<ClearGoalOutput>>;
  add_todo(args: {
    name: string;
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }): Promise<MCPResult<AddToDoOutput>>;
  update_todo(args: {
    id: number;
    name?: string;
    status?: 'pending' | 'completed' | 'blocked';
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }): Promise<MCPResult<CheckTodoOutput>>;
  mark_todo(args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }): Promise<MCPResult<CheckTodoOutput>>;
  clear_todos(args: { ids?: number[] }): Promise<MCPResult<BaseOutput>>;
  clear_session(): Promise<MCPResult<BaseOutput>>;
  add_memo(args: {
    memo: string;
  }): Promise<MCPResult<BaseOutput & { memos: Memo[] }>>;
  clear_memo(args: {
    id: number;
  }): Promise<MCPResult<BaseOutput & { memos: Memo[] }>>;
  get_current_state(args: {
    include_completed?: boolean;
    include_memos?: boolean;
  }): Promise<MCPResult<unknown>>;
  sequentialthinking(args: unknown): Promise<MCPResult<unknown>>;
}

export default planningServer;
