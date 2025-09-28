import { WebMCPServerProxy } from '@/context/WebMCPContext';
import {
  createMCPStructuredResponse,
  createMCPTextResponse,
} from '@/lib/mcp-response-utils';
import type { MCPResponse, MCPTool, WebMCPServer } from '@/lib/mcp-types';

/** Represents a single to-do item in the planning state. @internal */
interface SimpleTodo {
  id: number;
  name: string;
  status: 'pending' | 'completed';
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
  /** A list of recent observations or events. */
  observations: string[];
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
 * The output for the `toggle_todo` tool call.
 * @internal
 */
interface ToggleTodoOutput extends BaseOutput {
  todo: SimpleTodo | null;
  todos: SimpleTodo[];
}

const MAX_OBSERVATIONS = 10;

/**
 * Manages the in-memory state for the planning server, including goals,
 * to-dos, and observations. This state is not persisted and will be lost
 * when the worker is terminated.
 * @internal
 */
class EphemeralState {
  private goal: string | null = null;
  private lastClearedGoal: string | null = null;
  private todos: SimpleTodo[] = [];
  private observations: string[] = [];
  private nextId = 1;

  createGoal(goal: string): MCPResponse<CreateGoalOutput> {
    this.goal = goal;
    return createMCPStructuredResponse<CreateGoalOutput>(
      `Goal created: "${goal}"`,
      {
        goal,
        success: true,
      },
    );
  }

  clearGoal(): MCPResponse<ClearGoalOutput> {
    if (this.goal) {
      this.lastClearedGoal = this.goal;
      return createMCPStructuredResponse<ClearGoalOutput>(
        'Goal cleared successfully',
        {
          success: true,
        },
      );
    }
    this.goal = null;
    return createMCPStructuredResponse('No Goal to clear', { success: false });
  }

  addTodo(name: string): MCPResponse<AddToDoOutput> {
    const todo: SimpleTodo = {
      id: this.nextId++,
      name,
      status: 'pending',
    };
    this.todos.push(todo);
    return createMCPStructuredResponse<AddToDoOutput>(`Todo added: "${name}"`, {
      success: true,
      todos: this.todos,
    });
  }

  toggleTodo(id: number): MCPResponse<ToggleTodoOutput> {
    const todo = this.todos.find((t) => t.id === id);
    if (!todo) {
      const availableIds = this.todos.map((t) => t.id);
      return createMCPStructuredResponse<ToggleTodoOutput>(
        `Todo with ID ${id} not found. Available IDs: ${availableIds.length > 0 ? availableIds.join(', ') : 'none'}`,
        {
          success: false,
          todo: null,
          todos: this.todos,
        },
      );
    }

    todo.status = todo.status === 'completed' ? 'pending' : 'completed';

    // // History management: keep only 2 completed todos maximum
    // if (todo.status === 'completed') {
    //   this.manageCompletedTodoHistory();
    // }

    return createMCPStructuredResponse(
      `Todo "${todo.name}" marked as ${todo.status}`,
      {
        success: true,
        todo,
        todos: this.todos,
      },
    );
  }

  clearTodos(): MCPResponse<BaseOutput> {
    this.todos = [];
    return createMCPStructuredResponse<BaseOutput>('All todos cleared', {
      success: true,
    });
  }

  clear(): MCPResponse<BaseOutput> {
    this.goal = null;
    this.lastClearedGoal = null;
    this.todos = [];
    this.observations = [];
    this.nextId = 1;
    return createMCPStructuredResponse('Session state cleared', {
      success: true,
    });
  }

  getGoal(): string | null {
    return this.goal;
  }

  getTodos(): SimpleTodo[] {
    return this.todos;
  }

  addObservation(observation: string): MCPResponse<BaseOutput> {
    this.observations.push(observation);
    if (this.observations.length > MAX_OBSERVATIONS) {
      this.observations.shift();
    }
    return createMCPStructuredResponse<BaseOutput>(
      'Observation added to session',
      { success: true },
    );
  }

  getObservations(): string[] {
    return [...this.observations];
  }

  getLastClearedGoal(): string | null {
    return this.lastClearedGoal;
  }
}

/**
 * Session-based state manager that maintains separate EphemeralState instances
 * for each session ID.
 * @internal
 */
class SessionStateManager {
  private sessions = new Map<string, EphemeralState>();
  private currentSessionId: string | null = null;

  /**
   * Sets the current session context.
   * @param sessionId The session ID to set as current context.
   */
  setSession(sessionId: string): void {
    this.currentSessionId = sessionId;
    // Initialize session state if it doesn't exist
    if (!this.sessions.has(sessionId)) {
      this.sessions.set(sessionId, new EphemeralState());
    }
  }

