import type { MCPTool, WebMCPServer, MCPResponse, MCPContent } from '@/lib/mcp-types';
import { normalizeToolResult } from '@/lib/mcp-types';
import { createUIResource } from '@mcp-ui/server';

// Simplified data structures for Gemini API compatibility
interface SimpleTodo {
  name: string;
  status: 'pending' | 'completed';
}

class EphemeralState {
  private goal: string | null = null;
  private todos: SimpleTodo[] = [];

  createGoal(goal: string): string {
    this.goal = goal;
    return this.goal;
  }

  clearGoal(): { success: boolean } {
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
    const todo = this.todos[index];
    if (!todo) return { todo: null, todos: this.todos };

    todo.status = todo.status === 'completed' ? 'pending' : 'completed';
    return { todo, todos: this.todos };
  }

  clearTodos(): { success: boolean } {
    this.todos = [];
    return { success: true };
  }

  clear(): void {
    this.goal = null;
    this.todos = [];
  }

  getGoal(): string | null {
    return this.goal;
  }

  getTodos(): SimpleTodo[] {
    return this.todos;
  }
}

const state = new EphemeralState();

/**
 * Generate HTML content for promptUser tool
 */
function generatePromptHTML(params: {
  question: string;
  type: string;
  options?: string[];
}): string {
  const { question, type, options } = params;

  let inputSection = '';
  switch (type) {
    case 'yesno':
      inputSection = `
        <div style="margin-top: 16px;">
          <button onclick="window.parent.postMessage({type: 'prompt', payload: {prompt: 'yes'}}, '*')" 
                  style="margin-right: 12px; padding: 8px 16px; background: #2563eb; color: white; border: none; border-radius: 6px; cursor: pointer;">
            Yes
          </button>
          <button onclick="window.parent.postMessage({type: 'prompt', payload: {prompt: 'no'}}, '*')" 
                  style="padding: 8px 16px; background: #6b7280; color: white; border: none; border-radius: 6px; cursor: pointer;">
            No
          </button>
        </div>
      `;
      break;
    case 'options':
      if (options && options.length > 0) {
        inputSection = `
          <div style="margin-top: 16px;">
            ${options
              .map(
                (option) => `
              <button onclick="window.parent.postMessage({type: 'prompt', payload: {prompt: '${option}'}}, '*')" 
                      style="display: block; width: 100%; margin-bottom: 8px; padding: 8px 16px; background: #f3f4f6; border: 1px solid #d1d5db; border-radius: 6px; cursor: pointer; text-align: left;">
                ${option}
              </button>
            `,
              )
              .join('')}
          </div>
        `;
      }
      break;
    case 'text':
      inputSection = `
        <div style="margin-top: 16px;">
          <input type="text" id="textInput" placeholder="Type your response..." 
                 style="width: 100%; padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; margin-bottom: 8px;">
          <button onclick="
                    const input = document.getElementById('textInput');
                    if (input.value.trim()) {
                      window.parent.postMessage({type: 'prompt', payload: {prompt: input.value}}, '*');
                    }
                  " 
                  style="padding: 8px 16px; background: #2563eb; color: white; border: none; border-radius: 6px; cursor: pointer;">
            Submit
          </button>
        </div>
      `;
      break;
  }

  return `
    <div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 500px; margin: 0; padding: 20px; line-height: 1.5;">
      <div style="margin-bottom: 16px; padding: 16px; background: #f8fafc; border-left: 4px solid #2563eb; border-radius: 6px;">
        <h3 style="margin: 0 0 8px 0; color: #1e40af;">❓ User Input Required</h3>
        <div style="color: #1f2937; font-size: 16px;">${question}</div>
      </div>
      ${inputSection}
    </div>
  `;
}

/**
 * Create UIResource for promptUser tool
 */
function createPromptUIResource(params: {
  question: string;
  type: string;
  options?: string[];
}) {
  const htmlContent = generatePromptHTML(params);

  return createUIResource({
    uri: `ui://prompt/${Date.now()}`,
    content: {
      type: 'rawHtml',
      htmlString: htmlContent,
    },
    encoding: 'text',
  });
}

