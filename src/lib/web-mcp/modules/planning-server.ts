import { getLogger } from '@/lib/logger';
import type { MCPTool, WebMCPServer } from '@/lib/mcp-types';

const logger = getLogger('planning-server');

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

interface ThinkNote {
  id: number;
  content: string;
  tags?: string[];
  createdAt: Date;
}

class EphemeralState {
  private goal: Goal | null = null;
  private todos: Todo[] = [];
  private notes: ThinkNote[] = [];

  createGoal(name: string, description?: string): Goal {
    const id = 0;
    this.goal = {
      id,
      name,
      description,
      status: 'active',
      createdAt: new Date(),
    };
    logger.info('Goal created', { goalId: this.goal.id });
    return this.goal;
  }

  updateGoal(updates: Partial<Pick<Goal, 'name' | 'description' | 'status'>>): Goal | null {
    if (!this.goal) return null;
    Object.assign(this.goal, updates);
    logger.info('Goal updated', { goalId: this.goal.id, updates });
    return this.goal;
  }

  addTodo(name: string, description?: string): { todoId: number; todos: Todo[] } {
    const id = this.todos.length;
    const todo: Todo = {
      id,
      name,
      description,
      status: 'pending',
      createdAt: new Date(),
    };
    this.todos.push(todo);
    logger.info('Todo added', { todoId: todo.id });
    return { todoId: todo.id, todos: this.todos };
  }

  updateTodo(todoId: number, updates: Partial<Pick<Todo, 'name' | 'description' | 'status'>>): { todo: Todo | null; todos: Todo[] } {
    const todo = this.todos[todoId];
    if (!todo) return { todo: null, todos: this.todos };
    Object.assign(todo, updates);
    logger.info('Todo updated', { todoId, updates });
    return { todo, todos: this.todos };
  }

  listTodos(): Todo[] {
    return this.todos;
  }

  addNote(content: string, tags?: string[]): { noteId: number; notes: ThinkNote[] } {
    const id = this.notes.length;
    const note: ThinkNote = {
      id,
      content,
      tags,
      createdAt: new Date(),
    };
    this.notes.push(note);
    logger.info('Note added', { noteId: note.id });
    return { noteId: note.id, notes: this.notes };
  }

  clear(): void {
    this.goal = null;
    this.todos = [];
    this.notes = [];
    logger.info('Session cleared');
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
            status: { type: 'string', enum: ['pending', 'in_progress', 'completed'] },
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
    name: 'think_note',
    description: 'Record thoughts or notes during the session.',
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
  description: 'Ephemeral planning, thinking, and goal management for AI agents',
  tools,
  async callTool(name: string, args: unknown): Promise<unknown> {
    const typedArgs = args as Record<string, unknown>;
    switch (name) {
      case 'create_goal':
        return state.createGoal(typedArgs.name as string, typedArgs.description as string);
      case 'update_goal':
        return state.updateGoal(typedArgs.updates as Partial<Pick<Goal, 'name' | 'description' | 'status'>>);
      case 'add_todo':
        return state.addTodo(typedArgs.name as string, typedArgs.description as string);
      case 'update_todo':
        return state.updateTodo(typedArgs.todoId as number, typedArgs.updates as Partial<Pick<Todo, 'name' | 'description' | 'status'>>);
      case 'list_todos':
        return state.listTodos();
      case 'think_note':
        return state.addNote(typedArgs.content as string, typedArgs.tags as string[]);
      case 'clear_session':
        state.clear();
        return { success: true };
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  },
};

export default planningServer;