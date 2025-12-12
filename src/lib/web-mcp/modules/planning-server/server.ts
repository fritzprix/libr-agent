import {
  createMCPStructuredToolResult,
  createMCPErrorToolResult,
} from '@/lib/mcp-response-utils';
import type { MCPResult, WebMCPServer } from '@/lib/mcp-types';
import type { WebMCPServerProxy } from '@/context/WebMCPContext';
import type { ServiceContext, ServiceContextOptions } from '@/features/tools';
import { getLogger } from '@/lib/logger';
import { planningTools as tools } from './tools.ts';
import { SessionStateManager } from './state';
import type {
  PlanningState,
  CreateGoalOutput,
  ClearGoalOutput,
  AddToDoOutput,
  CheckTodoOutput,
  BaseOutput,
  ScratchpadItem,
} from './types';

const logger = getLogger('PlanningServer');

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
        return await stateManager.createGoal(typedArgs.goal as string);
      }
      case 'update_goal': {
        return await stateManager.updateGoal(typedArgs.goal as string);
      }
      case 'clear_goal': {
        return await stateManager.clearGoal();
      }
      case 'add_todo': {
        return await stateManager.addTodo(
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
        return await stateManager.updateTodo(id, {
          name: typedArgs.name as string | undefined,
          status: typedArgs.status as
            | 'pending'
            | 'completed'
            | 'blocked'
            | undefined,
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
        return await stateManager.checkTodo(id, completed, summary);
      }
      case 'clear_todos': {
        const ids = typedArgs.ids as number[] | undefined;
        return await stateManager.clearTodos(ids);
      }
      case 'clear_session':
        return await stateManager.clear();
      case 'add_scratchpad': {
        return await stateManager.addScratchpad(
          typedArgs.note as string,
          typedArgs.source as string | undefined,
        );
      }
      case 'clear_scratchpad': {
        const id = typedArgs.id as number;
        if (!Number.isInteger(id) || id < 0) {
          return createMCPErrorToolResult(
            `Invalid ID: ${id}. ID must be a non-negative integer.`,
          );
        }
        return await stateManager.clearScratchpad(id);
      }
      case 'sequentialthinking': {
        return stateManager.processThought(typedArgs);
      }
      case 'get_current_state': {
        const includeCompleted = typedArgs.include_completed !== false; // Default true
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
        const filteredTodos = includeCompleted
          ? todos
          : todos.filter((t) => t.status === 'pending');

        // Pagination logic for todos (optional, keeping simple for now)
        const limit = 50;
        const offset = 0;
        const paginatedTodos = filteredTodos.slice(offset, offset + limit);

        const todosText = paginatedTodos.length
          ? paginatedTodos
              .map((t) => {
                let checkbox = '[ ]';
                if (t.status === 'completed') checkbox = '[x]';
                else if (t.status === 'blocked') checkbox = '[!]';

                const summaryPart = t.summary ? ` - ${t.summary}` : '';
                const priorityPart = t.priority ? ` [${t.priority}]` : '';
                const dependsPart =
                  t.dependsOn && t.dependsOn.length > 0
                    ? ` (depends on: ${t.dependsOn.join(', ')})`
                    : '';
                return `- ID:${t.id} ${checkbox} ${t.name}${priorityPart}${summaryPart}${dependsPart}`;
              })
              .join('\n')
          : '(none)';

        const scratchpadText =
          includeScratchpad && scratchpad.length > 0
            ? scratchpad
                .map((m) => {
                  const sourcePart = m.source ? ` (source: ${m.source})` : '';
                  return `- [ID: ${m.id}] ${m.content}${sourcePart}`;
                })
                .join('\n')
            : '(none)';

        const goalText = goal ? `- ${goal}` : '(none)';

        const outputText = `# Planning State

**Summary**
- Total Todos: ${todos.length}
  - Pending: ${todos.filter((t) => t.status === 'pending').length}
  - Completed: ${todos.filter((t) => t.status === 'completed').length}
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
    const pendingTodos = todos.filter((t) => t.status === 'pending');

    const contextParts = [];
    if (goal) {
      contextParts.push(`Current Goal: "${goal}"`);
    }
    if (pendingTodos.length > 0) {
      contextParts.push(`Pending Todos (${pendingTodos.length}):`);
      pendingTodos.slice(0, 5).forEach((t) => {
        contextParts.push(`- [ ] ${t.name}`);
      });
      if (pendingTodos.length > 5) {
        contextParts.push(`...and ${pendingTodos.length - 5} more`);
      }
    }
    if (scratchpad.length > 0) {
      contextParts.push(`Scratchpad (${scratchpad.length}):`);
      scratchpad.slice(0, 3).forEach((m) => {
        contextParts.push(`- ${m.content}`);
      });
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
    logger.info(
      `Switched planning context to session: ${sessionId}, thread: ${threadId}`,
    );
  },
};

export interface PlanningServerProxy extends WebMCPServerProxy {
  create_goal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  update_goal(args: { goal: string }): Promise<MCPResult<CreateGoalOutput>>;
  clear_goal(): Promise<MCPResult<ClearGoalOutput>>;
  add_todo(args: {
    name: string;
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }): Promise<MCPResult<AddToDoOutput>>;
  update_todo(args: {
    id: number;
    name?: string;
    status?: 'pending' | 'completed' | 'blocked';
    priority?: 'low' | 'medium' | 'high';
    dependsOn?: number[];
  }): Promise<MCPResult<CheckTodoOutput>>;
  mark_todo(args: {
    id: number;
    completed?: boolean;
    summary?: string;
  }): Promise<MCPResult<CheckTodoOutput>>;
  clear_todos(args: { ids?: number[] }): Promise<MCPResult<BaseOutput>>;
  clear_session(): Promise<MCPResult<BaseOutput>>;
  add_scratchpad(args: {
    note: string;
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
  clear_scratchpad(args: {
    id: number;
  }): Promise<MCPResult<BaseOutput & { scratchpad: ScratchpadItem[] }>>;
  get_current_state(args: {
    include_completed?: boolean;
    include_scratchpad?: boolean;
  }): Promise<MCPResult<unknown>>;
  sequentialthinking(args: unknown): Promise<MCPResult<unknown>>;
}

export default planningServer;
