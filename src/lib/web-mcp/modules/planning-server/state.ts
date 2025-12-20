import { createMCPStructuredToolResult } from '@/lib/mcp-response-utils';
import type { MCPResult } from '@/lib/mcp-types';
import type {
  SimpleTodo,
  ScratchpadItem,
  PlanningState,
  BaseOutput,
  CreateGoalOutput,
  ClearGoalOutput,
  AddToDoOutput,
} from './types';
import { GoalManager } from './managers/goal-manager';
import { TodoManager } from './managers/todo-manager';
import { ScratchpadManager } from './managers/scratchpad-manager';
import { ThinkingManager } from './managers/thinking-manager';
import { getLogger } from '@/lib/logger';

const logger = getLogger('PersistentState');

/**
 * Manages the persistent state for the planning server using Dexie.js.
 * Coordinates between specialized managers for goals, todos, scratchpad, and thinking state.
 * Goals, Todos, and Scratchpad are persisted.
 * Sequential Thinking state remains ephemeral (in-memory).
 * @internal
 */
export class PersistentState {
  private goalManager: GoalManager;
  private todoManager: TodoManager;
  private scratchpadManager: ScratchpadManager;
  private thinkingManager: ThinkingManager;

  constructor(sessionId: string, threadId: string) {
    this.goalManager = new GoalManager(sessionId, threadId);
    this.todoManager = new TodoManager(sessionId, threadId);
    this.scratchpadManager = new ScratchpadManager(sessionId, threadId);
    this.thinkingManager = new ThinkingManager();
  }

  async createGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    const existingTodosCount = (await this.todoManager.getTodos()).length;
    return this.goalManager.createGoal(goal, existingTodosCount);
  }

  async updateGoal(goal: string): Promise<MCPResult<CreateGoalOutput>> {
    return this.goalManager.updateGoal(goal);
  }

  async clearGoal(): Promise<MCPResult<ClearGoalOutput>> {
    const remainingTodosCount = (await this.todoManager.getTodos()).length;
    return this.goalManager.clearGoal(remainingTodosCount);
  }

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
  ): Promise<MCPResult<AddToDoOutput>> {
    const activeGoalContent = await this.goalManager.getGoal();
    return this.todoManager.addTodo(
      title,
      description,
      priority,
      parentId,
      subtasks,
      activeGoalContent || null,
    );
  }

  async checkTodo(
    params: { id?: number; index?: number },
    checked: boolean = true,
    summary?: string,
  ): Promise<MCPResult<unknown>> {
    return this.todoManager.checkTodo(params, checked, summary);
  }

  async clearTodos(
    ids?: number[],
    indices?: number[],
  ): Promise<MCPResult<BaseOutput>> {
    const activeGoalContent = await this.goalManager.getGoal();
    return this.todoManager.clearTodos(ids, indices, activeGoalContent);
  }

  async addScratchpad(
    note: string,
    source?: string,
    title?: string,
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.scratchpadManager.addScratchpad(note, source, title, tags);
  }

  async readScratchpad(
    ids?: number[],
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.scratchpadManager.readScratchpad(ids, tags);
  }

  async clearScratchpad(
    id: number,
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.scratchpadManager.clearScratchpad(id);
  }

  async clear(): Promise<MCPResult<BaseOutput>> {
    const activeGoal = await this.goalManager.getGoal();
    const todos = await this.todoManager.getTodos();
    const scratchpad = await this.scratchpadManager.getScratchpadList();

    const clearedGoal = activeGoal;
    const clearedTodos = todos.length;
    const clearedScratchpad = scratchpad.length;

    // Clear all managers
    await Promise.all([
      this.goalManager.clearAllGoals(),
      this.todoManager.clearAllTodos(),
      this.scratchpadManager.clearAllScratchpad(),
    ]);

    // Reset sequential thinking
    this.thinkingManager.reset();

    const summaryText = `Session state cleared:\n- Goal: ${clearedGoal ? `"${clearedGoal}"` : '(none)'}\n- Todos cleared: ${clearedTodos}\n- Scratchpad items cleared: ${clearedScratchpad}\n\nSession is now completely reset.`;
    return createMCPStructuredToolResult(summaryText, {
      success: true,
    });
  }

  async getGoal(): Promise<string | null> {
    return this.goalManager.getGoal();
  }

  async getTodos(): Promise<SimpleTodo[]> {
    return this.todoManager.getTodos();
  }

  async getScratchpadList(): Promise<ScratchpadItem[]> {
    return this.scratchpadManager.getScratchpadList();
  }

  async getLastClearedGoal(): Promise<string | null> {
    return this.goalManager.getLastClearedGoal();
  }

  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    return this.thinkingManager.processThought(input);
  }

  processCritiqueAndReflection(
    input: unknown,
  ): MCPResult<Record<string, unknown>> {
    return this.thinkingManager.processCritiqueAndReflection(input);
  }

  processPauseAndThink(input: unknown): MCPResult<Record<string, unknown>> {
    return this.thinkingManager.processPauseAndThink(input);
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
      logger.info(
        `Session cleanup: removed all threads from session "${this.currentSessionId}"`,
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
    title: string,
    description?: string,
    priority?: 'low' | 'medium' | 'high',
    parentId?: number,
    subtasks?: Array<{
      title: string;
      description?: string;
      priority?: 'low' | 'medium' | 'high';
    }>,
  ): Promise<MCPResult<AddToDoOutput>> {
    return this.getCurrentState().addTodo(
      title,
      description,
      priority,
      parentId,
      subtasks,
    );
  }

  async clearTodos(
    ids?: number[],
    indices?: number[],
  ): Promise<MCPResult<BaseOutput>> {
    return this.getCurrentState().clearTodos(ids, indices);
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
    title?: string,
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.getCurrentState().addScratchpad(note, source, title, tags);
  }

  async readScratchpad(
    ids?: number[],
    tags?: string[],
  ): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>> {
    return this.getCurrentState().readScratchpad(ids, tags);
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

  processCritiqueAndReflection(
    input: unknown,
  ): MCPResult<Record<string, unknown>> {
    return this.getCurrentState().processCritiqueAndReflection(input);
  }

  processPauseAndThink(input: unknown): MCPResult<Record<string, unknown>> {
    return this.getCurrentState().processPauseAndThink(input);
  }

  async checkTodo(
    params: { id?: number; index?: number },
    checked: boolean = true,
    summary?: string,
  ): Promise<MCPResult<unknown>> {
    return this.getCurrentState().checkTodo(params, checked, summary);
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