  /**
   * Gets the current session's state, or creates a default session if none is set.
   * @returns The EphemeralState for the current session.
   */
  private getCurrentState(): EphemeralState {
    if (!this.currentSessionId) {
      // Fallback to default session
      this.setSession('default');
    }
    return this.sessions.get(this.currentSessionId!)!;
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

  toggleTodo(id: number): MCPResponse<ToggleTodoOutput> {
    return this.getCurrentState().toggleTodo(id);
  }

  clearTodos(): MCPResponse<BaseOutput> {
    return this.getCurrentState().clearTodos();
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

  addObservation(observation: string): MCPResponse<BaseOutput> {
    return this.getCurrentState().addObservation(observation);
  }

  getObservations(): string[] {
    return this.getCurrentState().getObservations();
  }

  getLastClearedGoal(): string | null {
    return this.getCurrentState().getLastClearedGoal();
  }

  /**
   * Clears all session states. Useful for testing or complete reset.
   */
  clearAllSessions(): void {
    this.sessions.clear();
    this.currentSessionId = null;
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
    name: 'toggle_todo',
    description:
      'Toggle a todo between pending and completed status using its unique ID.',
    inputSchema: {
      type: 'object',
      properties: {
        id: {
          type: 'number',
          minimum: 1,
          description:
            'The unique ID of the todo to toggle (use the ID shown in the todos list)',
        },
      },
      required: ['id'],
    },
  },
  {
    name: 'clear_todos',
    description:
      'Clear all todo items. Use when resetting or finishing all tasks.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'clear_session',
    description:
      'Clear all session state (goal, todos, and observations). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_observation',
    description:
      'Add a new observation to the session. Observations are recent events, user feedback, or system messages.',
    inputSchema: {
      type: 'object',
      properties: {
        observation: {
          type: 'string',
          description:
            'The observation text to add (e.g., "User requested feature X").',
        },
      },
      required: ['observation'],
    },
  },
  {
    name: 'get_current_state',
    description:
      'Get current planning state as structured JSON data for UI visualization',
    inputSchema: { type: 'object', properties: {} },
  },
];

// Planning server interface for better type safety
interface PlanningServerMethods {
  create_goal: (args: {
    goal: string;
  }) => Promise<MCPResponse<CreateGoalOutput>>;
  clear_goal: () => Promise<MCPResponse<ClearGoalOutput>>;
  add_todo: (args: { name: string }) => Promise<MCPResponse<AddToDoOutput>>;
  toggle_todo: (args: { id: number }) => Promise<MCPResponse<ToggleTodoOutput>>;
  clear_todos: () => Promise<MCPResponse<BaseOutput>>;
  clear_session: () => Promise<MCPResponse<BaseOutput>>;
  add_observation: (args: {
    observation: string;
  }) => Promise<MCPResponse<BaseOutput>>;
  get_current_state: () => Promise<MCPResponse<PlanningState>>;
}

/**
 * The implementation of the `WebMCPServer` interface for the planning service.
 * It defines the server's metadata and its `callTool` and `getServiceContext` methods.
 */
const planningServer: WebMCPServer & { methods?: PlanningServerMethods } = {
  name: 'planning',
  version: '2.1.0',
  description:
    'Ephemeral planning and goal management for AI agents with bounded observation queue',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse<unknown>> {
    // Debug logging for tool calls
    console.log(`[PlanningServer] callTool invoked: ${name}`, args);

    const typedArgs = args as Record<string, unknown>;
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
      case 'toggle_todo': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 1) {
          return createMCPTextResponse(
            `Invalid ID: ${id}. ID must be a positive integer.`,
          );
        }

        return stateManager.toggleTodo(id);
      }
      case 'clear_todos': {
        return stateManager.clearTodos();
      }
      case 'clear_session':
        return stateManager.clear();
      case 'add_observation': {
        return stateManager.addObservation(typedArgs.observation as string);
      }
      case 'get_current_state': {
        const currentState = {
          goal: stateManager.getGoal(),
          lastClearedGoal: stateManager.getLastClearedGoal(),
          todos: stateManager.getTodos(),
          observations: stateManager.getObservations(),
        };

        return createMCPStructuredResponse<PlanningState>(
          `Current planning state: ${currentState.todos.length} todos, ${currentState.observations.length} observations`,
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
  async getServiceContext(): Promise<string> {
    const goal = stateManager.getGoal();
    const todos = stateManager.getTodos();
    const observations = stateManager.getObservations();

    const goalText = goal ? `Current Goal: ${goal}` : 'No active goal';
    const lastGoalText = stateManager.getLastClearedGoal()
      ? `Last Cleared Goal: ${stateManager.getLastClearedGoal()}`
      : '';

    // Display all todos in order, accurately representing their status
    const todosText =
      todos.length > 0
        ? `Todos:\n${todos
            .map((t) => {
              const checkbox = t.status === 'completed' ? '[✓]' : '[ ]';
              return `  ID:${t.id} ${checkbox} ${t.name}`;
            })
            .join('\n')}`
        : 'Todos: (none)';

    const obsText =
      observations.length > 0
        ? `Recent Observations:\n${observations
            .map((obs, idx) => `  ${idx + 1}. ${obs}`)
            .join('\n')}`
        : 'Recent Observations: (none)';

    return `
# Instruction
ALWAYS START BY CREATING A PLAN before beginning any task:
1. First, create a clear goal using 'create_goal' for any new or complex task
2. Break down the goal into specific, actionable todos using 'add_todo'
3. Execute todos step by step, marking them complete with 'toggle_todo' using ID
4. Record important observations, user feedback, or results with 'add_observation'
5. Use memory limitations as an opportunity to organize and structure your work

Remember: Planning prevents poor performance. Always plan before you act.

# Context Information
${goalText}
${lastGoalText ? `\n${lastGoalText}` : ''}

${todosText}

${obsText}

# Prompt
Based on the current situation, determine and suggest the next appropriate action to progress toward your objectives. If no goal exists, start by creating one.
Use 'toggle_todo' with the ID number (not index) to mark todos as complete/incomplete.
  `.trim();
  },
  async setContext(context: Record<string, unknown>): Promise<void> {
    const sessionId = context.sessionId as string;
    if (sessionId) {
      stateManager.setSession(sessionId);
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
  toggle_todo: (args: { index: number }) => Promise<ToggleTodoOutput>;
  clear_todos: () => Promise<BaseOutput>;
  clear_session: () => Promise<BaseOutput>;
  add_observation: (args: { observation: string }) => Promise<BaseOutput>;
  get_current_state: () => Promise<PlanningState>;
}

export default planningServer;
