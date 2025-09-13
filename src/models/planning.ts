import type { WebMCPServerProxy } from '@/hooks/use-web-mcp-server';

// Re-export SimpleTodo interface from planning server to keep models centralized
export interface SimpleTodo {
  name: string;
  status: 'pending' | 'completed';
}

export interface PlanningState {
  goal: string | null;
  lastClearedGoal: string | null;
  todos: SimpleTodo[];
  observations: string[];
}

export interface PlanningServerProxy extends WebMCPServerProxy {
  create_goal: (args: { goal: string }) => Promise<string>;
  clear_goal: () => Promise<{ success: boolean }>;
  add_todo: (args: {
    name: string;
  }) => Promise<{ success: boolean; todos: SimpleTodo[] }>;
  toggle_todo: (args: {
    index: number;
  }) => Promise<{ todo: SimpleTodo | null; todos: SimpleTodo[] }>;
  clear_todos: () => Promise<{ success: boolean }>;
  clear_session: () => Promise<void>;
  add_observation: (args: { observation: string }) => Promise<void>;
  get_current_state: () => Promise<PlanningState>;
}