// Simplified tool definitions - flat schemas for Gemini API compatibility
const tools: MCPTool[] = [
  {
    name: 'create_goal',
    description:
      'Create a single goal for the session. Use when starting a new or complex task.',
    inputSchema: {
      type: 'object',
      properties: {
        goal: { type: 'string' },
      },
      required: ['goal'],
    },
  },
  {
    name: 'clear_goal',
    description:
      'Clear the current goal. Use when finishing or abandoning the current goal.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'add_todo',
    description:
      'Add a todo item to the goal. Use to break down a goal into actionable steps.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string' },
      },
      required: ['name'],
    },
  },
  {
    name: 'toggle_todo',
    description:
      'Toggle a todo between pending and completed status using its index.',
    inputSchema: {
      type: 'object',
      properties: {
        index: { type: 'number' },
      },
      required: ['index'],
    },
  },
  {
    name: 'clear_todos',
    description:
      'Clear all todo items. Use when resetting or finishing all tasks.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'clear_session',
    description:
      'Clear all session state (goal and todos). Use to reset everything and start fresh.',
    inputSchema: {
      type: 'object',
      properties: {},
    },
  },
  {
    name: 'promptUser',
    description:
      'Ask the user for additional information during task execution',
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string' },
        type: { type: 'string', enum: ['yesno', 'options', 'text'] },
        options: { type: 'array', items: { type: 'string' } },
      },
      required: ['question', 'type'],
    },
  },
];

const planningServer: WebMCPServer = {
  name: 'planning-server',
  version: '2.0.0',
  description:
    'Simplified ephemeral planning and goal management for AI agents with Gemini API compatibility',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse> {
    const typedArgs = args as Record<string, unknown>;
    switch (name) {
      case 'create_goal': {
        const result = state.createGoal(typedArgs.goal as string);
        return normalizeToolResult(`Goal created: "${result}"`, 'create_goal');
      }
      case 'clear_goal': {
        const result = state.clearGoal();
        return normalizeToolResult(
          result.success ? 'Goal cleared successfully' : 'Failed to clear goal',
          'clear_goal',
        );
      }
      case 'add_todo': {
        const result = state.addTodo(typedArgs.name as string);
        return normalizeToolResult(
          result.success
            ? `Todo added: "${typedArgs.name}" (Total: ${result.todos.length})`
            : 'Failed to add todo',
          'add_todo',
        );
      }
      case 'toggle_todo': {
        const result = state.toggleTodo(typedArgs.index as number);
        if (result.todo) {
          return normalizeToolResult(
            `Todo "${result.todo.name}" marked as ${result.todo.status}`,
            'toggle_todo',
          );
        } else {
          return normalizeToolResult(
            `Todo at index ${typedArgs.index} not found`,
            'toggle_todo',
          );
        }
      }
      case 'clear_todos': {
        const result = state.clearTodos();
        return normalizeToolResult(
          result.success ? 'All todos cleared' : 'Failed to clear todos',
          'clear_todos',
        );
      }
      case 'clear_session':
        state.clear();
        return normalizeToolResult('Session state cleared', 'clear_session');
      case 'promptUser': {
        const params = typedArgs as {
          question: string;
          type: string;
          options?: string[];
        };
        const uiResource: MCPContent  = createPromptUIResource(params);
        
        const baseResponse = normalizeToolResult(
          {
            success: true,
            question: params.question,
            type: params.type,
            options: params.options,
          },
          'promptUser',
        );

        if (baseResponse.result?.content) {
          baseResponse.result.content.unshift(
            uiResource
          );
        }

        return baseResponse;
      }
      default:
        throw new Error(`Unknown tool: ${name}`);
    }
  },
  async getServiceContext(): Promise<string> {
    const goal = state.getGoal();
    const todos = state.getTodos();

    const goalText = goal ? `Current Goal: ${goal}` : 'No active goal';
    const activeTodos = todos.filter((t) => t.status !== 'completed');
    const todosText =
      activeTodos.length > 0
        ? `Active Todos: ${activeTodos.map((t) => t.name).join(', ')}`
        : 'No active todos';

    return `
# Instruction
You have memory limitations (Context Window). For complex or long-term tasks, record and manage your goals and actionable steps.
Organize and update your objectives, break down tasks as needed.

# Context Information
${goalText}
${todosText}

# Prompt
Based on the current situation, determine and suggest the next appropriate action to progress toward your objectives.
  `.trim();
  },
};

export default planningServer;
