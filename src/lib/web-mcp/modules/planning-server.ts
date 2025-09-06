import type { MCPTool, WebMCPServer, MCPResponse } from '@/lib/mcp-types';
import { normalizeToolResult } from '@/lib/mcp-types';
import { createUIResource } from '@mcp-ui/server';

// Simplified data structures for Gemini API compatibility
interface SimpleTodo {
  name: string;
  status: 'pending' | 'completed';
}

type PromptType = "options" | "text"

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

// Argument type for promptUser tool
type PromptUserArgs = {
  question: string;
  type: PromptType;
  options?: string[];
};

/**
 * Validate promptUser arguments
 */
function validatePromptUserArgs(args: Record<string, unknown>): PromptUserArgs {
  if (!args.question || typeof args.question !== 'string') {
    throw new Error('question is required and must be a string');
  }

  if (!args.type || typeof args.type !== 'string') {
    throw new Error('type is required and must be a string');
  }

  const validTypes: PromptType[] = ['options', 'text'];
  if (!validTypes.includes(args.type as PromptType)) {
    throw new Error(`type must be one of: ${validTypes.join(', ')}`);
  }

  if (args.type === 'options') {
    if (!args.options || !Array.isArray(args.options)) {
      throw new Error('options array is required when type is "options"');
    }
    if (args.options.length === 0) {
      throw new Error('options array cannot be empty');
    }
    if (!args.options.every((option) => typeof option === 'string')) {
      throw new Error('all options must be strings');
    }
  }

  return args as PromptUserArgs;
}

/**
 * [NEW] Generate Remote DOM script for the promptUser tool.
 * This script will be executed on the client-side to build the UI dynamically.
 */
function generatePromptRemoteDomScript(params: PromptUserArgs): string {
  // Safely serialize parameters to be embedded in the script string
  const question = JSON.stringify(params.question);
  const type = JSON.stringify(params.type);
  const options = JSON.stringify(params.options || []);

  return `
    // Helper function to create an element with styles
    function createElement(tag, styles, textContent) {
      const el = document.createElement(tag);
      if (textContent) el.textContent = textContent;
      Object.assign(el.style, styles);
      return el;
    }

    // Main container
    const container = createElement('div', {
      fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif",
      maxWidth: '500px',
      margin: '0',
      padding: '20px',
      lineHeight: '1.5',
    });

    // Header section with the question
    const header = createElement('div', {
      marginBottom: '16px',
      padding: '16px',
      background: '#f8fafc',
      borderLeft: '4px solid #2563eb',
      borderRadius: '6px',
    });

    const title = createElement('h3', { margin: '0 0 8px 0', color: '#1e40af' }, '❓ User Input Required');
    const questionDiv = createElement('div', { color: '#1f2937', fontSize: '16px' }, ${question});
    
    header.appendChild(title);
    header.appendChild(questionDiv);
    container.appendChild(header);
    
    // Input section that varies by type
    const inputContainer = createElement('div', { marginTop: '16px' });
    const typeValue = ${type};
    const optionsValue = ${options};
    
    switch (typeValue) {
      case 'yesno': {
        const yesButton = createElement('button', {
          marginRight: '12px', padding: '8px 16px', background: '#2563eb', color: 'white', border: 'none', borderRadius: '6px', cursor: 'pointer'
        }, 'Yes');
        yesButton.addEventListener('click', () => window.parent.postMessage({ type: 'prompt', payload: { prompt: 'yes' } }, '*'));

        const noButton = createElement('button', {
          padding: '8px 16px', background: '#6b7280', color: 'white', border: 'none', borderRadius: '6px', cursor: 'pointer'
        }, 'No');
        noButton.addEventListener('click', () => window.parent.postMessage({ type: 'prompt', payload: { prompt: 'no' } }, '*'));

        inputContainer.appendChild(yesButton);
        inputContainer.appendChild(noButton);
        break;
      }
      case 'options': {
        optionsValue.forEach(optionText => {
          const optionButton = createElement('button', {
            display: 'block', width: '100%', marginBottom: '8px', padding: '8px 16px', background: '#f3f4f6', border: '1px solid #d1d5db', borderRadius: '6px', cursor: 'pointer', textAlign: 'left'
          }, optionText);
          optionButton.addEventListener('click', () => window.parent.postMessage({ type: 'prompt', payload: { prompt: optionText } }, '*'));
          inputContainer.appendChild(optionButton);
        });
        break;
      }
      case 'text': {
        const textInput = createElement('input', {
          width: 'calc(100% - 24px)', padding: '8px 12px', border: '1px solid #d1d5db', borderRadius: '6px', marginBottom: '8px'
        });
        textInput.type = 'text';
        textInput.placeholder = 'Type your response...';
        
        const submitButton = createElement('button', {
          padding: '8px 16px', background: '#2563eb', color: 'white', border: 'none', borderRadius: '6px', cursor: 'pointer'
        }, 'Submit');
        submitButton.addEventListener('click', () => {
          if (textInput.value.trim()) {
            window.parent.postMessage({ type: 'prompt', payload: { prompt: textInput.value } }, '*');
          }
        });
        
        inputContainer.appendChild(textInput);
        inputContainer.appendChild(submitButton);
        break;
      }
    }
    
    container.appendChild(inputContainer);
    // Attach the fully constructed UI to the root element provided by the client
    root.appendChild(container);
  `;
}


