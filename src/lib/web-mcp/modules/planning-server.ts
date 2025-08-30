import type { MCPTool, WebMCPServer, MCPResponse } from '@/lib/mcp-types';
import { normalizeToolResult } from '@/lib/mcp-types';

// Ephemeral 상태 관리 (메모리 기반)
interface Goal {
  id: number;
  name: string;
  description?: string;
  status: 'active' | 'completed' | 'paused';
  createdAt: Date;
}

interface Todo {
  id: number;
  name: string;
  description?: string;
  status: 'pending' | 'in_progress' | 'completed';
  createdAt: Date;
}

interface Observation {
  id: number;
  content: string;
  tags?: string[];
  createdAt: Date;
}

interface SequentialThinking {
  thoughtNumber: number;
  totalThoughts: number;
  thought: string;
  nextThoughtNeeded: boolean;
  createdAt: Date;
}

class EphemeralState {
  private goal: Goal | null = null;
  private todos: Todo[] = [];
  private observations: Observation[] = [];
  private sequentialThinking: SequentialThinking[] = [];

  createGoal(name: string, description?: string): Goal {
    const id = 0;
    this.goal = {
      id,
      name,
      description,
      status: 'active',
      createdAt: new Date(),
    };
    return this.goal;
  }

  updateGoal(
    updates: Partial<Pick<Goal, 'name' | 'description' | 'status'>>,
  ): Goal | null {
    if (!this.goal) return null;
    Object.assign(this.goal, updates);
    return this.goal;
  }

  addTodo(
    name: string,
    description?: string,
  ): { todoId: number; todos: Todo[] } {
    const id = this.todos.length;
    const todo: Todo = {
      id,
      name,
      description,
      status: 'pending',
      createdAt: new Date(),
    };
    this.todos.push(todo);
    return { todoId: todo.id, todos: this.todos };
  }

  updateTodo(
    todoId: number,
    updates: Partial<Pick<Todo, 'name' | 'description' | 'status'>>,
  ): { todo: Todo | null; todos: Todo[] } {
    const todo = this.todos[todoId];
    if (!todo) return { todo: null, todos: this.todos };
    Object.assign(todo, updates);
    return { todo, todos: this.todos };
  }

  listTodos(): Todo[] {
    return this.todos;
  }

  addObservation(
    content: string,
    tags?: string[],
  ): { observationId: number; observations: Observation[] } {
    if (this.observations.length >= 10) {
      this.observations.shift(); // Remove oldest observation
    }
    const id = this.observations.length;
    const observation: Observation = {
      id,
      content,
      tags,
      createdAt: new Date(),
    };
    this.observations.push(observation);
    return { observationId: observation.id, observations: this.observations };
  }

  addSequentialThinking(
    thought: string,
    thoughtNumber: number,
    totalThoughts: number,
    nextThoughtNeeded: boolean,
  ): SequentialThinking {
    const thinking: SequentialThinking = {
      thoughtNumber,
      totalThoughts,
      thought,
      nextThoughtNeeded,
      createdAt: new Date(),
    };
    this.sequentialThinking.push(thinking);
    return thinking;
  }

  getObservations(): Observation[] {
    return this.observations;
  }

  getSequentialThinking(): SequentialThinking[] {
    return this.sequentialThinking;
  }

  clear(): void {
    this.goal = null;
    this.todos = [];
    this.observations = [];
    this.sequentialThinking = [];
  }

  getGoal(): Goal | null {
    return this.goal;
  }
}

const state = new EphemeralState();

