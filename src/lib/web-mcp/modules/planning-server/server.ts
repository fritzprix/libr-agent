import { WebMCPServerProxy } from '@/context/WebMCPContext';
import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { getLogger } from '@/lib/logger';
import { planningTools as tools } from './tools.ts';

const logger = getLogger('PlanningServer');

/** Represents a single to-do item in the planning state. @internal */
interface SimpleTodo {
  id: number;
  name: string;
  status: 'pending' | 'completed';
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

const MAX_NOTES = 50;

/**
 * Manages the in-memory state for the planning server, including goals,
 * to-dos, and notes. This state is not persisted and will be lost
 * when the worker is terminated.
 * @internal
 */
class EphemeralState {
  private goal: string | null = null;
  private lastClearedGoal: string | null = null;
  private todos: SimpleTodo[] = [];
  private memos: Memo[] = [];
  private nextId = 1;
  // Sequential thinking state
  private thoughtHistory: ThoughtData[] = [];
  private branches: Record<string, ThoughtData[]> = {};
  private disableThoughtLogging = false;

  createGoal(goal: string): MCPResult<CreateGoalOutput> {
    const previousGoal = this.goal;
    this.goal = goal;
    const context = [];
    if (previousGoal) {
      context.push(`Previous goal: "${previousGoal}"`);
      context.push(`Todos from previous goal: ${this.todos.length}`);
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

  updateGoal(goal: string): MCPResult<CreateGoalOutput> {
    if (!this.goal) {
      return createMCPStructuredToolResult(
        'No active goal to update. Use create_goal first.',
        {
          success: false,
          goal: '',
        },
      );
    }
    const oldGoal = this.goal;
    this.goal = goal;
    return createMCPStructuredToolResult<CreateGoalOutput>(
      `Goal updated from "${oldGoal}" to "${goal}"`,
      {
        goal,
        success: true,
      },
    );
  }

  clearGoal(): MCPResult<ClearGoalOutput> {
    if (this.goal) {
      const clearedGoal = this.goal;
      this.lastClearedGoal = this.goal;
      this.goal = null;
      const remainingTodos = this.todos.length;
      const todoSummary =
        remainingTodos > 0
          ? `Remaining todos: ${remainingTodos}`
          : 'All todos have been completed or cleared.';
      return createMCPStructuredToolResult<ClearGoalOutput>(
        `Goal cleared: "${clearedGoal}"\n${todoSummary}\nSession is now ready for a new goal.`,
        {
          success: true,
        },
      );
    }
    return createMCPStructuredToolResult('No active goal to clear', {
      success: false,
    });
  }

  addTodo(
    name: string,
    priority?: 'low' | 'medium' | 'high',
    dependsOn?: number[],
  ): MCPResult<AddToDoOutput> {
    const todo: SimpleTodo = {
      id: this.nextId++,
      name,
      status: 'pending',
      priority,
      dependsOn,
    };
    this.todos.push(todo);
    const goalContext = this.goal
      ? `Goal: "${this.goal}"\n`
      : 'No active goal.\n';
    return createMCPStructuredToolResult<AddToDoOutput>(
      `Todo added: ID:${todo.id} "${name}"\n${goalContext}Total todos: ${this.todos.length}`,
      {
        success: true,
        todos: this.todos,
      },
    );
  }

  updateTodo(
    id: number,
    updates: {
      name?: string;
      status?: 'pending' | 'completed';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): MCPResult<CheckTodoOutput> {
    const todo = this.todos.find((t) => t.id === id);
    if (!todo) {
      const availableIds = this.todos.map((t) => t.id);
      return createMCPStructuredToolResult<CheckTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos: this.todos,
        },
      );
    }

    if (updates.name !== undefined) todo.name = updates.name;
    if (updates.status !== undefined) todo.status = updates.status;
    if (updates.priority !== undefined) todo.priority = updates.priority;
    if (updates.dependsOn !== undefined) todo.dependsOn = updates.dependsOn;

    return createMCPStructuredToolResult<CheckTodoOutput>(
      `Todo ${id} updated: "${todo.name}"`,
      {
        success: true,
        todo,
        todos: this.todos,
      },
    );
  }

  checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): MCPResult<CheckTodoOutput> {
    const todo = this.todos.find((t) => t.id === id);
    if (!todo) {
      const availableIds = this.todos.map((t) => t.id);
      return createMCPStructuredToolResult<CheckTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos: this.todos,
        },
      );
    }

