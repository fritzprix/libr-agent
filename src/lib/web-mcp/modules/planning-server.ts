import { WebMCPServerProxy } from '@/context/WebMCPContext';
import {
  createMCPStructuredResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, MCPTool, WebMCPServer } from '@/lib/mcp-types';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { getLogger } from '@/lib/logger';

const logger = getLogger('PlanningServer');

/** Represents a single to-do item in the planning state. @internal */
interface SimpleTodo {
  id: number;
  name: string;
  status: 'pending' | 'completed';
  summary?: string;
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

// `toggle_todo` has been removed: toggle functionality is deprecated.

/**
 * The output for the `check_todo` tool call.
 * @internal
 */
interface CheckTodoOutput extends BaseOutput {
  todo: SimpleTodo | null;
  todos: SimpleTodo[];
}

const MAX_NOTES = 10;

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

  createGoal(goal: string): MCPResponse<CreateGoalOutput> {
    const previousGoal = this.goal;
    this.goal = goal;
    const context = [];
    if (previousGoal) {
      context.push(`Previous goal: "${previousGoal}"`);
      context.push(`Todos from previous goal: ${this.todos.length}`);
    }
    const contextStr = context.length > 0 ? `\n${context.join('\n')}\n` : '';
    return createMCPStructuredResponse<CreateGoalOutput>(
      `Goal created: "${goal}"${contextStr}New todos can be added to support this goal.`,
      {
        goal,
        success: true,
      },
    );
  }

  clearGoal(): MCPResponse<ClearGoalOutput> {
    if (this.goal) {
      const clearedGoal = this.goal;
      this.lastClearedGoal = this.goal;
      this.goal = null;
      const remainingTodos = this.todos.length;
      const todoSummary =
        remainingTodos > 0
          ? `Remaining todos: ${remainingTodos}`
          : 'All todos have been completed or cleared.';
      return createMCPStructuredResponse<ClearGoalOutput>(
        `Goal cleared: "${clearedGoal}"\n${todoSummary}\nSession is now ready for a new goal.`,
        {
          success: true,
        },
      );
    }
    return createMCPStructuredResponse('No active goal to clear', {
      success: false,
    });
  }

  addTodo(name: string): MCPResponse<AddToDoOutput> {
    const todo: SimpleTodo = {
      id: this.nextId++,
      name,
      status: 'pending',
    };
    this.todos.push(todo);
    const goalContext = this.goal
      ? `Goal: "${this.goal}"\n`
      : 'No active goal.\n';
    return createMCPStructuredResponse<AddToDoOutput>(
      `Todo added: ID:${todo.id} "${name}"\n${goalContext}Total todos: ${this.todos.length}`,
      {
        success: true,
        todos: this.todos,
      },
    );
  }

  // toggle functionality removed; use remove_todo / clear_todos / add_todo
  // to manage todo lifecycle programmatically.

  checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): MCPResponse<CheckTodoOutput> {
    const todo = this.todos.find((t) => t.id === id);
    if (!todo) {
      const availableIds = this.todos.map((t) => t.id);
      return createMCPStructuredResponse<CheckTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos: this.todos,
        },
      );
    }

    todo.status = check ? 'completed' : 'pending';
    // Treat empty string as undefined
    if (summary !== undefined) {
      todo.summary = summary || undefined;
    }

    const summaryText = todo.summary ? ` (Summary: "${todo.summary}")` : '';
    return createMCPStructuredResponse<CheckTodoOutput>(
      `Todo ${check ? 'checked' : 'unchecked'}: "${todo.name}"${summaryText}`,
      {
        success: true,
        todo,
        todos: this.todos,
      },
    );
  }

  clearTodos(ids?: number[]): MCPResponse<BaseOutput> {
    if (!ids || ids.length === 0) {
      // Clear all todos if no IDs specified
      const clearedCount = this.todos.length;
      this.todos = [];
      const msg =
        clearedCount > 0
          ? `All ${clearedCount} todo(s) cleared`
          : 'No todos to clear.';
      return createMCPStructuredResponse<BaseOutput>(
        `${msg}\nSession todos reset. Current goal: ${this.goal || '(none)'}`,
        {
          success: true,
        },
      );
    }

    // Clear specific todos by IDs
    const initialCount = this.todos.length;
    const clearedTodos = this.todos.filter((todo) => ids.includes(todo.id));
    this.todos = this.todos.filter((todo) => !ids.includes(todo.id));
    const removedCount = initialCount - this.todos.length;

    if (removedCount === 0) {
      return createMCPStructuredResponse<BaseOutput>(
        `No todos found with the specified IDs: ${ids.join(', ')}\nAvailable IDs: ${initialCount > 0 ? this.todos.map((t) => t.id).join(', ') : '(none)'}`,
        { success: false },
      );
    }

    const clearedNames = clearedTodos.map((t) => t.name).join(', ');
    return createMCPStructuredResponse<BaseOutput>(
      `Cleared ${removedCount} todo(s): ${clearedNames}\nRemaining todos: ${this.todos.length}`,
      { success: true },
    );
  }

  clear(): MCPResponse<BaseOutput> {
    const clearedGoal = this.goal;
    const clearedTodos = this.todos.length;
    const clearedMemos = this.memos.length;

    this.goal = null;
    this.lastClearedGoal = null;
    this.todos = [];
    this.memos = [];
    this.nextId = 1;

    const summaryText = `Session state cleared:\n- Goal: ${clearedGoal ? `"${clearedGoal}"` : '(none)'}\n- Todos cleared: ${clearedTodos}\n- Memos cleared: ${clearedMemos}\n\nSession is now completely reset.`;
    return createMCPStructuredResponse(summaryText, {
      success: true,
    });
  }

  getGoal(): string | null {
    return this.goal;
  }

  getTodos(): SimpleTodo[] {
    return this.todos;
  }

  addMemo(memo: string): MCPResponse<BaseOutput & { memos: Memo[] }> {
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
    return createMCPStructuredResponse<BaseOutput & { memos: Memo[] }>(
      `Memo ID:${nMeme.id} added\n${capacityWarning}`,
      { success: true, memos: [...this.memos] },
    );
  }

  clearMemo(id: number): MCPResponse<BaseOutput & { memos: Memo[] }> {
    const index = this.memos.findIndex((memo) => memo.id === id);
    if (index === -1) {
      const validIds = this.memos.map((memo) => memo.id);
      return createMCPStructuredResponse<BaseOutput & { memos: Memo[] }>(
        `Memo ID:${id} not found.\nValid IDs: ${validIds.length > 0 ? validIds.join(', ') : '(no memos)'}`,
        { success: false, memos: [...this.memos] },
      );
    }
    const removed = this.memos.splice(index, 1)[0];
    return createMCPStructuredResponse<BaseOutput & { memos: Memo[] }>(
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

  processThought(input: unknown): MCPResponse<Record<string, unknown>> {
    try {
      const data = input as Record<string, unknown>;

      if (!data.thought || typeof data.thought !== 'string') {
        throw new Error('Invalid thought: must be a string');
      }
      if (
        data.thoughtNumber === undefined ||
        typeof data.thoughtNumber !== 'number'
      ) {
        throw new Error('Invalid thoughtNumber: must be a number');
      }
      if (
        data.totalThoughts === undefined ||
        typeof data.totalThoughts !== 'number'
      ) {
        throw new Error('Invalid totalThoughts: must be a number');
      }
      if (typeof data.nextThoughtNeeded !== 'boolean') {
        throw new Error('Invalid nextThoughtNeeded: must be a boolean');
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
        // lightweight console output for server logs
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

      return createMCPStructuredResponse('Thought processed', summary);
    } catch (error) {
      return createMCPStructuredResponse('Failed to process thought', {
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

  /**
   * Sets the current session context and cleans up the previous session's state.
   * @param sessionId The session ID to set as current context.
   * @param threadId The thread ID to set as current context (optional, defaults to sessionId).
   */
  setSession(sessionId: string, threadId?: string): void {
    const effectiveThreadId = threadId || sessionId;

    // ✅ CLEANUP: Remove previous session's all thread states
    if (this.currentSessionId && this.currentSessionId !== sessionId) {
      const oldThreadMap = this.sessions.get(this.currentSessionId);
      if (oldThreadMap) {
        // Clear all threads in old session
        oldThreadMap.clear();
      }
      // Remove the session entry entirely
      this.sessions.delete(this.currentSessionId);
      console.info(
        `[PlanningServer] Session cleanup: removed all threads from session "${this.currentSessionId}"`,
      );
    }

    // Set new session (lazy initialization)
    this.currentSessionId = sessionId;
    this.currentThreadId = effectiveThreadId;
  }

  /**
   * Returns the currently active session id for diagnostics.
   */
  getCurrentSessionId(): string | null {
    return this.currentSessionId;
  }

  /**
   * Returns the currently active thread id for diagnostics.
   */
  getCurrentThreadId(): string | null {
    return this.currentThreadId;
  }

  /**
   * Get or create state for (sessionId, threadId) pair.
   * ✅ Lazy initialization: state is only created when a tool is called
   */
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

  /**
   * Gets the current session's state, or creates a default session if none is set.
   * @returns The EphemeralState for the current (sessionId, threadId).
   */
  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) {
      // Fallback to default session
      this.setSession('default');
    }
    const effectiveThreadId = this.currentThreadId || this.currentSessionId!;
    return this.getState(this.currentSessionId!, effectiveThreadId);
  }

  createGoal(goal: string): MCPResponse<CreateGoalOutput> {
    return this.getCurrentState().createGoal(goal);
  }

  clearGoal(): MCPResponse<ClearGoalOutput> {
    return this.getCurrentState().clearGoal();
  }

  addTodo(name: string): MCPResponse<AddToDoOutput> {
    return this.getCurrentState().addTodo(name);
  }

  // toggleTodo removed; use remove_todo / clear_todos / add_todo instead

  clearTodos(ids?: number[]): MCPResponse<BaseOutput> {
    return this.getCurrentState().clearTodos(ids);
  }

  clear(): MCPResponse<BaseOutput> {
    return this.getCurrentState().clear();
  }

  getGoal(): string | null {
    return this.getCurrentState().getGoal();
  }

  getTodos(): SimpleTodo[] {
    return this.getCurrentState().getTodos();
  }

  addMemo(memo: string): MCPResponse<BaseOutput & { memos: Memo[] }> {
    return this.getCurrentState().addMemo(memo);
  }

  removeMemo(id: number): MCPResponse<BaseOutput & { memos: Memo[] }> {
    return this.getCurrentState().clearMemo(id);
  }

  getMemos(): Memo[] {
    return this.getCurrentState().getMemos();
  }

  getLastClearedGoal(): string | null {
    return this.getCurrentState().getLastClearedGoal();
  }

  processThought(input: unknown): MCPResponse<Record<string, unknown>> {
    return this.getCurrentState().processThought(input);
  }

  checkTodo(
    id: number,
    check: boolean = true,
    summary?: string,
  ): MCPResponse<CheckTodoOutput> {
    return this.getCurrentState().checkTodo(id, check, summary);
  }

  /**
   * Clears all session states. Useful for testing or complete reset.
   */
  clearAllSessions(): void {
    // ✅ Cleanup all sessions and their threads
    for (const [sessionId, threadMap] of this.sessions.entries()) {
      threadMap.clear();
      this.sessions.delete(sessionId);
    }
    this.currentSessionId = null;
    this.currentThreadId = null;
  }

  /**
   * Gets the planning state for a specific session/thread without changing current context.
   * This allows querying other sessions' states without side effects.
   * @param sessionId The session ID to query
   * @param threadId The thread ID to query (optional, defaults to sessionId)
   * @returns The planning state for the requested session/thread
   */
  getStateForSession(
    sessionId: string,
    threadId?: string,
  ): PlanningState | null {
    const effectiveThreadId = threadId || sessionId;
    const threadMap = this.sessions.get(sessionId);
    if (!threadMap) {
      return null; // Session does not exist
    }
    const state = threadMap.get(effectiveThreadId);
    if (!state) {
      return null; // Thread does not exist
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

// Simplified tool definitions - flat schemas for Gemini API compatibility
const tools: MCPTool[] = [
  {
    name: 'create_goal',
    description:
      'Create a single goal for the session. Use when starting a new or complex task.',
    inputSchema: {
      type: 'object',
      properties: {
        goal: {
          type: 'string',
          description:
            'The goal text to set for the session (e.g., "Complete project setup").',
        },
      },
      required: ['goal'],
    },
  },
  {
    name: 'clear_goal',
    description:
      'Clear the current goal. Use when finishing or abandoning the current goal.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_todo',
    description:
      'Add a todo item to the goal. Use to break down a goal into actionable steps.',
    inputSchema: {
      type: 'object',
      properties: {
        name: {
          type: 'string',
          description:
            'The name or description of the todo item to add (e.g., "Write documentation").',
        },
      },
      required: ['name'],
    },
  },
  {
    name: 'mark_todo',
    description:
      'Mark a todo item as completed or pending by its ID, optionally with a completion summary.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 1,
          description: 'The ID of the todo to update',
        },
        completed: {
          type: 'boolean',
          description:
            'Whether to mark the todo as completed (true) or pending (false). Defaults to true.',
        },
        summary: {
          type: 'string',
          description:
            'Optional summary or completion note for the todo (e.g., "Completed with PR #42").',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'clear_todos',
    description:
      'Clear specific todos by their IDs, or all todos if no IDs are provided. Use to remove completed tasks or reset the todo list.',
    inputSchema: {
      type: 'object',
      properties: {
        ids: {
          type: 'array',
          items: { type: 'number', minimum: 1 },
          description:
            'Array of todo IDs to clear. If not provided or empty, all todos will be cleared.',
        },
      },
    },
  },
  {
    name: 'clear_session',
    description:
      'Clear all session state (goal, todos, and notes). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_memo',
    description:
      'Add a memo to the session. Memos are temporary records, observations, or context information. Each memo is assigned a unique ID for reference.',
    inputSchema: {
      type: 'object',
      properties: {
        memo: {
          type: 'string',
          description:
            'The memo text to add (e.g., "User requested feature X").',
        },
      },
      required: ['memo'],
    },
  },
  {
    name: 'clear_memo',
    description: 'Clear a memo from the session by its ID.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 0,
          description: 'The ID of the memo to clear.',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'get_current_state',
    description:
      'Get current planning state as structured JSON data for UI visualization',
    inputSchema: { type: 'object', properties: {} },
  },
  // Sequential thinking tool - adapted from thinking_seq.ts
  {
    name: 'sequentialthinking',
    description:
      'Sequential thinking tool for multi-step reflective problem solving. Accepts a thought payload and maintains per-session thought history and branches.',
    inputSchema: {
      type: 'object',
      properties: {
        thought: { type: 'string', description: 'Your current thinking step' },
        nextThoughtNeeded: {
          type: 'boolean',
          description: 'Whether another thought step is needed',
        },
        thoughtNumber: { type: 'integer', minimum: 1 },
        totalThoughts: { type: 'integer', minimum: 1 },
        isRevision: { type: 'boolean' },
        revisesThought: { type: 'integer', minimum: 1 },
        branchFromThought: { type: 'integer', minimum: 1 },
        branchId: { type: 'string' },
        needsMoreThoughts: { type: 'boolean' },
      },
      required: [
        'thought',
        'nextThoughtNeeded',
        'thoughtNumber',
        'totalThoughts',
      ],
    },
  },
];

// Planning server interface for better type safety
interface PlanningServerMethods {
  create_goal: (args: {
    goal: string;
  }) => Promise<MCPResponse<CreateGoalOutput>>;
  clear_goal: () => Promise<MCPResponse<ClearGoalOutput>>;
  add_todo: (args: { name: string }) => Promise<MCPResponse<AddToDoOutput>>;
  mark_todo: (args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }) => Promise<MCPResponse<CheckTodoOutput>>;
  clear_todos: (args?: { ids?: number[] }) => Promise<MCPResponse<BaseOutput>>;
  clear_session: () => Promise<MCPResponse<BaseOutput>>;
  add_memo: (args: {
    memo: string;
  }) => Promise<MCPResponse<BaseOutput & { memos: Memo[] }>>;
  remove_memo: (args: {
    id: number;
  }) => Promise<MCPResponse<BaseOutput & { memos: Memo[] }>>;
  get_current_state: () => Promise<MCPResponse<PlanningState>>;
}

/**
 * The implementation of the `WebMCPServer` interface for the planning service.
 * It defines the server's metadata and its `callTool` and `getServiceContext` methods.
 */
const planningServer: WebMCPServer & { methods?: PlanningServerMethods } = {
  name: 'planning',
  version: '2.2.0',
  description:
    'Ephemeral planning and goal management for AI agents with note-taking and completion summaries',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // Debug logging for tool calls
    console.log(`[PlanningServer] callTool invoked: ${name}`, {
      args,
      currentSessionId: stateManager.getCurrentSessionId(),
      currentThreadId: stateManager.getCurrentThreadId(),
    });

    const typedArgs = (args as Record<string, unknown>) || {};

    // If caller included a sessionId in the tool args, do NOT implicitly
    // switch the server session. Accepting session changes via tool args is
    // an implicit API contract and can lead to race conditions, inconsistent
    // state, and security surprises. Clients should call the explicit
    // `switchContext`/`setContext` API to change sessions before invoking
    // tools. Here we log a warning if a sessionId is present and ignore it.
    if (typeof typedArgs.sessionId === 'string' && typedArgs.sessionId) {
      console.warn(
        `[PlanningServer] callTool: sessionId provided in args ("${String(
          typedArgs.sessionId,
        )}") - ignored. Use switchContext/setContext to change sessions.`,
      );
    }
    // Similarly, ignore threadId if provided in args
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
      case 'clear_goal': {
        return stateManager.clearGoal();
      }
      case 'add_todo': {
        return stateManager.addTodo(typedArgs.name as string);
      }
      case 'mark_todo': {
        const id = typedArgs.id as number;
        const completed =
          typedArgs.completed !== undefined
            ? (typedArgs.completed as boolean)
            : true;
        const summary = typedArgs.summary as string | undefined;

        if (!Number.isInteger(id) || id < 1) {
          return createMCPTextResponse(
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
          return createMCPTextResponse(
            `Invalid ID: ${id}. ID must be a non-negative integer.`,
          );
        }
        return stateManager.removeMemo(id);
      }
      case 'sequentialthinking': {
        return stateManager.processThought(typedArgs);
      }
      case 'get_current_state': {
        const currentState: PlanningState = {
          goal: stateManager.getGoal(),
          lastClearedGoal: stateManager.getLastClearedGoal(),
          todos: stateManager.getTodos(),
          memos: stateManager.getMemos(),
        };

        // Provide a human-readable Markdown summary that includes the
        // important fields. This is friendlier than raw JSON for text-only
        // consumers (LLMs or UI previews) while the structuredContent still
        // contains the typed object.
        const todosText = currentState.todos.length
          ? currentState.todos
              .map((t) => {
                const checkbox = t.status === 'completed' ? '✓' : ' ';
                const summaryPart = t.summary ? ` - ${t.summary}` : '';
                return `- ID:${t.id} [${checkbox}] ${t.name}${summaryPart}`;
              })
              .join('\n')
          : '- (none)';

        // Sanitize notes: replace newlines with spaces to maintain Markdown formatting
        const notesText = currentState.memos.length
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
        lines.push(
          '',
          '**Todos**',
          todosText,
          '',
          '**Recent Notes**',
          notesText,
        );

        const detailedText = lines.join('\n');

        return createMCPStructuredResponse<PlanningState>(
          detailedText,
          currentState,
        );
      }
      default: {
        const availableTools = tools.map((t) => t.name).join(', ');
        const errorMessage = `Unknown tool: ${name}. Available tools: ${availableTools}`;
        console.error(`[PlanningServer] ${errorMessage}`);
        throw new Error(errorMessage);
      }
    }
  },
  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<PlanningState>> {
    // If options are provided with sessionId, query that specific session's state
    // without changing the current context or triggering cleanup
    let planningState: PlanningState | null = null;

    if (options?.sessionId) {
      // Query a specific session's state without side effects
      planningState = stateManager.getStateForSession(
        options.sessionId,
        options.threadId,
      );
      if (!planningState) {
        logger.debug('getServiceContext: requested session/thread not found', {
          sessionId: options.sessionId,
          threadId: options.threadId,
        });
        // Return empty state if session doesn't exist
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
      // Use current context
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
      // ✅ This triggers cleanup of previous session's all threads
      stateManager.setSession(sessionId, threadId);
      console.info(
        `[PlanningServer] switchContext -> session: ${sessionId}, thread: ${threadId || sessionId} (previous session cleaned up)`,
      );
    }
  },
};

/**
 * Extends the `WebMCPServerProxy` with typed methods for the planning server's tools.
 * This provides a strongly-typed client for interacting with the planning server.
 */
export interface PlanningServerProxy extends WebMCPServerProxy {
  create_goal: (args: { goal: string }) => Promise<CreateGoalOutput>;
  clear_goal: () => Promise<ClearGoalOutput>;
  add_todo: (args: { name: string }) => Promise<AddToDoOutput>;
  mark_todo: (args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }) => Promise<CheckTodoOutput>;
  clear_todos: (args?: { ids?: number[] }) => Promise<BaseOutput>;
  clear_session: () => Promise<BaseOutput>;
  add_memo: (args: { memo: string }) => Promise<BaseOutput & { memos: Memo[] }>;
  clear_memo: (args: { id: number }) => Promise<BaseOutput & { memos: Memo[] }>;
  get_current_state: () => Promise<PlanningState>;
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