// Tool definitions
const tools: MCPTool[] = [
  {
    name: 'create_goal',
    description: 'Create a single goal for the session.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string' },
        description: { type: 'string' },
      },
      required: ['name'],
    },
  },
  {
    name: 'update_goal',
    description: 'Update the current goal and return the final state.',
    inputSchema: {
      type: 'object',
      properties: {
        updates: {
          type: 'object',
          properties: {
            name: { type: 'string' },
            description: { type: 'string' },
            status: { type: 'string', enum: ['active', 'completed', 'paused'] },
          },
        },
      },
      required: ['updates'],
    },
  },
  {
    name: 'add_todo',
    description: 'Add a todo item to the goal.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string' },
        description: { type: 'string' },
      },
      required: ['name'],
    },
  },
  {
    name: 'update_todo',
    description: 'Update a todo item and return the final state.',
    inputSchema: {
      type: 'object',
      properties: {
        todoId: { type: 'number' },
        updates: {
          type: 'object',
          properties: {
            name: { type: 'string' },
            description: { type: 'string' },
            status: {
              type: 'string',
              enum: ['pending', 'in_progress', 'completed'],
            },
          },
        },
      },
      required: ['todoId', 'updates'],
    },
  },
  {
    name: 'list_todos',
    description: 'List all current todo items.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'add_observation',
    description:
      'Record important facts and observations discovered during work/planning/execution. Maximum 10 observations stored.',
    inputSchema: {
      type: 'object',
      properties: {
        content: { type: 'string' },
        tags: { type: 'array', items: { type: 'string' } },
      },
      required: ['content'],
    },
  },
  {
    name: 'sequential_thinking',
    description:
      'Record step-by-step thoughts for complex problem solving, tracking each stage of reasoning.',
    inputSchema: {
      type: 'object',
      properties: {
        thought: { type: 'string' },
        thoughtNumber: { type: 'number' },
        totalThoughts: { type: 'number' },
        nextThoughtNeeded: { type: 'boolean' },
      },
      required: [
        'thought',
        'thoughtNumber',
        'totalThoughts',
        'nextThoughtNeeded',
      ],
    },
  },
  {
    name: 'clear_session',
    description: 'Clear all session state (goal, todos, and notes).',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
];

const planningServer: WebMCPServer = {
  name: 'planning-server',
  version: '1.0.0',
  description:
    'Ephemeral planning, thinking, and goal management for AI agents',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse> {
    const typedArgs = args as Record<string, unknown>;
    switch (name) {
      case 'create_goal':
        return normalizeToolResult(
          state.createGoal(
            typedArgs.name as string,
            typedArgs.description as string,
          ),
          'create_goal',
        );
      case 'update_goal':
        return normalizeToolResult(
          state.updateGoal(
            typedArgs.updates as Partial<
              Pick<Goal, 'name' | 'description' | 'status'>
            >,
          ),
          'update_goal',
        );
      case 'add_todo':
        return normalizeToolResult(
          state.addTodo(
            typedArgs.name as string,
            typedArgs.description as string,
          ),
          'add_todo',
        );
      case 'update_todo':
        return normalizeToolResult(
          state.updateTodo(
            typedArgs.todoId as number,
            typedArgs.updates as Partial<
              Pick<Todo, 'name' | 'description' | 'status'>
            >,
          ),
          'update_todo',
        );
      case 'list_todos':
        return normalizeToolResult(state.listTodos(), 'list_todos');
      case 'add_observation':
        return normalizeToolResult(
          state.addObservation(
            typedArgs.content as string,
            typedArgs.tags as string[],
          ),
          'add_observation',
        );
      case 'sequential_thinking':
        return normalizeToolResult(
          state.addSequentialThinking(
            typedArgs.thought as string,
            typedArgs.thoughtNumber as number,
            typedArgs.totalThoughts as number,
            typedArgs.nextThoughtNeeded as boolean,
          ),
          'sequential_thinking',
        );
      case 'clear_session':
        state.clear();
        return normalizeToolResult({ success: true }, 'clear_session');
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  },
  async getServiceContext(): Promise<string> {
    const goal = state.getGoal();
    const todos = state.listTodos();
    const observations = state.getObservations();
    const sequentialThinking = state.getSequentialThinking();

    const goalText = goal ? `Current Goal: ${goal.name}` : 'No active goal';
    const activeTodos = todos.filter((t) => t.status !== 'completed');
    const todosText =
      activeTodos.length > 0
        ? `Active Todos: ${activeTodos.map((t) => t.name).join(', ')}`
        : 'No active todos';
    const observationsText =
      observations.length > 0
        ? `Observations: ${observations.map((o) => o.content.substring(0, 50) + '...').join('; ')}`
        : 'No observations';
    const thinkingText =
      sequentialThinking.length > 0
        ? `Sequential Thinking: ${sequentialThinking[sequentialThinking.length - 1].thought.substring(0, 50)}...`
        : 'No sequential thinking';

    return `
# Instruction
AI Agents have memory limitations (Context Window), so complex or long-term tasks must be recorded as goals, todos, and observations.
Use the tools below to manage goals, todos, observations, and reasoning.

# Tools
- create_goal: Create a goal
- add_todo: Add a todo item
- add_observation: Record important facts (max 10 items)
- sequential_thinking: Record step-by-step reasoning

# Context Information
${goalText}
${todosText}
${observationsText}
${thinkingText}

# Prompt
Select appropriate tools to suggest or execute the next action based on the current situation.
    `.trim();
  },
};

export default planningServer;
