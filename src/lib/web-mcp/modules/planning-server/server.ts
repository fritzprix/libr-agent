import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { WebMCPServerProxy } from '@/context/WebMCPContext';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { planningTools as tools } from './tools.ts';
import { SessionStateManager } from './state';
import {
  resolveToolName,
  logDeprecationWarning,
} from '@/lib/web-mcp/tool-name-migration';
import type {
  PlanningState,
  CreateGoalOutput,
  ClearGoalOutput,
  AddToDoOutput,
  CheckTodoOutput,
  BaseOutput,
  ScratchpadItem,
} from './types';

const stateManager = new SessionStateManager();

const planningServer: WebMCPServer = {
  name: 'planning',
  displayName: 'Task Planning',
  description: 'Goal setting, task planning',
  version: '2.2.0',
  tools,
  async callTool(name: string, args: unknown): Promise<MCPResult<unknown>> {
    // Handle tool name migration (backwards compatibility)
    const { resolvedName, isDeprecated } = resolveToolName(name);

    if (isDeprecated) {
      logDeprecationWarning(name, resolvedName);
    }

    console.log(`[PlanningServer] callTool invoked: ${resolvedName}`, {
      originalName: name !== resolvedName ? name : undefined,
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
    switch (resolvedName) {
      case 'createGoal': {
        return await stateManager.createGoal(typedArgs.goal as string);
      }
      case 'updateGoal': {
        return await stateManager.updateGoal(typedArgs.goal as string);
      }
      case 'clearGoal': {
        return await stateManager.clearGoal();
      }
      case 'addTodo': {
        if (!typedArgs.title || typeof typedArgs.title !== 'string') {
          return createMCPErrorToolResult(
            'The "title" argument is required and must be a string.',
          );
        }
        return await stateManager.addTodo(
          typedArgs.title as string,
          typedArgs.description as string | undefined,
          typedArgs.priority as 'low' | 'medium' | 'high' | undefined,
        );
      }
      case 'checkTodo': {
        const id = typedArgs.id as number | undefined;
        const index = typedArgs.index as number | undefined;
        const checked =
          typedArgs.checked !== undefined
            ? (typedArgs.checked as boolean)
            : true;
        const summary = typedArgs.summary as string | undefined;

        // Validate that at least one identifier is provided
        if (id === undefined && index === undefined) {
          return createMCPErrorToolResult(
            'Either "id" or "index" must be provided.',
          );
        }

        // Validate id if provided
        if (id !== undefined && (!Number.isInteger(id) || id < 1)) {
          return createMCPErrorToolResult(
            `Invalid id: ${id}. ID must be a positive integer.`,
          );
        }

        // Validate index if provided
        if (index !== undefined && (!Number.isInteger(index) || index < 0)) {
          return createMCPErrorToolResult(
            `Invalid index: ${index}. Index must be a non-negative integer.`,
          );
        }

        return await stateManager.checkTodo({ id, index }, checked, summary);
      }
      case 'clearTodos': {
        const ids = typedArgs.ids as number[] | undefined;
        return await stateManager.clearTodos(ids);
      }
      case 'clearSession':
        return await stateManager.clear();
      case 'addScratchpad': {
        return await stateManager.addScratchpad(
          typedArgs.note as string,
          typedArgs.source as string | undefined,
          typedArgs.title as string | undefined,
          typedArgs.tags as string[] | undefined,
        );
      }
      case 'readScratchpad': {
        return await stateManager.readScratchpad(
          typedArgs.ids as number[] | undefined,
          typedArgs.tags as string[] | undefined,
        );
      }
      case 'clearScratchpad': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 0) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a non-negative integer.`,
          );
        }
        return await stateManager.clearScratchpad(id);
      }
      case 'pauseAndThink': {
        return stateManager.processPauseAndThink(typedArgs);
      }
      case 'sequentialthinking': {
        return stateManager.processThought(typedArgs);
      }
      case 'critiqueAndReflection': {
        return stateManager.processCritiqueAndReflection(typedArgs);
      }
      case 'getCurrentState': {
        const includeChecked = typedArgs.include_checked !== false; // Default true
        const includeScratchpad = typedArgs.include_scratchpad !== false; // Default true
        const state = await stateManager.getStateForSession(
          stateManager.getCurrentSessionId() || 'default',
          stateManager.getCurrentThreadId() || 'default',
        );

        if (!state) {
          return createMCPStructuredToolResult('No active state found.', {
            success: false,
          });
        }

        const { goal, todos, scratchpad } = state;
        const filteredTodos = includeChecked
          ? todos
          : todos.filter((t) => !t.checked);

        // Pagination logic for todos (optional, keeping simple for now)
        const limit = 50;
        const offset = 0;
        const paginatedTodos = filteredTodos.slice(offset, offset + limit);

        const todosText = paginatedTodos.length
          ? paginatedTodos
              .map((t) => {
                const checkbox = t.checked ? '[x]' : '[ ]';

                const summaryPart = t.summary ? ` - ${t.summary}` : '';
                const priorityPart = t.priority ? ` [${t.priority}]` : '';
                return `- ID:${t.id} ${checkbox} ${t.title}${priorityPart}${summaryPart}`;
              })
              .join('\n')
          : '(none)';

        const scratchpadText =
          includeScratchpad && scratchpad.length > 0
            ? scratchpad
                .map((m) => {
                  const titlePart = m.title ? ` [${m.title}]` : '';
                  const tagsPart =
                    Array.isArray(m.tags) && m.tags.length > 0
                      ? ` (tags: ${m.tags.join(', ')})`
                      : '';
                  const contentPreview = m.title
                    ? ''
                    : ` ${m.content.slice(0, 50)}${
                        m.content.length > 50 ? '...' : ''
                      }`;
                  return `- [ID: ${m.id}]${titlePart}${contentPreview}${tagsPart}`;
                })
                .join('\n')
            : '(none)';

        const goalText = goal ? `- ${goal}` : '(none)';

        const outputText = `# Planning State

**Summary**
- Total Todos: ${todos.length}
  - Unchecked: ${todos.filter((t) => !t.checked).length}
  - Checked: ${todos.filter((t) => t.checked).length}
- Scratchpad Items: ${scratchpad.length}

**Goal**
${goalText}

**Todos**
${todosText}

**Scratchpad**
${scratchpadText}`;

        return createMCPStructuredToolResult(outputText, {
          success: true,
          state: {
            goal,
            todos: filteredTodos,
            scratchpad: includeScratchpad ? scratchpad : [],
          },
        });
      }
    }

    return createMCPErrorToolResult(`Unknown tool: ${name}`);
  },

  async getServiceContext(
    options?: ServiceContextOptions,
  ): Promise<ServiceContext<PlanningState>> {
    const sessionId = options?.sessionId || 'default';
    const threadId = options?.threadId || sessionId;

    // Ensure session is set
    stateManager.setSession(sessionId, threadId);

    // We can use getStateForSession to get the state even if we just switched
    const state = await stateManager.getStateForSession(sessionId, threadId);

    if (!state) {
      // Should not happen as setSession creates it
      return {
        contextPrompt: 'Error: Could not retrieve planning state.',
        structuredState: {
          goal: null,
          lastClearedGoal: null,
          todos: [],
          scratchpad: [],
        },
      };
    }

    const { goal, todos, scratchpad } = state;
    const uncheckedTodos = todos.filter((t) => !t.checked);

    const contextParts = [];
    if (goal) {
      contextParts.push(`Current Goal: "${goal}"`);
    }
    if (uncheckedTodos.length > 0) {
      contextParts.push(`Unchecked Todos (${uncheckedTodos.length}):`);
      uncheckedTodos.slice(0, 5).forEach((t) => {
        contextParts.push(`- [ ] ${t.title}`);
      });
      if (uncheckedTodos.length > 5) {
        contextParts.push(`...and ${uncheckedTodos.length - 5} more`);
      }
    }
    if (scratchpad.length > 0) {
      contextParts.push(`Scratchpad (${scratchpad.length}):`);
      scratchpad.slice(0, 5).forEach((m) => {
        const titlePart = m.title ? `[${m.title}] ` : '';
        const tagsPart =
          Array.isArray(m.tags) && m.tags.length > 0
            ? ` (tags: ${m.tags.join(', ')})`
            : '';
        const contentPreview = m.title ? '' : `${m.content.slice(0, 30)}...`;
        contextParts.push(
          `- ID:${m.id} ${titlePart}${contentPreview}${tagsPart}`,
        );
      });
      if (scratchpad.length > 5) {
        contextParts.push(
          `...and ${
            scratchpad.length - 5
          } more. Use readScratchpad to view details.`,
        );
      }
    }

    return {
      contextPrompt:
        contextParts.length > 0
          ? contextParts.join('\n')
          : 'No active plan or todos.',
      structuredState: state,
    };
  },

  async switchContext(options: ServiceContextOptions): Promise<void> {
    const sessionId = options.sessionId || 'default';
    const threadId = options.threadId || sessionId;
    stateManager.setSession(sessionId, threadId);
  },
};

export interface PlanningServerProxy extends WebMCPServerProxy {
  createGoal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  updateGoal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  clearGoal(): Promise<MCPResult<ClearGoalOutput>>;
  addTodo(args: {
    title: string;
    description?: string;
    priority?: 'low' | 'medium' | 'high';
  }): Promise<MCPResult<AddToDoOutput>>;
  updateTodo(args: {
    id: number;
    title?: string;
    status?: 'pending' | 'completed' | 'blocked';
    priority?: 'low' | 'medium' | 'high';
  }): Promise<MCPResult<CheckTodoOutput>>;
  markTodo(args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }): Promise<MCPResult<CheckTodoOutput>>;
  clearTodos(args: { ids?: number[] }): Promise<MCPResult<BaseOutput>>;
  clearSession(): Promise<MCPResult<BaseOutput>>;
  addScratchpad(args: {
    note: string;
    title?: string;
    tags?: string[];
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
  readScratchpad(args: {
    ids?: number[];
    tags?: string[];
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
  clearScratchpad(args: {
    id: number;
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
  getCurrentState(args: {
    include_completed?: boolean;
    include_scratchpad?: boolean;
  }): Promise<MCPResult<unknown>>;
  sequentialthinking(args: unknown): Promise<MCPResult<unknown>>;
  critiqueAndReflection(args: unknown): Promise<MCPResult<unknown>>;
}

export default planningServer;
