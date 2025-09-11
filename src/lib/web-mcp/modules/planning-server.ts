import type { MCPTool, WebMCPServer, MCPResponse } from '@/lib/mcp-types';
import { createMCPTextResponse } from '@/lib/mcp-response-utils';

// Simplified data structures for Gemini API compatibility
interface SimpleTodo {
  name: string;
  status: 'pending' | 'completed';
}

const MAX_OBSERVATIONS = 10;

class EphemeralState {
  private goal: string | null = null;
  private lastClearedGoal: string | null = null;
  private todos: SimpleTodo[] = [];
  private observations: string[] = [];

  createGoal(goal: string): string {
    this.goal = goal;
    return this.goal;
  }

  clearGoal(): { success: boolean } {
    if (this.goal) {
      this.lastClearedGoal = this.goal;
    }
    this.goal = null;
    return { success: true };
  }

  addTodo(name: string): { success: boolean; todos: SimpleTodo[] } {
    const todo: SimpleTodo = {
      name,
      status: 'pending',
    };
    this.todos.push(todo);
    return { success: true, todos: this.todos };
  }

  toggleTodo(index: number): { todo: SimpleTodo | null; todos: SimpleTodo[] } {
    // Adjust for 1-based user input (subtract 1 for 0-based array access)
    const adjustedIndex = index - 1;

    // Validate the adjusted index
    if (adjustedIndex < 0 || adjustedIndex >= this.todos.length) {
      return { todo: null, todos: this.todos };
    }

    const todo = this.todos[adjustedIndex];
    if (!todo) return { todo: null, todos: this.todos };

    todo.status = todo.status === 'completed' ? 'pending' : 'completed';

    // History management: keep only 2 completed todos maximum
    if (todo.status === 'completed') {
      this.manageCompletedTodoHistory();
    }

    return { todo, todos: this.todos };
  }

  clearTodos(): { success: boolean } {
    this.todos = [];
    return { success: true };
  }

  clear(): void {
    this.goal = null;
    this.lastClearedGoal = null;
    this.todos = [];
    this.observations = [];
  }

  getGoal(): string | null {
    return this.goal;
  }

  getTodos(): SimpleTodo[] {
    return this.todos;
  }

  addObservation(observation: string): void {
    this.observations.push(observation);
    if (this.observations.length > MAX_OBSERVATIONS) {
      this.observations.shift();
    }
  }

  getObservations(): string[] {
    return [...this.observations];
  }

  private manageCompletedTodoHistory(): void {
    const completedTodos = this.todos.filter((t) => t.status === 'completed');
    if (completedTodos.length > 2) {
      // Find and remove the oldest completed todo
      const firstCompletedIndex = this.todos.findIndex(
        (t) => t.status === 'completed',
      );
      if (firstCompletedIndex !== -1) {
        this.todos.splice(firstCompletedIndex, 1);
      }
    }
  }

  getLastClearedGoal(): string | null {
    return this.lastClearedGoal;
  }
}

const state = new EphemeralState();

