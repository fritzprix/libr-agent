/**
 * Planning types for Agent V2
 * These types are used by both Rust backend planning server and frontend UI
 */

/** Represents a single to-do item in the planning state */
export interface SimpleTodo {
  id: number;
  title: string;
  description?: string;
  checked: boolean;
  summary?: string;
  priority?: 'low' | 'medium' | 'high';
  parentId?: number;
  subtasks?: SimpleTodo[];
}

/** Represents a todo with computed blocked/available status */
export interface TodoWithComputedState extends SimpleTodo {
  isBlocked: boolean;
  canStart: boolean;
}

/**
 * Complete planning state for a session.
 * This is returned as structured_state in ServiceContext from the planning server.
 */
export interface PlanningState {
  goal: string | null;
  todos: SimpleTodo[];
  lastUpdated?: string;
}

/**
 * Planning state metadata for display in UI
 */
export interface PlanningMetadata {
  totalTodos: number;
  completedTodos: number;
  activeTodos: number;
}

/**
 * Helper function to calculate planning metadata from state
 */
export function calculatePlanningMetadata(
  state: PlanningState | undefined,
): PlanningMetadata {
  if (!state) {
    return {
      totalTodos: 0,
      completedTodos: 0,
      activeTodos: 0,
    };
  }

  const totalTodos = state.todos.length;
  const completedTodos = state.todos.filter((t) => t.checked).length;
  const activeTodos = totalTodos - completedTodos;

  return {
    totalTodos,
    completedTodos,
    activeTodos,
  };
}
