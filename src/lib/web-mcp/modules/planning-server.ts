import type { MCPTool, WebMCPServer, MCPResponse } from '@/lib/mcp-types';
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
 * Goal과 Todo 현황을 HTML로 생성
 */
function generateGoalTodosHTML(
  goal: string | null,
  todos: SimpleTodo[],
): string {
  const goalSection = goal
    ? `
      <div class="goal-section" style="margin-bottom: 16px; padding: 12px; border-left: 4px solid #2563eb; background: #f8fafc;">
        <h3 style="margin: 0 0 8px 0; color: #1e40af;">🎯 Current Goal</h3>
        <div style="font-weight: bold; color: #1f2937;">${goal}</div>
      </div>
    `
    : `
      <div class="goal-section" style="margin-bottom: 16px; padding: 12px; border-left: 4px solid #d1d5db; background: #f9fafb;">
        <h3 style="margin: 0; color: #6b7280;">🎯 No Active Goal</h3>
      </div>
    `;

  const todosSection = `
    <div class="todos-section">
      <h3 style="margin: 0 0 12px 0; color: #1f2937;">📋 Todo List (${todos.length})</h3>
      ${
        todos.length === 0
          ? '<div style="color: #6b7280; font-style: italic;">No todos yet</div>'
          : todos
              .map(
                (todo, index) => `
          <div style="margin-bottom: 8px; padding: 8px; border: 1px solid #e5e7eb; border-radius: 6px; background: #ffffff;">
            <div style="display: flex; align-items: center; gap: 8px;">
              <span style="font-size: 12px; color: #6b7280;">#${index}</span>
              <span style="padding: 2px 6px; border-radius: 8px; font-size: 11px; background: ${
                todo.status === 'completed'
                  ? '#dcfce7; color: #166534'
                  : '#f3f4f6; color: #374151'
              };">
                ${todo.status}
              </span>
              <span style="font-weight: 500; color: #1f2937;">${todo.name}</span>
            </div>
          </div>
        `,
              )
              .join('')
      }
    </div>
  `;

  return `
    <div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; max-width: 600px; margin: 0; padding: 16px; line-height: 1.5;">
      <h2 style="margin: 0 0 16px 0; color: #1f2937; border-bottom: 2px solid #e5e7eb; padding-bottom: 8px;">
        📊 Goals & Todos Overview
      </h2>
      ${goalSection}
      ${todosSection}
      <div style="margin-top: 16px; padding: 8px; background: #f3f4f6; border-radius: 6px; font-size: 12px; color: #6b7280;">
        Last updated: ${new Date().toLocaleString()}
      </div>
    </div>
  `;
}

/**
 * Goal/Todo UIResource 생성 - @mcp-ui/server 표준 사용
 */
function createGoalTodosUIResource(goal: string | null, todos: SimpleTodo[]) {
  const htmlContent = generateGoalTodosHTML(goal, todos);

  return createUIResource({
    uri: `ui://planning/overview/${Date.now()}`,
    content: {
      type: 'rawHtml',
      htmlString: htmlContent,
    },
    encoding: 'text',
  });
}

/**
 * UIResource를 포함한 Tool 결과 정규화
 */
function normalizeToolResultWithUI(
  result: unknown,
  toolName: string,
  uiResource?: unknown,
): MCPResponse {
  const baseResponse = normalizeToolResult(result, toolName);

  if (uiResource && baseResponse.result?.content) {
    // @mcp-ui/server createUIResource returns { type: "resource", resource: {...} }
    // We can use it directly since it already has the correct structure
    baseResponse.result.content.unshift(
      uiResource as {
        type: 'resource';
        resource: {
          uri: string;
          mimeType: string;
          text: string;
        };
      },
    );
  }

  return baseResponse;
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
        const uiResource = createGoalTodosUIResource(
          state.getGoal(),
          state.getTodos(),
        );
        return normalizeToolResultWithUI(result, 'create_goal', uiResource);
      }
      case 'clear_goal': {
        const result = state.clearGoal();
        const uiResource = createGoalTodosUIResource(
          state.getGoal(),
          state.getTodos(),
        );
        return normalizeToolResultWithUI(result, 'clear_goal', uiResource);
      }
      case 'add_todo': {
        const result = state.addTodo(typedArgs.name as string);
        const uiResource = createGoalTodosUIResource(
          state.getGoal(),
          state.getTodos(),
        );
        return normalizeToolResultWithUI(result, 'add_todo', uiResource);
      }
      case 'toggle_todo': {
        const result = state.toggleTodo(typedArgs.index as number);
        const uiResource = createGoalTodosUIResource(
          state.getGoal(),
          state.getTodos(),
        );
        return normalizeToolResultWithUI(result, 'toggle_todo', uiResource);
      }
      case 'clear_todos': {
        const result = state.clearTodos();
        const uiResource = createGoalTodosUIResource(
          state.getGoal(),
          state.getTodos(),
        );
        return normalizeToolResultWithUI(result, 'clear_todos', uiResource);
      }
      case 'clear_session':
        state.clear();
        return normalizeToolResult({ success: true }, 'clear_session');
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