/**
 * [MODIFIED] Create UIResource for promptUser tool using Remote DOM.
 */
function createPromptUIResource(params: PromptUserArgs) {
  // Generate the JavaScript script instead of HTML
  const remoteDomScript = generatePromptRemoteDomScript(params);

  return createUIResource({
    uri: `ui://prompt/${Date.now()}`,
    content: {
      type: 'remoteDom', // Use 'remoteDom' type
      script: remoteDomScript, // Pass the script content
      framework: 'webcomponents', // Indicates vanilla JS, no framework needed
    },
    encoding: 'text',
  });
}

// Simplified tool definitions - flat schemas for Gemini API compatibility
const tools: MCPTool[] = [
    // ... (other tools remain the same) ...
  {
    name: 'create_goal',
    description: 'Create a single goal for the session. Use when starting a new or complex task.',
    inputSchema: { type: 'object', properties: { goal: { type: 'string' } }, required: ['goal'] },
  },
  {
    name: 'clear_goal',
    description: 'Clear the current goal. Use when finishing or abandoning the current goal.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'add_todo',
    description: 'Add a todo item to the goal. Use to break down a goal into actionable steps.',
    inputSchema: { type: 'object', properties: { name: { type: 'string' } }, required: ['name'] },
  },
  {
    name: 'toggle_todo',
    description: 'Toggle a todo between pending and completed status using its index.',
    inputSchema: { type: 'object', properties: { index: { type: 'number' } }, required: ['index'] },
  },
  {
    name: 'clear_todos',
    description: 'Clear all todo items. Use when resetting or finishing all tasks.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'clear_session',
    description: 'Clear all session state (goal and todos). Use to reset everything and start fresh.',
    inputSchema: { type: 'object', properties: {} },
  },
  {
    name: 'promptUser',
    description: 'Ask the user for additional information during task execution',
    inputSchema: {
      type: 'object',
      properties: {
        question: { type: 'string' },
        // [MODIFIED] Added 'yesno' to the schema to match implementation
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
  description: 'Simplified ephemeral planning and goal management for AI agents with Gemini API compatibility',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResponse> {
    const typedArgs = args as Record<string, unknown>;
    switch (name) {
      // ... (other tool cases remain the same) ...
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
        // This part remains the same, but it now calls the new remote-dom functions
        const params = validatePromptUserArgs(typedArgs);
        const uiResource = createPromptUIResource(params);
        const baseResponse = normalizeToolResult(
          { success: true, ...params },
          'promptUser',
        );

        baseResponse.result?.content?.unshift(uiResource);
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