// Simplified tool definitions - flat schemas for Gemini API compatibility
const tools: MCPTool[] = [
  // ... (other tools remain the same) ...
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
      'Toggle a todo between pending and completed status using its 1-based index (e.g., 1 for the first todo, 2 for the second).',
    inputSchema: {
      type: 'object',
      properties: {
        index: {
          type: 'number',
          minimum: 1,
          description:
            'The 1-based index of the todo to toggle (must be a positive integer)',
        },
      },
      required: ['index'],
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
  create_goal: (args: { goal: string }) => Promise<MCPResponse>;
  clear_goal: () => Promise<MCPResponse>;
  add_todo: (args: { name: string }) => Promise<MCPResponse>;
  toggle_todo: (args: { index: number }) => Promise<MCPResponse>;
  clear_todos: () => Promise<MCPResponse>;
  clear_session: () => Promise<MCPResponse>;
  add_observation: (args: { observation: string }) => Promise<MCPResponse>;
  get_current_state: () => Promise<MCPResponse>;
}

const planningServer: WebMCPServer & { methods?: PlanningServerMethods } = {
  name: 'planning',
  version: '2.1.0',
  description:
    'Ephemeral planning and goal management for AI agents with bounded observation queue',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse> {
    // Debug logging for tool calls
    console.log(`[PlanningServer] callTool invoked: ${name}`, args);

    const typedArgs = args as Record<string, unknown>;
    switch (name) {
      // ... (other tool cases remain the same) ...
      case 'create_goal': {
        const result = state.createGoal(typedArgs.goal as string);
        return createMCPTextResponse(`Goal created: "${result}"`);
      }
      case 'clear_goal': {
        const result = state.clearGoal();
        return createMCPTextResponse(
          result.success ? 'Goal cleared successfully' : 'Failed to clear goal',
        );
      }
      case 'add_todo': {
        const result = state.addTodo(typedArgs.name as string);
        return createMCPTextResponse(
          result.success
            ? `Todo added: "${typedArgs.name}" (Total: ${result.todos.length})`
            : 'Failed to add todo',
        );
      }
      case 'toggle_todo': {
        const index = typedArgs.index as number;
        if (!Number.isInteger(index) || index < 1) {
          return createMCPTextResponse(
            `Invalid index: ${index}. Index must be a positive integer (1-based).`,
          );
        }

        const result = state.toggleTodo(index);
        if (result.todo) {
          return createMCPTextResponse(
            `Todo "${result.todo.name}" marked as ${result.todo.status}`,
          );
        } else {
          return createMCPTextResponse(
            `Todo at index ${index} not found. Valid indices: 1 to ${result.todos.length}`,
          );
        }
      }
      case 'clear_todos': {
        const result = state.clearTodos();
        return createMCPTextResponse(
          result.success ? 'All todos cleared' : 'Failed to clear todos',
        );
      }
      case 'clear_session':
        state.clear();
        return createMCPTextResponse('Session state cleared');
      case 'add_observation': {
        state.addObservation(typedArgs.observation as string);
        return createMCPTextResponse('Observation added to session');
      }
      case 'get_current_state': {
        const currentState = {
          goal: state.getGoal(),
          lastClearedGoal: state.getLastClearedGoal(),
          todos: state.getTodos(),
          observations: state.getObservations(),
        };

        return {
          jsonrpc: '2.0',
          id: null,
          result: {
            content: [
              {
                type: 'text',
                text: JSON.stringify(currentState),
              },
            ],
          },
        };
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
    const goal = state.getGoal();
    const todos = state.getTodos();
    const observations = state.getObservations();

    const goalText = goal ? `Current Goal: ${goal}` : 'No active goal';
    const lastGoalText = state.getLastClearedGoal()
      ? `Last Cleared Goal: ${state.getLastClearedGoal()}`
      : '';

    const activeTodos = todos.filter((t) => t.status === 'pending');
    const completedTodos = todos.filter((t) => t.status === 'completed');

    const activeTodosText =
      activeTodos.length > 0
        ? `Active Todos:\n${activeTodos
            .map((t, idx) => `  ${idx + 1}. [ ] ${t.name}`)
            .join('\n')}`
        : 'Active Todos: (none)';

    const completedTodosText =
      completedTodos.length > 0
        ? `Recently Completed:\n${completedTodos
            .map((t, idx) => `  ${idx + 1}. [✔] ${t.name}`)
            .join('\n')}`
        : '';
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
3. Execute todos step by step, marking them complete with 'toggle_todo'
4. Record important observations, user feedback, or results with 'add_observation'
5. Use memory limitations as an opportunity to organize and structure your work

Remember: Planning prevents poor performance. Always plan before you act.

# Context Information
${goalText}
${lastGoalText ? `\n${lastGoalText}` : ''}

${activeTodosText}
${completedTodosText ? `\n${completedTodosText}` : ''}

${obsText}

# Prompt
Based on the current situation, determine and suggest the next appropriate action to progress toward your objectives. If no goal exists, start by creating one.
  `.trim();
  },
};

export default planningServer;
