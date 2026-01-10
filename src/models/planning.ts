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

/** Represents a scratchpad item for storing notes and context */
export interface ScratchpadItem {
  id: number;
  title?: string;
  content: string;
  tags?: string[];
  source?: string;
}

/** Represents a single thought in the sequential-thinking tool */
export interface ThoughtData {
  thought: string;
  thoughtNumber: number;
  totalThoughts: number;
  isRevision?: boolean;
  revisesThought?: number;
  branchFromThought?: number;
  branchId?: string;
  needsMoreThoughts?: boolean;
  nextThoughtNeeded: boolean;
  category?: string;
  relatedTodoId?: number;
  nextAction?: string;
}

/** Represents a single reflection entry */
export interface ReflectionData {
  critique: string;
  reflection: string;
  nextAction: string;
}

/**
 * Complete planning state for a session.
 * This is returned as structured_state in ServiceContext.
 */
export interface PlanningState {
  goal: string | null;
  todos: SimpleTodo[];
  scratchpad: ScratchpadItem[];
  thoughts: ThoughtData[];
  reflections: ReflectionData[];
  lastUpdated: string;
}

/**
 * Planning state metadata for display in UI
 */
export interface PlanningMetadata {
  totalTodos: number;
  completedTodos: number;
  activeTodos: number;
  scratchpadCount: number;
  thoughtCount: number;
  reflectionCount: number;
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
      scratchpadCount: 0,
      thoughtCount: 0,
      reflectionCount: 0,
    };
  }

  const totalTodos = state.todos.length;
  const completedTodos = state.todos.filter((t) => t.checked).length;
  const activeTodos = totalTodos - completedTodos;

  return {
    totalTodos,
    completedTodos,
    activeTodos,
    scratchpadCount: state.scratchpad?.length || 0,
    thoughtCount: state.thoughts?.length || 0,
    reflectionCount: state.reflections?.length || 0,
  };
}