    todo.status = check ? 'completed' : 'pending';
    if (summary !== undefined) {
      todo.summary = summary || undefined;
    }

    const summaryText = todo.summary ? ` (Summary: "${todo.summary}")` : '';
    return createMCPStructuredToolResult<CheckTodoOutput>(
      `Todo ${check ? 'checked' : 'unchecked'}: "${todo.name}"${summaryText}`,
      {
        success: true,
        todo,
        todos: this.todos,
      },
    );
  }

  clearTodos(ids?: number[]): MCPResult<BaseOutput> {
    if (!ids || ids.length === 0) {
      const clearedCount = this.todos.length;
      this.todos = [];
      const msg =
        clearedCount > 0
          ? `All ${clearedCount} todo(s) cleared`
          : 'No todos to clear.';
      return createMCPStructuredToolResult<BaseOutput>(
        `${msg}\nSession todos reset. Current goal: ${this.goal || '(none)'}`,
        {
          success: true,
        },
      );
    }

    const initialCount = this.todos.length;
    const clearedTodos = this.todos.filter((todo) => ids.includes(todo.id));
    this.todos = this.todos.filter((todo) => !ids.includes(todo.id));
    const removedCount = initialCount - this.todos.length;

    if (removedCount === 0) {
      return createMCPStructuredToolResult<BaseOutput>(
        `No todos found with the specified IDs: ${ids.join(', ')}\nAvailable IDs: ${initialCount > 0 ? this.todos.map((t) => t.id).join(', ') : '(none)'}`,
        { success: false },
      );
    }

    const clearedNames = clearedTodos.map((t) => t.name).join(', ');
    return createMCPStructuredToolResult<BaseOutput>(
      `Cleared ${removedCount} todo(s): ${clearedNames}\nRemaining todos: ${this.todos.length}`,
      { success: true },
    );
  }

  clear(): MCPResult<BaseOutput> {
    const clearedGoal = this.goal;
    const clearedTodos = this.todos.length;
    const clearedMemos = this.memos.length;

    this.goal = null;
    this.lastClearedGoal = null;
    this.todos = [];
    this.memos = [];
    this.nextId = 1;

    const summaryText = `Session state cleared:\n- Goal: ${clearedGoal ? `"${clearedGoal}"` : '(none)'}\n- Todos cleared: ${clearedTodos}\n- Memos cleared: ${clearedMemos}\n\nSession is now completely reset.`;
    return createMCPStructuredToolResult(summaryText, {
      success: true,
    });
  }

  getGoal(): string | null {
    return this.goal;
  }

  getTodos(): SimpleTodo[] {
    return this.todos;
  }

  addMemo(memo: string): MCPResult<BaseOutput & { memos: Memo[] }> {
    const nMeme = {
      id: this.memos.length > 0 ? this.memos[this.memos.length - 1].id + 1 : 0,
      content: memo,
    };
    this.memos.push(nMeme);
    if (this.memos.length > MAX_NOTES) {
      this.memos.shift();
    }
    const capacityWarning =
      this.memos.length === MAX_NOTES
        ? `⚠️ At capacity (${MAX_NOTES}/${MAX_NOTES}) - oldest memos will be removed`
        : `Memos: ${this.memos.length}/${MAX_NOTES}`;
    return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
      `Memo ID:${nMeme.id} added\n${capacityWarning}`,
      { success: true, memos: [...this.memos] },
    );
  }

  clearMemo(id: number): MCPResult<BaseOutput & { memos: Memo[] }> {
    const index = this.memos.findIndex((memo) => memo.id === id);
    if (index === -1) {
      const validIds = this.memos.map((memo) => memo.id);
      return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
        `Memo ID:${id} not found.\nValid IDs: ${validIds.length > 0 ? validIds.join(', ') : '(no memos)'}`,
        { success: false, memos: [...this.memos] },
      );
    }
    const removed = this.memos.splice(index, 1)[0];
    return createMCPStructuredToolResult<BaseOutput & { memos: Memo[] }>(
      `Memo ID:${id} cleared: "${removed.content}"\nRemaining memos: ${this.memos.length}`,
      { success: true, memos: [...this.memos] },
    );
  }

  getMemos(): Memo[] {
    return [...this.memos];
  }

  getLastClearedGoal(): string | null {
    return this.lastClearedGoal;
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
 * Session-based state manager that maintains separate EphemeralState instances
 * for each (sessionId, threadId) pair.
 * @internal
 */
class SessionStateManager {
  private sessions = new Map<string, Map<string, EphemeralState>>();
  private currentSessionId: string | null = null;
  private currentThreadId: string | null = null;

  setSession(sessionId: string, threadId?: string): void {
    const effectiveThreadId = threadId || sessionId;

    if (this.currentSessionId && this.currentSessionId !== sessionId) {
      const oldThreadMap = this.sessions.get(this.currentSessionId);
      if (oldThreadMap) {
        oldThreadMap.clear();
      }
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

  private getState(sessionId: string, threadId: string): EphemeralState {
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, new Map());
    }
    const threadMap = this.sessions.get(sessionId)!;
    if (!threadMap.has(threadId)) {
      threadMap.set(threadId, new EphemeralState());
    }
    return threadMap.get(threadId)!;
  }

  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) {
      this.setSession('default');
    }
    const effectiveThreadId = this.currentThreadId || this.currentSessionId!;
    return this.getState(this.currentSessionId!, effectiveThreadId);
  }

  createGoal(goal: string): MCPResult<CreateGoalOutput> {
    return this.getCurrentState().createGoal(goal);
  }

  updateGoal(goal: string): MCPResult<CreateGoalOutput> {
    return this.getCurrentState().updateGoal(goal);
  }

  clearGoal(): MCPResult<ClearGoalOutput> {
    return this.getCurrentState().clearGoal();
  }

  addTodo(
    name: string,
    priority?: 'low' | 'medium' | 'high',
    dependsOn?: number[],
  ): MCPResult<AddToDoOutput> {
    return this.getCurrentState().addTodo(name, priority, dependsOn);
  }

  updateTodo(
    id: number,
    updates: {
      name?: string;
      status?: 'pending' | 'completed';
      priority?: 'low' | 'medium' | 'high';
      dependsOn?: number[];
    },
  ): MCPResult<CheckTodoOutput> {
    return this.getCurrentState().updateTodo(id, updates);
  }

  clearTodos(ids?: number[]): MCPResult<BaseOutput> {
    return this.getCurrentState().clearTodos(ids);
  }

  clear(): MCPResult<BaseOutput> {
    return this.getCurrentState().clear();
  }

  getGoal(): string | null {
    return this.getCurrentState().getGoal();
  }

  getTodos(): SimpleTodo[] {
    return this.getCurrentState().getTodos();
  }

  addMemo(memo: string): MCPResult<BaseOutput & { memos: Memo[] }> {
    return this.getCurrentState().addMemo(memo);
  }

  removeMemo(id: number): MCPResult<BaseOutput & { memos: Memo[] }> {
    return this.getCurrentState().clearMemo(id);
  }

  getMemos(): Memo[] {
    return this.getCurrentState().getMemos();
  }

  getLastClearedGoal(): string | null {
    return this.getCurrentState().getLastClearedGoal();
  }

  processThought(input: unknown): MCPResult<Record<string, unknown>> {
    return this.getCurrentState().processThought(input);
  }

  checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): MCPResult<CheckTodoOutput> {
    return this.getCurrentState().checkTodo(id, check, summary);
  }

  clearAllSessions(): void {
    for (const [sessionId, threadMap] of this.sessions.entries()) {
      threadMap.clear();
      this.sessions.delete(sessionId);
    }
    this.currentSessionId = null;
    this.currentThreadId = null;
  }

  getStateForSession(
    sessionId: string,
    threadId?: string,
  ): PlanningState | null {
    const effectiveThreadId = threadId || sessionId;
    const threadMap = this.sessions.get(sessionId);
    if (!threadMap) {
      return null;
    }
    const state = threadMap.get(effectiveThreadId);
    if (!state) {
      return null;
    }
    return {
      goal: state.getGoal(),
      lastClearedGoal: state.getLastClearedGoal(),
      todos: state.getTodos(),
      memos: state.getMemos(),
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
        return stateManager.createGoal(typedArgs.goal as string);
      }
      case 'update_goal': {
        return stateManager.updateGoal(typedArgs.goal as string);
      }
      case 'clear_goal': {
        return stateManager.clearGoal();
      }
      case 'add_todo': {
        return stateManager.addTodo(
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
        return stateManager.updateTodo(id, {
          name: typedArgs.name as string | undefined,
          status: typedArgs.status as 'pending' | 'completed' | undefined,
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
        return stateManager.checkTodo(id, completed, summary);
      }
      case 'clear_todos': {
        const ids = typedArgs.ids as number[] | undefined;
        return stateManager.clearTodos(ids);
      }
      case 'clear_session':
        return stateManager.clear();
      case 'add_memo': {
        return stateManager.addMemo(typedArgs.memo as string);
      }
      case 'clear_memo': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 0) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a non-negative integer.`,
          );
        }
        return stateManager.removeMemo(id);
      }
      case 'sequentialthinking': {
        return stateManager.processThought(typedArgs);
      }
      case 'get_current_state': {
        const includeCompleted = typedArgs.include_completed !== false; // Default true
        const includeMemos = typedArgs.include_memos !== false; // Default true

        const currentState: PlanningState = {
          goal: stateManager.getGoal(),
          lastClearedGoal: stateManager.getLastClearedGoal(),
          todos: stateManager.getTodos(),
          memos: stateManager.getMemos(),
        };

        const filteredTodos = includeCompleted
          ? currentState.todos
          : currentState.todos.filter((t) => t.status === 'pending');

        const todosText = filteredTodos.length
          ? filteredTodos
              .map((t) => {
                const checkbox = t.status === 'completed' ? '✓' : ' ';
                const summaryPart = t.summary ? ` - ${t.summary}` : '';
                const priorityPart = t.priority ? ` [${t.priority}]` : '';
                const dependsPart =
                  t.dependsOn && t.dependsOn.length > 0
                    ? ` (depends on: ${t.dependsOn.join(', ')})`
                    : '';
                return `- ID:${t.id} [${checkbox}] ${t.name}${priorityPart}${dependsPart}${summaryPart}`;
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
        lines.push(`- Todos: ${currentState.todos.length}`);
        lines.push(`- Memos: ${currentState.memos.length}`, '');
        lines.push('**Goal**');
        lines.push(currentState.goal ? `- ${currentState.goal}` : '- (none)');
        if (currentState.lastClearedGoal) {
          lines.push(
            '',
            '**Last Cleared Goal**',
            `- ${currentState.lastClearedGoal}`,
          );
        }
        lines.push('', '**Todos**', todosText);

        if (includeMemos) {
          lines.push('', '**Memos**', notesText);
        }

        const detailedText = lines.join('\n');

        return createMCPStructuredToolResult<PlanningState>(
          detailedText,
          currentState,
        );
      }
      default: {
        const errorMessage = `Unknown tool: ${name}`;
        console.error(`[PlanningServer] ${errorMessage}`);
        return createMCPErrorToolResult(errorMessage, {
          toolName: name,
          availableTools: tools.map((t) => t.name),
        });
      }
    }
  },
  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<PlanningState>> {
    let planningState: PlanningState | null = null;

    if (options?.sessionId) {
      planningState = stateManager.getStateForSession(
        options.sessionId,
        options.threadId,
      );
      if (!planningState) {
        logger.debug('getServiceContext: requested session/thread not found', {
          sessionId: options.sessionId,
          threadId: options.threadId,
        });
        return {
          contextPrompt: '# No active goal',
          structuredState: {
            goal: null,
            lastClearedGoal: null,
            todos: [],
            memos: [],
          },
        };
      }
    } else {
      const goal = stateManager.getGoal();
      const todos = stateManager.getTodos();
      const memos = stateManager.getMemos();
      planningState = {
        goal,
        lastClearedGoal: stateManager.getLastClearedGoal(),
        todos,
        memos,
      };
    }

    const { goal, todos, memos } = planningState;

    const todosPrompt =
      todos.length > 0
        ? todos
            .map((t) => {
              const status = t.status === 'completed' ? '[✓]' : '[ ]';
              const summaryPart = t.summary ? ` (${t.summary})` : '';
              return `ID:${t.id} ${status} ${t.name}${summaryPart}`;
            })
            .join(', ')
        : '(none)';

    const contextPrompt = goal
      ? `# Current Goal: ${goal}
Todos: ${todosPrompt}
Recent Notes: ${
          memos.length > 0
            ? memos
                .slice(-2)
                .map((m) => `(ID: ${m.id}: ${m.content})`)
                .join('; ')
            : '(none)'
        }`
      : '# No active goal';

    return {
      contextPrompt,
      structuredState: {
        goal,
        lastClearedGoal: planningState.lastClearedGoal,
        todos,
        memos,
      },
    };
  },
  async switchContext(context: ServiceContextOptions): Promise<void> {
    const sessionId = context.sessionId;
    const threadId = context.threadId;
    if (sessionId) {
      stateManager.setSession(sessionId, threadId);
      console.info(
        `[PlanningServer] switchContext -> session: ${sessionId}, thread: ${threadId || sessionId} (previous session cleaned up)`,
      );
    }
  },
};

/**
 * Extends the `WebMCPServerProxy` with typed methods for the planning server's tools.
 */
export interface PlanningServerProxy extends WebMCPServerProxy {
  create_goal: (args: { goal: string }) => Promise<CreateGoalOutput>;
  update_goal: (args: { goal: string }) => Promise<CreateGoalOutput>;
  clear_goal: () => Promise<ClearGoalOutput>;
  add_todo: (args: {
    name: string;
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }) => Promise<AddToDoOutput>;
  update_todo: (args: {
    id: number;
    name?: string;
    status?: 'pending' | 'completed';
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }) => Promise<CheckTodoOutput>;
  mark_todo: (args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }) => Promise<CheckTodoOutput>;
  clear_todos: (args?: { ids?: number[] }) => Promise<BaseOutput>;
  clear_session: () => Promise<BaseOutput>;
  add_memo: (args: { memo: string }) => Promise<BaseOutput & { memos: Memo[] }>;
  clear_memo: (args: { id: number }) => Promise<BaseOutput & { memos: Memo[] }>;
  get_current_state: (args?: {
    include_completed?: boolean;
    include_memos?: boolean;
  }) => Promise<PlanningState>;
  sequentialthinking: (args: {
    thought: string;
    nextThoughtNeeded: boolean;
    thoughtNumber: number;
    totalThoughts: number;
    isRevision?: boolean;
    revisesThought?: number;
    branchFromThought?: number;
    branchId?: string;
    needsMoreThoughts?: boolean;
  }) => Promise<Record<string, unknown>>;
}

export default planningServer;
